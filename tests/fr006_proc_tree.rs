// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — Process-tree /proc agent detection
//! FR: FR-006
//!
//! AC-006.4 scan_agents finds known agent PIDs
//! AC-006.5 walk_agent_ancestors finds agent from child tool PID
//! AC-006.6 human shells without agent ancestors are not under-agent

use sharecli_core::{
    is_under_agent, scan_agents, walk_agent_ancestors, FakeProcSource, ProcSnapshot,
};

fn fixture() -> FakeProcSource {
    FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 50,
            ppid: 1,
            comm: "cursor-agent".into(),
            cmdline: vec!["cursor-agent".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 51,
            ppid: 50,
            comm: "node".into(),
            cmdline: vec!["node".into(), "tool.js".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 60,
            ppid: 1,
            comm: "zsh".into(),
            cmdline: vec!["-i".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 61,
            ppid: 60,
            comm: "cargo".into(),
            cmdline: vec!["cargo".into(), "test".into()],
            state: 'R',
        },
    ])
}

/// FR-006 / AC-006.4 — scan lists agent PIDs only.
#[test]
fn fr006_scan_agents_finds_known() {
    let agents = scan_agents(&fixture());
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].family, "cursor-agent");
    assert_eq!(agents[0].pid, 50);
}

/// FR-006 / AC-006.5 — child tool under agent walks to family.
#[test]
fn fr006_walk_ancestors_from_child_tool() {
    let hit = walk_agent_ancestors(&fixture(), 51).expect("node under cursor-agent");
    assert_eq!(hit.family, "cursor-agent");
    assert_eq!(hit.pid, 50);
    assert!(is_under_agent(&fixture(), 51));
}

/// FR-006 / AC-006.6 — human cargo under zsh is not an agent path.
#[test]
fn fr006_human_shell_not_under_agent() {
    assert!(walk_agent_ancestors(&fixture(), 61).is_none());
    assert!(!is_under_agent(&fixture(), 61));
    assert!(!is_under_agent(&fixture(), 60));
}
