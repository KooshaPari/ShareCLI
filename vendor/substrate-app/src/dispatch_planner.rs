//! Pure dispatch planning: select engine + session mode from a [`TaskSpec`],
//! available engines, and an optional routing preference.
//!
//! No adapter dependencies — argv is filled in by the composition root after
//! planning.

use engine_spec::TaskSpec;
use serde::{Deserialize, Serialize};
use substrate_core::{
    domain::EngineCapabilities,
    error::{Result, SubstrateError},
};

/// How the engine session is run once dispatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// Detached / non-blocking run (e.g. supervisor-managed).
    Background,
    /// Subprocess in the foreground (default for CLI engines).
    Foreground,
    /// In-process or embedded engine (e.g. fake-forge, HTTP adapter).
    InProcess,
}

impl SessionMode {
    /// Parse a CLI `--mode` value (`background`, `foreground`, `in_process`).
    pub fn parse_cli(s: &str) -> Option<Self> {
        match s {
            "background" => Some(SessionMode::Background),
            "foreground" => Some(SessionMode::Foreground),
            "in_process" | "in-process" => Some(SessionMode::InProcess),
            _ => None,
        }
    }
}

/// A registered engine the planner may choose among.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCandidate {
    /// Stable engine name (e.g. `"forge"`).
    pub name: String,
    /// Static capabilities advertised by the adapter.
    pub capabilities: EngineCapabilities,
}

/// Outcome of planning: which engine, how to run it, and the neutral spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchPlan {
    /// Chosen engine name.
    pub engine: String,
    /// How the session should be executed.
    pub session_mode: SessionMode,
    /// Provider-agnostic task description.
    pub spec: TaskSpec,
    /// Concrete process argv (`[program, arg0, …]`); populated by the driver.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
}

/// Inputs to [`DispatchPlanner::plan`].
#[derive(Debug, Clone, Copy)]
pub struct PlanRequest<'a> {
    /// What to run.
    pub spec: &'a TaskSpec,
    /// Engines available on this host.
    pub engines: &'a [EngineCandidate],
    /// Explicit override (`--engine`).
    pub explicit_engine: Option<&'a str>,
    /// Session mode override (`--mode`).
    pub session_mode: Option<SessionMode>,
    /// Preferred engine from a [`substrate_core::ports::RoutingPort`] (if wired).
    pub routing_engine: Option<&'a str>,
}

/// Deterministic engine + session-mode selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchPlanner;

impl DispatchPlanner {
    /// Produce a [`DispatchPlan`] for `input`. Pure given the inputs.
    pub fn plan(input: &PlanRequest<'_>) -> Result<DispatchPlan> {
        let engine = select_engine(
            input.spec,
            input.engines,
            input.explicit_engine,
            input.routing_engine,
        )?;
        let session_mode = input.session_mode.unwrap_or(SessionMode::Foreground);
        Ok(DispatchPlan {
            engine,
            session_mode,
            spec: input.spec.clone(),
            argv: Vec::new(),
        })
    }
}

fn spec_needs_resume(spec: &TaskSpec) -> bool {
    spec.resume.is_some()
}

fn spec_needs_subagents(spec: &TaskSpec) -> bool {
    spec.agent.as_deref() == Some("subagent")
}

fn engine_satisfies(spec: &TaskSpec, caps: &EngineCapabilities) -> bool {
    if spec_needs_resume(spec) && !caps.supports_resume {
        return false;
    }
    if spec_needs_subagents(spec) && !caps.supports_subagents {
        return false;
    }
    true
}

fn find_engine<'a>(name: &str, engines: &'a [EngineCandidate]) -> Option<&'a EngineCandidate> {
    engines.iter().find(|e| e.name == name)
}

fn select_engine(
    spec: &TaskSpec,
    engines: &[EngineCandidate],
    explicit_engine: Option<&str>,
    routing_engine: Option<&str>,
) -> Result<String> {
    if engines.is_empty() {
        return Err(SubstrateError::Routing(
            "no engines available for dispatch".to_string(),
        ));
    }

    if let Some(name) = explicit_engine {
        if find_engine(name, engines).is_none() {
            return Err(SubstrateError::Routing(format!(
                "explicit engine {name} is not available"
            )));
        }
        return Ok(name.to_string());
    }

    let mut capable: Vec<&EngineCandidate> = engines
        .iter()
        .filter(|e| engine_satisfies(spec, &e.capabilities))
        .collect();

    if capable.is_empty() {
        return Err(SubstrateError::Routing(
            "no engine satisfies task capability requirements".to_string(),
        ));
    }

    capable.sort_by(|a, b| a.name.cmp(&b.name));

    if let Some(pref) = routing_engine {
        if let Some(chosen) = capable.iter().find(|e| e.name == pref) {
            return Ok(chosen.name.clone());
        }
    }

    Ok(capable[0].name.clone())
}
