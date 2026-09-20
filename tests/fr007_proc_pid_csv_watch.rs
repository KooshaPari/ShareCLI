//! FR-007 — `sharecli proc --pid N --csv --watch`
//! FR: FR-007
//!
//! AC-007.91 CSV watch emits PID detail + gate → host_watch → pool → status companions
//! each tick on stdout; stderr silent on success.

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const FRAME_MARKER: &str = "# sharecli-proc-pid-watch-frame";
const DETAIL_CSV_HEADER: &str = "pid,ppid,comm,state,mem_rss_bytes,mem_rss,fd_count";
const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";
const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";
const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";
const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

/// Drain stdout/stderr while polling for `frames_needed` occurrences of
/// `frame_marker` in stdout. `max` bounds the total wait; if the watcher
/// can't emit that many frames in `max`, the call returns whatever was
/// captured so far (and the assertion below will likely fail).
///
/// Replaces a fixed-sleep drain that was flaky under parallel CI load: a
/// fixed sleep can land before the watcher has emitted enough frames.
/// Drain stdout/stderr while polling for `frames_needed` occurrences of
/// `frame_marker` in stdout. `max` bounds the total wait; if the watcher
/// can't emit that many frames in `max`, the call returns whatever was
/// captured so far (and the assertion below will likely fail).
///
/// Replaces a fixed-sleep drain that was flaky under parallel CI load: a
/// fixed sleep can land before the watcher has emitted enough frames.
fn drain_watch_pipes(
    child: &mut Child,
    frame_marker: &str,
    frames_needed: usize,
    max: Duration,
) -> (String, String) {
    let stdout_handle = child.stdout.take().expect("piped stdout");
    let stderr_handle = child.stderr.take().expect("piped stderr");
    let stdout_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let out_reader = {
        let buf = stdout_buf.clone();
        thread::spawn(move || {
            let mut s = String::new();
            let mut out = stdout_handle;
            let _ = out.read_to_string(&mut s);
            *buf.lock().expect("stdout lock") = s;
        })
    };
    let err_reader = {
        let buf = stderr_buf.clone();
        thread::spawn(move || {
            let mut s = String::new();
            let mut err = stderr_handle;
            let _ = err.read_to_string(&mut s);
            *buf.lock().expect("stderr lock") = s;
        })
    };

    let deadline = Instant::now() + max;
    loop {
        let count = stdout_buf
            .lock()
            .expect("stdout lock")
            .matches(frame_marker)
            .count();
        if count >= frames_needed || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = out_reader.join();
    let _ = err_reader.join();
    let stdout = stdout_buf.lock().expect("stdout lock").clone();
    let stderr = stderr_buf.lock().expect("stderr lock").clone();
    (stdout, stderr)
}

fn assert_csv_envelope(frame: &str, context: &str) {
    let body = frame
        .find(DETAIL_CSV_HEADER)
        .unwrap_or_else(|| panic!("{context} MUST include PID CSV header; got: {frame}"));
    let gate = frame[body..]
        .find(GATE_CSV_HEADER)
        .map(|p| body + p)
        .unwrap_or_else(|| panic!("{context} MUST include gate CSV companion; got: {frame}"));
    let host = frame[gate..]
        .find(HOST_CSV_HEADER)
        .map(|p| gate + p)
        .unwrap_or_else(|| panic!("{context} MUST include host_watch CSV companion; got: {frame}"));
    let pool = frame[host..]
        .find(POOL_CSV_HEADER)
        .map(|p| host + p)
        .unwrap_or_else(|| panic!("{context} MUST include pool CSV companion; got: {frame}"));
    let status = frame[pool..]
        .find(STATUS_CSV_HEADER)
        .map(|p| pool + p)
        .unwrap_or_else(|| panic!("{context} MUST include status CSV companion; got: {frame}"));
    assert!(
        body < gate && gate < host && host < pool && pool < status,
        "{context} MUST order body → gate → host_watch → pool → status (AC-007.91); got: {frame}"
    );
}

/// FR-007 / AC-007.91 — proc --pid --csv --watch stderr silent; multi-frame envelope.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_csv_watch_stderr_silent_and_envelope() {
    let self_pid = std::process::id().to_string();
    let mut child = bin()
        .args(["proc", "--pid", &self_pid, "--csv", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn proc --pid --csv --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, "
", 2, Duration::from_millis(30_000));

    assert!(
        stderr.is_empty(),
        "proc --pid --csv --watch MUST keep stderr silent (AC-007.91); stderr: {stderr:?}"
    );
    assert!(
        !stdout.contains("\x1b[2J"),
        "proc --pid --csv --watch MUST NOT emit ANSI clear (pipe-safe); got: {stdout}"
    );
    let complete_frames: Vec<&str> = stdout
        .split(FRAME_MARKER)
        .skip(1)
        .filter(|frame| frame.contains(DETAIL_CSV_HEADER))
        .collect();
    assert!(
        complete_frames.len() >= 2,
        "proc --pid --csv --watch MUST emit >=2 complete frames; got {} in: {stdout}",
        complete_frames.len()
    );
    assert!(
        stdout.contains("[watch]"),
        "proc --pid --csv --watch MUST include [watch] footer comment; got: {stdout}"
    );
    for (idx, frame) in complete_frames.iter().enumerate() {
        assert_csv_envelope(frame, &format!("pid csv watch frame {}", idx + 1));
    }
}

/// FR-007 / AC-007.91 — proc --pid --csv --json --watch remains rejected.
#[test]
fn fr007_proc_pid_csv_json_watch_still_rejected() {
    let self_pid = std::process::id().to_string();
    let out = bin()
        .args(["proc", "--pid", &self_pid, "--csv", "--json", "--watch", "1"])
        .output()
        .expect("spawn proc --pid --csv --json --watch");
    assert!(!out.status.success(), "proc --pid --csv --json --watch MUST fail (AC-007.91)");
}
