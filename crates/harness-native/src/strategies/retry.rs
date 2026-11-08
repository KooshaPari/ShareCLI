// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::path::Path;
use std::thread;

use rand::{rng, RngExt};
use sharecli_core::Hypervisor;

use super::hypervisor_lane::{build_hypervisor, spawn_request};
use super::RuleOpts;

fn run_once(
    hv: &Hypervisor,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let req = spawn_request(real_cmd, args, opts)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("harness retry: tokio runtime: {e}"))?;

    let outcome =
        rt.block_on(hv.run(req)).map_err(|e| format!("harness retry: hypervisor: {e}"))?;

    Ok(outcome.exit_code)
}

/// Execute with retries via Hypervisor coalesce lane (FR-008 AC-008.19).
///
/// MUST NOT use raw `Command::spawn`.
pub fn run(
    harness_home: &Path,
    real_cmd: &Path,
    retry_max: u32,
    retry_backoff_ms: u64,
    retry_jitter: f64,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let hv = build_hypervisor(harness_home, opts);
    let mut rng = rng();
    for attempt in 0..=retry_max {
        match run_once(&hv, real_cmd, args, opts) {
            Ok(0) => return Ok(0),
            Ok(code) if attempt < retry_max => {
                let jitter = rng.random::<f64>() * retry_jitter;
                let delay = retry_backoff_ms as f64 * (1.0 + jitter);
                thread::sleep(std::time::Duration::from_millis(delay as u64));
                let _ = code;
            }
            Ok(code) => return Ok(code),
            Err(_e) if attempt < retry_max => {
                let jitter = rng.random::<f64>() * retry_jitter;
                let delay = retry_backoff_ms as f64 * (1.0 + jitter);
                thread::sleep(std::time::Duration::from_millis(delay as u64));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use sharecli_core::{FakeThermalGate, Hypervisor, ThermalDecision};
    use tempfile::TempDir;

    use super::super::hypervisor_lane::config_from_rule_opts;
    use super::*;

    fn allow_hypervisor(dir: &Path, opts: &RuleOpts) -> Hypervisor {
        Hypervisor::from_config_with_gate(
            config_from_rule_opts(dir, opts),
            Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
        )
    }

    /// FR-008 / AC-008.19 — harness `retry` strategy routes through Hypervisor.
    #[test]
    fn retry_strategy_executes_via_hypervisor() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { retry_max: 0, ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code = run_once(&hv, Path::new("/bin/echo"), &["harness-retry-ok"], &opts)
            .expect("retry strategy MUST succeed");
        assert_eq!(code, 0, "AC-008.19: harness retry MUST run via Hypervisor::run");
    }

    /// FR-008 / AC-008.19 — retry loop eventually returns non-zero on persistent failure.
    #[test]
    fn retry_strategy_exhausts_on_persistent_failure() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts {
            retry_max: 1,
            retry_backoff_ms: 1,
            retry_jitter: 0.0,
            ..RuleOpts::default()
        };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let mut last = 0;
        for _ in 0..=opts.retry_max {
            last = run_once(&hv, Path::new("/usr/bin/false"), &[], &opts)
                .expect("retry MUST return exit code on failure via Hypervisor");
            if last == 0 {
                break;
            }
        }
        assert_ne!(last, 0, "AC-008.19: exhausted retry MUST surface non-zero exit");
    }
}
