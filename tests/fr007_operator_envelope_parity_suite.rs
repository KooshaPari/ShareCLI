// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — operator envelope matrix parity regression suite (AC-007.84, proc --tree AC-007.85,
//! proc --pid AC-007.86, CSV watch frame smoke AC-007.93, same-tick footer flush AC-007.95/97/98)
//! FR: FR-007
//!
//! Locks the full FR-007 operator envelope across proc/report/health/pool/status/ps --all
//! (text/JSON/CSV one-shot, including `proc --tree` and `proc --pid`), IPC, WS decode, dashboard,
//! tray, and thermal TUI companion markers. CSV `--watch` frame markers get a short smoke pass
//! (AC-007.93) plus same-tick `# [watch]` flush coverage (AC-007.95); text `--watch` same-tick
//! flush is locked by AC-007.97/98. Long multi-frame dwell cycles stay in per-AC integration files.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use sharecli_fleet::{PoolOperatorPanel, StatusOperatorPanel};
use sharecli_thermal_tui::{pool_panel_lines, status_panel_lines, HELP_OVERLAY_HINT};
use sharecli_tray_linux::ipc::{
    GateStatusSnapshot, HostResourceWatchJson, PoolSnapshot, StatusSnapshot,
};
use sharecli_tray_linux::operator_display as linux_display;
use sharecli_tray_windows::operator_display as win_display;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn manifest_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";
const POOL_PREFIX: &str = "Pool node";
const PROC_PREFIX: &str = "Proc scan";

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";
const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";
const POOL_COMPANION_CSV_HEADER: &str =
    "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";
const STATUS_COMPANION_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

struct TextCase {
    label: &'static str,
    args: &'static [&'static str],
    body_header: &'static str,
}

struct JsonCase {
    label: &'static str,
    args: &'static [&'static str],
    mode: JsonEnvelopeMode,
}

#[derive(Clone, Copy)]
enum JsonEnvelopeMode {
    FullPoolStatus,
    PoolTopStatusNested,
    StatusTopPoolNested,
}

struct CsvCase {
    label: &'static str,
    args: &'static [&'static str],
    body_header: &'static str,
}

struct CsvWatchCase {
    label: &'static str,
    args: &'static [&'static str],
    body_header: &'static str,
    frame_marker: &'static str,
}

struct TextWatchCase {
    label: &'static str,
    args: &'static [&'static str],
    body_header: &'static str,
}

/// Drain watch pipes, stopping as soon as `ready` matches stdout (or `max_dwell`).
/// Keeps AC-007.93 single-frame smoke short on fast hosts without flaking on slow proc scans.
fn drain_watch_until(
    child: &mut Child,
    max_dwell: Duration,
    mut ready: impl FnMut(&str) -> bool,
) -> (String, String) {
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let out_arc = Arc::clone(&stdout_buf);
    let err_arc = Arc::clone(&stderr_buf);
    let stdout_reader = thread::spawn(move || {
        let mut chunk = [0u8; 16_384];
        let mut out = stdout;
        loop {
            match out.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => out_arc
                    .lock()
                    .expect("stdout lock")
                    .push_str(&String::from_utf8_lossy(&chunk[..n])),
                Err(_) => break,
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        let mut err = stderr;
        loop {
            match err.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => err_arc
                    .lock()
                    .expect("stderr lock")
                    .push_str(&String::from_utf8_lossy(&chunk[..n])),
                Err(_) => break,
            }
        }
    });
    let deadline = Instant::now() + max_dwell;
    while Instant::now() < deadline {
        let snapshot = stdout_buf.lock().expect("stdout lock").clone();
        if ready(&snapshot) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    let stdout = stdout_buf.lock().expect("stdout lock").clone();
    let stderr = stderr_buf.lock().expect("stderr lock").clone();
    (stdout, stderr)
}

fn assert_csv_watch_frame_smoke(
    args: &[&str],
    frame_marker: &str,
    body_header: &str,
    context: &str,
) {
    let mut child = bin()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {context}: {e}"));
    // Adaptive single-frame smoke: stop once one complete frame is visible.
    // Cap stays above slow proc scans; multi-frame (>=2) dwells stay in per-AC files.
    let marker = frame_marker.to_string();
    let header = body_header.to_string();
    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(45), |buf| {
        buf.split(marker.as_str())
            .skip(1)
            .any(|f| f.contains(header.as_str()) && f.contains("[watch]"))
    });
    assert!(stderr.is_empty(), "{context} MUST keep stderr silent (AC-007.93); stderr: {stderr:?}");
    assert!(
        !stdout.contains("\x1b[2J"),
        "{context} MUST NOT emit ANSI clear (AC-007.93); got: {stdout}"
    );
    assert!(
        stdout.contains(frame_marker),
        "{context} MUST emit frame marker {frame_marker} (AC-007.93); got: {stdout}"
    );
    let frame = stdout
        .split(frame_marker)
        .skip(1)
        .find(|f| f.contains(body_header))
        .unwrap_or_else(|| panic!("{context} MUST emit >=1 complete CSV frame; got: {stdout}"));
    assert_csv_companion_order(frame, body_header, &format!("{context} frame"));
    assert!(
        frame.contains("[watch]") || stdout.contains("[watch]"),
        "{context} MUST include [watch] footer comment (AC-007.93); got: {stdout}"
    );
}

/// AC-007.95 — `# [watch]` must flush in the same tick (one frame marker before first footer).
fn assert_csv_watch_footer_same_tick(
    args: &[&str],
    frame_marker: &str,
    body_header: &str,
    context: &str,
) {
    let mut child = bin()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {context}: {e}"));
    let marker = frame_marker.to_string();
    let header = body_header.to_string();
    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(45), |buf| {
        buf.contains(marker.as_str()) && buf.contains(header.as_str()) && buf.contains("[watch]")
    });
    assert!(stderr.is_empty(), "{context} MUST keep stderr silent (AC-007.95); stderr: {stderr:?}");
    let watch_pos = stdout.find("[watch]").unwrap_or_else(|| {
        panic!("{context} MUST include [watch] footer (AC-007.95); got: {stdout}")
    });
    let markers_before = stdout[..watch_pos].matches(frame_marker).count();
    assert_eq!(
        markers_before, 1,
        "{context} MUST flush `# [watch]` in the same tick (AC-007.95); got {markers_before} markers before footer in: {stdout}"
    );
}

const CSV_WATCH_MATRIX: &[CsvWatchCase] = &[
    CsvWatchCase {
        label: "proc --csv --watch",
        args: &["proc", "--csv", "--watch", "1"],
        body_header: "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        frame_marker: "# sharecli-proc-watch-frame",
    },
    CsvWatchCase {
        label: "proc --tree --csv --watch",
        args: &["proc", "--tree", "--csv", "--watch", "1"],
        body_header: "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        frame_marker: "# sharecli-proc-watch-frame",
    },
    CsvWatchCase {
        label: "report --format csv --watch",
        args: &["report", "--format", "csv", "--watch", "1"],
        body_header: "record,timestamp,uptime_seconds,total_processes,total_memory_mb,thermal_pressure,detected_agents,agent_contention,gate_decision",
        frame_marker: "# sharecli-report-watch-frame",
    },
    CsvWatchCase {
        label: "health --csv --watch",
        args: &["health", "--csv", "--watch", "1"],
        body_header: "record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues",
        frame_marker: "# sharecli-health-watch-frame",
    },
    CsvWatchCase {
        label: "pool --csv --watch",
        args: &["pool", "--csv", "--watch", "1"],
        body_header: "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues",
        frame_marker: "# sharecli-pool-watch-frame",
    },
    CsvWatchCase {
        label: "status --csv --watch",
        args: &["status", "--csv", "--watch", "1"],
        body_header: "record,total_processes,scanned,watched,agent_rows",
        frame_marker: "# sharecli-status-watch-frame",
    },
    CsvWatchCase {
        label: "ps --all --csv --watch",
        args: &["ps", "--all", "--csv", "--watch", "1"],
        body_header: "record,pid,name,memory_mb,project,harness,agent",
        frame_marker: "# sharecli-ps-watch-frame",
    },
];

const TEXT_WATCH_MATRIX: &[TextWatchCase] = &[
    TextWatchCase {
        label: "proc --watch",
        args: &["proc", "--watch", "1"],
        body_header: "=== Host agents (proc scan) ===",
    },
    TextWatchCase {
        label: "proc --tree --watch",
        args: &["proc", "--tree", "--watch", "1"],
        body_header: "=== Agent process tree (proc scan) ===",
    },
    TextWatchCase {
        label: "report --watch",
        args: &["report", "--watch", "1"],
        body_header: "=== Fleet Analytics Report ===",
    },
    TextWatchCase {
        label: "health --watch",
        args: &["health", "--watch", "1"],
        body_header: "Shared runtime health:",
    },
    TextWatchCase {
        label: "pool --watch",
        args: &["pool", "--watch", "1"],
        body_header: "=== Shared Runtime Pool Status ===",
    },
    TextWatchCase {
        label: "status --watch",
        args: &["status", "--watch", "1"],
        body_header: "=== Process Status ===",
    },
    TextWatchCase {
        label: "ps --all --watch",
        args: &["ps", "--all", "--watch", "1"],
        body_header: "=== Host agents (proc scan) ===",
    },
];

/// AC-007.97 — text `[watch]` must flush in the same tick (one gate before first footer).
fn assert_text_watch_footer_same_tick(args: &[&str], body_header: &str, context: &str) {
    let mut child = bin()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {context}: {e}"));
    let header = body_header.to_string();
    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(45), |buf| {
        buf.contains(header.as_str()) && buf.contains("[watch]")
    });
    assert!(stderr.is_empty(), "{context} MUST keep stderr silent (AC-007.97); stderr: {stderr:?}");
    let watch_pos = stdout.find("[watch]").unwrap_or_else(|| {
        panic!("{context} MUST include [watch] footer (AC-007.97); got: {stdout}")
    });
    let gates_before = stdout[..watch_pos].matches(GATE_MARKER).count();
    assert_eq!(
        gates_before, 1,
        "{context} MUST flush `[watch]` in the same tick (AC-007.97); got {gates_before} gates before footer in: {stdout}"
    );
}

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST keep stderr silent on success (AC-007.84); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_text_envelope(stdout: &str, body_header: &str, context: &str) {
    assert!(
        stdout.contains(body_header),
        "{context} MUST include command body (AC-007.84); got: {stdout}"
    );
    assert!(stdout.contains(GATE_MARKER), "{context} MUST include gate (AC-007.84)");
    assert!(stdout.contains(WATCH_MARKER), "{context} MUST include host_watch (AC-007.84)");
    assert!(stdout.contains(POOL_PREFIX), "{context} MUST include pool line (AC-007.84)");
    assert!(stdout.contains(PROC_PREFIX), "{context} MUST include proc-scan line (AC-007.84)");

    let body_pos = stdout.find(body_header).expect("body header");
    let gate_pos = stdout.find(GATE_MARKER).expect("gate");
    let watch_pos = stdout.find(WATCH_MARKER).expect("host_watch");
    let pool_pos = stdout.find(POOL_PREFIX).expect("pool");
    let proc_pos = stdout.find(PROC_PREFIX).expect("proc-scan");
    assert!(
        body_pos < gate_pos && gate_pos < watch_pos && watch_pos < pool_pos && pool_pos < proc_pos,
        "{context} MUST serialize body → gate → host_watch → pool → proc-scan (AC-007.84); got: {stdout}"
    );
}

fn assert_pool_object(pool: &serde_json::Value, context: &str) {
    assert!(
        pool.get("node_total").is_some() && pool.get("healthy").is_some(),
        "{context} pool MUST include capacity fields (AC-007.84); got: {pool}"
    );
}

fn assert_status_object(status: &serde_json::Value, context: &str) {
    assert!(
        status.get("total_processes").is_some()
            && status.get("scanned").is_some()
            && status.get("watched").is_some(),
        "{context} status MUST include proc-scan fields (AC-007.84); got: {status}"
    );
}

fn assert_json_gate_host_watch(raw: &str, v: &serde_json::Value, context: &str) {
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.84)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.84)");
    assert!(gate.get("gate_decision").is_some(), "gate MUST include gate_decision (AC-007.84)");
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.84); got: {host}");
    }
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.84); got: {raw}"
    );
}

fn assert_json_envelope(raw: &str, mode: JsonEnvelopeMode, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON (AC-007.84)");
    assert_json_gate_host_watch(raw, &v, context);

    match mode {
        JsonEnvelopeMode::FullPoolStatus => {
            let pool = v.get("pool").expect("{context} MUST include pool (AC-007.84)");
            let status = v.get("status").expect("{context} MUST include status (AC-007.84)");
            assert_pool_object(pool, context);
            assert_status_object(status, context);
            let host_pos = raw.find("\"host_watch\"").expect("host_watch key");
            let pool_pos = raw.find("\"pool\"").expect("pool key (AC-007.84)");
            let status_pos = raw.find("\"status\"").expect("status key (AC-007.84)");
            assert!(
                host_pos < pool_pos && pool_pos < status_pos,
                "{context} MUST serialize gate → host_watch → pool → status (AC-007.84); got: {raw}"
            );
        }
        JsonEnvelopeMode::PoolTopStatusNested => {
            assert!(
                v.get("node_total").is_some(),
                "{context} top-level MUST include pool panel fields (AC-007.84); got: {v}"
            );
            let status = v.get("status").expect("{context} MUST include nested status (AC-007.84)");
            assert_status_object(status, context);
            let host_pos = raw.find("\"host_watch\"").expect("host_watch key");
            let status_pos = raw.rfind("\"status\"").expect("status key (AC-007.84)");
            assert!(
                host_pos < status_pos,
                "{context} MUST serialize host_watch before nested status (AC-007.84); got: {raw}"
            );
        }
        JsonEnvelopeMode::StatusTopPoolNested => {
            assert!(
                v.get("total_processes").is_some(),
                "{context} top-level MUST include proc-scan fields (AC-007.84); got: {v}"
            );
            let pool = v.get("pool").expect("{context} MUST include nested pool (AC-007.84)");
            assert_pool_object(pool, context);
            let host_pos = raw.find("\"host_watch\"").expect("host_watch key");
            let pool_pos = raw.rfind("\"pool\"").expect("pool key (AC-007.84)");
            assert!(
                host_pos < pool_pos,
                "{context} MUST serialize host_watch before nested pool (AC-007.84); got: {raw}"
            );
        }
    }
}

fn assert_csv_companion_order(stdout: &str, body_header: &str, context: &str) {
    assert!(
        stdout.contains(body_header),
        "{context} MUST include command CSV body (AC-007.84); got: {stdout}"
    );
    for header in
        [GATE_CSV_HEADER, HOST_CSV_HEADER, POOL_COMPANION_CSV_HEADER, STATUS_COMPANION_CSV_HEADER]
    {
        assert!(stdout.contains(header), "{context} MUST include {header} (AC-007.84)");
    }
    for prefix in ["gate,", "host,", "pool,", "status,"] {
        assert!(
            stdout.lines().any(|line| line.starts_with(prefix)),
            "{context} MUST include {prefix} companion row (AC-007.84); got: {stdout}"
        );
    }

    let body_pos = stdout.find(body_header).expect("body header");
    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    let host_pos = stdout[gate_pos..]
        .find(HOST_CSV_HEADER)
        .map(|p| gate_pos + p)
        .expect("host header after gate");
    let pool_pos = stdout[host_pos..]
        .find(POOL_COMPANION_CSV_HEADER)
        .map(|p| host_pos + p)
        .expect("pool companion after host");
    let status_pos = stdout[pool_pos..]
        .find(STATUS_COMPANION_CSV_HEADER)
        .map(|p| pool_pos + p)
        .expect("status companion after pool");

    assert!(
        body_pos < gate_pos
            && gate_pos < host_pos
            && host_pos < pool_pos
            && pool_pos < status_pos,
        "{context} MUST serialize body → gate → host_watch → pool → status (AC-007.84); got: {stdout}"
    );
}

fn run_cli(args: &[&str]) -> std::process::Output {
    bin().args(args).output().expect("spawn sharecli")
}

fn self_pid() -> String {
    std::process::id().to_string()
}

fn run_proc_pid(suffix: &[&str]) -> std::process::Output {
    let pid = self_pid();
    let mut args = vec!["proc", "--pid", pid.as_str()];
    args.extend_from_slice(suffix);
    run_cli(&args)
}

const PROC_PID_BODY_HEADER: &str = "=== Process detail (PID ";
const PROC_PID_CSV_HEADER: &str = "pid,ppid,comm,state,mem_rss_bytes,mem_rss,fd_count";

fn sample_pool_snapshot() -> PoolSnapshot {
    PoolSnapshot {
        node_total: 2,
        node_idle: 1,
        bun_total: 1,
        bun_idle: 0,
        max_per_type: 4,
        healthy: true,
        issues: vec![],
        gate: GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        },
        host_watch: HostResourceWatchJson {
            fd_count: 1,
            net_rx_bytes: 2,
            net_tx_bytes: 3,
            mem_rss_bytes: 4,
            load_1m: 0.5,
        },
    }
}

fn sample_status_snapshot() -> StatusSnapshot {
    StatusSnapshot {
        total_processes: 2,
        agents: vec![],
        scanned: 50,
        watched: 1,
        gate: GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        },
        host_watch: HostResourceWatchJson {
            fd_count: 1,
            net_rx_bytes: 2,
            net_tx_bytes: 3,
            mem_rss_bytes: 4,
            load_1m: 0.5,
        },
    }
}

/// FR-007 / AC-007.84 — CLI text one-shot matrix: body → gate → host_watch → pool → proc-scan.
#[test]
#[serial_test::serial]
fn fr007_operator_matrix_cli_text_one_shot() {
    let cases = [
        TextCase {
            label: "report",
            args: &["report"],
            body_header: "=== Fleet Analytics Report ===",
        },
        TextCase { label: "proc", args: &["proc"], body_header: "=== Host agents (proc scan) ===" },
        TextCase {
            label: "proc --tree",
            args: &["proc", "--tree"],
            body_header: "=== Agent process tree (proc scan) ===",
        },
        TextCase { label: "health", args: &["health"], body_header: "Shared runtime health:" },
        TextCase {
            label: "pool",
            args: &["pool"],
            body_header: "=== Shared Runtime Pool Status ===",
        },
        TextCase { label: "status", args: &["status"], body_header: "=== Process Status ===" },
        TextCase {
            label: "ps --all",
            args: &["ps", "--all"],
            body_header: "=== Host agents (proc scan) ===",
        },
    ];

    for case in cases {
        let out = run_cli(case.args);
        assert!(out.status.success(), "{} MUST exit 0 (AC-007.84)", case.label);
        assert_stderr_silent(&out.stderr, case.label);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_text_envelope(&stdout, case.body_header, case.label);
    }

    let out = run_proc_pid(&[]);
    assert!(out.status.success(), "proc --pid MUST exit 0 (AC-007.86)");
    assert_stderr_silent(&out.stderr, "proc --pid");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_text_envelope(&stdout, PROC_PID_BODY_HEADER, "proc --pid");
}

/// FR-007 / AC-007.84 — CLI JSON one-shot matrix: gate → host_watch → pool/status siblings.
#[test]
#[serial_test::serial]
fn fr007_operator_matrix_cli_json_one_shot() {
    let cases = [
        JsonCase {
            label: "health --json",
            args: &["health", "--json"],
            mode: JsonEnvelopeMode::FullPoolStatus,
        },
        JsonCase {
            label: "pool --json",
            args: &["pool", "--json"],
            mode: JsonEnvelopeMode::PoolTopStatusNested,
        },
        JsonCase {
            label: "status --json",
            args: &["status", "--json"],
            mode: JsonEnvelopeMode::StatusTopPoolNested,
        },
        JsonCase {
            label: "ps --all --json",
            args: &["ps", "--all", "--json"],
            mode: JsonEnvelopeMode::FullPoolStatus,
        },
        JsonCase {
            label: "proc --json",
            args: &["proc", "--json"],
            mode: JsonEnvelopeMode::FullPoolStatus,
        },
        JsonCase {
            label: "proc --tree --json",
            args: &["proc", "--tree", "--json"],
            mode: JsonEnvelopeMode::FullPoolStatus,
        },
        JsonCase {
            label: "report --format json",
            args: &["report", "--format", "json"],
            mode: JsonEnvelopeMode::FullPoolStatus,
        },
    ];

    for case in cases {
        let out = run_cli(case.args);
        assert!(out.status.success(), "{} MUST exit 0 (AC-007.84)", case.label);
        assert_stderr_silent(&out.stderr, case.label);
        let raw = String::from_utf8_lossy(&out.stdout);
        assert_json_envelope(&raw, case.mode, case.label);
    }

    let out = run_proc_pid(&["--json"]);
    assert!(out.status.success(), "proc --pid --json MUST exit 0 (AC-007.86)");
    assert_stderr_silent(&out.stderr, "proc --pid --json");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_envelope(&raw, JsonEnvelopeMode::FullPoolStatus, "proc --pid --json");
}

/// FR-007 / AC-007.84 — CLI CSV one-shot matrix: body → gate → host_watch → pool → status.
#[test]
#[serial_test::serial]
fn fr007_operator_matrix_cli_csv_one_shot() {
    let cases = [
        CsvCase {
            label: "proc --csv",
            args: &["proc", "--csv"],
            body_header: "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        },
        CsvCase {
            label: "proc --tree --csv",
            args: &["proc", "--tree", "--csv"],
            body_header: "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        },
        CsvCase {
            label: "report --format csv",
            args: &["report", "--format", "csv"],
            body_header: "record,timestamp,uptime_seconds,total_processes,total_memory_mb,thermal_pressure,detected_agents,agent_contention,gate_decision",
        },
        CsvCase {
            label: "health --csv",
            args: &["health", "--csv"],
            body_header: "record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues",
        },
        CsvCase {
            label: "pool --csv",
            args: &["pool", "--csv"],
            body_header: "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues",
        },
        CsvCase {
            label: "status --csv",
            args: &["status", "--csv"],
            body_header: "record,total_processes,scanned,watched,agent_rows",
        },
        CsvCase {
            label: "ps --all --csv",
            args: &["ps", "--all", "--csv"],
            body_header: "record,pid,name,memory_mb,project,harness,agent",
        },
    ];

    for case in cases {
        let out = run_cli(case.args);
        assert!(out.status.success(), "{} MUST exit 0 (AC-007.84)", case.label);
        assert_stderr_silent(&out.stderr, case.label);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_csv_companion_order(&stdout, case.body_header, case.label);
    }

    let out = run_proc_pid(&["--csv"]);
    assert!(out.status.success(), "proc --pid --csv MUST exit 0 (AC-007.86)");
    assert_stderr_silent(&out.stderr, "proc --pid --csv");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_csv_companion_order(&stdout, PROC_PID_CSV_HEADER, "proc --pid --csv");
}

/// FR-007 / AC-007.93 — CSV `--watch` frame-marker smoke (short dwell, >=1 frame).
#[test]
#[serial_test::serial]
fn fr007_operator_matrix_cli_csv_watch_frame_smoke() {
    for case in CSV_WATCH_MATRIX {
        assert_csv_watch_frame_smoke(case.args, case.frame_marker, case.body_header, case.label);
    }

    let self_pid = std::process::id().to_string();
    let pid_args = ["proc", "--pid", self_pid.as_str(), "--csv", "--watch", "1"];
    assert_csv_watch_frame_smoke(
        &pid_args,
        "# sharecli-proc-pid-watch-frame",
        PROC_PID_CSV_HEADER,
        "proc --pid --csv --watch",
    );
}

/// FR-007 / AC-007.95 — CSV `--watch` `# [watch]` footer flushes in the same tick (full matrix).
#[test]
#[serial_test::serial]
fn fr007_operator_matrix_cli_csv_watch_footer_same_tick() {
    for case in CSV_WATCH_MATRIX {
        assert_csv_watch_footer_same_tick(
            case.args,
            case.frame_marker,
            case.body_header,
            case.label,
        );
    }

    let self_pid = std::process::id().to_string();
    let pid_args = ["proc", "--pid", self_pid.as_str(), "--csv", "--watch", "1"];
    assert_csv_watch_footer_same_tick(
        &pid_args,
        "# sharecli-proc-pid-watch-frame",
        PROC_PID_CSV_HEADER,
        "proc --pid --csv --watch",
    );
}

/// FR-007 / AC-007.97 — text `--watch` `[watch]` footer flushes in the same tick (full matrix).
#[test]
#[serial_test::serial]
fn fr007_operator_matrix_cli_text_watch_footer_same_tick() {
    for case in TEXT_WATCH_MATRIX {
        assert_text_watch_footer_same_tick(case.args, case.body_header, case.label);
    }

    let self_pid = std::process::id().to_string();
    let pid_args = ["proc", "--pid", self_pid.as_str(), "--watch", "1"];
    assert_text_watch_footer_same_tick(&pid_args, PROC_PID_BODY_HEADER, "proc --pid --watch");
}

/// FR-007 / AC-007.84 — IPC health.status + monitoring.report carry gate → host_watch → pool/status.
#[tokio::test]
async fn fr007_operator_matrix_ipc_envelopes() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");

    let health = handler.dispatch(r#"{"id":1,"method":"health.status","params":{}}"#).await;
    assert!(health.error.is_none(), "health.status MUST succeed (AC-007.84)");
    let health_raw = serde_json::to_string(&health.result).expect("serialize health.status");
    assert_json_envelope(&health_raw, JsonEnvelopeMode::FullPoolStatus, "health.status");

    let report = handler.dispatch(r#"{"id":2,"method":"monitoring.report","params":{}}"#).await;
    assert!(report.error.is_none(), "monitoring.report MUST succeed (AC-007.84)");
    let report_raw = serde_json::to_string(&report.result).expect("serialize monitoring.report");
    assert_json_envelope(&report_raw, JsonEnvelopeMode::FullPoolStatus, "monitoring.report");
}

/// FR-007 / AC-007.84 — WS health_update decode stays on the expanded operator envelope path.
#[test]
fn fr007_operator_matrix_ws_health_update_decode() {
    use sharecli_ipc::ws_client::ClientMessage;

    let health = r#"{
        "managed_processes":3,"used_memory_mb":2048,"total_memory_mb":16384,"healthy":true,
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,"mem_rss_bytes":4,"load_1m":0.5},
        "pool":{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
        "healthy":true,"issues":[],
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,"mem_rss_bytes":4,"load_1m":0.5}},
        "status":{"total_processes":2,"agents":[],"scanned":50,"watched":1,
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,"mem_rss_bytes":4,"load_1m":0.5}}
    }"#;
    let raw = format!(r#"{{"type":"health_update","health":{health}}}"#);
    match ClientMessage::from_json(&raw) {
        ClientMessage::HealthUpdate(h) => {
            assert_eq!(h.pool.node_total, 2);
            assert_eq!(h.status.scanned, 50);
            assert_json_envelope(health, JsonEnvelopeMode::FullPoolStatus, "HealthUpdate wire");
        }
        other => panic!("expected HealthUpdate, got {other:?} (AC-007.84)"),
    }
}

/// FR-007 / AC-007.84 — tray formatter markers align across Linux + Windows.
#[test]
fn fr007_operator_matrix_tray_formatter_markers() {
    let pool = sample_pool_snapshot();
    let status = sample_status_snapshot();

    let linux_pool = linux_display::format_pool_tray_line(&pool);
    let linux_status = linux_display::format_status_snapshot_tray_line(&status);

    use sharecli_tray_windows::ipc::{
        GateStatusSnapshot as WinGate, HostResourceWatchJson as WinHost, PoolSnapshot as WinPool,
        StatusSnapshot as WinStatus,
    };

    let win_pool = WinPool {
        node_total: pool.node_total,
        node_idle: pool.node_idle,
        bun_total: pool.bun_total,
        bun_idle: pool.bun_idle,
        max_per_type: pool.max_per_type,
        healthy: pool.healthy,
        issues: pool.issues.clone(),
        gate: WinGate {
            thermal_pressure: pool.gate.thermal_pressure.clone(),
            detected_agents: pool.gate.detected_agents,
            agent_total_rss_bytes: pool.gate.agent_total_rss_bytes,
            agent_contention: pool.gate.agent_contention.clone(),
            gate_decision: pool.gate.gate_decision.clone(),
        },
        host_watch: WinHost {
            fd_count: pool.host_watch.fd_count,
            net_rx_bytes: pool.host_watch.net_rx_bytes,
            net_tx_bytes: pool.host_watch.net_tx_bytes,
            mem_rss_bytes: pool.host_watch.mem_rss_bytes,
            load_1m: pool.host_watch.load_1m,
        },
    };
    let win_status = WinStatus {
        total_processes: status.total_processes,
        agents: vec![],
        scanned: status.scanned,
        watched: status.watched,
        gate: WinGate {
            thermal_pressure: status.gate.thermal_pressure.clone(),
            detected_agents: status.gate.detected_agents,
            agent_total_rss_bytes: status.gate.agent_total_rss_bytes,
            agent_contention: status.gate.agent_contention.clone(),
            gate_decision: status.gate.gate_decision.clone(),
        },
        host_watch: WinHost {
            fd_count: status.host_watch.fd_count,
            net_rx_bytes: status.host_watch.net_rx_bytes,
            net_tx_bytes: status.host_watch.net_tx_bytes,
            mem_rss_bytes: status.host_watch.mem_rss_bytes,
            load_1m: status.host_watch.load_1m,
        },
    };

    let win_pool_line = win_display::format_pool_tray_line(&win_pool);
    let win_status_line = win_display::format_status_snapshot_tray_line(&win_status);

    for (label, line) in [
        ("linux pool", &linux_pool),
        ("linux status", &linux_status),
        ("windows pool", &win_pool_line),
        ("windows status", &win_status_line),
    ] {
        assert!(
            line.starts_with(POOL_PREFIX) || line.starts_with(PROC_PREFIX),
            "{label} MUST use operator prefix markers (AC-007.84); got: {line}"
        );
    }
    assert!(linux_pool.starts_with(POOL_PREFIX) && win_pool_line.starts_with(POOL_PREFIX));
    assert!(linux_status.starts_with(PROC_PREFIX) && win_status_line.starts_with(PROC_PREFIX));
}

/// FR-007 / AC-007.84 — Swift + C# tray sources ship pool/proc-scan operator formatters.
#[test]
fn fr007_operator_matrix_tray_source_markers() {
    let swift = fs::read_to_string(manifest_path(
        "desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift",
    ))
    .expect("read Swift OperatorDisplay.swift");
    assert!(swift.contains("Pool node"), "Swift tray MUST format pool line (AC-007.84)");
    assert!(swift.contains("Proc scan"), "Swift tray MUST format proc-scan line (AC-007.84)");

    let csharp = fs::read_to_string(manifest_path("windows/ShareCLITray/OperatorDisplay.cs"))
        .expect("read OperatorDisplay.cs");
    assert!(csharp.contains("Pool node"), "WinUI tray MUST format pool line (AC-007.84)");
    assert!(csharp.contains("Proc scan"), "WinUI tray MUST format proc-scan line (AC-007.84)");
}

/// FR-007 / AC-007.84 — dashboard HTML + thermal TUI companion panel markers present.
#[test]
fn fr007_operator_matrix_dashboard_tui_markers() {
    let html =
        fs::read_to_string(manifest_path("src/dashboard.html")).expect("read dashboard.html");
    for marker in [
        "data-operator-panels",
        "panel-gate",
        "panel-host-watch",
        "panel-pool",
        "panel-status",
        "renderOperatorPanels",
    ] {
        assert!(html.contains(marker), "dashboard MUST include {marker} (AC-007.84)");
    }

    assert!(HELP_OVERLAY_HINT.contains("2 pool"));
    assert!(HELP_OVERLAY_HINT.contains("3 status"));

    let pool_lines = pool_panel_lines(
        Some(PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
        }),
        false,
    );
    let status_lines = status_panel_lines(
        Some(StatusOperatorPanel { scanned: 50, watched: 1, total_processes: 2, agent_rows: 1 }),
        false,
    );
    let pool_text: String = pool_lines.iter().map(|l| l.to_string()).collect();
    let status_text: String = status_lines.iter().map(|l| l.to_string()).collect();
    assert!(pool_text.contains("Node idle/total"), "TUI pool panel MUST render (AC-007.84)");
    assert!(status_text.contains("Scanned:"), "TUI status panel MUST render (AC-007.84)");
}
