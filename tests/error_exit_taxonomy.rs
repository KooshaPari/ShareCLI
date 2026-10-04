//! FR: NFR-003 (error handling)
//!
//! Audit task 1.10 — `src/error.rs` CLI exit-code taxonomy (PLAN lines 179-181).
//!
//! The plan text names no FR/AC id for task 1.10, so the annotation above
//! cites the FR.md requirement these assertions verify (NFR-003, Error
//! Handling) rather than inventing a task-level id: FR UNKNOWN for the task.
//!
//! Contract under test (docs/audit/2026-09-20/PLAN.md:180):
//! distinct `UserInput → 64`, `NotFound → 2`, `Internal → 1`.

use sharecli::error::{ErrorCode, SharecliError};

/// PLAN.md:180 — `NotFound` is exit 2 (the audit value, not the sysexits 69).
#[test]
fn not_found_exits_2() {
    let err = SharecliError::not_found("no such pid: 999999");
    assert_eq!(err.exit_code(), 2, "NotFound MUST exit 2 (PLAN.md:180)");
    assert_eq!(err.code(), ErrorCode::NotFound, "NotFound must keep its code name");
}

/// PLAN.md:180 — `UserInput` stays 64 (already true before this task).
#[test]
fn user_input_exits_64() {
    let err = SharecliError::user_input("unknown theme 'nope'");
    assert_eq!(err.exit_code(), 64, "UserInput MUST exit 64 (PLAN.md:180)");
    assert_eq!(err.code(), ErrorCode::UserInput);
}

/// PLAN.md:180 — `Internal` stays 1 (already true before this task).
#[test]
fn internal_exits_1() {
    let err = SharecliError::internal("boom");
    assert_eq!(err.exit_code(), 1, "Internal MUST exit 1 (PLAN.md:180)");
    assert_eq!(err.code(), ErrorCode::Internal);
}

/// The three audit codes stay distinct from each other and from success.
#[test]
fn taxonomy_codes_are_distinct() {
    let codes = [
        SharecliError::user_input("x").exit_code(),
        SharecliError::not_found("x").exit_code(),
        SharecliError::internal("x").exit_code(),
    ];
    assert_eq!(codes, [64, 2, 1], "PLAN.md:180 taxonomy");
}

/// PLAN.md:181 — an anyhow conversion must not print the top frame twice.
///
/// `From<anyhow::Error>` stores the message as *both* the top frame and the
/// source, which made stderr carry `error: X` then `caused by: X`.
#[test]
fn anyhow_top_frame_is_printed_once() {
    let err = SharecliError::from(anyhow::anyhow!("boom"));
    for debug in [false, true] {
        let rendered = err.render_stderr(debug);
        assert_eq!(rendered.matches("boom").count(), 1, "debug={debug}: {rendered}");
    }
    assert!(
        err.render_stderr(false).starts_with("SHARECLI_ERROR_CODE=internal error: boom"),
        "the SHARECLI_ERROR_CODE line MUST be preserved"
    );
}

/// `From<std::io::Error>` has the same repeating shape and must dedupe too.
#[test]
fn io_conversion_deduplicates_its_message() {
    let err = SharecliError::from(std::io::Error::other("disk full"));
    assert_eq!(err.render_stderr(true).matches("disk full").count(), 1);
}

/// PLAN.md:181 / FINDINGS.md:84-85 — a genuinely distinct cause survives, but
/// only inside the debug block.
#[test]
fn distinct_cause_is_debug_only() {
    let err =
        SharecliError::io("failed to read config", std::io::Error::other("permission denied"));
    let quiet = err.render_stderr(false);
    assert!(!quiet.contains("caused by"), "debug detail must be off by default: {quiet}");
    let debug = err.render_stderr(true);
    assert!(debug.contains("\n  ↳ caused by: permission denied"), "{debug}");
    assert!(debug.starts_with("SHARECLI_ERROR_CODE=io error: failed to read config"), "{debug}");
}

/// A chained cause keeps only the frames below the top one.
#[test]
fn chained_cause_drops_the_repeated_top_frame() {
    use anyhow::Context as _;
    let err = SharecliError::from(anyhow::anyhow!("outer").context("inner detail"));
    let rendered = err.render_stderr(true);
    assert_eq!(rendered.matches("inner detail").count(), 1, "{rendered}");
    assert!(rendered.ends_with("↳ caused by: outer"), "{rendered}");
}
