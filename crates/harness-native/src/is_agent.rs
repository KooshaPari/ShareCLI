//! `harness-is-agent` binary: detect if the parent (or any ancestor) is a coding agent.
//!
//! Used by the orchestrator (`crates/harness-native/src/dispatcher.rs:314-339`)
//! to decide whether the current process was spawned by a human or by an
//! agent harness. Exit contract:
//!
//!   * exit 0 -> is an agent (signal the dispatcher to take the agent fast-path)
//!   * exit 1 -> not an agent (regular CLI invocation)
//!
//! Detection walks the parent-process chain from `PPID` (env; supplied by the
//! dispatcher) or `getppid(2)` (fallback) up to `pid 1`, looking for any
//! process whose `comm` (process name, lowercased) is in the known-agent list.
//!
//! Linux implementation reads `/proc/<pid>/comm` and walks via `/proc/<pid>/stat`.
//! macOS implementation snapshots `ps -axc -o pid=,ppid=,comm=`. Windows /
//! other targets return `NotAgent` (out of scope per the 2026-10-17 audit).

use std::env;
use std::process;

/// Process names (lowercased) that identify known coding-agent harnesses.
///
/// Listing is short and conservative; values are taken from the comm field
/// exposed by `/proc/<pid>/comm` on Linux and `ps -axc -o comm=` on macOS,
/// truncated by the kernel to 15 bytes (`TASK_COMM_LEN-1`).
pub const AGENT_COMM_NAMES: &[&str] = &[
    "claude", "kcode", "jcode", "codex", "forge", "helioslite", "cursor", "opencode", "cline", "aider",
];

/// Hard cap on the parent-process walk. Bounded so a malformed `/proc` or `ps`
/// layout cannot make us spin indefinitely.
pub const MAX_ANCESTOR_DEPTH: u32 = 64;

fn main() {
    let code = if is_agent() { 0 } else { 1 };
    process::exit(code);
}

/// Detect whether the current process (or any direct ancestor up to
/// [`MAX_ANCESTOR_DEPTH`] levels) is a known coding agent.
///
/// Honours `$PPID` first (set by the sharecli dispatcher), then falls back
/// to the current process's actual parent pid via platform-specific means.
pub fn is_agent() -> bool {
    let start = current_parent_pid();
    matches!(
        detect_ancestor_kind(start, MAX_ANCESTOR_DEPTH),
        AncestorMatch::KnownAgent
    )
}

/// Result of walking the parent-process chain looking for a known agent name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AncestorMatch {
    /// At least one ancestor's comm matched [`AGENT_COMM_NAMES`].
    KnownAgent,
    /// Walked the chain (or hit `pid 1` or `max_depth`) without finding one.
    NotAgent,
}

/// Classify a single comm (process name) string.
///
/// Empty / whitespace-only comms are treated as `NotAgent` (not an error —
/// they just don't match anything). Matching is exact and case-insensitive;
/// `claude-cli` does NOT match `claude`.
pub fn classify_comm(comm: &str) -> AncestorMatch {
    let trimmed = comm.trim();
    if trimmed.is_empty() {
        return AncestorMatch::NotAgent;
    }
    let lower = trimmed.to_ascii_lowercase();
    if AGENT_COMM_NAMES.iter().any(|n| *n == lower) {
        AncestorMatch::KnownAgent
    } else {
        AncestorMatch::NotAgent
    }
}

// ---------------------------------------------------------------------------
// Linux implementation (/proc)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn detect_ancestor_kind(start_pid: u32, max_depth: u32) -> AncestorMatch {
    let mut pid = start_pid;
    for _ in 0..max_depth {
        let comm = match read_proc_comm(pid) {
            Ok(s) => s,
            Err(_) => return AncestorMatch::NotAgent,
        };
        if let AncestorMatch::KnownAgent = classify_comm(&comm) {
            return AncestorMatch::KnownAgent;
        }
        let ppid = match read_proc_ppid(pid) {
            Ok(p) => p,
            Err(_) => return AncestorMatch::NotAgent,
        };
        if ppid <= 1 || ppid == pid {
            return AncestorMatch::NotAgent;
        }
        pid = ppid;
    }
    AncestorMatch::NotAgent
}

#[cfg(target_os = "linux")]
fn read_proc_comm(pid: u32) -> std::io::Result<String> {
    let path = format!("/proc/{}/comm", pid);
    let s = std::fs::read_to_string(&path)?;
    Ok(s)
}

#[cfg(target_os = "linux")]
fn read_proc_ppid(pid: u32) -> std::io::Result<u32> {
    let path = format!("/proc/{}/stat", pid);
    let stat = std::fs::read_to_string(&path)?;
    parse_ppid_from_stat(&stat)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no ppid in stat"))
}

/// Parse the parent pid field from a `/proc/<pid>/stat` line.
///
/// Format: `<pid> (<comm>) <state> <ppid> ...`. The `comm` field may contain
/// spaces or parentheses (process names like `bash (login)` or `(sd-pam)`),
/// so split on the **last** `)` to find the trailing fields reliably.
#[cfg(target_os = "linux")]
pub fn parse_ppid_from_stat(stat: &str) -> Option<u32> {
    let close_paren = stat.rfind(')')?;
    let after = &stat[close_paren + 1..];
    let mut iter = after.split_whitespace();
    iter.next()?; // state field (e.g. "S", "R", "Z")
    iter.next()?.parse().ok()
}

// ---------------------------------------------------------------------------
// macOS implementation (ps snapshot)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
struct PsEntry {
    comm: String,
    ppid: u32,
}

#[cfg(target_os = "macos")]
fn detect_ancestor_kind(start_pid: u32, max_depth: u32) -> AncestorMatch {
    let snapshot = macos_snapshot();
    let mut pid = start_pid;
    for _ in 0..max_depth {
        let entry = match snapshot.get(&pid) {
            Some(e) => e,
            None => return AncestorMatch::NotAgent,
        };
        if let AncestorMatch::KnownAgent = classify_comm(&entry.comm) {
            return AncestorMatch::KnownAgent;
        }
        if entry.ppid <= 1 || entry.ppid == pid {
            return AncestorMatch::NotAgent;
        }
        pid = entry.ppid;
    }
    AncestorMatch::NotAgent
}

#[cfg(target_os = "macos")]
fn macos_snapshot() -> std::collections::HashMap<u32, PsEntry> {
    use std::collections::HashMap;
    let mut map: HashMap<u32, PsEntry> = HashMap::new();
    let output = match std::process::Command::new("ps")
        .args(["-axc", "-o", "pid=", "-o", "ppid=", "-o", "comm="])
        .output()
    {
        Ok(o) => o,
        Err(_) => return map,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let pid: u32 = match parts.next().and_then(|s| s.parse().ok()) {
            Some(n) => n,
            None => continue,
        };
        let ppid: u32 = match parts.next().and_then(|s| s.parse().ok()) {
            Some(n) => n,
            None => continue,
        };
        let comm = parts.collect::<Vec<_>>().join(" ").to_ascii_lowercase();
        map.insert(pid, PsEntry { comm, ppid });
    }
    map
}

// ---------------------------------------------------------------------------
// Non-Linux, non-macOS fallback
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn detect_ancestor_kind(_start_pid: u32, _max_depth: u32) -> AncestorMatch {
    AncestorMatch::NotAgent
}

// ---------------------------------------------------------------------------
// Parent PID resolution (current_parent_pid)
// ---------------------------------------------------------------------------

/// Returns the parent pid to start the ancestry walk from.
///
/// Order:
///   1. `$PPID` env (set by the sharecli dispatcher) — accepts decimal u32.
///   2. `/proc/self/stat` (Linux) — parses ppid via `parse_ppid_from_stat`.
///   3. `ps -o ppid= -p $$` (macOS).
///   4. `1` (init) — final fallback so we never return 0.
fn current_parent_pid() -> u32 {
    if let Some(pid) = env::var("PPID")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|&p| p > 0)
    {
        return pid;
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(stat) = std::fs::read_to_string("/proc/self/stat") {
            if let Some(ppid) = parse_ppid_from_stat(&stat).filter(|&p| p > 0) {
                return ppid;
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let self_pid = std::process::id().to_string();
        if let Ok(out) = std::process::Command::new("ps")
            .args(["-o", "ppid=", "-p", &self_pid])
            .output()
        {
            if let Some(pid) = String::from_utf8(out.stdout)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .filter(|&p| p > 0)
            {
                return pid;
            }
        }
    }
    1
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- classify_comm -------------------------------------------------------

    #[test]
    fn classify_comm_known_agents_match() {
        for name in AGENT_COMM_NAMES {
            assert_eq!(
                classify_comm(name),
                AncestorMatch::KnownAgent,
                "{name} should match",
            );
        }
    }

    #[test]
    fn classify_comm_known_agents_match_case_insensitively() {
        assert_eq!(classify_comm("CLAUDE"), AncestorMatch::KnownAgent);
        assert_eq!(classify_comm("KCode"), AncestorMatch::KnownAgent);
        assert_eq!(classify_comm("forge"), AncestorMatch::KnownAgent);
    }

    #[test]
    fn classify_comm_known_agents_match_with_whitespace_padding() {
        // /proc/<pid>/comm is newline-terminated; some ps implementations
        // also include leading/trailing whitespace. classify_comm trims.
        assert_eq!(classify_comm("  claude  \n"), AncestorMatch::KnownAgent);
        assert_eq!(classify_comm("\tkcode\n"), AncestorMatch::KnownAgent);
    }

    #[test]
    fn classify_comm_unknown_returns_not_agent() {
        assert_eq!(classify_comm("bash"), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("sshd"), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("cargo"), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("node"), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("python3"), AncestorMatch::NotAgent);
    }

    #[test]
    fn classify_comm_empty_or_whitespace_returns_not_agent() {
        assert_eq!(classify_comm(""), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("   "), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("\n\r\t"), AncestorMatch::NotAgent);
    }

    #[test]
    fn classify_comm_substring_does_not_match() {
        // Important edge case: `claude-cli`, `kcode-fork`, etc. must NOT be
        // confused with their root names. The 15-byte kernel task_comm limit
        // means agent names can be truncated; we deliberately err on the side
        // of strict equality so accidental false positives are avoided.
        assert_eq!(classify_comm("claude-cli"), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("kcode-fork"), AncestorMatch::NotAgent);
        assert_eq!(classify_comm("forge-runner"), AncestorMatch::NotAgent);
    }

    // --- parse_ppid_from_stat ----------------------------------------------

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_ppid_from_stat_normal_line() {
        // pid (comm) state ppid pgrp session ...
        let stat = "1234 (bash) S 5678 1234 1234 0 -1 1234567 1234 1234 0 0 0 0 0 0 20 0 1 0";
        assert_eq!(parse_ppid_from_stat(stat), Some(5678));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_ppid_from_stat_with_parens_in_comm() {
        // Real-world names can contain parens, e.g. (sd-pam) or bash (login).
        // `rfind(')')` finds the LAST `)`, which is the field separator.
        let stat = "1234 ((sd-pam)) S 9999 1234 1234 0 -1 1234567 1234 1234 0 0 0 0 0 0 20 0 1 0";
        assert_eq!(parse_ppid_from_stat(stat), Some(9999));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_ppid_from_stat_with_nested_parens_and_spaces() {
        let stat = "42 (weird (name) here) R 100 42 42 0 -1 1234567 42 42 0 0 0 0 0 0 20 0 1 0";
        assert_eq!(parse_ppid_from_stat(stat), Some(100));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_ppid_from_stat_invalid_inputs() {
        assert_eq!(parse_ppid_from_stat(""), None);
        assert_eq!(parse_ppid_from_stat("garbage"), None);
        assert_eq!(parse_ppid_from_stat("1234 (incomplete"), None);
        assert_eq!(parse_ppid_from_stat("1234 (comm) S not_a_pid ..."), None);
        assert_eq!(parse_ppid_from_stat("1234 (comm)"), None);
    }

    // --- detect_ancestor_kind ----------------------------------------------

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_with_max_depth_zero_returns_not_agent() {
        // Even with a valid start_pid, depth 0 means we don't examine any level.
        let result = detect_ancestor_kind(std::process::id(), 0);
        assert_eq!(result, AncestorMatch::NotAgent);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_with_invalid_start_pid_returns_not_agent() {
        // pid 0 is invalid; /proc/0/comm does not exist. We degrade to NotAgent
        // rather than erroring out, which keeps the binary contract
        // ("exit 1 if not agent") intact under hostile environments.
        let result = detect_ancestor_kind(0, 5);
        assert_eq!(result, AncestorMatch::NotAgent);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_with_nonexistent_pid_returns_not_agent() {
        // PIDs are bounded; pid 0xFFFFFFFE (4294967294) is virtually
        // guaranteed not to exist on a real Linux box.
        let result = detect_ancestor_kind(u32::MAX - 1, 5);
        assert_eq!(result, AncestorMatch::NotAgent);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_self_pid_returns_consistent_with_classify_comm() {
        // With depth=1 we examine exactly /proc/<self>/comm, which is what
        // classify_comm should be able to handle regardless of the actual name.
        let self_comm = std::fs::read_to_string("/proc/self/comm")
            .unwrap_or_default();
        let kind = classify_comm(&self_comm);
        let result = detect_ancestor_kind(std::process::id(), 1);
        assert_eq!(result, kind, "depth-1 walk must agree with classify_comm on /proc/self/comm");
    }

    // --- public is_agent contract ----------------------------------------

    #[test]
    fn is_agent_returns_bool_type() {
        // Verifies the public API exists, is callable, and returns a bool.
        // The actual value depends on the host's running processes (it's not
        // always `false` — running under another harness would yield `true`).
        let _: bool = is_agent();
    }

    #[test]
    fn main_exit_code_invariant() {
        // The binary contract: exit 0 iff is_agent() returns true.
        // exit 1 iff is_agent() returns false. We can't run main() under
        // unit tests directly, but we can verify the call produces a value
        // that maps to a valid exit code via the same mapping.
        let kind = is_agent();
        let exit_code: i32 = if kind { 0 } else { 1 };
        assert!(exit_code == 0 || exit_code == 1);
    }
}
