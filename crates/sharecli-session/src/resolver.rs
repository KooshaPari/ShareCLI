//! Harness/session evidence resolution without shell evaluation.

use std::path::{Path, PathBuf};

use crate::{AgentSession, ResolutionConfidence};

/// Evidence source used to resolve a harness session identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceSource {
    Adapter,
    StateFile,
    Argv,
    Unavailable,
}

/// Resolver output, including confidence and the safe resume recipe.
#[derive(Clone, Debug)]
pub struct Resolution {
    pub session: Option<AgentSession>,
    pub confidence: ResolutionConfidence,
    pub source: EvidenceSource,
}

/// Resolve a known harness using explicit state, then corroborated argv evidence.
pub fn resolve(
    harness: &str,
    cwd: impl Into<PathBuf>,
    argv: &[String],
    state_session_id: Option<&str>,
    adapter_session_id: Option<&str>,
) -> Resolution {
    let cwd = cwd.into();
    if let Some(id) = adapter_session_id.filter(|id| !id.is_empty()) {
        return exact_recipe(harness, id, cwd, EvidenceSource::Adapter);
    }
    if let Some(id) = state_session_id.filter(|id| !id.is_empty()) {
        let confidence = if argv_mentions_id(argv, id) {
            ResolutionConfidence::Corroborated
        } else {
            ResolutionConfidence::Exact
        };
        return recipe(harness, id, cwd, confidence, EvidenceSource::StateFile);
    }
    if let Some(id) = session_id_from_argv(harness, argv) {
        return recipe(harness, &id, cwd, ResolutionConfidence::Exact, EvidenceSource::Argv);
    }
    Resolution {
        session: None,
        confidence: ResolutionConfidence::Unavailable,
        source: EvidenceSource::Unavailable,
    }
}

fn exact_recipe(harness: &str, id: &str, cwd: PathBuf, source: EvidenceSource) -> Resolution {
    recipe(harness, id, cwd, ResolutionConfidence::Exact, source)
}

fn recipe(
    harness: &str,
    id: &str,
    cwd: PathBuf,
    confidence: ResolutionConfidence,
    source: EvidenceSource,
) -> Resolution {
    let session = match harness {
        "forge" => AgentSession::forge(id, cwd),
        "codex" => AgentSession::codex(id, cwd),
        "opencode" => AgentSession::opencode(id, cwd),
        "kilo" => AgentSession::kilo(id, cwd),
        "cursor" | "cursor-agent" => AgentSession::cursor(id, cwd),
        _ => {
            return Resolution {
                session: None,
                confidence: ResolutionConfidence::Unavailable,
                source: EvidenceSource::Unavailable,
            }
        }
    };
    let mut session = session;
    session.confidence = confidence;
    Resolution { session: Some(session), confidence, source }
}

fn argv_mentions_id(argv: &[String], id: &str) -> bool {
    argv.iter().any(|value| value == id)
}

fn session_id_from_argv(harness: &str, argv: &[String]) -> Option<String> {
    let names = match harness {
        "forge" => ["--conversation-id"].as_slice(),
        "codex" => ["resume"].as_slice(),
        "opencode" | "kilo" => ["--session"].as_slice(),
        "cursor" | "cursor-agent" => ["--resume"].as_slice(),
        _ => return None,
    };
    for (index, value) in argv.iter().enumerate() {
        if names.contains(&value.as_str()) {
            return argv.get(index + 1).filter(|id| !id.is_empty()).cloned();
        }
    }
    None
}

#[allow(dead_code)]
fn _cwd_is_valid(cwd: &Path) -> bool {
    cwd.is_absolute() && cwd.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_argv_resolves_exact_recipe() {
        let result =
            resolve("codex", "/tmp", &["codex".into(), "resume".into(), "id-1".into()], None, None);
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
        assert_eq!(result.session.unwrap().resume.argv, vec!["codex", "resume", "id-1"]);
    }

    #[test]
    fn state_and_argv_are_corroborated() {
        let argv = vec!["codex".into(), "resume".into(), "id-2".into()];
        let result = resolve("codex", "/tmp", &argv, Some("id-2"), None);
        assert_eq!(result.confidence, ResolutionConfidence::Corroborated);
    }

    #[test]
    fn ambiguous_process_never_yields_recipe() {
        let result = resolve("codex", "/tmp", &["codex".into()], None, None);
        assert!(result.session.is_none());
        assert_eq!(result.confidence, ResolutionConfidence::Unavailable);
    }

    // --- Additional coverage tests ---

    #[test]
    fn adapter_session_id_is_exact() {
        let result = resolve("codex", "/tmp", &[], None, Some("adapter-id"));
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
        assert_eq!(result.source, EvidenceSource::Adapter);
        let session = result.session.unwrap();
        assert_eq!(session.session_id, "adapter-id");
    }

    #[test]
    fn adapter_takes_precedence_over_state() {
        let result = resolve("codex", "/tmp", &[], Some("state-id"), Some("adapter-id"));
        assert_eq!(result.source, EvidenceSource::Adapter);
        assert_eq!(result.session.unwrap().session_id, "adapter-id");
    }

    #[test]
    fn empty_adapter_id_falls_through_to_state() {
        let result = resolve("codex", "/tmp", &[], Some("state-id"), Some(""));
        assert_eq!(result.source, EvidenceSource::StateFile);
    }

    #[test]
    fn forge_harness_resolves() {
        let result = resolve(
            "forge",
            "/tmp",
            &["forge".into(), "--conversation-id".into(), "fg-1".into()],
            None,
            None,
        );
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
        let session = result.session.unwrap();
        assert_eq!(session.harness, "forge");
        assert_eq!(session.resume.argv[1], "--conversation-id");
    }

    #[test]
    fn opencode_harness_resolves_from_argv() {
        let result = resolve(
            "opencode",
            "/tmp",
            &["opencode".into(), "--session".into(), "oc-1".into()],
            None,
            None,
        );
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
        assert_eq!(result.session.unwrap().harness, "opencode");
    }

    #[test]
    fn kilo_harness_resolves_from_argv() {
        let result = resolve(
            "kilo",
            "/tmp",
            &["kilo".into(), "--session".into(), "kl-1".into()],
            None,
            None,
        );
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
        assert_eq!(result.session.unwrap().harness, "kilo");
    }

    #[test]
    fn cursor_harness_resolves_from_argv() {
        let result = resolve(
            "cursor",
            "/tmp",
            &["cursor-agent".into(), "--resume".into(), "cr-1".into()],
            None,
            None,
        );
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
        assert_eq!(result.session.unwrap().harness, "cursor-agent");
    }

    #[test]
    fn cursor_agent_harness_resolves() {
        let result = resolve(
            "cursor-agent",
            "/tmp",
            &["cursor-agent".into(), "--resume".into(), "ca-1".into()],
            None,
            None,
        );
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
    }

    #[test]
    fn unknown_harness_returns_unavailable() {
        let result = resolve("unknown-tool", "/tmp", &[], None, None);
        assert!(result.session.is_none());
        assert_eq!(result.confidence, ResolutionConfidence::Unavailable);
        assert_eq!(result.source, EvidenceSource::Unavailable);
    }

    #[test]
    fn state_id_corroborated_by_argv() {
        let argv = vec!["codex".into(), "resume".into(), "xyz".into()];
        let result = resolve("codex", "/tmp", &argv, Some("xyz"), None);
        assert_eq!(result.confidence, ResolutionConfidence::Corroborated);
        assert_eq!(result.source, EvidenceSource::StateFile);
    }

    #[test]
    fn state_id_without_argv_match_is_exact() {
        let result = resolve("codex", "/tmp", &["codex".into(), "other".into()], Some("sid"), None);
        assert_eq!(result.confidence, ResolutionConfidence::Exact);
        assert_eq!(result.source, EvidenceSource::StateFile);
    }

    #[test]
    fn evidence_source_variants() {
        let a = EvidenceSource::Adapter;
        let b = EvidenceSource::StateFile;
        let c = EvidenceSource::Argv;
        let d = EvidenceSource::Unavailable;
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(c, d);
        assert_ne!(a, d);
    }
}
