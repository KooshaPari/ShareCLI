// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --watch --json` NDJSON stream
//! FR: FR-006
//!
//! AC-006.18 watch + JSON emits one compact JSON object per line (NDJSON)
//! AC-006.37 NDJSON agent rows include `state` (parity with flat `--json`, AC-006.32)

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const WATCH_DEADLINE: Duration = Duration::from_secs(45);

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn complete_ndjson_line_count(output: &str) -> usize {
    output
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n') && !line.trim().is_empty())
        .count()
}

fn drain_watch_until(
    child: &mut Child,
    mut ready: impl FnMut(&str, &str) -> bool,
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
                Ok(0) | Err(_) => break,
                Ok(n) => out_arc
                    .lock()
                    .expect("stdout lock")
                    .push_str(&String::from_utf8_lossy(&chunk[..n])),
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        let mut err = stderr;
        loop {
            match err.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => err_arc
                    .lock()
                    .expect("stderr lock")
                    .push_str(&String::from_utf8_lossy(&chunk[..n])),
            }
        }
    });

    let deadline = Instant::now() + WATCH_DEADLINE;
    let mut early_exit = None;
    while Instant::now() < deadline {
        let stdout = stdout_buf.lock().expect("stdout lock").clone();
        let stderr = stderr_buf.lock().expect("stderr lock").clone();
        if ready(&stdout, &stderr) {
            break;
        }
        if let Some(status) = child.try_wait().expect("check watch child") {
            early_exit = Some(status);
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    if early_exit.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    let stdout = stdout_buf.lock().expect("stdout lock").clone();
    let stderr = stderr_buf.lock().expect("stderr lock").clone();
    if let Some(status) = early_exit {
        panic!("watch child exited before readiness: {status}; stdout: {stdout}; stderr: {stderr}");
    }
    (stdout, stderr)
}

#[test]
fn complete_ndjson_line_count_requires_newline_delimited_objects() {
    assert_eq!(complete_ndjson_line_count("{\"ts\":1}\n{\"ts\":2}\n"), 2);
    assert_eq!(complete_ndjson_line_count("{\"ts\":1}\n{\"ts\":2}"), 1);
}

/// FR-006 / AC-006.18 — each watch refresh is a single parseable NDJSON line with `ts`.
#[test]
#[serial_test::serial]
fn fr006_proc_watch_ndjson_one_line_per_refresh() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    let (stdout, _) =
        drain_watch_until(&mut child, |stdout, _| complete_ndjson_line_count(stdout) >= 2);

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.len() >= 2,
        "NDJSON watch MUST emit at least two lines before its feature-aware deadline; got {} line(s): {stdout}",
        lines.len()
    );
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).expect("each NDJSON line MUST parse");
        assert!(v.get("ts").and_then(|t| t.as_u64()).is_some(), "line MUST include ts");
        assert!(v.get("agents").and_then(|a| a.as_array()).is_some(), "line MUST include agents");
    }
}

/// FR-006 / AC-006.18 — NDJSON stdout stays pipe-clean (no watch footer, no ANSI clear).
#[test]
#[serial_test::serial]
fn fr006_proc_watch_ndjson_stdout_is_pipe_clean() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    let (stdout, stderr) = drain_watch_until(&mut child, |stdout, stderr| {
        complete_ndjson_line_count(stdout) >= 1 && stderr.contains("[watch]")
    });

    assert!(
        !stdout.contains("[watch]"),
        "NDJSON stdout MUST NOT contain watch footer; got: {stdout}"
    );
    assert!(!stdout.contains("\x1b[2J"), "NDJSON stdout MUST NOT contain terminal clear sequences");
    assert!(
        stderr.contains("[watch]"),
        "watch footer MUST appear on stderr in NDJSON mode; stderr: {stderr}"
    );
}

/// FR-006 / AC-006.37 — NDJSON watch agent objects expose `state` key (AC-006.32 parity).
#[test]
#[serial_test::serial]
fn fr006_proc_watch_ndjson_agent_rows_include_state_key() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    let (stdout, _) =
        drain_watch_until(&mut child, |stdout, _| complete_ndjson_line_count(stdout) >= 1);

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    let line = lines.first().copied().unwrap_or_else(|| {
        panic!("NDJSON watch MUST emit at least one line before its feature-aware deadline; got: {stdout}");
    });
    let v: serde_json::Value = serde_json::from_str(line).expect("NDJSON line MUST parse");
    let agents = v.get("agents").and_then(|a| a.as_array()).expect("agents array");
    if let Some(first) = agents.first() {
        assert!(
            first.get("state").is_some(),
            "NDJSON watch agent rows MUST include state when agents present; got: {first}"
        );
    }
}

/// FR-006 / AC-006.37 — serialized NDJSON line preserves agent state letters.
#[test]
fn fr006_proc_watch_ndjson_line_serializes_agent_state() {
    use sharecli::commands::proc::{AgentProcNdjsonLine, AgentProcRow, AgentProcSnapshot};

    let line = AgentProcNdjsonLine {
        ts: 1_750_000_000,
        snapshot: AgentProcSnapshot {
            agents: vec![AgentProcRow {
                pid: 42,
                family: "claude".into(),
                comm: "claude".into(),
                state: "S".into(),
                mem_rss_bytes: 4096,
                mem_rss: "4K".into(),
                fd_count: Some(3),
            }],
            scanned: 1,
            watched: 1,
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 1,
                agent_total_rss_bytes: 4096,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: sharecli::monitoring::HostResourceWatchJson::default(),
            pool: None,
            status: None,
        },
    };
    let json = serde_json::to_string(&line).expect("serialize NDJSON line");
    assert!(
        json.contains("\"state\":\"S\""),
        "NDJSON watch line MUST include agent state; got: {json}"
    );
}

/// FR-006 / AC-006.18 — one-shot proc --json remains pretty-printed (non-NDJSON).
#[test]
#[serial_test::serial]
fn fr006_proc_json_snapshot_not_ndjson() {
    let out = bin().args(["proc", "--json"]).output().expect("spawn sharecli proc --json");
    assert!(out.status.success(), "proc --json should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains('\n'), "one-shot --json MUST remain multi-line pretty JSON; got: {s}");
    assert!(!s.contains("\"ts\""), "one-shot --json MUST NOT inject ts field; got: {s}");
}
