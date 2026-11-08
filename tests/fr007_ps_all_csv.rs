// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli ps --all --csv` operator companion rows (AC-007.83)
//! FR: FR-007
//!
//! ps --all --csv emits managed-process + agent-inventory CSV bodies followed by companion
//! gate → host_watch → pool → status records (parity with health/pool/status CSV AC-007.82).

use std::process::Command;

use sharecli::commands::proc::AgentProcRow;
use sharecli::commands::render_ps_all_csv_body;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const MANAGED_CSV_HEADER: &str = "record,pid,name,memory_mb,project,harness,agent";

const AGENT_INVENTORY_CSV_HEADER: &str = "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count";

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";

const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

const POOL_COMPANION_CSV_HEADER: &str =
    "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";

const STATUS_COMPANION_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

fn assert_csv_companion_order(stdout: &str, context: &str) {
    assert!(
        stdout.contains(GATE_CSV_HEADER),
        "{context} MUST include gate CSV header (AC-007.83); got: {stdout}"
    );
    assert!(
        stdout.contains(HOST_CSV_HEADER),
        "{context} MUST include host_watch CSV header (AC-007.83); got: {stdout}"
    );
    assert!(
        stdout.contains(POOL_COMPANION_CSV_HEADER),
        "{context} MUST include pool CSV header (AC-007.83); got: {stdout}"
    );
    assert!(
        stdout.contains(STATUS_COMPANION_CSV_HEADER),
        "{context} MUST include status CSV header (AC-007.83); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("gate,")),
        "{context} MUST include gate companion row (AC-007.83); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("host,")),
        "{context} MUST include host companion row (AC-007.83); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("pool,")),
        "{context} MUST include pool companion row (AC-007.83); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("status,")),
        "{context} MUST include status companion row (AC-007.83); got: {stdout}"
    );

    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    let host_pos = stdout[gate_pos..]
        .find(HOST_CSV_HEADER)
        .map(|p| gate_pos + p)
        .expect("host header after gate");
    let pool_pos = stdout[host_pos..]
        .find(POOL_COMPANION_CSV_HEADER)
        .map(|p| host_pos + p)
        .expect("pool companion header after host");
    let status_pos = stdout[pool_pos..]
        .find(STATUS_COMPANION_CSV_HEADER)
        .map(|p| pool_pos + p)
        .expect("status companion header after pool");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.83); got: {stdout}"
    );
}

fn assert_body_precedes_companions(stdout: &str, body_header: &str, context: &str) {
    let body_pos = stdout.find(body_header).expect("body header");
    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    assert!(
        body_pos < gate_pos,
        "{context} MUST serialize command body before gate companions (AC-007.83); got: {stdout}"
    );
}

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch text on stderr (AC-007.83); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

/// FR-007 / AC-007.83 — CLI ps --all --csv appends gate → host_watch → pool → status companions.
#[test]
fn fr007_ps_all_csv_companion_order() {
    let out = bin().args(["ps", "--all", "--csv"]).output().expect("spawn sharecli ps --all --csv");
    assert!(out.status.success(), "ps --all --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert_body_precedes_companions(&s, MANAGED_CSV_HEADER, "ps --all --csv");
    assert!(
        s.contains(AGENT_INVENTORY_CSV_HEADER),
        "ps --all --csv MUST include agent inventory CSV header (AC-007.83); got: {s}"
    );
    assert_csv_companion_order(&s, "ps --all --csv");
    assert_stderr_silent(&out.stderr, "ps --all --csv");
}

/// FR-007 / AC-007.83 — ps --all --csv rejects --json.
#[test]
fn fr007_ps_all_csv_rejects_json() {
    let out = bin()
        .args(["ps", "--all", "--csv", "--json"])
        .output()
        .expect("spawn sharecli ps --all --csv --json");
    assert!(!out.status.success(), "ps --all --csv --json MUST fail loudly (AC-007.83)");
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.to_ascii_lowercase().contains("json") || combined.contains("--csv"),
        "MUST reject --csv with --json; got: {combined}"
    );
}

/// FR-007 / AC-007.83 — ps --csv without --all fails loudly.
#[test]
fn fr007_ps_csv_requires_all() {
    let out = bin().args(["ps", "--csv"]).output().expect("spawn sharecli ps --csv");
    assert!(!out.status.success(), "ps --csv without --all MUST fail loudly (AC-007.83)");
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.contains("--all") || combined.contains("AC-007.83"),
        "MUST require --all for --csv; got: {combined}"
    );
}

/// FR-007 / AC-007.83 — unit render_ps_all_csv_body preserves managed + agent sections.
#[test]
fn fr007_ps_all_csv_body_shape() {
    use sharecli::runtime::ProcState;
    use sharecli::runtime::ProcessInfo;
    use sharecli_fleet::HostProcSource;

    let proc_source = HostProcSource;
    let processes = vec![ProcessInfo {
        pid: 42,
        name: "test-agent".into(),
        memory_mb: 128,
        project: Some("demo".into()),
        harness: Some("claude".into()),
        cmd: vec![],
        start_time: 0,
        cpu_percent: 0.0,
        ppid: None,
        cwd: None,
        env_count: 0,
        state: ProcState::default(),
        disk_read_bytes: None,
        disk_write_bytes: None,
        fd_count: None,
        thread_count: None,
    }];
    let agents = vec![AgentProcRow {
        pid: 99,
        family: "claude".into(),
        comm: "claude".into(),
        state: "S".into(),
        mem_rss_bytes: 1_048_576,
        mem_rss: "1.0M".into(),
        fd_count: Some(12),
    }];
    let body = render_ps_all_csv_body(&processes, &proc_source, &agents, 1, 1);
    assert!(body.contains(MANAGED_CSV_HEADER));
    assert!(body.contains("process,42,test-agent,128,demo,claude,"));
    assert!(body.contains("summary,1,128"));
    assert!(body.contains("agent_inventory,1,1"));
    assert!(body.contains(AGENT_INVENTORY_CSV_HEADER));
    assert!(body.contains("99,claude,claude,S,1048576,1.0M,12"));
}
