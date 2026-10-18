// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C10 L100 — designed empty / zero-data states with actionable CTAs.
//! FR: FR-003

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// FR-003 / C10 L100 — idle `ps` prints headers plus a get-started CTA.
#[test]
fn c10_l100_ps_idle_pool_prints_get_started_cta() {
    let out = bin().args(["ps"]).output().expect("spawn sharecli ps");
    assert!(out.status.success(), "`ps` should exit 0; stderr: {:?}", out.stderr);
    let s = stdout(&out);
    assert!(s.contains("PID"), "ps MUST print table headers; got: {s}");
    assert!(s.contains("No managed processes yet"), "idle ps MUST explain empty pool; got: {s}");
    assert!(s.contains("sharecli start"), "idle ps MUST suggest start command; got: {s}");
    assert!(s.contains("sharecli serve"), "idle ps MUST suggest dashboard command; got: {s}");
}

/// FR-003 / C10 L100 — filtered-empty copy differs from first-run idle pool.
#[test]
fn c10_l100_ps_filtered_empty_prints_match_hint() {
    let out = bin()
        .args(["ps", "--project", "nonexistent-project-xyz"])
        .output()
        .expect("spawn sharecli ps --project");
    assert!(out.status.success(), "filtered ps should exit 0; stderr: {:?}", out.stderr);
    let s = stdout(&out);
    assert!(
        s.contains("No processes match project"),
        "filtered ps MUST explain zero matches; got: {s}"
    );
    assert!(
        s.contains("sharecli start nonexistent-project-xyz"),
        "filtered ps MUST echo project in CTA; got: {s}"
    );
    assert!(
        !s.contains("No managed processes yet"),
        "filtered ps MUST NOT use first-run copy; got: {s}"
    );
}

/// FR-003 / C10 L100 — dashboard ships first-run vs cleared empty-state branches.
#[test]
fn c10_l100_dashboard_empty_state_markup() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dashboard.html");
    let html = std::fs::read_to_string(&path).expect("read dashboard.html");
    assert!(html.contains("empty-state"), "dashboard MUST define empty-state panel");
    assert!(html.contains("data-empty-kind"), "dashboard MUST distinguish empty kinds");
    assert!(html.contains("'first-run'"), "dashboard MUST include first-run branch");
    assert!(html.contains("'cleared'"), "dashboard MUST include cleared branch");
    assert!(html.contains("sharecli start"), "dashboard first-run CTA MUST mention start command");
}
