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
// Gate 6: no job is soft-failed (the matrix is a hard gate)
// ---------------------------------------------------------------------------
//
// PR #1047 ("remove failure-masking constructs") turned the native, cross and
// musl-test jobs into hard gates. Asserted against parsed YAML rather than raw
// text, because raw-text matching both false-fails on comment prose and
// false-passes the expression form (`continue-on-error: ${{ matrix.experimental }}`).
// The evaluator below reads the ACTUAL value of `continue-on-error`; any value it
// cannot prove to be a hard gate is a failure, never a pass.

use serde_yaml::Value;

/// The value `continue-on-error` actually carries, as Actions would parse it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContinueOnError {
    /// Key is absent entirely: hard gate.
    Absent,
    /// Literal `false` (YAML bool, or the string `"false"`): hard gate.
    LiteralFalse,
    /// Literal `true` (YAML bool, or the string `"true"`): soft-failed.
    LiteralTrue,
    /// An Actions expression, e.g. `${{ matrix.experimental }}`.
    Expression(String),
    /// Anything else (null, number, unexpected string). Never assumed safe.
    Other(String),
}

impl ContinueOnError {
    fn of(node: &Value) -> Self {
        match node.get("continue-on-error") {
            None => Self::Absent,
            Some(Value::Bool(false)) => Self::LiteralFalse,
            Some(Value::Bool(true)) => Self::LiteralTrue,
            Some(Value::String(text)) => {
                let trimmed = text.trim();
                if trimmed.contains("${{") {
                    Self::Expression(trimmed.to_string())
                } else if trimmed.eq_ignore_ascii_case("false") {
                    Self::LiteralFalse
                } else if trimmed.eq_ignore_ascii_case("true") {
                    Self::LiteralTrue
                } else {
                    Self::Other(trimmed.to_string())
                }
            }
            Some(other) => Self::Other(format!("{other:?}")),
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Absent => "absent".to_string(),
            Self::LiteralFalse => "false".to_string(),
            Self::LiteralTrue => "true".to_string(),
            Self::Expression(text) => format!("expression `{text}`"),
            Self::Other(text) => format!("unescaped value `{text}`"),
        }
    }
}

/// One row of the evaluation table: where the key lives, what it evaluates to,
/// and the verdict. `None` means hard gate (OK); `Some(reason)` means violation.
struct EvaluationRow {
    job: String,
    step: String,
    value: ContinueOnError,
    violation: Option<String>,
}

impl EvaluationRow {
    fn verdict(&self) -> &'static str {
        if self.violation.is_none() {
            "OK"
        } else {
            "FAIL"
        }
    }
}

/// Walk `jobs -> steps` and evaluate every `continue-on-error` that is present.
fn evaluate_workflow(content: &str) -> Result<Vec<EvaluationRow>, String> {
    let doc: Value =
        serde_yaml::from_str(content).map_err(|error| format!("invalid YAML: {error}"))?;
    let jobs = doc
        .get("jobs")
        .and_then(Value::as_mapping)
        .ok_or_else(|| "workflow must declare jobs".to_string())?;

    let mut rows = Vec::new();
    for (name, job) in jobs {
        let job_name = name.as_str().unwrap_or("<non-string job>").to_string();
        push_row(&mut rows, &job_name, "<job>", job);
        if let Some(steps) = job.get("steps").and_then(Value::as_sequence) {
            for (index, step) in steps.iter().enumerate() {
                let label = step
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|name| format!("step {} ({name})", index + 1))
                    .unwrap_or_else(|| format!("step {}", index + 1));
                push_row(&mut rows, &job_name, &label, step);
            }
        }
    }
    Ok(rows)
}

fn push_row(rows: &mut Vec<EvaluationRow>, job: &str, step: &str, node: &Value) {
    let value = ContinueOnError::of(node);
    let violation = match &value {
        ContinueOnError::Absent | ContinueOnError::LiteralFalse => None,
        ContinueOnError::LiteralTrue => {
            Some(format!("{job} / {step} sets `continue-on-error: true`"))
        }
        ContinueOnError::Expression(text) => Some(format!(
            "{job} / {step} sets `continue-on-error: {text}`; an unevaluated expression is unproven"
        )),
        ContinueOnError::Other(text) => Some(format!(
            "{job} / {step} sets `continue-on-error: {text}`; only a literal `false` proves a hard gate"
        )),
    };
    rows.push(EvaluationRow { job: job.to_string(), step: step.to_string(), value, violation });
}

fn violations(rows: &[EvaluationRow]) -> Vec<String> {
    rows.iter().filter_map(|row| row.violation.clone()).collect()
}

#[test]
fn fr003_cross_platform_jobs_are_hard_gates() {
    let content = read_workflow();
    let rows =
        evaluate_workflow(&content).expect("cross-platform.yml must be valid YAML with jobs");
    let problems = violations(&rows);
    assert!(
        problems.is_empty(),
        "cross-platform.yml must not soft-fail any job or step:\n{}",
        problems.join("\n")
    );
}

/// Prints the per-key evaluation table. Run with `--nocapture` to capture it.
#[test]
fn fr003_continue_on_error_evaluation_table() {
    let content = read_workflow();
    let rows = evaluate_workflow(&content).expect("cross-platform.yml must parse");
    println!("job | step | continue-on-error | verdict");
    println!("---|---|---|---");
    for row in rows.iter().filter(|row| !matches!(row.value, ContinueOnError::Absent)) {
        println!("{} | {} | {} | {}", row.job, row.step, row.value.describe(), row.verdict());
    }
    println!("(keys absent are hard gates by default; {} keys inspected)", rows.len());
}

// ---------------------------------------------------------------------------
// Gate 6b: negative control — the evaluator must be able to FAIL
// ---------------------------------------------------------------------------
//
// A gate that has never been observed to fail is NOT_RUN, not PASS. This test
// writes a temporary fixture containing both false-pass modes the old raw-text
// matching missed, proves both are rejected, then removes the fixture.

#[test]
fn fr003_gate_evaluator_rejects_soft_failures() {
    let dir = tempfile::tempdir().expect("tempdir for negative control");
    let fixture_path = dir.path().join("soft-fail.yml");

    let fixture = r#"
name: Negative Control
on: [push]
jobs:
  literal:
    runs-on: ubuntu-latest
    steps:
      - name: literal soft-fail
        continue-on-error: true
        run: exit 1
  expression:
    runs-on: ubuntu-latest
    steps:
      - name: expression soft-fail
        continue-on-error: ${{ matrix.experimental }}
        run: exit 1
"#;
    std::fs::write(&fixture_path, fixture).expect("write negative-control fixture");

    let content = std::fs::read_to_string(&fixture_path).expect("read fixture");
    let rows = evaluate_workflow(&content).expect("fixture must parse");
    let problems = violations(&rows);

    println!("negative control fixture: {} violations", problems.len());
    for problem in &problems {
        println!("REJECTED: {problem}");
    }

    assert_eq!(
        problems.len(),
        2,
        "negative control must reject BOTH the literal `true` and the expression form; got {problems:?}"
    );
    assert!(
        problems.iter().any(|problem| problem.contains("continue-on-error: true")),
        "literal `true` must be rejected: {problems:?}"
    );
    assert!(
        problems.iter().any(|problem| problem.contains("${{ matrix.experimental }}")),
        "expression form must be rejected: {problems:?}"
    );

    // And the real workflows must still pass, so the control proves the reject
    // is caused by the fixture, not by the evaluator rejecting everything.
    let real = evaluate_workflow(&read_workflow()).expect("real workflow must parse");
    assert!(
        violations(&real).is_empty(),
        "negative control inverted: real workflow must PASS: {:?}",
        violations(&real)
    );

    std::fs::remove_file(&fixture_path).expect("remove negative-control fixture");
    assert!(!fixture_path.exists(), "negative-control fixture must not survive the test");
}

// ---------------------------------------------------------------------------
// Gate 6c: required-check wiring
// ---------------------------------------------------------------------------
//
// A hard gate is worthless if its job is not a required check. This repo ships no
// machine-readable required-check/ruleset declaration under .github, so the
// finding is recorded by asserting the exact check-run names a human must confirm
// in branch protection. If a machine-readable file is later added, this test names
// it too, so the list can be checked against it.

const REQUIRED_CHECK_NAMES: &[&str] = &[
    "ubuntu-latest",
    "macos-latest",
    "windows-latest",
    "cross / x86_64-unknown-linux-musl",
    "cross / x86_64-unknown-freebsd",
    "musl-test (x86_64-unknown-linux-musl)",
];

#[test]
fn fr003_cross_platform_check_names_are_declared() {
    let content = read_workflow();
    let doc: Value = serde_yaml::from_str(&content).expect("valid YAML");
    let jobs = doc.get("jobs").and_then(Value::as_mapping).expect("jobs");

    // Reconstruct each job's declared `name:` with matrix placeholders expanded,
    // so a renamed job fails this test instead of silently escaping branch
    // protection (GitHub matches required checks on the check-run name).
    let mut declared: Vec<String> = Vec::new();
    for (key, job) in jobs {
        let template = job
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| key.as_str().unwrap_or("<non-string job>").to_string());
        let matrix_values = job
            .get("strategy")
            .and_then(|strategy| strategy.get("matrix"))
            .map(matrix_axis_values)
            .unwrap_or_default();
        if template.contains("${{") && !matrix_values.is_empty() {
            for value in matrix_values {
                declared.push(expand_matrix(&template, &value));
            }
        } else {
            declared.push(template);
        }
    }

    for expected in REQUIRED_CHECK_NAMES {
        assert!(
            declared.iter().any(|name| name == expected),
            "required check `{expected}` is no longer declared; update branch protection and this list. declared: {declared:?}"
        );
    }

    // Finding, not an assertion: no machine-readable required-check list exists.
    let candidates = [
        ".github/rulesets",
        ".github/required-checks.yml",
        ".github/required-checks.yaml",
        ".github/branch-protection.yml",
        ".github/branch-protection.yaml",
    ];
    let mut found = Vec::new();
    for candidate in candidates {
        let path = format!("{}/{candidate}", repo_root());
        if Path::new(&path).exists() {
            found.push(candidate);
        }
    }
    println!(
        "required-check declaration: {}; checks a human must mark required in branch protection: {REQUIRED_CHECK_NAMES:?}",
        if found.is_empty() {
            "ABSENT (no machine-readable ruleset or required-checks file under .github)".to_string()
        } else {
            format!("found {found:?}")
        }
    );
}

/// Collect the `os` / `target` values a matrix produces. Supports both the
/// `os: [a, b]` / `target: [a, b]` axes and the `include: [{target: a}]` form
/// this workflow's `cross` job uses.
fn matrix_axis_values(matrix: &Value) -> Vec<String> {
    let mut values: Vec<String> = Vec::new();
    for axis in ["os", "target"] {
        if let Some(entries) = matrix.get(axis).and_then(Value::as_sequence) {
            values.extend(entries.iter().filter_map(Value::as_str).map(str::to_string));
        }
    }
    if let Some(includes) = matrix.get("include").and_then(Value::as_sequence) {
        for entry in includes {
            for axis in ["os", "target"] {
                if let Some(value) = entry.get(axis).and_then(Value::as_str) {
                    values.push(value.to_string());
                }
            }
        }
    }
    values
}

/// Replace `${{ matrix.os }}` / `${{ matrix.target }}` with a concrete value.
fn expand_matrix(template: &str, value: &str) -> String {
    template
        .replace("${{ matrix.os }}", value)
        .replace("${{ matrix.target }}", value)
        .replace("${{matrix.os}}", value)
        .replace("${{matrix.target}}", value)
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
