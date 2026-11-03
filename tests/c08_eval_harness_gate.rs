// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C08 eval harness gate (T-1110, Wave19 gap remediation)
//! FR: FR-002, FR-004, FR-007, FR-012
//! Scope: eval.yaml existence, benchmark/target alignment, threshold reasonableness,
//!        and regression detection logic.
//!
//! This test suite gates the C08 eval harness configuration. It ensures:
//!   1. `eval.yaml` exists and is parseable
//!   2. Benchmark names in eval.yaml match actual `[[bench]]` targets in Cargo.toml
//!   3. Thresholds are sane (non-zero, below absurd ceilings)
//!   4. Regression detection formulas are correct

use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Known bench targets from Cargo.toml [[bench]] sections and benches/*.rs.
/// These must stay in sync with the actual bench harnesses.
const KNOWN_BENCH_TARGETS: &[&str] = &[
    "config_toml_from_str",
    "pool_new_and_list_empty",
    "prometheus_render_32",
    "jwt_validate_rs256",
];

/// Known bench config names from eval.yaml benchmarks[].name.
const KNOWN_BENCH_NAMES: &[&str] =
    &["config_parse", "pool_list", "prometheus_render", "jwt_auth_validate"];

/// Maximum acceptable threshold in milliseconds.
/// Any benchmark threshold above this is considered absurdly high.
const MAX_THRESHOLD_MS: u64 = 10_000; // 10 seconds

/// Minimum acceptable threshold in milliseconds.
/// Thresholds at or below zero are nonsensical.
const MIN_THRESHOLD_MS: u64 = 1;

/// Maximum acceptable max_regression_pct.
const MAX_REGRESSION_PCT: f64 = 50.0;

/// Minimum acceptable min_pass_rate.
const MIN_PASS_RATE: f64 = 50.0;

// ---------------------------------------------------------------------------
// Helpers — lightweight YAML parsing (no serde_yaml in integration tests)
// ---------------------------------------------------------------------------

/// Extract the value of a simple `key: value` line from YAML content.
/// Returns None if the key is not found.
fn yaml_get_simple<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&format!("{key}:")) {
            return Some(rest.trim());
        }
    }
    None
}

/// Strip YAML list prefix (`- `) from a trimmed line, if present.
fn strip_yaml_list_prefix(s: &str) -> &str {
    s.strip_prefix("- ").unwrap_or(s)
}

/// Extract all values following a YAML key pattern like `name: <value>` within
/// benchmark blocks (indented under `benchmarks:`). Returns the list of name values found.
fn extract_bench_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_benchmarks = false;
    for line in content.lines() {
        let trimmed = line.trim();
        // Track when we enter/exit the benchmarks: section based on indentation.
        // A non-empty, non-comment line at indent 0 resets the section tracker.
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let leading_spaces = line.len() - line.trim_start().len();
            if leading_spaces == 0 && trimmed.ends_with(':') {
                in_benchmarks = trimmed == "benchmarks:";
            } else if leading_spaces == 0 && !trimmed.ends_with(':') {
                in_benchmarks = false;
            }
        }
        if in_benchmarks {
            let key_part = strip_yaml_list_prefix(trimmed);
            if let Some(val) = key_part.strip_prefix("name:") {
                let val = val.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    names.push(val.to_string());
                }
            }
        }
    }
    names
}

/// Extract bench_target values from eval.yaml benchmark definitions.
/// Section-aware: only extracts from within the `benchmarks:` block.
fn extract_bench_targets(content: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut in_benchmarks = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let leading_spaces = line.len() - line.trim_start().len();
            if leading_spaces == 0 && trimmed.ends_with(':') {
                in_benchmarks = trimmed == "benchmarks:";
            } else if leading_spaces == 0 && !trimmed.ends_with(':') {
                in_benchmarks = false;
            }
        }
        if in_benchmarks {
            let key_part = strip_yaml_list_prefix(trimmed);
            if let Some(val) = key_part.strip_prefix("bench_target:") {
                let val = val.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    targets.push(val.to_string());
                }
            }
        }
    }
    targets
}

/// Extract threshold_ms values from eval.yaml benchmark definitions.
/// Section-aware: only extracts from within the `benchmarks:` block.
fn extract_threshold_ms(content: &str) -> Vec<u64> {
    let mut thresholds = Vec::new();
    let mut in_benchmarks = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let leading_spaces = line.len() - line.trim_start().len();
            if leading_spaces == 0 && trimmed.ends_with(':') {
                in_benchmarks = trimmed == "benchmarks:";
            } else if leading_spaces == 0 && !trimmed.ends_with(':') {
                in_benchmarks = false;
            }
        }
        if in_benchmarks {
            let key_part = strip_yaml_list_prefix(trimmed);
            if let Some(val) = key_part.strip_prefix("threshold_ms:") {
                if let Ok(n) = val.trim().parse::<u64>() {
                    thresholds.push(n);
                }
            }
        }
    }
    thresholds
}

/// Parse a float from a YAML simple key:value line.
fn yaml_get_f64(content: &str, key: &str) -> Option<f64> {
    yaml_get_simple(content, key)
        .and_then(|v| v.trim_matches('"').trim_matches('\'').parse::<f64>().ok())
}

/// Parse a u64 from a YAML simple key:value line.
fn yaml_get_u64(content: &str, key: &str) -> Option<u64> {
    yaml_get_simple(content, key)
        .and_then(|v| v.trim_matches('"').trim_matches('\'').parse::<u64>().ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Assert that `eval.yaml` exists and is non-empty.
#[test]
fn eval_yaml_exists() {
    let path = Path::new("eval.yaml");
    assert!(path.exists(), "eval.yaml must exist at repo root (T-1110)");
    let content = fs::read_to_string(path).expect("eval.yaml must be readable");
    assert!(!content.trim().is_empty(), "eval.yaml must not be empty");
}

/// Assert that eval.yaml contains all expected benchmark names.
#[test]
fn eval_yaml_has_all_benchmarks() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");
    let names = extract_bench_names(&content);

    for expected in KNOWN_BENCH_NAMES {
        assert!(
            names.iter().any(|n| n == expected),
            "eval.yaml must define benchmark '{expected}'; found: {names:?}"
        );
    }
}

/// Assert that each eval.yaml benchmark has a bench_target matching a real
/// Criterion bench in benches/*.rs.
#[test]
fn eval_yaml_targets_match_actual_bench() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");
    let targets = extract_bench_targets(&content);

    assert!(!targets.is_empty(), "eval.yaml must define bench_target for each benchmark");

    for target in &targets {
        assert!(
            KNOWN_BENCH_TARGETS.contains(&target.as_str()),
            "bench_target '{target}' does not match any known Criterion bench target; \
             known: {KNOWN_BENCH_TARGETS:?}"
        );
    }
}

/// Assert that every known bench target has a corresponding eval.yaml entry.
#[test]
fn eval_yaml_covers_all_bench_targets() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");
    let targets = extract_bench_targets(&content);

    for expected in KNOWN_BENCH_TARGETS {
        assert!(
            targets.iter().any(|t| t == *expected),
            "bench target '{expected}' from benches/*.rs has no eval.yaml entry"
        );
    }
}

/// Assert that threshold_ms values are sane: non-zero and below absurd ceiling.
#[test]
fn eval_yaml_thresholds_reasonable() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");
    let thresholds = extract_threshold_ms(&content);

    assert_eq!(
        thresholds.len(),
        KNOWN_BENCH_NAMES.len(),
        "eval.yaml must have threshold_ms for each benchmark"
    );

    for (i, &ms) in thresholds.iter().enumerate() {
        assert!(
            ms >= MIN_THRESHOLD_MS,
            "threshold_ms[{i}] = {ms} must be >= {MIN_THRESHOLD_MS} (benchmark: {})",
            KNOWN_BENCH_NAMES[i]
        );
        assert!(
            ms <= MAX_THRESHOLD_MS,
            "threshold_ms[{i}] = {ms} must be <= {MAX_THRESHOLD_MS}ms (benchmark: {})",
            KNOWN_BENCH_NAMES[i]
        );
    }
}

/// Assert that global regression thresholds are sane.
#[test]
fn eval_yaml_global_thresholds_reasonable() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");

    let max_regression = yaml_get_f64(&content, "max_regression_pct");
    assert!(max_regression.is_some(), "max_regression_pct must be defined");
    let reg = max_regression.unwrap();
    assert!(
        reg > 0.0 && reg <= MAX_REGRESSION_PCT,
        "max_regression_pct ({reg}%) must be in (0, {MAX_REGRESSION_PCT}]"
    );

    let min_pass = yaml_get_f64(&content, "min_pass_rate");
    assert!(min_pass.is_some(), "min_pass_rate must be defined");
    let pass = min_pass.unwrap();
    assert!(
        pass >= MIN_PASS_RATE && pass <= 100.0,
        "min_pass_rate ({pass}%) must be in [{MIN_PASS_RATE}, 100]"
    );
}

/// Assert that the CI sample parameters are defined and reasonable.
#[test]
fn eval_yaml_ci_params_reasonable() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");

    let sample_size = yaml_get_u64(&content, "ci_sample_size");
    assert!(sample_size.is_some(), "ci_sample_size must be defined");
    let ss = sample_size.unwrap();
    assert!(ss >= 3 && ss <= 1000, "ci_sample_size ({ss}) must be in [3, 1000]");

    let warmup = yaml_get_u64(&content, "ci_warmup_time");
    assert!(warmup.is_some(), "ci_warmup_time must be defined");
    let wu = warmup.unwrap();
    assert!(wu <= 30, "ci_warmup_time ({wu}s) must be <= 30s");

    let measurement = yaml_get_u64(&content, "ci_measurement_time");
    assert!(measurement.is_some(), "ci_measurement_time must be defined");
    let mt = measurement.unwrap();
    assert!(mt >= 1 && mt <= 60, "ci_measurement_time ({mt}s) must be in [1, 60]");
}

/// Verify regression detection logic: a value exceeding threshold is flagged.
#[test]
fn regression_detection_logic() {
    // Simulate: baseline = 100 ns, threshold = 10%
    let baseline_ns: u64 = 100;
    let max_regression_pct: f64 = 10.0;

    // Case 1: No regression (measured = 105 ns → 5% regression)
    let measured_no_regress: u64 = 105;
    let regression_pct =
        ((measured_no_regress as f64 - baseline_ns as f64) / baseline_ns as f64) * 100.0;
    assert!(
        regression_pct < max_regression_pct,
        "105ns vs 100ns baseline should NOT be flagged as regression"
    );

    // Case 2: Regression detected (measured = 115 ns → 15% regression)
    let measured_regress: u64 = 115;
    let regression_pct =
        ((measured_regress as f64 - baseline_ns as f64) / baseline_ns as f64) * 100.0;
    assert!(
        regression_pct >= max_regression_pct,
        "115ns vs 100ns baseline SHOULD be flagged as regression (15% > 10%)"
    );

    // Case 3: Boundary — exactly at threshold (measured = 110 ns → 10%)
    let measured_boundary: u64 = 110;
    let regression_pct =
        ((measured_boundary as f64 - baseline_ns as f64) / baseline_ns as f64) * 100.0;
    assert!(
        regression_pct >= max_regression_pct,
        "110ns vs 100ns baseline SHOULD be flagged (10% >= 10%)"
    );
}

/// Verify pass rate calculation logic.
#[test]
fn pass_rate_calculation() {
    // Simulate: 20 benchmarks, 1 fails
    let total: u64 = 20;
    let passed: u64 = 19;
    let pass_rate = (passed as f64 / total as f64) * 100.0;
    let min_pass_rate = 95.0;

    assert!(pass_rate >= min_pass_rate, "19/20 (95%) should meet 95% threshold");

    // 2 fail
    let passed2: u64 = 18;
    let pass_rate2 = (passed2 as f64 / total as f64) * 100.0;
    assert!(pass_rate2 < min_pass_rate, "18/20 (90%) should NOT meet 95% threshold");
}

/// Verify eval.yaml contains the required sections.
#[test]
fn eval_yaml_structure() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");

    assert!(content.contains("benchmarks:"), "eval.yaml must contain 'benchmarks:' section");
    assert!(content.contains("thresholds:"), "eval.yaml must contain 'thresholds:' section");
    assert!(
        content.contains("max_regression_pct:"),
        "eval.yaml thresholds must define max_regression_pct"
    );
    assert!(content.contains("min_pass_rate:"), "eval.yaml thresholds must define min_pass_rate");
}

/// Verify eval.yaml schema_version or documentation header exists.
#[test]
fn eval_yaml_documentation() {
    let content = fs::read_to_string("eval.yaml").expect("eval.yaml readable");

    // Must have comment header explaining purpose
    assert!(content.contains("C08"), "eval.yaml must reference C08 in comments");
    assert!(content.contains("T-1110"), "eval.yaml must reference T-1110 task ID");
    // Must document the bench targets it references
    for target in KNOWN_BENCH_TARGETS {
        assert!(content.contains(target), "eval.yaml must document bench_target '{target}'");
    }
}
