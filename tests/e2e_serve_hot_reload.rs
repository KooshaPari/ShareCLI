//! E2E tier — config hot-reload resolves the same path `Config::load` uses.
//!
//! FR: FR-002 (config load) / serve hot-reload
//!
//! Phase 1 task 1.2. `sharecli serve` built its watch path inline from
//! `dirs::config_dir()`, while `Config::load` resolves through
//! `Config::config_path()`, which honours `SHARECLI_CONFIG_PATH`. When the
//! override is set — as it is for side-by-side installs, isolated testing and
//! any non-default location — the two disagreed, so the server loaded one file
//! and watched another. Saving to the file it had actually loaded produced no
//! reload at all.
//!
//! AC-12.1 with `SHARECLI_CONFIG_PATH` set, editing that file reloads the live
//!            config observed at `GET /config`
//! AC-12.2 the initially loaded value comes from that same file, not the default
//!
//! Guarded by `config_watch_path_uses_config_path_override` in
//! `src/commands/serve.rs`.

use std::fs;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn pick_port() -> u16 {
    // Distinct base from the other serve e2e suites so they do not collide.
    23_000 + (std::process::id() % 800) as u16
}

struct ServeChild {
    child: Child,
    port: u16,
}

impl Drop for ServeChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn curl_available() -> bool {
    Command::new("curl").arg("--version").status().map(|s| s.success()).unwrap_or(false)
}

fn wait_healthz(url: &str, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if Command::new("curl")
            .args(["-fsS", "-o", "/dev/null", "--max-time", "2", url])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

/// Live config as observed by the real `GET /config`.
fn live_projects_watched(url: &str) -> Option<String> {
    let out = Command::new("curl").args(["-sS", "--max-time", "3", url]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v.get("projects")?.get("watched")?.as_str().map(str::to_owned)
}

/// AC-12.1 / AC-12.2 — the watcher must follow `SHARECLI_CONFIG_PATH`.
#[test]
fn serve_hot_reload_follows_config_path_override() {
    if !curl_available() {
        eprintln!("skipping serve hot-reload e2e: curl not available");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "[projects]\nwatched = \"initial\"\n").expect("seed config");

    let port = pick_port();
    let mut child = bin()
        .args(["serve", "--bind", &format!("127.0.0.1:{port}")])
        .env("SHARECLI_CONFIG_PATH", &config_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sharecli serve");

    let config_url = format!("http://127.0.0.1:{port}/config");
    let healthz = format!("http://127.0.0.1:{port}/healthz");

    let mut ok = true;
    let mut detail = String::new();

    if !wait_healthz(&healthz, Duration::from_secs(30)) {
        ok = false;
        detail.push_str("serve /healthz never became reachable\n");
    } else {
        // AC-12.2: initial value must come from the overridden file.
        match live_projects_watched(&config_url) {
            Some(v) if v == "initial" => {}
            Some(v) => {
                ok = false;
                detail.push_str(&format!("initial value {v:?} != \"initial\"\n"));
            }
            None => {
                ok = false;
                detail.push_str("GET /config did not expose projects.watched\n");
            }
        }

        // AC-12.1: saving to the overridden file must reload the live config.
        fs::write(&config_path, "[projects]\nwatched = \"reloaded\"\n").expect("rewrite config");

        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut seen = None;
        while std::time::Instant::now() < deadline {
            seen = live_projects_watched(&config_url);
            if seen.as_deref() == Some("reloaded") {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }

        match seen.as_deref() {
            Some("reloaded") => {}
            other => {
                ok = false;
                detail.push_str(&format!(
                    "config hot-reload did not fire for SHARECLI_CONFIG_PATH; \
                     GET /config still reports {other:?}\n"
                ));
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(ok, "serve hot-reload e2e failed:\n{detail}");
}
