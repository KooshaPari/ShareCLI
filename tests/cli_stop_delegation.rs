//! FR: FR-006
//!
//! CRITICAL #1 (pre-existing design debt): `sharecli stop --pid` consulted
//! only the CLI's own in-process `ProcessPool`, which is always empty in a
//! fresh invocation, so it could NEVER stop anything — it reported
//! `no such pid` (exit 2) even for a live process the sidecar manages. The
//! working kill path is IPC `process.kill` against the sidecar (the same
//! path the tray uses, acceptance-verified with a real `/bin/sleep`).
//!
//! This test runs the REAL binaries: a real `sharecli-ipc` sidecar on an
//! isolated socket, a real sidecar-managed child, and the real `sharecli
//! stop --pid`. It fails on the current code (stop exits 2 and the child
//! survives) and passes once `stop` delegates to the sidecar.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Minimal JSON-RPC-over-unix-socket call against the sidecar. Kept local to
/// the test (rather than `GhosttyControlClient`) so fixture setup cannot be
/// affected by production-client quirks: the red must fail on `stop`, not
/// earlier.
fn rpc(sock: &Path, method: &str, params: Value) -> Value {
    let stream =
        std::os::unix::net::UnixStream::connect(sock).expect("fixture: connect to sidecar");
    let mut writer = std::io::BufWriter::new(&stream);
    serde_json::to_writer(&mut writer, &json!({ "id": 1, "method": method, "params": params }))
        .expect("fixture: encode request");
    writer.write_all(b"\n").expect("fixture: newline");
    writer.flush().expect("fixture: flush");

    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).expect("fixture: read response");
    let value: Value = serde_json::from_str(&line).expect("fixture: decode response");
    let error = value.get("error").cloned().unwrap_or(Value::Null);
    assert!(error.is_null(), "fixture: rpc {method} failed: {error}");
    value["result"].clone()
}

/// True while the pid exists (`kill -0`).
fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Wait up to `timeout` for the pid to disappear; false on timeout.
fn wait_gone(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// RAII guard: kills and reaps the sidecar even when an assertion or the
/// socket-wait panics, so a red run cannot leak a daemon on the machine.
struct Sidecar(Child);

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_sidecar(dir: &Path) -> (Sidecar, PathBuf) {
    // `CARGO_BIN_EXE_*` only covers bins of THIS package, so the sidecar
    // (package sharecli-ipc) is resolved from the test binary's location:
    // target/<profile>/deps/<test> -> target/<profile>/sharecli-ipc.
    // Workspace-level runs (CI Guardrail/nextest, `cargo test --workspace`)
    // build every package's bins, so it is always present there.
    let mut bin = std::env::current_exe().expect("current test binary");
    bin.pop(); // deps/
    bin.pop(); // target/<profile>/
    bin.push("sharecli-ipc");
    assert!(
        bin.exists(),
        "sidecar binary missing at {} — run `cargo build --workspace` first (workspace test runs build it automatically)",
        bin.display()
    );

    let sock = dir.join("ipc.sock");
    let child = Command::new(&bin)
        .env("SHARECLI_IPC_SOCK", &sock)
        .env("SHARECLI_CONFIG_PATH", dir.join("config.toml"))
        .env("HOME", dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("fixture: spawn sidecar");
    let guard = Sidecar(child);

    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if sock.exists() {
            return (guard, sock);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // `guard` drops here (kill + wait) before the panic surfaces.
    panic!("fixture: sidecar socket never appeared at {}", sock.display());
}

/// The red: a sidecar-managed child must actually be stopped by
/// `sharecli stop --pid`, with exit 0 — not "no such pid" / exit 2.
#[test]
fn stop_pid_stops_a_sidecar_managed_process() {
    let dir = tempfile::tempdir().expect("isolated fixture dir");
    let (sidecar, sock) = start_sidecar(dir.path());

    // Register a real child with the sidecar's pool (the only way anything
    // becomes "managed").
    let spawned = rpc(&sock, "process.spawn", json!({ "command": "sleep", "args": ["600"] }));
    assert_eq!(spawned["success"], Value::Bool(true), "fixture spawn: {spawned}");
    let pid = spawned["pid"].as_u64().expect("spawn returns a pid") as u32;
    assert!(alive(pid), "fixture: spawned child must be alive");

    // Run the real CLI against the same socket.
    let out = Command::new(env!("CARGO_BIN_EXE_sharecli"))
        .args(["stop", "--pid", &pid.to_string()])
        .env("SHARECLI_IPC_SOCK", &sock)
        .env("SHARECLI_CONFIG_PATH", dir.path().join("config.toml"))
        .env("HOME", dir.path())
        .output()
        .expect("run sharecli stop");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "stop --pid of a sidecar-managed process must succeed; exit={:?}\nstdout:{stdout}\nstderr:{stderr}",
        out.status.code()
    );
    assert!(stdout.contains("stopped"), "stop must report the process stopped; stdout:{stdout}");
    assert!(wait_gone(pid, Duration::from_secs(5)), "the managed child must be dead after stop");

    // Cleanup: kill the child if still alive (red path leaves it running);
    // the Sidecar guard kills + reaps the daemon on scope exit — including
    // when an assertion above panics.
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).stderr(Stdio::null()).status();
    drop(sidecar);
}

/// No sidecar at all: the exit-2 / `not_found` taxonomy must be preserved
/// (the CLI cannot manage anything on its own). Guards the fallback path so
/// delegation cannot regress the honest-miss contract of audit task 1.10.
#[test]
fn stop_pid_without_a_sidecar_keeps_the_not_found_taxonomy() {
    let dir = tempfile::tempdir().expect("isolated fixture dir");
    let sock = dir.path().join("does-not-exist.sock"); // never created

    let out = Command::new(env!("CARGO_BIN_EXE_sharecli"))
        .args(["stop", "--pid", "4194303"])
        .env("SHARECLI_IPC_SOCK", &sock)
        .env("SHARECLI_CONFIG_PATH", dir.path().join("config.toml"))
        .env("HOME", dir.path())
        .output()
        .expect("run sharecli stop");

    assert_eq!(
        out.status.code(),
        Some(2),
        "without a sidecar stop must keep exit 2; stdout:{} stderr:{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
