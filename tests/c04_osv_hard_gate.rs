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
    // only acceptable on the SARIF upload, never on the scan. Inspect the
    // step as a parsed YAML mapping - a text slice that starts after the
    // step's `name:` line would miss a key placed before it, and a slice
    // that stops at the first `continue-on-error` occurrence would remove
    // the very token it is looking for.
    let doc: serde_yaml::Value = serde_yaml::from_str(&osv).expect("osv.yml must be valid YAML");
    let jobs = doc
        .get("jobs")
        .and_then(serde_yaml::Value::as_mapping)
        .expect("osv.yml must define a jobs mapping");
    let osv_job = jobs
        .get(serde_yaml::Value::String("osv".to_string()))
        .expect("osv.yml must define the osv job");
    let steps = osv_job
        .get("steps")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("osv job must declare steps");

    let scan_step = steps
        .iter()
        .find(|step| step.get("with").and_then(|w| w.get("scan-args")).is_some())
        .expect("osv job must have a step that invokes the scanner");
    assert!(
        scan_step.get("continue-on-error").and_then(serde_yaml::Value::as_bool) != Some(true),
        "the OSV scan step must not be soft-failed via continue-on-error"
    );

    // The SARIF upload may be soft (it is best-effort reporting), but only
    // that step may be: no OTHER step may carry continue-on-error.
    for step in steps.iter() {
        let soft = step.get("continue-on-error").and_then(serde_yaml::Value::as_bool) == Some(true);
        if !soft {
            continue;
        }
        let name = step.get("name").and_then(serde_yaml::Value::as_str).unwrap_or("");
        assert!(
            name.contains("SARIF"),
            "only the SARIF upload step may be soft-failed, found a soft step named {name:?}"
        );
    }

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

    // Inspect the whole scan step as a parsed mapping rather than slicing
    // text: a slice that starts after the step's `name:` line misses a key
    // placed before it, and a slice that stops at the first
    // `continue-on-error` occurrence removes the very token being searched
    // for.
    let doc: serde_yaml::Value = serde_yaml::from_str(&osv).expect("osv.yml must be valid YAML");
    let jobs = doc
        .get("jobs")
        .and_then(serde_yaml::Value::as_mapping)
        .expect("osv.yml must define a jobs mapping");
    let osv_job = jobs
        .get(serde_yaml::Value::String("osv".to_string()))
        .expect("osv.yml must define the osv job");
    let steps = osv_job
        .get("steps")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("osv job must declare steps");
    let scan_step = steps
        .iter()
        .find(|step| step.get("with").and_then(|w| w.get("scan-args")).is_some())
        .expect("osv job must have a step that invokes the scanner");

    assert!(
        scan_step.get("continue-on-error").and_then(serde_yaml::Value::as_bool) != Some(true),
        "OSV scan step must not use continue-on-error"
    );
}
