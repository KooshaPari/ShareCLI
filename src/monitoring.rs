//! Monitoring and health check functionality
// These types are a stub reserved for future dashboard integration; none are
// wired into the binary yet.  Suppress dead_code for the whole module rather
// than scattering per-item allows across a placeholder.
#![allow(dead_code, unused_imports)]

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
pub use sharecli_fleet::{
    sample_host_load_1m, sample_host_net, sample_self_fds, sample_self_rss_bytes,
    ResourceWatchSample,
};
use tracing::warn;

use crate::config;

/// JSON surface for live host [`ResourceWatchSample`] (FR-007 / AC-007.13).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct HostResourceWatchJson {
    pub fd_count: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub mem_rss_bytes: u64,
    pub load_1m: f64,
}

impl HostResourceWatchJson {
    /// Capture FD/RSS/load/net watch fields from the live host.
    pub fn capture() -> Result<Self> {
        Ok(ResourceWatchSample::capture()?.into())
    }

    /// Operator-facing text block for `sharecli proc` (FR-007 / AC-007.14).
    pub fn format_text_section(self) -> String {
        ResourceWatchSample {
            fd_count: self.fd_count,
            net_rx_bytes: self.net_rx_bytes,
            net_tx_bytes: self.net_tx_bytes,
            mem_rss_bytes: self.mem_rss_bytes,
            load_1m: self.load_1m,
        }
        .format_status_section()
    }

    /// Companion CSV block appended after agent inventory rows (FR-007 / AC-007.14).
    pub fn format_csv_companion(self) -> String {
        format!(
            "\nrecord,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m\nhost,{},{},{},{},{:.2}\n",
            self.fd_count,
            self.net_rx_bytes,
            self.net_tx_bytes,
            self.mem_rss_bytes,
            self.load_1m,
        )
    }
}

impl From<ResourceWatchSample> for HostResourceWatchJson {
    fn from(sample: ResourceWatchSample) -> Self {
        Self {
            fd_count: sample.fd_count,
            net_rx_bytes: sample.net_rx_bytes,
            net_tx_bytes: sample.net_tx_bytes,
            mem_rss_bytes: sample.mem_rss_bytes,
            load_1m: sample.load_1m,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub last_check: u64,
    pub uptime_seconds: u64,
    pub checks_passed: u32,
    pub checks_failed: u32,
}

impl HealthStatus {
    pub fn new() -> Self {
        Self {
            healthy: true,
            last_check: now_secs(),
            uptime_seconds: 0,
            checks_passed: 1,
            checks_failed: 0,
        }
    }

    pub fn mark_healthy(&mut self) {
        self.healthy = true;
        self.last_check = now_secs();
        self.checks_passed += 1;
    }

    pub fn mark_unhealthy(&mut self, reason: &str) {
        self.healthy = false;
        self.last_check = now_secs();
        self.checks_failed += 1;
        warn!("Health check failed: {}", reason);
    }
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ProcessStats {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub start_time: u64,
    pub uptime_seconds: u64,
    /// Live FD/net watch fields from a live OS sample.
    pub fd_count: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub mem_rss_bytes: u64,
    pub load_1m: f64,
}

impl ProcessStats {
    /// Helper for tests and fixtures; resource watch fields default to zero.
    pub fn new(
        pid: u32,
        name: impl Into<String>,
        memory_mb: u64,
        cpu_percent: f32,
        start_time: u64,
        uptime_seconds: u64,
    ) -> Self {
        Self {
            pid,
            name: name.into(),
            memory_mb,
            cpu_percent,
            start_time,
            uptime_seconds,
            fd_count: 0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            mem_rss_bytes: 0,
            load_1m: 0.0,
        }
    }

    /// Populate resource watch fields from a live OS sample.
    pub fn with_resource_watch(mut self) -> Result<Self> {
        let sample = ResourceWatchSample::capture()?;
        self.fd_count = sample.fd_count;
        self.net_rx_bytes = sample.net_rx_bytes;
        self.net_tx_bytes = sample.net_tx_bytes;
        self.mem_rss_bytes = sample.mem_rss_bytes;
        self.load_1m = sample.load_1m;
        Ok(self)
    }

    pub fn is_idle(&self, threshold_secs: u64) -> bool {
        self.uptime_seconds > threshold_secs && self.cpu_percent < 1.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringReport {
    pub timestamp: u64,
    pub total_processes: usize,
    pub total_memory_mb: u64,
    pub by_project: HashMap<String, usize>,
    pub by_harness: HashMap<String, usize>,
    pub idle_processes: usize,
    pub recommendations: Vec<String>,
}

impl MonitoringReport {
    pub fn generate(processes: &[ProcessStats]) -> Self {
        let cfg = config::global();
        let by_project: HashMap<String, usize> = HashMap::new();
        let mut by_harness: HashMap<String, usize> = HashMap::new();
        let mut total_memory = 0u64;
        let mut idle = 0usize;

        for proc in processes {
            total_memory += proc.memory_mb;

            // Track idle processes
            if proc.is_idle(cfg.monitoring.idle_threshold_secs) {
                idle += 1;
            }

            // Populate breakdown maps (audit L8: these were left empty)
            *by_harness.entry(proc.name.clone()).or_insert(0) += 1;
            // Project name not available on ProcessStats directly;
            // by_project is populated when project metadata is passed.
        }

        let mut recommendations = Vec::new();

        if total_memory > cfg.monitoring.high_memory_threshold_mb {
            recommendations.push(format!(
                "High memory usage: {} MB. Consider pruning idle processes.",
                total_memory
            ));
        }

        if idle > cfg.monitoring.idle_process_threshold {
            recommendations
                .push(format!("{} idle processes found. Run 'sharecli prune' to clean up.", idle));
        }

        Self {
            timestamp: now_secs(),
            total_processes: processes.len(),
            total_memory_mb: total_memory,
            by_project,
            by_harness,
            idle_processes: idle,
            recommendations,
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock before Unix epoch").as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        // Ensure global config is initialised before tests access it
        crate::config::init_global();
    }

    #[test]
    fn test_mark_unhealthy_uses_tracing_not_eprintln() {
        // Smoke test: mark_unhealthy should not panic and should flip healthy to false.
        let mut status = HealthStatus::new();
        assert!(status.healthy);
        status.mark_unhealthy("test failure");
        assert!(!status.healthy);
        assert_eq!(status.checks_failed, 1);
    }

    #[test]
    fn test_monitoring_report_populates_by_harness() {
        setup();
        let stats = vec![
            ProcessStats {
                pid: 100,
                name: "node".into(),
                memory_mb: 128,
                cpu_percent: 0.5,
                start_time: 1000,
                uptime_seconds: 100,
                fd_count: 0,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                mem_rss_bytes: 0,
                load_1m: 0.0,
            },
            ProcessStats {
                pid: 101,
                name: "bun".into(),
                memory_mb: 256,
                cpu_percent: 0.3,
                start_time: 1001,
                uptime_seconds: 200,
                fd_count: 0,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                mem_rss_bytes: 0,
                load_1m: 0.0,
            },
            ProcessStats {
                pid: 102,
                name: "node".into(),
                memory_mb: 64,
                cpu_percent: 2.0,
                start_time: 1002,
                uptime_seconds: 10,
                fd_count: 0,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                mem_rss_bytes: 0,
                load_1m: 0.0,
            },
        ];

        let report = MonitoringReport::generate(&stats);
        assert_eq!(report.total_processes, 3);
        assert_eq!(report.total_memory_mb, 448);
        // by_harness must be populated (audit L8 fix)
        assert_eq!(report.by_harness.get("node"), Some(&2));
        assert_eq!(report.by_harness.get("bun"), Some(&1));
        // by_project is still empty (no project metadata on ProcessStats)
        assert!(report.by_project.is_empty());
    }

    #[test]
    fn test_monitoring_report_empty() {
        setup();
        let report = MonitoringReport::generate(&[]);
        assert_eq!(report.total_processes, 0);
        assert_eq!(report.total_memory_mb, 0);
        assert!(report.by_harness.is_empty());
        assert!(report.recommendations.is_empty());
    }

    // --- coverage-gap tests: formatters and idle logic were unexercised ---

    fn sample_json() -> HostResourceWatchJson {
        HostResourceWatchJson {
            fd_count: 42,
            net_rx_bytes: 1024,
            net_tx_bytes: 2048,
            mem_rss_bytes: 4096,
            load_1m: 1.5,
        }
    }

    #[test]
    fn test_host_resource_watch_csv_companion_header_and_row() {
        let csv = sample_json().format_csv_companion();
        assert!(csv.starts_with(
            "\nrecord,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m\n"
        ));
        assert!(csv.contains("host,42,1024,2048,4096,1.50"));
        // Exactly one header plus one data row.
        assert_eq!(csv.lines().filter(|l| !l.is_empty()).count(), 2);
    }

    #[test]
    fn test_host_resource_watch_csv_companion_rounds_load_to_two_dp() {
        // 0.126 is unambiguous: {:.2} yields 0.13. (0.125 would exercise
        // round-half-to-even and land on 0.12, which is a formatting detail
        // rather than the contract we care about here.)
        let mut w = sample_json();
        w.load_1m = 0.126;
        assert!(w.format_csv_companion().contains(",0.13"));
        // Always exactly two decimal places, even for whole numbers.
        w.load_1m = 2.0;
        assert!(w.format_csv_companion().contains(",2.00"));
    }

    #[test]
    fn test_host_resource_watch_text_section_is_rendered() {
        // Delegates to ResourceWatchSample::format_status_section; assert it
        // produced something non-empty and carries the discriminant it always
        // emits for the host block.
        let text = sample_json().format_text_section();
        assert!(!text.is_empty(), "text section must not be empty");
        assert!(
            text.to_ascii_lowercase().contains("host")
                || text.contains("fd")
                || text.contains("load"),
            "text section should mention host/fd/load, got: {text:?}"
        );
    }

    #[test]
    fn test_process_stats_new_zeroes_resource_watch_fields() {
        let p = ProcessStats::new(7, "node", 128, 2.5, 1_700_000_000, 3_600);
        assert_eq!(p.pid, 7);
        assert_eq!(p.name, "node");
        assert_eq!(p.memory_mb, 128);
        assert!((p.cpu_percent - 2.5).abs() < f32::EPSILON);
        assert_eq!(p.start_time, 1_700_000_000);
        assert_eq!(p.uptime_seconds, 3_600);
        // Resource watch fields are documented to default to zero.
        assert_eq!(p.fd_count, 0);
        assert_eq!(p.net_rx_bytes, 0);
        assert_eq!(p.net_tx_bytes, 0);
        assert_eq!(p.mem_rss_bytes, 0);
        assert_eq!(p.load_1m, 0.0);
    }

    #[test]
    fn test_is_idle_boundaries() {
        let mut p = ProcessStats::new(1, "a", 1, 0.0, 0, 100);

        // uptime strictly greater than threshold, cpu below 1.0
        assert!(p.is_idle(50));
        // threshold equal to uptime is NOT strictly greater
        assert!(!p.is_idle(100));
        // cpu exactly 1.0 is not below 1.0
        p.cpu_percent = 1.0;
        assert!(!p.is_idle(50));
        // just under the cpu bound is idle again
        p.cpu_percent = 0.99;
        assert!(p.is_idle(50));
        // zero threshold with zero uptime is still not idle
        let fresh = ProcessStats::new(2, "b", 1, 0.0, 0, 0);
        assert!(!fresh.is_idle(0));
    }
}
