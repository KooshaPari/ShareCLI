// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-004 NFR — C09 L81.7 system status visibility: indicatif ETA on batch CLI ops.
//! FR: FR-004

use sharecli::progress::{progress_enabled, StepProgress, PROGRESS_MIN_ITEMS};

/// Progress helpers are wired into the library surface (indicatif dep + module).
#[test]
fn fr004_progress_module_exports() {
    const { assert!(PROGRESS_MIN_ITEMS >= 2) };
    // CI runs without stderr TTY — must not panic.
    let _ = progress_enabled();
    let bar = StepProgress::new("test batch", 5);
    bar.inc(Some("step"));
    bar.finish("done");
}

/// Batch stop/prune commands must succeed in non-TTY CI (line mode, no bar).
#[test]
fn fr004_prune_dry_run_succeeds_without_tty_bar() {
    use std::process::Command;

    let out = Command::new(env!("CARGO_BIN_EXE_sharecli"))
        .args(["prune", "--idle-seconds", "999999"])
        .output()
        .expect("spawn prune");
    assert!(
        out.status.success(),
        "prune dry-run must succeed in CI; combined={}",
        String::from_utf8_lossy(&out.stdout)
    );
}
