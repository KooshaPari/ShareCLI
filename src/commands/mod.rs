//! CLI commands for sharecli

pub mod cast;
pub mod fuse;
pub mod history;
pub mod mesh;
pub mod proc;
pub mod report;
pub mod serve;
pub mod soak;
pub mod undo;
pub mod uninstall;
pub mod upgrade;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;
pub use serve::run as serve_run;
use sharecli_fleet::global_coalesce_meters;
use sharecli_fleet::global_slot_queue_meters;
use sharecli_fleet::thermal::ThermalGovernor;
use sharecli_fleet::ResourceWatchSample;
use sharecli_fleet::{
    agent_label_for_pid, count_host_agents, format_gate_status_section, gate_status_snapshot,
    scan_agents, watch_detected_agents, HostProcSource,
};
use sharecli_fuse::{
    global_neg_dentry_meters, global_read_cache_meters, global_write_serialize_meters,
};
use sharecli_mesh::capture_maildir_status;

use crate::config::{self, Config, ConfigCmd, ProjectCmd};
use crate::monitoring::HostResourceWatchJson;
use crate::progress::StepProgress;
#[cfg(test)]
use crate::runtime::ProcState;
use crate::runtime::{
    ProcessFilter, ProcessInfo, ProcessPool, ProjectLimits, ProjectResources, SharedRuntime,
};
use crate::spawn_policy::SpawnPolicy;

/// Shared runtime instance
static SHARED_RUNTIME: std::sync::OnceLock<SharedRuntime> = std::sync::OnceLock::new();

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
    use crate::monitoring::HostResourceWatchJson;

    let thermal = ThermalGovernor::new().poll()?;
    let agent_count = count_host_agents();
    eprint!("{}", format_gate_status_section(thermal, agent_count));
    eprint!("{}", HostResourceWatchJson::capture()?.format_text_section());
    let _ = std::io::stderr().flush();
    Ok(())
}

/// Live thermal gate + host resource watch for JSON envelopes (FR-007 / AC-007.44).
fn capture_live_gate_host_watch(
) -> Result<(sharecli_fleet::GateStatusSnapshot, HostResourceWatchJson)> {
    let thermal = ThermalGovernor::new().poll()?;
    let gate = gate_status_snapshot(thermal, count_host_agents());
    let host_watch = HostResourceWatchJson::capture()?;
    Ok((gate, host_watch))
}

/// Append gate → host_watch → pool → status CSV companion blocks (FR-007 / AC-007.79 / AC-007.82).
pub(crate) async fn append_operator_csv_companions(
    csv: String,
    gate: &sharecli_fleet::GateStatusSnapshot,
) -> Result<String> {
    use sharecli_fleet::{PoolOperatorPanel, StatusOperatorPanel};

    let mut out = csv;
    out.push_str(&gate.format_csv_companion());
    out.push_str(&HostResourceWatchJson::capture()?.format_csv_companion());
    let (pool_json, status_json) = fetch_operator_pool_status_siblings().await?;
    let pool: PoolOperatorPanel = pool_json.into();
    let status: StatusOperatorPanel = status_json.into();
    out.push_str(&pool.format_csv_companion());
    out.push_str(&status.format_csv_companion());
    Ok(out)
}

/// CSV `#` comment line separating each operator `--csv --watch` refresh frame (AC-007.89).
pub const HEALTH_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-health-watch-frame";
pub const POOL_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-pool-watch-frame";
pub const STATUS_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-status-watch-frame";
pub const PS_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-ps-watch-frame";

/// Emit the CSV watch frame delimiter before a tick's body (AC-007.89).
fn emit_operator_csv_watch_frame(marker: &str) {
    println!("{marker}");
}

/// Emit the CSV watch footer as a `#` comment on stdout (AC-007.89).
/// Flush so pipe consumers see `# [watch]` in the same tick (AC-007.94).
fn emit_operator_csv_watch_footer(interval_secs: u64) {
    println!("# [watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
    let _ = std::io::stdout().flush();
}

/// Print watch-mode footer for text, NDJSON, or CSV refresh loops (AC-007.64–66 / AC-007.89).
fn emit_operator_watch_footer(interval_secs: u64, ndjson: bool, csv_watch: bool) {
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

fn get_shared_runtime() -> &'static SharedRuntime {
    SHARED_RUNTIME.get_or_init(|| {
        let max = config::global().pool.max_per_type;
        SharedRuntime::new(max)
    })
}

/// Project resources instance
static PROJECT_RESOURCES: std::sync::OnceLock<ProjectResources> = std::sync::OnceLock::new();

fn get_project_resources() -> &'static ProjectResources {
    PROJECT_RESOURCES.get_or_init(ProjectResources::new)
}

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

fn ps_unix_ts_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Emit one compact JSON line and flush (piped stdout is block-buffered; AC-007.49).
fn emit_ps_ndjson_line<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    std::io::stdout().flush()?;
    Ok(())
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

/// Returns the filesystem path to the current sharecli log file, if any.
/// Read from the SHARECLI_LOG_PATH env var (with the same HOME fallback the
/// tracing-appender setup uses in main.rs). Resolved at status-snapshot time so
/// test isolation via env var is observed.
fn sharecli_log_location() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("SHARECLI_LOG_PATH") {
        return Some(std::path::PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)?;
    Some(home.join(".sharecli").join("logs").join("sharecli.log"))
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

/// Primary CSV body for `sharecli health --csv` (FR-007 / AC-007.82).
pub fn render_health_csv_body(health: &HealthJson) -> String {
    use proc::csv_escape_field;

    let issues = csv_escape_field(&health.issues.join(";"));
    format!(
        "record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues\n\
         health,{},{},{},{},{},{},{},{},{}\n",
        health.healthy,
        health.node_total,
        health.node_idle,
        health.node_in_use,
        health.bun_total,
        health.bun_idle,
        health.bun_in_use,
        health.max_per_type,
        issues,
    )
}

/// Primary CSV body for `sharecli pool --csv` (FR-007 / AC-007.82).
pub fn render_pool_csv_body(pool: &PoolJson) -> String {
    use proc::csv_escape_field;

    let issues = csv_escape_field(&pool.issues.join(";"));
    format!(
        "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues\n\
         pool,{},{},{},{},{},{},{}\n",
        pool.node_total,
        pool.node_idle,
        pool.bun_total,
        pool.bun_idle,
        pool.max_per_type,
        pool.healthy,
        issues,
    )
}

/// Primary CSV body for `sharecli status --csv` (FR-007 / AC-007.82).
pub fn render_status_csv_body(
    summary: &sharecli_fleet::StatusOperatorPanel,
    harness_rows: &[(String, usize, u64)],
    pool_status: &crate::runtime::PoolStatus,
    used_mb: u64,
    total_mb: u64,
) -> String {
    use proc::csv_escape_field;

    let mut out = format!(
        "record,total_processes,scanned,watched,agent_rows\n\
         status,{},{},{},{}\n",
        summary.total_processes, summary.scanned, summary.watched, summary.agent_rows,
    );
    out.push_str("\nrecord,harness,count,memory_mb\n");
    for (h, count, mem) in harness_rows {
        out.push_str(&format!("harness,{},{},{mem}\n", csv_escape_field(h), count,));
    }
    out.push_str("\nrecord,type,total,idle,max_per_type\n");
    out.push_str(&format!(
        "runtime_pool,node,{},{},{}\n",
        pool_status.node_total, pool_status.node_idle, pool_status.max_per_type,
    ));
    out.push_str(&format!(
        "runtime_pool,bun,{},{},{}\n",
        pool_status.bun_total, pool_status.bun_idle, pool_status.max_per_type,
    ));
    let pct = (used_mb * 100).checked_div(total_mb).unwrap_or(0);
    out.push_str("\nrecord,used_mb,total_mb,used_pct\n");
    out.push_str(&format!("system_memory,{used_mb},{total_mb},{pct}\n"));
    out
}

/// Primary CSV body for `sharecli ps --all --csv` (FR-007 / AC-007.83).
pub fn render_ps_all_csv_body(
    processes: &[ProcessInfo],
    proc_source: &HostProcSource,
    agents: &[proc::AgentProcRow],
    scanned: usize,
    watched: usize,
) -> String {
    use proc::csv_escape_field;

    let mut out = String::from("record,pid,name,memory_mb,project,harness,agent\n");
    for proc in processes {
        out.push_str(&format!(
            "process,{},{},{},{},{},{}\n",
            proc.pid,
            csv_escape_field(&proc.name),
            proc.memory_mb,
            csv_escape_field(proc.project.as_deref().unwrap_or("-")),
            csv_escape_field(proc.harness.as_deref().unwrap_or("-")),
            csv_escape_field(agent_label_for_pid(proc_source, proc.pid)),
        ));
    }
    let total_mem: u64 = processes.iter().map(|p| p.memory_mb).sum();
    out.push_str("\nrecord,process_count,total_memory_mb\n");
    out.push_str(&format!("summary,{},{total_mem}\n", processes.len()));
    out.push_str("\nrecord,scanned,watched\n");
    out.push_str(&format!("agent_inventory,{scanned},{watched}\n"));
    out.push_str("\npid,family,comm,state,mem_rss_bytes,mem_rss,fd_count\n");
    for row in agents {
        let fd = row.fd_count.map(|n| n.to_string()).unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            row.pid,
            csv_escape_field(&row.family),
            csv_escape_field(&row.comm),
            csv_escape_field(&row.state),
            row.mem_rss_bytes,
            csv_escape_field(&row.mem_rss),
            fd,
        ));
    }
    out
}

/// One NDJSON watch line for `status --watch --json` (FR-007 / AC-007.66).
#[derive(Debug, Clone, Serialize)]
pub struct StatusNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: StatusJson,
}

async fn build_ps_all_json(project: Option<&str>, harness: Option<&str>) -> Result<PsAllJson> {
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
    let (pool_panel, status_panel) = fetch_operator_pool_status_siblings().await?;
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
        let body = render_ps_all_csv_body(
            &processes,
            &proc_source,
            &payload.agents,
            payload.scanned,
            payload.watched,
        );
        let csv_out = append_operator_csv_companions(body, &payload.gate).await?;
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

/// List processes
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

/// Start a harness process
pub async fn start(project: &str, harness: &str, cwd: Option<&str>, args: &[String]) -> Result<()> {
    let cfg = Config::load()?;

    let project_path = if let Some(c) = cwd {
        PathBuf::from(expand_path(c))
    } else if let Some(path) = cfg.projects.get(project) {
        PathBuf::from(expand_path(path))
    } else {
        anyhow::bail!(
            "Unknown project: {}. Add with 'sharecli project add <name> <path>'",
            project
        );
    };

    if !project_path.exists() {
        anyhow::bail!("Project path does not exist: {:?}", project_path);
    }

    // Apply the spawn-policy throttle when the harness is a build harness (cargo/rustc/…).
    // The policy is constructed from the global config's [spawn_policy] section.
    let pool = {
        let policy = SpawnPolicy::new(cfg.spawn_policy.clone());
        ProcessPool::with_spawn_policy(Arc::new(policy))
    };
    println!("Starting {} harness for project '{}'...", harness, project);

    let info = pool
        .spawn(
            harness,
            args,
            Some(project_path.clone()),
            Some(project.to_string()),
            Some(harness.to_string()),
        )
        .await?;

    println!("Started process {} ({})", info.pid, info.name);
    println!("Working directory: {:?}", project_path);

    Ok(())
}

/// When `force` is set, destructive SIGKILL requires explicit `--yes` (C09 L81.6).
fn force_kill_requires_confirmation(force: bool, yes: bool) -> bool {
    force && !yes
}

/// Preview lines printed before aborting an unconfirmed force-kill.
fn force_kill_preview(target_label: &str, count: usize) {
    println!("Would force-kill (SIGKILL) {target_label} ({count} process(es)).");
    println!("Pass --yes to confirm. Prefer graceful stop without --force when possible.");
}

/// Stop processes
pub async fn stop(
    pid: Option<u32>,
    project: Option<&str>,
    harness: Option<&str>,
    all: bool,
    force: bool,
    yes: bool,
) -> Result<()> {
    let pool = ProcessPool::new();

    if all {
        if force_kill_requires_confirmation(force, yes) {
            let count = pool.list().await.len();
            force_kill_preview("all managed processes", count);
            return Ok(());
        }
        println!("Stopping all managed processes{}...", if force { " (force)" } else { "" });
        pool.kill_all().await?;
        println!("All processes stopped.");
        return Ok(());
    }

    if let Some(p) = pid {
        if force_kill_requires_confirmation(force, yes) {
            force_kill_preview(&format!("PID {p}"), 1);
            return Ok(());
        }
        println!("Stopping process {p}{}...", if force { " (force)" } else { "" });
        pool.kill(p).await?;
        println!("Process {p} stopped.");
        return Ok(());
    }

    let filter = if let Some(proj) = project {
        ProcessFilter::ByProject(proj.to_string())
    } else if let Some(h) = harness {
        ProcessFilter::ByHarness(h.to_string())
    } else {
        anyhow::bail!("Specify --pid, --project, --harness, or --all to select what to stop");
    };

    let processes = pool.find(filter).await;
    if force_kill_requires_confirmation(force, yes) {
        let label = if let Some(proj) = project {
            format!("project '{proj}'")
        } else {
            format!("harness '{}'", harness.unwrap_or(""))
        };
        force_kill_preview(&label, processes.len());
        return Ok(());
    }

    let progress = StepProgress::new("Stopping processes", processes.len());
    let line_mode = progress.uses_line_output();
    for proc in &processes {
        pool.kill(proc.pid).await?;
        progress.inc(Some(&format!("{} ({})", proc.pid, proc.name)));
        if line_mode {
            println!("Stopped {} ({})", proc.pid, proc.name);
        }
    }
    progress.finish("Processes stopped");

    Ok(())
}

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

/// Fetch independent pool + status JSON panels for operator sibling embedding (AC-007.77).
pub(crate) async fn fetch_operator_pool_status_siblings() -> Result<(PoolJson, StatusJson)> {
    let (pool, status) = tokio::join!(build_pool_json(), build_status_json());
    Ok((pool?, status?))
}

/// Render one status snapshot (one-shot or watch cycle).
///
/// One-shot `status --json` MUST NOT print gate/host_watch stderr companions (AC-007.32).
/// One-shot `status` text MUST NOT print gate/host_watch stderr companions either (AC-007.36);
/// gate/host_watch stay in text sections on stdout only (AC-007.27).
async fn render_status_once(verbose: bool, json: bool, csv: bool, ndjson: bool) -> Result<()> {
    if json {
        let mut payload = build_status_json().await?;
        payload.pool = Some(Box::new(build_pool_json().await?));
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
        let runtime = get_shared_runtime();
        let pool_status = runtime.status().await;
        let (used, total) = pool.system_memory_usage().await;
        let body = render_status_csv_body(&summary, &harness_rows, &pool_status, used, total);
        let (gate, _) = capture_live_gate_host_watch()?;
        let csv_out = append_operator_csv_companions(body, &gate).await?;
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
    let runtime = get_shared_runtime();
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

/// Check process status.
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

/// Configuration management
pub fn config(cfg_cmd: &ConfigCmd) -> Result<()> {
    match cfg_cmd {
        ConfigCmd::Init => {
            Config::init()?;
            println!("Configuration initialized.");
        }
        ConfigCmd::Validate => {
            let cfg = Config::load()?;
            println!("Configuration is valid.");
            println!("  Projects: {}", cfg.projects.len());
        }
        ConfigCmd::Show => {
            let cfg = Config::load()?;
            let serialized = toml::to_string_pretty(&cfg)?;
            println!("{}", serialized);
        }
        ConfigCmd::Get { key: _ } => {
            let cfg = Config::load()?;
            println!("Projects:");
            for (name, path) in &cfg.projects {
                println!("  {} = {}", name, path);
            }
        }
        ConfigCmd::Set { .. } => {
            println!("Set not implemented yet.");
        }
    }
    Ok(())
}

/// Filter a process list to those belonging to a specific project.
///
/// Used by the bulk project-group operations and exposed for unit testing.
#[cfg_attr(not(test), allow(dead_code))]
pub fn filter_by_project<'a>(processes: &'a [ProcessInfo], project: &str) -> Vec<&'a ProcessInfo> {
    processes.iter().filter(|p| p.project.as_deref() == Some(project)).collect()
}

/// Project management (async — bulk ops need an async runtime)
pub async fn project(proj_cmd: &ProjectCmd) -> Result<()> {
    match proj_cmd {
        ProjectCmd::Add { name, path } => {
            let mut cfg = Config::load()?;
            cfg.projects.insert(name.clone(), expand_path(path));
            cfg.save()?;
            println!("Added project '{}'", name);
        }
        ProjectCmd::Remove { name } => {
            let mut cfg = Config::load()?;
            if cfg.projects.remove(name).is_some() {
                cfg.save()?;
                println!("Removed project '{}'", name);
            }
        }
        ProjectCmd::List => {
            let cfg = Config::load()?;
            if cfg.projects.is_empty() {
                println!("No projects registered. Run 'sharecli project discover'.");
            } else {
                println!("Registered Projects:");
                for (name, path) in &cfg.projects {
                    println!("  {} -> {}", name, path);
                }
            }
        }
        ProjectCmd::Show { name } => {
            let cfg = Config::load()?;
            if let Some(path) = cfg.projects.get(name) {
                println!("Project: {}", name);
                println!("Path:    {}", path);
                println!("Exists:  {}", std::path::Path::new(path).exists());
            }
        }
        ProjectCmd::Discover { path } => {
            let cfg = config::global();
            let base =
                PathBuf::from(expand_path(path.as_deref().unwrap_or(&cfg.paths.discovery_path)));
            println!("Scanning {:?} for projects...", base);

            let mut found = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && p.join(".git").exists() {
                        let name =
                            p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                        found.push((name, p.to_string_lossy().to_string()));
                    }
                }
            }

            println!("\nFound {} projects:", found.len());
            for (name, path) in &found {
                println!("  {} -> {}", name, path);
            }
        }
        ProjectCmd::Generate { output } => {
            let cfg = config::global();
            let out_path = PathBuf::from(expand_path(
                output.as_deref().unwrap_or(&cfg.paths.default_compose_output),
            ));

            let sharewei_port = cfg.port.sharewei_port;
            let mut yaml = format!("# Generated by sharecli\nversion: \"0.5\"\n\nenv:\n  - SHAREWEI_PORT={}\n\nservices:\n", sharewei_port);

            for name in cfg.projects.keys() {
                yaml.push_str(&format!(
                    r#"  {}-agent:
    command: sharecli run --harness {} --project {}
    depends_on: {{}}
    log_location: .sharecli/logs/{}.log
    readiness_probe:
      exec:
        command: sharecli health --harness {}
      initial_delay_seconds: 5
      period_seconds: 10
      failure_threshold: 3

"#,
                    name, name, name, name, name
                ));
            }

            std::fs::write(&out_path, &yaml)?;
            println!("Generated process-compose.yml with {} services", cfg.projects.len());
            println!("Written to: {:?}", out_path);
        }
        ProjectCmd::Start { name, harness } => {
            project_group_start(name, harness.as_deref()).await?;
        }
        ProjectCmd::Stop { name, force, yes } => {
            project_group_stop(name, *force, *yes).await?;
        }
        ProjectCmd::Restart { name, harness, force, yes } => {
            project_group_stop(name, *force, *yes).await?;
            project_group_start(name, harness.as_deref()).await?;
        }
        ProjectCmd::Status { name, json } => {
            project_group_status(name, *json).await?;
        }
    }
    Ok(())
}

/// Start all stopped processes for a project group.
///
/// Spawns a process in the project's configured directory.  If `harness` is
/// `None` the function defaults to `"sh"` so that there is always something
/// runnable without additional flags.
async fn project_group_start(name: &str, harness: Option<&str>) -> Result<()> {
    let cfg = Config::load()?;
    let project_path = if let Some(path) = cfg.projects.get(name) {
        PathBuf::from(expand_path(path))
    } else {
        anyhow::bail!("Unknown project: '{}'. Add with 'sharecli project add <name> <path>'", name);
    };

    if !project_path.exists() {
        anyhow::bail!("Project path does not exist: {:?}", project_path);
    }

    let harness_name = harness.unwrap_or("sh");
    let pool = ProcessPool::new();

    println!("Starting '{}' harness for project group '{}'...", harness_name, name);
    let info = pool
        .spawn(
            harness_name,
            &[],
            Some(project_path),
            Some(name.to_string()),
            Some(harness_name.to_string()),
        )
        .await?;

    println!("Affected: 1 process started. PID {} ({})", info.pid, info.name);
    Ok(())
}

/// Stop all running processes in a project group.
///
/// Returns the number of processes killed.  Collects failures and reports
/// them after attempting every process so that a single bad PID does not
/// prevent the rest from being stopped.
async fn project_group_stop(name: &str, force: bool, yes: bool) -> Result<()> {
    let pool = ProcessPool::new();
    let processes = pool.find(ProcessFilter::ByProject(name.to_string())).await;

    if processes.is_empty() {
        println!("No running processes found for project '{}'.", name);
        return Ok(());
    }

    let total = processes.len();
    if force_kill_requires_confirmation(force, yes) {
        force_kill_preview(&format!("project '{name}'"), total);
        return Ok(());
    }
    let mut stopped = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let progress = StepProgress::new(&format!("Stopping project '{name}'"), total);
    let line_mode = progress.uses_line_output();

    for proc in &processes {
        match pool.kill(proc.pid).await {
            Ok(()) => {
                progress.inc(Some(&format!("{} ({})", proc.pid, proc.name)));
                if line_mode {
                    println!("Stopped {} ({})", proc.pid, proc.name);
                }
                stopped += 1;
            }
            Err(e) => {
                failures.push(format!("PID {} ({}): {}", proc.pid, proc.name, e));
            }
        }
    }
    progress.finish(&format!("Affected: {stopped}/{total} processes stopped"));
    if line_mode {
        println!("\nAffected: {}/{} processes stopped.", stopped, total);
    }
    if !failures.is_empty() {
        println!("Failures:");
        for f in &failures {
            println!("  - {}", f);
        }
        anyhow::bail!("{} process(es) could not be stopped", failures.len());
    }
    Ok(())
}

/// Show a status table for all processes in a project group.
async fn project_group_status(name: &str, json: bool) -> Result<()> {
    let pool = ProcessPool::new();
    let processes = pool.find(ProcessFilter::ByProject(name.to_string())).await;

    if json {
        // Emit a JSON array of process objects.
        let items: Vec<serde_json::Value> = processes
            .iter()
            .map(|p| {
                serde_json::json!({
                    "pid": p.pid,
                    "name": p.name,
                    "memory_mb": p.memory_mb,
                    "project": p.project,
                    "harness": p.harness,
                    "cmd": p.cmd,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    println!("=== Project '{}' — {} process(es) ===\n", name, processes.len());
    println!("{:<8} {:<20} {:<12} {:<15}", "PID", "NAME", "MEM(MB)", "HARNESS");
    println!("{}", "-".repeat(58));

    for proc in &processes {
        let harness = proc.harness.as_deref().unwrap_or("-");
        println!(
            "{:<8} {:<20} {:<12.1} {:<15}",
            proc.pid, proc.name, proc.memory_mb as f64, harness
        );
    }

    let total_mem: u64 = processes.iter().map(|p| p.memory_mb).sum();
    println!("\nTotal: {} processes, {} MB memory", processes.len(), total_mem);
    Ok(())
}

/// Run using pooled runtime
pub async fn run_pool(harness_type: &str, project: &str) -> Result<()> {
    let runtime = get_shared_runtime();
    let result = runtime.run_with_pool(harness_type, project, "").await?;
    println!("Pooled {} process {} for project {}", harness_type, result.0, project);
    println!("Output: {}", result.1);
    Ok(())
}

pub(crate) async fn build_pool_json() -> Result<PoolJson> {
    let runtime = get_shared_runtime();
    let status = runtime.status().await;
    let health = runtime.health_check().await;
    let (gate, host_watch) = capture_live_gate_host_watch()?;
    Ok(PoolJson {
        node_total: status.node_total,
        node_idle: status.node_idle,
        bun_total: status.bun_total,
        bun_idle: status.bun_idle,
        max_per_type: status.max_per_type,
        healthy: health.healthy,
        issues: health.issues,
        gate,
        host_watch,
        status: None,
    })
}

/// Render one pool snapshot (one-shot or watch cycle).
///
/// One-shot `pool --json` MUST NOT print gate/host_watch stderr companions (AC-007.44).
async fn render_pool_once(json: bool, csv: bool, ndjson: bool) -> Result<()> {
    if json {
        let mut payload = build_pool_json().await?;
        payload.status = Some(Box::new(build_status_json().await?));
        if ndjson {
            let line = PoolNdjsonLine { ts: ps_unix_ts_secs(), snapshot: payload };
            emit_ps_ndjson_line(&line)?;
            eprint_live_gate_host_watch_sections()?;
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        return Ok(());
    }

    if csv {
        let payload = build_pool_json().await?;
        let body = render_pool_csv_body(&payload);
        let csv_out = append_operator_csv_companions(body, &payload.gate).await?;
        print!("{csv_out}");
        return Ok(());
    }

    let runtime = get_shared_runtime();
    let status = runtime.status().await;
    let health = runtime.health_check().await;

    println!("=== Shared Runtime Pool Status ===\n");
    println!("{:<10} {:<10} {:<10} {:<10}", "TYPE", "TOTAL", "IDLE", "MAX");
    println!("{}", "-".repeat(40));
    println!(
        "{:<10} {:<10} {:<10} {:<10}",
        "node", status.node_total, status.node_idle, status.max_per_type
    );
    println!(
        "{:<10} {:<10} {:<10} {:<10}",
        "bun", status.bun_total, status.bun_idle, status.max_per_type
    );
    println!("\nMax per type: {}", status.max_per_type);

    // Health check
    println!("\n=== Health Check ===");
    if health.healthy {
        println!("Status: HEALTHY");
    } else {
        println!("Status: DEGRADED");
    }
    if !health.issues.is_empty() {
        println!("\nIssues:");
        for issue in &health.issues {
            println!("  - {}", issue);
        }
    }

    print_live_gate_section()?;
    print_live_host_watch_section()?;
    print_live_pool_status_operator_sections().await?;

    Ok(())
}

/// Show pool status
pub async fn pool_status(json: bool, csv: bool, watch: Option<u64>) -> Result<()> {
    if csv && json {
        anyhow::bail!("--csv cannot be combined with --json");
    }
    match watch {
        None => render_pool_once(json, csv, false).await,
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
                    emit_operator_csv_watch_frame(POOL_CSV_WATCH_FRAME_MARKER);
                }
                render_pool_once(json, csv, ndjson).await?;
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

async fn build_health_json() -> Result<HealthJson> {
    let runtime = get_shared_runtime();
    let pool_status = runtime.status().await;
    let health = runtime.health_check().await;
    let (gate, host_watch) = capture_live_gate_host_watch()?;
    let (pool_panel, status_panel) = fetch_operator_pool_status_siblings().await?;
    Ok(HealthJson {
        healthy: health.healthy,
        issues: health.issues,
        node_total: pool_status.node_total,
        node_idle: pool_status.node_idle,
        node_in_use: health.node_in_use,
        bun_total: pool_status.bun_total,
        bun_idle: pool_status.bun_idle,
        bun_in_use: health.bun_in_use,
        max_per_type: pool_status.max_per_type,
        gate,
        host_watch,
        pool: pool_panel,
        status: status_panel,
    })
}

/// Render one health snapshot (one-shot or watch cycle).
///
/// One-shot `health --json` MUST NOT print gate/host_watch stderr companions (AC-007.44).
async fn render_health_once(
    harness: Option<&str>,
    json: bool,
    csv: bool,
    ndjson: bool,
) -> Result<()> {
    if json {
        let payload = build_health_json().await?;
        if ndjson {
            let line = HealthNdjsonLine { ts: ps_unix_ts_secs(), snapshot: payload };
            emit_ps_ndjson_line(&line)?;
            eprint_live_gate_host_watch_sections()?;
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        return Ok(());
    }

    if csv {
        let payload = build_health_json().await?;
        let body = render_health_csv_body(&payload);
        let csv_out = append_operator_csv_companions(body, &payload.gate).await?;
        print!("{csv_out}");
        return Ok(());
    }

    if let Some(h) = harness {
        println!("Health probe requested for harness '{}'.", h);
        if h != "node" && h != "bun" {
            println!("Note: only the pooled node/bun runtimes are tracked currently.");
        }
    }

    let runtime = get_shared_runtime();
    let pool_status = runtime.status().await;
    let health = runtime.health_check().await;

    println!("\nShared runtime health: {}", if health.healthy { "HEALTHY" } else { "DEGRADED" });

    if !health.issues.is_empty() {
        println!("\nIssues detected:");
        for issue in &health.issues {
            println!("  - {}", issue);
        }
    } else {
        println!("No runtime issues detected.");
    }

    println!("\nPool summary:");
    println!(
        "  node: {} total, {} idle, {} in use",
        pool_status.node_total, pool_status.node_idle, health.node_in_use
    );
    println!(
        "  bun:  {} total, {} idle, {} in use",
        pool_status.bun_total, pool_status.bun_idle, health.bun_in_use
    );
    println!("\nMax per harness type: {}", pool_status.max_per_type);

    print_live_gate_section()?;
    print_live_host_watch_section()?;
    print_live_pool_status_operator_sections().await?;

    Ok(())
}

/// Run health probe for shared runtime
pub async fn health(
    harness: Option<&str>,
    json: bool,
    csv: bool,
    watch: Option<u64>,
) -> Result<()> {
    if csv && json {
        anyhow::bail!("--csv cannot be combined with --json");
    }
    match watch {
        None => render_health_once(harness, json, csv, false).await,
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
                    emit_operator_csv_watch_frame(HEALTH_CSV_WATCH_FRAME_MARKER);
                }
                render_health_once(harness, json, csv, ndjson).await?;
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

/// Set project limits
pub async fn set_limits(
    project: &str,
    memory_mb: Option<u64>,
    max_procs: Option<usize>,
) -> Result<()> {
    let resources = get_project_resources();
    let current = resources.get_limits(project).await;

    let memory_limit = memory_mb.unwrap_or(current.memory_limit_mb);
    let max_processes = max_procs.unwrap_or(current.max_processes);

    let limits = ProjectLimits {
        memory_limit_mb: memory_limit,
        max_processes,
        cpu_affinity: current.cpu_affinity,
    };

    resources.set_limits(project, limits).await;
    println!("Set limits for project '{}':", project);
    println!("  Memory: {} MB", memory_limit);
    println!("  Max processes: {}", max_processes);
    Ok(())
}

/// Check project limits
pub async fn check_limits(project: &str) -> Result<()> {
    let resources = get_project_resources();
    let check = resources.check_limits(project).await?;

    println!("=== Resource Limits for '{}' ===\n", project);

    println!("Memory: {} MB / {} MB", check.memory_mb, check.memory_limit_mb);
    if check.memory_ok {
        println!("  Status: OK");
    } else {
        println!("  Status: EXCEEDED (over by {} MB)", check.memory_mb - check.memory_limit_mb);
    }

    println!("\nProcesses: {} / {}", check.process_count, check.max_processes);
    if check.processes_ok {
        println!("  Status: OK");
    } else {
        println!("  Status: EXCEEDED (over by {})", check.process_count - check.max_processes);
    }

    println!("\nOverall: {}", if check.overall_ok { "OK" } else { "LIMIT EXCEEDED" });

    Ok(())
}

fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen("~/", &format!("{}/", home), 1);
        }
    }
    path.to_string()
}

#[cfg(test)]
mod project_group_tests {
    use super::*;

    fn make_proc(
        pid: u32,
        name: &str,
        project: Option<&str>,
        harness: Option<&str>,
    ) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cmd: vec![],
            memory_mb: 100,
            start_time: 0,
            cpu_percent: 0.0,
            project: project.map(str::to_string),
            harness: harness.map(str::to_string),
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::default(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        }
    }

    #[test]
    fn filter_returns_only_matching_project() {
        let procs = vec![
            make_proc(1, "alpha", Some("proj-a"), Some("cargo")),
            make_proc(2, "beta", Some("proj-b"), Some("node")),
            make_proc(3, "gamma", Some("proj-a"), Some("bun")),
            make_proc(4, "delta", None, None),
        ];
        let result = filter_by_project(&procs, "proj-a");
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.project.as_deref() == Some("proj-a")));
    }

    #[test]
    fn filter_returns_empty_when_no_match() {
        let procs = vec![
            make_proc(1, "alpha", Some("proj-a"), Some("cargo")),
            make_proc(2, "beta", Some("proj-b"), Some("node")),
        ];
        let result = filter_by_project(&procs, "proj-c");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_ignores_processes_with_no_project() {
        let procs = vec![
            make_proc(1, "untagged", None, None),
            make_proc(2, "tagged", Some("proj-a"), Some("cargo")),
        ];
        let result = filter_by_project(&procs, "proj-a");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pid, 2);
    }

    #[test]
    fn filter_returns_all_when_all_match() {
        let procs = vec![
            make_proc(1, "a", Some("myproj"), Some("cargo")),
            make_proc(2, "b", Some("myproj"), Some("node")),
            make_proc(3, "c", Some("myproj"), Some("bun")),
        ];
        let result = filter_by_project(&procs, "myproj");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn filter_is_case_sensitive() {
        let procs = vec![
            make_proc(1, "a", Some("Proj-A"), Some("cargo")),
            make_proc(2, "b", Some("proj-a"), Some("cargo")),
        ];
        let result = filter_by_project(&procs, "proj-a");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pid, 2);
    }

    #[test]
    fn filter_on_empty_list_returns_empty() {
        let procs: Vec<ProcessInfo> = vec![];
        let result = filter_by_project(&procs, "any-project");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_preserves_all_fields() {
        let procs = vec![make_proc(42, "my-proc", Some("target"), Some("cargo"))];
        let result = filter_by_project(&procs, "target");
        assert_eq!(result.len(), 1);
        let p = result[0];
        assert_eq!(p.pid, 42);
        assert_eq!(p.name, "my-proc");
        assert_eq!(p.harness.as_deref(), Some("cargo"));
        assert_eq!(p.memory_mb, 100);
    }

    #[test]
    fn force_kill_requires_confirmation_when_force_without_yes() {
        assert!(super::force_kill_requires_confirmation(true, false));
    }

    #[test]
    fn force_kill_skips_confirmation_when_yes() {
        assert!(!super::force_kill_requires_confirmation(true, true));
    }

    #[test]
    fn force_kill_skips_confirmation_when_not_force() {
        assert!(!super::force_kill_requires_confirmation(false, false));
    }

    // --- render_health_csv_body ---

    #[test]
    fn render_health_csv_body_healthy() {
        let gate_snap = sharecli_fleet::GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let hw = HostResourceWatchJson::default();
        let pool = PoolJson {
            node_total: 0,
            node_idle: 0,
            bun_total: 0,
            bun_idle: 0,
            max_per_type: 0,
            healthy: true,
            issues: vec![],
            gate: gate_snap.clone(),
            host_watch: hw.clone(),
            status: None,
        };
        let status = StatusJson {
            total_processes: 0,
            agents: vec![],
            scanned: 0,
            watched: 0,
            gate: gate_snap.clone(),
            host_watch: hw.clone(),
            pool: None,
            log_location: None,
        };
        let health = HealthJson {
            healthy: true,
            node_total: 3,
            node_idle: 1,
            node_in_use: 2,
            bun_total: 5,
            bun_idle: 3,
            bun_in_use: 2,
            max_per_type: 5,
            issues: vec![],
            gate: gate_snap,
            host_watch: hw,
            pool,
            status,
        };
        let csv = render_health_csv_body(&health);
        assert!(csv.starts_with("record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues"));
        assert!(csv.contains("health,true,3,1,2,5,3,2,5,"));
    }

    #[test]
    fn render_health_csv_body_unhealthy_with_issues() {
        let gate_snap = sharecli_fleet::GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let hw = HostResourceWatchJson::default();
        let health = HealthJson {
            healthy: false,
            node_total: 0,
            node_idle: 0,
            node_in_use: 0,
            bun_total: 0,
            bun_idle: 0,
            bun_in_use: 0,
            max_per_type: 5,
            issues: vec!["port conflict".into(), "OOM".into()],
            gate: gate_snap.clone(),
            host_watch: hw.clone(),
            pool: PoolJson {
                node_total: 0,
                node_idle: 0,
                bun_total: 0,
                bun_idle: 0,
                max_per_type: 0,
                healthy: true,
                issues: vec![],
                gate: gate_snap.clone(),
                host_watch: hw.clone(),
                status: None,
            },
            status: StatusJson {
                total_processes: 0,
                agents: vec![],
                scanned: 0,
                watched: 0,
                gate: gate_snap,
                host_watch: hw,
                pool: None,
                log_location: None,
            },
        };
        let csv = render_health_csv_body(&health);
        assert!(csv.contains("health,false,"));
        assert!(csv.contains("port conflict;OOM"));
    }

    // --- render_pool_csv_body ---

    #[test]
    fn render_pool_csv_body_basic() {
        let pool = PoolJson {
            node_total: 4,
            node_idle: 2,
            bun_total: 6,
            bun_idle: 4,
            max_per_type: 10,
            healthy: true,
            issues: vec![],
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            status: None,
        };
        let csv = render_pool_csv_body(&pool);
        assert!(csv.starts_with(
            "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues"
        ));
        assert!(csv.contains("pool,4,2,6,4,10,true,"));
    }

    #[test]
    fn render_pool_csv_body_with_issues() {
        let pool = PoolJson {
            node_total: 0,
            node_idle: 0,
            bun_total: 0,
            bun_idle: 0,
            max_per_type: 5,
            healthy: false,
            issues: vec!["stale lock".into()],
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            status: None,
        };
        let csv = render_pool_csv_body(&pool);
        assert!(csv.contains("pool,0,0,0,0,5,false,"));
        assert!(csv.contains("stale lock"));
    }

    // --- filter_by_project edge cases ---

    #[test]
    fn filter_by_project_with_multiple_harnesses_same_project() {
        let procs = vec![
            make_proc(1, "a", Some("proj"), Some("cargo")),
            make_proc(2, "b", Some("proj"), Some("node")),
            make_proc(3, "c", Some("proj"), Some("bun")),
        ];
        let result = filter_by_project(&procs, "proj");
        assert_eq!(result.len(), 3);
    }

    // --- force_kill_requires_confirmation edge cases ---

    #[test]
    fn force_kill_both_true_skips_confirmation() {
        assert!(!super::force_kill_requires_confirmation(true, true));
    }
}
