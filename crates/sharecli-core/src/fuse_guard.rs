//! FUSE IO-intercept guard.
//!
//! The hypervisor mounts a sharecli-fuse IO-intercept layer over the child's
//! working directory on every cache-miss spawn.  [`FuseGuard`] is the RAII
//! wrapper that owns the mountpoint and force-unmounts on drop; on non-FUSE
//! platforms or when the mount fails, it transparently degrades to a no-op
//! guard so the spawn proceeds without interception.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use sharecli_ipc::CommandKey;
use tracing::error;

// ---------------------------------------------------------------------------
// FUSE readiness constants
// ---------------------------------------------------------------------------

/// Max time to wait for the FUSE mount to become readable after spawn.
pub(crate) const FUSE_READY_TIMEOUT: Duration = Duration::from_secs(5);
/// Poll interval while waiting for mount readiness.
pub(crate) const FUSE_READY_POLL: Duration = Duration::from_millis(50);
/// Marker filename written under `backing` and probed via the mountpoint.
pub(crate) const FUSE_READY_MARKER: &str = ".sharecli-fuse-ready";

// ---------------------------------------------------------------------------
// Session id derivation
// ---------------------------------------------------------------------------

/// Derive the FUSE write-provenance session id for a coalesce [`CommandKey`].
///
/// Hypervisor cache-miss FUSE mounts pass this to [`sharecli_fuse::mount_with_session`]
/// so agent writes through the intercept layer correlate with the Lock-Wait-Cache key
/// (AC-009.12).
pub fn fuse_session_id_for_command_key(key: &CommandKey) -> String {
    let prefix = key.0.get(..16).unwrap_or(key.0.as_str());
    format!("hv-{prefix}")
}

// ---------------------------------------------------------------------------
// Mount readiness poll
// ---------------------------------------------------------------------------

/// Poll until `probe` is readable, `fail_rx` reports a mount error, or `timeout`.
///
/// Returns `Ok(())` when the probe path becomes a readable file. Returns `Err`
/// with a loud, actionable message on mount failure or timeout — never silently
/// treats an unmounted directory as ready.
pub(crate) fn wait_fuse_mount_ready(
    probe: &Path,
    fail_rx: &mpsc::Receiver<String>,
    timeout: Duration,
    poll: Duration,
) -> Result<(), String> {
    let mut waited = Duration::ZERO;
    while waited < timeout {
        if probe.is_file() && std::fs::read(probe).is_ok() {
            return Ok(());
        }
        if let Ok(msg) = fail_rx.try_recv() {
            return Err(format!("sharecli-fuse mount failed: {msg}"));
        }
        thread::sleep(poll);
        waited = waited.saturating_add(poll);
    }
    Err(format!(
        "sharecli-fuse mount readiness timed out after {}ms waiting for {} \
         (is FUSE installed and permitted?)",
        timeout.as_millis(),
        probe.display()
    ))
}

// ---------------------------------------------------------------------------
// FuseGuard (RAII mount lifetime)
// ---------------------------------------------------------------------------

/// RAII guard that manages a sharecli-fuse intercept mount lifetime.
///
/// On construction (best-effort) it creates a temporary directory, spawns a
/// background thread running the FUSE event loop that mirrors a backing path,
/// polls until a readiness marker is visible through the mount, and exposes
/// the mountpoint via [`mountpoint()`][FuseGuard::mountpoint].
///
/// On drop the guard force-unmounts the FUSE filesystem and removes the
/// temporary mountpoint directory.
///
/// # Best-effort semantics
///
/// [`FuseGuard::try_mount`] **never** returns an error to callers.  If mounting
/// fails or readiness polling times out, a loud message is printed to stderr /
/// the error log, a no-op guard is returned, and
/// [`mountpoint()`][FuseGuard::mountpoint] returns `None`. Callers must check
/// `mountpoint()` and fall back to the original path when it returns `None`.
/// An unready mountpoint is **never** returned as `Some`.
pub(crate) struct FuseGuard {
    /// Path to the temporary mountpoint directory.
    /// `None` when FUSE could not be started (no-op guard).
    mountpoint: Option<PathBuf>,
    /// Keep the TempDir alive so it is not cleaned up before the unmount in
    /// [`Drop`] runs.
    _tmpdir: Option<tempfile::TempDir>,
    /// Readiness marker path on the backing tree (removed on drop).
    readiness_marker: Option<PathBuf>,
}

impl FuseGuard {
    /// Attempt to mount a sharecli-fuse IO-intercept layer mirroring `backing`.
    ///
    /// When FUSE is unavailable, the mount fails, or readiness never appears, a
    /// no-op guard is returned (the spawn proceeds without interception) after a
    /// loud failure report.
    pub(crate) fn try_mount(backing: &Path, session_id: &str) -> Self {
        // Non-Linux/macOS: no-op guard — FUSE is not available.
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (backing, session_id);
            return Self { mountpoint: None, _tmpdir: None, readiness_marker: None };
        }

        // Linux / macOS: try to start FUSE and wait until it is ready.
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let tmpdir = match tempfile::tempdir() {
                Ok(d) => d,
                Err(e) => {
                    let msg = format!(
                        "sharecli-fuse: cannot create mount tempdir: {e} — proceeding without FUSE"
                    );
                    eprintln!("{msg}");
                    error!("{msg}");
                    return Self { mountpoint: None, _tmpdir: None, readiness_marker: None };
                }
            };
            let mountpoint = tmpdir.path().to_path_buf();
            let backing = backing.to_path_buf();

            // Unique marker under backing; InterceptFs mirrors it through the mount.
            let marker_name = format!("{FUSE_READY_MARKER}-{}", std::process::id());
            let marker_rel = Path::new(&marker_name);
            let marker_backing = backing.join(marker_rel);
            if let Err(e) = std::fs::write(&marker_backing, b"ready") {
                let msg = format!(
                    "sharecli-fuse: cannot write readiness marker {}: {e} — proceeding without FUSE",
                    marker_backing.display()
                );
                eprintln!("{msg}");
                error!("{msg}");
                return Self { mountpoint: None, _tmpdir: None, readiness_marker: None };
            }

            let mp = mountpoint.clone();
            let backing_for_mount = backing.clone();
            let session = session_id.to_string();
            let (fail_tx, fail_rx) = mpsc::channel::<String>();
            // Spawn the FUSE event loop on a background thread — it blocks
            // until unmounted.
            thread::spawn(move || {
                if let Err(err) =
                    sharecli_fuse::mount_with_session(&mp, &backing_for_mount, &session)
                {
                    let _ = fail_tx.send(err.to_string());
                }
            });

            let probe = mountpoint.join(marker_rel);
            match wait_fuse_mount_ready(&probe, &fail_rx, FUSE_READY_TIMEOUT, FUSE_READY_POLL) {
                Ok(()) => Self {
                    mountpoint: Some(mountpoint),
                    _tmpdir: Some(tmpdir),
                    readiness_marker: Some(marker_backing),
                },
                Err(msg) => {
                    let loud = format!("{msg} — proceeding without FUSE intercept");
                    eprintln!("{loud}");
                    error!("{loud}");
                    // Best-effort unmount of a half-started mount before drop.
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("fusermount")
                            .arg("-uz")
                            .arg(&mountpoint)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("umount")
                            .arg(&mountpoint)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status();
                    }
                    let _ = std::fs::remove_file(&marker_backing);
                    Self { mountpoint: None, _tmpdir: None, readiness_marker: None }
                }
            }
        }
    }

    /// The mountpoint directory, or `None` if the guard is a no-op.
    pub(crate) fn mountpoint(&self) -> Option<&Path> {
        self.mountpoint.as_deref()
    }
}

impl Drop for FuseGuard {
    fn drop(&mut self) {
        if let Some(ref mp) = self.mountpoint {
            // Force-unmount (the `_tmpdir` field is dropped after this,
            // which removes the now-empty mountpoint directory).
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("fusermount").arg("-uz").arg(mp).status();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("umount").arg(mp).status();
            }
        }
        if let Some(ref marker) = self.readiness_marker {
            let _ = std::fs::remove_file(marker);
        }
    }
}

// ---------------------------------------------------------------------------
// FUSE unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use sharecli_ipc::{command_key, CommandKey};
    use tempfile::TempDir;

    use super::{
        fuse_session_id_for_command_key, wait_fuse_mount_ready, FuseGuard, FUSE_READY_MARKER,
    };

    /// FuseGuard::try_mount never panics and returns a valid object on all
    /// platforms.  On non-FUSE platforms the guard is a no-op; on Linux/macOS
    /// it may or may not succeed depending on the FUSE kernel module. When the
    /// mount is not ready, mountpoint() MUST be None (never an unready Some).
    #[test]
    fn fuse_guard_try_mount_never_panics() {
        let dir = TempDir::new().expect("tempdir");
        let guard = FuseGuard::try_mount(dir.path(), "hv-test-session");
        // The guard object is valid regardless of whether FUSE is active.
        // Dropping it must not panic.
        drop(guard);
    }

    /// When FUSE is active the mountpoint must differ from the backing path
    /// and the readiness marker must be readable through it. When FUSE is
    /// inactive mountpoint() returns None — the no-op guard.
    #[test]
    fn fuse_guard_mountpoint_returns_path_or_none() {
        let dir = TempDir::new().expect("tempdir");
        let guard = FuseGuard::try_mount(dir.path(), "hv-test-session");
        match guard.mountpoint() {
            Some(mp) => {
                // FUSE is active — mountpoint must be different from backing.
                assert_ne!(mp, dir.path(), "mountpoint must differ from backing");
                assert!(mp.starts_with(std::env::temp_dir()), "mountpoint must be under temp dir");
                let probe_found = std::fs::read_dir(mp)
                    .expect("read mountpoint")
                    .filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().starts_with(FUSE_READY_MARKER));
                assert!(
                    probe_found,
                    "readiness marker must be visible through mountpoint before Some is returned"
                );
            }
            None => {
                // FUSE not available — best-effort, still valid.
            }
        }
    }

    /// FR-009 / AC-009.14 — FuseGuard stays mounted until drop (spawn/teardown lifecycle).
    #[test]
    fn fuse_guard_teardown_after_drop() {
        let dir = TempDir::new().expect("tempdir");
        let mountpoint = {
            let guard = FuseGuard::try_mount(dir.path(), "hv-teardown-session");
            guard.mountpoint().map(|p| p.to_path_buf())
        };
        if let Some(mp) = mountpoint {
            assert_ne!(mp, dir.path());
            // Guard drop runs force-unmount; mount tempdir may remain but must not
            // still expose the readiness marker tree as a live FUSE mount.
            let still_mounted = std::fs::read_dir(&mp).ok().is_some_and(|mut rd| {
                rd.any(|e| {
                    e.ok().is_some_and(|ent| {
                        ent.file_name().to_string_lossy().starts_with(FUSE_READY_MARKER)
                    })
                })
            });
            assert!(!still_mounted, "FUSE intercept MUST tear down on guard drop (AC-009.14)");
        }
    }

    /// Readiness poll reports mount failure immediately when fail_rx fires.
    #[test]
    fn fuse_ready_poll_fails_loudly_on_mount_error() {
        let (tx, rx) = mpsc::channel();
        tx.send("simulated mount boom".into()).expect("send");
        let probe = Path::new("/tmp/sharecli-fuse-ready-poll-never");
        let err =
            wait_fuse_mount_ready(probe, &rx, Duration::from_secs(2), Duration::from_millis(10))
                .expect_err("must fail when mount reports error");
        assert!(err.contains("mount failed"), "loud error must mention mount failure: {err}");
        assert!(
            err.contains("simulated mount boom"),
            "loud error must include mount message: {err}"
        );
    }

    /// Readiness poll times out with a loud message when the probe never appears.
    #[test]
    fn fuse_ready_poll_times_out_loudly_when_probe_missing() {
        let (_tx, rx) = mpsc::channel::<String>();
        let probe = Path::new("/tmp/sharecli-fuse-ready-poll-missing-xyz");
        let err =
            wait_fuse_mount_ready(probe, &rx, Duration::from_millis(80), Duration::from_millis(20))
                .expect_err("must time out when probe never appears");
        assert!(err.contains("timed out"), "loud error must mention timeout: {err}");
        assert!(err.contains("sharecli-fuse"), "loud error must identify sharecli-fuse: {err}");
    }

    /// Readiness poll succeeds when the probe file becomes readable.
    #[test]
    fn fuse_ready_poll_succeeds_when_probe_appears() {
        let dir = TempDir::new().expect("tempdir");
        let probe = dir.path().join("appears.txt");
        let (_tx, rx) = mpsc::channel::<String>();
        let probe_bg = probe.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            std::fs::write(&probe_bg, b"ready").expect("write probe");
        });
        wait_fuse_mount_ready(&probe, &rx, Duration::from_secs(2), Duration::from_millis(10))
            .expect("probe must become ready");
    }

    /// AC-009.12 — FUSE session id is deterministic and keyed from CommandKey prefix.
    #[test]
    fn fuse_session_id_for_command_key_is_deterministic() {
        let key = command_key(&["cargo".into(), "build".into()], Path::new("/repo"), &[]);
        let a = fuse_session_id_for_command_key(&key);
        let b = fuse_session_id_for_command_key(&key);
        assert_eq!(a, b);
        assert!(a.starts_with("hv-"));
        assert_eq!(a.len(), 19);
    }

    /// fuse_session_id_for_command_key truncates short keys gracefully.
    #[test]
    fn fuse_session_id_for_short_command_key() {
        // Key shorter than 16 chars — prefix should be the full key.
        let key = CommandKey("abc".into());
        let id = fuse_session_id_for_command_key(&key);
        assert!(id.starts_with("hv-"));
        assert_eq!(id, "hv-abc");
    }

    /// fuse_session_id_for_command_key uses exactly 16 chars from the key.
    #[test]
    fn fuse_session_id_uses_exactly_16_char_prefix() {
        let key = CommandKey("0123456789abcdef12345678".into());
        let id = fuse_session_id_for_command_key(&key);
        assert_eq!(id, "hv-0123456789abcdef");
        assert_eq!(id.len(), 19); // "hv-" + 16 hex chars
    }
}
