// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-011 / AC-011.6 — report + ps --all gate parity with status/TUI.
//!
//! AC-011.6: `sharecli report` and `sharecli ps --all` expose thermal+agent
//! gate fields (detected agent count, contention tier, ADMIT/DENY).

use sharecli::commands::report::{build_report, SortBy};
use sharecli_fleet::{gate_status_snapshot, ThermalLevel};

/// FR-011 / AC-011.6 — FleetReport JSON carries gate fields.
#[test]
fn fr011_report_json_includes_gate_fields() {
    let gate = gate_status_snapshot(ThermalLevel::Green, 8);
    let report = build_report(&[], &gate, &SortBy::Memory);
    assert_eq!(report.detected_agents, 8);
    assert_eq!(report.agent_contention, "REFUSE");
    assert_eq!(report.gate_decision, "DENY");

    let json = serde_json::to_string(&report).expect("serialize report");
    assert!(json.contains("\"detected_agents\":8"));
    assert!(json.contains("\"gate_decision\":\"DENY\""));
}

/// FR-011 / AC-011.6 — gate_status_snapshot matches effective_gate_decision.
#[test]
fn fr011_gate_status_snapshot_agent_refuse() {
    let snap = gate_status_snapshot(ThermalLevel::Green, 8);
    assert_eq!(snap.thermal_pressure, "GREEN");
    assert_eq!(snap.gate_decision, "DENY");
    assert_eq!(snap.agent_contention, "REFUSE");
}

fn ps_bin() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// FR-011 / AC-011.6 — ps --all prints thermal gate section when scan runs.
#[test]
fn fr011_ps_all_includes_gate_section() {
    let out = ps_bin().args(["ps", "--all"]).output().expect("ps --all");
    assert!(out.status.success(), "ps --all failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("=== Thermal Gate (FR-011) ==="),
        "ps --all MUST surface gate section (AC-011.6); got: {text}"
    );
    assert!(
        text.contains("Gate decision:"),
        "ps --all MUST include gate decision (AC-011.6); got: {text}"
    );
}
