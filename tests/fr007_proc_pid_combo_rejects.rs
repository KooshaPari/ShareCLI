// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli proc --pid` inventory-flag rejection (AC-007.92)
//! FR: FR-007
//!
//! Detail mode (`--pid`) MUST fail loudly when paired with inventory filters/tree.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn assert_pid_combo_rejected(extra: &[&str], must_mention: &str) {
    let self_pid = std::process::id().to_string();
    let mut args = vec!["proc", "--pid", self_pid.as_str()];
    args.extend_from_slice(extra);
    let out = bin().args(&args).output().unwrap_or_else(|e| panic!("spawn proc --pid …: {e}"));
    assert!(
        !out.status.success(),
        "proc --pid {} MUST fail (AC-007.92); stdout={} stderr={}",
        extra.join(" "),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.contains(must_mention) && combined.contains("AC-007.92"),
        "error MUST mention {must_mention} and AC-007.92; got: {combined}"
    );
}

/// FR-007 / AC-007.92 — --pid --tree rejected.
#[test]
fn fr007_proc_pid_rejects_tree() {
    assert_pid_combo_rejected(&["--tree"], "--tree");
}

/// FR-007 / AC-007.92 — --pid --family rejected.
#[test]
fn fr007_proc_pid_rejects_family() {
    assert_pid_combo_rejected(&["--family", "claude"], "--family");
}

/// FR-007 / AC-007.92 — --pid --sort --limit rejected.
#[test]
fn fr007_proc_pid_rejects_sort_limit() {
    assert_pid_combo_rejected(&["--sort", "rss", "--limit", "3"], "--sort");
}

/// FR-007 / AC-007.92 — --pid --csv remains allowed (AC-007.86); not inventory.
#[test]
fn fr007_proc_pid_csv_still_allowed() {
    let self_pid = std::process::id().to_string();
    let out =
        bin().args(["proc", "--pid", &self_pid, "--csv"]).output().expect("spawn proc --pid --csv");
    assert!(
        out.status.success(),
        "proc --pid --csv MUST succeed (AC-007.86); stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}
