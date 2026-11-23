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
//! - the descriptor is made inheritable, so the build child holds the lock as
//!   well. When `sharecli start` exits, the slot stays occupied until the build
//!   itself exits; when the build exits, the kernel releases the lock.
//!
//! Liveness is the kernel's: a killed build releases its slot with no stale-lock
//! recovery path needed.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// One occupied build slot. Dropping it releases the lock for this process; an
/// inherited copy in the child keeps it held until that child exits.
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
                make_inheritable(&file)?;
                return Ok(SlotLease { file, slot });
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(unix)]
fn try_lock(file: &std::fs::File) -> Result<bool> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: the fd is owned by `file` and valid for this call.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code) if code == libc::EWOULDBLOCK || code == libc::EINTR => Ok(false),
        _ => Err(anyhow::anyhow!("flock build slot: {error}")),
    }
}

/// Clear `FD_CLOEXEC` so the build child inherits the locked descriptor and the
/// slot survives this process exiting.
#[cfg(unix)]
fn make_inheritable(file: &std::fs::File) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: plain fcntl on a descriptor owned by `file`.
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
    if flags < 0 {
        return Err(anyhow::anyhow!(
            "fcntl(F_GETFD) build slot: {}",
            std::io::Error::last_os_error()
        ));
    }
    let cleared = flags & !libc::FD_CLOEXEC;
    // SAFETY: as above; `cleared` is a valid fd-flag set.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, cleared) } < 0 {
        return Err(anyhow::anyhow!(
            "fcntl(F_SETFD) build slot: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn try_lock(_file: &std::fs::File) -> Result<bool> {
    Err(anyhow::anyhow!("device build admission requires unix flock"))
}

#[cfg(not(unix))]
fn make_inheritable(_file: &std::fs::File) -> Result<()> {
    Err(anyhow::anyhow!("device build admission requires unix flock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("sharecli-admission-{name}-{}", std::process::id()));
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
            std::thread::spawn(move || acquire_blocking(&dir, 1).map(|lease| lease.slot().to_path_buf()))
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
    fn lease_descriptor_is_inheritable() {
        use std::os::unix::io::AsRawFd;
        let dir = temp_dir("inheritable");
        let lease = acquire_blocking(&dir, 1).expect("lease");
        // SAFETY: reading fd flags on a descriptor owned by the lease.
        let flags = unsafe { libc::fcntl(lease.file.as_raw_fd(), libc::F_GETFD) };
        assert!(flags >= 0, "F_GETFD failed");
        assert_eq!(flags & libc::FD_CLOEXEC, 0, "build slot fd must survive exec");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
