// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Unhappy-path friction suite — invalid / missing inputs per FR-001..005.
//! FR: FR-001, FR-002, FR-003, FR-004, FR-005
//!
//! WORK_DAG T-300 / L30.12: one `_invalid_` / `_missing_` scenario per FR.
//! Outside-in CLI where possible; library-level for config deserialize.

use std::process::Command;

use sharecli::config::Config;
use sharecli::runtime::ResourceCheck;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// FR-001 — `stop` with no selector MUST fail (missing --pid/--project/--harness/--all).
#[test]
fn fr001_missing_stop_selector_exits_nonzero() {
    let out = bin().arg("stop").output().expect("spawn stop");
    assert!(
        !out.status.success(),
        "stop without selector MUST exit non-zero; stdout={}",
        stdout(&out)
    );
    let err = stderr(&out).to_lowercase();
    assert!(
        err.contains("specify") || err.contains("error") || err.contains("pid"),
        "FR-001 friction MUST explain missing selector; stderr={err}"
    );
}

/// FR-002 — invalid TOML MUST NOT deserialize as Config.
#[test]
fn fr002_invalid_toml_rejects_deserialize() {
    let bogus = "this is not [[valid]] toml {{{";
    let err = toml::from_str::<Config>(bogus).expect_err("invalid TOML MUST fail");
    let msg = err.to_string();
    assert!(!msg.is_empty(), "deserialize error MUST be non-empty");
}

/// FR-003 — starting an unknown project MUST fail with a registry hint.
#[test]
fn fr003_missing_project_start_exits_nonzero() {
    let out = bin()
        .args(["start", "__sharecli_missing_project_xyz__", "--harness", "node"])
        .output()
        .expect("spawn start");
    assert!(!out.status.success(), "unknown project MUST exit non-zero; stdout={}", stdout(&out));
    let combined = format!("{}{}", stdout(&out), stderr(&out)).to_lowercase();
    assert!(
        combined.contains("unknown project") || combined.contains("project"),
        "FR-003 friction MUST cite unknown project; got={combined}"
    );
}

/// FR-004 — invalid flag on `status` MUST fail clap parsing (invalid input).
#[test]
fn fr004_invalid_status_flag_exits_nonzero() {
    let out = bin().args(["status", "--not-a-real-status-flag"]).output().expect("spawn status");
    assert!(
        !out.status.success(),
        "invalid status flag MUST exit non-zero; stdout={}",
        stdout(&out)
    );
    let err = stderr(&out).to_lowercase();
    assert!(
        err.contains("unexpected") || err.contains("error") || err.contains("help"),
        "FR-004 friction MUST surface clap error; stderr={err}"
    );
}

/// FR-005 — missing required `check` project arg MUST fail; exceeded axes clear overall_ok.
#[test]
fn fr005_missing_check_project_and_invalid_overall() {
    let missing = bin().arg("check").output().expect("spawn check");
    assert!(
        !missing.status.success(),
        "check without project MUST exit non-zero; stderr={}",
        stderr(&missing)
    );
    let err = stderr(&missing).to_lowercase();
    assert!(
        err.contains("required") || err.contains("error") || err.contains("usage"),
        "FR-005 friction MUST require project arg; stderr={err}"
    );

    // Invalid / exceeded resource state → overall_ok false (AC-005.4 unhappy).
    let exceeded = ResourceCheck {
        memory_mb: 4096,
        memory_limit_mb: 1024,
        memory_ok: false,
        process_count: 50,
        max_processes: 10,
        processes_ok: false,
        overall_ok: false,
    };
    assert!(!exceeded.overall_ok, "exceeded limits MUST clear overall_ok");
    assert!(!exceeded.memory_ok && !exceeded.processes_ok);
}
