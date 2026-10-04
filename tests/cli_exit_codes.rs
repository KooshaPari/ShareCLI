//! FR: FR-001 (stop-miss error reporting), NFR-003 (error handling)
//!
//! Audit task 1.10 — end-to-end CLI exit codes and single-copy error output.
//!
//! PLAN.md lines 179-181: distinct `UserInput → 64`, `NotFound → 2`,
//! `Internal → 1`; stop double-printing. The plan itself names no FR/AC id for
//! this task, so the annotation above cites the FR.md requirements these
//! assertions actually verify instead of inventing a task-level id: FR-001
//! (Managed Process Lifecycle — `stop` must report an unmanaged pid as an
//! error) and NFR-003 (Error Handling).
//!
//! These run the real binary (`CARGO_BIN_EXE_sharecli`) with a scratch `HOME`
//! so no user config or log file is read or written.

use std::path::Path;
use std::process::{Command, Output};

/// Spawn the real sharecli binary with an isolated HOME.
fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
        .args(args)
        .env("HOME", home)
        .env_remove("SHARECLI_LOG_PATH")
        .env_remove("RUST_BACKTRACE")
        .output()
        .unwrap_or_else(|e| panic!("spawn sharecli {args:?}: {e}"))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// PLAN.md:180 — a `NotFound` miss exits 2 and announces the taxonomy code.
///
/// `stop --pid 999999` in a pool that never managed the pid is the only
/// production `NotFound` path (`src/commands/mod.rs`, previously a raw
/// `std::process::exit(2)` that bypassed `src/error.rs`).
#[test]
fn stop_unmanaged_pid_exits_2_through_the_taxonomy() {
    let home = tempfile::tempdir().expect("tempdir");
    let out = run(home.path(), &["stop", "--pid", "999999"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(2), "stop on an unmanaged pid MUST exit 2; stderr={err}");
    assert!(
        err.contains("SHARECLI_ERROR_CODE=not_found"),
        "the miss MUST route through the error taxonomy (PLAN.md:180); stderr={err}"
    );
    assert!(
        err.contains("no such pid: 999999"),
        "the user-visible message MUST be preserved; stderr={err}"
    );
}

/// PLAN.md:180 — an untyped internal failure exits 1.
#[test]
fn internal_failure_exits_1() {
    let home = tempfile::tempdir().expect("tempdir");
    let out = run(home.path(), &["stop"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "Internal MUST exit 1 (PLAN.md:180); stderr={err}");
    assert!(err.contains("SHARECLI_ERROR_CODE=internal"), "stderr={err}");
}

/// PLAN.md:181 — the same message MUST NOT be printed twice.
///
/// A bare `anyhow::bail!` reaches `From<anyhow::Error>`, which stored the
/// message as both the top frame and the source, so stderr carried
/// `error: X` immediately followed by `caused by: X`.
#[test]
fn error_message_is_printed_once() {
    let home = tempfile::tempdir().expect("tempdir");
    let out = run(home.path(), &["stop"]);
    let err = stderr(&out);
    assert_eq!(
        err.matches("to select what to stop").count(),
        1,
        "the internal message MUST appear once (PLAN.md:181); stderr={err}"
    );
}

/// PLAN.md:180 — `UserInput` exits 64 end to end (the `--theme` producer).
#[test]
fn unknown_theme_exits_64() {
    let home = tempfile::tempdir().expect("tempdir");
    let out = run(home.path(), &["--theme", "nope", "ps"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(64), "UserInput MUST exit 64 (PLAN.md:180); stderr={err}");
    assert!(err.contains("SHARECLI_ERROR_CODE=user_input"), "stderr={err}");
    assert!(
        err.matches("unknown theme 'nope'").count() == 1,
        "the message MUST appear once; stderr={err}"
    );
}
