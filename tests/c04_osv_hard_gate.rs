// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C04 L38 — OSV / GHSA hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C04 L38 — `ci.yml` must aggregate OSV into `ci-success` without soft shims.
#[test]
fn fr003_ci_yml_osv_hard_gate_wired() {
    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("read ci.yml");

    assert!(
        ci.contains("name: OSV / GHSA lockfile scan (required)"),
        "ci.yml must define required OSV job"
    );
    assert!(
        ci.contains("--severity=HIGH,CRITICAL"),
        "ci.yml OSV job must fail on HIGH/CRITICAL only"
    );
    assert!(!ci.contains("Soft gate (always pass)"), "ci.yml must not contain OSV soft pass shim");

    let needs_line = ci
        .lines()
        .find(|l| l.contains("needs:") && l.contains("ci-success"))
        .or_else(|| {
            ci.lines().skip_while(|l| !l.contains("ci-success:")).find(|l| l.contains("needs:"))
        })
        .expect("ci-success needs block");

    assert!(needs_line.contains("osv"), "ci-success must need osv job; got: {needs_line}");
}

/// FR-003 / C04 L38 — standalone `osv.yml` stays hard (no continue-on-error on scan).
#[test]
fn fr003_osv_yml_no_soft_shim() {
    let osv =
        fs::read_to_string(repo_root().join(".github/workflows/osv.yml")).expect("read osv.yml");

    assert!(!osv.contains("Soft gate (always pass)"), "osv.yml must not contain soft pass shim");
    assert!(osv.contains("--severity=HIGH,CRITICAL"), "osv.yml must scan HIGH/CRITICAL severity");

    let scan_section =
        osv.split("Scan Cargo.lock with OSV-Scanner").nth(1).expect("osv.yml scan step");
    let scan_block = scan_section.split("Upload SARIF").next().expect("scan block before SARIF");

    assert!(
        !scan_block.contains("continue-on-error: true"),
        "OSV scan step must not use continue-on-error"
    );
}
