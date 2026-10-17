// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 L44 — traceparent inject on IPC/tray spawn paths (T-530 / W13.4).
//! FR: FR-003

use std::process::Command;

use sharecli::otel::apply_traceparent_spawn_env;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
