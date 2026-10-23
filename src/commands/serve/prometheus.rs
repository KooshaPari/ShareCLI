// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Prometheus text-exposition rendering for sharecli process metrics.
//!
//! `render_prometheus_metrics` is a pure function: it takes the
//! process list and the health-check map and returns the metrics
//! document. The HTTP handler in `super::handlers` then appends the
//! HTTP RED metrics and trace-context counters before serving.
//!
//! `escape_label_value` follows the Prometheus exposition format
//! (backslash, double-quote, newline).

use std::collections::HashMap;

use crate::health_check::HealthStatus;
use crate::runtime::ProcessInfo;

/// Escape a Prometheus label value: backslash, double-quote, and newline must be escaped.
pub fn escape_label_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// Render all sharecli process metrics in Prometheus text exposition format.
///
/// This is a pure function — no I/O — so it is straightforward to unit-test.
pub fn render_prometheus_metrics(
    processes: &[ProcessInfo],
    health_map: &HashMap<String, HealthStatus>,
) -> String {
    let mut out = String::with_capacity(512);

    // -- sharecli_process_memory_mb ------------------------------------------
    out.push_str("# HELP sharecli_process_memory_mb Resident memory usage in MiB per process\n");
    out.push_str("# TYPE sharecli_process_memory_mb gauge\n");
    for p in processes {
        let name = escape_label_value(&p.name);
        out.push_str(&format!(
            "sharecli_process_memory_mb{{process=\"{}\"}} {}\n",
            name, p.memory_mb
        ));
    }

    // -- sharecli_process_up -------------------------------------------------
    out.push_str(
        "# HELP sharecli_process_up 1 if the process is considered healthy, 0 otherwise\n",
    );
    out.push_str("# TYPE sharecli_process_up gauge\n");
    for p in processes {
        let name = escape_label_value(&p.name);
        let up = if let Some(status) = health_map.get(&p.name) {
            if status.healthy {
                1u8
            } else {
                0u8
            }
        } else {
            // No health-check configured → process is running, treat as up.
            1u8
        };
        out.push_str(&format!("sharecli_process_up{{process=\"{}\"}} {}\n", name, up));
    }

    // -- sharecli_health_check_consecutive_failures --------------------------
    out.push_str("# HELP sharecli_health_check_consecutive_failures Number of consecutive health-check failures\n");
    out.push_str("# TYPE sharecli_health_check_consecutive_failures gauge\n");
    for (proc_name, status) in health_map.iter() {
        let name = escape_label_value(proc_name);
        out.push_str(&format!(
            "sharecli_health_check_consecutive_failures{{process=\"{}\"}} {}\n",
            name, status.consecutive_failures
        ));
    }

    // -- sharecli_health_check_status ----------------------------------------
    out.push_str("# HELP sharecli_health_check_status 1 if health check is passing, 0 otherwise\n");
    out.push_str("# TYPE sharecli_health_check_status gauge\n");
    for (proc_name, status) in health_map.iter() {
        let name = escape_label_value(proc_name);
        let val = if status.healthy { 1u8 } else { 0u8 };
        out.push_str(&format!("sharecli_health_check_status{{process=\"{}\"}} {}\n", name, val));
    }

    out
}
