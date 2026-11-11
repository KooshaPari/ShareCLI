//! NDJSON watch helpers and CSV/stderr companion emission for `sharecli proc` (FR-006 / FR-007).
//!
//! - [`unix_ts_secs`] / [`emit_ndjson_line`] are the per-tick timestamp and
//!   single-line JSON emitter used by every `--watch --json` code path
//!   (AC-006.18 / AC-006.37 / AC-007.28 / AC-007.29 / AC-007.87).
//! - [`print_host_watch_text_footer`] prints the host-resource watch text
//!   section after the inventory body on text `proc` (AC-007.20).
//! - [`eprint_gate_host_watch_stderr_companions`] emits the NDJSON gate +
//!   host_watch companions on stderr (AC-007.28 / AC-007.29). One-shot
//!   `--json` / `--csv` / text paths MUST NOT call this helper
//!   (AC-007.30 / AC-007.31 / AC-007.33 / AC-007.34 / AC-007.35).
//! - [`append_gate_csv_companion`] / [`append_host_watch_csv_companion`] /
//!   [`append_proc_csv_companions`] append the gate / host_watch / pool /
//!   proc-scan panels as CSV companion rows for `--csv` modes
//!   (AC-007.19 / AC-007.86 / AC-007.88 / AC-007.91).

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;
use sharecli_fleet::{format_gate_status_section, ThermalLevel};

use crate::monitoring::HostResourceWatchJson;

/// CSV `#` comment line separating each `--csv --watch` refresh frame (AC-007.88).
pub const PROC_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-proc-watch-frame";
/// CSV `#` comment line separating each `proc --pid --csv --watch` refresh frame (AC-007.91).
pub const PROC_PID_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-proc-pid-watch-frame";

pub(crate) fn unix_ts_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Emit one compact JSON line and flush (piped stdout is block-buffered; AC-006.18).
pub(crate) fn emit_ndjson_line<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    std::io::stdout().flush()?;
    Ok(())
}

pub(crate) fn print_host_watch_text_footer() -> Result<()> {
    print!("{}", HostResourceWatchJson::capture()?.format_text_section());
    Ok(())
}

/// Gate + host watch text companions on stderr for NDJSON watch only (AC-007.28 / AC-007.29).
/// One-shot `proc --json`, `proc --tree --json`, and `proc --pid N --json` MUST NOT call
/// this helper (AC-007.30 / AC-007.31). One-shot `proc --csv` and `proc --tree --csv` MUST NOT
/// call this helper either (AC-007.33); gate/host_watch stay in CSV companion rows on stdout
/// only (AC-007.19). One-shot text `proc`, `proc --tree`, and `proc --pid N` MUST NOT call
/// this helper either (AC-007.34); gate/host_watch stay in text sections on stdout only
/// (AC-007.17 / AC-007.20 / AC-007.21). Text watch `proc --watch` and `proc --tree --watch`
/// (no `--json`) MUST NOT call this helper either (AC-007.35); gate/host_watch and `[watch]`
/// footer stay on stdout only (inverse of AC-007.28 / AC-007.29).
pub(crate) fn eprint_gate_host_watch_stderr_companions(
    thermal: ThermalLevel,
    agent_count: usize,
) -> Result<()> {
    eprint!("{}", format_gate_status_section(thermal, agent_count));
    eprint!("{}", HostResourceWatchJson::capture()?.format_text_section());
    let _ = std::io::stderr().flush();
    Ok(())
}

pub(crate) fn append_gate_csv_companion(
    csv: String,
    gate: &sharecli_fleet::GateStatusSnapshot,
) -> String {
    let mut out = csv;
    out.push_str(&gate.format_csv_companion());
    out
}

pub(crate) fn append_host_watch_csv_companion(csv: String) -> Result<String> {
    let mut out = csv;
    out.push_str(&HostResourceWatchJson::capture()?.format_csv_companion());
    Ok(out)
}

pub(crate) async fn append_proc_csv_companions(
    csv: String,
    gate: &sharecli_fleet::GateStatusSnapshot,
) -> Result<String> {
    let csv = append_host_watch_csv_companion(append_gate_csv_companion(csv, gate))?;
    let (pool_json, status_json) = super::super::fetch_operator_pool_status_siblings().await?;
    let pool: sharecli_fleet::PoolOperatorPanel = pool_json.into();
    let status: sharecli_fleet::StatusOperatorPanel = status_json.into();
    let mut out = csv;
    out.push_str(&pool.format_csv_companion());
    out.push_str(&status.format_csv_companion());
    Ok(out)
}
