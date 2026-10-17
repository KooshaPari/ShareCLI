// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C01 L11 — measured `--lib` coverage pin (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C01 L11 — matrix cites a retained llvm-cov snapshot with a numeric line %.
#[test]
fn fr003_coverage_matrix_pins_numeric_line_percent() {
    let matrix = fs::read_to_string(repo_root().join("TEST_COVERAGE_MATRIX.md"))
        .expect("read TEST_COVERAGE_MATRIX.md");

    assert!(
        matrix.contains("77.34%"),
        "TEST_COVERAGE_MATRIX must pin measured --lib line coverage at 77.34%"
    );
    assert!(
        matrix.contains("audit/coverage-snapshots/fa887e9.coverage-snapshot.json"),
        "TEST_COVERAGE_MATRIX must cite retained lib-pin snapshot artifact path"
    );
}

/// FR-003 / C01 L11 — retained snapshot is machine-readable and matches the matrix pin.
#[test]
fn fr003_coverage_snapshot_artifact_matches_pin() {
    let path = repo_root().join("audit/coverage-snapshots/fa887e9.coverage-snapshot.json");
    let raw = fs::read_to_string(&path).expect("read coverage snapshot artifact");
    let snapshot: serde_json::Value =
        serde_json::from_str(&raw).expect("parse coverage snapshot JSON");

    let lines_percent = snapshot["coverage"]["lines"]["percent"].as_f64().expect("lines.percent");
    assert!(
        (lines_percent - 77.34).abs() < 0.01,
        "snapshot lines.percent must match matrix pin; got {lines_percent}"
    );

    let sha = snapshot["source"]["git_sha"].as_str().expect("source.git_sha");
    assert!(sha.starts_with("fa887e9"), "snapshot git_sha must match filename pin; got {sha}");
}

/// FR-003 / C01 L11 — coverage workflow retains compact llvm-cov snapshot artifact.
#[test]
fn fr003_coverage_yml_emits_snapshot_artifact() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/coverage.yml"))
        .expect("read coverage.yml");

    assert!(
        workflow.contains("scripts/coverage_snapshot.py"),
        "coverage.yml must run coverage_snapshot.py"
    );
    assert!(
        workflow.contains("coverage-snapshot-"),
        "coverage.yml must upload SHA-keyed coverage-snapshot artifact"
    );
    // Empty-suite false positive: workflow env sets CARGO_TERM_COLOR=always, but the
    // guard must force never so libtest ANSI does not break `grep ': test$'`.
    assert!(
        workflow.contains("CARGO_TERM_COLOR: never"),
        "coverage.yml guard/llvm-cov steps must force CARGO_TERM_COLOR=never"
    );
    assert!(
        !workflow.contains("2>/dev/null || true"),
        "coverage.yml must not swallow cargo test --list failures"
    );
    assert!(
        workflow.contains("tool: cargo-llvm-cov"),
        "coverage.yml must pass tool: cargo-llvm-cov to install-action (SHA pins drop tag tool names)"
    );
    assert!(
        workflow.contains("--ignore-run-fail"),
        "coverage.yml must use --ignore-run-fail so evidence snapshot is produced despite known test flakes"
    );
}

/// FR-003 / C01 L11 — Wave18 workspace-broad snapshot retained and matches matrix pin.
#[test]
fn fr003_coverage_workspace_broad_snapshot_matches_pin() {
    let path = repo_root().join("audit/coverage-snapshots/d152eda.coverage-snapshot.json");
    let raw = fs::read_to_string(&path).expect("read d152eda coverage snapshot artifact");
    let snapshot: serde_json::Value =
        serde_json::from_str(&raw).expect("parse d152eda coverage snapshot JSON");

    let lines_percent = snapshot["coverage"]["lines"]["percent"].as_f64().expect("lines.percent");
    assert!(
        (lines_percent - 82.38).abs() < 0.01,
        "d152eda snapshot lines.percent must be 82.38%; got {lines_percent}"
    );

    let sha = snapshot["source"]["git_sha"].as_str().expect("source.git_sha");
    assert!(sha.starts_with("d152eda"), "d152eda snapshot git_sha must match filename; got {sha}");
}

/// FR-003 / C01 L11 — prior workspace-broad pin snapshot retained as historical evidence.
#[test]
fn fr003_coverage_workspace_pin_retained_as_historical() {
    let path = repo_root().join("audit/coverage-snapshots/5d8dc08.coverage-snapshot.json");
    let raw = fs::read_to_string(&path).expect("read 5d8dc08 coverage snapshot artifact");
    let snapshot: serde_json::Value =
        serde_json::from_str(&raw).expect("parse 5d8dc08 coverage snapshot JSON");

    let lines_percent = snapshot["coverage"]["lines"]["percent"].as_f64().expect("lines.percent");
    assert!(
        (lines_percent - 80.51).abs() < 0.01,
        "5d8dc08 snapshot lines.percent must remain 80.51%; got {lines_percent}"
    );

    let sha = snapshot["source"]["git_sha"].as_str().expect("source.git_sha");
    assert!(sha.starts_with("5d8dc08"), "5d8dc08 snapshot git_sha must match filename; got {sha}");
}
