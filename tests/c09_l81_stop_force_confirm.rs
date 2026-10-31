// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-004 NFR — C09 L81.6 error prevention: `stop --force` requires `--yes`.
//! FR: FR-004

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn combined(out: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    format!("{stdout}{stderr}")
}

/// `stop --all --force` without `--yes` MUST preview and exit zero (no kill).
#[test]
fn fr004_stop_force_without_yes_previews_and_succeeds() {
    let out = bin().args(["stop", "--all", "--force"]).output().expect("spawn stop");
    assert!(
        out.status.success(),
        "force stop without --yes MUST dry-run successfully; combined={}",
        combined(&out)
    );
    let body = combined(&out);
    assert!(body.contains("Would force-kill"), "must preview force-kill; body={body}");
    assert!(body.contains("--yes"), "must hint --yes confirmation; body={body}");
}

/// `stop --all --force --yes` on an empty pool MUST succeed without preview gate.
#[test]
fn fr004_stop_force_with_yes_on_empty_pool_succeeds() {
    let out = bin().args(["stop", "--all", "--force", "--yes"]).output().expect("spawn stop");
    assert!(out.status.success(), "confirmed force stop MUST succeed; combined={}", combined(&out));
    let body = combined(&out);
    assert!(!body.contains("Would force-kill"), "confirmed stop must not dry-run; body={body}");
}

/// `project stop --force` without `--yes` MUST preview when processes would match.
#[test]
fn fr004_project_stop_force_without_yes_previews() {
    let out = bin()
        .args(["project", "stop", "nonexistent-proj-for-preview", "--force"])
        .output()
        .expect("spawn project stop");
    assert!(
        out.status.success(),
        "empty project force-stop preview MUST succeed; combined={}",
        combined(&out)
    );
}

/// `quit` is an ergonomic alias for the destructive-process command.
#[test]
fn fr004_quit_alias_stops_all() {
    let out = bin().args(["quit", "--all"]).output().expect("spawn quit");
    assert!(out.status.success(), "quit alias MUST dispatch to stop; combined={}", combined(&out));
    let body = combined(&out);
    assert!(
        body.contains("All processes stopped."),
        "quit alias MUST use stop semantics; body={body}"
    );
}
