// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 acceptance gates for C07 L69 — Cross-platform CI.
//!
//! These tests assert that the cross-platform CI infrastructure is
//! real, correctly configured, and covers the expected target matrix.

use std::path::Path;

/// Root of the repository (three levels up from tests/)
fn repo_root() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

fn workflow_path() -> String {
    format!("{}/.github/workflows/cross-platform.yml", repo_root())
}

fn read_workflow() -> String {
    std::fs::read_to_string(workflow_path()).expect("cross-platform.yml must exist")
}

// ---------------------------------------------------------------------------
// Gate 1: workflow file exists and is valid YAML (parses without error)
// ---------------------------------------------------------------------------

#[test]
fn fr003_cross_platform_workflow_exists() {
    assert!(
        Path::new(&workflow_path()).exists(),
        "cross-platform.yml must exist at .github/workflows/"
    );
}

// ---------------------------------------------------------------------------
// Gate 2: workflow name is "Cross-Platform"
// ---------------------------------------------------------------------------

#[test]
fn fr003_cross_platform_workflow_name() {
    let content = read_workflow();
    assert!(content.contains("name: Cross-Platform"), "Workflow must be named 'Cross-Platform'");
}

// ---------------------------------------------------------------------------
// Gate 3: native OS matrix includes ubuntu, macos, windows
// ---------------------------------------------------------------------------

#[test]
fn fr003_native_matrix_covers_three_os() {
    let content = read_workflow();
    assert!(content.contains("ubuntu-latest"), "Matrix must include ubuntu-latest");
    assert!(content.contains("macos-latest"), "Matrix must include macos-latest");
    assert!(content.contains("windows-latest"), "Matrix must include windows-latest");
}

// ---------------------------------------------------------------------------
// Gate 4: cross job matrix includes musl, freebsd, wasm32
// ---------------------------------------------------------------------------

#[test]
fn fr003_cross_matrix_includes_extended_targets() {
    let content = read_workflow();
    assert!(content.contains("x86_64-unknown-linux-musl"), "Cross matrix must include musl target");
    assert!(content.contains("x86_64-unknown-freebsd"), "Cross matrix must include FreeBSD target");
    assert!(content.contains("wasm32-unknown-unknown"), "Cross matrix must include wasm32 target");
}

// ---------------------------------------------------------------------------
// Gate 5: musl test job exists with lib tests
// ---------------------------------------------------------------------------

#[test]
fn fr003_musl_test_job_exists() {
    let content = read_workflow();
    assert!(content.contains("musl-test"), "musl-test job must exist");
    assert!(
        content.contains("--target x86_64-unknown-linux-musl"),
        "musl-test must target x86_64-unknown-linux-musl"
    );
    assert!(content.contains("--lib"), "musl-test must run --lib tests");
}

// ---------------------------------------------------------------------------
// Gate 6: cross-platform must not mask failures with continue-on-error
// ---------------------------------------------------------------------------

/// True when this mapping declares `continue-on-error` in a form Actions treats as
/// truthy. Booleans and the string `"true"` (any case) both mask; `false` and
/// expressions are left to the workflow author.
fn masks_failures(node: &serde_yaml::Value) -> bool {
    match node.get("continue-on-error") {
        Some(serde_yaml::Value::Bool(flag)) => *flag,
        Some(serde_yaml::Value::String(text)) => text.trim().eq_ignore_ascii_case("true"),
        _ => false,
    }
}

#[test]
fn fr003_cross_platform_jobs_do_not_mask_failures() {
    // Policy: PR #1047 removed the failure-masking from this workflow so its results
    // stay honest. cross-platform is a signal workflow, but masking a failing job or
    // step hides regressions and would silently weaken the signal if any job were
    // later promoted into a required check. Asserted against parsed YAML rather than
    // raw text so a comment mentioning the construct cannot fail the gate on its own.
    let content = read_workflow();
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&content).expect("cross-platform.yml must be valid YAML");
    let jobs = doc
        .get("jobs")
        .and_then(serde_yaml::Value::as_mapping)
        .expect("cross-platform.yml must declare jobs");
    for (name, job) in jobs {
        assert!(
            !masks_failures(job),
            "job {name:?} must not mask failures; failures must stay visible"
        );
        // Steps can override the job: a single masked step neutralizes the gate just
        // as effectively as a masked job.
        if let Some(steps) = job.get("steps").and_then(serde_yaml::Value::as_sequence) {
            for (index, step) in steps.iter().enumerate() {
                assert!(
                    !masks_failures(step),
                    "step {} of job {name:?} must not mask failures",
                    index + 1
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Gate 7: workflow triggers on push to main and pull_request
// ---------------------------------------------------------------------------

#[test]
fn fr003_cross_platform_workflow_triggers() {
    let content = read_workflow();
    assert!(
        content.contains("push:") && content.contains("branches: [main]"),
        "Workflow must trigger on push to main"
    );
    assert!(content.contains("pull_request:"), "Workflow must trigger on pull_request");
}

// ---------------------------------------------------------------------------
// Gate 8: checkout action is pinned to SHA
// ---------------------------------------------------------------------------

#[test]
fn fr003_cross_platform_uses_pinned_checkout() {
    let content = read_workflow();
    assert!(
        content.contains("actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"),
        "All checkout actions must be pinned to SHA (not tag)"
    );
}

// ---------------------------------------------------------------------------
// Gate 9: concurrency group prevents duplicate runs
// ---------------------------------------------------------------------------

#[test]
fn fr003_cross_platform_has_concurrency_control() {
    let content = read_workflow();
    assert!(content.contains("concurrency:"), "Workflow must define concurrency group");
    assert!(content.contains("cancel-in-progress: true"), "Workflow must cancel in-progress runs");
}
