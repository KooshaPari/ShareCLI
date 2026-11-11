//! `ProcessInfo` (the dashboard / IPC payload) plus platform-specific helpers
//! for counting open file descriptors and threads per PID.
//!
//! `ProcessInfo::from_sysinfo` snapshots one PID into the struct that tray
//! and report consumers read. `count_open_fds` and `count_threads` are the
//! backends for two optional fields (`fd_count`, `thread_count`); they live
//! here because they share the platform-conditional code that touches
//! `/proc/<pid>/fd` / `/proc/<pid>/task` (Linux), `/bin/ps -M` (macOS), or
//! `lsof` (fallback).

use super::audit::ProcState;
use sysinfo::{Pid, System};

/// Process metadata surfaced to the tray dashboard, IPC payloads, and the
/// `sharecli report` / `sharecli ps` text+CSV outputs.
///
/// The bin crate only writes most of these fields; they are consumed by the
/// tray dashboard consumers (IPC payloads / JSON snapshots) outside this
/// crate's compile unit, which the dead-code pass cannot see.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cmd: Vec<String>,
    pub memory_mb: u64,
    pub start_time: u64,
    /// CPU utilization percentage reported by `sysinfo` (0..100 * num_cores).
    /// Requires sysinfo to have collected at least two samples; the first
    /// refresh after a process start reports 0.
    pub cpu_percent: f32,
    pub project: Option<String>,
    pub harness: Option<String>,
    /// Parent PID (sysinfo extension trait). `None` for kernel threads / early
    /// boot processes. Used by the tray dashboard tree view.
    pub ppid: Option<u32>,
    /// Current working directory (sysinfo extension trait). Empty when the
    /// platform doesn't expose it (unknown/apple-sandbox).
    pub cwd: Option<String>,
    /// Number of environment variables. Computed from `sysinfo::Process::environ()`
    /// length; cross-platform.
    pub env_count: u32,
    /// Process state (Idle/Run/Sleep/etc.) mapped through `ProcState` for
    /// stable serialisation. Used by tray dashboard state-color coding.
    pub state: ProcState,
    /// Total bytes read from disk (Linux-only via `disk_usage()`). `None` on
    /// macOS, Windows, and other platforms. Used by tray dashboard Io column.
    pub disk_read_bytes: Option<u64>,
    /// Total bytes written to disk (Linux-only). `None` on non-Linux.
    pub disk_write_bytes: Option<u64>,
    /// Number of open file descriptors. Computed via `lsof -p <pid>` on
    /// all platforms (cross-platform, ~20ms per process). `None` if the
    /// process is not accessible or `lsof` is unavailable.
    pub fd_count: Option<u32>,
    /// Thread count. Computed via `lsof -p <pid> -F f | grep '^t' | wc -l`
    /// on all platforms. `None` if inaccessible.
    pub thread_count: Option<u32>,
}

impl ProcessInfo {
    pub fn from_sysinfo(pid: Pid, name: String, sys: &System) -> Option<Self> {
        let p = sys.process(pid)?;
        // sysinfo 0.39: parent/cwd/environ/status/disk_usage are direct
        // methods on Process (no ProcessExt trait, which was removed in 0.32).
        #[cfg(unix)]
        let ppid = p.parent().map(|p| p.as_u32());
        #[cfg(not(unix))]
        let ppid: Option<u32> = None;

        #[cfg(unix)]
        let cwd = {
            let s = p.cwd().map(|c| c.to_string_lossy().into_owned()).unwrap_or_default();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };
        #[cfg(not(unix))]
        let cwd: Option<String> = None;

        #[cfg(unix)]
        let env_count = p.environ().len() as u32;
        #[cfg(not(unix))]
        let env_count: u32 = 0;

        let state: ProcState = {
            #[cfg(unix)]
            {
                p.status().into()
            }
            #[cfg(not(unix))]
            ProcState::Unknown
        };

        #[cfg(target_os = "linux")]
        let (disk_read_bytes, disk_write_bytes) = {
            let du = p.disk_usage();
            (Some(du.total_read_bytes), Some(du.total_written_bytes))
        };
        let fd_count = count_open_fds(pid.as_u32());
        let thread_count = count_threads(pid.as_u32());

        #[cfg(not(target_os = "linux"))]
        let (disk_read_bytes, disk_write_bytes): (Option<u64>, Option<u64>) = (None, None);

        Some(ProcessInfo {
            pid: pid.as_u32(),
            name,
            cmd: p.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect(),
            memory_mb: p.memory() / 1024 / 1024,
            start_time: p.start_time(),
            cpu_percent: p.cpu_usage(),
            project: None,
            harness: None,
            ppid,
            cwd,
            env_count,
            state,
            fd_count,
            thread_count,
            disk_read_bytes,
            disk_write_bytes,
        })
    }
}

/// Count descriptors without crossing the runtime/IPC layer boundary.
/// Linux uses `/proc` first; macOS and other Unix systems fall back to lsof.
pub(crate) fn count_open_fds(pid: u32) -> Option<u32> {
    #[cfg(target_os = "linux")]
    if let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) {
        return Some(entries.filter_map(std::result::Result::ok).count() as u32);
    }

    for path in ["/usr/sbin/lsof", "/usr/bin/lsof", "/bin/lsof"] {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        let output = std::process::Command::new(path)
            .args(["-p", &pid.to_string(), "-F", "f"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        return Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.starts_with('f') && line.len() > 1)
                .count() as u32,
        );
    }
    None
}

pub(crate) fn count_threads(pid: u32) -> Option<u32> {
    #[allow(clippy::needless_return)]
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/task")) {
            return Some(entries.filter_map(std::result::Result::ok).count() as u32);
        }
        None
    }

    #[cfg(target_os = "macos")]
    #[allow(clippy::needless_return)]
    {
        let output = std::process::Command::new("/bin/ps")
            .args(["-M", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        return Some(String::from_utf8_lossy(&output.stdout).lines().skip(1).count() as u32);
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        None
    }
}

/// RAII env-var guard — restores a variable to its previous value on drop.
/// Used to temporarily inject `CARGO_BUILD_JOBS` / `RUSTC_WRAPPER` for a
/// single spawn, then atomically restore prior values when the
/// `spawn_blocking` closure returns.
///
/// Fields are `pub(crate)` so `pool.rs` can construct an `EnvGuard` while
/// applying per-spawn env overrides.
pub(crate) struct EnvGuard {
    pub(crate) key: String,
    pub(crate) prev: Option<String>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var(&self.key, v) },
            None => unsafe { std::env::remove_var(&self.key) },
        }
    }
}
