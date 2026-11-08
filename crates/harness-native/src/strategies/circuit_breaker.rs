// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sharecli_core::Hypervisor;

use super::hypervisor_lane::{build_hypervisor, spawn_request};
use super::RuleOpts;

static FAILURE_COUNT: AtomicU32 = AtomicU32::new(0);
static WINDOW_START_MS: AtomicU64 = AtomicU64::new(0);
/// Serialize breaker mutations — global atomics are shared across harness invocations/tests.
static BREAKER_LOCK: Mutex<()> = Mutex::new(());

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

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
        .map_err(|e| format!("harness circuit_breaker: tokio runtime: {e}"))?;

    let outcome = rt
        .block_on(hv.run(req))
        .map_err(|e| format!("harness circuit_breaker: hypervisor: {e}"))?;

    Ok(outcome.exit_code)
}

/// Execute behind a failure-window breaker via Hypervisor (FR-008 AC-008.19).
///
/// MUST NOT use raw `Command::spawn`.
pub fn run(
    harness_home: &Path,
    real_cmd: &Path,
    breaker_threshold: u32,
    breaker_window_secs: u64,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let _guard = BREAKER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let window_ms = breaker_window_secs.saturating_mul(1000);
    let prev_start = WINDOW_START_MS.load(Ordering::SeqCst);
    let now = now_ms();

    if now.saturating_sub(prev_start) >= window_ms {
        WINDOW_START_MS.store(now, Ordering::SeqCst);
        FAILURE_COUNT.store(0, Ordering::SeqCst);
    }

    if FAILURE_COUNT.load(Ordering::SeqCst) >= breaker_threshold {
        return Err("circuit open".to_string());
    }

    let hv = build_hypervisor(harness_home, opts);
    match run_once(&hv, real_cmd, args, opts) {
        Ok(0) => {
            FAILURE_COUNT.store(0, Ordering::SeqCst);
            Ok(0)
        }
        Ok(code) => {
            FAILURE_COUNT.fetch_add(1, Ordering::SeqCst);
            Err(format!("command failed: exit {code}"))
        }
        Err(e) => {
            FAILURE_COUNT.fetch_add(1, Ordering::SeqCst);
            Err(e)
        }
    }
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

    /// FR-008 / AC-008.19 — harness `circuit_breaker` routes through Hypervisor.
    #[test]
    fn circuit_breaker_strategy_executes_via_hypervisor() {
        let _guard = BREAKER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        FAILURE_COUNT.store(0, Ordering::SeqCst);
        WINDOW_START_MS.store(now_ms(), Ordering::SeqCst);

        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { breaker_threshold: 3, breaker_window: 60, ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code = run_once(&hv, Path::new("/bin/echo"), &["harness-breaker-ok"], &opts)
            .expect("circuit_breaker strategy MUST succeed");
        assert_eq!(code, 0, "AC-008.19: harness circuit_breaker MUST run via Hypervisor::run");
    }

    /// FR-008 / AC-008.19 — open circuit fails loudly without spawning.
    #[test]
    fn circuit_breaker_open_fails_loudly() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { breaker_threshold: 1, breaker_window: 3600, ..RuleOpts::default() };

        {
            let _guard = BREAKER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            FAILURE_COUNT.store(5, Ordering::SeqCst);
            WINDOW_START_MS.store(now_ms(), Ordering::SeqCst);
        }

        let err = run(
            tmp.path(),
            Path::new("/bin/echo"),
            opts.breaker_threshold,
            opts.breaker_window,
            &["should-not-run"],
            &opts,
        )
        .expect_err("open circuit MUST fail");
        assert_eq!(err, "circuit open", "AC-008.19: open circuit must fail loudly");

        let _guard = BREAKER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        FAILURE_COUNT.store(0, Ordering::SeqCst);
        WINDOW_START_MS.store(0, Ordering::SeqCst);
    }
}
