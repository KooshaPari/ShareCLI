//! Agent inventory + process-forest panel rendering (FR-006 × AC-006.22 / AC-006.39).
//!
//! Pure line builders; the [`crate::render::render_agents`] function consumes them.

use std::collections::HashMap;

use ratatui::{
    style::{Color, Style},
    text::Line,
};

use sharecli_fleet::{state_text_for_pid, AgentTreeNode, DetectedAgentWatch};

use sharecli_fleet::format_rss_bytes;

/// Max agent rows rendered in the full-layout DetectedAgent panel.
pub const MAX_AGENT_LINES: usize = 4;

/// Max tree lines rendered in the full-layout DetectedAgent panel (AC-006.22).
pub const MAX_AGENT_TREE_LINES: usize = 4;

/// Lines for the host agent inventory panel (FR-006 / AC-006.9, AC-006.40 TUI slice).
pub fn agent_lines(
    agents: &[DetectedAgentWatch],
    state_by_pid: &HashMap<u32, char>,
    compact: bool,
) -> Vec<Line<'static>> {
    if agents.is_empty() {
        let msg = if compact { " none" } else { "  No agent processes detected" };
        return vec![Line::from(ratatui::text::Span::styled(
            msg,
            Style::default().fg(Color::DarkGray),
        ))];
    }

    if compact {
        let summary: String = agents
            .iter()
            .take(2)
            .map(|row| {
                let state = state_text_for_pid(state_by_pid, row.agent.pid);
                format!(
                    "{}:{}:{}@{}",
                    row.agent.family,
                    row.agent.pid,
                    state,
                    format_rss_bytes(row.resource.mem_rss_bytes)
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let extra =
            if agents.len() > 2 { format!(" +{}", agents.len() - 2) } else { String::new() };
        return vec![Line::from(format!(" {summary}{extra}"))];
    }

    let mut lines = vec![Line::from(format!("  Agents: {}", agents.len()))];
    for row in agents.iter().take(MAX_AGENT_LINES) {
        let state = state_text_for_pid(state_by_pid, row.agent.pid);
        let fd = row.resource.fd_count.map(|n| format!(" FD {n}")).unwrap_or_default();
        lines.push(Line::from(format!(
            "    PID {}  {}  {}  RSS {}{}  ({})",
            row.agent.pid,
            state,
            row.agent.family,
            format_rss_bytes(row.resource.mem_rss_bytes),
            fd,
            row.agent.comm
        )));
    }
    if agents.len() > MAX_AGENT_LINES {
        lines.push(Line::from(format!("    … +{} more", agents.len() - MAX_AGENT_LINES)));
    }
    lines
}

/// Format a single tree-node description: `[pid] state family rss (comm)`.
pub fn format_tree_node_line(
    node: &AgentTreeNode,
    rss_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
) -> String {
    let family = node.family.map(|f| format!("{f} ")).unwrap_or_default();
    let state = state_text_for_pid(state_by_pid, node.pid);
    let rss = rss_by_pid
        .get(&node.pid)
        .map(|bytes| format!(" RSS {}", format_rss_bytes(*bytes)))
        .unwrap_or_default();
    format!("[{}] {state} {family}{}{}", node.pid, rss, format_comm_suffix(&node.comm))
}

/// `(comm)` suffix, omitted when the comm is empty.
pub fn format_comm_suffix(comm: &str) -> String {
    if comm.is_empty() {
        String::new()
    } else {
        format!(" ({comm})")
    }
}

/// Recursively walk an [`AgentTreeNode`] emitting tree-printer lines into `lines`.
///
/// `budget` is decremented on each emitted line; recursion halts when it reaches zero.
pub fn append_agent_tree_lines(
    node: &AgentTreeNode,
    prefix: &str,
    is_last: bool,
    rss_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
    lines: &mut Vec<Line<'static>>,
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    let connector =
        if prefix.is_empty() { String::new() } else if is_last { "└── ".to_string() } else { "├── ".to_string() };
    let lead = if prefix.is_empty() { "  " } else { prefix };
    lines.push(Line::from(format!(
        "{lead}{connector}{}",
        format_tree_node_line(node, rss_by_pid, state_by_pid)
    )));
    *budget -= 1;

    let child_prefix = if prefix.is_empty() {
        "    ".to_string()
    } else {
        format!("{}{}", prefix, if is_last { "    " } else { "│   " })
    };
    for (i, child) in node.children.iter().enumerate() {
        append_agent_tree_lines(
            child,
            &child_prefix,
            i + 1 == node.children.len(),
            rss_by_pid,
            state_by_pid,
            lines,
            budget,
        );
        if *budget == 0 {
            break;
        }
    }
}

/// Lines for the host agent inventory panel — tree when forests are present (AC-006.22, AC-006.39).
pub fn agent_forest_lines(
    forests: &[AgentTreeNode],
    watched: &[DetectedAgentWatch],
    state_by_pid: &HashMap<u32, char>,
    compact: bool,
) -> Vec<Line<'static>> {
    if compact || forests.is_empty() {
        return agent_lines(watched, state_by_pid, compact);
    }
    let rss_by_pid: HashMap<u32, u64> =
        watched.iter().map(|row| (row.agent.pid, row.resource.mem_rss_bytes)).collect();
    let mut lines = vec![Line::from(format!("  Forests: {}", forests.len()))];
    let mut budget = MAX_AGENT_TREE_LINES;
    for (i, root) in forests.iter().enumerate() {
        if budget == 0 {
            if i < forests.len() {
                lines.push(Line::from(format!("    … +{} more roots", forests.len() - i)));
            }
            break;
        }
        append_agent_tree_lines(root, "", true, &rss_by_pid, state_by_pid, &mut lines, &mut budget);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use sharecli_fleet::{AgentResourceSample, DetectedAgent};

    fn agent_watch(
        pid: u32,
        family: &'static str,
        comm: &str,
        rss: u64,
        fds: Option<u64>,
    ) -> DetectedAgentWatch {
        DetectedAgentWatch {
            agent: DetectedAgent { pid, family, comm: comm.into() },
            resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: fds },
        }
    }

    fn tree_node(pid: u32, family: Option<&'static str>, comm: &str) -> AgentTreeNode {
        AgentTreeNode { pid, ppid: 0, comm: comm.into(), family: family.map(|s| s.into()), children: vec![] }
    }

    #[test]
    fn test_agent_lines_empty_compact() {
        let lines = agent_lines(&[], &HashMap::new(), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("none"));
    }

    #[test]
    fn test_agent_lines_lists_detected_agents() {
        let agents = vec![
            agent_watch(100, "claude", "claude", 52_428_800, Some(42)),
            agent_watch(200, "cursor", "cursor-agent", 104_857_600, None),
        ];
        let mut state = HashMap::new();
        state.insert(100, 'S');
        state.insert(200, 'R');
        let lines = agent_lines(&agents, &state, false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("Agents: 2"));
        assert!(rendered.contains("PID 100  S  claude"));
        assert!(rendered.contains("RSS 50M"));
        assert!(rendered.contains("FD 42"));
        assert!(rendered.contains("PID 200  R  cursor"));
    }

    #[test]
    fn test_agent_lines_truncates_overflow() {
        let agents: Vec<DetectedAgentWatch> = (0..6)
            .map(|i| agent_watch(100 + i, "claude", &format!("claude-{i}"), 1_048_576, None))
            .collect();
        let lines = agent_lines(&agents, &HashMap::new(), false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("+2 more"));
    }

    #[test]
    fn test_agent_lines_compact_summary() {
        let agents = vec![agent_watch(100, "claude", "claude", 52_428_800, None)];
        let mut state = HashMap::new();
        state.insert(100, 'S');
        let lines = agent_lines(&agents, &state, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("claude:100:S@"));
    }

    #[test]
    fn test_format_tree_node_line_with_rss_and_state() {
        let mut rss = HashMap::new();
        rss.insert(100, 52_428_800u64);
        let mut state = HashMap::new();
        state.insert(100, 'S');
        let node = tree_node(100, Some("claude"), "claude");
        let line = format_tree_node_line(&node, &rss, &state);
        assert_eq!(line, "[100] S claude  RSS 50M (claude)");
    }

    #[test]
    fn test_format_tree_node_line_no_rss_no_family() {
        let node = tree_node(200, None, "");
        let line = format_tree_node_line(&node, &HashMap::new(), &HashMap::new());
        assert_eq!(line, "[200] - ");
    }

    #[test]
    fn test_format_comm_suffix_empty() {
        assert_eq!(format_comm_suffix(""), "");
    }

    #[test]
    fn test_format_comm_suffix_nonempty() {
        assert_eq!(format_comm_suffix("claude"), " (claude)");
    }

    #[test]
    fn test_agent_forest_lines_compact_delegates() {
        let forests = vec![tree_node(100, Some("claude"), "claude")];
        let agents = vec![agent_watch(100, "claude", "claude", 1_048_576, None)];
        let lines = agent_forest_lines(&forests, &agents, &HashMap::new(), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        // compact path → falls through to flat agent_lines
        assert!(text.contains("claude:100:"));
    }
}
