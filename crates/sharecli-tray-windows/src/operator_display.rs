//! Compact gate / host_watch strings + thermal gate visuals for Windows tray operator UI
//! (FR-007 / AC-007.56 text, AC-007.57 icon/badge/color).
//!
//! Parity with `sharecli-tray-linux/src/operator_display.rs`; integration tests MUST keep
//! Linux/Windows format strings and visual tokens identical.

use crate::ipc::{GateStatusSnapshot, HostResourceWatchJson, PoolSnapshot, StatusSnapshot};

/// Tray thermal/gate severity bucket (dashboard `#thermal-status` + gate decision).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayGateSeverity {
    Normal,
    Warning,
    Critical,
    Offline,
}

/// Visual tokens for tray icon / badge / color (parity with `src/dashboard.html`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayGateVisual {
    pub severity: TrayGateSeverity,
    pub decision_class: &'static str,
    pub thermal_class: &'static str,
    pub color_hex: &'static str,
    pub badge_label: &'static str,
    pub linux_icon_name: &'static str,
    pub swift_symbol_name: &'static str,
}

/// Map `gate_decision` → dashboard gate CSS class (AC-007.41 / AC-007.57).
pub fn resolve_gate_decision_class(gate_decision: &str) -> &'static str {
    match gate_decision {
        "ADMIT" => "gate-admit",
        "DENY" => "gate-deny",
        _ => "gate-unavailable",
    }
}

/// Map `thermal_pressure` → dashboard `#thermal-status` CSS suffix.
pub fn resolve_thermal_class(thermal_pressure: &str) -> &'static str {
    match thermal_pressure {
        "GREEN" => "",
        "YELLOW" => "warning",
        "RED" => "critical",
        _ => "warning",
    }
}

/// Resolve tray icon/badge/color from live gate fields (AC-007.57).
pub fn resolve_tray_gate_visual(
    thermal_pressure: &str,
    gate_decision: &str,
    connected: bool,
) -> TrayGateVisual {
    if !connected {
        return TrayGateVisual {
            severity: TrayGateSeverity::Offline,
            decision_class: "gate-unavailable",
            thermal_class: "warning",
            color_hex: "#d29922",
            badge_label: "Offline",
            linux_icon_name: "network-offline",
            swift_symbol_name: "wifi.slash",
        };
    }

    let decision_class = resolve_gate_decision_class(gate_decision);
    let thermal_class = resolve_thermal_class(thermal_pressure);

    let severity = if gate_decision == "DENY" || thermal_pressure == "RED" {
        TrayGateSeverity::Critical
    } else if gate_decision == "THROTTLE"
        || thermal_pressure == "YELLOW"
        || thermal_pressure == "UNAVAILABLE"
        || decision_class == "gate-unavailable"
    {
        TrayGateSeverity::Warning
    } else {
        TrayGateSeverity::Normal
    };

    let (color_hex, badge_label, linux_icon_name, swift_symbol_name) = match severity {
        TrayGateSeverity::Critical => ("#f85149", "Critical", "dialog-error", "flame.fill"),
        TrayGateSeverity::Warning => {
            let label = if thermal_pressure == "UNAVAILABLE" { "Unavailable" } else { "Warning" };
            ("#d29922", label, "dialog-warning", "exclamationmark.triangle.fill")
        }
        TrayGateSeverity::Normal => ("#3fb950", "Normal", "utilities-system-monitor", "cpu"),
        TrayGateSeverity::Offline => unreachable!("handled above"),
    };

    TrayGateVisual {
        severity,
        decision_class,
        thermal_class,
        color_hex,
        badge_label,
        linux_icon_name,
        swift_symbol_name,
    }
}

/// Convenience: derive visuals from a gate snapshot + IPC connectivity.
pub fn resolve_tray_gate_visual_from_gate(
    gate: &GateStatusSnapshot,
    connected: bool,
) -> TrayGateVisual {
    resolve_tray_gate_visual(&gate.thermal_pressure, &gate.gate_decision, connected)
}

/// Human-readable byte count for tray lines (parity with dashboard `formatBytes`).
pub fn format_bytes_compact(n: u64) -> String {
    if n >= 1_048_576 {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

/// Primary thermal gate line for tray menus / status bars.
pub fn format_gate_tray_line(gate: &GateStatusSnapshot) -> String {
    format!(
        "Gate [{}] · {} · agents {} · {}",
        gate.gate_decision, gate.thermal_pressure, gate.detected_agents, gate.agent_contention,
    )
}

/// Agent RSS companion line (gate snapshot field).
pub fn format_gate_rss_tray_line(gate: &GateStatusSnapshot) -> String {
    format!("Agent RSS: {}", format_bytes_compact(gate.agent_total_rss_bytes))
}

/// Host load / FD / RSS line for tray menus / status bars.
pub fn format_host_watch_tray_line(host: &HostResourceWatchJson) -> String {
    format!(
        "Host load {:.2} · FDs {} · RSS {}",
        host.load_1m,
        host.fd_count,
        format_bytes_compact(host.mem_rss_bytes),
    )
}

/// Host network RX/TX line for tray menus / status bars.
pub fn format_host_net_tray_line(host: &HostResourceWatchJson) -> String {
    format!(
        "Net RX {} · TX {}",
        format_bytes_compact(host.net_rx_bytes),
        format_bytes_compact(host.net_tx_bytes),
    )
}

/// Ordered tray menu / tooltip lines: gate → host_watch (AC-007.56).
pub fn format_operator_tray_lines(
    gate: &GateStatusSnapshot,
    host: &HostResourceWatchJson,
) -> Vec<String> {
    vec![
        format_gate_tray_line(gate),
        format_gate_rss_tray_line(gate),
        format_host_watch_tray_line(host),
        format_host_net_tray_line(host),
    ]
}

/// Single-line operator summary for compact status bars.
pub fn format_operator_status_summary(
    gate: &GateStatusSnapshot,
    host: &HostResourceWatchJson,
) -> String {
    format!(
        "{} | {} | {}",
        format_gate_tray_line(gate),
        format_gate_rss_tray_line(gate),
        format_host_watch_tray_line(host),
    )
}

/// Pool capacity line from `pool.status` IPC (AC-007.69).
pub fn format_pool_tray_line(pool: &PoolSnapshot) -> String {
    format!(
        "Pool node {}/{} idle · bun {}/{} idle · max {} · {}",
        pool.node_total,
        pool.node_idle,
        pool.bun_total,
        pool.bun_idle,
        pool.max_per_type,
        if pool.healthy { "healthy" } else { "degraded" },
    )
}

/// Proc-scan summary line from `status.snapshot` IPC (AC-007.69).
pub fn format_status_snapshot_tray_line(status: &StatusSnapshot) -> String {
    format!(
        "Proc scan {} · watched {} · {} managed · {} agent row(s)",
        status.scanned,
        status.watched,
        status.total_processes,
        status.agents.len(),
    )
}

/// Supplementary pool + status operator lines (AC-007.69).
pub fn format_pool_status_operator_lines(
    pool: &PoolSnapshot,
    status: &StatusSnapshot,
) -> Vec<String> {
    vec![format_pool_tray_line(pool), format_status_snapshot_tray_line(status)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{GateStatusSnapshot, HostResourceWatchJson};

    fn sample_gate() -> GateStatusSnapshot {
        GateStatusSnapshot {
            thermal_pressure: "YELLOW".into(),
            detected_agents: 1,
            agent_total_rss_bytes: 512,
            agent_contention: "WARN".into(),
            gate_decision: "THROTTLE".into(),
        }
    }

    fn sample_host() -> HostResourceWatchJson {
        HostResourceWatchJson {
            fd_count: 10,
            net_rx_bytes: 100,
            net_tx_bytes: 200,
            mem_rss_bytes: 4096,
            load_1m: 1.25,
        }
    }

    #[test]
    fn format_operator_tray_lines_gate_before_host_watch() {
        let lines = format_operator_tray_lines(&sample_gate(), &sample_host());
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("Gate ["));
        assert!(lines[2].starts_with("Host load"));
    }

    #[test]
    fn resolve_tray_gate_visual_matches_linux_admit_green() {
        let v = resolve_tray_gate_visual("GREEN", "ADMIT", true);
        assert_eq!(v.severity, TrayGateSeverity::Normal);
        assert_eq!(v.decision_class, "gate-admit");
        assert_eq!(v.linux_icon_name, "utilities-system-monitor");
    }
}
