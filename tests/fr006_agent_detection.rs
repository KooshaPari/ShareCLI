// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — Agent Detection (proc pattern, no bin wrap)
//! FR: FR-006
//!
//! AC-006.1 pattern registry matches known agent names
//! AC-006.2 unknown processes do not match
//! AC-006.3 Hypervisor runs argv as-is (no vendor-bin wrap)

use std::sync::Arc;

use sharecli_core::{
    match_known_agent, FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest, ThermalDecision,
    KNOWN_AGENT_FAMILIES,
};
use tempfile::TempDir;

/// FR-006 / AC-006.1 — known agent names and path basenames resolve to families.
#[test]
fn fr006_pattern_registry_matches_known_names() {
    let cases = [
        ("claude", &[] as &[&str], Some("claude")),
        ("codex", &[], Some("codex")),
        ("gemini", &[], Some("gemini")),
        ("cursor-agent", &[], Some("cursor-agent")),
        ("aider", &[], Some("aider")),
        ("amp", &[], Some("amp")),
        ("node", &["/opt/homebrew/bin/claude"], Some("claude")),
        ("python3", &["-m", "aider"], Some("aider")),
    ];
    for (comm, cmdline, expect) in cases {
        assert_eq!(match_known_agent(comm, cmdline), expect, "comm={comm:?} cmdline={cmdline:?}");
    }
    assert!(KNOWN_AGENT_FAMILIES.contains(&"claude"));
}

/// FR-006 / AC-006.2 — ordinary shells / tools are not agents.
#[test]
fn fr006_unknown_process_is_not_agent() {
    assert_eq!(match_known_agent("bash", &["-lc", "ls"]), None);
    assert_eq!(match_known_agent("cargo", &["test"]), None);
    assert_eq!(match_known_agent("rustc", &[] as &[&str]), None);
}

/// FR-006 / AC-006.3 — hypervisor executes argv directly (observation path, no wrap).
#[tokio::test]
async fn fr006_hypervisor_runs_argv_as_is() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "fr006-no-wrap".to_string()];
    #[cfg(windows)]
    let argv =
        vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "fr006-no-wrap".to_string()];

    let outcome = hv
        .run(SpawnRequest {
            argv,
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect("spawn argv as-is");

    assert_eq!(outcome.exit_code, 0);
    assert!(!outcome.from_cache);
    // The caller can itself be a supported agent (for example, Codex running
    // this suite). Caller observation is orthogonal to the FR-006.3 contract:
    // the child command must still run exactly as supplied.
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(stdout.contains("fr006-no-wrap"), "stdout must reflect argv payload, got {stdout:?}");
}
