// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C11 L121 — `sharecli uninstall` subcommand (FR-003).
//!
//! FR: FR-003

use std::process::Command;

#[test]
fn fr003_uninstall_prints_package_manager_guidance() {
    let bin = env!("CARGO_BIN_EXE_sharecli");
    let output = Command::new(bin).arg("uninstall").output().expect("run sharecli uninstall");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo uninstall sharecli"));
    assert!(stdout.contains("config:"));
    assert!(stdout.contains("--purge-data"));
}

#[test]
fn fr003_uninstall_dry_run_purge_does_not_error() {
    let bin = env!("CARGO_BIN_EXE_sharecli");
    let output = Command::new(bin)
        .args(["uninstall", "--purge-data", "--dry-run"])
        .output()
        .expect("run sharecli uninstall --purge-data --dry-run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dry run"));
}
