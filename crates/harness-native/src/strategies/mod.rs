// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Execution strategies: coalesce, queue, debounce, retry, etc.

mod batch;
mod causal_order;
mod circuit_breaker;
mod coalesce;
mod debounce;
mod hypervisor_lane;
mod incremental;
mod jobserver;
mod load_balance;
mod proactive_warm;
mod process;
mod queue;
mod resource_throttle;
mod retry;
mod speculative;

use std::path::Path;

/// Rule options parsed from rules.conf
#[derive(Debug, Clone, Default)]
pub struct RuleOpts {
    pub ttl: u64,
    pub debounce_ms: u64,
    pub error_ttl: u64,
    pub max_concurrent: u32,
    pub priority: String,
    pub cache_key: String,
    pub batch_key: String,
    pub causal_domain: String,
    pub breaker_threshold: u32,
    pub breaker_window: u64,
    pub breaker_cooldown: u64,
    pub retry_max: u32,
    pub retry_backoff_ms: u64,
    pub retry_jitter: f64,
    pub jobserver_auth: String,
    pub jobserver_tokens: u32,
    pub jobserver_borrow: bool,
    pub stale_threshold: u64,
    pub semantic: bool,
    /// Per-rule nocache flag list from rules.conf `nocache_args=`.
    ///
    /// `None` — rule did not specify; harness [`build_hypervisor`] keeps
    /// [`DEFAULT_NOCACHE_ARGS`] from [`Hypervisor::from_config`].
    /// `Some(vec![])` — rule set `nocache_args=` (explicit empty → no bypass).
    /// `Some(flags)` — rule-listed mutating tokens only.
    pub nocache_args: Option<Vec<String>>,
}

/// Execute a strategy. Returns exit code.
pub struct ExecRequest<'a> {
    pub strategy: &'a str,
    pub harness_home: &'a Path,
    pub real_cmd: &'a Path,
    pub cmd_name: &'a str,
    pub subcmd: &'a str,
    pub cache_key: &'a str,
    pub opts: &'a RuleOpts,
    pub args: &'a [String],
    pub agent_name: &'a str,
}

pub fn execute(req: ExecRequest<'_>) -> Result<i32, String> {
    let full_args: Vec<&str> = req.args.iter().map(|s| s.as_str()).collect();

    match req.strategy {
        "passthrough" => process::run_status(req.harness_home, req.real_cmd, &full_args, req.opts),
        "coalesce" | "cache" => {
            coalesce::run(req.harness_home, req.real_cmd, req.cmd_name, &full_args, req.opts)
        }
        "queue" | "priority_queue" => {
            queue::run(req.harness_home, req.real_cmd, req.cmd_name, &full_args, req.opts)
        }
        "debounce" => {
            debounce::run(req.harness_home, req.real_cmd, req.cmd_name, &full_args, req.opts)
        }
        "retry" => retry::run(
            req.harness_home,
            req.real_cmd,
            req.opts.retry_max,
            req.opts.retry_backoff_ms,
            req.opts.retry_jitter,
            &full_args,
            req.opts,
        ),
        "incremental" => incremental::run(req.harness_home, req.real_cmd, &full_args, req.opts),
        "circuit_breaker" => circuit_breaker::run(
            req.harness_home,
            req.real_cmd,
            req.opts.breaker_threshold,
            req.opts.breaker_window,
            &full_args,
            req.opts,
        ),
        "resource_throttle" => {
            resource_throttle::run(req.harness_home, req.real_cmd, &full_args, req.opts)
        }
        "jobserver" => jobserver::run(req.harness_home, req.real_cmd, &full_args, req.opts),
        "load_balance" => load_balance::run(req.harness_home, req.real_cmd, &full_args, req.opts),
        "speculative" => speculative::run(req.harness_home, req.real_cmd, &full_args, req.opts),
        "proactive_warm" => {
            proactive_warm::run(req.harness_home, req.real_cmd, &full_args, req.opts)
        }
        "batch" => batch::run(req.harness_home, req.real_cmd, &full_args, req.opts),
        "causal_order" => causal_order::run(req.harness_home, req.real_cmd, &full_args, req.opts),
        _ => {
            let _ = (req.cmd_name, req.subcmd, req.cache_key, req.agent_name);
            coalesce::run(req.harness_home, req.real_cmd, req.cmd_name, &full_args, req.opts)
        }
    }
}
