// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C06 L54 — network-block hermetic hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C06 L54 — the netblock hermetic gate must be hard and must run on
/// PRs. The `netblock` job used to live in the deleted
/// `.github/workflows/ci.yml` (feeding the `ci-success` aggregator); it now
/// lives in `ci-gate.yml` as the `ci-netblock` job.
#[test]
fn fr003_ci_gate_netblock_hard_gate_wired() {
    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci-gate.yml"))
        .expect("read ci-gate.yml");

    assert!(
        ci.contains("name: netblock hermetic (required)"),
        "ci-gate.yml must define the hard netblock job"
    );
    assert!(
        ci.contains("bash scripts/ci/netblock_check.sh"),
        "ci-gate.yml netblock job must run netblock_check.sh"
    );

    // PR-time coverage must be restored: the deleted ci.yml `netblock` job fed
    // the `ci-success` aggregator on every PR.
    assert!(
        ci.contains("pull_request:"),
        "ci-gate.yml must trigger on pull_request so the deleted ci.yml netblock coverage is not lost"
    );

    let netblock_section = ci.split("ci-netblock:").nth(1).expect("netblock job block");

    // The job block comments explain that `netblock-soft.yml` runs the same
    // script under `continue-on-error: true`; only real YAML keys count here.
    let netblock_keys = netblock_section
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !netblock_keys.contains("continue-on-error: true"),
        "netblock job must not use continue-on-error"
    );
}

/// FR-003 / C06 L54 — hermetic contract documents hard CI enforcement.
#[test]
fn fr003_hermetic_builds_doc_hard_gate() {
    let doc = fs::read_to_string(repo_root().join("docs/ops/hermetic-builds.md"))
        .expect("read hermetic-builds.md");

    assert!(
        doc.contains("ci-gate.yml"),
        "hermetic-builds.md must document the ci-gate.yml hard gate"
    );
    assert!(doc.contains("ci-netblock"), "hermetic-builds.md must reference the ci-netblock job");
    assert!(doc.contains("netblock"), "hermetic-builds.md must reference netblock enforcement");
}
