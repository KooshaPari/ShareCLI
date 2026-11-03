// SPDX-License-Identifier: Apache-2.0 OR MIT
//! E2E tier — serve `/healthz` liveness over real HTTP (C07 L64).
//! FR: FR-003

use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn pick_port() -> u16 {
    // Ephemeral port in dev range; avoids clashing with default 9000.
    19_000 + (std::process::id() % 800) as u16
}

struct ServeChild {
    child: Child,
    port: u16,
}

impl ServeChild {
    fn spawn(port: u16) -> Self {
        let child = bin()
            .args(["serve", "--bind", &format!("127.0.0.1:{port}"), "--on-conflict", "replace"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sharecli serve");
        Self { child, port }
    }

    fn healthz_url(&self) -> String {
        format!("http://127.0.0.1:{}/healthz", self.port)
    }
}

impl Drop for ServeChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn curl_ok(url: &str) -> bool {
    Command::new("curl")
        .args(["-fsS", "-o", "/dev/null", "--max-time", "2", url])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn wait_healthz(url: &str, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if curl_ok(url) {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

/// FR-003 / C07 L64 — e2e tier: real serve process answers `/healthz`.
#[test]
fn serve_healthz_e2e_returns_200() {
    if !Command::new("curl").arg("--version").status().map(|s| s.success()).unwrap_or(false) {
        eprintln!("curl not available; skipping serve_healthz_e2e_returns_200");
        return;
    }

    let port = pick_port();
    let serve = ServeChild::spawn(port);
    let url = serve.healthz_url();

    assert!(
        wait_healthz(&url, Duration::from_secs(20)),
        "serve /healthz must become reachable at {url}"
    );
}
