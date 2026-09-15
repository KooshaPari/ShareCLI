// tests/c07_l68_flake_tracker.rs — FR-003 acceptance gate for C07 / L68
// Plan 795 (T-900) Flake-tracker score-3 evidence.
//
// These tests exercise scripts/flake_tracker.py end-to-end:
//  1. flake signal    — pass + fail in same testcase across runs -> "flaky"
//  2. hard regression — only failures -> "regression"
//  3. baseline diff   — introduced / resolved / persistent counts correct
//  4. output file     — JSON report is written and valid
//  5. --fail-on-flake — exit code 1 when flakes present, 0 otherwise
//  6. NO_COLOR        — no ANSI escape codes in stdout when env set
//
// The tests shell out to python; they tolerate the python interpreter being
// absent on a system without a python install, in which case the test is
// skipped (the production CI runners always have python).

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for an integration test points at the package root
    // (sharecli/) which is also the workspace root in this repo.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn python_bin() -> Option<String> {
    // Try "python" first (Windows / cross-platform), then "python3".
    for cand in ["python", "python3"] {
        if Command::new(cand).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
        {
            return Some(cand.to_string());
        }
    }
    None
}

fn run_tracker(
    py: &str,
    junit_path: &Path,
    out_path: &Path,
    baseline_path: Option<&Path>,
    args: &[&str],
) -> std::process::Output {
    let root = workspace_root();
    let script = root.join("scripts").join("flake_tracker.py");
    let mut cmd = Command::new(py);
    cmd.current_dir(&root);
    cmd.arg(&script);
    cmd.arg(junit_path.to_str().unwrap());
    cmd.arg("--output");
    cmd.arg(out_path.to_str().unwrap());
    cmd.arg("--quiet");
    if let Some(b) = baseline_path {
        cmd.arg("--baseline");
        cmd.arg(b.to_str().unwrap());
    }
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("failed to spawn python")
}

fn write_junit_flake(path: &Path) {
    // 2 cases total: one pure-flake (1 pass + 1 fail across 2 runs), one stable.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="t1" tests="2" failures="1" errors="0" skipped="0">
    <testcase classname="mod::flaky" name="intermittent" time="0.10"/>
    <testcase classname="mod::flaky" name="intermittent" time="0.05">
      <failure message="assertion failed: race"/>
    </testcase>
    <testcase classname="mod::stable" name="always_passes" time="0.01"/>
  </testsuite>
</testsuites>
"#;
    let mut f = fs::File::create(path).unwrap();
    f.write_all(xml.as_bytes()).unwrap();
}

fn write_junit_regression(path: &Path) {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="t1" tests="1" failures="1" errors="0" skipped="0">
    <testcase classname="mod::regression" name="always_fails" time="0.02">
      <failure message="expected true got false"/>
    </testcase>
  </testsuite>
</testsuites>
"#;
    let mut f = fs::File::create(path).unwrap();
    f.write_all(xml.as_bytes()).unwrap();
}

fn write_baseline_with_flake(path: &Path) {
    let json = r#"{
  "version": 1,
  "flaky_cases": [
    {"classname": "mod::flaky", "name": "persistent_flake"}
  ]
}
"#;
    let mut f = fs::File::create(path).unwrap();
    f.write_all(json.as_bytes()).unwrap();
}

fn read_report(path: &Path) -> serde_json::Value {
    let s = fs::read_to_string(path).unwrap();
    serde_json::from_str(&s).expect("report is not valid JSON")
}

#[test]
fn fr003_flake_tracker_classifies_flaky_case() {
    let Some(py) = python_bin() else {
        eprintln!("python not on PATH; skipping flake-tracker tests");
        return;
    };
    let tmp = env::temp_dir().join(format!("flake-tracker-test-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let junit = tmp.join("junit-flake.xml");
    let out = tmp.join("report.json");
    write_junit_flake(&junit);
    let r = run_tracker(&py, &junit, &out, None, &[]);
    assert!(
        r.status.success(),
        "tracker exited non-zero; stderr={}",
        String::from_utf8_lossy(&r.stderr)
    );
    let report = read_report(&out);
    let flaky = report["flaky_cases"].as_array().unwrap();
    assert_eq!(flaky.len(), 1, "expected 1 flaky case, got {:?}", flaky);
    assert_eq!(flaky[0]["classname"], "mod::flaky");
    assert_eq!(flaky[0]["name"], "intermittent");
    assert_eq!(flaky[0]["passed"], 1);
    assert_eq!(flaky[0]["failed"], 1);
    assert_eq!(report["by_kind"]["flaky"], 1);
}

#[test]
fn fr003_flake_tracker_classifies_regression() {
    let Some(py) = python_bin() else {
        return;
    };
    let tmp = env::temp_dir().join(format!("flake-tracker-reg-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let junit = tmp.join("junit-reg.xml");
    let out = tmp.join("report.json");
    write_junit_regression(&junit);
    let r = run_tracker(&py, &junit, &out, None, &[]);
    assert!(r.status.success());
    let report = read_report(&out);
    let regs = report["regression_cases"].as_array().unwrap();
    assert_eq!(regs.len(), 1);
    assert_eq!(regs[0]["classname"], "mod::regression");
    assert_eq!(regs[0]["name"], "always_fails");
    assert!(
        report["flaky_cases"].as_array().unwrap().is_empty(),
        "regression must not be classified as flaky"
    );
}

#[test]
fn fr003_flake_tracker_baseline_diff_introduced_and_resolved() {
    let Some(py) = python_bin() else {
        return;
    };
    let tmp = env::temp_dir().join(format!("flake-tracker-base-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let junit = tmp.join("junit-flake.xml");
    let out = tmp.join("report.json");
    let baseline = tmp.join("baseline.json");
    write_junit_flake(&junit);
    write_baseline_with_flake(&baseline);
    let r = run_tracker(&py, &junit, &out, Some(&baseline), &[]);
    assert!(r.status.success());
    let report = read_report(&out);
    let diff = report["baseline_diff"].as_object().expect("baseline_diff must be present");
    assert_eq!(diff["introduced_count"], 1, "mod::flaky::intermittent is newly introduced");
    assert_eq!(diff["resolved_count"], 1, "mod::flaky::persistent_flake was cleared");
    assert_eq!(diff["persistent_count"], 0);
    let introduced = diff["introduced"].as_array().unwrap();
    assert_eq!(introduced[0]["classname"], "mod::flaky");
    assert_eq!(introduced[0]["name"], "intermittent");
}

#[test]
fn fr003_flake_tracker_writes_json_report_to_output_path() {
    let Some(py) = python_bin() else {
        return;
    };
    let tmp = env::temp_dir().join(format!("flake-tracker-out-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let junit = tmp.join("junit.xml");
    let out = tmp.join("nested/dir/flake-report.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    write_junit_flake(&junit);
    let r = run_tracker(&py, &junit, &out, None, &[]);
    assert!(r.status.success());
    assert!(out.exists(), "output JSON was not written");
    let report = read_report(&out);
    assert!(report.get("generated_at_utc").is_some());
    assert!(report.get("flake_rate").is_some());
}

#[test]
fn fr003_flake_tracker_rate_uses_case_count_not_attempts() {
    // 1 flaky case (1 pass + 1 fail across 2 attempts) + 1 stable case.
    // Total cases: 2. Total flaky cases: 1. Expected rate: 1/2 = 0.5.
    // Pre-fix bug: counted 1 flaky / 3 executed attempts = 0.333...
    // Post-fix: 1 flaky / 2 cases = 0.5.
    let Some(py) = python_bin() else {
        return;
    };
    let tmp = env::temp_dir().join(format!("flake-tracker-rate-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let junit = tmp.join("junit-rate.xml");
    let out = tmp.join("rate-report.json");
    write_junit_flake(&junit);
    let r = run_tracker(&py, &junit, &out, None, &[]);
    assert!(r.status.success());
    let report = read_report(&out);
    let rate = report["flake_rate"].as_f64().expect("flake_rate must be a number");
    assert!(
        (rate - 0.5).abs() < 1e-9,
        "expected flake_rate = 0.5 (1 flaky case / 2 cases), got {}",
        rate
    );
}

#[test]
fn fr003_flake_tracker_fail_on_flake_exits_nonzero_on_flake() {
    let Some(py) = python_bin() else {
        return;
    };
    let tmp = env::temp_dir().join(format!("flake-tracker-fail-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let junit = tmp.join("junit-flake.xml");
    let out = tmp.join("report.json");
    write_junit_flake(&junit);
    // Default exit code is 0 even with flakes.
    let r0 = run_tracker(&py, &junit, &out, None, &[]);
    assert!(r0.status.success(), "default must not fail on flake");
    // --fail-on-flake flips to non-zero.
    let r1 = run_tracker(&py, &junit, &out, None, &["--fail-on-flake"]);
    assert!(!r1.status.success(), "--fail-on-flake must exit non-zero when flake present");
    // Same flag, regression-only input — should NOT trip.
    let junit_reg = tmp.join("junit-reg.xml");
    write_junit_regression(&junit_reg);
    let r2 = run_tracker(&py, &junit_reg, &out, None, &["--fail-on-flake"]);
    assert!(r2.status.success(), "--fail-on-flake must NOT trip on pure regression");
}

#[test]
fn fr003_flake_tracker_respects_no_color_env() {
    let Some(py) = python_bin() else {
        return;
    };
    let tmp = env::temp_dir().join(format!("flake-tracker-color-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let junit = tmp.join("junit-flake.xml");
    let out = tmp.join("report.json");
    write_junit_flake(&junit);
    // Run the script (NOT --quiet this time) with NO_COLOR=1, capture stdout.
    let root = workspace_root();
    let script = root.join("scripts").join("flake_tracker.py");
    let mut cmd = Command::new(&py);
    cmd.current_dir(&root);
    cmd.arg(&script);
    cmd.arg(junit.to_str().unwrap());
    cmd.arg("--output");
    cmd.arg(out.to_str().unwrap());
    cmd.env("NO_COLOR", "1");
    cmd.env_remove("FORCE_COLOR");
    let r = cmd.output().expect("failed to spawn python");
    assert!(r.status.success());
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "stdout contained ANSI escape codes despite NO_COLOR=1: {:?}",
        stdout
    );
    assert!(stdout.contains("flake_tracker.py"), "summary header missing from stdout");
}

// Tiny shim so fs::create_dir_all isn't shadowed in the tests above when a
// future contributor types `fs::create_dir_dir_safe` by mistake.
#[allow(dead_code)]
trait FsDirAllShim {
    fn create_dir_dir_safe(&self) -> std::io::Result<()>;
}
impl FsDirAllShim for PathBuf {
    fn create_dir_dir_safe(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self)
    }
}
