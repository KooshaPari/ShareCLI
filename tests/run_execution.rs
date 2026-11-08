//! FR: FR-001 - run must fail closed until command execution is wired.
use std::process::Command;

#[test]
fn run_rejects_unavailable_execution_without_success_output() {
    let state = tempfile::tempdir().unwrap();
    for harness in ["node", "bun", "missing-backend"] {
        let output = Command::new(env!("CARGO_BIN_EXE_sharecli"))
            .args(["run", harness, "synthetic-project"])
            .env("HOME", state.path())
            .env("XDG_CONFIG_HOME", state.path())
            .env("PATH", state.path())
            .output()
            .expect("candidate CLI starts even with no runtime on PATH");
        assert!(!output.status.success(), "unavailable execution cannot succeed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stdout.contains("Pooled"), "{stdout}");
        assert!(!stdout.contains("Output:"), "{stdout}");
        assert!(stderr.contains("no command execution backend"), "{stderr}");
        assert!(!stderr.contains("Failed to spawn"), "must reject before spawn: {stderr}");
    }
}

#[cfg(unix)]
#[test]
fn run_does_not_even_probe_available_harnesses() {
    use std::os::unix::fs::PermissionsExt;
    let state = tempfile::tempdir().unwrap();
    let marker = state.path().join("invoked");
    for harness in ["node", "bun"] {
        let probe = state.path().join(harness);
        std::fs::write(&probe, "#!/bin/sh\n: > \"$PROBE_MARKER\"\nexit 0\n").unwrap();
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_sharecli"))
            .args(["run", harness, "synthetic-project"])
            .env("HOME", state.path())
            .env("XDG_CONFIG_HOME", state.path())
            .env("PATH", state.path())
            .env("PROBE_MARKER", &marker)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!marker.exists(), "unsupported execution must reject before probing {harness}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("no command execution backend"));
    }
}

/// FR-008: characterize the existing native queued adapter separately from unavailable pooled run.
#[cfg(unix)]
#[tokio::test]
async fn native_queue_preserves_output_exit_and_spawn_failure() {
    use sharecli_core::{FakeThermalGate, Hypervisor, SpawnRequest, ThermalDecision};
    use std::sync::Arc;
    let state = tempfile::tempdir().unwrap();
    let hv = Hypervisor::with_thermal_gate(
        state.path(),
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
    );
    let request = SpawnRequest::new(
        vec![
            "/bin/sh".into(),
            "-c".into(),
            "printf native-out; printf native-err >&2; exit 7".into(),
        ],
        state.path().to_path_buf(),
        vec![],
    );
    let output = hv.run_queued(request, "execution-witness").await.unwrap();
    assert_eq!(output.stdout, b"native-out");
    assert_eq!(output.stderr, b"native-err");
    assert_eq!(output.exit_code, 7);
    assert!(!output.from_cache);
    let missing = SpawnRequest::new(
        vec![state.path().join("absent-command").to_string_lossy().into_owned()],
        state.path().to_path_buf(),
        vec![],
    );
    assert!(hv.run_queued(missing, "execution-witness").await.is_err());
    // A failed spawn must return its queue slot for the next actual command.
    let success = SpawnRequest::new(
        vec!["/bin/sh".into(), "-c".into(), "printf reclaimed".into()],
        state.path().to_path_buf(),
        vec![],
    );
    let output = hv.run_queued(success, "execution-witness").await.unwrap();
    assert_eq!(output.stdout, b"reclaimed");
    assert_eq!(output.exit_code, 0);
}
