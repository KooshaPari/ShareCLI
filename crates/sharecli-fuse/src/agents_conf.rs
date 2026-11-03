// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Feb `agents.conf` parser — agent process-name substring patterns.
//!
//! Matching is substring-based and case-sensitive (parity with Downloads
//! `etc/agents.conf`). Blank lines and `#` comments are ignored.

use std::{fs, io, path::Path};

/// Parsed agent-name patterns from an `agents.conf` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentsConf {
    patterns: Vec<String>,
}

impl AgentsConf {
    /// Empty pattern set (no name matches).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Parse `agents.conf` text.
    pub fn parse(text: &str) -> Self {
        let patterns = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect();
        Self { patterns }
    }

    /// Load and parse a file.
    pub fn load(path: &Path) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        Ok(Self::parse(&text))
    }

    /// Patterns in file order.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// `true` when `name` contains any configured substring.
    pub fn matches_name(&self, name: &str) -> bool {
        self.patterns.iter().any(|p| name.contains(p.as_str()))
    }

    /// Validate that `agent` is a non-empty sanitized agent id (alnum / `_` / `-`).
    pub fn is_valid_agent_id(agent: &str) -> bool {
        !agent.is_empty()
            && agent.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }
}

/// Sanitize an agent id for use as a directory name under the CoW root.
///
/// Invalid characters become `_`. Empty input becomes `default`.
pub fn sanitize_agent_id(agent: &str) -> String {
    let trimmed = agent.trim();
    if trimmed.is_empty() {
        return "default".to_string();
    }
    let out: String = trimmed
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if out.is_empty() {
        "default".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-009 / AC-009.18 — agents.conf comments/blanks ignored; substring match.
    #[test]
    fn ac_009_18_agents_conf_parse_and_match() {
        let conf = AgentsConf::parse(
            r#"
# comment
claude
cursor

aider
"#,
        );
        assert_eq!(conf.patterns(), &["claude", "cursor", "aider"]);
        assert!(conf.matches_name("claude-code"));
        assert!(conf.matches_name("/usr/bin/cursor"));
        assert!(!conf.matches_name("vim"));
    }

    /// FR-009 / AC-009.18 — agent id sanitize.
    #[test]
    fn ac_009_18_sanitize_agent_id() {
        assert_eq!(sanitize_agent_id("agent-1"), "agent-1");
        assert_eq!(sanitize_agent_id("a/b"), "a_b");
        assert_eq!(sanitize_agent_id("  "), "default");
        assert!(AgentsConf::is_valid_agent_id("claude"));
        assert!(!AgentsConf::is_valid_agent_id("a/b"));
    }
}
