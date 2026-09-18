//! Outside-in Quick Start journey — maps CLI steps → FR IDs.
//! FR: FR-001, FR-002, FR-003
//!
//! Journey source: `docs/journeys/quick-start.md` (+ verify step FR-004).
//!
//! This is an outside-in acceptance test (L30.6 / WORK_DAG T-240): it invokes
//! the real `sharecli` binary, not library internals. Mutating steps
//! (`config init`, `project add`) are intentionally omitted so CI does not
//! write into the operator's real config directory; those flows are covered
//! by `tests/fr002_*.rs` / `tests/fr003_*.rs` under tempfile.

use std::path::Path;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn bin_with_config(config_path: &Path) -> Command {
    let mut command = bin();
    command.env("SHARECLI_CONFIG_PATH", config_path);
    command
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Quick Start journey: install → configure → register → run → verify.
///
/// | Step | CLI surface | FR |
/// |------|-------------|-----|
/// | 1. Install / identity | `-V` | NFR-001 |
/// | 2. Configure | `config --help` + `config validate` | FR-002 |
/// | 3. Register | `project --help` + `project list` | FR-003 |
/// | 4. Run | `ps` | FR-001 |
/// | 5. Verify | `status` + `health` | FR-004 |
#[test]
fn quick_start_journey_maps_steps_to_frs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_path = tmp.path().join("sharecli").join("config.toml");

    // Step 1 — NFR-001: binary under test runs and identifies as sharecli.
    let ver = bin().arg("-V").output().expect("spawn sharecli -V");
    assert!(ver.status.success(), "-V MUST exit 0; stderr: {}", stderr(&ver));
    let ver_out = stdout(&ver);
    assert!(
        ver_out.to_lowercase().contains("sharecli"),
        "install step MUST identify sharecli; got: {ver_out}"
    );

    // Step 2 — FR-002: config surface exposes init/validate; validate reports projects.
    let cfg_help = bin_with_config(&config_path)
        .args(["config", "--help"])
        .output()
        .expect("config --help");
    assert!(cfg_help.status.success(), "stderr: {}", stderr(&cfg_help));
    let cfg_help_out = stdout(&cfg_help).to_lowercase();
    assert!(
        cfg_help_out.contains("init") && cfg_help_out.contains("validate"),
        "FR-002 journey MUST advertise init+validate; got: {cfg_help_out}"
    );

    let validate = bin_with_config(&config_path)
        .args(["config", "validate"])
        .output()
        .expect("config validate");
    assert!(
        validate.status.success(),
        "config validate MUST exit 0; stderr: {}",
        stderr(&validate)
    );
    let validate_out = stdout(&validate);
    assert!(
        validate_out.contains("Configuration is valid."),
        "FR-002 validate MUST confirm validity; got: {validate_out}"
    );
    assert!(
        validate_out.contains("Projects:"),
        "FR-002 validate MUST report project count; got: {validate_out}"
    );

    // Step 3 — FR-003: project surface exposes add/list; list is readable.
    let proj_help = bin_with_config(&config_path)
        .args(["project", "--help"])
        .output()
        .expect("project --help");
    assert!(proj_help.status.success(), "stderr: {}", stderr(&proj_help));
    let proj_help_out = stdout(&proj_help).to_lowercase();
    assert!(
        proj_help_out.contains("add") && proj_help_out.contains("list"),
        "FR-003 journey MUST advertise add+list; got: {proj_help_out}"
    );

    let list = bin_with_config(&config_path)
        .args(["project", "list"])
        .output()
        .expect("project list");
    assert!(list.status.success(), "project list MUST exit 0; stderr: {}", stderr(&list));
    let list_out = stdout(&list);
    assert!(
        list_out.contains("Registered Projects:") || list_out.contains("No projects registered"),
        "FR-003 list MUST print registry status; got: {list_out}"
    );

    // Step 4 — FR-001: ps prints the process table (empty set is OK).
    let ps = bin().arg("ps").output().expect("ps");
    assert!(ps.status.success(), "ps MUST exit 0; stderr: {}", stderr(&ps));
    let ps_out = stdout(&ps);
    assert!(
        ps_out.contains("PID") && ps_out.contains("MEM"),
        "FR-001 ps MUST print table headers; got: {ps_out}"
    );

    // Step 5 — FR-004: status + health are reachable verify surfaces.
    let status = bin().arg("status").output().expect("status");
    assert!(status.status.success(), "status MUST exit 0; stderr: {}", stderr(&status));
    let status_out = stdout(&status);
    assert!(
        status_out.contains("Process Status") || status_out.contains("Shared Runtime"),
        "FR-004 status MUST print health/status sections; got: {status_out}"
    );

    let health = bin().arg("health").output().expect("health");
    assert!(health.status.success(), "health MUST exit 0; stderr: {}", stderr(&health));
    let health_out = stdout(&health);
    assert!(!health_out.trim().is_empty(), "FR-004 health MUST print a probe result; got empty");
}
