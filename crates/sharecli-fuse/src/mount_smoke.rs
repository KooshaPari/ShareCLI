// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Optional privileged FUSE mount smoke (operator / CI with macFUSE or libfuse).
//!
//! Default `cargo test` skips live mount verification. Set
//! `SHARECLI_FUSE_MOUNT_SMOKE=1` to run a read/write round-trip through a real
//! FUSE mountpoint (requires platform FUSE support and sufficient privileges).

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::provenance::{default_session_id, read_provenance};

/// Environment variable that opts into privileged mount smoke tests.
pub const ENV_FUSE_MOUNT_SMOKE: &str = "SHARECLI_FUSE_MOUNT_SMOKE";

/// Return `true` when `SHARECLI_FUSE_MOUNT_SMOKE` is `1` or `true` (case-insensitive).
pub fn fuse_mount_smoke_enabled() -> bool {
    match std::env::var(ENV_FUSE_MOUNT_SMOKE) {
        Ok(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}

/// Best-effort force-unmount for a sharecli FUSE mountpoint.
pub fn force_unmount(mountpoint: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        for bin in ["fusermount3", "fusermount"] {
            match std::process::Command::new(bin).arg("-uz").arg(mountpoint).status() {
                Ok(status) if status.success() => return Ok(()),
                _ => continue,
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("umount").arg(mountpoint).status()?;
        if status.success() {
            return Ok(());
        }
    }
    #[cfg(windows)]
    {
        return crate::winfsp_mount::force_unmount_winfsp(mountpoint);
    }
    Err(std::io::Error::other(format!(
        "sharecli-fuse: force_unmount failed for {}",
        mountpoint.display()
    )))
}

/// RAII session: background FUSE mount over `backing`, unmount on drop.
pub struct MountSession {
    mountpoint: PathBuf,
    _mount_dir: tempfile::TempDir,
    mount_thread: Option<JoinHandle<()>>,
    #[cfg(windows)]
    _winfsp: Option<crate::winfsp_mount::WinfspMountSession>,
}

impl MountSession {
    /// Mount `backing` at a fresh temp mountpoint; waits until the seed file is visible.
    pub fn start(backing: &Path, seed_rel: &Path) -> anyhow::Result<Self> {
        #[cfg(windows)]
        {
            let mount_dir = tempfile::tempdir()?;
            let mountpoint = mount_dir.path().to_path_buf();
            let session_id = default_session_id();
            let handle = std::sync::Arc::new(crate::CowMountHandle::from_options(
                backing,
                &crate::InterceptFsOptions {
                    session_id: session_id.clone(),
                    cow: false,
                    cow_dir: None,
                    agent: None,
                    serialize: true,
                    agents_conf: None,
                },
            ));
            let session = crate::winfsp_mount::WinfspMountSession::start(
                &mountpoint,
                backing,
                &session_id,
                handle,
            )?;
            let seed_on_mount = mountpoint.join(seed_rel);
            let deadline = Duration::from_secs(8);
            let poll = Duration::from_millis(100);
            let mut waited = Duration::ZERO;
            while waited < deadline {
                if seed_on_mount.is_file() && std::fs::read(&seed_on_mount).is_ok() {
                    return Ok(Self {
                        mountpoint,
                        _mount_dir: mount_dir,
                        mount_thread: None,
                        _winfsp: Some(session),
                    });
                }
                thread::sleep(poll);
                waited += poll;
            }
            drop(session);
            anyhow::bail!(
                "sharecli-fuse mount smoke: timed out waiting for {} (is WinFsp installed?)",
                seed_on_mount.display()
            );
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let mount_dir = tempfile::tempdir()?;
            let mountpoint = mount_dir.path().to_path_buf();
            let backing = backing.to_path_buf();
            let mp = mountpoint.clone();

            let (fail_tx, fail_rx) = mpsc::channel::<String>();
            let mount_thread = thread::spawn(move || {
                if let Err(err) = crate::mount(&mp, &backing) {
                    let _ = fail_tx.send(err.to_string());
                }
            });

            let seed_on_mount = mountpoint.join(seed_rel);
            let deadline = Duration::from_secs(8);
            let poll = Duration::from_millis(100);
            let mut waited = Duration::ZERO;

            while waited < deadline {
                if seed_on_mount.is_file() && std::fs::read(&seed_on_mount).is_ok() {
                    return Ok(Self {
                        mountpoint,
                        _mount_dir: mount_dir,
                        mount_thread: Some(mount_thread),
                    });
                }
                if let Ok(msg) = fail_rx.try_recv() {
                    anyhow::bail!("sharecli-fuse mount smoke: mount failed: {msg}");
                }
                thread::sleep(poll);
                waited += poll;
            }

            let _ = force_unmount(&mountpoint);
            anyhow::bail!(
                "sharecli-fuse mount smoke: timed out waiting for {} (is FUSE installed and permitted?)",
                seed_on_mount.display()
            )
        }
    }

    /// Path where the FUSE layer is mounted.
    pub fn mountpoint(&self) -> &Path {
        &self.mountpoint
    }
}

impl Drop for MountSession {
    fn drop(&mut self) {
        let _ = force_unmount(&self.mountpoint);
        if let Some(handle) = self.mount_thread.take() {
            let _ = handle.join();
        }
    }
}

/// After a live FUSE write, provenance xattrs MUST be present on the backing file.
pub fn verify_mount_smoke_provenance(backing_file: &Path) -> anyhow::Result<()> {
    let prov = read_provenance(backing_file)
        .map_err(|e| anyhow::anyhow!("mount smoke provenance read failed: {e}"))?
        .ok_or_else(|| {
            anyhow::anyhow!("mount smoke provenance: missing xattrs on {}", backing_file.display())
        })?;
    let expected_session = default_session_id();
    anyhow::ensure!(
        prov.session_id == expected_session,
        "mount smoke provenance: session_id {:?} != expected {:?}",
        prov.session_id,
        expected_session
    );
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    anyhow::ensure!(
        prov.written_at_unix <= now.saturating_add(5),
        "mount smoke provenance: written_at {} is in the future (now {now})",
        prov.written_at_unix
    );
    anyhow::ensure!(
        now.saturating_sub(prov.written_at_unix) <= 60,
        "mount smoke provenance: written_at {} is stale (now {now})",
        prov.written_at_unix
    );
    Ok(())
}

/// Read/write round-trip through a live FUSE mount (privileged smoke).
///
/// After the write, asserts write provenance xattrs on the backing path
/// (AC-009.6 × AC-009.8).
pub fn run_mount_smoke(backing: &Path) -> anyhow::Result<()> {
    let seed_rel = Path::new("smoke-seed.txt");
    std::fs::write(backing.join(seed_rel), b"before-fuse")?;

    let session = MountSession::start(backing, seed_rel)?;
    let mounted_seed = session.mountpoint().join(seed_rel);
    let body = std::fs::read(&mounted_seed)?;
    anyhow::ensure!(
        body == b"before-fuse",
        "mount smoke read: expected seed payload, got {} bytes",
        body.len()
    );

    std::fs::write(&mounted_seed, b"after-fuse")?;
    let backing_seed = backing.join(seed_rel);
    let backing_body = std::fs::read(&backing_seed)?;
    anyhow::ensure!(
        backing_body == b"after-fuse",
        "mount smoke write: backing file not updated through FUSE"
    );
    verify_mount_smoke_provenance(&backing_seed)?;

    // Create, mkdir, unlink, rename round-trip (AC-009.16).
    let new_file = session.mountpoint().join("smoke-new.txt");
    std::fs::write(&new_file, b"created-via-fuse")?;
    anyhow::ensure!(
        std::fs::read(backing.join("smoke-new.txt"))? == b"created-via-fuse",
        "mount smoke create: backing file missing after FUSE create"
    );
    verify_mount_smoke_provenance(&backing.join("smoke-new.txt"))?;

    let new_dir = session.mountpoint().join("smoke-dir");
    std::fs::create_dir(&new_dir)?;
    anyhow::ensure!(backing.join("smoke-dir").is_dir(), "mount smoke mkdir: backing dir missing");

    std::fs::remove_file(&new_file)?;
    anyhow::ensure!(
        !backing.join("smoke-new.txt").exists(),
        "mount smoke unlink: backing file still present"
    );

    let rename_src = session.mountpoint().join("smoke-rename.txt");
    std::fs::write(&rename_src, b"rename-me")?;
    let rename_dst = session.mountpoint().join("smoke-renamed.txt");
    std::fs::rename(&rename_src, &rename_dst)?;
    anyhow::ensure!(
        backing.join("smoke-renamed.txt").is_file(),
        "mount smoke rename: destination missing on backing"
    );
    std::fs::rename(&rename_dst, &rename_src)?;
    anyhow::ensure!(
        backing.join("smoke-rename.txt").is_file(),
        "mount smoke rename round-trip: source not restored on backing"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::annotate_write;

    #[test]
    fn verify_mount_smoke_provenance_accepts_annotated_backing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tracked.txt");
        std::fs::write(&path, b"payload").expect("write");
        annotate_write(&path, &default_session_id()).expect("annotate");
        verify_mount_smoke_provenance(&path).expect("provenance must validate");
    }

    #[test]
    fn verify_mount_smoke_provenance_fails_loudly_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("plain.txt");
        std::fs::write(&path, b"plain").expect("write");
        let err = verify_mount_smoke_provenance(&path).expect_err("must fail without xattrs");
        assert!(
            err.to_string().contains("missing xattrs"),
            "loud error must mention missing xattrs: {err}"
        );
    }

    #[test]
    fn smoke_env_disabled_by_default() {
        let prev = std::env::var(ENV_FUSE_MOUNT_SMOKE).ok();
        std::env::remove_var(ENV_FUSE_MOUNT_SMOKE);
        assert!(!fuse_mount_smoke_enabled());
        std::env::set_var(ENV_FUSE_MOUNT_SMOKE, "1");
        assert!(fuse_mount_smoke_enabled());
        std::env::set_var(ENV_FUSE_MOUNT_SMOKE, "true");
        assert!(fuse_mount_smoke_enabled());
        match prev {
            Some(v) => std::env::set_var(ENV_FUSE_MOUNT_SMOKE, v),
            None => std::env::remove_var(ENV_FUSE_MOUNT_SMOKE),
        }
    }
}
