// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli proc --csv --watch` / `--tree --csv --watch`
//! FR: FR-007
//!
//! AC-007.88 CSV watch emits inventory + gate → host_watch → pool → status companions
//! each tick on stdout; stderr silent on success.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const FRAME_MARKER: &str = "# sharecli-proc-watch-frame";
const FLAT_CSV_HEADER: &str = "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count";
const TREE_CSV_HEADER: &str =
    "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count";
const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";
const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";
const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";
const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

fn assert_csv_envelope(frame: &str, body_header: &str, context: &str) {
    let body = frame
        .find(body_header)
        .unwrap_or_else(|| panic!("{context} MUST include inventory CSV header; got: {frame}"));
    let gate = frame
        .find(GATE_CSV_HEADER)
        .unwrap_or_else(|| panic!("{context} MUST include gate CSV companion; got: {frame}"));
    let host = frame
        .find(HOST_CSV_HEADER)
        .unwrap_or_else(|| panic!("{context} MUST include host_watch CSV companion; got: {frame}"));
    let pool = frame
        .find(POOL_CSV_HEADER)
        .unwrap_or_else(|| panic!("{context} MUST include pool CSV companion; got: {frame}"));
    let status = frame
        .find(STATUS_CSV_HEADER)
        .unwrap_or_else(|| panic!("{context} MUST include status CSV companion; got: {frame}"));
    assert!(
        body < gate && gate < host && host < pool && pool < status,
        "{context} MUST order body → gate → host_watch → pool → status (AC-007.88); got: {frame}"
    );
}

/// FR-007 / AC-007.88 — flat proc --csv --watch stderr silent; multi-frame envelope.
#[test]
#[serial_test::serial]
fn fr007_proc_csv_watch_stderr_silent_and_envelope() {
    let mut child = bin()
        .args(["proc", "--csv", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn proc --csv --watch 1");

    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(45), |buf| {
        complete_csv_frame_count(buf, FLAT_CSV_HEADER) >= 2
    });

    assert!(
        stderr.is_empty(),
        "proc --csv --watch MUST keep stderr silent (AC-007.88); stderr: {stderr:?}"
    );
    assert!(
        !stdout.contains("\x1b[2J"),
        "proc --csv --watch MUST NOT emit ANSI clear (pipe-safe); got: {stdout}"
    );
    let complete_frames: Vec<&str> = stdout
        .split(FRAME_MARKER)
        .skip(1)
        .filter(|frame| frame.contains(FLAT_CSV_HEADER) && frame.contains("[watch]"))
        .collect();
    assert!(
        complete_frames.len() >= 2,
        "proc --csv --watch MUST emit >=2 complete frames; got {} in: {stdout}",
        complete_frames.len()
    );
    assert!(
        stdout.contains("[watch]"),
        "proc --csv --watch MUST include [watch] footer comment; got: {stdout}"
    );
    for (idx, frame) in complete_frames.iter().enumerate() {
        assert_csv_envelope(frame, FLAT_CSV_HEADER, &format!("flat csv watch frame {}", idx + 1));
    }
}

/// FR-007 / AC-007.88 — proc --tree --csv --watch same stderr/envelope contract.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_csv_watch_stderr_silent_and_envelope() {
    let mut child = bin()
        .args(["proc", "--tree", "--csv", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn proc --tree --csv --watch 1");

    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(45), |buf| {
        complete_csv_frame_count(buf, TREE_CSV_HEADER) >= 2
    });

    assert!(
        stderr.is_empty(),
        "proc --tree --csv --watch MUST keep stderr silent (AC-007.88); stderr: {stderr:?}"
    );
    let complete_frames: Vec<&str> = stdout
        .split(FRAME_MARKER)
        .skip(1)
        .filter(|frame| frame.contains(TREE_CSV_HEADER) && frame.contains("[watch]"))
        .collect();
    assert!(
        complete_frames.len() >= 2,
        "proc --tree --csv --watch MUST emit >=2 complete frames; got {} in: {stdout}",
        complete_frames.len()
    );
    for (idx, frame) in complete_frames.iter().enumerate() {
        assert_csv_envelope(frame, TREE_CSV_HEADER, &format!("tree csv watch frame {}", idx + 1));
    }
}

/// FR-007 / AC-007.88 — proc --csv --json --watch remains rejected.
#[test]
fn fr007_proc_csv_json_watch_still_rejected() {
    let out = bin()
        .args(["proc", "--csv", "--json", "--watch", "1"])
        .output()
        .expect("spawn proc --csv --json --watch");
    assert!(!out.status.success(), "proc --csv --json --watch MUST fail (AC-007.88)");
}

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
    let mut early_exit = None;
    while Instant::now() < deadline {
        let snapshot = stdout_buf.lock().expect("stdout lock").clone();
        if ready(&snapshot) {
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

fn complete_csv_frame_count(output: &str, body_header: &str) -> usize {
    output
        .split(FRAME_MARKER)
        .skip(1)
        .filter(|frame| frame.contains(body_header) && frame.contains("[watch]"))
        .count()
}

#[test]
fn complete_csv_frame_count_requires_watch_footer() {
    let partial = format!("{FRAME_MARKER}\n{FLAT_CSV_HEADER}\n");
    let complete = format!("{partial}# [watch] tick=1\n");
    assert_eq!(complete_csv_frame_count(&partial, FLAT_CSV_HEADER), 0);
    assert_eq!(complete_csv_frame_count(&complete, FLAT_CSV_HEADER), 1);
}

/// FR-007 / AC-007.94 — `# [watch]` footer must flush in the same tick as the CSV body.
#[test]
#[serial_test::serial]
fn fr007_proc_csv_watch_footer_flushed_same_tick() {
    let mut child = bin()
        .args(["proc", "--csv", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn proc --csv --watch 1");

    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(45), |buf| {
        buf.contains(FRAME_MARKER) && buf.contains(FLAT_CSV_HEADER) && buf.contains("[watch]")
    });

    assert!(
        stderr.is_empty(),
        "proc --csv --watch MUST keep stderr silent (AC-007.94); stderr: {stderr:?}"
    );
    assert!(
        stdout.contains("[watch]"),
        "proc --csv --watch MUST include [watch] footer (AC-007.94); got: {stdout}"
    );
    let watch_pos = stdout.find("[watch]").expect("[watch] must be present after assert above");
    let markers_before_footer = stdout[..watch_pos].matches(FRAME_MARKER).count();
    assert_eq!(
        markers_before_footer, 1,
        "AC-007.94: `# [watch]` MUST flush in the same tick (exactly one frame marker before first footer); got {markers_before_footer} in: {stdout}"
    );
}
