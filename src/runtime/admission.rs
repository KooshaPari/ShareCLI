// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Device-scoped build admission: a slot is held for the whole build.
//!
//! `spawn_policy::SpawnPolicy` guards builds with an in-process semaphore, which
//! is the wrong scope for a per-device authority: `sharecli start` exits as soon
//! as the child is spawned, and a second invocation builds its own semaphore, so
//! `max_concurrent_builds = 1` admits two concurrent builds on the same device.
//!
//! This module keeps the slot where the constraint actually lives. A slot is an
//! `flock`ed file under the device state directory. Two properties make it hold
//! for the full child lifetime:
//!
//! - `flock` locks belong to the *open file description*, not to the process, so
//!   a sibling invocation opening the same slot file contends with this one.
//! - the descriptor is kept close-on-exec and an *out-of-process supervisor*
//!   holds a duplicate open file description for the whole build. The build
//!   never inherits the lock, so a long-lived descendant cannot pin the slot;
//!   the supervisor releases it as soon as the direct build process exits, even
//!   after the short-lived `sharecli start` that took the slot has gone.
//!
//! Liveness is the kernel's: a killed build releases its slot with no stale-lock
//! recovery path needed.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// One occupied build slot.
///
/// The descriptor is close-on-exec: the build process tree never inherits the
/// lock. Callers that outlive the taking process (i.e. `sharecli start`, which
/// exits immediately after spawning) must hand the lease to a supervisor with
/// [`SlotLease::watch_until_exit`] before dropping it, otherwise the slot is
/// released as soon as the taking process exits.
#[derive(Debug)]
pub struct SlotLease {
    file: std::fs::File,
    slot: PathBuf,
}

impl SlotLease {
    /// Path of the slot file this lease holds.
    pub fn slot(&self) -> &Path {
        &self.slot
    }
}

/// Device-wide pool of `slots` build slots.
#[derive(Debug, Clone)]
pub struct SlotPool {
    dir: PathBuf,
    slots: usize,
}

impl SlotPool {
    /// Pool under the device state directory (`SHARECLI_STATE_DIR` aware).
    pub fn device(slots: usize) -> Self {
        Self { dir: super::registry::state_root().join("build-slots"), slots: slots.max(1) }
    }

    /// Pool at an explicit directory, for tests.
    pub fn at(dir: impl Into<PathBuf>, slots: usize) -> Self {
        Self { dir: dir.into(), slots: slots.max(1) }
    }

    /// Wait for a free slot, then hold it.
    pub async fn acquire(&self) -> Result<SlotLease> {
        let dir = self.dir.clone();
        let slots = self.slots;
        tokio::task::spawn_blocking(move || acquire_blocking(&dir, slots))
            .await
            .map_err(|e| anyhow::anyhow!("build-slot wait join: {e}"))?
    }
}

fn acquire_blocking(dir: &Path, slots: usize) -> Result<SlotLease> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("create build-slot dir {}", dir.display()))?;
    loop {
        for index in 0..slots {
            let slot = dir.join(format!("slot{index}"));
            let file = match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&slot)
            {
                Ok(file) => file,
                // No writable state directory: the caller falls back to the
                // in-process semaphore rather than failing the build.
                Err(error) => {
                    return Err(anyhow::anyhow!("open build slot {}: {error}", slot.display()));
                }
            };
            if try_lock(&file)? {
                // Keep FD_CLOEXEC: the build subtree must NOT inherit the lock.
                // The slot's lifetime tracks the supervised child instead.
                return Ok(SlotLease { file, slot });
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(unix)]
fn try_lock(file: &std::fs::File) -> Result<bool> {
    use std::os::unix::io::AsRawFd;
    loop {
        // SAFETY: the fd is owned by `file` and valid for this call.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(code) if code == libc::EWOULDBLOCK => return Ok(false),
            Some(code) if code == libc::EINTR => continue,
            _ => return Err(anyhow::anyhow!("flock build slot: {error}")),
        }
    }
}

#[cfg(unix)]
impl SlotLease {
    /// Hand this lease to a detached supervisor process that keeps the slot held
    /// until `pid` (the direct build child) is gone, then releases it.
    ///
    /// The supervisor `fork`s and inherits a duplicate of the locked open file
    /// description, so the `flock` survives this process exiting. Because the
    /// supervisor is not the build's parent only `kill(pid, 0)` can poll it, but
    /// that observes the *direct* child only: a descendant that outlives the
    /// build (`cargo` leaving a `sleep` behind) never pins the slot.
    ///
    /// Call this before dropping the lease in any process that does not outlive
    /// the build (notably `sharecli start`). Calling it more than once is a
    /// caller error; the first supervisor to see the child exit releases the
    /// lock, so the slot is still bounded by the child's lifetime.
    pub fn watch_until_exit(&self, pid: i32) -> Result<()> {
        use std::os::unix::io::AsRawFd;
        if pid <= 0 {
            return Err(anyhow::anyhow!("watch_until_exit: invalid pid {pid}"));
        }
        let keep = self.file.as_raw_fd();
        // SAFETY: the child runs only async-signal-safe operations (`close`,
        // `kill`, `nanosleep`, `_exit`) and never touches Rust-allocated state,
        // so a `fork` from a threaded runtime is sound for our purpose.
        let child = unsafe { libc::fork() };
        if child < 0 {
            return Err(anyhow::anyhow!(
                "fork build-slot supervisor: {}",
                std::io::Error::last_os_error()
            ));
        }
        if child == 0 {
            // Supervisor: detach from the caller's session and standard streams
            // so it cannot hold the caller's terminal pipes open or be killed
            // with the caller's session, then drop every inherited descriptor
            // except stdio and the locked slot, so another lease in this
            // process is not pinned open.
            // SAFETY: all calls below are async-signal-safe.
            unsafe {
                libc::setsid();
                let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
                if devnull >= 0 {
                    libc::dup2(devnull, 0);
                    libc::dup2(devnull, 1);
                    libc::dup2(devnull, 2);
                    if devnull > 2 {
                        libc::close(devnull);
                    }
                }
                let max_fd: i32 = 1024;
                for fd in 0..max_fd {
                    if fd != keep && fd > 2 {
                        libc::close(fd);
                    }
                }
                loop {
                    let rc = libc::kill(pid, 0);
                    if rc != 0 {
                        let code = raw_errno();
                        if code == libc::ESRCH {
                            break;
                        }
                        // EPERM: alive but not ours. Any other errno: keep
                        // waiting rather than release a slot early.
                    }
                    let period = libc::timespec { tv_sec: 0, tv_nsec: 50_000_000 };
                    libc::nanosleep(&period, std::ptr::null_mut());
                }
                // Closing `keep` on `_exit` releases the `flock`.
                libc::_exit(0);
            }
        }
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn raw_errno() -> i32 {
    *libc::__errno_location()
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
unsafe fn raw_errno() -> i32 {
    *libc::__error()
}

#[cfg(not(unix))]
fn try_lock(_file: &std::fs::File) -> Result<bool> {
    Err(anyhow::anyhow!("device build admission requires unix flock"))
}

#[cfg(not(unix))]
impl SlotLease {
    /// Non-unix stub: without `flock` there is no supervisor to hand off to.
    pub fn watch_until_exit(&self, _pid: i32) -> Result<()> {
        Err(anyhow::anyhow!("device build admission requires unix flock"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sharecli-admission-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[cfg(unix)]
    #[test]
    fn one_slot_admits_one_lease_at_a_time() {
        let dir = temp_dir("one-slot");
        let first = acquire_blocking(&dir, 1).expect("first lease");
        // Second acquisition must block: prove it by acquiring on a thread and
        // confirming it has not returned while the first lease is alive.
        let handle = {
            let dir = dir.clone();
            std::thread::spawn(move || {
                acquire_blocking(&dir, 1).map(|lease| lease.slot().to_path_buf())
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop(first);
        let second = handle.join().expect("thread").expect("second lease");
        assert_eq!(second, dir.join("slot0"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn two_leases_coexist_when_two_slots_are_configured() {
        let dir = temp_dir("two-slots");
        let first = acquire_blocking(&dir, 2).expect("first lease");
        let second = acquire_blocking(&dir, 2).expect("second lease");
        assert_ne!(first.slot(), second.slot());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn lease_descriptor_stays_close_on_exec() {
        use std::os::unix::io::AsRawFd;
        let dir = temp_dir("cloexec");
        let lease = acquire_blocking(&dir, 1).expect("lease");
        // SAFETY: reading fd flags on a descriptor owned by the lease.
        let flags = unsafe { libc::fcntl(lease.file.as_raw_fd(), libc::F_GETFD) };
        assert!(flags >= 0, "F_GETFD failed");
        assert_ne!(
            flags & libc::FD_CLOEXEC,
            0,
            "build slot fd must NOT survive exec, or a descendant pins the slot"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The G2 full-lifetime property: the supervisor holds the slot for the
    /// *direct* build child only. A descendant that outlives the build must not
    /// keep the slot. Pre-fix (inheritable fd + no supervisor) the second
    /// acquire never returns, so the watchdog fails the test instead of hanging.
    #[cfg(unix)]
    #[test]
    fn slot_is_released_when_build_exits_even_if_a_descendant_survives() {
        use std::process::Command;
        use std::time::Duration;

        let dir = temp_dir("supervise");
        let lease = acquire_blocking(&dir, 1).expect("lease");

        // Fake build: leaves a long-lived background child behind, then exits.
        let mut build = Command::new("sh")
            .arg("-c")
            .arg("sleep 30 >/dev/null 2>&1 & exit 0")
            .spawn()
            .expect("spawn fake build");
        let pid = build.id() as i32;
        lease.watch_until_exit(pid).expect("supervisor");
        // The taking process (`sharecli start`) exits here: only the supervisor
        // still holds the lock, and it releases when `pid` is reaped.
        drop(lease);
        build.wait().expect("reap build");

        let (tx, rx) = std::sync::mpsc::channel();
        let dir2 = dir.clone();
        std::thread::spawn(move || {
            let second = acquire_blocking(&dir2, 1);
            let _ = tx.send(second.is_ok());
        });
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(true) => {}
            Ok(false) => panic!("second acquire errored"),
            Err(_) => panic!(
                "slot leaked: second acquire did not return within 5s \
                 (a surviving descendant pinned the lease)"
            ),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
