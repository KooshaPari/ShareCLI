// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-009
//!
//! C01 — FUSE backend selection / fallback depth tests.
//!
//! Exercises the backend negotiation contract in `sharecli-fuse/src/backend.rs`:
//!   - `SHARECLI_FUSE_BACKEND=fskit` always selects the FSKit path.
//!   - `SHARECLI_FUSE_BACKEND=kernel` selects the kernel path ONLY when
//!     the macFUSE kext is actually loaded; otherwise degrades closed.
//!   - Invalid overrides (`unknown`, empty, garbage) MUST degrade to
//!     `FuseBackend::Unavailable` so the caller never silently picks a
//!     path the platform cannot satisfy.
//!   - When no env var is set, the auto-negotiation prefers the kernel
//!     backend when loaded and falls back to FSKit (consistent with the
//!     macOS latency note in `backend.rs`).
//!   - Mount failure error messages MUST contain both the kernel attempt
//!     failure and the FSKit fallback failure (when the Kernel branch
//!     was selected) so operators can diagnose macFUSE misconfiguration
//!     from logs alone.
//!
//! All env-var mutating tests are `#[serial_test::serial]` to avoid
//! cross-test contamination of `SHARECLI_FUSE_BACKEND`. Tests do NOT
//! require a live FUSE mount — they exercise the pure negotiation
//! contract and `mount_with_session`'s error-shape contract.
//!
//! Platform split: the override-handling tests (fskit / kernel / invalid /
//! deterministic) run on Linux and macOS because `select_backend` honors
//! `SHARECLI_FUSE_BACKEND` on both. The no-override test and the two
//! mount-error-envelope tests are macOS-only: on Linux/Windows the backend is
//! documented as always `Unavailable` (`backend.rs`) and the Linux mount path
//! goes straight to `fuser::mount` without backend negotiation, so those
//! assertions cannot hold there.

#![cfg(any(target_os = "linux", target_os = "macos"))]

//! The mount-error-envelope tests below are macOS-only; on Linux their
//! imports would be unused, so gate them alongside.

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;
use std::sync::Mutex;

use sharecli_fuse::{select_backend, FuseBackend};
#[cfg(target_os = "macos")]
use tempfile::TempDir;

/// Serialize env-var-mutating tests so the `SHARECLI_FUSE_BACKEND` slot
/// never carries state across tests. (Different env var keys can run in
/// parallel; this guard is only for the shared `SHARECLI_FUSE_BACKEND`.)
static BACKEND_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Backend selection — explicit `fskit` override MUST return `Fskit`,
/// regardless of whether the macFUSE kext is loaded.
#[test]
#[serial_test::serial]
fn backend_select_fskit_override_is_fskit() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    std::env::set_var("SHARECLI_FUSE_BACKEND", "fskit");
    let backend = select_backend();
    restore_env("SHARECLI_FUSE_BACKEND", prev);
    assert_eq!(
        backend,
        FuseBackend::Fskit,
        "explicit fskit override MUST return Fskit (got {backend:?})"
    );
}

/// Backend selection — uppercase / mixed-case `FSKIT` must be honored
/// (the override matcher lowercases the value).
#[test]
#[serial_test::serial]
fn backend_select_fskit_override_is_case_insensitive() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    std::env::set_var("SHARECLI_FUSE_BACKEND", "FSKit");
    let backend = select_backend();
    restore_env("SHARECLI_FUSE_BACKEND", prev);
    assert_eq!(
        backend,
        FuseBackend::Fskit,
        "FSKit override (mixed case) MUST return Fskit (got {backend:?})"
    );
}

/// Backend selection — garbage values degrade closed (`Unavailable`)
/// instead of silently picking an unsupported backend.
#[test]
#[serial_test::serial]
fn backend_select_invalid_override_degrades_closed() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    for value in ["invalid", "", "bogus", "kernel-but-typo"] {
        std::env::set_var("SHARECLI_FUSE_BACKEND", value);
        let backend = select_backend();
        assert_eq!(
            backend,
            FuseBackend::Unavailable,
            "override {value:?} MUST degrade to Unavailable (got {backend:?})"
        );
    }
    restore_env("SHARECLI_FUSE_BACKEND", prev);
}

/// Backend selection — `kernel` override returns either `Kernel` (when
/// the macFUSE kext is loaded) or `Unavailable` (when the kext is not
/// loaded on this host). Either result is a valid contract; the test
/// only asserts that the override is at least considered and never
/// silently falls back to FSKit.
#[test]
#[serial_test::serial]
fn backend_select_kernel_override_is_respected() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    std::env::set_var("SHARECLI_FUSE_BACKEND", "kernel");
    let backend = select_backend();
    restore_env("SHARECLI_FUSE_BACKEND", prev);
    assert_ne!(
        backend,
        FuseBackend::Fskit,
        "explicit kernel override MUST NOT silently fall back to Fskit (got {backend:?})"
    );
    assert!(
        matches!(backend, FuseBackend::Kernel | FuseBackend::Unavailable),
        "kernel override MUST yield Kernel or Unavailable (got {backend:?})"
    );
    // On Linux the macFUSE kext can never load (`kmutil` does not exist), so
    // the kernel override degrades closed deterministically — a mutant that
    // widens the loaded-guard to unconditional would return Kernel here.
    #[cfg(target_os = "linux")]
    assert_eq!(
        backend,
        FuseBackend::Unavailable,
        "on Linux the kernel override MUST degrade to Unavailable (got {backend:?})"
    );
}

/// Backend selection — when no env var is set, `select_backend` returns
/// either `Kernel` (when loaded) or `Fskit` (the documented macOS
/// fallback). It MUST never return `Unavailable` without an explicit
/// override — operators who did not opt out expect at least one of the
/// two macFUSE paths to be picked.
///
/// On non-macOS the contract is the mirror image: the FUSE layer is
/// platform-native (libfuse3 / WinFsp) and `backend.rs` documents the
/// backend as always `Unavailable`, so no-override MUST degrade to
/// `Unavailable` rather than pretending a macFUSE backend exists.
#[test]
#[serial_test::serial]
fn backend_select_no_override_picks_kernel_or_fskit() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    std::env::remove_var("SHARECLI_FUSE_BACKEND");
    let backend = select_backend();
    restore_env("SHARECLI_FUSE_BACKEND", prev);
    #[cfg(target_os = "macos")]
    assert_ne!(
        backend,
        FuseBackend::Unavailable,
        "no-override selection MUST yield Kernel or Fskit (got {backend:?})"
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        backend,
        FuseBackend::Unavailable,
        "non-macOS has no macFUSE backend; no-override MUST degrade to Unavailable (got {backend:?})"
    );
}

/// Backend selection — calling `select_backend` twice with the same
/// state MUST return the same value. This catches any accidental
/// stateful caching that would break the auto-negotiation contract.
#[test]
#[serial_test::serial]
fn backend_select_is_deterministic_for_same_state() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    std::env::remove_var("SHARECLI_FUSE_BACKEND");
    let first = select_backend();
    let second = select_backend();
    let third = select_backend();
    restore_env("SHARECLI_FUSE_BACKEND", prev);
    assert_eq!(first, second, "select_backend drifted between calls 1 and 2");
    assert_eq!(second, third, "select_backend drifted between calls 2 and 3");
}

/// Mount failure — when `SHARECLI_FUSE_BACKEND=invalid` is set, the
/// error from `mount_with_session` MUST identify the unavailable path
/// (so the operator can see *why* the mount failed without digging into
/// `kmutil`). We don't require a live FUSE backend for this test — we
/// only require the error envelope to be correct.
///
/// macOS-only: the Linux mount path skips backend negotiation entirely
/// (`fuser::mount` direct), so the backend-unavailable envelope exists
/// only on macOS.
#[test]
#[serial_test::serial]
#[cfg(target_os = "macos")]
fn mount_failure_unavailable_backend_mentions_backend() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    std::env::set_var("SHARECLI_FUSE_BACKEND", "invalid");

    let backing = TempDir::new().expect("backing tempdir");
    let seed = Path::new("seed-unavail.txt");
    fs::write(backing.path().join(seed), b"x").expect("seed");

    // Use a real tempdir as the mountpoint so the kernel mount path at
    // least gets past the directory-existence check before failing on
    // the unavailable backend.
    let mountpoint = TempDir::new().expect("mountpoint tempdir");

    let err = sharecli_fuse::mount_with_session(mountpoint.path(), backing.path(), "sess-unavail")
        .expect_err("mount_with_session MUST fail with invalid backend override");
    let msg = err.to_string();
    assert!(
        msg.contains("backend") || msg.contains("Backend") || msg.contains("unavailable"),
        "error envelope MUST mention backend/unavailable (got {msg:?})"
    );

    // Best-effort cleanup in case the failure path left a residual mount.
    let _ = sharecli_fuse::force_unmount(mountpoint.path());
    restore_env("SHARECLI_FUSE_BACKEND", prev);
}

/// Mount failure — when an invalid mountpoint path is supplied
/// (`/definitely/does/not/exist`), the resulting error envelope MUST
/// surface the mountpoint path so the operator can correlate the
/// failure with their `--mountpoint` flag. This catches regressions in
/// the error wrapping in `lib.rs`.
///
/// macOS-only: on Linux `fuser::mount` reports bare io errors (e.g.
/// `No such file or directory`) without the mountpoint, and there is no
/// macFUSE negotiation error to wrap.
#[test]
#[serial_test::serial]
#[cfg(target_os = "macos")]
fn mount_failure_invalid_mountpoint_mentions_path() {
    let _guard = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("SHARECLI_FUSE_BACKEND").ok();
    std::env::remove_var("SHARECLI_FUSE_BACKEND");

    let backing = TempDir::new().expect("backing tempdir");
    let bogus_mountpoint = Path::new("/definitely/does/not/exist/sharecli-fuse-test");
    let err = sharecli_fuse::mount_with_session(bogus_mountpoint, backing.path(), "sess-bad-mp")
        .expect_err("mount at nonexistent path MUST fail");

    let msg = err.to_string();
    // The lib.rs `mount_with_session` wraps the mount failure with the
    // mountpoint path; we accept either the full path or a leaf token.
    assert!(
        msg.contains("does/not/exist") || msg.contains("does\\not\\exist") || msg.contains("bogus"),
        "error envelope MUST surface the bogus mountpoint (got {msg:?})"
    );

    restore_env("SHARECLI_FUSE_BACKEND", prev);
}

fn restore_env(key: &str, prev: Option<String>) {
    match prev {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}
