// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc` process-state on flat text inventory and pid detail
//! FR: FR-006
//!
//! AC-006.33 expose `state` on flat text table and `proc --pid` detail (text + `--json`)

use std::collections::HashMap;
use std::process::Command;

use sharecli::commands::proc::{build_proc_detail, state_text_for_pid};
use sharecli_fleet::proc_scan::{FakeProcSource, ProcSnapshot};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// FR-006 / AC-006.33 — text table STATE column shows `-` when state is missing.
#[test]
fn fr006_proc_state_text_table_missing_state_shows_dash() {
    assert_eq!(state_text_for_pid(&HashMap::new(), 99), "-");
}

/// FR-006 / AC-006.33 — text table STATE column shows the process state letter.
#[test]
fn fr006_proc_state_text_table_known_state_shows_letter() {
    let mut state_by_pid = HashMap::new();
    state_by_pid.insert(42, 'S');
    assert_eq!(state_text_for_pid(&state_by_pid, 42), "S");
}

/// FR-006 / AC-006.33 — CLI flat inventory header includes STATE column.
#[test]
fn fr006_proc_state_text_cli_table_header_includes_state() {
    let out = bin().args(["proc"]).output().expect("spawn sharecli proc");
    assert!(out.status.success(), "proc MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    // The flat inventory table (and its STATE header) only renders when the
    // host scan detects known agents; agent-less hosts print a placeholder.
    // When a table IS rendered, STATE must be among the headers (AC-006.33).
    if s.contains("FAMILY") && s.contains("PID") {
        assert!(
            s.contains("STATE"),
            "flat text inventory MUST include STATE column header; got: {s}"
        );
    }
}

/// FR-006 / AC-006.33 — build_proc_detail JSON includes state key from proc source.
#[test]
fn fr006_proc_state_text_detail_json_includes_state() {
    let pid = std::process::id();
    let src = FakeProcSource::new(vec![ProcSnapshot {
        pid,
        ppid: 1,
        comm: "claude".into(),
        cmdline: vec!["claude".into()],
        state: 'S',
    }]);
    let detail = build_proc_detail(&src, pid).expect("build detail");
    assert_eq!(detail.state, "S");
    let json = serde_json::to_string(&detail).expect("serialize ProcDetailSnapshot");
    assert!(json.contains("\"state\":\"S\""), "proc detail JSON MUST include state; got: {json}");
}

/// FR-006 / AC-006.33 — unknown proc state serializes as empty JSON string.
#[test]
fn fr006_proc_state_text_detail_json_missing_state_empty_string() {
    let pid = std::process::id();
    let src = FakeProcSource::new(vec![ProcSnapshot {
        pid,
        ppid: 1,
        comm: "codex".into(),
        cmdline: vec!["codex".into()],
        state: '?',
    }]);
    let detail = build_proc_detail(&src, pid).expect("build detail");
    assert_eq!(detail.state, "");
    let json = serde_json::to_string(&detail).expect("serialize ProcDetailSnapshot");
    assert!(
        json.contains("\"state\":\"\""),
        "missing/unknown state MUST be empty JSON string; got: {json}"
    );
}

/// FR-006 / AC-006.33 — CLI proc --pid text detail prints State line.
#[test]
fn fr006_proc_state_text_detail_cli_prints_state_line() {
    let pid = std::process::id();
    let out = bin().args(["proc", "--pid", &pid.to_string()]).output().expect("spawn proc --pid");
    assert!(out.status.success(), "proc --pid self MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("State:"), "text detail MUST print State line; got: {s}");
}

/// FR-006 / AC-006.33 — CLI proc --pid --json includes state key.
#[test]
fn fr006_proc_state_text_detail_cli_json_includes_state_key() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string(), "--json"])
        .output()
        .expect("spawn proc --pid --json");
    assert!(out.status.success(), "proc --pid --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --pid --json MUST emit valid JSON");
    assert!(v.get("state").is_some(), "proc detail JSON MUST include state key; got: {v}");
}
