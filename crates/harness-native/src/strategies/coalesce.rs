// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::path::Path;

use sharecli_core::Hypervisor;

use super::hypervisor_lane::{build_hypervisor, spawn_request};
use super::RuleOpts;

fn run_with_hypervisor(
    hv: &Hypervisor,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let req = spawn_request(real_cmd, args, opts)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("harness coalesce: tokio runtime: {e}"))?;

    let outcome =
        rt.block_on(hv.run(req)).map_err(|e| format!("harness coalesce: hypervisor: {e}"))?;

    Ok(outcome.exit_code)
}

/// Execute via Hypervisor Lock-Wait-Cache lane (FR-008 AC-008.17).
pub fn run(
    harness_home: &Path,
    real_cmd: &Path,
    _cmd_name: &str,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let hv = build_hypervisor(harness_home, opts);
    run_with_hypervisor(&hv, real_cmd, args, opts)
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

    /// FR-008 / AC-008.17 — harness `coalesce` strategy routes through Hypervisor (not raw spawn).
    #[test]
    fn coalesce_strategy_executes_via_hypervisor() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { ttl: 300, ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code =
            run_with_hypervisor(&hv, Path::new("/bin/echo"), &["harness-coalesce-ok"], &opts)
                .expect("coalesce strategy MUST succeed");
        assert_eq!(code, 0, "AC-008.17: harness coalesce MUST run via Hypervisor::run");
    }

    /// FR-008 / AC-008.17 — harness `cache` alias shares Hypervisor coalesce lane.
    #[test]
    fn cache_strategy_executes_via_hypervisor() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts::default();
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code = run_with_hypervisor(&hv, Path::new("/bin/echo"), &["harness-cache-ok"], &opts)
            .expect("cache strategy MUST succeed");
        assert_eq!(code, 0, "AC-008.17: harness cache MUST run via Hypervisor::run");
    }

    /// FR-008 / AC-008.17 — repeated identical invocations MUST coalesce via cache.
    #[test]
    fn coalesce_strategy_serves_cache_on_replay() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { ttl: 300, ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let req = spawn_request(Path::new("/bin/echo"), &["coalesce-replay"], &opts)
            .expect("spawn request");

        let rt =
            tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");

        let first = rt.block_on(hv.run(req.clone())).expect("first coalesce run");
        assert!(!first.from_cache, "AC-008.17: first coalesce run MUST miss cache");

        let second = rt.block_on(hv.run(req)).expect("second coalesce run");
        assert!(second.from_cache, "AC-008.17: harness coalesce MUST serve cache on replay");
        assert_eq!(second.exit_code, 0);
        let _ = hypervisor_cache_root(tmp.path());
    }
}
