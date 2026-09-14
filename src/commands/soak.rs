//! Harbor soak harness — long-running CLI stability evaluation.
//!
//! `sharecli soak [--duration <secs>] [--interval <secs>] [--config <path>] [--output <path>]`
//!
//! Runs a set of CLI scenarios repeatedly for the configured duration, tracking
//! error rates, latency percentiles, and memory usage.  Produces a JSON report
//! that the CI soak gate (`workflows/soak.yml`) and the integration test
//! (`tests/c08_harbor_soak_gate.rs`) can validate against thresholds.

// All public functions in this module are dispatched via the CLI subcommand
// enum, so the compiler sees them as unused. Suppress the warning.
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Top-level soak configuration loaded from `soak.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct SoakConfig {
    /// Total soak duration in seconds.
    pub duration_seconds: u64,

    /// Path to write the JSON report.
    #[serde(default = "default_report_file")]
    pub report_file: String,

    /// Thresholds that the soak gate asserts against.
    #[serde(default)]
    pub thresholds: Thresholds,

    /// Scenarios to execute each interval.
    #[serde(default = "default_scenarios")]
    pub scenarios: Vec<Scenario>,
}

fn default_report_file() -> String {
    "soak-report.json".to_string()
}

fn default_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "healthz".into(),
            command: vec!["sharecli".into(), "health".into(), "--json".into()],
            timeout_secs: 10,
        },
        Scenario {
            name: "status".into(),
            command: vec!["sharecli".into(), "status".into(), "--json".into()],
            timeout_secs: 15,
        },
        Scenario {
            name: "config-show".into(),
            command: vec!["sharecli".into(), "config".into(), "show".into(), "--json".into()],
            timeout_secs: 10,
        },
    ]
}

/// Thresholds for pass/fail gating.
#[derive(Debug, Clone, Deserialize)]
pub struct Thresholds {
    /// Maximum acceptable error rate (0.0–1.0).
    #[serde(default = "default_max_error_rate")]
    pub max_error_rate: f64,

    /// Minimum acceptable uptime percentage.
    #[serde(default = "default_min_uptime_pct")]
    pub min_uptime_pct: f64,

    /// Maximum acceptable p99 latency in milliseconds.
    #[serde(default = "default_max_p99_ms")]
    pub max_p99_latency_ms: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            max_error_rate: default_max_error_rate(),
            min_uptime_pct: default_min_uptime_pct(),
            max_p99_latency_ms: default_max_p99_ms(),
        }
    }
}

fn default_max_error_rate() -> f64 {
    0.05
}
fn default_min_uptime_pct() -> f64 {
    95.0
}
fn default_max_p99_ms() -> u64 {
    2000
}

/// A single soak scenario.
#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub command: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    10
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// JSON report emitted by the soak harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoakReport {
    pub version: String,
    pub sha: String,
    pub duration_sec: u64,
    pub interval_sec: u64,
    pub started_at: String,
    pub finished_at: String,
    pub total_requests: u64,
    pub errors: u64,
    pub error_rate: f64,
    pub uptime_pct: f64,
    pub p99_latency_ms: u64,
    pub p50_latency_ms: u64,
    pub max_memory_bytes: u64,
    pub scenario_results: Vec<ScenarioResult>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub runs: u64,
    pub errors: u64,
    pub p50_ms: u64,
    pub p99_ms: u64,
    pub max_memory_bytes: u64,
}

// ---------------------------------------------------------------------------
// Load config
// ---------------------------------------------------------------------------

/// Load soak config from a path, falling back to defaults if missing.
pub fn load_config(path: &Path) -> Result<SoakConfig> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("reading soak config from {}", path.display()))?;
        let config: SoakConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("parsing soak config from {}", path.display()))?;
        Ok(config)
    } else {
        Ok(SoakConfig {
            duration_seconds: 600,
            report_file: "soak-report.json".into(),
            thresholds: Thresholds::default(),
            scenarios: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Run the soak harness.
pub fn run(
    duration_secs: Option<u64>,
    interval_secs: Option<u64>,
    config_path: &Path,
    output_path: Option<&Path>,
) -> Result<SoakReport> {
    let mut config = load_config(config_path)?;

    // CLI overrides
    if let Some(d) = duration_secs {
        config.duration_seconds = d;
    }
    let interval = interval_secs.map(Duration::from_secs).unwrap_or_else(|| {
        let raw = config.duration_seconds as f64 / 10.0;
        Duration::from_millis((raw.clamp(1.0, 30.0) * 1000.0) as u64)
    });

    let report_path =
        output_path.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from(&config.report_file));

    let total_duration = Duration::from_secs(config.duration_seconds);
    let git_sha = get_git_sha();
    let started_at = chrono_timestamp();
    let start = Instant::now();

    // Per-scenario accumulators: (name, latencies, error_count, max_memory)
    let mut scenario_runs: Vec<(String, Vec<u64>, u64, u64)> =
        config.scenarios.iter().map(|s| (s.name.clone(), Vec::new(), 0u64, 0u64)).collect();

    let mut total_requests: u64 = 0;
    let mut total_errors: u64 = 0;
    let mut max_memory: u64 = 0;

    if config.scenarios.is_empty() {
        let report = SoakReport {
            version: "1".into(),
            sha: git_sha,
            duration_sec: config.duration_seconds,
            interval_sec: interval.as_secs(),
            started_at,
            finished_at: chrono_timestamp(),
            total_requests: 0,
            errors: 0,
            error_rate: 0.0,
            uptime_pct: 100.0,
            p99_latency_ms: 0,
            p50_latency_ms: 0,
            max_memory_bytes: 0,
            scenario_results: vec![],
            note: "no scenarios configured".into(),
        };
        write_report(&report_path, &report)?;
        return Ok(report);
    }

    eprintln!(
        "[soak] starting — {} scenarios, duration={}s, interval={}s",
        config.scenarios.len(),
        config.duration_seconds,
        interval.as_secs()
    );

    loop {
        if start.elapsed() >= total_duration {
            break;
        }

        for (i, scenario) in config.scenarios.iter().enumerate() {
            let cmd_name = &scenario.command[0];
            let args = &scenario.command[1..];

            let run_start = Instant::now();
            let result = Command::new(cmd_name).args(args).env("NO_COLOR", "1").output();

            let elapsed_ms = run_start.elapsed().as_millis() as u64;
            total_requests += 1;

            match result {
                Ok(output) => {
                    if !output.status.success() {
                        total_errors += 1;
                        scenario_runs[i].2 += 1;
                    }
                    scenario_runs[i].1.push(elapsed_ms);

                    let mem = get_process_memory_bytes();
                    if mem > max_memory {
                        max_memory = mem;
                    }
                    if mem > scenario_runs[i].3 {
                        scenario_runs[i].3 = mem;
                    }
                }
                Err(e) => {
                    total_errors += 1;
                    scenario_runs[i].2 += 1;
                    eprintln!("[soak] scenario '{}' failed to execute: {}", scenario.name, e);
                }
            }
        }

        let elapsed = start.elapsed();
        if elapsed + interval < total_duration {
            std::thread::sleep(interval);
        } else {
            break;
        }
    }

    let finished_at = chrono_timestamp();
    let duration_actual = start.elapsed().as_secs();

    let error_rate =
        if total_requests > 0 { total_errors as f64 / total_requests as f64 } else { 0.0 };
    let uptime_pct = if total_requests > 0 {
        ((total_requests - total_errors) as f64 / total_requests as f64) * 100.0
    } else {
        100.0
    };

    let mut all_latencies: Vec<u64> =
        scenario_runs.iter().flat_map(|(_, lats, _, _)| lats.iter().copied()).collect();
    all_latencies.sort_unstable();

    let p50 = percentile(&all_latencies, 50);
    let p99 = percentile(&all_latencies, 99);

    let scenario_results: Vec<ScenarioResult> = scenario_runs
        .into_iter()
        .map(|(name, mut latencies, errors, max_mem)| {
            latencies.sort_unstable();
            ScenarioResult {
                name,
                runs: latencies.len() as u64,
                errors,
                p50_ms: percentile(&latencies, 50),
                p99_ms: percentile(&latencies, 99),
                max_memory_bytes: max_mem,
            }
        })
        .collect();

    let report = SoakReport {
        version: "1".into(),
        sha: git_sha,
        duration_sec: duration_actual,
        interval_sec: interval.as_secs(),
        started_at,
        finished_at,
        total_requests,
        errors: total_errors,
        error_rate,
        uptime_pct,
        p99_latency_ms: p99,
        p50_latency_ms: p50,
        max_memory_bytes: max_memory,
        scenario_results,
        note: String::new(),
    };

    eprintln!(
        "[soak] finished — {} requests, {} errors, error_rate={:.4}, uptime={:.1}%, p99={}ms",
        report.total_requests,
        report.errors,
        report.error_rate,
        report.uptime_pct,
        report.p99_latency_ms
    );

    write_report(&report_path, &report)?;
    Ok(report)
}

// ---------------------------------------------------------------------------
// Report subcommand
// ---------------------------------------------------------------------------

/// `sharecli soak report [--output <path>]` — print or re-generate the report.
pub fn report_cmd(output_path: &Path) -> Result<()> {
    if !output_path.exists() {
        bail!("no soak report at {}", output_path.display());
    }
    let content = fs::read_to_string(output_path)
        .with_context(|| format!("reading {}", output_path.display()))?;
    let report: SoakReport = serde_json::from_str(&content)
        .with_context(|| format!("parsing {}", output_path.display()))?;

    println!(
        "Soak Report (sha={})\n\
         Duration: {}s | Interval: {}s\n\
         Requests: {} | Errors: {} | Error rate: {:.4}\n\
         Uptime: {:.1}% | p50: {}ms | p99: {}ms\n\
         Max memory: {} bytes",
        report.sha,
        report.duration_sec,
        report.interval_sec,
        report.total_requests,
        report.errors,
        report.error_rate,
        report.uptime_pct,
        report.p50_latency_ms,
        report.p99_latency_ms,
        report.max_memory_bytes,
    );

    if !report.scenario_results.is_empty() {
        println!("\nScenario breakdown:");
        for sr in &report.scenario_results {
            println!(
                "  {} — runs={}, errors={}, p50={}ms, p99={}ms",
                sr.name, sr.runs, sr.errors, sr.p50_ms, sr.p99_ms
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_report(path: &Path, report: &SoakReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report).context("serializing soak report")?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    let mut f = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    f.write_all(json.as_bytes())?;
    eprintln!("[soak] report written to {}", path.display());
    Ok(())
}

/// Compute percentile (0–100) from a sorted slice.
fn percentile(sorted: &[u64], p: u64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p as f64 / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx]
}

/// Get current process RSS in bytes (Linux procfs). Returns 0 on other platforms.
fn get_process_memory_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(kvp) = line.strip_prefix("VmRSS:") {
                    if let Some(kb_str) = kvp.trim().strip_suffix(" kB") {
                        if let Ok(kb) = kb_str.trim().parse::<u64>() {
                            return kb * 1024;
                        }
                    }
                }
            }
        }
    }
    0
}

/// Get the current git SHA (short).
fn get_git_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".into())
}

/// Current UTC timestamp in ISO 8601.
fn chrono_timestamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_empty() {
        assert_eq!(percentile(&[], 50), 0);
    }

    #[test]
    fn percentile_single() {
        assert_eq!(percentile(&[100], 50), 100);
    }

    #[test]
    fn percentile_p50() {
        let data: Vec<u64> = (1..=100).collect();
        // idx = round(0.5 * 99) = 50; data[50] = 51
        assert_eq!(percentile(&data, 50), 51);
    }

    #[test]
    fn percentile_p99() {
        let data: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile(&data, 99), 99);
    }

    #[test]
    fn days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn default_config_has_scenarios() {
        let cfg = SoakConfig {
            duration_seconds: 600,
            report_file: "test-report.json".into(),
            thresholds: Thresholds::default(),
            scenarios: default_scenarios(),
        };
        assert_eq!(cfg.scenarios.len(), 3);
        assert_eq!(cfg.scenarios[0].name, "healthz");
    }

    #[test]
    fn load_config_missing_file() {
        let cfg = load_config(Path::new("/nonexistent/soak.yaml")).unwrap();
        assert_eq!(cfg.duration_seconds, 600);
        assert!(cfg.scenarios.is_empty());
    }

    #[test]
    fn threshold_defaults() {
        let t = Thresholds::default();
        assert!((t.max_error_rate - 0.05).abs() < f64::EPSILON);
        assert!((t.min_uptime_pct - 95.0).abs() < f64::EPSILON);
        assert_eq!(t.max_p99_latency_ms, 2000);
    }

    // --- Additional edge-case tests ---

    #[test]
    fn percentile_p0() {
        let data = vec![10, 20, 30, 40, 50];
        assert_eq!(percentile(&data, 0), 10);
    }

    #[test]
    fn percentile_p100() {
        let data = vec![10, 20, 30, 40, 50];
        assert_eq!(percentile(&data, 100), 50);
    }

    #[test]
    fn percentile_two_elements() {
        let data = vec![10, 20];
        // idx = round(0.5 * 1) = 1 → data[1] = 20
        assert_eq!(percentile(&data, 50), 20);
    }

    #[test]
    fn percentile_large_dataset() {
        let data: Vec<u64> = (1..=1000).collect();
        assert_eq!(percentile(&data, 50), 501);
        assert_eq!(percentile(&data, 99), 990);
        assert_eq!(percentile(&data, 1), 11);
    }

    #[test]
    fn days_to_ymd_recent_date() {
        // 2024-01-15 = 19737 days since epoch
        let (y, m, d) = days_to_ymd(19737);
        assert_eq!((y, m, d), (2024, 1, 15));
    }

    #[test]
    fn days_to_ymd_leap_year_boundary() {
        // 2000-03-01 (leap year) = 11017
        let (y, m, d) = days_to_ymd(11017);
        assert_eq!((y, m, d), (2000, 3, 1));
    }

    #[test]
    fn days_to_ymd_year_2038() {
        // 2038-01-19 = 24855 (Unix 32-bit overflow date)
        let (y, m, d) = days_to_ymd(24855);
        assert_eq!((y, m, d), (2038, 1, 19));
    }

    #[test]
    fn days_to_ymd_jan_1_2000() {
        // Jan 1 2000 = 10957 days since epoch
        let (y, m, d) = days_to_ymd(10957);
        assert_eq!((y, m, d), (2000, 1, 1));
    }

    #[test]
    fn chrono_timestamp_format() {
        let ts = chrono_timestamp();
        assert!(ts.ends_with('Z'), "timestamp MUST end with Z; got: {ts}");
        assert_eq!(ts.len(), 20, "timestamp MUST be ISO 8601 length; got: {ts}");
        assert!(ts.contains('T'), "timestamp MUST contain T separator");
    }

    #[test]
    fn default_timeout_value() {
        assert_eq!(default_timeout(), 10);
    }

    #[test]
    fn default_report_file_value() {
        assert_eq!(default_report_file(), "soak-report.json");
    }

    #[test]
    fn scenario_defaults() {
        let s = Scenario {
            name: "test".into(),
            command: vec!["echo".into(), "hi".into()],
            timeout_secs: default_timeout(),
        };
        assert_eq!(s.timeout_secs, 10);
    }

    #[test]
    fn load_config_valid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("soak.yaml");
        fs::write(
            &path,
            r#"
duration_seconds: 120
report_file: "custom-report.json"
thresholds:
  max_error_rate: 0.10
  min_uptime_pct: 99.0
  max_p99_latency_ms: 5000
scenarios:
  - name: "test-scenario"
    command: ["echo", "hello"]
    timeout_secs: 5
"#,
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.duration_seconds, 120);
        assert_eq!(config.report_file, "custom-report.json");
        assert!((config.thresholds.max_error_rate - 0.10).abs() < f64::EPSILON);
        assert!((config.thresholds.min_uptime_pct - 99.0).abs() < f64::EPSILON);
        assert_eq!(config.thresholds.max_p99_latency_ms, 5000);
        assert_eq!(config.scenarios.len(), 1);
        assert_eq!(config.scenarios[0].name, "test-scenario");
        assert_eq!(config.scenarios[0].timeout_secs, 5);
    }

    #[test]
    fn load_config_invalid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("soak.yaml");
        fs::write(&path, "not valid: [yaml: {{{{").unwrap();
        let result = load_config(&path);
        assert!(result.is_err(), "invalid YAML MUST return error");
    }

    #[test]
    fn soak_report_json_roundtrip() {
        let report = SoakReport {
            version: "1".into(),
            sha: "abc123".into(),
            duration_sec: 300,
            interval_sec: 30,
            started_at: "2024-01-01T00:00:00Z".into(),
            finished_at: "2024-01-01T00:05:00Z".into(),
            total_requests: 100,
            errors: 3,
            error_rate: 0.03,
            uptime_pct: 97.0,
            p99_latency_ms: 1500,
            p50_latency_ms: 200,
            max_memory_bytes: 1024 * 1024,
            scenario_results: vec![ScenarioResult {
                name: "healthz".into(),
                runs: 50,
                errors: 1,
                p50_ms: 180,
                p99_ms: 1200,
                max_memory_bytes: 512 * 1024,
            }],
            note: String::new(),
        };

        let json = serde_json::to_string(&report).unwrap();
        let back: SoakReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total_requests, 100);
        assert_eq!(back.errors, 3);
        assert!((back.error_rate - 0.03).abs() < f64::EPSILON);
        assert_eq!(back.scenario_results.len(), 1);
        assert_eq!(back.scenario_results[0].name, "healthz");
    }

    #[test]
    fn write_report_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output").join("report.json");
        let report = SoakReport {
            version: "1".into(),
            sha: "test".into(),
            duration_sec: 60,
            interval_sec: 10,
            started_at: "2024-01-01T00:00:00Z".into(),
            finished_at: "2024-01-01T00:01:00Z".into(),
            total_requests: 10,
            errors: 0,
            error_rate: 0.0,
            uptime_pct: 100.0,
            p99_latency_ms: 100,
            p50_latency_ms: 50,
            max_memory_bytes: 0,
            scenario_results: vec![],
            note: "test report".into(),
        };
        write_report(&path, &report).unwrap();
        assert!(path.exists(), "report file MUST be created");

        let content = fs::read_to_string(&path).unwrap();
        let parsed: SoakReport = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.note, "test report");
    }

    #[test]
    fn report_cmd_missing_file_fails() {
        let result = report_cmd(Path::new("/nonexistent/report.json"));
        assert!(result.is_err());
    }

    #[test]
    fn report_cmd_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");
        let report = SoakReport {
            version: "1".into(),
            sha: "abc".into(),
            duration_sec: 120,
            interval_sec: 15,
            started_at: "2024-01-01T00:00:00Z".into(),
            finished_at: "2024-01-01T00:02:00Z".into(),
            total_requests: 50,
            errors: 2,
            error_rate: 0.04,
            uptime_pct: 96.0,
            p99_latency_ms: 800,
            p50_latency_ms: 100,
            max_memory_bytes: 2048,
            scenario_results: vec![ScenarioResult {
                name: "healthz".into(),
                runs: 25,
                errors: 1,
                p50_ms: 90,
                p99_ms: 700,
                max_memory_bytes: 1024,
            }],
            note: String::new(),
        };
        let json = serde_json::to_string(&report).unwrap();
        fs::write(&path, json).unwrap();

        let result = report_cmd(&path);
        assert!(result.is_ok(), "valid report MUST parse successfully");
    }

    #[test]
    fn get_git_sha_returns_nonempty() {
        let sha = get_git_sha();
        assert!(!sha.is_empty(), "git SHA must not be empty (unless not in a git repo)");
    }

    #[test]
    fn get_process_memory_bytes_returns_value() {
        // On macOS this always returns 0 (no procfs), on Linux returns RSS
        let mem = get_process_memory_bytes();
        let _ = mem; // just verify it doesn't panic
    }

    #[test]
    fn scenario_result_defaults() {
        let sr = ScenarioResult {
            name: "test".into(),
            runs: 0,
            errors: 0,
            p50_ms: 0,
            p99_ms: 0,
            max_memory_bytes: 0,
        };
        assert_eq!(sr.runs, 0);
        assert_eq!(sr.errors, 0);
    }

    #[test]
    fn soake_config_defaults_for_missing_fields() {
        let yaml = r#"duration_seconds: 60"#;
        let config: SoakConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.duration_seconds, 60);
        assert_eq!(config.report_file, "soak-report.json");
        // Default config provides 3 built-in scenarios
        assert_eq!(config.scenarios.len(), 3);
        assert!((config.thresholds.max_error_rate - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn percentile_sorted_not_mutated() {
        let mut data = vec![50, 10, 30, 20, 40];
        let original = data.clone();
        let _ = percentile(&data, 50);
        assert_eq!(data, original, "percentile MUST NOT mutate input");
    }
}
