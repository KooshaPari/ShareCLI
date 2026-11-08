// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 L46 — MWMB burn-rate alerts + error budget policy (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C05 L46 — formal error-budget policy document on disk.
#[test]
fn fr003_error_budget_policy_present() {
    let policy = repo_root().join("docs/ops/error-budget-policy.md");
    let body = fs::read_to_string(&policy).expect("read error-budget-policy.md");

    for needle in ["MWMB", "burn_window", "SharecliHttpErrorBudgetBurnFast", "Escalation"] {
        assert!(body.contains(needle), "policy must mention {needle}");
    }
}

/// FR-003 / C05 L46 — Prometheus rules ship MWMB fast/slow pairs per SLO.
#[test]
fn fr003_alert_rules_mwmb_pairs() {
    let rules = fs::read_to_string(repo_root().join("docs/ops/alertmanager/sharecli.yml"))
        .expect("read sharecli.yml");

    let fast_slow_pairs = [
        ("SharecliHttpErrorBudgetBurnFast", "SharecliHttpErrorBudgetBurn"),
        ("SharecliAuthFailBurnFast", "SharecliAuthFailBurn"),
        ("SharecliHealthzDown", "SharecliSlo1AvailabilityBurnSlow"),
    ];

    for (fast, slow) in fast_slow_pairs {
        assert!(rules.contains(fast), "missing fast burn alert {fast}");
        assert!(rules.contains(slow), "missing slow burn alert {slow}");
    }

    assert!(rules.matches("burn_window: fast").count() >= 3, "expected at least 3 fast-burn rules");
    assert!(rules.matches("burn_window: slow").count() >= 3, "expected at least 3 slow-burn rules");
}

/// FR-003 / C05 L46 — SLO.md links error-budget policy (SSOT cross-ref).
#[test]
fn fr003_slo_md_links_error_budget_policy() {
    let slo = fs::read_to_string(repo_root().join("docs/ops/SLO.md")).expect("read SLO.md");
    assert!(slo.contains("error-budget-policy.md"), "SLO.md must link error-budget-policy.md");
}
