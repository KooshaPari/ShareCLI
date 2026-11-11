//! Live gate / host-watch / pool-status sections printed by operator commands.
//!
//! Each `print_live_*` helper is invoked from one or more of the `ps` / `pool` /
//! `status` / `health` text renderers, and the JSON siblings (`eprint_live_*`,
//! `capture_live_gate_host_watch`, `fetch_operator_pool_status_siblings`) are
//! shared with the `--json` / `--watch --json` code paths (FR-007 / AC-007.42,
//! 007.44, 007.73, 007.77).
//!
//! Keeping these helpers in their own module means each subcommand file stays
//! focused on its own command shape and does not need to repeat the
//! thermal-governor + agent-scan + host-watch sampling boilerplate.

use std::io::Write;

use anyhow::Result;

use crate::commands::pool::build_pool_json;
use crate::commands::ps::build_status_json;
use crate::commands::types::{PoolJson, StatusJson};
use crate::monitoring::HostResourceWatchJson;
use sharecli_fleet::thermal::ThermalGovernor;
use sharecli_fleet::{
    count_host_agents, format_gate_status_section, gate_status_snapshot, ResourceWatchSample,
};

/// Poll live thermal + proc-scan agent inventory and print the FR-011 gate section.
pub(crate) fn print_live_gate_section() -> Result<()> {
    let thermal = ThermalGovernor::new().poll()?;
    let agent_count = count_host_agents();
    print!("{}", format_gate_status_section(thermal, agent_count));
    Ok(())
}

/// Poll live host FD/RSS/load/net watch and print the FR-007 status section.
pub(crate) fn print_live_host_watch_section() -> Result<()> {
    let resource_watch = ResourceWatchSample::capture()?;
    print!("{}", resource_watch.format_status_section());
    Ok(())
}

/// Runtime pool + proc-scan operator lines after gate → host_watch (FR-007 / AC-007.74–007.76).
pub(crate) async fn print_live_pool_status_operator_sections() -> Result<()> {
    use sharecli_fleet::{format_pool_operator_line, format_status_operator_line};

    let (pool_json, status_json) = tokio::join!(build_pool_json(), build_status_json());
    let pool_panel: sharecli_fleet::PoolOperatorPanel = pool_json?.into();
    let status_panel: sharecli_fleet::StatusOperatorPanel = status_json?.into();
    println!();
    println!("{}", format_pool_operator_line(&pool_panel));
    println!("{}", format_status_operator_line(&status_panel));
    Ok(())
}

/// Gate + host watch text companions on stderr for NDJSON watch modes (AC-007.28 / AC-007.42).
pub(crate) fn eprint_live_gate_host_watch_sections() -> Result<()> {
    let thermal = ThermalGovernor::new().poll()?;
    let agent_count = count_host_agents();
    eprint!("{}", format_gate_status_section(thermal, agent_count));
    eprint!("{}", HostResourceWatchJson::capture()?.format_text_section());
    let _ = std::io::stderr().flush();
    Ok(())
}

/// Live thermal gate + host resource watch for JSON envelopes (FR-007 / AC-007.44).
pub(crate) fn capture_live_gate_host_watch(
) -> Result<(sharecli_fleet::GateStatusSnapshot, HostResourceWatchJson)> {
    let thermal = ThermalGovernor::new().poll()?;
    let gate = gate_status_snapshot(thermal, count_host_agents());
    let host_watch = HostResourceWatchJson::capture()?;
    Ok((gate, host_watch))
}

/// Fetch independent pool + status JSON panels for operator sibling embedding (AC-007.77).
pub(crate) async fn fetch_operator_pool_status_siblings() -> Result<(PoolJson, StatusJson)> {
    let (pool, status) = tokio::join!(build_pool_json(), build_status_json());
    Ok((pool?, status?))
}
