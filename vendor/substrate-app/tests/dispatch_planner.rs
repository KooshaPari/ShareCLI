//! Pure planner tests — capability selection, explicit override, session defaults.

use engine_spec::TaskSpec;
use substrate_app::{DispatchPlanner, EngineCandidate, PlanRequest, SessionMode};
use substrate_core::domain::EngineCapabilities;

fn forge() -> EngineCandidate {
    EngineCandidate {
        name: "forge".into(),
        capabilities: EngineCapabilities {
            supports_resume: true,
            supports_subagents: true,
            supports_mcp_import: false,
        },
    }
}

fn claude() -> EngineCandidate {
    EngineCandidate {
        name: "claude".into(),
        capabilities: EngineCapabilities {
            supports_resume: true,
            supports_subagents: true,
            supports_mcp_import: false,
        },
    }
}

fn codex() -> EngineCandidate {
    EngineCandidate {
        name: "codex".into(),
        capabilities: EngineCapabilities {
            supports_resume: false,
            supports_subagents: true,
            supports_mcp_import: false,
        },
    }
}

fn agentapi() -> EngineCandidate {
    EngineCandidate {
        name: "agentapi".into(),
        capabilities: EngineCapabilities {
            supports_resume: false,
            supports_subagents: false,
            supports_mcp_import: false,
        },
    }
}

fn plan<'a>(
    spec: &'a TaskSpec,
    engines: &'a [EngineCandidate],
    explicit: Option<&str>,
    mode: Option<SessionMode>,
    routing: Option<&str>,
) -> substrate_app::DispatchPlan {
    DispatchPlanner::plan(&PlanRequest {
        spec,
        engines,
        explicit_engine: explicit,
        session_mode: mode,
        routing_engine: routing,
    })
    .unwrap()
}

#[test]
fn explicit_engine_is_honored() {
    let spec = TaskSpec::new("hi", "/tmp");
    let engines = vec![forge(), agentapi()];
    let p = plan(&spec, &engines, Some("agentapi"), None, None);
    assert_eq!(p.engine, "agentapi");
}

#[test]
fn resume_picks_supports_resume_engine() {
    let spec = TaskSpec {
        prompt: "continue".into(),
        cwd: "/tmp".into(),
        agent: None,
        resume: Some("conv-abc".into()),
    };
    let engines = vec![agentapi(), codex(), claude()];
    let p = plan(&spec, &engines, None, None, None);
    assert_eq!(p.engine, "claude");
}

#[test]
fn subagents_picks_supports_subagents_engine() {
    let spec = TaskSpec::new("delegate", "/tmp").with_agent("subagent");
    let engines = vec![agentapi(), forge()];
    let p = plan(&spec, &engines, None, None, None);
    assert_eq!(p.engine, "forge");
}

#[test]
fn routing_preference_used_when_capable() {
    let spec = TaskSpec::new("hi", "/tmp");
    let engines = vec![forge(), claude(), codex()];
    let p = plan(&spec, &engines, None, None, Some("codex"));
    assert_eq!(p.engine, "codex");
}

#[test]
fn session_mode_defaults_to_foreground() {
    let spec = TaskSpec::new("hi", "/tmp");
    let engines = vec![forge()];
    let p = plan(&spec, &engines, None, None, None);
    assert_eq!(p.session_mode, SessionMode::Foreground);
}

#[test]
fn session_mode_override_is_respected() {
    let spec = TaskSpec::new("hi", "/tmp");
    let engines = vec![forge()];
    let p = plan(&spec, &engines, None, Some(SessionMode::Background), None);
    assert_eq!(p.session_mode, SessionMode::Background);
}

#[test]
fn deterministic_capability_fallback_sorts_by_name() {
    let spec = TaskSpec::new("hi", "/tmp");
    let engines = vec![forge(), claude()];
    let p = plan(&spec, &engines, None, None, None);
    assert_eq!(p.engine, "claude");
}
