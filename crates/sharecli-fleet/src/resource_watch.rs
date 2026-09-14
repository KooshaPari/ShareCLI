//! Host FD / network resource watch sampling (FR-007).
//!
//! Used by [`sharecli_core::Hypervisor::run`] for live watch signals and
//! re-exported from `sharecli::monitoring` for ProcessStats enrichment.
//! Per-agent PID sampling supports thermal TUI and `ps --all` (FR-006 / FR-007).

use anyhow::{Context, Result};

use crate::proc_scan::{scan_host_agents, DetectedAgent};

/// Point-in-time CPU/MEM/Net/FD resource watch sample (FR-007).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ResourceWatchSample {
    pub fd_count: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub mem_rss_bytes: u64,
    pub load_1m: f64,
}

/// Per-process RSS / FD sample for a detected agent PID (FR-007 agent slice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgentResourceSample {
    pub mem_rss_bytes: u64,
    /// Open FD count when the OS exposes it for the target PID (`/proc/{pid}/fd`).
    pub fd_count: Option<u64>,
}

impl AgentResourceSample {
    /// Sample RSS (and FD on Linux) for `pid`. Fails when the process is gone.
    pub fn capture_for_pid(pid: u32) -> Result<Self> {
        let mem_rss_bytes = sample_pid_rss_bytes(pid)?;
        let fd_count = sample_pid_fds(pid).ok();
        Ok(Self { mem_rss_bytes, fd_count })
    }
}

/// Detected agent plus live per-PID resource watch (FR-006 × FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedAgentWatch {
    pub agent: DetectedAgent,
    pub resource: AgentResourceSample,
}

/// Sum RSS across a watched-agent inventory slice (thermal TUI / gate snapshot).
pub fn sum_detected_agent_rss_bytes(agents: &[DetectedAgentWatch]) -> u64 {
    agents.iter().map(|row| row.resource.mem_rss_bytes).sum()
}

/// Sample every detected agent that is still alive on the host.
pub fn watch_detected_agents(agents: &[DetectedAgent]) -> Vec<DetectedAgentWatch> {
    agents
        .iter()
        .filter_map(|agent| {
            AgentResourceSample::capture_for_pid(agent.pid)
                .ok()
                .map(|resource| DetectedAgentWatch { agent: agent.clone(), resource })
        })
        .collect()
}

/// Scan host agents and attach per-PID resource samples.
pub fn watch_host_agents() -> Vec<DetectedAgentWatch> {
    watch_detected_agents(&scan_host_agents())
}

/// Human-readable RSS for operator panels (`1.2G`, `512M`, `48K`).
pub fn format_rss_bytes(bytes: u64) -> String {
    const GIB: u64 = 1_073_741_824;
    const MIB: u64 = 1_048_576;
    if bytes >= GIB {
        format!("{:.1}G", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.0}M", bytes as f64 / MIB as f64)
    } else {
        format!("{}K", bytes / 1024)
    }
}

/// Parse operator RSS sizes for proc filters (plain bytes or `K`/`M`/`G` suffix).
pub fn parse_rss_bytes(input: &str, flag: &str) -> Result<u64> {
    let s = input.trim();
    if s.is_empty() {
        anyhow::bail!("{flag} value MUST be non-empty");
    }
    let upper = s.to_ascii_uppercase();
    const KIB: u64 = 1024;
    const MIB: u64 = 1_048_576;
    const GIB: u64 = 1_073_741_824;
    if let Some(num) = upper.strip_suffix('G') {
        let value: f64 =
            num.trim().parse().map_err(|_| anyhow::anyhow!("invalid {flag} size: {input}"))?;
        return Ok((value * GIB as f64) as u64);
    }
    if let Some(num) = upper.strip_suffix('M') {
        let value: f64 =
            num.trim().parse().map_err(|_| anyhow::anyhow!("invalid {flag} size: {input}"))?;
        return Ok((value * MIB as f64) as u64);
    }
    if let Some(num) = upper.strip_suffix('K') {
        let value: f64 =
            num.trim().parse().map_err(|_| anyhow::anyhow!("invalid {flag} size: {input}"))?;
        return Ok((value * KIB as f64) as u64);
    }
    s.parse::<u64>().map_err(|_| anyhow::anyhow!("invalid {flag} size: {input}"))
}

impl ResourceWatchSample {
    /// Capture FD, network, RSS, and host load for the current process/host.
    pub fn capture() -> Result<Self> {
        let fd_count = sample_self_fds()?;
        let (net_rx_bytes, net_tx_bytes) = sample_host_net()?;
        let mem_rss_bytes = sample_self_rss_bytes()?;
        let load_1m = sample_host_load_1m()?;
        Ok(Self { fd_count, net_rx_bytes, net_tx_bytes, mem_rss_bytes, load_1m })
    }

    /// Operator-facing status block for `sharecli status` (FR-007 / AC-007.10).
    pub fn format_status_section(self) -> String {
        let mut out = String::from("\n=== Host Resource Watch ===\n\n");
        out.push_str(&format!(
            "Open FDs:     {}\nRSS:          {} bytes\nLoad (1m):    {:.2}\nNet RX:       {} bytes\nNet TX:       {} bytes\n",
            self.fd_count,
            self.mem_rss_bytes,
            self.load_1m,
            self.net_rx_bytes,
            self.net_tx_bytes,
        ));
        out
    }
}

/// Count open file descriptors for the current process.
pub fn sample_self_fds() -> Result<u64> {
    sample_self_fds_impl()
}

/// Sum host-wide network RX/TX byte counters (non-loopback where applicable).
pub fn sample_host_net() -> Result<(u64, u64)> {
    sample_host_net_impl()
}

/// Resident set size (RSS) in bytes for the current process.
pub fn sample_self_rss_bytes() -> Result<u64> {
    sample_self_rss_bytes_impl()
}

/// RSS in bytes for an arbitrary process PID.
pub fn sample_pid_rss_bytes(pid: u32) -> Result<u64> {
    sample_pid_rss_bytes_impl(pid)
}

/// Open FD count for an arbitrary process PID (Linux `/proc/{pid}/fd`).
pub fn sample_pid_fds(pid: u32) -> Result<u64> {
    sample_pid_fds_impl(pid)
}

/// Host 1-minute load average.
pub fn sample_host_load_1m() -> Result<f64> {
    sample_host_load_1m_impl()
}

#[cfg(target_os = "linux")]
fn sample_self_fds_impl() -> Result<u64> {
    let entries =
        std::fs::read_dir("/proc/self/fd").context("failed to read /proc/self/fd for FD watch")?;
    Ok(entries.count() as u64)
}

#[cfg(target_os = "macos")]
fn sample_self_fds_impl() -> Result<u64> {
    let entries =
        std::fs::read_dir("/dev/fd").context("failed to read /dev/fd for FD watch on macOS")?;
    Ok(entries.count() as u64)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn sample_self_fds_impl() -> Result<u64> {
    anyhow::bail!("sample_self_fds is unsupported on this OS")
}

#[cfg(target_os = "linux")]
fn sample_host_net_impl() -> Result<(u64, u64)> {
    let contents =
        std::fs::read_to_string("/proc/net/dev").context("failed to read /proc/net/dev")?;
    let mut rx_total = 0u64;
    let mut tx_total = 0u64;

    for line in contents.lines().skip(2) {
        let Some((iface, rest)) = line.split_once(':') else {
            continue;
        };
        let iface = iface.trim();
        if iface == "lo" {
            continue;
        }
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 9 {
            continue;
        }
        let rx: u64 =
            fields[0].parse().with_context(|| format!("invalid RX bytes for interface {iface}"))?;
        let tx: u64 =
            fields[8].parse().with_context(|| format!("invalid TX bytes for interface {iface}"))?;
        rx_total = rx_total.saturating_add(rx);
        tx_total = tx_total.saturating_add(tx);
    }

    Ok((rx_total, tx_total))
}

#[cfg(target_os = "macos")]
fn sample_host_net_impl() -> Result<(u64, u64)> {
    use std::process::Command;

    let output = Command::new("netstat")
        .args(["-ib"])
        .output()
        .context("failed to spawn netstat -ib for network watch")?;
    if !output.status.success() {
        anyhow::bail!("netstat -ib exited with status {}", output.status.code().unwrap_or(-1));
    }

    let stdout =
        String::from_utf8(output.stdout).context("netstat -ib output was not valid UTF-8")?;
    let mut rx_total = 0u64;
    let mut tx_total = 0u64;

    for line in stdout.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 {
            continue;
        }
        let name = cols[0];
        if name == "Name" || name.starts_with("lo") {
            continue;
        }
        let rx: u64 =
            cols[6].parse().with_context(|| format!("invalid Ibytes for interface {name}"))?;
        let tx: u64 =
            cols[9].parse().with_context(|| format!("invalid Obytes for interface {name}"))?;
        rx_total = rx_total.saturating_add(rx);
        tx_total = tx_total.saturating_add(tx);
    }

    Ok((rx_total, tx_total))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn sample_host_net_impl() -> Result<(u64, u64)> {
    anyhow::bail!("sample_host_net is unsupported on this OS")
}

#[cfg(target_os = "linux")]
fn sample_self_rss_bytes_impl() -> Result<u64> {
    let status =
        std::fs::read_to_string("/proc/self/status").context("failed to read /proc/self/status")?;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        let kb: u64 = rest
            .trim()
            .trim_end_matches(" kB")
            .parse()
            .context("invalid VmRSS in /proc/self/status")?;
        return Ok(kb.saturating_mul(1024));
    }
    anyhow::bail!("VmRSS not found in /proc/self/status")
}

#[cfg(target_os = "macos")]
fn sample_self_rss_bytes_impl() -> Result<u64> {
    use libc::{getrusage, rusage, RUSAGE_SELF};

    let mut usage: rusage = unsafe { std::mem::zeroed() };
    let rc = unsafe { getrusage(RUSAGE_SELF, &mut usage) };
    if rc != 0 {
        anyhow::bail!("getrusage(RUSAGE_SELF) failed with status {rc}");
    }
    // macOS reports ru_maxrss in bytes.
    Ok(usage.ru_maxrss as u64)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn sample_self_rss_bytes_impl() -> Result<u64> {
    anyhow::bail!("sample_self_rss_bytes is unsupported on this OS")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn sample_host_load_1m_impl() -> Result<f64> {
    use libc::{c_double, getloadavg};

    let mut loads = [0.0f64 as c_double; 3];
    let count = unsafe { getloadavg(loads.as_mut_ptr(), 3) };
    if count < 1 {
        anyhow::bail!("getloadavg returned no load samples");
    }
    Ok(loads[0])
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn sample_host_load_1m_impl() -> Result<f64> {
    anyhow::bail!("sample_host_load_1m is unsupported on this OS")
}

#[cfg(target_os = "linux")]
fn sample_pid_rss_bytes_impl(pid: u32) -> Result<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .with_context(|| format!("failed to read /proc/{pid}/status"))?;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        let kb: u64 = rest
            .trim()
            .trim_end_matches(" kB")
            .parse()
            .with_context(|| format!("invalid VmRSS for pid {pid}"))?;
        return Ok(kb.saturating_mul(1024));
    }
    anyhow::bail!("VmRSS not found in /proc/{pid}/status")
}

#[cfg(target_os = "macos")]
fn sample_pid_rss_bytes_impl(pid: u32) -> Result<u64> {
    use sysinfo::{Pid, ProcessesToUpdate, System};

    let pid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let proc =
        sys.process(pid).with_context(|| format!("process {pid} not found for RSS sample"))?;
    Ok(proc.memory())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn sample_pid_rss_bytes_impl(_pid: u32) -> Result<u64> {
    anyhow::bail!("sample_pid_rss_bytes is unsupported on this OS")
}

#[cfg(target_os = "linux")]
fn sample_pid_fds_impl(pid: u32) -> Result<u64> {
    let entries = std::fs::read_dir(format!("/proc/{pid}/fd"))
        .with_context(|| format!("failed to read /proc/{pid}/fd"))?;
    Ok(entries.count() as u64)
}

#[cfg(not(target_os = "linux"))]
fn sample_pid_fds_impl(_pid: u32) -> Result<u64> {
    anyhow::bail!("sample_pid_fds is unsupported on this OS")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proc_scan::DetectedAgent;

    #[test]
    fn resource_watch_sample_capture() {
        let sample = ResourceWatchSample::capture().expect("resource watch sample");
        assert!(sample.fd_count >= 3, "process MUST have stdin/stdout/stderr FDs");
        assert!(sample.mem_rss_bytes > 0, "RSS MUST be non-zero for live process");
        assert!(sample.load_1m >= 0.0, "load average MUST be sampled");
    }

    #[test]
    fn test_sample_self_rss_bytes() {
        let rss = super::sample_self_rss_bytes().expect("RSS sample");
        assert!(rss > 0, "live process MUST have non-zero RSS");
    }

    #[test]
    fn test_sample_host_load_1m() {
        let load = super::sample_host_load_1m().expect("load sample");
        assert!(load >= 0.0);
    }

    #[test]
    fn test_sample_self_fds() {
        let fds = super::sample_self_fds().expect("FD sample");
        assert!(fds >= 3, "open FD count MUST include stdio");
    }

    #[test]
    fn test_sample_host_net() {
        let (rx, tx) = super::sample_host_net().expect("network sample");
        let _ = (rx, tx);
    }

    #[test]
    fn test_format_status_section() {
        let sample = ResourceWatchSample::capture().expect("resource watch sample");
        let section = sample.format_status_section();
        assert!(section.contains("=== Host Resource Watch ==="));
        assert!(section.contains("Open FDs:"));
        assert!(section.contains("RSS:"));
        assert!(section.contains("Load (1m):"));
        assert!(section.contains("Net RX:"));
        assert!(section.contains("Net TX:"));
    }

    #[test]
    fn test_agent_resource_sample_self_pid() {
        let pid = std::process::id();
        let sample = AgentResourceSample::capture_for_pid(pid).expect("self PID resource sample");
        assert!(sample.mem_rss_bytes > 0, "live process MUST have non-zero RSS");
        #[cfg(target_os = "linux")]
        assert!(sample.fd_count.unwrap_or(0) >= 3, "live process MUST have stdio FDs on Linux");
    }

    #[test]
    fn test_sum_detected_agent_rss_bytes() {
        let agents = vec![
            DetectedAgentWatch {
                agent: DetectedAgent { pid: 1, family: "claude", comm: "claude".into() },
                resource: AgentResourceSample { mem_rss_bytes: 1_000, fd_count: None },
            },
            DetectedAgentWatch {
                agent: DetectedAgent { pid: 2, family: "forge", comm: "forge".into() },
                resource: AgentResourceSample { mem_rss_bytes: 2_500, fd_count: None },
            },
        ];
        assert_eq!(super::sum_detected_agent_rss_bytes(&agents), 3_500);
        assert_eq!(super::sum_detected_agent_rss_bytes(&[]), 0);
    }

    #[test]
    fn test_format_rss_bytes() {
        assert_eq!(super::format_rss_bytes(1_073_741_824), "1.0G");
        assert_eq!(super::format_rss_bytes(52_428_800), "50M");
    }

    #[test]
    fn test_parse_rss_bytes() {
        assert_eq!(super::parse_rss_bytes("100", "--min-rss").unwrap(), 100);
        assert_eq!(super::parse_rss_bytes("50M", "--min-rss").unwrap(), 52_428_800);
        assert_eq!(super::parse_rss_bytes("1G", "--max-rss").unwrap(), 1_073_741_824);
        assert!(super::parse_rss_bytes("not-a-size", "--max-rss").is_err());
    }

    // --- Additional edge-case tests ---

    #[test]
    fn test_format_rss_bytes_zero() {
        assert_eq!(super::format_rss_bytes(0), "0K");
    }

    #[test]
    fn test_format_rss_bytes_small() {
        assert_eq!(super::format_rss_bytes(512), "0K");
        assert_eq!(super::format_rss_bytes(1024), "1K");
        assert_eq!(super::format_rss_bytes(5120), "5K");
    }

    #[test]
    fn test_format_rss_bytes_mib_boundary() {
        assert_eq!(super::format_rss_bytes(1_048_575), "1023K");
        assert_eq!(super::format_rss_bytes(1_048_576), "1M");
        assert_eq!(super::format_rss_bytes(1_572_864), "2M");
    }

    #[test]
    fn test_format_rss_bytes_gib() {
        assert_eq!(super::format_rss_bytes(2_147_483_648), "2.0G");
    }

    #[test]
    fn test_parse_rss_bytes_empty_string() {
        assert!(super::parse_rss_bytes("", "--flag").is_err());
    }

    #[test]
    fn test_parse_rss_bytes_whitespace_only() {
        assert!(super::parse_rss_bytes("  ", "--flag").is_err());
    }

    #[test]
    fn test_parse_rss_bytes_invalid_suffix() {
        assert!(super::parse_rss_bytes("100T", "--flag").is_err());
    }

    #[test]
    fn test_parse_rss_bytes_decimal_g() {
        let result = super::parse_rss_bytes("1.5G", "--flag").unwrap();
        assert_eq!(result, (1.5 * 1_073_741_824.0) as u64);
    }

    #[test]
    fn test_parse_rss_bytes_decimal_m() {
        let result = super::parse_rss_bytes("2.5M", "--flag").unwrap();
        assert_eq!(result, (2.5 * 1_048_576.0) as u64);
    }

    #[test]
    fn test_parse_rss_bytes_decimal_k() {
        let result = super::parse_rss_bytes("1.5K", "--flag").unwrap();
        assert_eq!(result, (1.5 * 1024.0) as u64);
    }

    #[test]
    fn test_parse_rss_bytes_invalid_number_with_suffix() {
        assert!(super::parse_rss_bytes("abcM", "--flag").is_err());
    }

    #[test]
    fn test_parse_rss_bytes_negative() {
        assert!(super::parse_rss_bytes("-100", "--flag").is_err());
    }

    #[test]
    fn test_parse_rss_bytes_gib() {
        let result = super::parse_rss_bytes("2G", "--flag").unwrap();
        assert_eq!(result, 2_147_483_648);
    }

    #[test]
    fn test_parse_rss_bytes_case_insensitive() {
        assert_eq!(super::parse_rss_bytes("50m", "--flag").unwrap(), 52_428_800);
        assert_eq!(super::parse_rss_bytes("1g", "--flag").unwrap(), 1_073_741_824);
        assert_eq!(super::parse_rss_bytes("100k", "--flag").unwrap(), 102_400);
    }

    #[test]
    fn test_parse_rss_bytes_error_messages_include_flag() {
        let err = super::parse_rss_bytes("bad", "--my-flag").unwrap_err();
        assert!(err.to_string().contains("--my-flag"), "error MUST include flag name; got: {err}");
    }

    #[test]
    fn test_sum_detected_agent_rss_single() {
        let agents = vec![DetectedAgentWatch {
            agent: DetectedAgent { pid: 1, family: "test", comm: "test".into() },
            resource: AgentResourceSample { mem_rss_bytes: 42_000, fd_count: Some(10) },
        }];
        assert_eq!(super::sum_detected_agent_rss_bytes(&agents), 42_000);
    }

    #[test]
    fn test_sum_detected_agent_rss_two_agents() {
        let agents = vec![
            DetectedAgentWatch {
                agent: DetectedAgent { pid: 1, family: "a", comm: "a".into() },
                resource: AgentResourceSample { mem_rss_bytes: 1_000, fd_count: None },
            },
            DetectedAgentWatch {
                agent: DetectedAgent { pid: 2, family: "b", comm: "b".into() },
                resource: AgentResourceSample { mem_rss_bytes: 2_000, fd_count: None },
            },
        ];
        assert_eq!(super::sum_detected_agent_rss_bytes(&agents), 3_000);
    }

    #[test]
    fn test_watch_detected_agents_empty() {
        let result = super::watch_detected_agents(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_watch_detected_agents_filters_dead() {
        // Use a PID that definitely doesn't exist
        let agents =
            vec![DetectedAgent { pid: 2_000_000, family: "test", comm: "nonexistent".into() }];
        let result = super::watch_detected_agents(&agents);
        assert!(result.is_empty(), "dead PID MUST be filtered out by watch");
    }

    #[test]
    fn test_watch_detected_agents_self_pid() {
        let pid = std::process::id();
        let agents = vec![DetectedAgent { pid, family: "test", comm: "test-process".into() }];
        let result = super::watch_detected_agents(&agents);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].agent.pid, pid);
        assert!(result[0].resource.mem_rss_bytes > 0);
    }

    #[test]
    fn test_format_status_section_structure() {
        let sample = ResourceWatchSample {
            fd_count: 42,
            net_rx_bytes: 1000,
            net_tx_bytes: 2000,
            mem_rss_bytes: 3000,
            load_1m: 1.5,
        };
        let section = sample.format_status_section();
        assert!(section.contains("Open FDs:"));
        assert!(section.contains("42"));
        assert!(section.contains("RSS:"));
        assert!(section.contains("3000"));
        assert!(section.contains("Load (1m):"));
        assert!(section.contains("1.50"));
        assert!(section.contains("Net RX:"));
        assert!(section.contains("1000"));
        assert!(section.contains("Net TX:"));
        assert!(section.contains("2000"));
    }

    #[test]
    fn test_resource_watch_sample_default() {
        let sample = ResourceWatchSample::default();
        assert_eq!(sample.fd_count, 0);
        assert_eq!(sample.net_rx_bytes, 0);
        assert_eq!(sample.net_tx_bytes, 0);
        assert_eq!(sample.mem_rss_bytes, 0);
        assert_eq!(sample.load_1m, 0.0);
    }

    #[test]
    fn test_agent_resource_sample_default() {
        let sample = AgentResourceSample::default();
        assert_eq!(sample.mem_rss_bytes, 0);
        assert!(sample.fd_count.is_none());
    }

    #[test]
    fn test_sample_pid_rss_bytes_self() {
        let pid = std::process::id();
        let rss = super::sample_pid_rss_bytes(pid).expect("self RSS");
        assert!(rss > 0, "self PID MUST have non-zero RSS");
    }

    #[test]
    fn test_sample_pid_rss_bytes_dead_pid() {
        let result = super::sample_pid_rss_bytes(2_000_000);
        assert!(result.is_err(), "dead PID MUST return error");
    }
}
