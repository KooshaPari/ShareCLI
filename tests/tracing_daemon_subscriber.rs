//! Audit task 1.11 (lane 5) — daemonized (non-TTY) tracing subscriber.
//!
//! PLAN: `docs/audit/2026-09-20/PLAN.md` lines 183-185 —
//! "main.rs:906,915 — always install subscriber; route to syslog/journald when
//! stderr is not a TTY."
//! FINDINGS: `docs/audit/2026-09-20/FINDINGS.md:107` — "no tracing subscriber
//! when stderr is not a TTY; daemonized sharecli serve emits zero logs."
//!
//! FR/AC: UNKNOWN — PLAN task 1.11 names no FR or AC id (do not infer one).
//!
//! Invariant: the FR-007 stderr-silent contract (25 test files) requires stderr
//! to stay empty on a piped run, so the daemon route must never add a stderr
//! formatting layer.

use std::process::{Command, Stdio};

use sharecli::log_sink::{plan_subscriber, LogSinkRoute, SubscriberPlan};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sharecli")
}

/// Task 1.11 — the install/route decision is pure so it can be pinned without a
/// PTY: a piped (non-TTY) run must still install a subscriber and must not
/// attach a stderr console layer.
#[test]
fn non_tty_plan_installs_daemon_route() {
    assert_eq!(
        plan_subscriber(false, false, false),
        SubscriberPlan { install: true, route: LogSinkRoute::Daemon, level: tracing::Level::INFO },
        "non-TTY run MUST install a subscriber on the daemon route at INFO (PLAN 1.11)"
    );
}

/// Task 1.11 — an interactive (TTY) run keeps the stderr console sink.
#[test]
fn tty_plan_installs_console_route() {
    assert_eq!(
        plan_subscriber(false, false, true),
        SubscriberPlan { install: true, route: LogSinkRoute::Console, level: tracing::Level::INFO },
        "TTY run MUST install a subscriber on the console route at INFO (PLAN 1.11)"
    );
}

/// Task 1.11 — `--verbose` selects the console route at DEBUG even without a TTY
/// (existing behaviour that must be preserved).
#[test]
fn verbose_plan_is_console_at_debug() {
    assert_eq!(
        plan_subscriber(false, true, false),
        SubscriberPlan {
            install: true,
            route: LogSinkRoute::Console,
            level: tracing::Level::DEBUG
        },
        "--verbose MUST keep the console route at DEBUG (PLAN 1.11)"
    );
}

/// Task 1.11 — `--quiet` is an explicit opt-out and stays the only case that
/// skips installation (documented decision; PLAN text does not name it).
#[test]
fn quiet_plan_skips_installation() {
    let plan = plan_subscriber(true, true, true);
    assert!(
        !plan.install,
        "--quiet MUST skip subscriber installation (PLAN 1.11; documented decision)"
    );
}

/// Task 1.11 — a non-TTY (daemonized) run installs the subscriber and routes
/// events to the daemon log sink instead of the (absent) stderr console sink.
#[test]
#[serial_test::serial]
fn daemonized_run_writes_logs_to_log_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log_path = tmp.path().join("nested").join("sharecli.log");

    let out = Command::new(binary())
        .arg("proc")
        .env("SHARECLI_LOG_PATH", &log_path)
        .stdin(Stdio::null())
        .output()
        .expect("spawn sharecli proc");

    assert!(
        out.status.success(),
        "proc MUST exit 0; status {:?}; stderr: {:?}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // FR-007 stderr-silent contract: piped stderr must stay empty, so the
    // daemon route must not attach a stderr formatting layer.
    assert!(
        out.stderr.is_empty(),
        "daemonized run MUST keep stderr silent (FR-007 stderr-silent contract); stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log_path_display = log_path.display().to_string();
    let contents = std::fs::read_to_string(&log_path).unwrap_or_else(|err| {
        panic!(
            "daemonized run MUST install the tracing subscriber and write {} (PLAN 1.11 / \
             FINDINGS.md:107); error: {err}",
            log_path.display()
        )
    });

    assert!(
        contents.contains("INFO"),
        "daemon log MUST contain a level-tagged INFO line (PLAN 1.11); got: {contents:?}"
    );
    assert!(
        contents.contains("sharecli log file"),
        "daemon log MUST record the log-sink startup line (PLAN 1.11); got: {contents:?}"
    );
    assert!(
        contents.contains(&log_path_display),
        "daemon log MUST record its own log path {log_path_display} (PLAN 1.11); got: {contents:?}"
    );
}
