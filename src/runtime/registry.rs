// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Device-scoped registry of sharecli-managed processes.
//!
//! Each `sharecli` invocation constructs its own [`ProcessPool`], whose handle
//! map is in-memory only. That makes the pool an authority for exactly one
//! process: `sharecli start` records a handle that the next `sharecli stop`
//! invocation cannot see, so the stop is a silent no-op on the device.
//!
//! This registry is the device-level authority that closes that gap. `spawn`
//! appends an entry, `kill` removes one, and `list` / `find` / `kill` on a fresh
//! pool read back what earlier invocations started.
//!
//! Scope and limits, stated plainly:
//!
//! - Every read-modify-write (`insert`, `remove`, `prune_dead`) runs under an
//!   exclusive advisory `flock` on a sidecar `processes.json.lock` and publishes
//!   through a per-writer temp file, so two invocations spawning at the same
//!   instant cannot drop one another's entry. The lock is advisory: a writer
//!   that bypasses this module can still race, and a resident supervisor would
//!   remove even that.
//! - Liveness is probed with `kill(pid, 0)`. `EPERM` counts as alive, a zombie
//!   counts as dead (on Linux), and pid 0 or anything above `i32::MAX` is never
//!   alive. A recycled PID is still possible in principle; the recorded
//!   `started_at` and `cmd` let a caller discriminate.
//! - A registry that exists but cannot be read or parsed is an error for every
//!   writer, so a transient read failure can never replace records it did not
//!   read. Read-only callers degrade to an empty view instead.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

/// One process recorded by a `sharecli` invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub pid: u32,
    pub project: Option<String>,
    pub harness: Option<String>,
    pub cmd: Vec<String>,
    /// Unix epoch seconds, used to tell a live process from a recycled PID.
    pub started_at: u64,
}

impl RegistryEntry {
    pub fn new(
        pid: u32,
        project: Option<String>,
        harness: Option<String>,
        cmd: Vec<String>,
    ) -> Self {
        Self { pid, project, harness, cmd, started_at: now_secs() }
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// True when `pid` is a value the OS can address. `kill(0, sig)` targets the
/// caller's whole process group, and an id above `i32::MAX` wraps negative when
/// cast to `pid_t`, so neither may ever reach a syscall.
fn signalable(pid: u32) -> bool {
    pid > 0 && pid <= i32::MAX as u32
}

/// True when `pid` is a zombie: it has exited but its parent has not reaped it.
/// `kill(pid, 0)` still succeeds for a zombie, so a liveness probe that only
/// checks the signal would call a finished workload alive forever in a container
/// whose PID 1 does not reap.
#[cfg(target_os = "linux")]
fn is_zombie(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    // Field 3 is the state, after `pid (comm)`. `comm` may itself contain spaces
    // and parentheses, so split after the LAST `)`.
    match stat.rfind(')') {
        Some(end) => matches!(stat[end + 1..].trim_start().chars().next(), Some('Z' | 'X' | 'x')),
        None => false,
    }
}

#[cfg(not(target_os = "linux"))]
fn is_zombie(_pid: u32) -> bool {
    false
}

/// True when `pid` exists. `kill(pid, 0)` performs the permission and existence
/// check without delivering a signal.
#[cfg(unix)]
pub fn pid_alive(pid: u32) -> bool {
    if !signalable(pid) {
        return false;
    }
    // SAFETY: signal 0 delivers nothing; it only validates the pid.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return !is_zombie(pid);
    }
    // `EPERM` means the process exists but belongs to another user: still alive.
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// True when `pid` exists. Windows has no `kill(pid, 0)`, so open the process
/// with the narrowest query right and read its exit code: a live, unreaped
/// process reports `STILL_ACTIVE`. An `ACCESS_DENIED` open means the pid exists
/// under another account, which is still alive (the `EPERM` analogue).
#[cfg(windows)]
pub fn pid_alive(pid: u32) -> bool {
    if !signalable(pid) {
        return false;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_ACCESS_DENIED: i32 = 5;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(handle: *mut std::ffi::c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    // SAFETY: standard Win32 handle lifecycle; every acquired handle is closed.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return std::io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED);
    }
    let mut code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    ok != 0 && code == STILL_ACTIVE
}

/// Tolerance when comparing a recorded spawn time against the live process
/// start time. The record is written just after the child starts, so the
/// recorded value is normally a little later than the true start; a pid that
/// has been recycled since is separated by far more than this.
const START_TIME_TOLERANCE_SECS: u64 = 5;

/// Start time of the process currently on `pid`, in Unix epoch seconds.
///
/// `None` when the pid is not signalable, is gone, or the platform cannot
/// report a start time. Used to tell the recorded process apart from a
/// recycled PID that now happens to own the same id.
pub(crate) fn live_start_epoch(pid: u32) -> Option<u64> {
    if !signalable(pid) {
        return None;
    }
    // `ProcessRefreshKind::nothing()` still collects pid/name/start time, and
    // restricting the refresh to one pid keeps this cheap enough for the
    // prune/insert/kill paths that call it.
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        false,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    system.process(pid).map(|process| process.start_time())
}

/// True when the live process on `entry.pid` is still the recorded process.
///
/// A recycled PID is the hazard this closes: `kill(pid, 0)` succeeds for
/// whatever now owns the id, so liveness alone would let `kill` signal an
/// unrelated process. When the platform cannot report a start time, fall back
/// to the plain liveness probe so behaviour is unchanged there.
pub(crate) fn identity_matches_live(entry: &RegistryEntry) -> bool {
    match live_start_epoch(entry.pid) {
        Some(live) => live.abs_diff(entry.started_at) <= START_TIME_TOLERANCE_SECS,
        None => pid_alive(entry.pid),
    }
}

/// True while `entry` is both alive and still the process it recorded. A
/// mismatch is treated as dead: pruned, listed as unmanaged, never signalled.
pub(crate) fn entry_is_live(entry: &RegistryEntry) -> bool {
    pid_alive(entry.pid) && identity_matches_live(entry)
}

/// Signal a pid, preferring its process group so a harness's own children go
/// with it (sharecli spawns each harness as a group leader).
///
/// Returns an error when neither the group signal nor the single-process
/// fallback could be delivered, so a caller can never mistake a failed signal
/// (`ESRCH` for a gone pid, `EPERM` for one it may not signal) for success.
#[cfg(unix)]
pub fn signal_group_or_pid(pid: u32, signal: i32) -> Result<()> {
    if !signalable(pid) {
        // A registry file is operator-writable data; never let a 0 or a wrapped
        // id turn into a signal for our own group or an unrelated process.
        return Err(anyhow::anyhow!("pid {pid} is not a signalable process id"));
    }
    // SAFETY: the target is a pid sharecli recorded at spawn time.
    let as_group = unsafe { libc::kill(-(pid as libc::pid_t), signal) };
    if as_group == 0 {
        return Ok(());
    }
    // SAFETY: same pid, single-process fallback for a non-group child.
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        return Ok(());
    }
    Err(std::io::Error::last_os_error()).with_context(|| format!("signal pid {pid} with {signal}"))
}

/// Terminate the process on `pid`. Windows has no POSIX signals or process
/// groups, so this opens the process for termination and calls
/// `TerminateProcess`; the requested signal number is carried as the exit code
/// so the caller's log still names the intent.
#[cfg(windows)]
pub fn signal_group_or_pid(pid: u32, signal: i32) -> Result<()> {
    if !signalable(pid) {
        return Err(anyhow::anyhow!("pid {pid} is not a signalable process id"));
    }
    const PROCESS_TERMINATE: u32 = 0x0001;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn TerminateProcess(handle: *mut std::ffi::c_void, code: u32) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    // SAFETY: standard Win32 handle lifecycle; the handle is always closed.
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("open pid {pid} for termination"));
    }
    let exit_code = u32::try_from(signal.max(1)).unwrap_or(1);
    let ok = unsafe { TerminateProcess(handle, exit_code) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    if ok == 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("terminate pid {pid} with {signal}"));
    }
    Ok(())
}

/// Handle to the on-disk registry.
#[derive(Debug, Clone)]
pub struct Registry {
    path: PathBuf,
}

/// Exclusive advisory lock on the registry's sidecar `processes.json.lock`.
///
/// Held across the whole read-modify-write of a writer, so two invocations can
/// no longer read the same snapshot and let the later write drop the other's
/// entry. Released when dropped.
struct RegistryLock {
    file: std::fs::File,
}

impl RegistryLock {
    fn acquire(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("create registry dir {}", dir.display()))?;
        }
        let lock_path = path.with_extension("json.lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .with_context(|| format!("open registry lock {}", lock_path.display()))?;
        file.lock_exclusive().with_context(|| format!("lock registry {}", lock_path.display()))?;
        Ok(Self { file })
    }
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::open()
    }
}

impl Registry {
    /// Registry at the standard per-device location.
    pub fn open() -> Self {
        Self { path: default_path() }
    }

    /// Registry at an explicit path (used by tests and by `SHARECLI_STATE_DIR`).
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Strictly read every recorded entry. A missing file is an empty registry;
    /// any other read or parse failure is an error, so a writer can never
    /// replace records it failed to read.
    fn read_entries(&self) -> Result<Vec<RegistryEntry>> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("parse registry {}", self.path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => {
                Err(error).with_context(|| format!("read registry {}", self.path.display()))
            }
        }
    }

    /// Read every recorded entry, ignoring a corrupt or absent file.
    ///
    /// This is the read-only view (`list`, `find`, `contains`). Writers use
    /// [`Self::read_entries`] instead so a corrupt file is never silently
    /// replaced with the empty list it appears to be.
    pub fn entries(&self) -> Vec<RegistryEntry> {
        self.read_entries().unwrap_or_default()
    }

    /// Entries whose process is still alive. Dead entries are dropped from the
    /// file as a side effect so the registry cannot grow without bound.
    pub fn prune_dead(&self) -> Vec<RegistryEntry> {
        let Ok(_guard) = RegistryLock::acquire(&self.path) else {
            // Cannot serialise with other writers: report what is readable
            // without rewriting the file.
            return self.entries().into_iter().filter(entry_is_live).collect();
        };
        let Ok(all) = self.read_entries() else {
            // Never publish the live subset of a registry we failed to read: an
            // empty write here would destroy every record.
            return Vec::new();
        };
        let (live, dead): (Vec<_>, Vec<_>) = all.into_iter().partition(entry_is_live);
        if !dead.is_empty() {
            let _ = self.write(&live);
        }
        live
    }

    /// Record a spawned process. Idempotent per pid. Fails rather than
    /// overwriting a registry it could not read.
    pub fn insert(&self, entry: RegistryEntry) -> Result<()> {
        let _guard = RegistryLock::acquire(&self.path)?;
        let mut entries = self.read_entries()?;
        entries.retain(|e| e.pid != entry.pid && entry_is_live(e));
        entries.push(entry);
        self.write(&entries)
    }

    /// Remove a pid. Returns whether the entry was actually removed from the
    /// registry, so a failed read or write reports `false` instead of claiming a
    /// removal that never reached the file.
    pub fn remove(&self, pid: u32) -> bool {
        let Ok(_guard) = RegistryLock::acquire(&self.path) else {
            return false;
        };
        let Ok(mut entries) = self.read_entries() else {
            return false;
        };
        let before = entries.len();
        entries.retain(|e| e.pid != pid);
        if entries.len() == before {
            return false;
        }
        self.write(&entries).is_ok()
    }

    pub fn contains(&self, pid: u32) -> bool {
        self.entries().iter().any(|e| e.pid == pid)
    }

    /// The recorded entry for `pid`, if any. Callers that are about to signal
    /// the pid use this to validate its recorded identity first.
    pub fn get(&self, pid: u32) -> Option<RegistryEntry> {
        self.entries().into_iter().find(|e| e.pid == pid)
    }

    fn write(&self, entries: &[RegistryEntry]) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("create registry dir {}", dir.display()))?;
        }
        let body = serde_json::to_string_pretty(entries)?;
        let tmp = self.tmp_path();
        let written = std::fs::OpenOptions::new().create_new(true).write(true).open(&tmp).and_then(
            |mut file| {
                file.write_all(body.as_bytes())?;
                // Publish only complete, durable bytes.
                file.sync_all()
            },
        );
        if let Err(error) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(error).with_context(|| format!("write registry tmp {}", tmp.display()));
        }
        if let Err(error) = std::fs::rename(&tmp, &self.path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(error).with_context(|| format!("publish registry {}", self.path.display()));
        }
        Ok(())
    }

    /// A per-writer temporary path: two writers must never truncate, read, or
    /// publish the same in-progress file.
    fn tmp_path(&self) -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let name = self.path.file_name().and_then(|name| name.to_str()).unwrap_or("processes.json");
        self.path.with_file_name(format!("{name}.{}.{nanos}.{seq}.tmp", std::process::id()))
    }
}

/// Device state root shared by the process registry and the build-slot pool.
/// `SHARECLI_STATE_DIR` overrides it so tests and the acceptance harness never
/// touch the operator's real state.
pub(crate) fn state_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("SHARECLI_STATE_DIR") {
        return PathBuf::from(dir);
    }
    let base = match dirs::state_dir().or_else(dirs::data_local_dir).or_else(dirs::config_dir) {
        Some(base) => base,
        None => {
            // `PathBuf::from(".")` would scatter per-device state across
            // whatever directory each invocation runs from, breaking the "one
            // resident authority per device" property. Use one stable per-user
            // location instead, and say so rather than failing silently.
            // A stable per-user tag that needs no platform state directory.
            #[cfg(unix)]
            // SAFETY: getuid cannot fail and touches no memory.
            let user_tag = unsafe { libc::getuid() }.to_string();
            #[cfg(not(unix))]
            let user_tag = std::env::var("USERNAME")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "user".to_string());
            let fallback = std::env::temp_dir().join(format!("sharecli-{user_tag}"));
            eprintln!(
                "sharecli: warning: no platform state directory is available; using {} \
                 for per-device state",
                fallback.display()
            );
            fallback
        }
    };
    base.join("sharecli")
}

/// `<state dir>/sharecli/processes.json`. `SHARECLI_STATE_DIR` overrides it so
/// tests and the acceptance harness never touch the operator's real registry.
fn default_path() -> PathBuf {
    state_root().join("processes.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry(name: &str) -> Registry {
        let dir =
            std::env::temp_dir().join(format!("sharecli-registry-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Registry::at(dir.join("processes.json"))
    }

    #[test]
    fn insert_then_read_back() {
        let registry = temp_registry("roundtrip");
        let entry = RegistryEntry::new(
            std::process::id(),
            Some("proj".into()),
            Some("cargo".into()),
            vec!["cargo".into(), "build".into()],
        );
        registry.insert(entry.clone()).expect("insert");
        assert_eq!(registry.entries(), vec![entry]);
    }

    #[test]
    fn remove_clears_entry() {
        let registry = temp_registry("remove");
        let pid = std::process::id();
        registry.insert(RegistryEntry::new(pid, None, None, vec!["sleep".into()])).expect("insert");
        assert!(registry.contains(pid));
        assert!(registry.remove(pid));
        assert!(!registry.contains(pid));
    }

    #[test]
    fn prune_drops_dead_pids_and_keeps_live_ones() {
        let registry = temp_registry("prune");
        let live = std::process::id();
        registry
            .insert(RegistryEntry::new(live, None, None, vec!["live".into()]))
            .expect("insert live");
        // PID 0 is never a user process, so it is a stable "dead" id.
        registry
            .insert(RegistryEntry::new(0, None, None, vec!["dead".into()]))
            .expect("insert dead");
        let pruned = registry.prune_dead();
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0].pid, live);
        assert_eq!(registry.entries().len(), 1);
    }

    #[test]
    fn prune_drops_recycled_pid_with_stale_start_time() {
        let registry = temp_registry("prune-recycled");
        let live = std::process::id();
        // The pid is very much alive, but the record claims it started an hour
        // ago: that pid must have been recycled, so the entry is not "managed".
        let mut recycled = RegistryEntry::new(live, None, None, vec!["stale".into()]);
        recycled.started_at = now_secs().saturating_sub(3600);
        registry.insert(recycled).expect("insert recycled");

        let pruned = registry.prune_dead();

        assert!(pruned.is_empty(), "a recycled pid must not read as managed: {pruned:?}");
        assert!(registry.entries().is_empty(), "the recycled entry must be dropped from the file");
    }

    #[test]
    fn insert_prunes_a_recycled_entry_rather_than_keeping_it() {
        let registry = temp_registry("insert-recycled");
        let live = std::process::id();
        let mut recycled = RegistryEntry::new(live, None, None, vec!["stale".into()]);
        recycled.started_at = now_secs().saturating_sub(3600);
        registry.insert(recycled).expect("insert recycled");
        assert!(registry.contains(live), "a write records the entry it was given");

        // A later insert is a read-modify-write: the recycled entry must not
        // survive it.
        registry
            .insert(RegistryEntry::new(0, None, None, vec!["dead".into()]))
            .expect("insert other");

        assert!(!registry.contains(live), "a later write must prune a recycled-pid entry");
    }

    #[test]
    fn corrupt_file_reads_as_empty_not_panic() {
        let registry = temp_registry("corrupt");
        if let Some(dir) = registry.path().parent() {
            std::fs::create_dir_all(dir).expect("mkdir");
        }
        std::fs::write(registry.path(), "{not json").expect("write");
        assert!(registry.entries().is_empty());
    }

    #[test]
    fn pid_alive_rejects_ids_the_os_cannot_address() {
        assert!(!pid_alive(0), "pid 0 is the caller's group, never a process handle");
        assert!(!pid_alive(i32::MAX as u32 + 1), "ids above i32::MAX wrap when cast to pid_t");
        assert!(!pid_alive(u32::MAX), "u32::MAX wraps when cast to pid_t");
        assert!(pid_alive(std::process::id()), "the test's own pid must read as alive");
    }

    #[test]
    fn unreaped_zombie_child_is_not_alive() {
        let mut child =
            std::process::Command::new("sh").args(["-c", "exit 0"]).spawn().expect("spawn sh");
        let pid = child.id();
        // The shell exits immediately and `Child` has not reaped it, so it is a
        // zombie. `kill(pid, 0)` still succeeds for a zombie, so liveness must
        // inspect the process state instead of the signal alone.
        let mut alive = true;
        for _ in 0..200 {
            alive = pid_alive(pid);
            if !alive {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = child.wait();
        assert!(!alive, "a zombie whose parent has not reaped it must read as dead");
    }

    #[test]
    fn concurrent_inserts_keep_every_entry() {
        let registry = temp_registry("concurrent");
        let mut children: Vec<std::process::Child> = (0..8)
            .map(|_| {
                std::process::Command::new("sleep")
                    .arg("30")
                    .spawn()
                    .expect("spawn a live stand-in process")
            })
            .collect();
        let pids: Vec<u32> = children.iter().map(|child| child.id()).collect();
        std::thread::scope(|scope| {
            for pid in &pids {
                let registry = registry.clone();
                let pid = *pid;
                scope.spawn(move || {
                    registry
                        .insert(RegistryEntry::new(pid, None, None, vec!["sleep".into()]))
                        .expect("insert");
                });
            }
        });
        let mut recorded: Vec<u32> = registry.entries().iter().map(|e| e.pid).collect();
        let mut expected = pids;
        recorded.sort_unstable();
        expected.sort_unstable();
        for child in &mut children {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert_eq!(recorded, expected, "a concurrent insert must not drop another entry");
    }

    #[test]
    fn corrupt_registry_is_not_overwritten_by_a_writer() {
        let registry = temp_registry("corrupt-write");
        if let Some(dir) = registry.path().parent() {
            std::fs::create_dir_all(dir).expect("mkdir");
        }
        std::fs::write(registry.path(), "{not json").expect("write");
        let error = registry
            .insert(RegistryEntry::new(std::process::id(), None, None, vec!["x".into()]))
            .expect_err("a corrupt registry must fail the write, not be replaced");
        assert!(
            error.to_string().contains("parse registry"),
            "unexpected error for a corrupt registry: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(registry.path()).expect("corrupt file must survive"),
            "{not json",
            "a failed read must never be laundered into an empty registry"
        );
    }

    #[test]
    fn write_publishes_through_a_unique_temp_file_and_cleans_up() {
        let registry = temp_registry("tmp-cleanup");
        registry
            .insert(RegistryEntry::new(std::process::id(), None, None, vec!["x".into()]))
            .expect("insert");
        let dir = registry.path().parent().expect("parent").to_path_buf();
        let leftovers: Vec<String> = std::fs::read_dir(&dir)
            .expect("read_dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files must be published or removed: {leftovers:?}");
    }
}
