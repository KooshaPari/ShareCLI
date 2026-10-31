// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli ps` and `sharecli status` operator commands (FR-007 / AC-007.43, 007.49, 007.25, 007.66).
//!
//! `ps` lists managed pool processes (with optional host-agent inventory under `--all`),
//! filtering by project / harness, and emitting text / JSON / CSV / NDJSON output with
//! optional `--watch` refresh. `status` is the proc-scan status panel with the same
//! output mode and live gate / host-watch companions.
//!
//! Both commands share the same `build_*_json` envelope builders and the
//! `render_*_once` text / JSON / CSV branches; keeping them in one file lets us
//! avoid duplicating the watch-loop scaffolding (CSV frame marker, stdout flush,
//! `tokio::select!` ctrl-c sleep) and the shared `ProcessPool::find` /
//! `AgentProcSnapshot::capture` / `fetch_operator_pool_status_siblings` flow.

use std::io::Write;
use std::time::Duration;

use anyhow::Result;

use sharecli_fleet::global_coalesce_meters;
use sharecli_fleet::global_slot_queue_meters;
use sharecli_fleet::thermal::ThermalGovernor;
use sharecli_fleet::{agent_label_for_pid, format_gate_status_section, HostProcSource};
use sharecli_fuse::{
    global_neg_dentry_meters, global_read_cache_meters, global_write_serialize_meters,
};
use sharecli_mesh::capture_maildir_status;

use crate::commands::live_sections::{
    capture_live_gate_host_watch, eprint_live_gate_host_watch_sections, print_live_gate_section,
    print_live_host_watch_section, print_live_pool_status_operator_sections,
};
use crate::commands::proc;
use crate::commands::render_csv;
use crate::commands::types::{
    emit_operator_csv_watch_frame, emit_operator_watch_footer, emit_ps_ndjson_line,
    ps_unix_ts_secs, sharecli_log_location, PoolJson, PsAllJson, PsAllNdjsonLine,
    PsManagedProcessRow, StatusJson, StatusNdjsonLine, PS_CSV_WATCH_FRAME_MARKER,
    STATUS_CSV_WATCH_FRAME_MARKER,
};
use crate::runtime::{ProcessFilter, ProcessInfo, ProcessPool};

/// Build the `ps --all --json` payload (FR-007 / AC-007.43 / 007.77).
pub(crate) async fn build_ps_all_json(
    project: Option<&str>,
    harness: Option<&str>,
) -> Result<PsAllJson> {
    let pool = ProcessPool::new();
    let filter = if let Some(p) = project {
        ProcessFilter::ByProject(p.to_string())
    } else if let Some(h) = harness {
        ProcessFilter::ByHarness(h.to_string())
    } else {
        ProcessFilter::All
    };

    let processes: Vec<ProcessInfo> = pool.find(filter).await;
    let proc_source = HostProcSource;
    let total_mem: u64 = processes.iter().map(|p| p.memory_mb).sum();
    let managed: Vec<PsManagedProcessRow> = processes
        .iter()
        .map(|proc| PsManagedProcessRow {
            pid: proc.pid,
            name: proc.name.clone(),
            memory_mb: proc.memory_mb,
            project: proc.project.clone(),
            harness: proc.harness.clone(),
            agent: agent_label_for_pid(&proc_source, proc.pid).to_string(),
        })
        .collect();
    let snapshot = proc::AgentProcSnapshot::capture()?;
    let (pool_panel, status_panel) =
        crate::commands::live_sections::fetch_operator_pool_status_siblings().await?;
    Ok(PsAllJson {
        processes: managed,
        total_memory_mb: total_mem,
        agents: snapshot.agents,
        scanned: snapshot.scanned,
        watched: snapshot.watched,
        gate: snapshot.gate,
        host_watch: snapshot.host_watch,
        pool: pool_panel,
        status: status_panel,
    })
}

/// Render the `ps` text table (FR-007 / AC-007.43 with `--all`, AC-007.76 text path).
fn print_ps_text_table(
    processes: &[ProcessInfo],
    proc_source: &HostProcSource,
    project: Option<&str>,
    harness: Option<&str>,
    all: bool,
) -> Result<()> {
    println!(
        "{:<8} {:<20} {:<12} {:<15} {:<14} HARNESS",
        "PID", "NAME", "MEM(MB)", "PROJECT", "AGENT"
    );
    println!("{}", "-".repeat(84));

    for proc in processes {
        let project = proc.project.as_deref().unwrap_or("-");
        let harness = proc.harness.as_deref().unwrap_or("-");
        let agent = agent_label_for_pid(proc_source, proc.pid);
        println!(
            "{:<8} {:<20} {:<12.1} {:<15} {:<14} {}",
            proc.pid, proc.name, proc.memory_mb as f64, project, agent, harness
        );
    }

    let total_mem: u64 = processes.iter().map(|p| p.memory_mb).sum();
    println!("\nTotal: {} processes, {} MB memory", processes.len(), total_mem);

    if all {
        print_host_agent_scan(proc_source)?;
    }

    if processes.is_empty() {
        print_ps_empty_hint(project, harness);
    }

    Ok(())
}

async fn render_ps_once(
    project: Option<&str>,
    harness: Option<&str>,
    all: bool,
    json: bool,
    csv: bool,
    ndjson: bool,
) -> Result<()> {
    if json {
        let payload = build_ps_all_json(project, harness).await?;
        if ndjson {
            let line = PsAllNdjsonLine { ts: ps_unix_ts_secs(), snapshot: payload };
            emit_ps_ndjson_line(&line)?;
            eprint_live_gate_host_watch_sections()?;
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        return Ok(());
    }

    if csv {
        let payload = build_ps_all_json(project, harness).await?;
        let pool = ProcessPool::new();
        let filter = if let Some(p) = project {
            ProcessFilter::ByProject(p.to_string())
        } else if let Some(h) = harness {
            ProcessFilter::ByHarness(h.to_string())
        } else {
            ProcessFilter::All
        };
        let processes: Vec<ProcessInfo> = pool.find(filter).await;
        let proc_source = HostProcSource;
        let body = render_csv::render_ps_all_csv_body(
            &processes,
            &proc_source,
            &payload.agents,
            payload.scanned,
            payload.watched,
        );
        let csv_out = render_csv::append_operator_csv_companions(body, &payload.gate).await?;
        print!("{csv_out}");
        return Ok(());
    }

    let pool = ProcessPool::new();
    let filter = if let Some(p) = project {
        ProcessFilter::ByProject(p.to_string())
    } else if let Some(h) = harness {
        ProcessFilter::ByHarness(h.to_string())
    } else {
        ProcessFilter::All
    };
    let processes: Vec<ProcessInfo> = pool.find(filter).await;
    let proc_source = HostProcSource;
    print_ps_text_table(&processes, &proc_source, project, harness, all)?;
    if all {
        // AC-007.76: pool → proc-scan on stdout after gate → host_watch (ps --all text path).
        print_live_pool_status_operator_sections().await?;
    }
    Ok(())
}

/// List processes (FR-007 / AC-007.43 / 007.49 / 007.83).
pub async fn ps(
    project: Option<&str>,
    harness: Option<&str>,
    all: bool,
    json: bool,
    csv: bool,
    watch: Option<u64>,
) -> Result<()> {
    if json && !all {
        anyhow::bail!(
            "`sharecli ps --json` requires `--all` for host agent inventory parity (AC-007.43)"
        );
    }
    if csv && !all {
        anyhow::bail!(
            "`sharecli ps --csv` requires `--all` for host agent inventory parity (AC-007.83)"
        );
    }
    if csv && json {
        anyhow::bail!("--csv cannot be combined with --json");
    }
    if watch.is_some() && json && !all {
        anyhow::bail!(
            "`sharecli ps --watch --json` requires `--all` for host agent inventory parity (AC-007.49)"
        );
    }

    match watch {
        None => render_ps_once(project, harness, all, json, csv, false).await,
        Some(interval_secs) => {
            if interval_secs == 0 {
                anyhow::bail!("--watch interval must be >= 1 second");
            }
            let ndjson = json;
            let csv_watch = csv;
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    emit_operator_csv_watch_frame(PS_CSV_WATCH_FRAME_MARKER);
                }
                render_ps_once(project, harness, all, json, csv, ndjson).await?;
                if !ndjson {
                    std::io::stdout().flush()?;
                }
                emit_operator_watch_footer(interval_secs, ndjson, csv_watch);
                let idle = cycle_start.elapsed();
                let period = Duration::from_secs(interval_secs);
                let sleep_for = period.saturating_sub(idle);
                tokio::select! {
                    _ = tokio::time::sleep(sleep_for) => {},
                    _ = tokio::signal::ctrl_c() => {
                        if ndjson {
                            eprintln!("\nExiting watch mode.");
                        } else {
                            println!("\nExiting watch mode.");
                        }
                        break;
                    }
                }
            }
            Ok(())
        }
    }
}

/// FR-006 host inventory printed by `sharecli ps --all`.
fn print_host_agent_scan(source: &HostProcSource) -> Result<()> {
    use sharecli_fleet::{scan_agents, watch_detected_agents};

    let agents = scan_agents(source);
    let watched = watch_detected_agents(&agents);
    let agent_pids: Vec<u32> = agents.iter().map(|a| a.pid).collect();
    let state_by_pid = proc::build_agent_state_map(source, &agent_pids);
    println!();
    proc::render_agent_inventory(&watched, agents.len(), &state_by_pid);
    if let Ok(thermal) = ThermalGovernor::new().poll() {
        print!("{}", format_gate_status_section(thermal, agents.len()));
    }
    print_live_host_watch_section()?;
    Ok(())
}

/// Actionable empty-pool copy for `ps` (C10 L100).
fn print_ps_empty_hint(project: Option<&str>, harness: Option<&str>) {
    if let Some(p) = project {
        println!("\nNo processes match project '{p}'. Try: sharecli start {p} <harness>");
    } else if let Some(h) = harness {
        println!("\nNo processes match harness '{h}'. Try: sharecli start <project> {h}");
    } else {
        println!("\nNo managed processes yet. Get started: sharecli start <project> <harness>");
        println!("Or open the dashboard: sharecli serve");
    }
}

/// Build the `status --json` payload (FR-007 / AC-007.25, pool sibling AC-007.77).
pub(crate) async fn build_status_json() -> Result<StatusJson> {
    let pool = ProcessPool::new();
    let processes: Vec<ProcessInfo> = pool.list().await;
    let snapshot = proc::AgentProcSnapshot::capture()?;
    Ok(StatusJson {
        total_processes: processes.len(),
        agents: snapshot.agents,
        scanned: snapshot.scanned,
        watched: snapshot.watched,
        gate: snapshot.gate,
        host_watch: snapshot.host_watch,
        pool: None,
        log_location: sharecli_log_location(),
    })
}

/// Render one status snapshot (one-shot or watch cycle).
///
/// One-shot `status --json` MUST NOT print gate/host_watch stderr companions (AC-007.32).
/// One-shot `status` text MUST NOT print gate/host_watch stderr companions either (AC-007.36);
/// gate/host_watch stay in text sections on stdout only (AC-007.27).
async fn render_status_once(verbose: bool, json: bool, csv: bool, ndjson: bool) -> Result<()> {
    if json {
        let mut payload = build_status_json().await?;
        payload.pool = Some(Box::new(crate::commands::pool::build_pool_json().await?));
        if ndjson {
            let line = StatusNdjsonLine { ts: ps_unix_ts_secs(), snapshot: payload };
            emit_ps_ndjson_line(&line)?;
            eprint_live_gate_host_watch_sections()?;
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        return Ok(());
    }

    if csv {
        let status_json = build_status_json().await?;
        let summary: sharecli_fleet::StatusOperatorPanel = status_json.clone().into();
        let pool = ProcessPool::new();
        let processes: Vec<ProcessInfo> = pool.list().await;
        let mut by_harness: std::collections::HashMap<String, (usize, u64)> =
            std::collections::HashMap::new();
        for proc in &processes {
            let h = proc.harness.as_deref().unwrap_or("unknown").to_string();
            let entry = by_harness.entry(h).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += proc.memory_mb;
        }
        let mut harness_rows: Vec<(String, usize, u64)> =
            by_harness.into_iter().map(|(h, (count, mem))| (h, count, mem)).collect();
        harness_rows.sort_by(|a, b| a.0.cmp(&b.0));
        let runtime = crate::commands::runtime_helpers::get_shared_runtime();
        let pool_status = runtime.status().await;
        let (used, total) = pool.system_memory_usage().await;
        let body =
            render_csv::render_status_csv_body(&summary, &harness_rows, &pool_status, used, total);
        let (gate, _) = capture_live_gate_host_watch()?;
        let csv_out = render_csv::append_operator_csv_companions(body, &gate).await?;
        print!("{csv_out}");
        return Ok(());
    }

    let pool = ProcessPool::new();
    let processes: Vec<ProcessInfo> = pool.list().await;

    let mut by_harness: std::collections::HashMap<&str, (usize, u64)> =
        std::collections::HashMap::new();

    for proc in &processes {
        let h = proc.harness.as_deref().unwrap_or("unknown");
        let entry = by_harness.entry(h).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += proc.memory_mb;
    }

    println!("=== Process Status ===\n");
    println!("Total: {} processes\n", processes.len());

    println!("{:<15} {:<10} {:<15}", "HARNESS", "COUNT", "MEMORY(MB)");
    println!("{}", "-".repeat(40));

    for (h, (count, mem)) in by_harness.iter() {
        println!("{:<15} {:<10} {:<15}", h, count, mem);
    }

    // Show pool status
    let runtime = crate::commands::runtime_helpers::get_shared_runtime();
    let pool_status = runtime.status().await;
    println!("\n=== Shared Runtime Pool ===\n");
    println!("{:<10} {:<10} {:<10}", "TYPE", "TOTAL", "IDLE");
    println!("{}", "-".repeat(30));
    println!("{:<10} {:<10} {:<10}", "node", pool_status.node_total, pool_status.node_idle);
    println!("{:<10} {:<10} {:<10}", "bun", pool_status.bun_total, pool_status.bun_idle);
    println!("\nMax per type: {}", pool_status.max_per_type);

    // Show system memory
    let (used, total) = pool.system_memory_usage().await;
    println!("\n=== System Memory ===\n");
    println!("Used: {} MB / {} MB ({}%)", used, total, (used * 100) / total);

    print_live_gate_section()?;
    print_live_host_watch_section()?;
    print_live_pool_status_operator_sections().await?;

    let fuse_meters = global_read_cache_meters();
    print!("{}", fuse_meters.format_status_section());

    let neg_meters = global_neg_dentry_meters();
    print!("{}", neg_meters.format_status_section());

    print!("{}", global_coalesce_meters().format_status_section());

    print!("{}", global_slot_queue_meters().format_status_section());

    if let Some(st) = capture_maildir_status()? {
        print!("{}", st.format_status_section());
    }

    print!("{}", global_write_serialize_meters().format_status_section());

    if verbose {
        println!("\n=== Detailed Process List ===\n");
        for proc in &processes {
            println!("PID: {}, Name: {}, Memory: {} MB", proc.pid, proc.name, proc.memory_mb);
            if !proc.cmd.is_empty() {
                println!("  Cmd: {}", proc.cmd.join(" "));
            }
        }
    }

    Ok(())
}

/// Check process status (FR-007 / AC-007.25, 007.32, 007.36, 007.66).
pub async fn status(verbose: bool, json: bool, csv: bool, watch: Option<u64>) -> Result<()> {
    if csv && json {
        anyhow::bail!("--csv cannot be combined with --json");
    }
    match watch {
        None => render_status_once(verbose, json, csv, false).await,
        Some(interval_secs) => {
            if interval_secs == 0 {
                anyhow::bail!("--watch interval must be >= 1 second");
            }
            let ndjson = json;
            let csv_watch = csv;
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    emit_operator_csv_watch_frame(STATUS_CSV_WATCH_FRAME_MARKER);
                }
                render_status_once(verbose, json, csv, ndjson).await?;
                if !ndjson {
                    std::io::stdout().flush()?;
                }
                emit_operator_watch_footer(interval_secs, ndjson, csv_watch);
                let idle = cycle_start.elapsed();
                let period = Duration::from_secs(interval_secs);
                let sleep_for = period.saturating_sub(idle);
                tokio::select! {
                    _ = tokio::time::sleep(sleep_for) => {},
                    _ = tokio::signal::ctrl_c() => {
                        if ndjson {
                            eprintln!("\nExiting watch mode.");
                        } else {
                            println!("\nExiting watch mode.");
                        }
                        break;
                    }
                }
            }
            Ok(())
        }
    }
}

/// Filter a process list to those belonging to a specific project.
///
/// Used by the bulk project-group operations and exposed for unit testing.
#[cfg_attr(not(test), allow(dead_code))]
pub fn filter_by_project<'a>(processes: &'a [ProcessInfo], project: &str) -> Vec<&'a ProcessInfo> {
    processes.iter().filter(|p| p.project.as_deref() == Some(project)).collect()
}
