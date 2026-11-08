// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Capability-aware surface discovery and durable observation recording.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    resolve_session, AgentSession, ObservationKind, SessionObservation, SessionStore,
    SurfaceAdapter, SurfaceCapabilities, SurfaceRecord,
};

/// Optional state-file lookup used to corroborate process argv evidence.
pub trait SessionStateProvider: Send + Sync {
    /// Return the harness session id recorded for a surface, when available.
    fn session_id(&self, surface: &SurfaceRecord, harness: &str) -> Result<Option<String>>;
}

/// State provider used when no harness-specific state file is available.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoStateProvider;

impl SessionStateProvider for NoStateProvider {
    fn session_id(&self, _surface: &SurfaceRecord, _harness: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

/// Small deterministic state provider useful for adapters and tests.
#[derive(Clone, Debug, Default)]
pub struct MapStateProvider {
    ids: HashMap<String, String>,
}

impl MapStateProvider {
    /// Build a provider keyed by surface id.
    pub fn new(ids: HashMap<String, String>) -> Self {
        Self { ids }
    }

    /// Insert or replace the session id associated with a surface.
    pub fn insert(&mut self, surface_id: impl Into<String>, session_id: impl Into<String>) {
        self.ids.insert(surface_id.into(), session_id.into());
    }
}

impl SessionStateProvider for MapStateProvider {
    fn session_id(&self, surface: &SurfaceRecord, _harness: &str) -> Result<Option<String>> {
        Ok(self.ids.get(&surface.id).cloned())
    }
}

/// One surface that could not be recorded without discarding the rest of a scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryFailure {
    /// Stable surface identifier.
    pub surface_id: String,
    /// Human-readable reason for the isolated failure.
    pub error: String,
}

/// Materialized result for one successfully recorded surface.
#[derive(Clone, Debug)]
pub struct DiscoveryResult {
    /// Stable surface identifier.
    pub surface_id: String,
    /// Known harness name, when process evidence identified one.
    pub harness: Option<String>,
    /// Resolved session id, when safe resume evidence was found.
    pub session_id: Option<String>,
    /// Observation transition derived from the existing ledger history.
    pub kind: ObservationKind,
    /// Capabilities captured for the surface.
    pub capabilities: SurfaceCapabilities,
    /// Session recipe, when resolution produced one.
    pub session: Option<AgentSession>,
}

/// Aggregate outcome for a single discovery pass.
#[derive(Clone, Debug, Default)]
pub struct DiscoveryReport {
    /// Number of surfaces returned by the adapter.
    pub scanned: usize,
    /// Number of observations appended to the durable ledger.
    pub recorded: usize,
    /// Isolated surface failures that did not abort the pass.
    pub failures: Vec<DiscoveryFailure>,
    /// Successfully recorded surface results.
    pub results: Vec<DiscoveryResult>,
}

/// Reusable scanner for native or managed surface adapters.
pub struct SurfaceObservationScanner<'a, A, S> {
    adapter: &'a A,
    state: &'a S,
    store: &'a SessionStore,
}

impl<'a, A, S> SurfaceObservationScanner<'a, A, S>
where
    A: SurfaceAdapter,
    S: SessionStateProvider,
{
    /// Construct a scanner over an adapter and durable ledger.
    pub fn new(adapter: &'a A, state: &'a S, store: &'a SessionStore) -> Self {
        Self { adapter, state, store }
    }

    /// Discover and append one observation per usable surface.
    pub fn scan(&self, observed_at: &str) -> Result<DiscoveryReport> {
        scan_and_record(self.adapter, self.state, self.store, observed_at)
    }
}

/// Discover surfaces, resolve only evidence-backed sessions, and append records.
pub fn scan_and_record<A, S>(
    adapter: &A,
    state: &S,
    store: &SessionStore,
    observed_at: &str,
) -> Result<DiscoveryReport>
where
    A: SurfaceAdapter,
    S: SessionStateProvider,
{
    if observed_at.trim().is_empty() {
        anyhow::bail!("observed_at must not be empty")
    }
    let surfaces = adapter.discover().context("discover terminal surfaces")?;
    let mut report = DiscoveryReport { scanned: surfaces.len(), ..Default::default() };
    for surface in surfaces {
        let surface_id = surface.id.clone();
        let capabilities = match adapter.capabilities(&surface) {
            Ok(value) => value,
            Err(error) => {
                report.failures.push(DiscoveryFailure { surface_id, error: error.to_string() });
                continue;
            }
        };
        let harness = surface_harness(&surface);
        let resolution = if let Some(harness) = harness.as_deref() {
            let state_id = state
                .session_id(&surface, harness)
                .with_context(|| format!("read session state for surface {}", surface.id))?;
            resolve_session(
                harness,
                surface_process_cwd(&surface),
                &surface_process_argv(&surface),
                state_id.as_deref(),
                None,
            )
        } else {
            crate::resolver::Resolution {
                session: None,
                confidence: crate::ResolutionConfidence::Unavailable,
                source: crate::EvidenceSource::Unavailable,
            }
        };
        let kind = if store.observations(Some(&surface.id))?.is_empty() {
            ObservationKind::Discovered
        } else {
            ObservationKind::Updated
        };
        let session = resolution.session;
        store.append_observation(&SessionObservation::new(
            observed_at,
            surface,
            session.clone(),
            capabilities.clone(),
            kind,
        ))?;
        report.recorded += 1;
        report.results.push(DiscoveryResult {
            surface_id,
            harness,
            session_id: session.as_ref().map(|value| value.session_id.clone()),
            kind,
            capabilities,
            session,
        });
    }
    Ok(report)
}

fn surface_harness(surface: &SurfaceRecord) -> Option<String> {
    let executable = surface.process.as_ref()?.argv.first()?;
    let name = Path::new(executable).file_name()?.to_str()?.to_ascii_lowercase();
    match name.as_str() {
        "forge" | "codex" | "opencode" | "kilo" | "cursor" | "cursor-agent" => Some(name),
        _ => None,
    }
}

fn surface_process_cwd(surface: &SurfaceRecord) -> std::path::PathBuf {
    surface
        .process
        .as_ref()
        .map(|process| process.cwd.clone())
        .unwrap_or_else(|| surface.cwd.clone())
}

fn surface_process_argv(surface: &SurfaceRecord) -> Vec<String> {
    surface.process.as_ref().map(|process| process.argv.clone()).unwrap_or_default()
}
