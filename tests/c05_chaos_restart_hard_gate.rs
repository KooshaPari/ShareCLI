// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 L50 — chaos restart hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Strip YAML comments so an assertion cannot be satisfied by a
/// documented-but-inactive line (for example a `pull_request:` key that is
/// commented out but whose text still appears in the file).
fn strip_yaml_comments(src: &str) -> String {
    src.lines()
        .map(|line| match line.find('#') {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// FR-003 / C05 L50 — the chaos restart gate must be hard and must run on PRs.
/// The `chaos-restart-hard` job used to live in the deleted
/// `.github/workflows/ci.yml` (feeding the `ci-success` aggregator); it now
/// lives in `chaos-restart-hard.yml`, which carries a `pull_request` trigger.
#[test]
fn fr003_chaos_restart_hard_yml_chaos_gate_wired() {
    let ci = fs::read_to_string(repo_root().join(".github/workflows/chaos-restart-hard.yml"))
        .expect("read chaos-restart-hard.yml");
    // Assert against the comment-stripped workflow: a `pull_request:` key
    // that has been commented out must NOT satisfy the trigger requirement.
    let active = strip_yaml_comments(&ci);

    assert!(
        active.contains("name: chaos restart (hard)"),
        "chaos-restart-hard.yml must define the hard chaos-restart job"
    );
    assert!(
        active.contains("bash scripts/load/chaos_restart.sh"),
        "chaos-restart-hard.yml chaos job must run chaos_restart.sh"
    );

    // PR-time coverage must be restored: the deleted ci.yml `chaos-restart-hard`
    // job fed the `ci-success` aggregator on every PR.
    let trigger_block = active
        .split("\non:")
        .nth(1)
        .and_then(|s| s.split("\njobs:").next())
        .expect("chaos-restart-hard.yml trigger block");
    assert!(
        trigger_block.contains("pull_request:"),
        "chaos-restart-hard.yml must trigger on pull_request so the deleted ci.yml chaos coverage is not lost"
    );

    let chaos_section = active
        .split("chaos-restart-hard:")
        .nth(1)
        .and_then(|s| s.split("\n  ci-success:").next())
        .expect("chaos-restart-hard job block");

    assert!(
        !chaos_section.contains("continue-on-error: true"),
        "chaos-restart-hard job must not use continue-on-error"
    );
}

/// FR-003 / C05 L50 — standalone workflow stays hard (no continue-on-error on chaos step).
#[test]
fn fr003_chaos_restart_hard_yml_no_soft_shim() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/chaos-restart-hard.yml"))
        .expect("read chaos-restart-hard.yml");

    assert!(
        workflow.contains("bash scripts/load/chaos_restart.sh"),
        "chaos-restart-hard.yml must run chaos_restart.sh"
    );

    let chaos_step =
        workflow.split("Chaos restart /healthz recovery").nth(1).expect("chaos restart step");

    assert!(
        !chaos_step.contains("continue-on-error: true"),
        "chaos restart step must not use continue-on-error"
    );
}
