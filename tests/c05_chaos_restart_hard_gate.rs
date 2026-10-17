// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 L50 — chaos restart hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C05 L50 — `ci.yml` must aggregate chaos restart into `ci-success`.
#[test]
fn fr003_ci_yml_chaos_restart_hard_gate_wired() {
    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("read ci.yml");

    assert!(
        ci.contains("name: chaos restart (required)"),
        "ci.yml must define required chaos-restart-hard job"
    );
    assert!(
        ci.contains("bash scripts/load/chaos_restart.sh"),
        "ci.yml chaos job must run chaos_restart.sh"
    );

    let needs_line = ci
        .lines()
        .find(|l| l.contains("needs:") && l.contains("ci-success"))
        .or_else(|| {
            ci.lines().skip_while(|l| !l.contains("ci-success:")).find(|l| l.contains("needs:"))
        })
        .expect("ci-success needs block");

    assert!(
        needs_line.contains("chaos-restart-hard"),
        "ci-success must need chaos-restart-hard job; got: {needs_line}"
    );

    let chaos_section = ci
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
