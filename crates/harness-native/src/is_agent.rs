// SPDX-License-Identifier: Apache-2.0 OR MIT
//! harness-is-agent binary: detect if the parent (or any ancestor) is an agent.
//!
//! Detection contract: the orchestrator writes a sentinel file at
//! `${HARNESS_VAR}/agent_sessions/${PPID}` whose trimmed contents classify
//! the parent process. The producer convention (consumed elsewhere by
//! `dispatcher::get_agent_name`) is:
//!
//!   * `"human"`  -> a human operator (NOT an agent)
//!   * `""`       -> unset / unknown  (NOT an agent)
//!   * anything else non-empty -> an agent, optionally named (e.g. `"koala"`).
//!
//! This binary preserves the public contract that the dispatcher depends on:
//!
//!   * exit 0 -> the current process was spawned as (or by) an agent
//!   * exit 1 -> not an agent (regular CLI invocation, or sentinel missing)
//!
//! The detection runs once and is memoised via [`std::sync::OnceLock`] so that
//! repeated invocations from a single process pay no extra I/O cost beyond
//! the first call.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::OnceLock;

/// Pure detection helper — testable in isolation.
///
/// Returns `true` if `${harness_var}/agent_sessions/${ppid}` exists as a
/// regular file whose trimmed content is non-empty and not equal to
/// `"human"`. Any other outcome (missing env, missing file, permission
/// denied, empty content, or the literal `"human"`) returns `false`.
///
/// `harness_var` and `ppid` are `Option<&str>` to make the call ergonomic
/// for both production code (which forwards env vars verbatim) and unit
/// tests (which pass `Some("...")` for the temp dir and a known ppid).
pub fn detect_from_env(harness_var: Option<&str>, ppid: Option<&str>) -> bool {
    let (Some(root), Some(pid)) = (harness_var, ppid) else {
        return false;
    };
    if root.is_empty() || pid.is_empty() {
        return false;
    }
    let session_file = session_file(root, pid);
    match fs::read_to_string(&session_file) {
        Ok(content) => classify(&content),
        Err(_) => false,
    }
}

/// Map a sentinel file's contents to the is-agent boolean.
///
/// `true` for any non-empty, non-`"human"` content. Centralised so tests
/// can pin the policy without standing up a real temp dir.
pub fn classify(content: &str) -> bool {
    let trimmed = content.trim();
    !trimmed.is_empty() && trimmed != "human"
}

fn session_file(harness_var: &str, ppid: &str) -> PathBuf {
    Path::new(harness_var).join("agent_sessions").join(ppid)
}

/// Process-wide memoised result of [`is_agent`]. The first call performs
/// the sentinel read; subsequent calls are a single atomic load.
static CACHED: OnceLock<bool> = OnceLock::new();

/// Detect whether the current process was invoked as (or by) an agent.
///
/// Reads `HARNESS_VAR` and `PPID` from the environment, then forwards
/// them to [`detect_from_env`]. The result is computed exactly once per
/// process and cached for the lifetime of the program.
pub fn is_agent() -> bool {
    *CACHED.get_or_init(|| {
        detect_from_env(env::var("HARNESS_VAR").ok().as_deref(), env::var("PPID").ok().as_deref())
    })
}

fn main() {
    let exit_code = if is_agent() { 0 } else { 1 };
    process::exit(exit_code);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as std_fs;
    use tempfile::TempDir;

    /// Write `content` into `${tmp}/agent_sessions/${ppid}` and return
    /// the harness_var root the caller should pass to [`detect_from_env`].
    fn write_sentinel(tmp: &TempDir, ppid: &str, content: &str) -> String {
        let sessions = tmp.path().join("agent_sessions");
        std_fs::create_dir_all(&sessions).expect("mkdir agent_sessions");
        std_fs::write(sessions.join(ppid), content).expect("write sentinel");
        tmp.path().to_string_lossy().into_owned()
    }

    #[test]
    fn classify_marks_non_human_non_empty_as_agent() {
        assert!(classify("agent"));
        assert!(classify("koala"));
        assert!(classify("  agent  "));
        assert!(classify("named-agent"));
    }

    #[test]
    fn classify_marks_human_and_empty_as_not_agent() {
        assert!(!classify("human"));
        assert!(!classify("  human  "));
        assert!(!classify(""));
        assert!(!classify("   "));
        assert!(!classify("\n"));
    }

    #[test]
    fn detect_returns_false_when_harness_var_missing() {
        assert!(!detect_from_env(None, Some("1234")));
    }

    #[test]
    fn detect_returns_false_when_ppid_missing() {
        assert!(!detect_from_env(Some("/tmp"), None));
    }

    #[test]
    fn detect_returns_false_when_sentinel_file_absent() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().to_string_lossy().into_owned();
        assert!(!detect_from_env(Some(&root), Some("9999")));
    }

    #[test]
    fn detect_returns_true_for_agent_marker() {
        let tmp = TempDir::new().expect("tempdir");
        let root = write_sentinel(&tmp, "1234", "agent");
        assert!(detect_from_env(Some(&root), Some("1234")));
    }

    #[test]
    fn detect_returns_true_for_named_agent_with_whitespace() {
        let tmp = TempDir::new().expect("tempdir");
        let root = write_sentinel(&tmp, "1234", "  koala\n");
        assert!(detect_from_env(Some(&root), Some("1234")));
    }

    #[test]
    fn detect_returns_false_for_human_marker() {
        let tmp = TempDir::new().expect("tempdir");
        let root = write_sentinel(&tmp, "1234", "human");
        assert!(!detect_from_env(Some(&root), Some("1234")));
    }

    #[test]
    fn detect_returns_false_for_empty_sentinel() {
        let tmp = TempDir::new().expect("tempdir");
        let root = write_sentinel(&tmp, "1234", "");
        assert!(!detect_from_env(Some(&root), Some("1234")));
    }

    #[test]
    fn detect_returns_false_for_empty_harness_var_or_ppid() {
        assert!(!detect_from_env(Some(""), Some("1")));
        assert!(!detect_from_env(Some("/tmp"), Some("")));
    }

    #[test]
    fn session_file_joins_path_components_safely() {
        let p = session_file("/var/harness", "42");
        assert_eq!(p, PathBuf::from("/var/harness/agent_sessions/42"));
    }

    /// Smoke-check the cached public entry point. We can't reliably set
    /// `HARNESS_VAR`/`PPID` from a test thread (they are process-global and
    /// other tests run in the same process), but we can at least confirm
    /// the function is callable, returns a `bool`, and is stable across
    /// repeated invocations.
    #[test]
    fn is_agent_is_stable_and_returns_a_bool() {
        let first = is_agent();
        let second = is_agent();
        assert_eq!(first, second, "is_agent must be memoised");
    }
}
