//! E2E tier — serve observability layer order over real HTTP.
//!
//! FR: FR-004 (observability) / serve middleware layering
//!
//! Phase 1 task 1.1. `Router::layer` wraps everything added before it, so the
//! *last* `.layer(...)` call is outermost. Observability used to be applied
//! **first**, which made it innermost: an auth `401` and a rate-limit `429`
//! short-circuited before ever reaching it, so those failures were invisible
//! to RED metrics and the response carried no `traceparent`.
//!
//! This drives the REAL `sharecli serve` binary over a real socket:
//! AC-11.1 unauthenticated request to a protected route → `401` **with** a
//!            well-formed `traceparent` header
//! AC-11.2 that `401` is counted by the real `GET /metrics/prometheus` RED
//!            counter `sharecli_http_unauthorized_total`
//! AC-11.3 controls: authenticated `/config` still `200`, public `/healthz`
//!            still reachable without credentials
//!
//! Guarded by `observability_records_auth_401` and
//! `observability_records_rate_limit_429` in `src/commands/serve.rs`, which
//! drive the same stack at router level.

use std::fs;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

const TOKEN: &str = "e2e-observability-token";

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn pick_port() -> u16 {
    // Distinct base from `e2e_serve_healthz` so the two suites do not collide.
    21_000 + (std::process::id() % 800) as u16
}

struct ServeChild {
    child: Child,
    port: u16,
}

impl ServeChild {
    fn spawn(port: u16) -> Self {
        let child = bin()
            .args(["serve", "--bind", &format!("127.0.0.1:{port}")])
            .env("SHARECLI_SERVE_TOKEN", TOKEN)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sharecli serve");
        Self { child, port }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }
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

fn skip(reason: &str) {
    eprintln!("skipping serve observability e2e: {reason}");
}

/// `curl -sS -D <headers> -o /dev/null [-H auth] <url>` → (status, headers).
fn head(url: &str, auth: Option<&str>, headers_path: &str) -> Option<(u16, String)> {
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "-o", "/dev/null", "-D", headers_path, "--max-time", "5"]);
    if let Some(a) = auth {
        cmd.args(["-H", &format!("Authorization: {a}")]);
    }
    let status = cmd.arg(url).status().ok()?.code()?;
    if status != 0 {
        // curl reports the HTTP status as its own exit code only with -f,
        // which we deliberately omit; a non-zero code here is a transport error.
        return None;
    }
    let text = fs::read_to_string(headers_path).ok()?;
    let code = text
        .lines()
        .find_map(|l| l.strip_prefix("HTTP/1.1 ").or_else(|| l.strip_prefix("HTTP/2 ")))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|c| c.parse::<u16>().ok())?;
    Some((code, text))
}

/// Body of `curl -sS [-H auth] <url>`.
fn body(url: &str, auth: Option<&str>) -> Option<String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--max-time", "5"]);
    if let Some(a) = auth {
        cmd.args(["-H", &format!("Authorization: {a}")]);
    }
    let out = cmd.arg(url).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
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

fn header_value(headers: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}:");
    headers.lines().find_map(|l| l.strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

fn unauthorized_total(metrics: &str) -> u64 {
    metrics
        .lines()
        .find_map(|l| l.strip_prefix("sharecli_http_unauthorized_total "))
        .and_then(|v| v.trim().parse::<u64>().ok())
        .expect("sharecli_http_unauthorized_total exposed by /metrics/prometheus")
}

/// AC-11.1 / AC-11.2 / AC-11.3 — real serve process, real 401 reaches RED.
#[test]
fn serve_observability_records_auth_401_e2e() {
    if !curl_available() {
        skip("curl not available");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let headers = dir.path().join("401.headers");

    let port = pick_port();
    let serve = ServeChild::spawn(port);
    let healthz = serve.url("/healthz");

    assert!(
        wait_healthz(&healthz, Duration::from_secs(30)),
        "serve /healthz must become reachable at {healthz}"
    );

    let auth = format!("Bearer {TOKEN}");

    // Baseline: RED counter before the unauthenticated request.
    let metrics_before = body(&serve.url("/metrics/prometheus"), Some(&auth))
        .expect("GET /metrics/prometheus with valid bearer");
    let before = unauthorized_total(&metrics_before);

    // AC-11.1: unauthenticated request to a protected route.
    let (status, header_text) =
        head(&serve.url("/config"), None, headers.to_str().expect("utf-8 temp path"))
            .expect("unauthenticated GET /config must reach the server");
    assert_eq!(status, 401, "protected /config without credentials");

    let traceparent = header_value(&header_text, "traceparent");
    let tp = traceparent.as_deref().unwrap_or("<missing>");
    assert!(
        traceparent.is_some(),
        "401 response must carry a traceparent header; got headers:\n{header_text}"
    );
    let valid_tp = |v: &str| {
        let p: Vec<&str> = v.split('-').collect();
        p.len() == 4 && p[0] == "00" && p[1].len() == 32 && p[2].len() == 16 && p[3].len() == 2
    };
    assert!(valid_tp(tp), "traceparent must be W3C-shaped (00-<32>-<16>-<2>), got {tp:?}");

    // AC-11.2: that 401 must now be visible in the real RED metrics.
    let metrics_after = body(&serve.url("/metrics/prometheus"), Some(&auth))
        .expect("GET /metrics/prometheus with valid bearer");
    let after = unauthorized_total(&metrics_after);
    assert!(after > before, "401 must be counted in RED metrics: before={before} after={after}");

    // AC-11.3a: control — valid credentials still authorise.
    let (auth_status, _) =
        head(&serve.url("/config"), Some(&auth), headers.to_str().expect("utf-8 temp path"))
            .expect("authenticated GET /config must reach the server");
    assert_eq!(auth_status, 200, "authenticated /config with valid bearer");

    // AC-11.3b: control — a public route is not swallowed by the reorder.
    let (health_status, _) =
        head(&serve.url("/healthz"), None, headers.to_str().expect("utf-8 temp path"))
            .expect("public GET /healthz must reach the server");
    assert_eq!(health_status, 200, "public /healthz must stay unauthenticated");
}
