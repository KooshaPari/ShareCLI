//! Live process-tree agent detection (FR-006).
//!
//! Walks parent chains (`/proc` on Linux; `sysinfo` elsewhere) and matches
//! [`crate::detect::match_known_agent`] — observation only, no vendor-bin wrap.

use std::collections::HashMap;

use crate::detect::{match_known_agent, match_system_tool};

/// One process row used for tree walks and scans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcSnapshot {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    pub cmdline: Vec<String>,
    /// Linux `/proc` state letter (R|S|D|Z|T|t|…); AC-006.31.
    pub state: char,
}

/// Map sysinfo process status to a Linux-style state letter.
pub fn sysinfo_status_to_char(status: sysinfo::ProcessStatus) -> char {
    use sysinfo::ProcessStatus;
    match status {
        ProcessStatus::Run => 'R',
        ProcessStatus::Sleep => 'S',
        ProcessStatus::UninterruptibleDiskSleep => 'D',
        ProcessStatus::Stop => 'T',
        ProcessStatus::Zombie => 'Z',
        ProcessStatus::Tracing => 't',
        ProcessStatus::Dead => 'X',
        ProcessStatus::Wakekill => 'K',
        ProcessStatus::Waking => 'W',
        ProcessStatus::Parked => 'P',
        ProcessStatus::Idle => 'I',
        ProcessStatus::LockBlocked | ProcessStatus::Suspended => '?',
        ProcessStatus::Unknown(_) => '?',
    }
}

/// A detected agent instance on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedAgent {
    pub pid: u32,
    pub family: &'static str,
    pub comm: String,
}

/// One node in an agent-rooted process subtree (AC-006.16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTreeNode {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    pub family: Option<&'static str>,
    pub children: Vec<AgentTreeNode>,
}

/// Abstract process table — production backends + in-memory fixtures for tests.
pub trait ProcSource {
    fn list(&self) -> Vec<ProcSnapshot>;
}

/// In-memory process table for unit tests (FR fixtures).
#[derive(Debug, Default, Clone)]
pub struct FakeProcSource {
    procs: Vec<ProcSnapshot>,
}

impl FakeProcSource {
    pub fn new(procs: Vec<ProcSnapshot>) -> Self {
        Self { procs }
    }
}

impl ProcSource for FakeProcSource {
    fn list(&self) -> Vec<ProcSnapshot> {
        self.procs.clone()
    }
}

/// Scan every process; return those whose `comm`/cmdline match known agents.
pub fn scan_agents(source: &dyn ProcSource) -> Vec<DetectedAgent> {
    let mut out = Vec::new();
    for p in source.list() {
        if let Some(family) = match_known_agent(&p.comm, &p.cmdline) {
            out.push(DetectedAgent { pid: p.pid, family, comm: p.comm });
        }
    }
    out.sort_by_key(|a| a.pid);
    out
}

/// Walk `pid` and its ancestors via `ppid` until a known agent is found.
pub fn walk_agent_ancestors(source: &dyn ProcSource, pid: u32) -> Option<DetectedAgent> {
    let by_pid: HashMap<u32, ProcSnapshot> =
        source.list().into_iter().map(|p| (p.pid, p)).collect();
    let mut seen = std::collections::HashSet::new();
    let mut cur = pid;
    while seen.insert(cur) {
        let Some(proc) = by_pid.get(&cur) else {
            break;
        };
        if let Some(family) = match_known_agent(&proc.comm, &proc.cmdline) {
            return Some(DetectedAgent { pid: proc.pid, family, comm: proc.comm.clone() });
        }
        if proc.ppid == 0 || proc.ppid == proc.pid {
            break;
        }
        cur = proc.ppid;
    }
    None
}

/// True when `pid` itself or any ancestor matches a known agent family.
pub fn is_under_agent(source: &dyn ProcSource, pid: u32) -> bool {
    walk_agent_ancestors(source, pid).is_some()
}

/// Table cell for the `sharecli ps` AGENT column: nearest ancestor family or `"-"`.
pub fn agent_label_for_pid(source: &dyn ProcSource, pid: u32) -> &'static str {
    walk_agent_ancestors(source, pid).map(|a| a.family).unwrap_or("-")
}

/// One system-tool instance detected on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemToolScan {
    pub pid: u32,
    pub tool: &'static str,
    pub comm: String,
}

/// Scan every process; return those whose `comm`/cmdline match system-tool patterns.
pub fn scan_system_tools(source: &dyn ProcSource) -> Vec<SystemToolScan> {
    let mut out = Vec::new();
    for p in source.list() {
        if let Some(tool) = match_system_tool(&p.comm, &p.cmdline) {
            out.push(SystemToolScan { pid: p.pid, tool, comm: p.comm });
        }
    }
    out.sort_by_key(|a| a.pid);
    out
}

/// Live-host convenience for [`scan_system_tools`].
pub fn scan_host_system_tools() -> Vec<SystemToolScan> {
    scan_system_tools(&HostProcSource)
}

/// Live host process source.
#[derive(Debug, Default, Clone, Copy)]
pub struct HostProcSource;

impl ProcSource for HostProcSource {
    fn list(&self) -> Vec<ProcSnapshot> {
        #[cfg(target_os = "linux")]
        {
            list_linux_proc()
        }
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            list_sysinfo_proc()
        }
        #[cfg(not(unix))]
        {
            list_sysinfo_proc()
        }
    }
}

/// Convenience: scan the live host for known agents.
pub fn scan_host_agents() -> Vec<DetectedAgent> {
    scan_agents(&HostProcSource)
}

/// Lookup one live process row by PID (AC-006.23).
pub fn lookup_proc(source: &dyn ProcSource, pid: u32) -> Option<ProcSnapshot> {
    source.list().into_iter().find(|p| p.pid == pid)
}

/// Lookup one process on the live host by PID.
pub fn lookup_host_proc(pid: u32) -> Option<ProcSnapshot> {
    lookup_proc(&HostProcSource, pid)
}

/// Build parent-child forests rooted at top-level detected agents (AC-006.16).
///
/// Each forest includes the agent root and descendant processes until a nested
/// agent boundary (nested agents appear as child subtrees). Human-only shells
/// outside any agent ancestry are omitted.
pub fn build_agent_forests(source: &dyn ProcSource) -> Vec<AgentTreeNode> {
    let procs = source.list();
    let by_pid: HashMap<u32, ProcSnapshot> = procs.iter().cloned().map(|p| (p.pid, p)).collect();
    let agents = scan_agents(source);
    if agents.is_empty() {
        return Vec::new();
    }
    let agent_pids: std::collections::HashSet<u32> = agents.iter().map(|a| a.pid).collect();
    let mut roots: Vec<u32> = agents
        .iter()
        .filter(|a| {
            let ppid = by_pid.get(&a.pid).map(|p| p.ppid).unwrap_or(0);
            !agent_pids.contains(&ppid)
        })
        .map(|a| a.pid)
        .collect();
    roots.sort_unstable();
    roots
        .into_iter()
        .filter_map(|root_pid| build_agent_subtree(root_pid, &by_pid, &agent_pids))
        .collect()
}

/// Live-host convenience for [`build_agent_forests`].
pub fn build_host_agent_forests() -> Vec<AgentTreeNode> {
    build_agent_forests(&HostProcSource)
}

/// Collect every PID in agent forests, roots and nested children (AC-006.35).
pub fn collect_forest_pids(forests: &[AgentTreeNode]) -> Vec<u32> {
    let mut pids = Vec::new();
    collect_forest_pids_inner(forests, &mut pids);
    pids
}

fn collect_forest_pids_inner(nodes: &[AgentTreeNode], out: &mut Vec<u32>) {
    for node in nodes {
        out.push(node.pid);
        collect_forest_pids_inner(&node.children, out);
    }
}

/// Map agent PID → process state letter from a proc source (AC-006.31).
pub fn build_agent_state_map(source: &dyn ProcSource, agent_pids: &[u32]) -> HashMap<u32, char> {
    agent_pids
        .iter()
        .filter_map(|&pid| lookup_proc(source, pid).map(|proc| (pid, proc.state)))
        .collect()
}

/// Live process-state map for all nodes in displayed forests (AC-006.35).
pub fn build_forest_state_map(
    source: &dyn ProcSource,
    forests: &[AgentTreeNode],
) -> HashMap<u32, char> {
    build_agent_state_map(source, &collect_forest_pids(forests))
}

/// Live-host convenience for [`build_forest_state_map`].
pub fn build_host_forest_state_map(forests: &[AgentTreeNode]) -> HashMap<u32, char> {
    build_forest_state_map(&HostProcSource, forests)
}

/// Live-host convenience for [`build_agent_state_map`] (flat agent inventory, AC-006.40).
pub fn build_host_agent_state_map(agent_pids: &[u32]) -> HashMap<u32, char> {
    build_agent_state_map(&HostProcSource, agent_pids)
}

/// STATE display for text inventory / tree nodes; missing/unknown → `-` (AC-006.33, AC-006.34).
pub fn state_text_for_pid(state_by_pid: &HashMap<u32, char>, pid: u32) -> String {
    state_by_pid
        .get(&pid)
        .filter(|ch| **ch != '?')
        .map(|ch| ch.to_string())
        .unwrap_or_else(|| "-".into())
}

fn build_agent_subtree(
    pid: u32,
    by_pid: &HashMap<u32, ProcSnapshot>,
    agent_pids: &std::collections::HashSet<u32>,
) -> Option<AgentTreeNode> {
    let proc = by_pid.get(&pid)?;
    let family =
        if agent_pids.contains(&pid) { match_known_agent(&proc.comm, &proc.cmdline) } else { None };
    let mut child_pids: Vec<u32> =
        by_pid.values().filter(|p| p.ppid == pid).map(|p| p.pid).collect();
    child_pids.sort_unstable();
    let children = child_pids
        .into_iter()
        .filter_map(|child| build_agent_subtree(child, by_pid, agent_pids))
        .collect();
    Some(AgentTreeNode { pid, ppid: proc.ppid, comm: proc.comm.clone(), family, children })
}

/// Nearest known-agent ancestor for the current process (hypervisor spawn context).
pub fn detect_caller_agent() -> Option<DetectedAgent> {
    walk_agent_ancestors(&HostProcSource, std::process::id())
}

#[cfg(target_os = "linux")]
fn list_linux_proc() -> Vec<ProcSnapshot> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in entries.flatten() {
        let name = ent.file_name();
        let Some(pid_str) = name.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let base = ent.path();
        let comm =
            std::fs::read_to_string(base.join("comm")).unwrap_or_default().trim().to_string();
        let cmdline_raw = std::fs::read(base.join("cmdline")).unwrap_or_default();
        let cmdline = parse_cmdline(&cmdline_raw);
        let ppid = read_ppid(&base.join("status")).unwrap_or(0);
        let state = read_state_from_stat(&base.join("stat")).unwrap_or('?');
        out.push(ProcSnapshot { pid, ppid, comm, cmdline, state });
    }
    out
}

#[cfg(target_os = "linux")]
fn parse_cmdline(raw: &[u8]) -> Vec<String> {
    raw.split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

#[cfg(target_os = "linux")]
fn read_state_from_stat(stat: &std::path::Path) -> Option<char> {
    let content = std::fs::read_to_string(stat).ok()?;
    let rparen = content.rfind(')')?;
    content[rparen + 1..].trim_start().chars().next()
}

#[cfg(target_os = "linux")]
fn read_ppid(status: &std::path::Path) -> Option<u32> {
    let text = std::fs::read_to_string(status).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

#[cfg(any(all(unix, not(target_os = "linux")), not(unix)))]
fn list_sysinfo_proc() -> Vec<ProcSnapshot> {
    use sysinfo::{ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut out = Vec::new();
    for (pid, proc) in sys.processes() {
        let pid_u = pid.as_u32();
        let ppid = proc.parent().map(|p| p.as_u32()).unwrap_or(0);
        let comm = proc.name().to_string_lossy().into_owned();
        let cmdline: Vec<String> =
            proc.cmd().iter().map(|c| c.to_string_lossy().into_owned()).collect();
        let state = sysinfo_status_to_char(proc.status());
        out.push(ProcSnapshot { pid: pid_u, ppid, comm, cmdline, state });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> FakeProcSource {
        FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot { pid: 10, ppid: 1, comm: "launchd".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot {
                pid: 100,
                ppid: 10,
                comm: "claude".into(),
                cmdline: vec!["/usr/local/bin/claude".into()],
                state: 'R',
            },
            ProcSnapshot {
                pid: 200,
                ppid: 100,
                comm: "bash".into(),
                cmdline: vec!["bash".into()],
                state: 'R',
            },
            ProcSnapshot {
                pid: 300,
                ppid: 200,
                comm: "ruff".into(),
                cmdline: vec!["ruff".into(), "check".into(), ".".into()],
                state: 'R',
            },
            ProcSnapshot {
                pid: 400,
                ppid: 10,
                comm: "zsh".into(),
                cmdline: vec!["-l".into()],
                state: 'R',
            },
        ])
    }

    #[test]
    fn scan_finds_claude_only() {
        let agents = scan_agents(&tree());
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].family, "claude");
        assert_eq!(agents[0].pid, 100);
    }

    #[test]
    fn lookup_proc_returns_row_or_none() {
        let src = tree();
        let hit = lookup_proc(&src, 300).expect("ruff child");
        assert_eq!(hit.comm, "ruff");
        assert_eq!(hit.ppid, 200);
        assert!(lookup_proc(&src, 999).is_none());
    }

    #[test]
    fn walk_from_ruff_finds_claude_ancestor() {
        let hit = walk_agent_ancestors(&tree(), 300).expect("under claude");
        assert_eq!(hit.family, "claude");
        assert_eq!(hit.pid, 100);
        assert!(is_under_agent(&tree(), 300));
        assert!(is_under_agent(&tree(), 200));
        assert!(!is_under_agent(&tree(), 400), "human zsh MUST not be agent path");
    }

    #[test]
    fn walk_self_agent() {
        let hit = walk_agent_ancestors(&tree(), 100).expect("self");
        assert_eq!(hit.pid, 100);
    }

    #[test]
    fn agent_label_for_child_and_human() {
        assert_eq!(agent_label_for_pid(&tree(), 300), "claude");
        assert_eq!(agent_label_for_pid(&tree(), 400), "-");
    }

    #[test]
    fn build_forests_agent_child_tools_only() {
        let forests = build_agent_forests(&tree());
        assert_eq!(forests.len(), 1);
        assert_eq!(forests[0].pid, 100);
        assert_eq!(forests[0].family, Some("claude"));
        assert_eq!(forests[0].children.len(), 1);
        assert_eq!(forests[0].children[0].pid, 200);
        assert_eq!(forests[0].children[0].children.len(), 1);
        assert_eq!(forests[0].children[0].children[0].pid, 300);
    }

    #[test]
    fn build_forests_excludes_human_shell_subtrees() {
        let forests = build_agent_forests(&tree());
        let all_pids: Vec<u32> = flatten_pids(&forests);
        assert!(!all_pids.contains(&400));
        assert!(!all_pids.contains(&60));
    }

    fn flatten_pids(nodes: &[AgentTreeNode]) -> Vec<u32> {
        let mut out = Vec::new();
        for n in nodes {
            out.push(n.pid);
            out.extend(flatten_pids(&n.children));
        }
        out
    }

    #[test]
    fn build_forests_nested_agent_under_parent() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot {
                pid: 10,
                ppid: 1,
                comm: "claude".into(),
                cmdline: vec!["claude".into()],
                state: 'R',
            },
            ProcSnapshot {
                pid: 20,
                ppid: 10,
                comm: "forge".into(),
                cmdline: vec!["forge".into(), "conversation".into(), "list".into()],
                state: 'R',
            },
            ProcSnapshot {
                pid: 21,
                ppid: 20,
                comm: "bash".into(),
                cmdline: vec!["bash".into()],
                state: 'R',
            },
        ]);
        let forests = build_agent_forests(&src);
        assert_eq!(forests.len(), 1);
        assert_eq!(forests[0].pid, 10);
        assert_eq!(forests[0].children.len(), 1);
        assert_eq!(forests[0].children[0].pid, 20);
        assert_eq!(forests[0].children[0].family, Some("forge"));
        assert_eq!(forests[0].children[0].children.len(), 1);
        assert_eq!(forests[0].children[0].children[0].pid, 21);
    }

    #[test]
    fn collect_forest_pids_includes_nested_children() {
        let forests = build_agent_forests(&tree());
        let mut pids = collect_forest_pids(&forests);
        pids.sort_unstable();
        assert_eq!(pids, vec![100, 200, 300]);
    }

    #[test]
    fn build_forest_state_map_resolves_child_states() {
        let forests = build_agent_forests(&tree());
        let map = build_forest_state_map(&tree(), &forests);
        assert_eq!(map.get(&100), Some(&'R'));
        assert_eq!(map.get(&300), Some(&'R'));
        assert_eq!(state_text_for_pid(&map, 300), "R");
        assert_eq!(state_text_for_pid(&HashMap::new(), 300), "-");
    }

    // --- Additional edge-case tests ---

    #[test]
    fn sysinfo_status_to_char_all_variants() {
        use sysinfo::ProcessStatus;
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Run), 'R');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Sleep), 'S');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::UninterruptibleDiskSleep), 'D');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Stop), 'T');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Zombie), 'Z');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Tracing), 't');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Dead), 'X');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Wakekill), 'K');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Waking), 'W');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Parked), 'P');
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Idle), 'I');
    }

    #[test]
    fn sysinfo_status_to_char_unknown() {
        use sysinfo::ProcessStatus;
        assert_eq!(sysinfo_status_to_char(ProcessStatus::Unknown(99)), '?');
    }

    #[test]
    fn state_text_for_pid_unknown_state() {
        let mut map = HashMap::new();
        map.insert(100, '?');
        assert_eq!(state_text_for_pid(&map, 100), "-", "state '?' MUST display as '-'");
    }

    #[test]
    fn state_text_for_pid_valid_states() {
        let mut map = HashMap::new();
        map.insert(1, 'R');
        map.insert(2, 'S');
        map.insert(3, 'D');
        map.insert(4, 'Z');
        assert_eq!(state_text_for_pid(&map, 1), "R");
        assert_eq!(state_text_for_pid(&map, 2), "S");
        assert_eq!(state_text_for_pid(&map, 3), "D");
        assert_eq!(state_text_for_pid(&map, 4), "Z");
    }

    #[test]
    fn state_text_for_pid_missing_pid() {
        let map = HashMap::new();
        assert_eq!(state_text_for_pid(&map, 999), "-");
    }

    #[test]
    fn scan_agents_empty_source() {
        let src = FakeProcSource::new(vec![]);
        assert!(scan_agents(&src).is_empty());
    }

    #[test]
    fn scan_agents_multiple_known_agents() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot {
                pid: 10,
                ppid: 1,
                comm: "claude".into(),
                cmdline: vec!["claude".into()],
                state: 'S',
            },
            ProcSnapshot {
                pid: 20,
                ppid: 1,
                comm: "forge".into(),
                cmdline: vec!["forge".into(), "conversation".into(), "list".into()],
                state: 'R',
            },
            ProcSnapshot { pid: 30, ppid: 1, comm: "unknown".into(), cmdline: vec![], state: 'R' },
        ]);
        let agents = scan_agents(&src);
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().any(|a| a.family == "claude"));
        assert!(agents.iter().any(|a| a.family == "forge"));
    }

    #[test]
    fn walk_agent_ancestors_no_match() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot { pid: 2, ppid: 1, comm: "bash".into(), cmdline: vec![], state: 'R' },
        ]);
        assert!(walk_agent_ancestors(&src, 2).is_none());
    }

    #[test]
    fn walk_agent_ancestors_cycle_prevention() {
        // Create a cycle: pid 1 -> ppid 2, pid 2 -> ppid 1
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 2, comm: "a".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot { pid: 2, ppid: 1, comm: "b".into(), cmdline: vec![], state: 'R' },
        ]);
        // Should not hang — cycle detection prevents infinite loop
        assert!(walk_agent_ancestors(&src, 1).is_none());
    }

    #[test]
    fn is_under_agent_direct_match() {
        let src = FakeProcSource::new(vec![ProcSnapshot {
            pid: 100,
            ppid: 0,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
            state: 'R',
        }]);
        assert!(is_under_agent(&src, 100));
    }

    #[test]
    fn is_under_agent_no_match() {
        let src = FakeProcSource::new(vec![ProcSnapshot {
            pid: 100,
            ppid: 0,
            comm: "bash".into(),
            cmdline: vec![],
            state: 'R',
        }]);
        assert!(!is_under_agent(&src, 100));
    }

    #[test]
    fn agent_label_for_pid_direct_agent() {
        let src = FakeProcSource::new(vec![ProcSnapshot {
            pid: 100,
            ppid: 0,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
            state: 'R',
        }]);
        assert_eq!(agent_label_for_pid(&src, 100), "claude");
    }

    #[test]
    fn agent_label_for_pid_non_agent() {
        let src = FakeProcSource::new(vec![ProcSnapshot {
            pid: 100,
            ppid: 0,
            comm: "bash".into(),
            cmdline: vec![],
            state: 'R',
        }]);
        assert_eq!(agent_label_for_pid(&src, 100), "-");
    }

    #[test]
    fn build_agent_state_map_empty() {
        let src = FakeProcSource::new(vec![]);
        let map = build_agent_state_map(&src, &[1, 2, 3]);
        assert!(map.is_empty());
    }

    #[test]
    fn build_agent_state_map_partial() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 100, ppid: 0, comm: "a".into(), cmdline: vec![], state: 'S' },
            ProcSnapshot { pid: 200, ppid: 0, comm: "b".into(), cmdline: vec![], state: 'R' },
        ]);
        let map = build_agent_state_map(&src, &[100, 200, 999]);
        assert_eq!(map.get(&100), Some(&'S'));
        assert_eq!(map.get(&200), Some(&'R'));
        assert!(!map.contains_key(&999), "missing PID MUST not be in map");
    }

    #[test]
    fn collect_forest_pids_empty() {
        assert!(collect_forest_pids(&[]).is_empty());
    }

    #[test]
    fn collect_forest_pids_deep_nesting() {
        let forest = AgentTreeNode {
            pid: 1,
            ppid: 0,
            comm: "root".into(),
            family: Some("claude"),
            children: vec![AgentTreeNode {
                pid: 2,
                ppid: 1,
                comm: "child".into(),
                family: None,
                children: vec![AgentTreeNode {
                    pid: 3,
                    ppid: 2,
                    comm: "grandchild".into(),
                    family: None,
                    children: vec![],
                }],
            }],
        };
        let mut pids = collect_forest_pids(&[forest]);
        pids.sort();
        assert_eq!(pids, vec![1, 2, 3]);
    }

    #[test]
    fn build_agent_forests_empty_source() {
        let src = FakeProcSource::new(vec![]);
        assert!(build_agent_forests(&src).is_empty());
    }

    #[test]
    fn build_agent_forests_only_non_agents() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "bash".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot { pid: 2, ppid: 1, comm: "vim".into(), cmdline: vec![], state: 'R' },
        ]);
        assert!(build_agent_forests(&src).is_empty());
    }

    #[test]
    fn build_forest_state_map_empty_forests() {
        let src = FakeProcSource::new(vec![]);
        let map = build_forest_state_map(&src, &[]);
        assert!(map.is_empty());
    }

    #[test]
    fn lookup_proc_nonexistent_pid() {
        let src = FakeProcSource::new(vec![ProcSnapshot {
            pid: 1,
            ppid: 0,
            comm: "a".into(),
            cmdline: vec![],
            state: 'R',
        }]);
        assert!(lookup_proc(&src, 999).is_none());
    }

    #[test]
    fn proc_snapshot_clone_and_eq() {
        let p = ProcSnapshot {
            pid: 1,
            ppid: 0,
            comm: "test".into(),
            cmdline: vec!["a".into()],
            state: 'R',
        };
        let p2 = p.clone();
        assert_eq!(p, p2);
    }

    #[test]
    fn detected_agent_clone_and_eq() {
        let a = DetectedAgent { pid: 1, family: "claude", comm: "claude".into() };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn agent_tree_node_clone_and_eq() {
        let node = AgentTreeNode {
            pid: 1,
            ppid: 0,
            comm: "root".into(),
            family: Some("claude"),
            children: vec![],
        };
        let cloned = node.clone();
        assert_eq!(node, cloned);
    }
}
