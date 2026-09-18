//! Integration tests for the sharecli CLI binary.
//! FR: FR-001
//!
//! These tests exercise the `sharecli` binary end-to-end via
//! `env!("CARGO_BIN_EXE_sharecli")`, which cargo provides automatically when a
//! binary of the same name as the package exists. The tests assert on exit
//! codes, stdout, and stderr so the contract is explicit.
//!
//! Smallest possible diff: a single new file under `tests/`. No source-code
//! changes to the CLI surface itself — the existing default binary already
//! exposes the subcommands we rely on (`--help`, `version`, `list`, `ps`,
//! `util`, `completions`).

use std::process::Command;

/// Build a `Command` for the sharecli binary under test.
///
/// Resolved at compile time via the standard cargo `CARGO_BIN_EXE_<bin>`
/// environment variable, so this works in both `cargo test` and
/// `cargo test --test integration_cli`.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// Decode the lossily-decoded stdout/stderr bytes safely.
fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn cli_help_exits_zero_and_prints_usage() {
    let out = bin().arg("--help").output().expect("spawn sharecli --help");
    assert!(out.status.success(), "--help should exit 0; stderr: {}", stderr(&out));
    let s = stdout(&out);
    assert!(
        s.contains("Usage:") || s.contains("Usage "),
        "--help output should mention Usage; got: {s}"
    );
    // The binary is named `sharecli` and should advertise itself in help.
    assert!(s.contains("sharecli"), "--help output should reference the sharecli name; got: {s}");
}

#[test]
fn cli_short_version_flag_prints_version() {
    // `-V` is the clap short for version on a CommandFactory-derived CLI.
    let out = bin().arg("-V").output().expect("spawn sharecli -V");
    assert!(out.status.success(), "-V should exit 0; stderr: {}", stderr(&out));
    let s = stdout(&out);
    assert!(
        s.contains("sharecli") && s.contains("0.8.0"),
        "-V should print 'sharecli <version>'; got: {s}"
    );
}

#[test]
fn cli_version_subcommand_prints_splash_and_version() {
    let out = bin().arg("version").output().expect("spawn sharecli version");
    assert!(out.status.success(), "`version` should exit 0; stderr: {}", stderr(&out));
    let s = stdout(&out);
    // ASCII-art splash contains the brand letters; the version string follows.
    assert!(s.contains("sharecli"), "version output should mention sharecli; got: {s}");
    assert!(s.contains("0.8.0"), "version output should print 0.8.0; got: {s}");
}

#[test]
fn cli_list_enumerates_surfaces_and_is_nonempty() {
    let out = bin().arg("list").output().expect("spawn sharecli list");
    assert!(out.status.success(), "`list` should exit 0; stderr: {}", stderr(&out));
    let s = stdout(&out);
    assert!(!s.trim().is_empty(), "`list` output should be non-empty");
    // The command advertises CLI surfaces — `cast` and `util` are the two
    // documented categories shipped with the default binary.
    assert!(s.contains("cast"), "`list` output should advertise cast; got: {s}");
    assert!(s.contains("util"), "`list` output should advertise util; got: {s}");
}

#[test]
fn cli_list_help_explains_subcommands() {
    let out = bin().args(["list", "--help"]).output().expect("spawn sharecli list --help");
    assert!(out.status.success(), "`list --help` should exit 0; stderr: {}", stderr(&out));
    let s = stdout(&out);
    assert!(
        s.to_lowercase().contains("cast") || s.to_lowercase().contains("util"),
        "`list --help` should explain cast/util surfaces; got: {s}"
    );
}

#[test]
fn cli_fuse_probe_is_read_only_and_reports_fallback_contract() {
    let out = bin()
        .args(["fuse", "probe", "/tmp/sharecli-fuse-probe-test", "--json"])
        .output()
        .expect("spawn sharecli fuse probe");
    assert!(out.status.success(), "fuse probe should exit 0; stderr: {}", stderr(&out));
    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("fuse probe should emit valid JSON");
    assert!(report.get("selected_backend").is_some(), "report must include backend");
    assert_eq!(report.get("non_fuse_fallback"), Some(&serde_json::Value::Bool(true)));
    assert!(report.get("mountpoint").is_some(), "report must include mountpoint");
}

#[test]
fn cli_ps_runs_and_prints_table_header() {
    // `ps` exits 0 even when no managed processes are alive.
    let out = bin().arg("ps").output().expect("spawn sharecli ps");
    assert!(out.status.success(), "`ps` should exit 0; stderr: {}", stderr(&out));
    let s = stdout(&out);
    // The header row from `ps` includes `PID`, `MEM(MB)`, and FR-006 `AGENT`.
    assert!(s.contains("PID"), "`ps` output should include the PID column header; got: {s}");
    assert!(s.contains("MEM"), "`ps` output should include the MEM column header; got: {s}");
    assert!(s.contains("AGENT"), "`ps` output should include the AGENT column header; got: {s}");
}

#[test]
fn cli_unknown_subcommand_exits_nonzero() {
    let out =
        bin().arg("definitely-not-a-real-subcommand-xyz").output().expect("spawn sharecli <bad>");
    assert!(
        !out.status.success(),
        "unknown subcommand should exit non-zero; got exit 0 and stdout: {}",
        stdout(&out)
    );
    // clap prints the error to stderr with the word "error".
    let err = stderr(&out);
    assert!(
        err.to_lowercase().contains("error"),
        "unknown subcommand error should mention 'error'; got stderr: {err}"
    );
}

#[test]
fn cli_completions_bash_outputs_function_definitions() {
    let out =
        bin().args(["completions", "bash"]).output().expect("spawn sharecli completions bash");
    assert!(out.status.success(), "`completions bash` should exit 0; stderr: {}", stderr(&out));
    let s = stdout(&out);
    // The bash completion script defines a function named `_sharecli`.
    assert!(s.contains("_sharecli"), "`completions bash` should define _sharecli(); got: {s}");
}
