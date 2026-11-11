//! JSON envelope structs for `ps` / `pool` / `status` / `health` operator commands.
//!
//! These types are emitted by the matching commands' `--json` and `--watch --json`
//! modes (FR-007 / AC-007.43, 007.44, 007.49, 007.64–66, 007.77, 007.83) and are
//! re-exported at `commands::` so external test files (`tests/fr00X_*.rs`) keep
//! resolving `sharecli::commands::PsAllJson` / `PoolJson` / `StatusJson` /
//! `HealthJson` / `*NdjsonLine` without an extra module path.
//!
//! Companion helpers (`ps_unix_ts_secs`, `emit_ps_ndjson_line`,
//! `sharecli_log_location`) and the CSV watch markers + footer emitters live
//! here too — they are pure helpers with no command-specific business logic
//! and are only consumed by the `ps` / `pool` / `status` / `health` command
//! families, so colocating them keeps the four subcommands slim.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;

use crate::commands::proc;
use crate::monitoring::HostResourceWatchJson;

/// One managed pool row for `sharecli ps --all --json` (FR-007 / AC-007.43).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PsManagedProcessRow {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    pub agent: String,
}

/// JSON envelope for `sharecli ps --all --json` (FR-007 / AC-007.43, pool/status AC-007.77).
///
/// Managed pool fields precede host agent inventory; live `gate`, `host_watch`, `pool`, and
/// `status` siblings follow (parity with `report --format json` AC-007.73 key order).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PsAllJson {
    pub processes: Vec<PsManagedProcessRow>,
    pub total_memory_mb: u64,
    pub agents: Vec<proc::AgentProcRow>,
    pub scanned: usize,
    pub watched: usize,
    pub gate: sharecli_fleet::GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool operator panel (FR-007 / AC-007.77).
    pub pool: PoolJson,
    /// Proc-scan status operator panel (FR-007 / AC-007.77).
    pub status: StatusJson,
}

/// One NDJSON watch line for `ps --all --watch --json` (FR-007 / AC-007.49).
#[derive(Debug, Clone, Serialize)]
pub struct PsAllNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: PsAllJson,
}

/// One NDJSON watch line for `health --watch --json` (FR-007 / AC-007.64).
#[derive(Debug, Clone, Serialize)]
pub struct HealthNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: HealthJson,
}

/// One NDJSON watch line for `pool --watch --json` (FR-007 / AC-007.65).
#[derive(Debug, Clone, Serialize)]
pub struct PoolNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: PoolJson,
}

/// One NDJSON watch line for `status --watch --json` (FR-007 / AC-007.66).
#[derive(Debug, Clone, Serialize)]
pub struct StatusNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: StatusJson,
}

/// JSON envelope for `sharecli pool --json` (FR-007 / AC-007.44, status sibling AC-007.77).
///
/// Pool status fields precede live `gate` and `host_watch` siblings; `pool --json` adds a
/// nested `status` sibling after `host_watch` (no redundant nested `pool` — top-level fields
/// already are the pool panel).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PoolJson {
    pub node_total: usize,
    pub node_idle: usize,
    pub bun_total: usize,
    pub bun_idle: usize,
    pub max_per_type: usize,
    pub healthy: bool,
    pub issues: Vec<String>,
    pub gate: sharecli_fleet::GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    /// Proc-scan status sibling; only emitted by `pool --json` (AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Box<StatusJson>>,
}

/// JSON envelope for `sharecli health --json` (FR-007 / AC-007.44, pool/status AC-007.77).
///
/// Runtime health fields precede live `gate`, `host_watch`, `pool`, and `status` siblings
/// (parity with `report --format json` AC-007.73 key order).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HealthJson {
    pub healthy: bool,
    pub issues: Vec<String>,
    pub node_total: usize,
    pub node_idle: usize,
    pub node_in_use: usize,
    pub bun_total: usize,
    pub bun_idle: usize,
    pub bun_in_use: usize,
    pub max_per_type: usize,
    pub gate: sharecli_fleet::GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool operator panel (FR-007 / AC-007.77).
    pub pool: PoolJson,
    /// Proc-scan status operator panel (FR-007 / AC-007.77).
    pub status: StatusJson,
}

/// JSON envelope for `sharecli status --json` (FR-007 / AC-007.25, pool sibling AC-007.77).
///
/// Proc-scan fields precede live `gate` and `host_watch`; `status --json` adds a nested `pool`
/// sibling after `host_watch` (no redundant nested `status` — top-level fields already are the
/// proc-scan panel).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusJson {
    pub total_processes: usize,
    pub agents: Vec<proc::AgentProcRow>,
    pub scanned: usize,
    pub watched: usize,
    pub gate: sharecli_fleet::GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool sibling; only emitted by `status --json` (AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<Box<PoolJson>>,
    /// Filesystem path to the sharecli log file (PR 8 of dashboard expansion).
    /// Swift tray reads this directly and tails the file with `tail -F`
    /// semantics — no separate log.tail IPC needed. None when the file-rotation
    /// subscriber hasn't been installed (e.g., quiet mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_location: Option<std::path::PathBuf>,
}

impl From<PoolJson> for sharecli_fleet::PoolOperatorPanel {
    fn from(p: PoolJson) -> Self {
        Self {
            node_total: p.node_total,
            node_idle: p.node_idle,
            bun_total: p.bun_total,
            bun_idle: p.bun_idle,
            max_per_type: p.max_per_type,
            healthy: p.healthy,
        }
    }
}

impl From<StatusJson> for sharecli_fleet::StatusOperatorPanel {
    fn from(s: StatusJson) -> Self {
        Self {
            scanned: s.scanned,
            watched: s.watched,
            total_processes: s.total_processes,
            agent_rows: s.agents.len(),
        }
    }
}

/// Current wall clock as unix seconds (FR-007 / AC-007.49 / 007.64–66).
pub(crate) fn ps_unix_ts_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Emit one compact JSON line and flush (piped stdout is block-buffered; AC-007.49).
pub(crate) fn emit_ps_ndjson_line<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    std::io::stdout().flush()?;
    Ok(())
}

/// Returns the filesystem path to the current sharecli log file, if any.
/// Read from the SHARECLI_LOG_PATH env var (with the same HOME fallback the
/// tracing-appender setup uses in main.rs). Resolved at status-snapshot time so
/// test isolation via env var is observed.
pub(crate) fn sharecli_log_location() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("SHARECLI_LOG_PATH") {
        return Some(std::path::PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)?;
    Some(home.join(".sharecli").join("logs").join("sharecli.log"))
}

/// CSV `#` comment line separating each operator `--csv --watch` refresh frame (AC-007.89).
pub const HEALTH_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-health-watch-frame";
pub const POOL_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-pool-watch-frame";
pub const STATUS_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-status-watch-frame";
pub const PS_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-ps-watch-frame";

/// Emit the CSV watch frame delimiter before a tick's body (AC-007.89).
pub(crate) fn emit_operator_csv_watch_frame(marker: &str) {
    println!("{marker}");
}

/// Emit the CSV watch footer as a `#` comment on stdout (AC-007.89).
/// Flush so pipe consumers see `# [watch]` in the same tick (AC-007.94).
pub(crate) fn emit_operator_csv_watch_footer(interval_secs: u64) {
    println!("# [watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
    let _ = std::io::stdout().flush();
}

/// Print watch-mode footer for text, NDJSON, or CSV refresh loops (AC-007.64–66 / AC-007.89).
pub(crate) fn emit_operator_watch_footer(interval_secs: u64, ndjson: bool, csv_watch: bool) {
    let footer = format!("\n[watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
    if ndjson {
        eprint!("{footer}");
        let _ = std::io::stderr().flush();
    } else if csv_watch {
        emit_operator_csv_watch_footer(interval_secs);
    } else {
        println!("{footer}");
        let _ = std::io::stdout().flush();
    }
}
