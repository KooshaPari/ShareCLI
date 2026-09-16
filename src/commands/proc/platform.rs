//! Platform-specific `/proc` helpers for process sampling (CPU%, age).

/// `clock ticks per second` for `/proc/stat` and `/proc/{pid}/stat`. Linux default is 100.
#[cfg(target_os = "linux")]
pub(crate) const CLK_TCK: u64 = 100;

#[cfg(not(target_os = "linux"))]
pub(crate) const CLK_TCK: u64 = 100;

/// Read `/proc/{pid}/stat` and return `(utime + stime, starttime)` in clock ticks.
/// Returns `None` when the process is gone or `/proc` is unavailable.
#[cfg(target_os = "linux")]
pub(crate) fn read_pid_stat_linux(pid: u32) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // /proc/{pid}/stat layout: `pid (comm) state ppid pgrp session tty_nr tpgid flags
    //   minflt cminflt majflt cmajflt utime stime cutime cstime priority nice num_threads
    //   itrealvalue starttime ...`. The COMM field can contain spaces, so we MUST split
    //   on the last `)` before tokenizing the trailing fields.
    let rparen = text.rfind(')')?;
    let rest = &text[rparen + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After ')', the field indices (0-based here) are: 0=state 1=ppid 2=pgrp 3=session
    // 4=tty_nr 5=tpgid 6=flags 7=minflt 8=cminflt 9=majflt 10=cmajflt 11=utime 12=stime
    // 13=cutime 14=cstime ... 19=starttime.
    if fields.len() < 20 {
        return None;
    }
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    let starttime: u64 = fields[19].parse().ok()?;
    Some((utime + stime, starttime))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_pid_stat_linux(_pid: u32) -> Option<(u64, u64)> {
    None
}

/// Read host uptime in seconds from `/proc/uptime` (first field).
#[cfg(target_os = "linux")]
pub(crate) fn read_uptime_secs() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    text.split_whitespace().next()?.parse().ok()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_uptime_secs() -> Option<f64> {
    None
}

/// Sample one-shot CPU usage percent for a PID using `/proc/{pid}/stat` + `/proc/uptime`.
/// Returns `0.0` when the data is unavailable (non-Linux or process gone).
pub(crate) fn sample_pid_cpu_percent(pid: u32) -> f64 {
    let Some((cpu_ticks, starttime)) = read_pid_stat_linux(pid) else {
        return 0.0;
    };
    let Some(uptime_secs) = read_uptime_secs() else {
        return 0.0;
    };
    let tck = CLK_TCK as f64;
    let starttime_secs = starttime as f64 / tck;
    let elapsed = uptime_secs - starttime_secs;
    if elapsed <= 0.0 {
        return 0.0;
    }
    let cpu_secs = cpu_ticks as f64 / tck;
    (cpu_secs / elapsed) * 100.0
}

/// Sample one-shot process age in seconds for a PID. Returns `u64::MAX` when data is
/// unavailable (non-Linux or process gone) so missing rows sort last under `Age`.
pub(crate) fn sample_pid_age_secs(pid: u32) -> u64 {
    let Some((_, starttime)) = read_pid_stat_linux(pid) else {
        return u64::MAX;
    };
    let Some(uptime_secs) = read_uptime_secs() else {
        return u64::MAX;
    };
    let starttime_secs = starttime as f64 / CLK_TCK as f64;
    let age = uptime_secs - starttime_secs;
    if age < 0.0 {
        0
    } else {
        age as u64
    }
}
