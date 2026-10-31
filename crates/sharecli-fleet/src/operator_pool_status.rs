// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Pool + proc-scan operator panel shapes (FR-007 / AC-007.69–007.71).
//!
//! Shared field layout for tray, dashboard, and thermal TUI operator surfaces.

/// Runtime pool capacity snapshot for operator panels (parity with `PoolJson` / IPC `PoolSnapshot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolOperatorPanel {
    pub node_total: usize,
    pub node_idle: usize,
    pub bun_total: usize,
    pub bun_idle: usize,
    pub max_per_type: usize,
    pub healthy: bool,
}

/// Proc-scan / managed-process summary for operator panels (parity with `StatusJson` / IPC `StatusSnapshot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusOperatorPanel {
    pub scanned: usize,
    pub watched: usize,
    pub total_processes: usize,
    pub agent_rows: usize,
}

impl PoolOperatorPanel {
    /// Companion CSV block appended after gate/host_watch records (FR-007 / AC-007.79).
    pub fn format_csv_companion(&self) -> String {
        format!(
            "\nrecord,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy\n\
             pool,{},{},{},{},{},{}\n",
            self.node_total,
            self.node_idle,
            self.bun_total,
            self.bun_idle,
            self.max_per_type,
            self.healthy,
        )
    }
}

impl StatusOperatorPanel {
    /// Companion CSV block appended after pool record (FR-007 / AC-007.79).
    pub fn format_csv_companion(&self) -> String {
        format!(
            "\nrecord,scanned,watched,total_processes,agent_rows\n\
             status,{},{},{},{}\n",
            self.scanned, self.watched, self.total_processes, self.agent_rows,
        )
    }
}

/// Tray/dashboard pool operator line (AC-007.69).
pub fn format_pool_operator_line(pool: &PoolOperatorPanel) -> String {
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

/// Tray/dashboard proc-scan operator line (AC-007.69).
pub fn format_status_operator_line(status: &StatusOperatorPanel) -> String {
    format!(
        "Proc scan {} · watched {} · {} managed · {} agent row(s)",
        status.scanned, status.watched, status.total_processes, status.agent_rows,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_operator_panel_format_csv_companion() {
        let csv = PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 3,
            bun_idle: 2,
            max_per_type: 4,
            healthy: true,
        }
        .format_csv_companion();
        assert!(
            csv.contains("record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy"),
            "CSV companion MUST include pool header; got: {csv}"
        );
        assert!(
            csv.trim().ends_with("pool,2,1,3,2,4,true"),
            "CSV companion MUST include pool data row; got: {csv}"
        );
    }

    #[test]
    fn status_operator_panel_format_csv_companion() {
        let csv =
            StatusOperatorPanel { scanned: 5, watched: 3, total_processes: 12, agent_rows: 3 }
                .format_csv_companion();
        assert!(
            csv.contains("record,scanned,watched,total_processes,agent_rows"),
            "CSV companion MUST include status header; got: {csv}"
        );
        assert!(
            csv.trim().ends_with("status,5,3,12,3"),
            "CSV companion MUST include status data row; got: {csv}"
        );
    }
}
