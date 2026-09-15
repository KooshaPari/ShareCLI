//! Agent process pattern detection (FR-006).
//!
//! Discovers known coding agents by process name / cmdline tokens.
//! Detection is observation-only — sharecli MUST NOT wrap or replace vendor
//! agent binaries as the primary integration path.
//!
//! Patterns are loaded from `~/.config/sharecli/agent_patterns.toml` at
//! startup. When the file is absent, compile-time defaults apply (backwards
//! compatible). Agents may also define system-tool patterns for build-process
//! classification.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Compile-time defaults (fallback when no TOML is loaded)
// ---------------------------------------------------------------------------

/// Known agent family ids returned by [`match_known_agent`].
///
/// This is the compile-time canonical list. Runtime-configurable patterns in
/// `agent_patterns.toml` extend this at startup without recompilation.
pub const KNOWN_AGENT_FAMILIES: &[&str] = &[
    "claude",
    "codex",
    "gemini",
    "cursor-agent",
    "aider",
    "amp",
    "goose",
    "forge",
    "jcode",
    "opencode",
];

/// Families whose short `comm` names collide with non-agent tooling — require cmdline fingerprints.
const AMBIGUOUS_FAMILIES: &[&str] = &["forge", "goose", "gemini", "cargo"];

/// Cmdline substrings that fingerprint each family (AC-006.11, AC-006.20).
///
/// Runtime-configurable patterns from `agent_patterns.toml` are merged with
/// this list at startup (see `load_runtime_patterns`).
pub const CMDLINE_FINGERPRINTS: &[(&str, &[&str])] = &[
    ("claude", &["claude", "claude-code", ".claude"]),
    ("codex", &["openai-codex", "@openai/codex", "codex-cli", "/bin/codex"]),
    ("gemini", &["gemini", "gemini-cli", "google-gemini"]),
    ("cursor-agent", &["cursor-agent", ".cursor", "cursor agent", "@cursor/agent"]),
    ("aider", &["aider", "aider-chat", ".aider"]),
    ("amp", &["amp", "amp-code", "sourcegraph/amp", "@sourcegraph/amp", ".amp"]),
    ("goose", &["goose", "block-goose", "goose-agent"]),
    ("forge", &["forge", ".forge", "forge conversation"]),
    ("jcode", &["jcode", ".jcode", "jcode-cli"]),
    ("opencode", &["opencode", ".opencode", "opencode-cli"]),
];

// ---------------------------------------------------------------------------
// TOML deserialization types
// ---------------------------------------------------------------------------

/// Top-level structure of `agent_patterns.toml`.
#[derive(Debug, Deserialize)]
struct AgentPatternsFile {
    #[serde(default)]
    agents: Vec<AgentPatternEntry>,
    #[serde(default)]
    system_tools: Vec<SystemToolEntry>,
}

/// Single `[[agents]]` table entry.
#[derive(Debug, Deserialize)]
struct AgentPatternEntry {
    family: String,
    comm_names: Vec<String>,
    cmdline_markers: Vec<String>,
    #[serde(default)]
    ambiguous: bool,
}

/// Single `[[system_tools]]` table entry.
#[derive(Debug, Deserialize)]
struct SystemToolEntry {
    name: String,
    comm_names: Vec<String>,
    cmdline_markers: Vec<String>,
    #[serde(default)]
    ambiguous: bool,
}

// ---------------------------------------------------------------------------
// Runtime pattern types (loaded into thread_local)
// ---------------------------------------------------------------------------

/// A runtime-configured agent detection pattern.
///
/// Loaded once at startup from `agent_patterns.toml`. The `family` string is
/// leaked via `Box::leak` to produce a `&'static str` — acceptable because
/// patterns are loaded once for the process lifetime.
#[derive(Debug, Clone)]
pub struct RuntimePattern {
    /// The raw family string (borrowed from leaked allocation).
    pub family: &'static str,
    /// Comm names that identify this agent family.
    pub comm_names: Vec<String>,
    /// Cmdline substrings that fingerprint this family.
    pub cmdline_markers: Vec<String>,
    /// Whether this family requires cmdline disambiguation.
    pub ambiguous: bool,
}

impl RuntimePattern {
    /// Returns `&'static str` for the family id (leaked at load time).
    pub fn family_str(&self) -> &'static str {
        self.family
    }
}

/// A runtime-configured system tool detection pattern.
#[derive(Debug, Clone)]
pub struct RuntimeSystemTool {
    /// Tool name (e.g. "rustc", "cargo").
    pub name: &'static str,
    /// Comm names that identify this tool.
    pub comm_names: Vec<String>,
    /// Cmdline substrings that fingerprint this tool.
    pub cmdline_markers: Vec<String>,
    /// Whether this tool requires cmdline disambiguation.
    pub ambiguous: bool,
}

// ---------------------------------------------------------------------------
// Thread-local runtime state
// ---------------------------------------------------------------------------

thread_local! {
    static RUNTIME_PATTERNS: RefCell<Vec<RuntimePattern>> = const { RefCell::new(Vec::new()) };
    static SYSTEM_TOOL_PATTERNS: RefCell<Vec<RuntimeSystemTool>> = const { RefCell::new(Vec::new()) };
    static PATTERNS_LOADED: RefCell<bool> = const { RefCell::new(false) };
}

// ---------------------------------------------------------------------------
// Loading functions
// ---------------------------------------------------------------------------

/// Default path: `~/.config/sharecli/agent_patterns.toml`
fn default_patterns_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sharecli")
        .join("agent_patterns.toml")
}

/// Load agent and system-tool patterns from the TOML file at `path`.
///
/// If the file does not exist, the thread-locals remain empty and
/// [`match_known_agent`] falls through to compile-time defaults.
/// If the file exists but fails to parse, a warning is logged and defaults
/// are used.
///
/// Safe to call multiple times — only the first call takes effect.
pub fn load_runtime_patterns(path: &Path) {
    PATTERNS_LOADED.with(|loaded| {
        if *loaded.borrow() {
            return;
        }
        *loaded.borrow_mut() = true;
    });

    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("agent_patterns: could not read {}: {e}", path.display());
            return;
        }
    };

    let file: AgentPatternsFile = match toml::from_str(&contents) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("agent_patterns: failed to parse {}: {e}", path.display());
            return;
        }
    };

    RUNTIME_PATTERNS.with(|patterns| {
        let mut guard = patterns.borrow_mut();
        guard.clear();
        for entry in file.agents {
            // Leak the family string once so we get &'static str.
            let leaked: &'static str = Box::leak(entry.family.into_boxed_str());
            guard.push(RuntimePattern {
                family: leaked,
                comm_names: entry.comm_names,
                cmdline_markers: entry.cmdline_markers,
                ambiguous: entry.ambiguous,
            });
        }
    });

    SYSTEM_TOOL_PATTERNS.with(|patterns| {
        let mut guard = patterns.borrow_mut();
        guard.clear();
        for entry in file.system_tools {
            let leaked: &'static str = Box::leak(entry.name.into_boxed_str());
            guard.push(RuntimeSystemTool {
                name: leaked,
                comm_names: entry.comm_names,
                cmdline_markers: entry.cmdline_markers,
                ambiguous: entry.ambiguous,
            });
        }
    });

    tracing::debug!(
        agents = RUNTIME_PATTERNS.with(|p| p.borrow().len()),
        tools = SYSTEM_TOOL_PATTERNS.with(|p| p.borrow().len()),
        "agent_patterns: loaded runtime patterns from {}",
        path.display()
    );
}

/// Convenience: load from the default path (`~/.config/sharecli/agent_patterns.toml`).
pub fn load_default_runtime_patterns() {
    load_runtime_patterns(&default_patterns_path());
}

/// Load patterns from a specific path, falling back to the default path if not provided.
pub fn load_patterns(override_path: Option<&Path>) {
    match override_path {
        Some(p) => load_runtime_patterns(p),
        None => load_runtime_patterns(&default_patterns_path()),
    }
}

/// Clear all loaded runtime patterns (useful for tests).
pub fn clear_runtime_patterns() {
    RUNTIME_PATTERNS.with(|p| p.borrow_mut().clear());
    SYSTEM_TOOL_PATTERNS.with(|p| p.borrow_mut().clear());
    PATTERNS_LOADED.with(|l| *l.borrow_mut() = false);
}

// ---------------------------------------------------------------------------
// Agent matching
// ---------------------------------------------------------------------------

/// Match a process `comm` (short name) and optional cmdline tokens against the
/// known-agent pattern registry.
///
/// Returns the canonical family id when a pattern matches; `None` otherwise.
/// Matching never implies wrapping the agent binary.
pub fn match_known_agent(comm: &str, cmdline: &[impl AsRef<str>]) -> Option<&'static str> {
    let comm_l = comm.to_ascii_lowercase();
    if let Some(family) = match_token(&comm_l) {
        let exact_comm = is_exact_comm_basename(&comm_l, family);
        if family_allowed(family, cmdline, exact_comm) {
            return Some(family);
        }
    }
    for arg in cmdline {
        let t = arg.as_ref().to_ascii_lowercase();
        let base = t.rsplit('/').next().unwrap_or(&t);
        let base = base.rsplit('\\').next().unwrap_or(base);
        if let Some(family) = match_token(base) {
            if family_allowed(family, cmdline, false) {
                return Some(family);
            }
        }
        if let Some(family) = match_token(&t) {
            if family_allowed(family, cmdline, false) {
                return Some(family);
            }
        }
    }
    match_fingerprint_only(cmdline)
}

/// Match argv fingerprints when comm/token heuristics did not resolve (AC-006.20).
fn match_fingerprint_only(cmdline: &[impl AsRef<str>]) -> Option<&'static str> {
    // Compile-time fingerprints
    if let Some(family) = CMDLINE_FINGERPRINTS.iter().find_map(|(family, _)| {
        if cmdline_has_fingerprint(family, cmdline) {
            Some(*family)
        } else {
            None
        }
    }) {
        return Some(family);
    }

    // Runtime fingerprint patterns
    RUNTIME_PATTERNS.with(|patterns| {
        for pat in patterns.borrow().iter() {
            if pat.ambiguous && runtime_cmdline_has_marker(cmdline, &pat.cmdline_markers) {
                return Some(pat.family_str());
            }
        }
        None
    })
}

fn family_allowed(
    family: &'static str,
    cmdline: &[impl AsRef<str>],
    exact_comm_basename: bool,
) -> bool {
    // Check compile-time ambiguous list first
    if AMBIGUOUS_FAMILIES.contains(&family) {
        if exact_comm_basename && cmdline.is_empty() {
            return true;
        }
        return cmdline_has_fingerprint(family, cmdline);
    }

    // Check runtime ambiguous list
    let runtime_result = RUNTIME_PATTERNS.with(|patterns| -> Option<bool> {
        for pat in patterns.borrow().iter() {
            if pat.family == family && pat.ambiguous {
                if exact_comm_basename && cmdline.is_empty() {
                    return Some(true);
                }
                return Some(runtime_cmdline_has_marker(cmdline, &pat.cmdline_markers));
            }
        }
        None
    });

    runtime_result.unwrap_or(true)
}

fn is_exact_comm_basename(comm: &str, family: &str) -> bool {
    comm == family
}

fn cmdline_has_fingerprint(family: &str, cmdline: &[impl AsRef<str>]) -> bool {
    let Some(markers) = CMDLINE_FINGERPRINTS.iter().find(|(f, _)| *f == family).map(|(_, m)| *m)
    else {
        return false;
    };
    for arg in cmdline {
        let t = arg.as_ref().to_ascii_lowercase();
        if markers.iter().any(|marker| t.contains(marker)) {
            return true;
        }
    }
    false
}

/// Check if any runtime cmdline marker appears in the command line.
fn runtime_cmdline_has_marker(cmdline: &[impl AsRef<str>], markers: &[String]) -> bool {
    for arg in cmdline {
        let t = arg.as_ref().to_ascii_lowercase();
        if markers.iter().any(|marker| t.contains(marker.as_str())) {
            return true;
        }
    }
    false
}

fn match_token(token: &str) -> Option<&'static str> {
    // --- Compile-time patterns ---
    if token == "claude" || token.starts_with("claude-") || token.contains("claude-code") {
        return Some("claude");
    }
    if token == "codex" || token == "openai-codex" {
        return Some("codex");
    }
    if token == "gemini" || token.starts_with("gemini-") {
        return Some("gemini");
    }
    if token == "cursor-agent" || token == "cursor-agent.exe" {
        return Some("cursor-agent");
    }
    if token == "aider" {
        return Some("aider");
    }
    if token == "amp" || token == "amp-cli" {
        return Some("amp");
    }
    if token == "goose" || token.starts_with("goose-") {
        return Some("goose");
    }
    if token == "forge" || token.starts_with("forge-") {
        return Some("forge");
    }
    if token == "jcode" || token.starts_with("jcode-") || token == ".jcode" {
        return Some("jcode");
    }
    if token == "opencode" || token.starts_with("opencode-") || token == ".opencode" {
        return Some("opencode");
    }
    // --- Runtime patterns (loaded from agent_patterns.toml) ---
    RUNTIME_PATTERNS.with(|patterns| {
        for pat in patterns.borrow().iter() {
            if pat.family == token || pat.comm_names.iter().any(|name| token == name.as_str()) {
                return Some(pat.family_str());
            }
        }
        None
    })
}

// ---------------------------------------------------------------------------
// System tool matching
// ---------------------------------------------------------------------------

/// Match a process `comm` and cmdline against the runtime system-tool patterns.
///
/// Returns the tool name (e.g. `"rustc"`, `"cargo"`) when a pattern matches;
/// `None` otherwise. System tool detection is observation-only.
pub fn match_system_tool(comm: &str, cmdline: &[impl AsRef<str>]) -> Option<&'static str> {
    let comm_l = comm.to_ascii_lowercase();

    SYSTEM_TOOL_PATTERNS.with(|patterns| {
        for pat in patterns.borrow().iter() {
            let comm_match = pat.comm_names.iter().any(|name| comm_l == name.as_str());

            if comm_match {
                // For ambiguous tools, require a cmdline marker match.
                if pat.ambiguous {
                    if runtime_cmdline_has_marker(cmdline, &pat.cmdline_markers) {
                        return Some(pat.name);
                    }
                    // Exact comm with empty cmdline is a bare-name hit.
                    if cmdline.is_empty() {
                        return Some(pat.name);
                    }
                    continue;
                }
                return Some(pat.name);
            }

            // Check cmdline markers as a fallback.
            if runtime_cmdline_has_marker(cmdline, &pat.cmdline_markers) {
                return Some(pat.name);
            }
        }
        None
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Compile-time agent tests (unchanged) ---

    #[test]
    fn detects_claude_comm() {
        assert_eq!(match_known_agent("claude", &[] as &[&str]), Some("claude"));
    }

    #[test]
    fn detects_path_cmdline() {
        assert_eq!(match_known_agent("node", &["/usr/local/bin/claude"]), Some("claude"));
    }

    #[test]
    fn unknown_is_none() {
        assert_eq!(match_known_agent("bash", &["-c", "echo hi"]), None);
    }

    #[test]
    fn ambiguous_forge_without_fingerprint_is_none() {
        assert_eq!(
            match_known_agent("forge", &["build", "release"]),
            None,
            "non-agent forge tooling MUST NOT match without fingerprint"
        );
    }

    #[test]
    fn ambiguous_forge_with_fingerprint_matches() {
        assert_eq!(match_known_agent("forge", &["forge", "conversation", "list"]), Some("forge"));
    }

    #[test]
    fn ambiguous_gemini_bare_comm_matches() {
        assert_eq!(match_known_agent("gemini", &[] as &[&str]), Some("gemini"));
        assert_eq!(match_known_agent("gemini", &["gemini-cli", "chat"]), Some("gemini"));
    }

    // --- Runtime pattern tests ---

    /// Helper: load patterns from a TOML string into thread-local state.
    fn load_from_str(toml: &str) {
        let file: AgentPatternsFile = toml::from_str(toml).expect("valid TOML");
        RUNTIME_PATTERNS.with(|patterns| {
            let mut guard = patterns.borrow_mut();
            guard.clear();
            for entry in file.agents {
                let leaked: &'static str = Box::leak(entry.family.into_boxed_str());
                guard.push(RuntimePattern {
                    family: leaked,
                    comm_names: entry.comm_names,
                    cmdline_markers: entry.cmdline_markers,
                    ambiguous: entry.ambiguous,
                });
            }
        });
        SYSTEM_TOOL_PATTERNS.with(|patterns| {
            let mut guard = patterns.borrow_mut();
            guard.clear();
            for entry in file.system_tools {
                let leaked: &'static str = Box::leak(entry.name.into_boxed_str());
                guard.push(RuntimeSystemTool {
                    name: leaked,
                    comm_names: entry.comm_names,
                    cmdline_markers: entry.cmdline_markers,
                    ambiguous: entry.ambiguous,
                });
            }
        });
        PATTERNS_LOADED.with(|l| *l.borrow_mut() = true);
    }

    #[test]
    fn runtime_pattern_matches_custom_agent() {
        load_from_str(
            r#"
            [[agents]]
            family = "my-custom-agent"
            comm_names = ["mca"]
            cmdline_markers = ["mca", "my-custom-agent"]
            "#,
        );

        assert_eq!(match_known_agent("mca", &["mca", "run"]), Some("my-custom-agent"));
        // Also matches via cmdline marker.
        assert_eq!(
            match_known_agent("node", &["/usr/bin/my-custom-agent", "start"]),
            Some("my-custom-agent")
        );

        clear_runtime_patterns();
    }

    #[test]
    fn runtime_ambiguous_agent_requires_fingerprint() {
        load_from_str(
            r#"
            [[agents]]
            family = "my-ambiguous"
            comm_names = ["mytool"]
            cmdline_markers = ["mytool-agent", "--agent"]
            ambiguous = true
            "#,
        );

        // Bare comm without cmdline: should not match (ambiguous).
        assert_eq!(match_known_agent("mytool", &[] as &[&str]), None);

        // With fingerprint: matches.
        assert_eq!(
            match_known_agent("mytool", &["mytool", "--agent", "run"]),
            Some("my-ambiguous")
        );

        clear_runtime_patterns();
    }

    #[test]
    fn runtime_pattern_does_not_override_compile_time() {
        // Compile-time "claude" pattern must still work even when runtime
        // patterns are loaded.
        load_from_str(
            r#"
            [[agents]]
            family = "extra-agent"
            comm_names = ["extra"]
            cmdline_markers = ["extra"]
            "#,
        );

        // Compile-time match still works.
        assert_eq!(match_known_agent("claude", &[] as &[&str]), Some("claude"));
        // Runtime match also works.
        assert_eq!(match_known_agent("extra", &[] as &[&str]), Some("extra-agent"));

        clear_runtime_patterns();
    }

    #[test]
    fn match_system_tool_basic() {
        load_from_str(
            r#"
            [[system_tools]]
            name = "rustc"
            comm_names = ["rustc"]
            cmdline_markers = ["rustc", "rustc.exe"]
            "#,
        );

        assert_eq!(match_system_tool("rustc", &[] as &[&str]), Some("rustc"));
        assert_eq!(match_system_tool("bash", &["rustc", "check"]), Some("rustc"));

        clear_runtime_patterns();
    }

    #[test]
    fn match_system_tool_ambiguous() {
        load_from_str(
            r#"
            [[system_tools]]
            name = "cargo"
            comm_names = ["cargo"]
            cmdline_markers = ["cargo", "cargo.exe"]
            ambiguous = true
            "#,
        );

        // Ambiguous with empty cmdline: bare-name hit.
        assert_eq!(match_system_tool("cargo", &[] as &[&str]), Some("cargo"));
        // Ambiguous with marker in cmdline.
        assert_eq!(match_system_tool("bash", &["cargo", "build"]), Some("cargo"));

        clear_runtime_patterns();
    }

    #[test]
    fn match_system_tool_unknown_returns_none() {
        load_from_str(
            r#"
            [[system_tools]]
            name = "rustc"
            comm_names = ["rustc"]
            cmdline_markers = ["rustc"]
            "#,
        );

        assert_eq!(match_system_tool("vim", &[] as &[&str]), None);

        clear_runtime_patterns();
    }

    // --- Additional coverage tests ---

    #[test]
    fn detects_codex_comm() {
        assert_eq!(match_known_agent("codex", &[] as &[&str]), Some("codex"));
    }

    #[test]
    fn detects_codex_openai_prefix() {
        assert_eq!(match_known_agent("openai-codex", &[] as &[&str]), Some("codex"));
    }

    #[test]
    fn detects_aider_comm() {
        assert_eq!(match_known_agent("aider", &[] as &[&str]), Some("aider"));
    }

    #[test]
    fn detects_aider_cmdline() {
        assert_eq!(match_known_agent("python", &["aider", "run"]), Some("aider"));
    }

    #[test]
    fn detects_amp_comm() {
        assert_eq!(match_known_agent("amp", &[] as &[&str]), Some("amp"));
    }

    #[test]
    fn detects_amp_cli_comm() {
        assert_eq!(match_known_agent("amp-cli", &[] as &[&str]), Some("amp"));
    }

    #[test]
    fn detects_jcode_comm() {
        assert_eq!(match_known_agent("jcode", &[] as &[&str]), Some("jcode"));
    }

    #[test]
    fn detects_jcode_cli_comm() {
        assert_eq!(match_known_agent("jcode-cli", &[] as &[&str]), Some("jcode"));
    }

    #[test]
    fn detects_jcode_dotfile_cmdline() {
        assert_eq!(match_known_agent("node", &["/usr/bin/.jcode"]), Some("jcode"));
    }

    #[test]
    fn detects_opencode_comm() {
        assert_eq!(match_known_agent("opencode", &[] as &[&str]), Some("opencode"));
    }

    #[test]
    fn detects_opencode_cli_comm() {
        assert_eq!(match_known_agent("opencode-cli", &[] as &[&str]), Some("opencode"));
    }

    #[test]
    fn detects_opencode_dotfile_cmdline() {
        assert_eq!(match_known_agent("node", &["/usr/bin/.opencode"]), Some("opencode"));
    }

    #[test]
    fn detects_cursor_agent_exact() {
        assert_eq!(match_known_agent("cursor-agent", &[] as &[&str]), Some("cursor-agent"));
    }

    #[test]
    fn detects_cursor_agent_exe() {
        assert_eq!(match_known_agent("cursor-agent.exe", &[] as &[&str]), Some("cursor-agent"));
    }

    #[test]
    fn detects_cursor_cmdline_marker() {
        assert_eq!(
            match_known_agent("node", &["/usr/bin/cursor-agent", "run"]),
            Some("cursor-agent")
        );
    }

    #[test]
    fn detects_goose_comm() {
        assert_eq!(match_known_agent("goose", &[] as &[&str]), Some("goose"));
    }

    #[test]
    fn detects_goose_cmdline_marker() {
        assert_eq!(match_known_agent("goose", &["goose", "block-goose", "run"]), Some("goose"));
    }

    #[test]
    fn ambiguous_goose_without_fingerprint() {
        assert_eq!(match_known_agent("goose", &["build", "release"]), None);
    }

    #[test]
    fn detects_gemini_ambiguous_needs_fingerprint() {
        // "gemini" is ambiguous — bare comm with empty cmdline should NOT match.
        assert_eq!(match_known_agent("gemini", &[] as &[&str]), Some("gemini"));
        // But "gemini-cli" as bare comm IS ambiguous too — needs fingerprint.
        assert_eq!(match_known_agent("gemini-cli", &[] as &[&str]), None);
        // With a fingerprint, it matches.
        assert_eq!(match_known_agent("gemini", &["gemini-cli", "chat"]), Some("gemini"));
    }

    #[test]
    fn detects_claude_code_cmdline() {
        assert_eq!(match_known_agent("node", &["/usr/bin/claude-code", "run"]), Some("claude"));
    }

    #[test]
    fn detects_claude_dotfile_cmdline() {
        assert_eq!(match_known_agent("node", &["/home/user/.claude/config"]), Some("claude"));
    }

    #[test]
    fn detects_codex_cmdline_marker() {
        assert_eq!(match_known_agent("node", &["/usr/bin/openai-codex", "run"]), Some("codex"));
    }

    #[test]
    fn detects_codex_at_marker() {
        assert_eq!(
            match_known_agent("node", &["node_modules/@openai/codex/bin.js"]),
            Some("codex")
        );
    }

    #[test]
    fn detects_aider_dotfile_cmdline() {
        assert_eq!(
            match_known_agent("python", &["/usr/bin/python3", ".aider", "run"]),
            Some("aider")
        );
    }

    #[test]
    fn detects_amp_cmdline_marker() {
        assert_eq!(match_known_agent("node", &["@sourcegraph/amp", "serve"]), Some("amp"));
    }

    #[test]
    fn fingerprint_only_codex_cli() {
        assert_eq!(match_known_agent("bash", &["codex-cli", "start"]), Some("codex"));
    }

    #[test]
    fn fingerprint_only_cursor_dotfile() {
        assert_eq!(
            match_known_agent("bash", &["/home/user/.cursor/config.json"]),
            Some("cursor-agent")
        );
    }

    #[test]
    fn unknown_empty_cmdline() {
        assert_eq!(match_known_agent("ls", &[] as &[&str]), None);
    }

    #[test]
    fn load_patterns_none_uses_default() {
        clear_runtime_patterns();
        load_patterns(None);
        // Should not panic; defaults apply
        clear_runtime_patterns();
    }

    #[test]
    fn load_patterns_nonexistent_file() {
        clear_runtime_patterns();
        load_runtime_patterns(Path::new("/nonexistent/path/patterns.toml"));
        // Should not panic; falls through to defaults
        clear_runtime_patterns();
    }

    #[test]
    fn load_patterns_from_file() {
        use tempfile::NamedTempFile;

        let content = r#"
            [[agents]]
            family = "test-agent"
            comm_names = ["ta"]
            cmdline_markers = ["test-agent"]

            [[system_tools]]
            name = "test-tool"
            comm_names = ["tt"]
            cmdline_markers = ["test-tool"]
        "#;

        let tmp = NamedTempFile::new().expect("create temp file");
        std::fs::write(tmp.path(), content).expect("write temp file");

        clear_runtime_patterns();
        load_runtime_patterns(tmp.path());

        assert_eq!(match_known_agent("ta", &["ta", "run"]), Some("test-agent"));
        assert_eq!(match_system_tool("tt", &[] as &[&str]), Some("test-tool"));

        clear_runtime_patterns();
    }

    #[test]
    fn load_nonexistent_file_is_noop() {
        clear_runtime_patterns();
        load_runtime_patterns(Path::new("/nonexistent/path/agent_patterns.toml"));
        // Should still match compile-time patterns.
        assert_eq!(match_known_agent("claude", &[] as &[&str]), Some("claude"));
    }

    #[test]
    fn clear_runtime_patterns_resets_state() {
        load_from_str(
            r#"
            [[agents]]
            family = "ephemeral"
            comm_names = ["ep"]
            cmdline_markers = ["ephemeral"]
            "#,
        );
        assert_eq!(match_known_agent("ep", &[] as &[&str]), Some("ephemeral"));

        clear_runtime_patterns();
        // After clearing, the runtime agent is gone but compile-time still works.
        assert_eq!(match_known_agent("ep", &[] as &[&str]), None);
        assert_eq!(match_known_agent("claude", &[] as &[&str]), Some("claude"));
    }
}
