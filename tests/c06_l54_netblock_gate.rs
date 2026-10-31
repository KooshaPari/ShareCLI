// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C06 L54 — network-block hermetic hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C06 L54 — `ci.yml` must aggregate netblock into `ci-success`.
#[test]
fn fr003_ci_yml_netblock_hard_gate_wired() {
    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("read ci.yml");

    assert!(
        ci.contains("name: netblock hermetic (required)"),
        "ci.yml must define required netblock job"
    );
    assert!(
        ci.contains("bash scripts/ci/netblock_check.sh"),
        "ci.yml netblock job must run netblock_check.sh"
    );

    let needs_line = ci
        .lines()
        .find(|l| l.contains("needs:") && l.contains("ci-success"))
        .or_else(|| {
            ci.lines().skip_while(|l| !l.contains("ci-success:")).find(|l| l.contains("needs:"))
        })
        .expect("ci-success needs block");

    assert!(
        needs_line.contains("netblock"),
        "ci-success must need netblock job; got: {needs_line}"
    );

    let netblock_section = ci
        .split("netblock:")
        .nth(1)
        .and_then(|s| s.split("\n  ci-success:").next())
        .expect("netblock job block");

    assert!(
        !netblock_section.contains("continue-on-error: true"),
        "netblock job must not use continue-on-error"
    );
}

/// FR-003 / C06 L54 — hermetic contract documents hard CI enforcement.
#[test]
fn fr003_hermetic_builds_doc_hard_gate() {
    let doc = fs::read_to_string(repo_root().join("docs/ops/hermetic-builds.md"))
        .expect("read hermetic-builds.md");

    assert!(doc.contains("ci-success"), "hermetic-builds.md must document ci-success hard gate");
    assert!(doc.contains("netblock"), "hermetic-builds.md must reference netblock enforcement");
}
