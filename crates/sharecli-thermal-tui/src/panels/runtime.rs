//! Operator panels for runtime pool and proc-scan status (FR-007 / AC-007.71).
//!
//! Two compact panels driven by the same `PoolOperatorPanel` /
//! `StatusOperatorPanel` structs that the CLI / IPC surfaces report.

use ratatui::{style::{Color, Style}, text::{Line, Span}};

use sharecli_fleet::{format_pool_operator_line, format_status_operator_line, PoolOperatorPanel, StatusOperatorPanel};

/// Lines for the runtime pool operator panel (FR-007 / AC-007.71 TUI slice).
pub fn pool_panel_lines(pool: Option<PoolOperatorPanel>, compact: bool) -> Vec<Line<'static>> {
    let Some(pool) = pool else {
        let msg = if compact { " pool unavailable" } else { "  Runtime pool unavailable" };
        return vec![Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))];
    };

    let summary = format_pool_operator_line(&pool);
    if compact {
        return vec![Line::from(format!(" {summary}"))];
    }

    vec![
        Line::from(format!("  Node idle/total: {}/{}", pool.node_idle, pool.node_total)),
        Line::from(format!("  Bun idle/total:  {}/{}", pool.bun_idle, pool.bun_total)),
        Line::from(format!(
            "  Max per type: {} · {}",
            pool.max_per_type,
            if pool.healthy { "healthy" } else { "degraded" }
        )),
    ]
}

/// Lines for the proc-scan / status snapshot operator panel (FR-007 / AC-007.71 TUI slice).
pub fn status_panel_lines(
    status: Option<StatusOperatorPanel>,
    compact: bool,
) -> Vec<Line<'static>> {
    let Some(status) = status else {
        let msg = if compact { " status unavailable" } else { "  Proc scan status unavailable" };
        return vec![Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))];
    };

    let summary = format_status_operator_line(&status);
    if compact {
        return vec![Line::from(format!(" {summary}"))];
    }

    vec![
        Line::from(format!("  Scanned:  {}", status.scanned)),
        Line::from(format!("  Watched:  {}", status.watched)),
        Line::from(format!(
            "  Managed:  {} · agent rows: {}",
            status.total_processes, status.agent_rows
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_panel_lines_full() {
        let pool = PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
        };
        let lines = pool_panel_lines(Some(pool), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Node idle/total: 1/2"));
        assert!(text.contains("Bun idle/total:  0/1"));
        assert!(text.contains("Max per type: 4 · healthy"));
    }

    #[test]
    fn test_status_panel_lines_full() {
        let status =
            StatusOperatorPanel { scanned: 50, watched: 1, total_processes: 2, agent_rows: 1 };
        let lines = status_panel_lines(Some(status), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Scanned:  50"));
        assert!(text.contains("Watched:  1"));
        assert!(text.contains("Managed:  2 · agent rows: 1"));
    }

    #[test]
    fn test_pool_status_panel_lines_compact_match_tray() {
        let pool = PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
        };
        let status =
            StatusOperatorPanel { scanned: 50, watched: 1, total_processes: 2, agent_rows: 1 };
        let pool_text: String =
            pool_panel_lines(Some(pool), true).iter().map(|l| l.to_string()).collect();
        let status_text: String =
            status_panel_lines(Some(status), true).iter().map(|l| l.to_string()).collect();
        assert!(pool_text.contains("Pool node 2/1 idle · bun 1/0 idle · max 4 · healthy"));
        assert!(status_text.contains("Proc scan 50 · watched 1 · 2 managed · 1 agent row(s)"));
    }

    #[test]
    fn test_pool_panel_lines_unavailable_compact() {
        let lines = pool_panel_lines(None, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("pool unavailable"));
    }

    #[test]
    fn test_status_panel_lines_unavailable_compact() {
        let lines = status_panel_lines(None, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("status unavailable"));
    }
}
