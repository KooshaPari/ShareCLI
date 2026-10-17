//! Lines for the thermal gate decision panel (FR-007 / AC-007.26 TUI slice).
//!
//! Derives ADMIT/DENY from [`GateStatusSnapshot`] built with live agent count
//! and RSS (parity with `sharecli proc` / `status --json` gate).

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use sharecli_fleet::{format_rss_bytes, GateStatusSnapshot};
use sharecli_fleet::thermal::ThermalLevel;

use super::level::{decision_color, level_pressure_raw};

/// Lines for the thermal gate decision panel (FR-007 / AC-007.26 TUI slice).
pub fn gate_panel_lines(
    snap: &GateStatusSnapshot,
    thermal: ThermalLevel,
    compact: bool,
) -> Vec<Line<'static>> {
    let decision = snap.gate_decision.as_str();
    let color = if decision == "DENY" { Color::Red } else { decision_color(thermal) };
    let decision_span = Span::styled(
        format!("[ {decision} ]"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    );

    if compact {
        return vec![Line::from(vec![
            Span::raw(" Gate: "),
            decision_span,
            Span::raw(format!(" RSS:{} {}", snap.agent_total_rss_bytes, snap.agent_contention)),
        ])];
    }

    let hint = if decision == "DENY" {
        if thermal == ThermalLevel::Red {
            "  — hypervisor will retry up to 5x before returning Err"
        } else {
            "  — agent contention limit reached; hypervisor will back-pressure"
        }
    } else {
        ""
    };

    vec![
        Line::from(vec![Span::raw("  Gate decision: "), decision_span, Span::raw(hint)]),
        Line::from(format!(
            "  Agent RSS total: {} bytes ({})",
            snap.agent_total_rss_bytes,
            format_rss_bytes(snap.agent_total_rss_bytes)
        )),
        Line::from(format!("  Agent contention: {}", snap.agent_contention)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sharecli_fleet::gate_status_snapshot_with_rss;

    // --- gate_panel_lines (AC-007.26) ---
    #[test]
    fn test_gate_panel_lines_rss_refuse_denies_on_green() {
        const RSS_REFUSE: u64 = 32 * 1_073_741_824;
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Green, 1, RSS_REFUSE);
        assert_eq!(snap.gate_decision, "DENY");
        assert_eq!(snap.agent_contention, "REFUSE");

        let lines = gate_panel_lines(&snap, ThermalLevel::Green, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("DENY"), "full gate panel MUST show DENY; got: {text}");
        assert!(
            text.contains(&RSS_REFUSE.to_string()),
            "full gate panel MUST show agent RSS total; got: {text}"
        );
        assert!(text.contains("REFUSE"), "full gate panel MUST show contention; got: {text}");

        let compact = gate_panel_lines(&snap, ThermalLevel::Green, true);
        let compact_text: String = compact.iter().map(|l| l.to_string()).collect();
        assert!(compact_text.contains("DENY"), "compact gate MUST show DENY; got: {compact_text}");
        assert!(
            compact_text.contains(&RSS_REFUSE.to_string()),
            "compact gate MUST show agent RSS total; got: {compact_text}"
        );
        assert!(
            compact_text.contains("REFUSE"),
            "compact gate MUST show contention; got: {compact_text}"
        );
    }

    #[test]
    fn test_gate_panel_lines_count_only_warn_still_admits() {
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Green, 4, 0);
        assert_eq!(snap.gate_decision, "ADMIT");
        assert_eq!(snap.agent_contention, "WARN");

        let lines = gate_panel_lines(&snap, ThermalLevel::Green, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("ADMIT"));
        assert!(text.contains("WARN"));
    }

    // Touch level_pressure_raw so the import isn't dead in --no-default-features builds.
    #[test]
    fn test_level_pressure_raw_import_is_alive() {
        assert_eq!(level_pressure_raw(ThermalLevel::Red), 4);
    }
}
