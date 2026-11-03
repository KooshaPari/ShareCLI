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
        .map_err(|e| format!("harness debounce: tokio runtime: {e}"))?;

    let outcome =
        rt.block_on(hv.run(req)).map_err(|e| format!("harness debounce: hypervisor: {e}"))?;

    Ok(outcome.exit_code)
}

/// Execute via Hypervisor Lock-Wait-Cache lane with debounce window (FR-008 AC-008.18).
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
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use sharecli_core::{FakeThermalGate, Hypervisor, ThermalDecision};
    use sharecli_ipc::{command_key, CachedResult, CoalesceCache};
    use tempfile::TempDir;

    use super::super::hypervisor_lane::{config_from_rule_opts, hypervisor_cache_root};
    use super::*;

    fn allow_hypervisor(dir: &Path, opts: &RuleOpts) -> Hypervisor {
        Hypervisor::from_config_with_gate(
            config_from_rule_opts(dir, opts),
            Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
        )
    }

    /// FR-008 / AC-008.18 — harness `debounce` strategy routes through Hypervisor (not raw spawn).
    #[test]
    fn debounce_strategy_executes_via_hypervisor() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { ttl: 300, debounce_ms: 75, ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let code =
            run_with_hypervisor(&hv, Path::new("/bin/echo"), &["harness-debounce-ok"], &opts)
                .expect("debounce strategy MUST succeed");
        assert_eq!(code, 0, "AC-008.18: harness debounce MUST run via Hypervisor::run");
    }

    /// FR-008 / AC-008.18 — repeated identical invocations MUST coalesce via cache.
    #[test]
    fn debounce_strategy_serves_cache_on_replay() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { ttl: 300, debounce_ms: 75, ..RuleOpts::default() };
        let hv = allow_hypervisor(tmp.path(), &opts);
        let req = spawn_request(Path::new("/bin/echo"), &["debounce-replay"], &opts)
            .expect("spawn request");

        let rt =
            tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");

        let first = rt.block_on(hv.run(req.clone())).expect("first debounce run");
        assert!(!first.from_cache, "AC-008.18: first debounce run MUST miss cache");

        let second = rt.block_on(hv.run(req)).expect("second debounce run");
        assert!(second.from_cache, "AC-008.18: harness debounce MUST serve cache on replay");
        assert_eq!(second.exit_code, 0);
        let _ = hypervisor_cache_root(tmp.path());
    }

    /// FR-008 / AC-008.18 — debounce_ms MUST share an in-window sibling store (AC-008.6 path).
    #[test]
    fn debounce_strategy_shares_in_window_store() {
        let tmp = TempDir::new().expect("tempdir");
        let debounce = Duration::from_millis(120);
        let opts =
            RuleOpts { ttl: 300, debounce_ms: debounce.as_millis() as u64, ..RuleOpts::default() };
        let cache_root = hypervisor_cache_root(tmp.path());
        let hv = allow_hypervisor(tmp.path(), &opts);

        let req = spawn_request(Path::new("/bin/echo"), &["debounce-in-window"], &opts)
            .expect("spawn request");
        let key = command_key(&req.argv, &req.cwd, &req.env);
        let hits = Arc::new(AtomicU32::new(0));

        let cache_bg = CoalesceCache::with_options(&cache_root, Duration::from_secs(300), debounce);
        let key_bg = key.clone();
        let hits_bg = Arc::clone(&hits);
        let producer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            cache_bg
                .store(
                    &key_bg,
                    &CachedResult {
                        exit_code: 0,
                        stdout: b"shared-debounce".to_vec(),
                        stderr: vec![],
                    },
                )
                .expect("bg store");
            hits_bg.fetch_add(1, Ordering::SeqCst);
        });

        let rt =
            tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
        let outcome = rt.block_on(hv.run(req)).expect("debounced harness run");

        producer.join().expect("producer join");

        assert_eq!(
            outcome.stdout, b"shared-debounce",
            "AC-008.18: harness debounce MUST share in-window result"
        );
        assert_eq!(outcome.exit_code, 0);
        assert!(
            outcome.from_cache,
            "AC-008.18: debounce share MUST surface as from_cache on Hypervisor::run"
        );
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "AC-008.18: miss path MUST NOT run when debounce shares in-window store"
        );
    }
}
