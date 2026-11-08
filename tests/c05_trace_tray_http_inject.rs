// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 L44 — traceparent inject on tray dashboard HTTP paths (T-610 / W14).
//! FR: FR-003

use std::process::Command;
use std::sync::Mutex;

use sharecli::otel::{apply_traceparent_spawn_env, traceparent_http_value};
use sharecli::tray_http::{get, inject_dashboard_traceparent};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Tray FFI path: `apply_traceparent_spawn_env` forwards operator `traceparent`
/// into `TRACEPARENT` on the child command (sharecli-ipc sidecar).
#[test]
fn tray_ipc_spawn_injects_traceparent_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    let key = "traceparent";
    let prev = std::env::var(key).ok();
    let prev_upper = std::env::var("TRACEPARENT").ok();
    std::env::remove_var("TRACEPARENT");
    std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");

    let (bin, args) = traceparent_child_cmd();
    let mut cmd = Command::new(bin);
    cmd.args(args);
    apply_traceparent_spawn_env(&mut cmd);
    let output = cmd.output().expect("spawn ipc sidecar stand-in");
    let got = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if let Some(prev) = prev {
        std::env::set_var(key, prev);
    } else {
        std::env::remove_var(key);
    }
    if let Some(prev) = prev_upper {
        std::env::set_var("TRACEPARENT", prev);
    }

    assert_eq!(got, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");
}

/// Tray dashboard HTTP: `traceparent_http_value` maps operator env for outbound GET.
#[test]
fn tray_dashboard_http_maps_traceparent_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    let key = "traceparent";
    let prev = std::env::var(key).ok();
    let prev_upper = std::env::var("TRACEPARENT").ok();
    std::env::remove_var("TRACEPARENT");
    std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");

    let out = traceparent_http_value();

    if let Some(prev) = prev {
        std::env::set_var(key, prev);
    } else {
        std::env::remove_var(key);
    }
    if let Some(prev) = prev_upper {
        std::env::set_var("TRACEPARENT", prev);
    }

    assert_eq!(out, Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string()));
}

/// Serve dashboard HTML embeds `data-traceparent` for in-page fetch propagation.
#[test]
fn tray_dashboard_html_injects_traceparent_attribute() {
    let _guard = ENV_LOCK.lock().unwrap();
    let key = "traceparent";
    let prev = std::env::var(key).ok();
    std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");

    let html = inject_dashboard_traceparent("<html lang=\"en\"><body></body></html>");

    if let Some(prev) = prev {
        std::env::set_var(key, prev);
    } else {
        std::env::remove_var(key);
    }

    assert!(html
        .contains("data-traceparent=\"00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01\""));
}

/// Tray HTTP client sends `traceparent` on loopback GET (dashboard health probe).
#[test]
fn tray_dashboard_http_get_sends_traceparent_header() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let _guard = ENV_LOCK.lock().unwrap();
    let key = "traceparent";
    let prev = std::env::var(key).ok();
    std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            headers.push_str(&line);
        }
        tx.send(headers).unwrap();
        let response =
            "HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
        stream.write_all(response.as_bytes()).unwrap();
    });

    let url = format!("http://{addr}/healthz");
    let body = get(&url).expect("tray dashboard GET");

    if let Some(prev) = prev {
        std::env::set_var(key, prev);
    } else {
        std::env::remove_var(key);
    }

    let headers = rx.recv_timeout(Duration::from_secs(5)).expect("headers captured");
    handle.join().expect("server thread");
    assert_eq!(body, "{\"status\":\"ok\"}");
    assert!(
        headers.contains("traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    );
}

fn traceparent_child_cmd() -> (&'static str, Vec<&'static str>) {
    #[cfg(unix)]
    {
        ("printenv", vec!["TRACEPARENT"])
    }
    #[cfg(windows)]
    {
        (
            "powershell",
            vec!["-NoProfile", "-NonInteractive", "-Command", "Write-Output $env:TRACEPARENT"],
        )
    }
}
