//! harness-is-agent binary: detect if current process is an agent
//!
//! Detection strategy: read the parent process (`PPID`) command name via
//! `ps -p <ppid> -o comm=` and classify whether it matches a known agent
//! harness. If the parent name contains one of the recognized agent
//! tokens (case-insensitive substring match), the binary exits 0; otherwise
//! it exits 1.
//!
//! Recognized agent harnesses: `claude`, `codex`, `jcode`, `kcode`,
//! `opencode`, `cursor`, `aider`. Process names containing any of these
//! substrings (e.g. `Claude Code.app/Contents/MacOS/...`, `codex-cli`)
//! trigger an agent-positive result.
//!
//! Platform note: the `ps` invocation is portable (macOS / Linux); a
//! `/proc/$PPID/comm` fast path could be added for Linux only via a
//! `#[cfg(target_os = "linux")]` arm in a follow-up.

use std::env;
use std::process;

/// Process-name substrings that identify a known agent harness.
///
/// Match is case-insensitive substring (the `comm` field is lowercased
/// before comparison).
const AGENT_TOKENS: &[&str] = &[
    "claude",
    "codex",
    "jcode",
    "kcode",
    "opencode",
    "cursor",
    "aider",
];

/// Inspect the parent process and decide whether it is a known agent
/// harness. Returns `true` when the parent's `comm` lowercases to a
/// string containing any [`AGENT_TOKENS`] entry.
///
/// Falls back to `false` on:
/// - missing or unparseable `PPID`
/// - `ps` invocation failure (not installed, permission denied)
/// - non-UTF8 or empty `comm`
fn is_agent_process() -> bool {
    let ppid = env::var("PPID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);

    let output = process::Command::new("ps")
        .args(["-p", &ppid.to_string(), "-o", "comm="])
        .output();

    match output {
        Ok(o) => {
            let comm = String::from_utf8_lossy(&o.stdout).to_lowercase();
            AGENT_TOKENS.iter().any(|tok| comm.contains(tok))
        }
        Err(_) => false,
    }
}

fn main() {
    if is_agent_process() {
        process::exit(0);
    } else {
        process::exit(1);
    }
}
