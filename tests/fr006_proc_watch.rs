//! FR-006 — `sharecli proc --watch` live refresh
//! FR: FR-006
//!
//! AC-006.15 `sharecli proc --watch N` re-renders agent inventory until Ctrl-C

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn watch_grace(normal: Duration, profiled: Duration) -> Duration {
    matches!(std::env::var("SHARECLI_DHAT_PROFILE"), Ok(value) if value == "1")
        .then_some(profiled)
        .unwrap_or(normal)
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// Drain stdout (and stderr) while polling for `frames_needed` occurrences
/// of `frame_marker` in stdout. `max` bounds the total wait; if the watcher
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

/// FR-006 / AC-006.15 — proc help documents live watch mode.
#[test]
fn fr006_proc_help_documents_watch() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success(), "proc --help should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--watch"), "proc --help MUST document --watch; got: {s}");
}

/// FR-006 / AC-006.15 — watch mode prints refresh banner and re-renders inventory.
#[test]
fn fr006_proc_watch_renders_twice_before_exit() {
    let mut child = bin()
        .args(["proc", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(
        &mut child,
        "Host agents (proc scan)",
        2,
        watch_grace(Duration::from_secs(30), Duration::from_secs(60)),
    );

    assert!(
        stdout.contains("Host agents (proc scan)"),
        "watch MUST render inventory header; got: {stdout}"
    );
    assert!(stdout.contains("[watch]"), "watch MUST print refresh footer; got: {stdout}");
    assert!(
        stdout.matches("Host agents (proc scan)").count() >= 2,
        "watch MUST re-render at least twice before its feature-aware deadline; got {} headers",
        stdout.matches("Host agents (proc scan)").count()
    );
}

/// FR-006 / AC-006.15 — watch honors --json (valid JSON each refresh).
#[test]
fn fr006_proc_watch_json_emits_valid_payload() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(
        &mut child,
        "\n",
        1,
        watch_grace(Duration::from_secs(30), Duration::from_secs(60)),
    );

    assert!(stdout.contains("\"agents\""), "watch --json MUST emit agents array; got: {stdout}");
    assert!(
        !stdout.contains("[watch]"),
        "watch --json NDJSON MUST keep footer off stdout; got: {stdout}"
    );
    let line = stdout.lines().next().expect("NDJSON line");
    let v: serde_json::Value = serde_json::from_str(line).expect("valid proc NDJSON");
    assert!(v.get("ts").is_some(), "NDJSON line MUST include ts");
}
