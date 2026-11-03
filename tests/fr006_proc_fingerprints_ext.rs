// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — extended cmdline fingerprints (codex, amp, aider, cursor-agent)
//! FR: FR-006
//!
//! AC-006.20 vendor wrapper argv and amp family markers

use sharecli_fleet::{match_known_agent, KNOWN_AGENT_FAMILIES};

/// FR-006 / AC-006.20 — amp is a known agent family.
#[test]
fn fr006_amp_in_known_families() {
    assert!(KNOWN_AGENT_FAMILIES.contains(&"amp"));
}

/// FR-006 / AC-006.20 — amp comm resolves directly.
#[test]
fn fr006_amp_comm_matches() {
    assert_eq!(match_known_agent("amp", &[] as &[&str]), Some("amp"));
}

/// FR-006 / AC-006.20 — amp via npx wrapper argv.
#[test]
fn fr006_amp_npx_wrapper_matches() {
    assert_eq!(match_known_agent("node", &["npx", "@sourcegraph/amp", "chat"]), Some("amp"));
}

/// FR-006 / AC-006.20 — codex via openai-codex package path.
#[test]
fn fr006_codex_npm_wrapper_matches() {
    assert_eq!(
        match_known_agent("node", &["/opt/node_modules/.bin/openai-codex", "run"]),
        Some("codex")
    );
}

/// FR-006 / AC-006.20 — unrelated codex-prefixed filename is not an agent.
#[test]
fn fr006_codex_prefixed_unrelated_path_rejected() {
    assert_eq!(
        match_known_agent("node", &["./codex-database-migration.js"]),
        None,
        "generic codex- prefixed paths MUST NOT match without fingerprint"
    );
}

/// FR-006 / AC-006.20 — aider via python module argv.
#[test]
fn fr006_aider_python_module_matches() {
    assert_eq!(match_known_agent("python3", &["-m", "aider"]), Some("aider"));
}

/// FR-006 / AC-006.20 — aider-chat fingerprint on node wrapper.
#[test]
fn fr006_aider_chat_wrapper_matches() {
    assert_eq!(match_known_agent("node", &["node_modules/aider-chat/dist/cli.js"]), Some("aider"));
}

/// FR-006 / AC-006.20 — cursor-agent via .cursor path segment.
#[test]
fn fr006_cursor_agent_dot_cursor_path_matches() {
    assert_eq!(
        match_known_agent("node", &["/Users/dev/.cursor/extensions/cursor-agent/dist/index.js"]),
        Some("cursor-agent")
    );
}

/// FR-006 / AC-006.20 — bare node without agent markers is not cursor-agent.
#[test]
fn fr006_node_without_fingerprint_not_cursor_agent() {
    assert_eq!(match_known_agent("node", &["server.js"]), None);
}
