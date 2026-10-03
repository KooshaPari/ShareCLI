use std::path::Path;

use sharecli_core::{ChildCommandConfigurator, Hypervisor};

use super::hypervisor_lane::{build_hypervisor, spawn_request};
use super::RuleOpts;

fn run_with_hypervisor(
    hv: &Hypervisor,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    run_with_hypervisor_configurator(hv, real_cmd, args, opts, None)
}

fn run_with_hypervisor_configurator(
    hv: &Hypervisor,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
    configurator: Option<&dyn ChildCommandConfigurator>,
) -> Result<i32, String> {
    let req = spawn_request(real_cmd, args, opts)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("harness process: tokio runtime: {e}"))?;

    let outcome = match configurator {
        Some(configurator) => rt.block_on(hv.run_with_command_configurator(req, configurator)),
        None => rt.block_on(hv.run(req)),
    }
    .map_err(|e| format!("harness process: hypervisor: {e}"))?;

    Ok(outcome.exit_code)
}

/// Execute via Hypervisor coalesce lane (FR-008 AC-008.19).
///
/// Replaces raw `Command::spawn` for `passthrough` and strategies that delegate here.
pub fn run_status(
    harness_home: &Path,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    let hv = build_hypervisor(harness_home, opts);
    run_with_hypervisor(&hv, real_cmd, args, opts)
}

/// Execute through the same Hypervisor lane while allowing a native admission
/// provider to configure the exact child command.
pub fn run_status_with_command_configurator(
    harness_home: &Path,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
    configurator: &dyn ChildCommandConfigurator,
) -> Result<i32, String> {
    let hv = build_hypervisor(harness_home, opts);
    run_with_hypervisor_configurator(&hv, real_cmd, args, opts, Some(configurator))
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

    /// FR-008 / AC-008.19 — harness `passthrough` / process routes through Hypervisor.
    #[test]
    fn process_strategy_executes_via_hypervisor() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts::default();
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code = run_with_hypervisor(&hv, Path::new("/bin/echo"), &["harness-process-ok"], &opts)
            .expect("process strategy MUST succeed");
        assert_eq!(code, 0, "AC-008.19: harness process MUST run via Hypervisor::run");
    }

    /// FR-008 / AC-008.19 — missing binary fails loudly (no silent degrade).
    #[test]
    fn process_strategy_missing_binary_fails_loudly() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts::default();
        let hv = allow_hypervisor(tmp.path(), &opts);
        let err = run_with_hypervisor(
            &hv,
            Path::new("/nonexistent/sharecli-ac-008-19-missing"),
            &["x"],
            &opts,
        )
        .expect_err("missing binary MUST fail");
        assert!(
            err.contains("hypervisor") || err.contains("No such file") || err.contains("not found"),
            "AC-008.19: missing binary error must be loud, got: {err}"
        );
    }
}
