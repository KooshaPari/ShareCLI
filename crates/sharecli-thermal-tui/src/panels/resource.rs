//! Host resource watch and host agent inventory panels (FR-007 / FR-006).
//!
//! Both panels are pure: take a sample / agent slice + a `compact` flag, return
//! a `Vec<Line<'static>>` ready to hand to a `Paragraph` widget.

use ratatui::{style::{Color, Style}, text::{Line, Span}};

use sharecli_fleet::{proc_scan::DetectedAgent, ResourceWatchSample};

/// Lines for the host resource watch panel (FR-007 / AC-007.10, AC-007.12 TUI slice).
pub fn resource_watch_lines(
    sample: Option<ResourceWatchSample>,
    compact: bool,
) -> Vec<Line<'static>> {
    let Some(sample) = sample else {
        let msg = if compact {
            " watch unavailable"
        } else {
            "  Resource watch unavailable on this host"
        };
        return vec![Line::from(Span::styled(msg, Style::default().fg(Color::Red)))];
    };

    if compact {
        return vec![Line::from(format!(
            " FD:{} RSS:{} L:{:.1} RX:{} TX:{}",
            sample.fd_count,
            sample.mem_rss_bytes,
            sample.load_1m,
            sample.net_rx_bytes,
            sample.net_tx_bytes,
        ))];
    }

    vec![
        Line::from(format!("  Open FDs:  {}", sample.fd_count)),
        Line::from(format!("  RSS:       {} bytes", sample.mem_rss_bytes)),
        Line::from(format!("  Load (1m): {:.2}", sample.load_1m)),
        Line::from(format!("  Net RX:    {} bytes", sample.net_rx_bytes)),
        Line::from(format!("  Net TX:    {} bytes", sample.net_tx_bytes)),
    ]
}

/// Lines for host agent inventory (FR-006 / thermal operator panel).
pub fn host_agent_lines(agents: &[DetectedAgent], compact: bool) -> Vec<Line<'static>> {
    if agents.is_empty() {
        let msg = if compact { " agents: none" } else { "  Host agents: none detected" };
        return vec![Line::from(Span::raw(msg))];
    }

    if compact {
        let summary =
            agents.iter().map(|a| format!("{}:{}", a.family, a.pid)).collect::<Vec<_>>().join(" ");
        return vec![Line::from(format!(" agents: {summary}"))];
    }

    let mut lines = vec![Line::from("  Host agents:")];
    for agent in agents.iter().take(4) {
        lines.push(Line::from(format!("    {} pid={} ({})", agent.family, agent.pid, agent.comm)));
    }
    if agents.len() > 4 {
        lines.push(Line::from(format!("    … +{} more", agents.len() - 4)));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ok: bool) -> Option<ResourceWatchSample> {
        ok.then(|| ResourceWatchSample {
            fd_count: 17,
            mem_rss_bytes: 1_048_576,
            load_1m: 0.42,
            net_rx_bytes: 2048,
            net_tx_bytes: 1024,
        })
    }

    #[test]
    fn test_resource_watch_lines_unavailable_compact() {
        let lines = resource_watch_lines(None, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("watch unavailable"));
    }

    #[test]
    fn test_resource_watch_lines_unavailable_full() {
        let lines = resource_watch_lines(None, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Resource watch unavailable on this host"));
    }

    #[test]
    fn test_resource_watch_lines_compact_summary() {
        let lines = resource_watch_lines(sample(true), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("FD:17"));
        assert!(text.contains("RSS:1048576"));
        assert!(text.contains("L:0.4"));
    }

    #[test]
    fn test_resource_watch_lines_full_breakdown() {
        let lines = resource_watch_lines(sample(true), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Open FDs:  17"));
        assert!(text.contains("RSS:       1048576 bytes"));
        assert!(text.contains("Load (1m): 0.42"));
        assert!(text.contains("Net RX:    2048 bytes"));
        assert!(text.contains("Net TX:    1024 bytes"));
    }

    #[test]
    fn test_host_agent_lines_empty_compact() {
        let lines = host_agent_lines(&[], true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("agents: none"));
    }

    #[test]
    fn test_host_agent_lines_empty_full() {
        let lines = host_agent_lines(&[], false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Host agents: none detected"));
    }

    #[test]
    fn test_host_agent_lines_compact_summary() {
        let agents = vec![
            DetectedAgent { pid: 100, family: "claude", comm: "claude".into() },
            DetectedAgent { pid: 200, family: "cursor", comm: "cursor-agent".into() },
        ];
        let lines = host_agent_lines(&agents, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("claude:100"));
        assert!(text.contains("cursor:200"));
    }

    #[test]
    fn test_host_agent_lines_truncates_overflow() {
        let agents: Vec<DetectedAgent> = (0..6)
            .map(|i| DetectedAgent {
                pid: 100 + i,
                family: "claude",
                comm: format!("claude-{i}"),
            })
            .collect();
        let lines = host_agent_lines(&agents, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("+2 more"), "MUST truncate at 4 lines; got: {text}");
    }
}
