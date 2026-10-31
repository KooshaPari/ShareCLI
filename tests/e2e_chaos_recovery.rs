// SPDX-License-Identifier: Apache-2.0 OR MIT
//! E2E chaos tier — kill/restart serve and verify `/healthz` recovery (C07 L64).
//! FR: FR-003

use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn pick_port() -> u16 {
    19_800 + (std::process::id() % 200) as u16
}

fn spawn_serve(port: u16) -> Child {
    bin()
        .args(["serve", "--bind", &format!("127.0.0.1:{port}"), "--on-conflict", "replace"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sharecli serve")
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

/// FR-003 / C07 L64 — chaos e2e: SIGKILL serve, restart, `/healthz` recovers < 30s.
#[test]
fn chaos_restart_healthz_e2e_recovers() {
    if !Command::new("curl").arg("--version").status().map(|s| s.success()).unwrap_or(false) {
        eprintln!("curl not available; skipping chaos_restart_healthz_e2e_recovers");
        return;
    }

    let port = pick_port();
    let url = format!("http://127.0.0.1:{port}/healthz");

    let mut child = spawn_serve(port);
    assert!(wait_healthz(&url, Duration::from_secs(20)), "initial serve must answer /healthz");

    let _ = child.kill();
    let _ = child.wait();

    thread::sleep(Duration::from_secs(1));

    let mut child = spawn_serve(port);
    assert!(
        wait_healthz(&url, Duration::from_secs(30)),
        "restarted serve must recover /healthz within 30s"
    );

    let _ = child.kill();
    let _ = child.wait();
}
