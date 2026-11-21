// SPDX-License-Identifier: Apache-2.0 OR MIT
//! E2E tier — serve `/healthz` liveness over real HTTP (C07 L64).
//! FR: FR-003

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn pick_port() -> u16 {
    // Ephemeral port in dev range; avoids clashing with default 9000.
    19_000 + (std::process::id() % 800) as u16
}

/// Cap on captured serve output per stream. Diagnostics only need the head of
/// the log; an unbounded buffer can balloon test-process memory if `serve`
/// logs aggressively while we poll for readiness.
const CAPTURE_LIMIT: usize = 256 * 1024;
const TRUNCATED_MARKER: &[u8] = b"\n...[output truncated]...\n";

struct ServeChild {
    child: Option<Child>,
    port: u16,
    stdout_buf: Arc<Mutex<Vec<u8>>>,
    stderr_buf: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
    _lock_dir: tempfile::TempDir,
}

fn append_capped(buf: &mut Vec<u8>, chunk: &[u8]) {
    if buf.len() >= CAPTURE_LIMIT {
        if !buf.ends_with(TRUNCATED_MARKER) {
            buf.extend_from_slice(TRUNCATED_MARKER);
        }
        return;
    }
    let remaining = CAPTURE_LIMIT - buf.len();
    let take = remaining.min(chunk.len());
    buf.extend_from_slice(&chunk[..take]);
    if take < chunk.len() {
        buf.extend_from_slice(TRUNCATED_MARKER);
    }
}

fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
    buf: Arc<Mutex<Vec<u8>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut guard) = buf.lock() {
                        append_capped(&mut guard, &chunk[..n]);
                    }
                }
            }
        }
    })
}

impl ServeChild {
    fn spawn(port: u16) -> Self {
        let lock_dir = tempfile::tempdir().expect("create isolated serve lock dir");
        let mut child = bin()
            .args(["serve", "--bind", &format!("127.0.0.1:{port}"), "--on-conflict", "replace"])
            .env("XDG_RUNTIME_DIR", lock_dir.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sharecli serve");

        let stdout_buf = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf = Arc::new(Mutex::new(Vec::new()));
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            readers.push(spawn_reader(out, Arc::clone(&stdout_buf)));
        }
        if let Some(err) = child.stderr.take() {
            readers.push(spawn_reader(err, Arc::clone(&stderr_buf)));
        }

        Self { child: Some(child), port, stdout_buf, stderr_buf, readers, _lock_dir: lock_dir }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("serve child already taken")
    }

    fn healthz_url(&self) -> String {
        format!("http://127.0.0.1:{}/healthz", self.port)
    }

    fn captured_output(&self) -> String {
        let stdout = self
            .stdout_buf
            .lock()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let stderr = self
            .stderr_buf
            .lock()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        format!("--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}")
    }

    fn kill_and_reap(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // The readers hit EOF once the child's pipe write ends close, so join
        // them instead of guessing at a sleep. Bound the wait so a grandchild
        // that inherited the pipe cannot hang the harness.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && self.readers.iter().any(|h| !h.is_finished()) {
            thread::sleep(Duration::from_millis(10));
        }
        for handle in std::mem::take(&mut self.readers) {
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for ServeChild {
    fn drop(&mut self) {
        self.kill_and_reap();
    }
}

fn curl_ok(url: &str) -> bool {
    Command::new("curl")
        .args(["-fsS", "-o", "/dev/null", "--max-time", "2", url])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn wait_for_healthz_or_exit(serve: &mut ServeChild, timeout: Duration) -> Result<(), String> {
    let url = serve.healthz_url();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if curl_ok(&url) {
            return Ok(());
        }
        match serve.child_mut().try_wait() {
            Ok(Some(status)) => {
                thread::sleep(Duration::from_millis(50));
                return Err(format!(
                    "sharecli serve exited before /healthz became reachable (status: {status})\n{}",
                    serve.captured_output()
                ));
            }
            Ok(None) => {}
            Err(e) => {
                return Err(format!(
                    "failed to poll sharecli serve child status: {e}\n{}",
                    serve.captured_output()
                ))
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(format!(
        "timed out after {timeout:?} waiting for /healthz at {url}\n{}",
        serve.captured_output()
    ))
}

/// FR-003 / C07 L64 — e2e tier: real serve process answers `/healthz`.
#[test]
fn serve_healthz_e2e_returns_200() {
    if !Command::new("curl").arg("--version").status().map(|s| s.success()).unwrap_or(false) {
        eprintln!("curl not available; skipping serve_healthz_e2e_returns_200");
        return;
    }

    let port = pick_port();
    let mut serve = ServeChild::spawn(port);

    if let Err(msg) = wait_for_healthz_or_exit(&mut serve, Duration::from_secs(20)) {
        serve.kill_and_reap();
        panic!("serve /healthz must become reachable: {msg}");
    }
}
