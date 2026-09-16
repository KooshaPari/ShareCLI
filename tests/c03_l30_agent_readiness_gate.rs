//! C03 L30.1 / L30.3 / L30.9 — agent-readiness evidence gates (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const FR_ACCEPTANCE_FILES: &[(&str, &[&str])] = &[
    ("FR-001", &["tests/fr001_process_lifecycle.rs", "tests/fr001_stop_filter.rs"]),
    ("FR-002", &["tests/fr002_config_init.rs", "tests/fr002_config_load.rs"]),
    ("FR-003", &["tests/fr003_project_registry.rs", "tests/fr003_project_discover.rs"]),
    ("FR-004", &["tests/fr004_status_health.rs", "tests/fr004_pool_status.rs"]),
    ("FR-005", &["tests/fr005_project_limits.rs", "tests/fr005_resource_check.rs"]),
];

/// FR-003 / C03 L30.1 — FR-NNN grammar with on-disk acceptance paths.
#[test]
fn fr003_l301_fr_grammar_and_acceptance_paths() {
    let root = repo_root();
    let fr_doc = fs::read_to_string(root.join("FUNCTIONAL_REQUIREMENTS.md"))
        .expect("read FUNCTIONAL_REQUIREMENTS.md");
    let fr_detail =
        fs::read_to_string(root.join("docs/specs/FR.md")).expect("read docs/specs/FR.md");

    for (fr_id, paths) in FR_ACCEPTANCE_FILES {
        assert!(fr_doc.contains(fr_id), "FUNCTIONAL_REQUIREMENTS must list {fr_id}");
        assert!(fr_detail.contains(fr_id), "docs/specs/FR.md must list {fr_id}");
        for path in *paths {
            assert!(fr_doc.contains(path), "FUNCTIONAL_REQUIREMENTS must cite {path} for {fr_id}");
            assert!(root.join(path).is_file(), "acceptance file must exist on disk: {path}");
        }
    }
}

/// FR-003 / C03 L30.3 — FR guardrail suites covered + measured coverage pin.
#[test]
fn fr003_l303_fr_guardrail_coverage_and_pin() {
    let root = repo_root();
    let matrix = fs::read_to_string(root.join("TEST_COVERAGE_MATRIX.md"))
        .expect("read TEST_COVERAGE_MATRIX.md");
    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read ci.yml");
    let justfile = fs::read_to_string(root.join("justfile")).expect("read justfile");

    assert!(!matrix.contains("| TBD |"), "TEST_COVERAGE_MATRIX must not contain TBD FR rows");
    for (fr_id, paths) in FR_ACCEPTANCE_FILES {
        assert!(
            matrix.contains(&format!("| {fr_id} |")),
            "TEST_COVERAGE_MATRIX must include row for {fr_id}"
        );
        assert!(matrix.contains("**Covered**"), "TEST_COVERAGE_MATRIX must mark FR rows Covered");
        for path in *paths {
            assert!(matrix.contains(path), "TEST_COVERAGE_MATRIX must cite {path} for {fr_id}");
        }
    }

    let measured_start = matrix
        .find("## Measured coverage pin")
        .expect("TEST_COVERAGE_MATRIX must have Measured coverage pin section");
    let prior_start = matrix
        .find("### Prior pin (superseded)")
        .expect("TEST_COVERAGE_MATRIX must have Prior pin section");
    let measured_section = &matrix[measured_start..prior_start];
    assert!(
        measured_section.contains("77.34%"),
        "Measured pin section must contain lib pin 77.34%"
    );
    assert!(
        measured_section.contains("fa887e9"),
        "Measured pin section must pin current source revision fa887e9"
    );
    assert!(
        measured_section.contains("5d8dc08"),
        "Measured pin section must reference retained snapshot 5d8dc08"
    );
    assert!(
        measured_section.contains("80.51%"),
        "Measured pin section must reference prior workspace pin 80.51%"
    );
    assert!(
        root.join("audit/coverage-snapshots/5d8dc08.coverage-snapshot.json").is_file(),
        "retained llvm-cov snapshot artifact must exist"
    );
    assert!(ci.contains("cargo nextest run"), "ci.yml must run nextest guardrail suite");
    assert!(justfile.contains("test"), "justfile must expose test recipe");
}

/// FR-003 / C03 L30.9 — multi-agent claim-lock protocol for shared paths.
#[test]
fn fr003_l309_claim_lock_protocol_documented() {
    let agents = fs::read_to_string(repo_root().join("AGENTS.md")).expect("read AGENTS.md");

    assert!(agents.contains("Claim-lock protocol"), "AGENTS.md must document claim-lock protocol");
    assert!(agents.contains("L30.9"), "AGENTS.md must cite L30.9 pillar");
    assert!(agents.contains("WORK_DAG.md"), "AGENTS.md claim-lock must reference WORK_DAG.md");
    assert!(agents.contains("Shared path"), "AGENTS.md must define shared-path ownership table");
    assert!(
        agents.contains("Conflict rule"),
        "AGENTS.md must document conflict serialization rule"
    );
}
