// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::path::Path;

use sharecli_core::Hypervisor;

use super::hypervisor_lane::{build_hypervisor, spawn_request};
use super::RuleOpts;

fn run_with_hypervisor(
    hv: &Hypervisor,
    real_cmd: &Path,
    cmd_name: &str,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let req = spawn_request(real_cmd, args, opts)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("harness queue: tokio runtime: {e}"))?;

    let outcome = rt
        .block_on(hv.run_queued(req, cmd_name))
        .map_err(|e| format!("harness queue: hypervisor: {e}"))?;

    Ok(outcome.exit_code)
}

/// Execute via Hypervisor SlotQueue lane (FR-008 AC-008.16).
pub fn run(
    harness_home: &Path,
    real_cmd: &Path,
    cmd_name: &str,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let hv = build_hypervisor(harness_home, opts);
    run_with_hypervisor(&hv, real_cmd, cmd_name, args, opts)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use sharecli_core::{FakeThermalGate, Hypervisor, ThermalDecision};
    use tempfile::TempDir;

    use super::super::hypervisor_lane::{config_from_rule_opts, hypervisor_cache_root};
    use super::*;

    fn allow_hypervisor(dir: &Path, opts: &RuleOpts) -> Hypervisor {
        Hypervisor::from_config_with_gate(
            config_from_rule_opts(dir, opts),
            Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
        )
    }

    /// FR-008 / AC-008.16 — harness `queue` strategy routes through Hypervisor (not raw spawn).
    #[test]
    fn queue_strategy_executes_via_hypervisor() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { priority: "normal".to_string(), ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code =
            run_with_hypervisor(&hv, Path::new("/bin/echo"), "echo", &["harness-queue-ok"], &opts)
                .expect("queue strategy MUST succeed");
        assert_eq!(code, 0, "AC-008.16: harness queue MUST run via Hypervisor::run_queued");
    }

    /// FR-008 / AC-008.16 — `priority_queue` strategy shares Hypervisor queue lane.
    #[test]
    fn priority_queue_strategy_executes_via_hypervisor() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { priority: "high".to_string(), ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code =
            run_with_hypervisor(&hv, Path::new("/bin/echo"), "echo", &["priority-queue-ok"], &opts)
                .expect("priority_queue strategy MUST succeed");
        assert_eq!(
            code, 0,
            "AC-008.16: harness priority_queue MUST run via Hypervisor::run_queued"
        );
        let _ = hypervisor_cache_root(tmp.path());
    }
}
