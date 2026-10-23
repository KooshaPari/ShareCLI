// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C01 L12 — FR ↔ acceptance-test SSOT (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const FR_ACCEPTANCE_FILES: &[(&str, &[&str])] = &[
    (
        "FR-001",
        &[
            "tests/fr001_process_lifecycle.rs",
            "tests/fr001_stop_filter.rs",
            "tests/integration_cli.rs",
        ],
    ),
    ("FR-002", &["tests/fr002_config_init.rs", "tests/fr002_config_load.rs"]),
    ("FR-003", &["tests/fr003_project_registry.rs", "tests/fr003_project_discover.rs"]),
    ("FR-004", &["tests/fr004_status_health.rs", "tests/fr004_pool_status.rs"]),
    ("FR-005", &["tests/fr005_project_limits.rs", "tests/fr005_resource_check.rs"]),
];

/// FR-003 / C01 L12 — root FR index cites on-disk acceptance test paths.
#[test]
fn fr003_functional_requirements_acceptance_paths_exist() {
    let root = repo_root();
    let fr_doc = fs::read_to_string(root.join("FUNCTIONAL_REQUIREMENTS.md"))
        .expect("read FUNCTIONAL_REQUIREMENTS.md");

    for (fr_id, paths) in FR_ACCEPTANCE_FILES {
        assert!(fr_doc.contains(fr_id), "FUNCTIONAL_REQUIREMENTS.md must document {fr_id}");
        for path in *paths {
            assert!(
                fr_doc.contains(path),
                "FUNCTIONAL_REQUIREMENTS.md must cite acceptance path {path} for {fr_id}"
            );
            assert!(root.join(path).is_file(), "acceptance file must exist on disk: {path}");
        }
    }
}

/// FR-003 / C01 L12 — TRACEABILITY matrix lists ACCEPTED status with zero gaps.
#[test]
fn fr003_traceability_index_has_no_gaps() {
    let trace = fs::read_to_string(repo_root().join("docs/specs/TRACEABILITY.md"))
        .expect("read TRACEABILITY.md");

    assert!(trace.contains("0 gaps"), "TRACEABILITY.md must report zero acceptance gaps");
    for (fr_id, paths) in FR_ACCEPTANCE_FILES {
        assert!(trace.contains(fr_id), "TRACEABILITY.md must list {fr_id}");
        for path in *paths {
            if *path == "tests/integration_cli.rs" {
                continue;
            }
            assert!(trace.contains(path), "TRACEABILITY.md must cite {path} for {fr_id}");
        }
    }
}

/// FR-003 / C01 L12 — TEST_COVERAGE_MATRIX maps FR-001..005 to Covered rows.
#[test]
fn fr003_coverage_matrix_fr_rows_covered() {
    let matrix = fs::read_to_string(repo_root().join("TEST_COVERAGE_MATRIX.md"))
        .expect("read TEST_COVERAGE_MATRIX.md");

    assert!(!matrix.contains("| TBD |"), "TEST_COVERAGE_MATRIX must not contain TBD FR rows");
    for (fr_id, paths) in FR_ACCEPTANCE_FILES {
        let row_marker = format!("| {fr_id} |");
        assert!(matrix.contains(&row_marker), "TEST_COVERAGE_MATRIX must include row for {fr_id}");
        assert!(matrix.contains("**Covered**"), "TEST_COVERAGE_MATRIX must mark FR rows Covered");
        for path in *paths {
            if *path == "tests/integration_cli.rs" {
                continue;
            }
            assert!(matrix.contains(path), "TEST_COVERAGE_MATRIX must cite {path} for {fr_id}");
        }
    }
}

/// FR-003 / C01 L12 — acceptance tests carry FR annotations for grepability.
#[test]
fn fr003_acceptance_tests_declare_fr_tags() {
    let root = repo_root();
    for (_fr_id, paths) in FR_ACCEPTANCE_FILES {
        for path in *paths {
            if *path == "tests/integration_cli.rs" {
                continue;
            }
            let body =
                fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"));
            assert!(
                body.contains("FR:") || body.contains("FR-"),
                "{path} must declare FR traceability tag"
            );
        }
    }
}

/// FR-003 / C01 L12 — governance SSOT cross-links are present.
#[test]
fn fr003_governance_ssot_cross_links() {
    let root = repo_root();
    for rel in ["docs/specs/TRACEABILITY.md", "docs/specs/FR.md", "TEST_COVERAGE_MATRIX.md"] {
        assert!(root.join(rel).is_file(), "SSOT artifact missing: {rel}");
    }

    let fr_root = fs::read_to_string(root.join("FUNCTIONAL_REQUIREMENTS.md")).unwrap();
    assert!(fr_root.contains("docs/specs/TRACEABILITY.md"));
    assert!(fr_root.contains("docs/specs/FR.md"));

    let matrix = fs::read_to_string(root.join("TEST_COVERAGE_MATRIX.md")).unwrap();
    assert!(matrix.contains("docs/specs/TRACEABILITY.md"));
    assert!(matrix.contains("FUNCTIONAL_REQUIREMENTS.md"));
}
