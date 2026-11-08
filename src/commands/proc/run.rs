// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli proc` CLI entry point and dispatch (FR-006).
//!
//! - [`render_pid_detail_once`] / [`render_pid_detail`] drive the `--pid N`
//!   one-shot and watch detail surfaces (AC-006.23 / AC-007.86 / AC-007.87).
//! - [`render_once`] is the top-level inventory dispatcher: it selects between
//!   `--tree` and flat modes, applies [`crate::commands::proc::filter::ProcFilter`]
//!   / [`crate::commands::proc::sort::ProcSort`] via
//!   [`crate::commands::proc::scan`], and emits text / CSV / JSON / NDJSON
//!   (AC-006.11–18, AC-006.24–26, AC-006.32–37, AC-007.13–21, AC-007.28–35,
//!   AC-007.75, AC-007.77, AC-007.86–88, AC-007.91).
//! - [`reject_pid_inventory_combos`] validates that `--pid` is not combined
//!   with any inventory filter (AC-007.92).
//! - [`run`] is the async CLI entry point.

use std::io::Write;
use std::time::Duration;

use anyhow::{bail, Result};
use sharecli_fleet::{
    build_forest_state_map, build_host_agent_forests, format_gate_status_section,
    gate_status_snapshot, scan_host_agents, watch_detected_agents, HostProcSource, ThermalGovernor,
};
use tokio::time::sleep;

use super::build::{
    agent_row_from_watch, agent_tree_node_to_json, build_agent_cmdline_map, build_agent_ppid_map,
    build_proc_detail,
};
use super::filter::ProcFilter;
use super::ndjson::{
    append_proc_csv_companions, emit_ndjson_line, eprint_gate_host_watch_stderr_companions,
    print_host_watch_text_footer, unix_ts_secs, PROC_CSV_WATCH_FRAME_MARKER,
    PROC_PID_CSV_WATCH_FRAME_MARKER,
};
use super::render::{
    render_agent_inventory, render_agent_inventory_csv, render_agent_tree, render_agent_tree_csv,
    render_proc_detail_csv, render_proc_detail_text,
};
use super::scan::{
    apply_sort_forests, apply_sort_watched, fd_map_from_watched, filter_agent_forests,
    filter_watched_agents, rss_map_from_watched, sort_agent_forests,
};
use super::sort::{limit_agent_forests, limit_watched_agents, parse_proc_limit, ProcSort};
use super::types::{
    AgentProcNdjsonLine, AgentProcSnapshot, AgentTreeNdjsonLine, AgentTreeSnapshot,
    ProcDetailNdjsonLine,
};

use crate::monitoring::HostResourceWatchJson;

pub(crate) async fn render_pid_detail_once(pid: u32, json: bool, ndjson: bool) -> Result<()> {
    let scanned_agents = scan_host_agents();
    let thermal = ThermalGovernor::new().poll()?;
    let mut detail = build_proc_detail(&HostProcSource, pid)?;
    if json {
        let (pool_panel, status_panel) =
            super::super::fetch_operator_pool_status_siblings().await?;
        detail.pool = Some(pool_panel);
        detail.status = Some(status_panel);
        if ndjson {
            let line = ProcDetailNdjsonLine { ts: unix_ts_secs(), snapshot: detail };
            emit_ndjson_line(&line)?;
            eprint_gate_host_watch_stderr_companions(thermal, scanned_agents.len())?;
            return Ok(());
        }
        println!("{}", serde_json::to_string_pretty(&detail)?);
        return Ok(());
    }
    render_proc_detail_text(&detail)?;
    // AC-007.75 / AC-007.86 / AC-007.87: gate → host_watch → pool → proc-scan on stdout.
    super::super::print_live_pool_status_operator_sections().await?;
    Ok(())
}

pub(crate) async fn render_pid_detail(pid: u32, json: bool, csv: bool) -> Result<()> {
    if csv {
        let detail = build_proc_detail(&HostProcSource, pid)?;
        let body = render_proc_detail_csv(&detail);
        print!("{}", append_proc_csv_companions(body, &detail.gate).await?);
        return Ok(());
    }
    render_pid_detail_once(pid, json, false).await
}

/// Render one host agent inventory snapshot (text or JSON).
pub async fn render_once(
    json: bool,
    csv: bool,
    tree: bool,
    filter: &ProcFilter,
    ndjson: bool,
    sort: Option<ProcSort>,
    limit: Option<usize>,
) -> Result<()> {
    let scanned_agents = scan_host_agents();
    let thermal = ThermalGovernor::new().poll()?;
    let gate = gate_status_snapshot(thermal, scanned_agents.len());
    let watched_all = watch_detected_agents(&scanned_agents);
    let rss_by_pid = rss_map_from_watched(&watched_all);
    let fd_by_pid = fd_map_from_watched(&watched_all);
    let agent_pids: Vec<u32> = scanned_agents.iter().map(|a| a.pid).collect();
    let ppid_by_pid = build_agent_ppid_map(&HostProcSource, &agent_pids);
    let cmdline_by_pid = build_agent_cmdline_map(&HostProcSource, &agent_pids);
    let state_by_pid = sharecli_fleet::build_agent_state_map(&HostProcSource, &agent_pids);

    if tree {
        let forests = filter_agent_forests(
            &build_host_agent_forests(),
            filter,
            &rss_by_pid,
            &fd_by_pid,
            &cmdline_by_pid,
            &state_by_pid,
        );
        let forests = apply_sort_forests(forests, sort, &rss_by_pid, &fd_by_pid, &state_by_pid);
        let forests = limit_agent_forests(forests, limit);
        let tree_state_by_pid = build_forest_state_map(&HostProcSource, &forests);
        // One-shot `proc --tree --csv` MUST NOT print gate/host_watch stderr companions (AC-007.33).
        if csv {
            print!(
                "{}",
                append_proc_csv_companions(
                    render_agent_tree_csv(&forests, &rss_by_pid, &fd_by_pid, &tree_state_by_pid,),
                    &gate,
                )
                .await?
            );
            return Ok(());
        }
        let snap = AgentTreeSnapshot {
            forests: forests
                .iter()
                .map(|root| agent_tree_node_to_json(root, &tree_state_by_pid))
                .collect(),
            roots: forests.len(),
            gate,
            host_watch: HostResourceWatchJson::capture()?,
            pool: None,
            status: None,
        };
        if json {
            let (pool_panel, status_panel) =
                super::super::fetch_operator_pool_status_siblings().await?;
            let snap =
                AgentTreeSnapshot { pool: Some(pool_panel), status: Some(status_panel), ..snap };
            if ndjson {
                let line = AgentTreeNdjsonLine { ts: unix_ts_secs(), snapshot: snap };
                emit_ndjson_line(&line)?;
                eprint_gate_host_watch_stderr_companions(thermal, scanned_agents.len())?;
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&snap)?);
            return Ok(());
        }
        // One-shot `proc --tree` text MUST NOT print gate/host_watch stderr companions (AC-007.34).
        render_agent_tree(&forests, &tree_state_by_pid);
        print!("{}", format_gate_status_section(thermal, scanned_agents.len()));
        print_host_watch_text_footer()?;
        // AC-007.34 / AC-007.75: gate → host_watch → pool → proc-scan on stdout after tree body.
        super::super::print_live_pool_status_operator_sections().await?;
        return Ok(());
    }

    let watched = limit_watched_agents(
        apply_sort_watched(
            filter_watched_agents(
                &watched_all,
                filter,
                &ppid_by_pid,
                &cmdline_by_pid,
                &state_by_pid,
            ),
            sort,
            &state_by_pid,
        ),
        limit,
    );
    // One-shot `proc --csv` MUST NOT print gate/host_watch stderr companions (AC-007.33).
    if csv {
        print!(
            "{}",
            append_proc_csv_companions(render_agent_inventory_csv(&watched, &state_by_pid), &gate,)
                .await?
        );
        return Ok(());
    }
    if json {
        let (pool_panel, status_panel) =
            super::super::fetch_operator_pool_status_siblings().await?;
        let snap = AgentProcSnapshot {
            agents: watched.iter().map(|row| agent_row_from_watch(row, &state_by_pid)).collect(),
            scanned: scanned_agents.len(),
            watched: watched.len(),
            gate,
            host_watch: HostResourceWatchJson::capture()?,
            pool: Some(pool_panel),
            status: Some(status_panel),
        };
        if ndjson {
            let line = AgentProcNdjsonLine { ts: unix_ts_secs(), snapshot: snap };
            emit_ndjson_line(&line)?;
            eprint_gate_host_watch_stderr_companions(thermal, scanned_agents.len())?;
            return Ok(());
        }
        println!("{}", serde_json::to_string_pretty(&snap)?);
        return Ok(());
    }
    // One-shot `proc` text MUST NOT print gate/host_watch stderr companions (AC-007.34).
    render_agent_inventory(&watched, scanned_agents.len(), &state_by_pid);
    print!("{}", format_gate_status_section(thermal, scanned_agents.len()));
    print_host_watch_text_footer()?;
    // AC-007.34 / AC-007.75: gate → host_watch → pool → proc-scan on stdout after inventory body.
    super::super::print_live_pool_status_operator_sections().await?;
    Ok(())
}

/// `sharecli proc` — list host-detected agents with live RSS/FD samples.
/// Reject inventory-mode flags when `--pid` selects detail mode (AC-007.92).
// CLI flag aggregation: one Option per inventory filter keeps the dispatch
// layer mechanical; the wide signature is intentional.
#[allow(clippy::too_many_arguments)]
fn reject_pid_inventory_combos(
    tree: bool,
    family: &Option<String>,
    exclude_family: &Option<String>,
    comm: &Option<String>,
    cmdline: &Option<String>,
    state: &Option<String>,
    min_rss: &Option<String>,
    max_rss: &Option<String>,
    min_fd: &Option<String>,
    max_fd: &Option<String>,
    sort: &Option<String>,
    limit: Option<u64>,
) -> Result<()> {
    let mut conflicts: Vec<&str> = Vec::new();
    if tree {
        conflicts.push("--tree");
    }
    if family.is_some() {
        conflicts.push("--family");
    }
    if exclude_family.is_some() {
        conflicts.push("--exclude-family");
    }
    if comm.is_some() {
        conflicts.push("--comm");
    }
    if cmdline.is_some() {
        conflicts.push("--cmdline");
    }
    if state.is_some() {
        conflicts.push("--state");
    }
    if min_rss.is_some() {
        conflicts.push("--min-rss");
    }
    if max_rss.is_some() {
        conflicts.push("--max-rss");
    }
    if min_fd.is_some() {
        conflicts.push("--min-fd");
    }
    if max_fd.is_some() {
        conflicts.push("--max-fd");
    }
    if sort.is_some() {
        conflicts.push("--sort");
    }
    if limit.is_some() {
        conflicts.push("--limit");
    }
    if conflicts.is_empty() {
        return Ok(());
    }
    bail!(
        "--pid cannot be combined with {} (AC-007.92); use inventory mode without --pid",
        conflicts.join(", ")
    )
}

// CLI entry point: all `proc` flags are threaded through as individual
// arguments by the shared dispatch layer; the wide signature is intentional.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    json: bool,
    csv: bool,
    tree: bool,
    watch: Option<u64>,
    family: Option<String>,
    exclude_family: Option<String>,
    comm: Option<String>,
    cmdline: Option<String>,
    state: Option<String>,
    min_rss: Option<String>,
    max_rss: Option<String>,
    min_fd: Option<String>,
    max_fd: Option<String>,
    sort: Option<String>,
    limit: Option<u64>,
    pid: Option<u32>,
    ppid: Option<u32>,
) -> Result<()> {
    if csv && json {
        bail!("--csv cannot be combined with --json");
    }
    if pid.is_some() && ppid.is_some() {
        bail!("--ppid cannot be combined with --pid");
    }
    if pid.is_some() {
        // AC-007.92: --pid is detail mode; inventory filters/tree MUST fail loudly.
        reject_pid_inventory_combos(
            tree,
            &family,
            &exclude_family,
            &comm,
            &cmdline,
            &state,
            &min_rss,
            &max_rss,
            &min_fd,
            &max_fd,
            &sort,
            limit,
        )?;
    }
    if let Some(target_pid) = pid {
        if let Some(interval_secs) = watch {
            if interval_secs == 0 {
                bail!("--watch interval must be >= 1 second");
            }
            let ndjson = json;
            let csv_watch = csv;
            let period = Duration::from_secs(interval_secs);
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    // AC-007.91: frame marker + full PID CSV body + `# [watch]` on stdout.
                    println!("{PROC_PID_CSV_WATCH_FRAME_MARKER}");
                    render_pid_detail(target_pid, false, true).await?;
                } else {
                    render_pid_detail_once(target_pid, json, ndjson).await?;
                }
                if !ndjson {
                    std::io::stdout().flush()?;
                }
                if ndjson {
                    let footer = format!(
                        "\n[watch] Refreshing every {interval_secs}s — press Ctrl-C to stop."
                    );
                    eprint!("{footer}");
                    let _ = std::io::stderr().flush();
                } else if csv_watch {
                    println!("# [watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                    // AC-007.94: flush so `# [watch]` reaches pipe consumers this tick.
                    std::io::stdout().flush()?;
                } else {
                    println!("\n[watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                    // AC-007.96: flush text `[watch]` footer same tick (parity with CSV).
                    std::io::stdout().flush()?;
                }
                let idle = period.saturating_sub(cycle_start.elapsed());
                tokio::select! {
                    _ = sleep(idle) => {},
                    _ = tokio::signal::ctrl_c() => {
                        if ndjson {
                            eprintln!("\nExiting watch mode.");
                        } else {
                            println!("\nExiting watch mode.");
                        }
                        break;
                    },
                }
            }
            return Ok(());
        }
        return render_pid_detail(target_pid, json, csv).await;
    }
    let filter = ProcFilter::from_cli(
        family,
        exclude_family,
        comm,
        cmdline,
        state,
        min_rss,
        max_rss,
        min_fd,
        max_fd,
        ppid,
    )?;
    let sort_key = ProcSort::from_cli(sort.as_deref())?;
    let row_limit = parse_proc_limit(limit)?;
    match watch {
        None => render_once(json, csv, tree, &filter, false, sort_key, row_limit).await,
        Some(interval_secs) => {
            if interval_secs == 0 {
                bail!("--watch interval must be >= 1 second");
            }
            let ndjson = json;
            let csv_watch = csv;
            let period = Duration::from_secs(interval_secs);
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    println!("{PROC_CSV_WATCH_FRAME_MARKER}");
                }
                render_once(json, csv, tree, &filter, ndjson, sort_key, row_limit).await?;
                if !ndjson {
                    std::io::stdout().flush()?;
                }
                // Text watch: gate/host_watch + `[watch]` footer on stdout only (AC-007.35).
                // CSV watch: frame marker + full CSV body + `# [watch]` comment on stdout (AC-007.88).
                // NDJSON watch: companions + footer on stderr (AC-007.28 / AC-007.29).
                let footer =
                    format!("\n[watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                if ndjson {
                    eprint!("{footer}");
                    let _ = std::io::stderr().flush();
                } else if csv_watch {
                    println!("# [watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                    // AC-007.94: flush so `# [watch]` reaches pipe consumers this tick.
                    std::io::stdout().flush()?;
                } else {
                    println!("{footer}");
                    // AC-007.96: flush text `[watch]` footer same tick (parity with CSV).
                    std::io::stdout().flush()?;
                }
                let idle = period.saturating_sub(cycle_start.elapsed());
                tokio::select! {
                    _ = sleep(idle) => {},
                    _ = tokio::signal::ctrl_c() => {
                        if ndjson {
                            eprintln!("\nExiting watch mode.");
                        } else {
                            println!("\nExiting watch mode.");
                        }
                        break;
                    },
                }
            }
            Ok(())
        }
    }
}
