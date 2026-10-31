// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `serve_lock` — lock-based single-instance guard for product-CLI `serve` commands.
//!
//! # Why
//!
//! Every product CLI in the Phenotype org (sharecli, SessionLedger `sl-daemon`,
//! substrate `forge-daemon`) exposes a `serve` command. When multiple actors —
//! agents here or there, a stray shell, a re-run — try to serve the same service,
//! they must **not** silently double-serve or collide on a port. Instead, a second
//! actor should *detect* the existing deploy and make an explicit decision:
//! attach to it, replace it, or abort.
//!
//! This module provides that primitive, built on the same `fs2` advisory-lock
//! mechanism the crate already uses for the Lock-Wait-Cache
//! ([`crate::CoalesceCache::with_lock`]).
//!
//! # Shape
//!
//! - [`ServeLock`] — RAII holder of an exclusive advisory lock on a well-known
//!   pidfile. Acquiring it writes a JSON [`ServeInfo`] (`pid`, `service`, `url`,
//!   `started_at_unix`) so *other* actors can read the running deploy's identity.
//!   Dropping it releases the lock and removes the pidfile.
//! - [`probe`] — read-only: returns [`ServeState`] (`Free` / `Running { stale }`)
//!   without taking the lock, so a caller can inspect before committing.
//! - [`decide`] — pure policy: maps `(ServeState, OnConflict)` → [`Decision`].
//!   The interactive prompt itself is the CLI's job; this returns the decision.
//!
//! # Safety default
//!
//! [`OnConflict::Prompt`] maps to [`Decision::Abort`] here. A non-interactive
//! caller that forgets to wire a prompt therefore *aborts* rather than
//! double-serving — fail loud, never silently collide.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

/// Identity of a running `serve`, persisted into the pidfile so other actors can
/// read who holds the deploy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeInfo {
    /// OS process id of the server that holds the lock.
    pub pid: u32,
    /// Logical service name (e.g. `sl-daemon`, `sharecli`, `forge-daemon`).
    pub service: String,
    /// The URL / socket the server is listening on, for a human-readable prompt.
    pub url: String,
    /// Unix epoch seconds when the serve started.
    pub started_at_unix: u64,
}

/// The observed state of a service's serve-lock, from a read-only [`probe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeState {
    /// No live server: no pidfile, or a pidfile whose owner is gone and whose
    /// lock is free.
    Free,
    /// A pidfile exists. `stale` is `true` when its `pid` is no longer alive
    /// (a crashed server that never cleaned up) — the caller may take over.
    Running { info: ServeInfo, stale: bool },
}

/// What a would-be second server should do on conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnConflict {
    /// Caller will ask the user interactively. Resolves to [`Decision::Abort`]
    /// here so a forgotten prompt never double-serves silently.
    Prompt,
    /// Attach to the existing deploy (do not start a second server).
    Attach,
    /// Replace the existing deploy (take over the lock).
    Replace,
    /// Abort — refuse to serve.
    Abort,
}

/// The resolved action for the caller to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Proceed to serve (the port is free, or we are taking over a stale lock).
    Serve,
    /// Attach to the already-running deploy instead of serving.
    Attach,
    /// Replace the running deploy — the caller should stop it, then serve.
    Replace,
    /// Do nothing; a live server already holds the deploy and policy says abort.
    Abort,
}

/// Resolve `(state, policy)` into a concrete [`Decision`]. Pure — no I/O.
///
/// - `Free`                      → always [`Decision::Serve`].
/// - `Running { stale: true }`   → [`Decision::Serve`] (take over the dead lock),
///   regardless of policy: a crashed server should never block a healthy one.
/// - `Running { stale: false }`  → follow `policy`:
///   - `Attach`  → [`Decision::Attach`]
///   - `Replace` → [`Decision::Replace`]
///   - `Abort`   → [`Decision::Abort`]
///   - `Prompt`  → [`Decision::Abort`] (safe default; caller wires the real prompt)
pub fn decide(state: &ServeState, policy: OnConflict) -> Decision {
    match state {
        ServeState::Free => Decision::Serve,
        ServeState::Running { stale: true, .. } => Decision::Serve,
        ServeState::Running { stale: false, .. } => match policy {
            OnConflict::Attach => Decision::Attach,
            OnConflict::Replace => Decision::Replace,
            OnConflict::Abort => Decision::Abort,
            OnConflict::Prompt => Decision::Abort,
        },
    }
}

/// Default lock directory: `$XDG_RUNTIME_DIR` if set, else the system temp dir.
fn lock_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// Well-known pidfile path for a service: `<lock_dir>/<service>.serve.lock`.
///
/// `service` is sanitized (path separators → `_`) so it can't escape `lock_dir`.
pub fn pidfile_path(service: &str) -> PathBuf {
    let safe: String =
        service.chars().map(|c| if matches!(c, '/' | '\\' | ':') { '_' } else { c }).collect();
    lock_dir().join(format!("{safe}.serve.lock"))
}

/// Is `pid` a live process?
///
/// Unix: `kill(pid, 0)` — signal 0 probes existence (`Ok`/`EPERM` ⇒ alive, `ESRCH` ⇒ gone).
/// Windows: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` — handle open ⇒ alive.
#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // Reject sentinels / values that do not fit a positive `pid_t`. Casting
    // `u32::MAX` to `pid_t` yields `-1`, and `kill(-1, 0)` means "every process
    // we can signal" — which falsely reports the sentinel as alive.
    let Ok(pid_i) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid_i <= 0 {
        return false;
    }
    // SAFETY: `kill` with signal 0 sends no signal; it only probes existence.
    let rc = unsafe { libc::kill(pid_i, 0) };
    if rc == 0 {
        return true;
    }
    // errno == EPERM => process exists but we can't signal it (still "alive").
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    // SAFETY: OpenProcess/CloseHandle are standard Win32; null handle means gone.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    unsafe {
        let _ = CloseHandle(handle);
    }
    true
}

/// Seconds since the Unix epoch (best-effort; 0 if the clock is before epoch).
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Read-only probe of a service's serve state. Does **not** take the lock, so it
/// is safe to call from any actor to inspect an existing deploy before deciding.
pub fn probe(service: &str) -> Result<ServeState> {
    let path = pidfile_path(service);
    let mut file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ServeState::Free),
        Err(e) => return Err(e).with_context(|| format!("open pidfile {}", path.display())),
    };

    // A held exclusive lock means a server is actively serving. If we can grab a
    // shared lock, no one holds it exclusively — but the pidfile contents still
    // tell us whether the last owner is alive (crash without cleanup).
    let exclusively_held = file.try_lock_shared().is_err();
    if !exclusively_held {
        // We took a shared lock; release it immediately so we stay read-only.
        let _ = file.unlock();
    }

    let mut buf = String::new();
    match file.read_to_string(&mut buf) {
        Ok(_) => {}
        Err(e) if exclusively_held => {
            // Windows mandatory locks block readers; use unlocked sidecar.
            buf = fs::read_to_string(info_sidecar_path(&path)).with_context(|| {
                format!(
                    "read pidfile {} (locked) and sidecar {}",
                    path.display(),
                    info_sidecar_path(&path).display()
                )
            })?;
            let _ = e;
        }
        Err(e) => return Err(e).with_context(|| format!("read pidfile {}", path.display())),
    }

    // Empty or malformed pidfile with no live holder → treat as Free.
    let info: ServeInfo = match serde_json::from_str(buf.trim()) {
        Ok(i) => i,
        Err(_) if !exclusively_held => return Ok(ServeState::Free),
        Err(e) => {
            return Err(e).with_context(|| format!("parse pidfile {}", path.display()));
        }
    };

    // The pidfile owner is live if either the lock is currently held exclusively
    // (a running server) or the recorded pid is still alive. Otherwise it's a
    // stale entry from a crashed server that never cleaned up.
    let alive = exclusively_held || pid_alive(info.pid);
    Ok(ServeState::Running { info, stale: !alive })
}

fn info_sidecar_path(lock_path: &Path) -> PathBuf {
    lock_path.with_extension("info")
}

/// RAII holder of an exclusive serve-lock. While alive, this process owns the
/// deploy for `service`; the pidfile carries its [`ServeInfo`]. Drop releases the
/// lock and removes the pidfile.
#[derive(Debug)]
pub struct ServeLock {
    path: PathBuf,
    file: fs::File,
    info: ServeInfo,
}

impl ServeLock {
    /// Try to acquire the serve-lock for `service`, advertising `url`. Returns
    /// `Ok(Some(lock))` on success, `Ok(None)` if another **live** server already
    /// holds it (caller should [`probe`] + [`decide`]), or `Err` on I/O failure.
    ///
    /// A *stale* pidfile (previous owner dead) is taken over transparently.
    pub fn try_acquire(service: &str, url: impl Into<String>) -> Result<Option<Self>> {
        let path = pidfile_path(service);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create lock dir {}", parent.display()))?;
        }

        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("open pidfile {}", path.display()))?;

        // Non-blocking exclusive lock: if a live server holds it, this fails fast.
        // Note: Linux `flock` can still succeed for a *second* open in the same
        // process while the first fd holds the lock — detect that below.
        if file.try_lock_exclusive().is_err() {
            return Ok(None);
        }

        let mut f = file;

        // If the pidfile already names a live owner, do not double-serve.
        // Covers (1) same-process flock re-acquire and (2) races where the
        // exclusive lock was obtained over a still-valid deploy record.
        {
            let mut existing = String::new();
            let _ = f.seek(SeekFrom::Start(0));
            if f.read_to_string(&mut existing).is_ok() {
                if let Ok(prev) = serde_json::from_str::<ServeInfo>(existing.trim()) {
                    if pid_alive(prev.pid) {
                        let _ = f.unlock();
                        return Ok(None);
                    }
                }
            }
        }

        // We hold the lock. Anything previously written is a stale owner's info —
        // we now overwrite with our own identity.
        let info = ServeInfo {
            pid: process::id(),
            service: service.to_string(),
            url: url.into(),
            started_at_unix: now_unix(),
        };
        f.set_len(0).with_context(|| format!("truncate pidfile {}", path.display()))?;
        let bytes = serde_json::to_vec(&info).context("serialize ServeInfo")?;
        f.write_all(&bytes).with_context(|| format!("write pidfile {}", path.display()))?;
        f.flush().ok();
        let _ = fs::write(info_sidecar_path(&path), &bytes);

        Ok(Some(Self { path, file: f, info }))
    }

    /// The identity written to the pidfile for this lock.
    pub fn info(&self) -> &ServeInfo {
        &self.info
    }

    /// The pidfile path backing this lock.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ServeLock {
    fn drop(&mut self) {
        // Best-effort: release the advisory lock and remove the pidfile so the
        // next actor sees `Free` rather than a stale entry.
        let _ = self.file.unlock();
        let sidecar = info_sidecar_path(&self.path);
        let _ = fs::remove_file(&sidecar);
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;

    /// Serialize tests that mutate `$XDG_RUNTIME_DIR` (process-global). Without
    /// this, parallel `#[test]`s race and `probe` can see another test's empty
    /// dir → false `Free` while a lock is held.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Dead-but-plausible pid: positive `pid_t`, above typical pid_max, and not
    /// `u32::MAX` (which truncates to `-1` and makes `kill(-1, 0)` look alive).
    const DEAD_PID: u32 = 2_000_000;

    /// Isolate each test's lock dir via a tempdir bound to $XDG_RUNTIME_DIR.
    struct LockEnv {
        _dir: tempfile::TempDir,
        prev: Option<std::ffi::OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl LockEnv {
        fn new() -> Self {
            let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let dir = tempfile::tempdir().expect("tempdir");
            let prev = std::env::var_os("XDG_RUNTIME_DIR");
            // SAFETY: serialized by ENV_LOCK; only one LockEnv alive at a time.
            unsafe { std::env::set_var("XDG_RUNTIME_DIR", dir.path()) };
            Self { _dir: dir, prev, _guard: guard }
        }

        fn runtime_dir(&self) -> PathBuf {
            self._dir.path().to_path_buf()
        }
    }

    impl Drop for LockEnv {
        fn drop(&mut self) {
            // SAFETY: serialized by `_guard`; no concurrent set_var/remove_var.
            match &self.prev {
                Some(v) => unsafe { std::env::set_var("XDG_RUNTIME_DIR", v) },
                None => unsafe { std::env::remove_var("XDG_RUNTIME_DIR") },
            }
        }
    }

    /// Child entry: hold a serve-lock until the hold-file is removed.
    ///
    /// Spawned via `current_exe() --exact …::child_hold_serve_lock` so flock
    /// contention is truly cross-process (Linux same-process flock is not a
    /// reliable stand-in for two `serve` actors).
    #[test]
    fn child_hold_serve_lock() {
        if std::env::var_os("SHARECLI_IPC_SERVE_LOCK_CHILD").is_none() {
            return;
        }
        let service = std::env::var("SHARECLI_IPC_SERVE_LOCK_SERVICE").expect("service");
        let ready = PathBuf::from(std::env::var("SHARECLI_IPC_SERVE_LOCK_READY").expect("ready"));
        let hold = PathBuf::from(std::env::var("SHARECLI_IPC_SERVE_LOCK_HOLD").expect("hold"));
        let url = std::env::var("SHARECLI_IPC_SERVE_LOCK_URL").unwrap_or_else(|_| "child".into());
        let _lock = ServeLock::try_acquire(&service, url)
            .expect("child acquire io")
            .expect("child acquire free");
        fs::write(&ready, b"1").expect("ready");
        while hold.exists() {
            thread::sleep(Duration::from_millis(20));
        }
    }

    struct ChildLock {
        child: std::process::Child,
        hold_path: PathBuf,
        ready_path: PathBuf,
    }

    impl ChildLock {
        fn spawn(env: &LockEnv, service: &str, url: &str) -> Self {
            let runtime = env.runtime_dir();
            let ready_path = runtime.join(format!("{service}.ready"));
            let hold_path = runtime.join(format!("{service}.hold"));
            let _ = fs::remove_file(&ready_path);
            fs::write(&hold_path, b"1").expect("hold file");

            let mut child =
                std::process::Command::new(std::env::current_exe().expect("current_exe"))
                    .args(["--exact", "serve_lock::tests::child_hold_serve_lock", "--nocapture"])
                    .env("SHARECLI_IPC_SERVE_LOCK_CHILD", "1")
                    .env("SHARECLI_IPC_SERVE_LOCK_SERVICE", service)
                    .env("SHARECLI_IPC_SERVE_LOCK_READY", &ready_path)
                    .env("SHARECLI_IPC_SERVE_LOCK_HOLD", &hold_path)
                    .env("SHARECLI_IPC_SERVE_LOCK_URL", url)
                    .env("XDG_RUNTIME_DIR", &runtime)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("spawn serve-lock child");

            let start = Instant::now();
            while !ready_path.exists() {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "timed out waiting for child lock holder"
                );
                if let Some(status) = child.try_wait().expect("try_wait") {
                    panic!("lock-holder child exited early: {status}");
                }
                thread::sleep(Duration::from_millis(20));
            }

            Self { child, hold_path, ready_path }
        }

        fn pid(&self) -> u32 {
            self.child.id()
        }
    }

    impl Drop for ChildLock {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.hold_path);
            let _ = fs::remove_file(&self.ready_path);
            let _ = self.child.wait();
        }
    }

    #[test]
    fn probe_free_when_no_pidfile() {
        let _env = LockEnv::new();
        assert_eq!(probe("svc-none").unwrap(), ServeState::Free);
    }

    #[test]
    fn acquire_when_free_then_probe_running() {
        let env = LockEnv::new();

        // In-process acquire still writes identity correctly.
        {
            let lock = ServeLock::try_acquire("svc-a-local", "http://127.0.0.1:9001")
                .unwrap()
                .expect("free → acquired");
            assert_eq!(lock.info().pid, process::id());
            assert_eq!(lock.info().url, "http://127.0.0.1:9001");
        }

        // Probe a lock held by another process — the real multi-actor path.
        // Same-process probe can race with `$XDG_RUNTIME_DIR` and is a weak
        // stand-in for Linux flock ownership across serve actors.
        let holder = ChildLock::spawn(&env, "svc-a", "http://127.0.0.1:9001");
        match probe("svc-a").unwrap() {
            ServeState::Running { info, stale } => {
                assert_eq!(info.pid, holder.pid());
                assert_eq!(info.url, "http://127.0.0.1:9001");
                assert!(!stale, "live child-held lock must not be stale");
            }
            ServeState::Free => panic!("expected Running while lock held"),
        }
    }

    #[test]
    fn second_acquire_blocks_while_first_held() {
        let env = LockEnv::new();
        // Cross-process: Linux flock may allow same-process re-acquire on a
        // second fd; two serve actors are always separate processes.
        let _holder = ChildLock::spawn(&env, "svc-b", "u1");
        let second = ServeLock::try_acquire("svc-b", "u2").unwrap();
        assert!(second.is_none(), "second acquire must fail while first held");
    }

    #[test]
    fn drop_releases_and_removes_pidfile() {
        let _env = LockEnv::new();
        let path = pidfile_path("svc-c");
        {
            let _lock = ServeLock::try_acquire("svc-c", "u").unwrap().unwrap();
            assert!(path.exists(), "pidfile present while held");
        }
        assert!(!path.exists(), "pidfile removed on drop");
        assert_eq!(probe("svc-c").unwrap(), ServeState::Free);
    }

    #[test]
    fn stale_pidfile_reports_stale_and_allows_takeover() {
        let _env = LockEnv::new();
        // Hand-write a pidfile owned by an impossible/dead pid, unlocked.
        let path = pidfile_path("svc-d");
        let dead = ServeInfo {
            pid: DEAD_PID,
            service: "svc-d".into(),
            url: "u".into(),
            started_at_unix: 1,
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_vec(&dead).unwrap()).unwrap();

        match probe("svc-d").unwrap() {
            ServeState::Running { stale, info } => {
                assert!(stale, "dead-pid pidfile must be stale");
                assert_eq!(info.pid, DEAD_PID);
            }
            ServeState::Free => panic!("expected Running{{stale}}"),
        }

        // A takeover acquisition should succeed over the stale file.
        let lock = ServeLock::try_acquire("svc-d", "u2").unwrap();
        assert!(lock.is_some(), "stale lock must be takeable");
    }

    #[test]
    fn decide_free_always_serves() {
        assert_eq!(decide(&ServeState::Free, OnConflict::Prompt), Decision::Serve);
        assert_eq!(decide(&ServeState::Free, OnConflict::Abort), Decision::Serve);
    }

    #[test]
    fn decide_running_live_follows_policy() {
        let running = ServeState::Running {
            info: ServeInfo {
                pid: process::id(),
                service: "s".into(),
                url: "u".into(),
                started_at_unix: 1,
            },
            stale: false,
        };
        assert_eq!(decide(&running, OnConflict::Prompt), Decision::Abort);
        assert_eq!(decide(&running, OnConflict::Attach), Decision::Attach);
        assert_eq!(decide(&running, OnConflict::Replace), Decision::Replace);
        assert_eq!(decide(&running, OnConflict::Abort), Decision::Abort);
    }

    #[test]
    fn decide_running_stale_serves_regardless_of_policy() {
        let stale = ServeState::Running {
            info: ServeInfo {
                pid: DEAD_PID,
                service: "s".into(),
                url: "u".into(),
                started_at_unix: 1,
            },
            stale: true,
        };
        assert_eq!(decide(&stale, OnConflict::Prompt), Decision::Serve);
        assert_eq!(decide(&stale, OnConflict::Abort), Decision::Serve);
    }
}
