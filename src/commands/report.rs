//! Fleet analytics report command.
//!
//! `sharecli report [--format text|json|csv] [--watch <secs>] [--sort memory|name]`
//! prints a fleet analytics snapshot to stdout.  With `--watch N` it clears the
//! terminal and re-renders every N seconds until Ctrl-C.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sharecli_fleet::GateStatusSnapshot;

use crate::commands::{PoolJson, StatusJson};
use crate::monitoring::HostResourceWatchJson;
#[cfg(test)]
use crate::runtime::ProcState;
use crate::runtime::{ProcessInfo, ProcessPool};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// CSV `#` comment line separating each `report --format csv --watch` refresh frame (AC-007.90).
pub const REPORT_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-report-watch-frame";

/// Output format for `sharecli report`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ReportFormat {
    #[default]
    Text,
    Json,
    Csv,
}

/// Sort key for top-consumers list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SortBy {
    /// Descending memory usage (default).
    #[default]
    Memory,
    /// Ascending process name (alphabetical).
    Name,
}

impl std::str::FromStr for SortBy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "memory" => Ok(Self::Memory),
            "name" => Ok(Self::Name),
            other => anyhow::bail!("unknown sort key '{}'; expected 'memory' or 'name'", other),
        }
    }
}

impl std::str::FromStr for ReportFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            other => anyhow::bail!("unknown format '{}'; expected 'text', 'json', or 'csv'", other),
        }
    }
}

/// Per-project breakdown included in the report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectBreakdown {
    pub count: usize,
    pub memory_mb: u64,
}

/// Summary of one of the top memory consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopConsumer {
    pub pid: u32,
    pub name: String,
    pub project: Option<String>,
    pub memory_mb: u64,
}

/// Full analytics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetReport {
    /// Unix timestamp (seconds) when the snapshot was taken.
    pub timestamp: u64,
    /// Approximate daemon uptime in seconds (based on earliest process start time).
    pub uptime_seconds: u64,
    /// Total number of tracked processes.
    pub total_processes: usize,
    /// Sum of `memory_mb` across all tracked processes.
    pub total_memory_mb: u64,
    /// Per-project count + memory.
    pub by_project: HashMap<String, ProjectBreakdown>,
    /// Top-5 memory consumers (descending).
    pub top_consumers: Vec<TopConsumer>,
    /// Thermal pressure level string ("GREEN" / "YELLOW" / "RED" or "UNAVAILABLE").
    pub thermal_pressure: String,
    /// Live proc-scan agent inventory (FR-011).
    pub detected_agents: usize,
    /// Agent contention tier (`OK` / `WARN` / `REFUSE`).
    pub agent_contention: String,
    /// Effective gate decision (`ADMIT` / `DENY`).
    pub gate_decision: String,
}

/// JSON envelope for `sharecli report --format json` (FR-007 / AC-007.40, pool/status AC-007.73).
///
/// Fleet analytics fields are followed by live `gate`, `host_watch`, `pool`, and `status`
/// siblings (parity with `monitoring.report` AC-007.72 / dashboard WS AC-007.70 key order).
#[derive(Debug, Clone, Serialize)]
pub struct FleetReportJson {
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub total_processes: usize,
    pub total_memory_mb: u64,
    pub by_project: HashMap<String, ProjectBreakdown>,
    pub top_consumers: Vec<TopConsumer>,
    pub thermal_pressure: String,
    pub detected_agents: usize,
    pub agent_contention: String,
    pub gate_decision: String,
    /// Live thermal + agent gate snapshot (FR-007 / AC-007.40).
    pub gate: GateStatusSnapshot,
    /// Live host FD/RSS/load/net watch (FR-007 / AC-007.40).
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool status (FR-007 / AC-007.73).
    pub pool: PoolJson,
    /// Proc-scan status snapshot (FR-007 / AC-007.73).
    pub status: StatusJson,
}

impl FleetReportJson {
    pub fn from_parts(
        report: &FleetReport,
        gate: GateStatusSnapshot,
        host_watch: HostResourceWatchJson,
        pool: PoolJson,
        status: StatusJson,
    ) -> Self {
        Self {
            timestamp: report.timestamp,
            uptime_seconds: report.uptime_seconds,
            total_processes: report.total_processes,
            total_memory_mb: report.total_memory_mb,
            by_project: report.by_project.clone(),
            top_consumers: report.top_consumers.clone(),
            thermal_pressure: report.thermal_pressure.clone(),
            detected_agents: report.detected_agents,
            agent_contention: report.agent_contention.clone(),
            gate_decision: report.gate_decision.clone(),
            gate,
            host_watch,
            pool,
            status,
        }
    }
}

/// One NDJSON watch line for `report --watch --format json` (FR-007 / AC-007.42).
#[derive(Debug, Clone, Serialize)]
pub struct FleetReportNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: FleetReportJson,
}

fn unix_ts_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Emit one compact JSON line and flush (piped stdout is block-buffered; AC-007.42).
fn emit_ndjson_line<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    std::io::stdout().flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Aggregation logic (pure function — easy to unit-test)
// ---------------------------------------------------------------------------

/// Sort a mutable slice of [`TopConsumer`] in-place according to `sort`.
///
/// - `SortBy::Memory` — descending by `memory_mb` (highest first).
/// - `SortBy::Name`   — ascending by `name` (alphabetical).
pub fn sort_consumers(consumers: &mut [TopConsumer], sort: &SortBy) {
    match sort {
        SortBy::Memory => consumers.sort_by_key(|c| Reverse(c.memory_mb)),
        SortBy::Name => consumers.sort_by(|a, b| a.name.cmp(&b.name)),
    }
}

/// Build a [`FleetReport`] from a slice of process snapshots.
///
/// `gate` carries live thermal + agent inventory gate fields (FR-011).
/// `sort` controls the order of `top_consumers`.
pub fn build_report(
    processes: &[ProcessInfo],
    gate: &GateStatusSnapshot,
    sort: &SortBy,
) -> FleetReport {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    let total_memory_mb: u64 = processes.iter().map(|p| p.memory_mb).sum();

    // Per-project breakdown
    let mut by_project: HashMap<String, ProjectBreakdown> = HashMap::new();
    for p in processes {
        let key = p.project.clone().unwrap_or_else(|| "<untagged>".to_string());
        let entry = by_project.entry(key).or_insert(ProjectBreakdown { count: 0, memory_mb: 0 });
        entry.count += 1;
        entry.memory_mb += p.memory_mb;
    }

    // Top-5 consumers, ordered by `sort`
    let mut candidates: Vec<TopConsumer> = processes
        .iter()
        .map(|p| TopConsumer {
            pid: p.pid,
            name: p.name.clone(),
            project: p.project.clone(),
            memory_mb: p.memory_mb,
        })
        .collect();
    // Always collect top-5 by memory first so we get the "most relevant" 5,
    // then re-sort by the requested key.
    candidates.sort_by_key(|c| Reverse(c.memory_mb));
    candidates.truncate(5);
    sort_consumers(&mut candidates, sort);
    let top_consumers = candidates;

    // Uptime: time since the earliest process started (0 if no processes)
    let earliest_start = processes.iter().map(|p| p.start_time).filter(|&t| t > 0).min();
    let uptime_seconds = earliest_start.map(|t| now.saturating_sub(t)).unwrap_or(0);

    FleetReport {
        timestamp: now,
        uptime_seconds,
        total_processes: processes.len(),
        total_memory_mb,
        by_project,
        top_consumers,
        thermal_pressure: gate.thermal_pressure.clone(),
        detected_agents: gate.detected_agents,
        agent_contention: gate.agent_contention.clone(),
        gate_decision: gate.gate_decision.clone(),
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn render_text(report: &FleetReport) {
    println!("=== Fleet Analytics Report ===");
    println!("Timestamp:       {}", report.timestamp);
    println!("Uptime:          {} s", report.uptime_seconds);
    println!("Thermal:         {}", report.thermal_pressure);
    println!("Detected agents: {}", report.detected_agents);
    println!("Agent contention: {}", report.agent_contention);
    println!("Gate decision:   {}", report.gate_decision);
    println!("Total processes: {}", report.total_processes);
    println!("Total memory:    {} MB", report.total_memory_mb);

    println!("\n--- Per-Project Breakdown ---");
    println!("{:<25} {:>8} {:>12}", "PROJECT", "PROCS", "MEM (MB)");
    println!("{}", "-".repeat(47));
    let mut projects: Vec<(&String, &ProjectBreakdown)> = report.by_project.iter().collect();
    projects.sort_by(|a, b| a.0.cmp(b.0));
    for (name, bd) in &projects {
        println!("{:<25} {:>8} {:>12}", name, bd.count, bd.memory_mb);
    }

    if !report.top_consumers.is_empty() {
        println!("\n--- Top Memory Consumers ---");
        println!("{:>8} {:<25} {:<20} {:>12}", "PID", "NAME", "PROJECT", "MEM (MB)");
        println!("{}", "-".repeat(67));
        for tc in &report.top_consumers {
            println!(
                "{:>8} {:<25} {:<20} {:>12}",
                tc.pid,
                tc.name,
                tc.project.as_deref().unwrap_or("-"),
                tc.memory_mb
            );
        }
    }
}

fn render_json(
    report: &FleetReport,
    gate: &GateStatusSnapshot,
    host_watch: &HostResourceWatchJson,
    pool: &PoolJson,
    status: &StatusJson,
) -> Result<()> {
    let payload = FleetReportJson::from_parts(
        report,
        gate.clone(),
        *host_watch,
        pool.clone(),
        status.clone(),
    );
    let json = serde_json::to_string_pretty(&payload)?;
    println!("{}", json);
    Ok(())
}

/// RFC 4180-style fleet analytics CSV body (summary + project + consumer sections).
pub fn render_report_csv_body(report: &FleetReport) -> String {
    use crate::commands::proc::csv_escape_field;

    let mut out = String::new();
    out.push_str(
        "record,timestamp,uptime_seconds,total_processes,total_memory_mb,thermal_pressure,detected_agents,agent_contention,gate_decision\n",
    );
    out.push_str(&format!(
        "summary,{},{},{},{},{},{},{},{}\n",
        report.timestamp,
        report.uptime_seconds,
        report.total_processes,
        report.total_memory_mb,
        csv_escape_field(&report.thermal_pressure),
        report.detected_agents,
        csv_escape_field(&report.agent_contention),
        csv_escape_field(&report.gate_decision),
    ));

    out.push_str("\nrecord,project,count,memory_mb\n");
    let mut projects: Vec<_> = report.by_project.iter().collect();
    projects.sort_by(|a, b| a.0.cmp(b.0));
    for (name, bd) in projects {
        out.push_str(&format!(
            "project,{},{},{}\n",
            csv_escape_field(name),
            bd.count,
            bd.memory_mb,
        ));
    }

    out.push_str("\nrecord,pid,name,project,memory_mb\n");
    for tc in &report.top_consumers {
        out.push_str(&format!(
            "consumer,{},{},{},{}\n",
            tc.pid,
            csv_escape_field(&tc.name),
            csv_escape_field(tc.project.as_deref().unwrap_or("-")),
            tc.memory_mb,
        ));
    }
    out
}

async fn append_report_csv_companions(csv: String, gate: &GateStatusSnapshot) -> Result<String> {
    use sharecli_fleet::{PoolOperatorPanel, StatusOperatorPanel};

    let mut out = csv;
    out.push_str(&gate.format_csv_companion());
    out.push_str(&HostResourceWatchJson::capture()?.format_csv_companion());
    let (pool_json, status_json) = super::fetch_operator_pool_status_siblings().await?;
    let pool: PoolOperatorPanel = pool_json.into();
    let status: StatusOperatorPanel = status_json.into();
    out.push_str(&pool.format_csv_companion());
    out.push_str(&status.format_csv_companion());
    Ok(out)
}

async fn render_csv(report: &FleetReport, gate: &GateStatusSnapshot) -> Result<()> {
    let body = render_report_csv_body(report);
    let csv = append_report_csv_companions(body, gate).await?;
    print!("{csv}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Render one snapshot and print it according to `format`.
async fn render_once(format: &ReportFormat, sort: &SortBy, ndjson: bool) -> Result<()> {
    let pool = ProcessPool::new();
    let processes = pool.list().await;

    // Live thermal + agent gate (FR-011)
    let gate = {
        use sharecli_fleet::thermal::ThermalGovernor;
        use sharecli_fleet::{count_host_agents, gate_status_snapshot};
        let gov = ThermalGovernor::new();
        match gov.poll() {
            Ok(level) => gate_status_snapshot(level, count_host_agents()),
            Err(_) => GateStatusSnapshot {
                thermal_pressure: "UNAVAILABLE".to_string(),
                detected_agents: count_host_agents(),
                agent_total_rss_bytes: 0,
                agent_contention: "UNAVAILABLE".to_string(),
                gate_decision: "UNAVAILABLE".to_string(),
            },
        }
    };

    let report = build_report(&processes, &gate, sort);

    match format {
        ReportFormat::Text => {
            render_text(&report);
            // AC-007.39 / AC-007.74: gate → host_watch → pool → proc-scan on stdout after report body.
            super::print_live_gate_section()?;
            super::print_live_host_watch_section()?;
            super::print_live_pool_status_operator_sections().await?;
        }
        ReportFormat::Json => {
            // AC-007.40/AC-007.73 one-shot / AC-007.42 watch NDJSON: gate → host_watch → pool → status.
            let (host_watch, pool_json, status_json) = tokio::join!(
                async { HostResourceWatchJson::capture() },
                super::build_pool_json(),
                super::build_status_json(),
            );
            let host_watch = host_watch?;
            let pool = pool_json?;
            let status = status_json?;
            if ndjson {
                let payload =
                    FleetReportJson::from_parts(&report, gate.clone(), host_watch, pool, status);
                let line = FleetReportNdjsonLine { ts: unix_ts_secs(), snapshot: payload };
                emit_ndjson_line(&line)?;
                super::eprint_live_gate_host_watch_sections()?;
            } else {
                render_json(&report, &gate, &host_watch, &pool, &status)?;
            }
        }
        ReportFormat::Csv => {
            // AC-007.81 one-shot: fleet CSV body → gate → host_watch → pool → status companions.
            render_csv(&report, &gate).await?;
        }
    }

    Ok(())
}

/// Run the report command.
///
/// - `watch`: if `Some(n)`, clear terminal and re-render every `n` seconds
///   until Ctrl-C; if `None`, run once and exit.
/// - `sort`: controls ordering of the top-consumers section.
pub async fn run(format: ReportFormat, watch: Option<u64>, sort: SortBy) -> Result<()> {
    match watch {
        None => render_once(&format, &sort, false).await,
        Some(interval_secs) => {
            if interval_secs == 0 {
                bail!("--watch interval must be >= 1 second");
            }
            let ndjson = format == ReportFormat::Json;
            let csv_watch = format == ReportFormat::Csv;
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    // AC-007.90: frame marker + full CSV body + `# [watch]` on stdout.
                    println!("{REPORT_CSV_WATCH_FRAME_MARKER}");
                }
                render_once(&format, &sort, ndjson).await?;
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_proc(
        pid: u32,
        name: &str,
        project: Option<&str>,
        memory_mb: u64,
        start_time: u64,
    ) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cmd: vec![],
            memory_mb,
            start_time,
            cpu_percent: 0.0,
            project: project.map(String::from),
            harness: None,
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

    fn gate(thermal: &str, agents: usize, contention: &str, decision: &str) -> GateStatusSnapshot {
        GateStatusSnapshot {
            thermal_pressure: thermal.to_string(),
            detected_agents: agents,
            agent_total_rss_bytes: 0,
            agent_contention: contention.to_string(),
            gate_decision: decision.to_string(),
        }
    }

    fn make_empty_gate() -> GateStatusSnapshot {
        gate("GREEN", 0, "OK", "ADMIT")
    }

    fn make_empty_pool_json() -> PoolJson {
        PoolJson {
            node_total: 0,
            node_idle: 0,
            bun_total: 0,
            bun_idle: 0,
            max_per_type: 0,
            healthy: true,
            issues: vec![],
            gate: make_empty_gate(),
            host_watch: HostResourceWatchJson::default(),
            status: None,
        }
    }

    fn make_empty_status_json() -> StatusJson {
        StatusJson {
            total_processes: 0,
            agents: vec![],
            scanned: 0,
            watched: 0,
            gate: make_empty_gate(),
            host_watch: HostResourceWatchJson::default(),
            pool: None,
            log_location: None,
        }
    }

    #[test]
    fn test_build_report_empty() {
        let report = build_report(&[], &gate("GREEN", 0, "OK", "ADMIT"), &SortBy::Memory);
        assert_eq!(report.total_processes, 0);
        assert_eq!(report.total_memory_mb, 0);
        assert!(report.by_project.is_empty());
        assert!(report.top_consumers.is_empty());
        assert_eq!(report.thermal_pressure, "GREEN");
        assert_eq!(report.detected_agents, 0);
        assert_eq!(report.gate_decision, "ADMIT");
    }

    #[test]
    fn test_build_report_aggregation() {
        let procs = vec![
            make_proc(1, "cargo", Some("alpha"), 300, 1_000_000),
            make_proc(2, "bun", Some("alpha"), 100, 1_000_100),
            make_proc(3, "node", Some("beta"), 200, 1_000_200),
            make_proc(4, "forge", None, 50, 1_000_300),
        ];
        let report = build_report(&procs, &gate("YELLOW", 2, "OK", "ADMIT"), &SortBy::Memory);

        assert_eq!(report.total_processes, 4);
        assert_eq!(report.total_memory_mb, 650);

        let alpha = report.by_project.get("alpha").expect("alpha missing");
        assert_eq!(alpha.count, 2);
        assert_eq!(alpha.memory_mb, 400);

        let beta = report.by_project.get("beta").expect("beta missing");
        assert_eq!(beta.count, 1);
        assert_eq!(beta.memory_mb, 200);

        let untagged = report.by_project.get("<untagged>").expect("untagged missing");
        assert_eq!(untagged.count, 1);
        assert_eq!(untagged.memory_mb, 50);
    }

    #[test]
    fn test_top_consumers_order_and_limit() {
        let procs: Vec<ProcessInfo> =
            (0u32..8).map(|i| make_proc(i, "proc", None, (i as u64 + 1) * 100, 0)).collect();
        let report = build_report(&procs, &gate("GREEN", 0, "OK", "ADMIT"), &SortBy::Memory);

        assert_eq!(report.top_consumers.len(), 5);
        // First element must be the highest memory consumer
        assert_eq!(report.top_consumers[0].memory_mb, 800);
        // Must be in descending order
        for w in report.top_consumers.windows(2) {
            assert!(w[0].memory_mb >= w[1].memory_mb);
        }
    }

    #[test]
    fn test_json_roundtrip() {
        let procs = vec![make_proc(10, "claude", Some("proj-a"), 512, 1_700_000_000)];
        let report = build_report(&procs, &gate("RED", 8, "REFUSE", "DENY"), &SortBy::Memory);
        let json = serde_json::to_string(&report).expect("serialize");
        let back: FleetReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.total_processes, report.total_processes);
        assert_eq!(back.total_memory_mb, report.total_memory_mb);
        assert_eq!(back.thermal_pressure, "RED");
        assert_eq!(back.detected_agents, 8);
        assert_eq!(back.agent_contention, "REFUSE");
        assert_eq!(back.gate_decision, "DENY");
        let pa = back.by_project.get("proj-a").unwrap();
        assert_eq!(pa.count, 1);
        assert_eq!(pa.memory_mb, 512);
    }

    // ------------------------------------------------------------------
    // Sort logic tests
    // ------------------------------------------------------------------

    #[test]
    fn test_sort_by_name_ascending() {
        let procs = vec![
            make_proc(1, "zebra", None, 500, 0),
            make_proc(2, "alpha", None, 100, 0),
            make_proc(3, "mango", None, 300, 0),
        ];
        let report = build_report(&procs, &gate("GREEN", 0, "OK", "ADMIT"), &SortBy::Name);
        let names: Vec<&str> = report.top_consumers.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mango", "zebra"]);
    }

    #[test]
    fn test_sort_by_memory_descending() {
        let procs = vec![
            make_proc(1, "low", None, 50, 0),
            make_proc(2, "high", None, 900, 0),
            make_proc(3, "mid", None, 400, 0),
        ];
        let report = build_report(&procs, &gate("GREEN", 0, "OK", "ADMIT"), &SortBy::Memory);
        let mems: Vec<u64> = report.top_consumers.iter().map(|c| c.memory_mb).collect();
        assert_eq!(mems, vec![900, 400, 50]);
    }

    #[test]
    fn test_sort_consumers_in_place() {
        let mut consumers = vec![
            TopConsumer { pid: 1, name: "zebra".into(), project: None, memory_mb: 10 },
            TopConsumer { pid: 2, name: "alpha".into(), project: None, memory_mb: 50 },
            TopConsumer { pid: 3, name: "mango".into(), project: None, memory_mb: 30 },
        ];
        sort_consumers(&mut consumers, &SortBy::Name);
        assert_eq!(consumers[0].name, "alpha");
        assert_eq!(consumers[1].name, "mango");
        assert_eq!(consumers[2].name, "zebra");

        sort_consumers(&mut consumers, &SortBy::Memory);
        assert_eq!(consumers[0].memory_mb, 50);
        assert_eq!(consumers[1].memory_mb, 30);
        assert_eq!(consumers[2].memory_mb, 10);
    }

    #[test]
    fn test_sort_by_from_str() {
        use std::str::FromStr;
        assert_eq!(SortBy::from_str("memory").unwrap(), SortBy::Memory);
        assert_eq!(SortBy::from_str("MEMORY").unwrap(), SortBy::Memory);
        assert_eq!(SortBy::from_str("name").unwrap(), SortBy::Name);
        assert_eq!(SortBy::from_str("NAME").unwrap(), SortBy::Name);
        assert!(SortBy::from_str("pid").is_err());
    }

    #[test]
    fn test_report_format_from_str() {
        use std::str::FromStr;
        assert_eq!(ReportFormat::from_str("text").unwrap(), ReportFormat::Text);
        assert_eq!(ReportFormat::from_str("TEXT").unwrap(), ReportFormat::Text);
        assert_eq!(ReportFormat::from_str("json").unwrap(), ReportFormat::Json);
        assert_eq!(ReportFormat::from_str("JSON").unwrap(), ReportFormat::Json);
        assert_eq!(ReportFormat::from_str("csv").unwrap(), ReportFormat::Csv);
        assert_eq!(ReportFormat::from_str("CSV").unwrap(), ReportFormat::Csv);
        assert!(ReportFormat::from_str("xml").is_err());
    }

    #[test]
    fn test_render_report_csv_body() {
        let mut by_project = HashMap::new();
        by_project.insert("alpha".into(), ProjectBreakdown { count: 2, memory_mb: 400 });
        let report = FleetReport {
            timestamp: 1_700_000_000,
            uptime_seconds: 3600,
            total_processes: 2,
            total_memory_mb: 400,
            by_project,
            top_consumers: vec![TopConsumer {
                pid: 42,
                name: "cargo".into(),
                project: Some("alpha".into()),
                memory_mb: 300,
            }],
            thermal_pressure: "GREEN".into(),
            detected_agents: 1,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let csv = render_report_csv_body(&report);
        assert!(
            csv.contains(
                "record,timestamp,uptime_seconds,total_processes,total_memory_mb,thermal_pressure,detected_agents,agent_contention,gate_decision"
            ),
            "CSV body MUST include summary header; got: {csv}"
        );
        assert!(
            csv.contains("summary,1700000000,3600,2,400,GREEN,1,OK,ADMIT"),
            "CSV body MUST include summary row; got: {csv}"
        );
        assert!(
            csv.contains("record,project,count,memory_mb"),
            "CSV body MUST include project header; got: {csv}"
        );
        assert!(
            csv.contains("project,alpha,2,400"),
            "CSV body MUST include project row; got: {csv}"
        );
        assert!(
            csv.contains("record,pid,name,project,memory_mb"),
            "CSV body MUST include consumer header; got: {csv}"
        );
        assert!(
            csv.contains("consumer,42,cargo,alpha,300"),
            "CSV body MUST include consumer row; got: {csv}"
        );
    }

    // --- FleetReportJson::from_parts ---

    #[test]
    fn test_fleet_report_json_from_parts_preserves_fields() {
        use sharecli_fleet::GateStatusSnapshot;

        let by_project =
            HashMap::from([("alpha".into(), ProjectBreakdown { count: 1, memory_mb: 100 })]);
        let report = FleetReport {
            timestamp: 1_700_000_000,
            uptime_seconds: 3600,
            total_processes: 1,
            total_memory_mb: 100,
            by_project,
            top_consumers: vec![TopConsumer {
                pid: 10,
                name: "cargo".into(),
                project: Some("alpha".into()),
                memory_mb: 100,
            }],
            thermal_pressure: "YELLOW".into(),
            detected_agents: 2,
            agent_contention: "WARN".into(),
            gate_decision: "ADMIT".into(),
        };
        let gate = GateStatusSnapshot {
            thermal_pressure: "YELLOW".into(),
            detected_agents: 2,
            agent_total_rss_bytes: 200,
            agent_contention: "WARN".into(),
            gate_decision: "ADMIT".into(),
        };
        let host_watch = HostResourceWatchJson::default();
        let pool = make_empty_pool_json();
        let status = make_empty_status_json();
        let json = FleetReportJson::from_parts(&report, gate, host_watch, pool, status);
        assert_eq!(json.timestamp, 1_700_000_000);
        assert_eq!(json.uptime_seconds, 3600);
        assert_eq!(json.total_processes, 1);
        assert_eq!(json.total_memory_mb, 100);
        assert_eq!(json.thermal_pressure, "YELLOW");
        assert_eq!(json.detected_agents, 2);
        assert_eq!(json.agent_contention, "WARN");
        assert_eq!(json.gate_decision, "ADMIT");
        assert_eq!(json.top_consumers.len(), 1);
        assert_eq!(json.top_consumers[0].pid, 10);
    }

    #[test]
    fn test_fleet_report_json_serializes_with_gate_host_watch_pool_status() {
        use sharecli_fleet::GateStatusSnapshot;

        let report = FleetReport {
            timestamp: 1_000_000,
            uptime_seconds: 0,
            total_processes: 0,
            total_memory_mb: 0,
            by_project: HashMap::new(),
            top_consumers: vec![],
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let gate = GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let json = FleetReportJson::from_parts(
            &report,
            gate,
            HostResourceWatchJson::default(),
            make_empty_pool_json(),
            make_empty_status_json(),
        );
        let serialized = serde_json::to_string(&json).expect("serialize");
        assert!(serialized.contains("\"gate\""), "MUST include gate field");
        assert!(serialized.contains("\"host_watch\""), "MUST include host_watch field");
        assert!(serialized.contains("\"pool\""), "MUST include pool field");
        assert!(serialized.contains("\"status\""), "MUST include status field");
    }

    // --- CSV body edge cases ---

    #[test]
    fn test_render_report_csv_body_empty_all_sections() {
        let report = FleetReport {
            timestamp: 1_700_000_000,
            uptime_seconds: 0,
            total_processes: 0,
            total_memory_mb: 0,
            by_project: HashMap::new(),
            top_consumers: vec![],
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let csv = render_report_csv_body(&report);
        assert!(csv.contains("summary,1700000000,0,0,0,GREEN,0,OK,ADMIT"));
        // Empty project section should have header but no rows
        assert!(csv.contains("record,project,count,memory_mb"));
        // Empty consumer section should have header but no rows
        assert!(csv.contains("record,pid,name,project,memory_mb"));
    }

    #[test]
    fn test_render_report_csv_body_multiple_projects_sorted() {
        let mut by_project = HashMap::new();
        by_project.insert("beta".into(), ProjectBreakdown { count: 1, memory_mb: 50 });
        by_project.insert("alpha".into(), ProjectBreakdown { count: 2, memory_mb: 200 });
        let report = FleetReport {
            timestamp: 1,
            uptime_seconds: 0,
            total_processes: 3,
            total_memory_mb: 250,
            by_project,
            top_consumers: vec![],
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let csv = render_report_csv_body(&report);
        let alpha_pos = csv.find("project,alpha").expect("alpha project row");
        let beta_pos = csv.find("project,beta").expect("beta project row");
        assert!(alpha_pos < beta_pos, "projects MUST be sorted alphabetically");
    }

    #[test]
    fn test_render_report_csv_body_consumer_no_project() {
        let report = FleetReport {
            timestamp: 1,
            uptime_seconds: 0,
            total_processes: 1,
            total_memory_mb: 50,
            by_project: HashMap::new(),
            top_consumers: vec![TopConsumer {
                pid: 1,
                name: "orphan".into(),
                project: None,
                memory_mb: 50,
            }],
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let csv = render_report_csv_body(&report);
        assert!(csv.contains("consumer,1,orphan,-,50"));
    }

    // --- REPORT_CSV_WATCH_FRAME_MARKER ---

    #[test]
    fn test_report_csv_watch_frame_marker_value() {
        assert_eq!(REPORT_CSV_WATCH_FRAME_MARKER, "# sharecli-report-watch-frame");
    }

    // --- build_report edge cases ---

    #[test]
    fn test_build_report_no_processes_no_memory() {
        let report = build_report(&[], &gate("GREEN", 0, "OK", "ADMIT"), &SortBy::Memory);
        assert_eq!(report.total_processes, 0);
        assert_eq!(report.total_memory_mb, 0);
        assert!(report.top_consumers.is_empty());
        assert!(report.by_project.is_empty());
    }

    #[test]
    fn test_build_report_all_same_project() {
        let procs = vec![
            make_proc(1, "a", Some("same"), 100, 0),
            make_proc(2, "b", Some("same"), 200, 0),
            make_proc(3, "c", Some("same"), 300, 0),
        ];
        let report = build_report(&procs, &gate("GREEN", 0, "OK", "ADMIT"), &SortBy::Memory);
        assert_eq!(report.by_project.len(), 1);
        let entry = report.by_project.get("same").unwrap();
        assert_eq!(entry.count, 3);
        assert_eq!(entry.memory_mb, 600);
    }

    #[test]
    fn test_top_consumers_tied_memory_stable_order() {
        let procs = vec![
            make_proc(1, "a", None, 100, 0),
            make_proc(2, "b", None, 100, 0),
            make_proc(3, "c", None, 100, 0),
        ];
        let report = build_report(&procs, &gate("GREEN", 0, "OK", "ADMIT"), &SortBy::Memory);
        assert_eq!(report.top_consumers.len(), 3);
    }
}
