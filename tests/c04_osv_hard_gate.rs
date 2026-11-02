// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C04 L38 — OSV / GHSA hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Strip YAML comments so an assertion cannot pass by matching a commented-out
/// or documented-but-inactive line. A `#` that is not inside a quoted scalar
/// starts a comment; the workflows under test use plain scalars, so a
/// line-leading / whitespace-preceded `#` is the correct rule here.
fn strip_yaml_comments(src: &str) -> String {
    src.lines()
        .map(|line| match line.find('#') {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// FR-003 / C04 L38 — the OSV / GHSA scan must be hard (no soft shim) and must
/// run on PRs. The PR-time `osv` job used to live in the deleted
/// `.github/workflows/ci.yml`; it now lives in `osv.yml` with a
/// `pull_request` trigger, so this gate is asserted against that workflow.
#[test]
fn fr003_osv_yml_osv_hard_gate_wired() {
    let osv =
        fs::read_to_string(repo_root().join(".github/workflows/osv.yml")).expect("read osv.yml");
    // Assertions run against the comment-stripped workflow: a claim that only
    // exists in a comment (for example a documented-but-unimplemented
    // `--severity=HIGH,CRITICAL` flag) must not satisfy the gate.
    let active = strip_yaml_comments(&osv);

    assert!(
        active.contains("name: OSV / GHSA lockfile scan"),
        "osv.yml must define the OSV / GHSA lockfile scan job"
    );
    // osv-scanner v2.x has no `--severity` CLI flag, so the severity policy is
    // enforced by the config file, not by a scanner argument. Assert the
    // ACTIVE enforcement mechanism: the scanner must consume the config that
    // carries the policy, and the scan must not be soft-failed.
    assert!(
        active.contains("--config=osv-scanner.toml"),
        "osv.yml OSV job must run the scanner against osv-scanner.toml, which carries the severity policy"
    );
    assert!(
        active.contains("--lockfile=Cargo.lock"),
        "osv.yml OSV job must scan the workspace Cargo.lock"
    );
    assert!(
        !active.contains("Soft gate (always pass)"),
        "osv.yml must not contain OSV soft pass shim"
    );
    // The scan step itself must be a hard step: `continue-on-error: true` is
    // only acceptable on the SARIF upload, never on the scan.
    let scan_block = active
        .split("scan-args:")
        .nth(1)
        .and_then(|rest| rest.split("continue-on-error").next())
        .unwrap_or("");
    assert!(
        !scan_block.contains("continue-on-error: true"),
        "the OSV scan step must not be soft-failed via continue-on-error"
    );

    // PR-time coverage must be restored: the deleted ci.yml `osv` job fed the
    // `ci-success` aggregator, so the surviving workflow must trigger on PRs.
    assert!(
        active.contains("pull_request:"),
        "osv.yml must trigger on pull_request so the deleted ci.yml OSV coverage is not lost"
    );
}

/// FR-003 / C04 L38 — standalone `osv.yml` stays hard (no continue-on-error on scan).
#[test]
fn fr003_osv_yml_no_soft_shim() {
    let osv =
        fs::read_to_string(repo_root().join(".github/workflows/osv.yml")).expect("read osv.yml");
    let active = strip_yaml_comments(&osv);

    assert!(!active.contains("Soft gate (always pass)"), "osv.yml must not contain soft pass shim");
    // The severity policy lives in osv-scanner.toml (osv-scanner v2.x has no
    // `--severity` CLI flag), so the gate must assert that the scanner is
    // actually pointed at that config. Asserting the literal
    // `--severity=HIGH,CRITICAL` string would pass on a commented-out line and
    // prove nothing about enforcement.
    assert!(
        active.contains("--config=osv-scanner.toml"),
        "osv.yml must scan with osv-scanner.toml, which carries the HIGH/CRITICAL severity policy"
    );

    let scan_section =
        active.split("Scan Cargo.lock with OSV-Scanner").nth(1).expect("osv.yml scan step");
    let scan_block = scan_section.split("Upload SARIF").next().expect("scan block before SARIF");

    assert!(
        !scan_block.contains("continue-on-error: true"),
        "OSV scan step must not use continue-on-error"
    );
}
