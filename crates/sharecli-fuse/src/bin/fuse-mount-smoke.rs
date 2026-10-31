// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Privileged mount smoke binary (AC-009.8 / AC-009.16 / AC-009.25).
//! Prefer this over full `sharecli` package tests inside Containerfile.fuse-smoke.

fn main() {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        use sharecli_fuse::{fuse_mount_smoke_enabled, run_mount_smoke, ENV_FUSE_MOUNT_SMOKE};
        if !fuse_mount_smoke_enabled() {
            eprintln!("fuse-mount-smoke: set {ENV_FUSE_MOUNT_SMOKE}=1 to run privileged smoke");
            std::process::exit(2);
        }
        #[cfg(windows)]
        {
            if !sharecli_fuse::winfsp_installed() {
                eprintln!("fuse-mount-smoke: FAIL: WinFsp not installed (winfsp_missing)");
                std::process::exit(1);
            }
        }
        let dir = tempfile::tempdir().expect("tempdir");
        if let Err(e) = run_mount_smoke(dir.path()) {
            eprintln!("fuse-mount-smoke: FAIL: {e:#}");
            std::process::exit(1);
        }
        println!("fuse-mount-smoke: PASS");
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        eprintln!("fuse-mount-smoke: unsupported platform");
        std::process::exit(2);
    }
}
