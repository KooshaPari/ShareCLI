// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli ps` AGENT column + `--all` host proc scan
//! FR: FR-006
//!
//! AC-006.7 ps table includes AGENT from proc_scan ancestor walk
//! AC-006.8 ps --all lists host-detected agent processes

use std::process::Command;

use sharecli_core::{agent_label_for_pid, scan_agents, FakeProcSource, ProcSnapshot};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn fixture() -> FakeProcSource {
    FakeProcSource::new(vec![
        ProcSnapshot {
            pid: 100,
            ppid: 1,
            comm: "forge".into(),
            cmdline: vec!["forge".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 200,
            ppid: 100,
            comm: "cargo".into(),
            cmdline: vec!["cargo".into(), "test".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 300,
            ppid: 1,
            comm: "zsh".into(),
            cmdline: vec!["-i".into()],
            state: 'R',
        },
    ])
}

/// FR-006 / AC-006.7 — managed child inherits ancestor agent family label.
#[test]
fn fr006_ps_agent_label_walks_ancestors() {
    let src = fixture();
    assert_eq!(agent_label_for_pid(&src, 200), "forge");
    assert_eq!(agent_label_for_pid(&src, 100), "forge");
    assert_eq!(agent_label_for_pid(&src, 300), "-");
}

/// FR-006 / AC-006.7 — CLI `ps` prints AGENT column header.
#[test]
fn fr006_ps_cli_prints_agent_column() {
    let out = bin().args(["ps"]).output().expect("spawn sharecli ps");
    assert!(out.status.success(), "ps should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("AGENT"), "ps MUST include AGENT column; got: {s}");
    assert!(s.contains("HARNESS"), "ps MUST retain HARNESS column; got: {s}");
}

/// FR-006 / AC-006.8 — `--all` surfaces host proc-scan section (may be empty).
#[test]
fn fr006_ps_all_includes_host_agent_scan_section() {
    let out = bin().args(["ps", "--all"]).output().expect("spawn sharecli ps --all");
    assert!(out.status.success(), "ps --all should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("Host agents (proc scan)"),
        "ps --all MUST print host agent inventory header; got: {s}"
    );
}

/// FR-006 / AC-006.4 regression — scan_agents still lists direct agent PIDs only.
#[test]
fn fr006_scan_agents_lists_agent_comm_only() {
    let agents = scan_agents(&fixture());
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].family, "forge");
    assert_eq!(agents[0].pid, 100);
}
