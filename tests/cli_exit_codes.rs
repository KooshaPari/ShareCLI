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
///
/// `rust_backtrace` selects the child's `RUST_BACKTRACE` value: `None` removes
/// it (the production default; `debug_causes_enabled()` is then false) and
/// `Some(value)` sets it.
fn spawn(home: &Path, args: &[&str], rust_backtrace: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sharecli"));
    command.args(args).env("HOME", home).env_remove("SHARECLI_LOG_PATH");
    match rust_backtrace {
        Some(value) => {
            command.env("RUST_BACKTRACE", value);
        }
        None => {
            command.env_remove("RUST_BACKTRACE");
        }
    }
    command.output().unwrap_or_else(|e| panic!("spawn sharecli {args:?}: {e}"))
}

/// Spawn the real sharecli binary with an isolated HOME and no debug detail.
fn run(home: &Path, args: &[&str]) -> Output {
    spawn(home, args, None)
}

/// Spawn with `RUST_BACKTRACE=1`, the value `debug_causes_enabled()` accepts.
///
/// `SharecliError::render_stderr` appends the `↳ caused by:` continuation only
/// when that predicate is true (src/error.rs:219-221), so this is the only
/// configuration in which a duplicated cause can reach stderr.
fn run_with_debug_causes(home: &Path, args: &[&str]) -> Output {
    spawn(home, args, Some("1"))
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
/// A bare `anyhow::bail!` reaches `From<anyhow::Error>`, which stores the
/// message as both the top frame and the source, so the cause renders
/// identically to the frame above it. Pre-1.10 `eprint` printed that cause
/// unconditionally, so stderr carried `error: X` immediately followed by
/// `caused by: X`.
///
/// The only line that can duplicate the message is the `↳ caused by:`
/// continuation, emitted solely while `debug_causes_enabled()` is true — i.e.
/// while `RUST_BACKTRACE` is set. A run that removes the variable therefore
/// never reaches the line it guards, and the count is trivially one. Assert
/// with debug causes ON as well: that is the configuration in which the dedupe
/// must hold, so the check can actually fail if it regresses.
#[test]
fn error_message_is_printed_once() {
    let home = tempfile::tempdir().expect("tempdir");

    let plain = stderr(&run(home.path(), &["stop"]));
    assert_eq!(
        plain.matches("to select what to stop").count(),
        1,
        "the internal message MUST appear once (PLAN.md:181); stderr={plain}"
    );

    let debug = stderr(&run_with_debug_causes(home.path(), &["stop"]));
    assert_eq!(
        debug.matches("to select what to stop").count(),
        1,
        "the internal message MUST still appear once with debug causes on (PLAN.md:181); \
         stderr={debug}"
    );
    assert!(
        !debug.contains("↳ caused by:"),
        "a cause that only repeats the top frame MUST be suppressed; stderr={debug}"
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
