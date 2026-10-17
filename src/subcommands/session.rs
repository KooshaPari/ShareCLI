//! `Session` subcommand enum (durable agent session ledger).

use clap::Subcommand;
use sharecli_session::DEFAULT_RECOVERY_MAX_AGE_SECONDS;

#[derive(Subcommand, Debug)]
pub enum SessionCmd {
    /// List persisted sessions
    List {
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Inspect one persisted session
    Inspect {
        id: String,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Print the pending recovery plan
    RecoveryPlan {
        /// Maximum observation age in seconds (default: four hours)
        #[arg(long, default_value_t = DEFAULT_RECOVERY_MAX_AGE_SECONDS, value_parser = clap::value_parser!(u64).range(1..=604800))]
        max_age_seconds: u64,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Append one JSON observation to the durable ledger
    Observe {
        /// JSON file containing a SessionObservation
        input: std::path::PathBuf,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// List append-only observations, optionally for one terminal surface
    Observations {
        #[arg(long)]
        surface_id: Option<String>,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Compact the observation WAL while retaining the newest record per surface
    Compact {
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Recover verified sessions; defaults to a dry run
    Recover {
        /// Launch exact/corroborated recipes instead of printing a dry run
        #[arg(long)]
        execute: bool,
        /// Maximum number of concurrent launches
        #[arg(long, default_value_t = 4)]
        max_parallel: usize,
        /// Maximum observation age in seconds (default: four hours)
        #[arg(long, default_value_t = DEFAULT_RECOVERY_MAX_AGE_SECONDS, value_parser = clap::value_parser!(u64).range(1..=604800))]
        max_age_seconds: u64,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// List durable terminal layout snapshots
    LayoutList {
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Inspect one durable terminal layout snapshot
    LayoutInspect {
        id: String,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Persist a validated terminal layout snapshot from JSON
    LayoutSave {
        /// JSON file containing a LayoutSnapshot
        input: std::path::PathBuf,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
    /// Append an exact launch-time harness mapping to the session sidecar
    Register {
        #[arg(long, env = "GHOSTTY_SURFACE_ID")]
        surface_id: Option<String>,
        #[arg(long)]
        harness: String,
        #[arg(long)]
        session_id: String,
        /// PID of the harness process, when the launcher can provide it
        #[arg(long)]
        pid: Option<u32>,
        #[arg(long, env = "SHARECLI_SESSION_SIDECAR")]
        state_sidecar: Option<std::path::PathBuf>,
    },
    /// Continuously snapshot Ghostty surfaces into the durable ledger
    Watch {
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=3600))]
        interval_seconds: u64,
        /// Capture one inventory and exit instead of sleeping
        #[arg(long)]
        once: bool,
        #[arg(long, env = "SHARECLI_GHOSTTY_SOCKET", default_value = "/tmp/sharecli-ghostty.sock")]
        socket: std::path::PathBuf,
        #[arg(long, env = "SHARECLI_GHOSTTY_TOKEN")]
        token: Option<String>,
        #[arg(long, env = "SHARECLI_SESSION_SIDECAR")]
        state_sidecar: Option<std::path::PathBuf>,
        #[arg(long)]
        db: Option<std::path::PathBuf>,
    },
}
