// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C04 L38 — OSV / GHSA hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C04 L38 — the OSV / GHSA scan must be hard (no soft shim) and must
/// run on PRs. The PR-time `osv` job used to live in the deleted
/// `.github/workflows/ci.yml`; it now lives in `osv.yml` with a
/// `pull_request` trigger, so this gate is asserted against that workflow.
#[test]
fn fr003_osv_yml_osv_hard_gate_wired() {
    let osv =
        fs::read_to_string(repo_root().join(".github/workflows/osv.yml")).expect("read osv.yml");

    assert!(
        osv.contains("name: OSV / GHSA lockfile scan"),
        "osv.yml must define the OSV / GHSA lockfile scan job"
    );
    assert!(
        osv.contains("--severity=HIGH,CRITICAL"),
        "osv.yml OSV job must fail on HIGH/CRITICAL only"
    );
    assert!(
        !osv.contains("Soft gate (always pass)"),
        "osv.yml must not contain OSV soft pass shim"
    );

    // PR-time coverage must be restored: the deleted ci.yml `osv` job fed the
    // `ci-success` aggregator, so the surviving workflow must trigger on PRs.
    assert!(
        osv.contains("pull_request:"),
        "osv.yml must trigger on pull_request so the deleted ci.yml OSV coverage is not lost"
    );
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
