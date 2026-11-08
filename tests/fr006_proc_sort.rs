// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --sort`
//! FR: FR-006
//!
//! AC-006.19 `--sort rss|fd|pid` orders inventory rows and tree root forests
//! AC-006.36 `--sort state` orders by process state letter with PID tie-break

use std::collections::HashMap;
use std::process::Command;

use sharecli::commands::proc::{sort_agent_forests, sort_watched_agents, ProcSort};
use sharecli_fleet::{
    proc_scan::{AgentTreeNode, DetectedAgent},
    AgentResourceSample, DetectedAgentWatch,
};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn watch(pid: u32, family: &'static str, rss: u64, fds: Option<u64>) -> DetectedAgentWatch {
    DetectedAgentWatch {
        agent: DetectedAgent { pid, family, comm: family.into() },
        resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: fds },
    }
}

fn tree_root(pid: u32, family: Option<&'static str>) -> AgentTreeNode {
    AgentTreeNode { pid, ppid: 1, comm: "agent".into(), family, children: vec![] }
}

/// FR-006 / AC-006.19 — proc help documents --sort.
#[test]
fn fr006_proc_help_documents_sort() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success(), "proc --help should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--sort"), "proc --help MUST document --sort; got: {s}");
}

/// FR-006 / AC-006.19 — rss sort is descending with pid tie-break.
#[test]
fn fr006_proc_sort_rss_descending() {
    let rows = vec![
        watch(10, "claude", 100, Some(5)),
        watch(20, "codex", 300, Some(1)),
        watch(30, "aider", 200, Some(2)),
    ];
    let sorted = sort_watched_agents(&rows, ProcSort::Rss, &HashMap::new());
    assert_eq!(sorted.iter().map(|r| r.agent.pid).collect::<Vec<_>>(), vec![20, 30, 10]);
}

/// FR-006 / AC-006.19 — fd sort treats missing FD as zero, descending.
#[test]
fn fr006_proc_sort_fd_descending() {
    let rows = vec![
        watch(1, "claude", 100, None),
        watch(2, "codex", 100, Some(50)),
        watch(3, "aider", 100, Some(10)),
    ];
    let sorted = sort_watched_agents(&rows, ProcSort::Fd, &HashMap::new());
    assert_eq!(sorted.iter().map(|r| r.agent.pid).collect::<Vec<_>>(), vec![2, 3, 1]);
}

/// FR-006 / AC-006.19 — pid sort is ascending.
#[test]
fn fr006_proc_sort_pid_ascending() {
    let rows = vec![watch(99, "claude", 1, None), watch(1, "codex", 1, None)];
    let sorted = sort_watched_agents(&rows, ProcSort::Pid, &HashMap::new());
    assert_eq!(sorted.iter().map(|r| r.agent.pid).collect::<Vec<_>>(), vec![1, 99]);
}

/// FR-006 / AC-006.19 — tree root forests honor rss sort via live samples.
#[test]
fn fr006_proc_sort_tree_roots_by_rss() {
    let forests = vec![tree_root(10, Some("claude")), tree_root(20, Some("codex"))];
    let mut rss = HashMap::new();
    rss.insert(10, 500_u64);
    rss.insert(20, 100_u64);
    let sorted =
        sort_agent_forests(&forests, ProcSort::Rss, &rss, &HashMap::new(), &HashMap::new());
    assert_eq!(sorted.iter().map(|n| n.pid).collect::<Vec<_>>(), vec![10, 20]);
}

/// FR-006 / AC-006.36 — state sort is ascending by letter with pid tie-break.
#[test]
fn fr006_proc_sort_state_ascending() {
    let rows = vec![
        watch(10, "claude", 100, None),
        watch(20, "codex", 100, None),
        watch(30, "aider", 100, None),
    ];
    let mut state = HashMap::new();
    state.insert(10, 'S');
    state.insert(20, 'R');
    state.insert(30, 'D');
    let sorted = sort_watched_agents(&rows, ProcSort::State, &state);
    assert_eq!(sorted.iter().map(|r| r.agent.pid).collect::<Vec<_>>(), vec![30, 20, 10]);
}

/// FR-006 / AC-006.36 — state sort tie-breaks equal letters by ascending pid.
#[test]
fn fr006_proc_sort_state_pid_tiebreak() {
    let rows = vec![watch(99, "claude", 1, None), watch(1, "codex", 1, None)];
    let mut state = HashMap::new();
    state.insert(99, 'R');
    state.insert(1, 'R');
    let sorted = sort_watched_agents(&rows, ProcSort::State, &state);
    assert_eq!(sorted.iter().map(|r| r.agent.pid).collect::<Vec<_>>(), vec![1, 99]);
}

/// FR-006 / AC-006.36 — missing state sorts after known letters.
#[test]
fn fr006_proc_sort_state_missing_sorts_last() {
    let rows = vec![watch(10, "claude", 1, None), watch(20, "codex", 1, None)];
    let mut state = HashMap::new();
    state.insert(10, 'D');
    let sorted = sort_watched_agents(&rows, ProcSort::State, &state);
    assert_eq!(sorted.iter().map(|r| r.agent.pid).collect::<Vec<_>>(), vec![10, 20]);
}

/// FR-006 / AC-006.36 — tree root forests honor state sort via live lookup.
#[test]
fn fr006_proc_sort_tree_roots_by_state() {
    let forests = vec![tree_root(10, Some("claude")), tree_root(20, Some("codex"))];
    let mut state = HashMap::new();
    state.insert(10, 'S');
    state.insert(20, 'R');
    let sorted =
        sort_agent_forests(&forests, ProcSort::State, &HashMap::new(), &HashMap::new(), &state);
    assert_eq!(sorted.iter().map(|n| n.pid).collect::<Vec<_>>(), vec![20, 10]);
}

/// FR-006 / AC-006.36 — proc help documents state sort key.
#[test]
fn fr006_proc_help_documents_sort_state() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success(), "proc --help should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("state"), "proc --help MUST document state sort key; got: {s}");
}

/// FR-006 / AC-006.19 — invalid sort key exits non-zero.
#[test]
fn fr006_proc_sort_invalid_exits_nonzero() {
    let out =
        bin().args(["proc", "--sort", "bogus"]).output().expect("spawn sharecli proc --sort bogus");
    assert!(!out.status.success(), "invalid --sort MUST fail; stdout: {:?}", out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("rss") || err.contains("sort"),
        "error MUST mention valid keys; got: {err}"
    );
}
