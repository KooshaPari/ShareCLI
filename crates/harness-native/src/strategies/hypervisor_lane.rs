use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sharecli_core::{Hypervisor, HypervisorConfig, SpawnRequest};
use sharecli_ipc::{CacheKeyMode, CoalesceCache, DEFAULT_NOCACHE_ARGS};

use super::RuleOpts;

/// Harness cache root for Hypervisor coalesce / queue state.
pub fn hypervisor_cache_root(harness_home: &Path) -> PathBuf {
    harness_home.join("var").join("sharecli-hypervisor")
}

/// Map rules.conf options into a [`HypervisorConfig`] (FR-008 AC-008.17).
pub fn config_from_rule_opts(harness_home: &Path, opts: &RuleOpts) -> HypervisorConfig {
    let cache_root = hypervisor_cache_root(harness_home);
    let coalesce_ttl =
        if opts.ttl == 0 { CoalesceCache::DEFAULT_TTL } else { Duration::from_secs(opts.ttl) };
    let queue_max_concurrent =
        if opts.max_concurrent == 0 { 1 } else { opts.max_concurrent as usize };
    HypervisorConfig {
        cache_root: cache_root.clone(),
        queue_root: cache_root.join("queue"),
        queue_max_concurrent,
        coalesce_ttl,
        coalesce_debounce: Duration::from_millis(opts.debounce_ms),
        cache_key_mode: CacheKeyMode::parse(&opts.cache_key),
        semantic: opts.semantic,
        error_ttl: Duration::from_secs(opts.error_ttl),
    }
}

/// Build a production hypervisor from harness home + rules.conf options.
///
/// Nocache semantics (Feb harness):
/// - Rule omits `nocache_args` → keep [`DEFAULT_NOCACHE_ARGS`] from [`Hypervisor::from_config`].
/// - Rule sets `nocache_args=` (empty) → no mutating bypass for that rule.
/// - Rule sets `nocache_args=--fix,...` → only those tokens bypass coalesce.
pub fn build_hypervisor(harness_home: &Path, opts: &RuleOpts) -> Hypervisor {
    let mut hv = Hypervisor::from_config(config_from_rule_opts(harness_home, opts));
    if let Some(flags) = &opts.nocache_args {
        hv.set_nocache_args(flags.clone());
    }
    hv
}

pub fn spawn_request(
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<SpawnRequest, String> {
    let cwd = env::current_dir().map_err(|e| format!("harness spawn: cwd: {e}"))?;
    let mut argv: Vec<String> = vec![real_cmd.to_string_lossy().into_owned()];
    argv.extend(args.iter().map(|s| (*s).to_string()));
    Ok(SpawnRequest::from_operator(argv, cwd, vec![], Some(&opts.priority)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sharecli_core::{FakeThermalGate, ThermalDecision};
    use tempfile::TempDir;

    use super::*;

    /// FR-008 / AC-008.17 — rules.conf ttl/debounce/max_concurrent map into HypervisorConfig.
    #[test]
    fn rule_opts_plumb_hypervisor_config() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { ttl: 42, debounce_ms: 99, max_concurrent: 3, ..RuleOpts::default() };
        let cfg = config_from_rule_opts(tmp.path(), &opts);
        assert_eq!(cfg.coalesce_ttl, Duration::from_secs(42));
        assert_eq!(cfg.coalesce_debounce, Duration::from_millis(99));
        assert_eq!(cfg.queue_max_concurrent, 3);

        let hv = Hypervisor::from_config_with_gate(
            cfg,
            Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
        );
        assert_eq!(hv.coalesce_ttl(), Duration::from_secs(42));
        assert_eq!(hv.coalesce_debounce(), Duration::from_millis(99));
        assert_eq!(hv.queue_max_concurrent(), 3);
    }

    /// FR-008 / AC-008.19 — rules.conf `cache_key=` maps into HypervisorConfig.
    #[test]
    fn rule_opts_plumb_cache_key_mode_and_semantic() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { cache_key: "git".to_string(), semantic: true, ..RuleOpts::default() };
        let cfg = config_from_rule_opts(tmp.path(), &opts);
        assert_eq!(cfg.cache_key_mode, CacheKeyMode::Git);
        assert!(cfg.semantic);
    }

    /// FR-008 / AC-008.19 — per-rule nocache_args override defaults when set.
    #[test]
    fn build_hypervisor_rule_nocache_args_override() {
        let tmp = TempDir::new().expect("tempdir");
        let opts =
            RuleOpts { nocache_args: Some(vec!["--custom".to_string()]), ..RuleOpts::default() };
        let hv = build_hypervisor(tmp.path(), &opts);
        assert_eq!(hv.nocache_args(), &["--custom"]);
    }

    /// FR-008 / AC-008.19 — explicit empty nocache_args disables bypass for rule.
    #[test]
    fn build_hypervisor_rule_nocache_args_empty_disables_bypass() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { nocache_args: Some(vec![]), ..RuleOpts::default() };
        let hv = build_hypervisor(tmp.path(), &opts);
        assert!(hv.nocache_args().is_empty());
    }

    /// FR-008 / AC-008.19 — omitted nocache_args keeps Hypervisor defaults.
    #[test]
    fn build_hypervisor_omitted_nocache_args_keeps_defaults() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts::default();
        let hv = build_hypervisor(tmp.path(), &opts);
        assert_eq!(
            hv.nocache_args(),
            DEFAULT_NOCACHE_ARGS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()
        );
    }

    /// FR-008 — rules.conf `error_ttl` plumbs into HypervisorConfig.
    #[test]
    fn rule_opts_plumb_error_ttl() {
        let tmp = TempDir::new().expect("tempdir");
        let opts = RuleOpts { error_ttl: 15, ..RuleOpts::default() };
        let cfg = config_from_rule_opts(tmp.path(), &opts);
        assert_eq!(
            cfg.error_ttl,
            Duration::from_secs(15),
            "error_ttl MUST be wired from RuleOpts to HypervisorConfig"
        );
    }
}
