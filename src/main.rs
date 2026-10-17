//! sharecli - Shared CLI process manager
//!
//! This file owns the top-level `Cli` argument parser (re-exported from
//! `crate::subcommands::root::Cli`), tracing initialisation, and a thin
//! `run()` that defers all subcommand dispatch to
//! [`crate::cli_dispatch::dispatch`].
//!
//! Concrete subcommand impls live in:
//!   - `cli_impl_session`  -- session / surface
//!   - `cli_impl_meta`     -- man / list / version / fleet / proc-compose
//!   - `cli_impl_optimize` -- optimize / prune
//!
//! Subcommand enums (one per surface) live under `crate::subcommands`.

mod alloc;
mod plugins;

use std::io::Write;

use anyhow::Result;
use clap::Parser;

use crate::error::SharecliError;

mod audit_log;
mod cast;
mod cli_dispatch;
mod cli_impl_meta;
mod cli_impl_optimize;
mod cli_impl_session;
mod commands;
mod config;
mod config_validator;
mod config_watcher;
mod dashboard_assets;
mod error;
mod error_envelope;
mod hash_util;
mod health_check;
mod http_red;
mod metrics;
mod monitoring;
mod notifier;
mod otel;
mod paths;
mod pprof_http;
mod proc_compose;
mod progress;
#[cfg(test)]
mod proptest_util;
mod rate_limiter;
mod runtime;
mod serve_auth;
mod serve_lock;
mod serve_rate_limit;
mod shutdown;
mod spawn_policy;
mod subcommands;
mod theme;
mod tray_http;

use subcommands::root::{Cli, Commands};

// Re-exports preserved for any external callers (binaries, integration
// tests) that pulled types from `main` before the decomposition.
pub use subcommands::root::Cli as CliArgs;
pub use subcommands::root::Commands as RootCommands;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        match err.downcast::<SharecliError>() {
            Ok(domain) => {
                domain.eprint();
                std::process::exit(i32::from(domain.exit_code()));
            }
            Err(err) => {
                let domain = SharecliError::from(err);
                domain.eprint();
                std::process::exit(i32::from(domain.exit_code()));
            }
        }
    }
}

async fn run() -> Result<()> {
    use std::io::IsTerminal;

    #[cfg(feature = "dhat-heap")]
    let _dhat_profiler = std::env::var("SHARECLI_DHAT_PROFILE")
        .is_ok_and(|value| value == "1")
        .then(dhat::Profiler::new_heap);

    let cli = Cli::parse();
    let tokens = theme::Tokens::from_name(&cli.theme)
        .ok_or_else(|| {
            SharecliError::user_input(format!(
                "unknown theme '{}': expected backbone-2 / bb2 / dark or backbone-2-light / light",
                cli.theme,
            ))
        })
        .map_err(|e| anyhow::Error::new(e))?;
    if std::io::stderr().is_terminal() && !cli_impl_meta::is_no_color() {
        eprintln!("{}", tokens.panel.ansi_fg());
    }

    let cfg = config::init_global();

    {
        let errors = config_validator::validate_config(cfg);
        if !errors.is_empty() {
            config_validator::report_and_exit(&errors);
        }
    }

    if !cli.quiet && (cli.verbose || std::io::stderr().is_terminal()) {
        use tracing_subscriber::prelude::*;

        crate::otel::ensure_trace_context_propagator();

        let level = if cli.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO };
        let json = std::env::var("SHARECLI_LOG_FORMAT")
            .map(|v| v.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        let filter = tracing_subscriber::filter::LevelFilter::from_level(level);
        let log_path: std::path::PathBuf = std::env::var_os("SHARECLI_LOG_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                home.join(".sharecli").join("logs").join("sharecli.log")
            });
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let log_path_for_writer = log_path.clone();
        let file_make_writer = move || -> Box<dyn std::io::Write + Send> {
            Box::new(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path_for_writer)
                    .expect("reopen log file"),
            )
        };
        if json {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(std::io::stderr)
                .with_filter(filter);
            let file_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(file_make_writer);
            let registry = tracing_subscriber::registry().with(fmt_layer).with(file_layer);
            if let Some(otel_layer) = crate::otel::try_otel_layer() {
                registry.with(otel_layer).init();
            } else {
                registry.init();
            }
        } else {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .with_ansi(!cli_impl_meta::is_no_color())
                .with_writer(std::io::stderr)
                .with_filter(filter);
            let file_layer =
                tracing_subscriber::fmt::layer().with_ansi(false).with_writer(file_make_writer);
            let registry = tracing_subscriber::registry().with(fmt_layer).with(file_layer);
            if let Some(otel_layer) = crate::otel::try_otel_layer() {
                registry.with(otel_layer).init();
            } else {
                registry.init();
            }
        }
        tracing::debug!(path = %log_path.display(), "sharecli log file");
    } else {
        crate::otel::ensure_trace_context_propagator();
    }

    cli_dispatch::dispatch(&cli).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap_complete::Shell;

    #[test]
    fn test_man_page_contains_th_sharecli() {
        let man = clap_mangen::Man::new(Cli::command());
        let mut buf = Vec::new();
        man.render(&mut buf).expect("render man page");
        let output = String::from_utf8(buf).expect("valid utf-8");
        assert!(
            output.contains(".TH \"sharecli\"") || output.contains(".TH sharecli"),
            "man page should declare sharecli(1), got prefix: {}",
            &output[..output.len().min(200)]
        );
    }

    #[test]
    fn test_completions_zsh_contains_compdef() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        clap_complete::generate(Shell::Zsh, &mut cmd, "sharecli", &mut buf);
        let output = String::from_utf8(buf).expect("valid utf-8");
        assert!(
            output.contains("#compdef"),
            "zsh completion should start with #compdef, got: {output}"
        );
    }

    #[test]
    fn test_no_color_respects_env_var() {
        // When NO_COLOR is unset, is_no_color should return false
        unsafe { std::env::remove_var("NO_COLOR") };
        assert!(!cli_impl_meta::is_no_color());

        // When NO_COLOR is set to empty string, should return false
        unsafe { std::env::set_var("NO_COLOR", "") };
        assert!(!cli_impl_meta::is_no_color());

        // When NO_COLOR is set to non-empty, should return true
        unsafe { std::env::set_var("NO_COLOR", "1") };
        assert!(cli_impl_meta::is_no_color());

        // Clean up
        unsafe { std::env::remove_var("NO_COLOR") };
    }
}
