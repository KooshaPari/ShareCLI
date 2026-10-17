//! Additive mature-recovery subject and operation model.
//!
//! This does not replace the existing RecoveryExecutor. It preserves identity
//! needed by the future generation-safe public recovery path.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AgentSession, ResolutionConfidence, ResumeRecipe};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryProcessGeneration {
    pub provider: String,
    pub pid: u32,
    pub generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySubject {
    pub session_id: String,
    pub source_observation_ids: Vec<i64>,
    pub last_surface_id: Option<String>,
    pub last_process_generation: Option<RecoveryProcessGeneration>,
    pub ownership_claim_id: Option<String>,
    pub confidence: ResolutionConfidence,
    pub observed_at: Option<DateTime<Utc>>,
    pub recipe: ResumeRecipe,
}

impl RecoverySubject {
    pub fn from_legacy_session(session: &AgentSession) -> Self {
        Self {
            session_id: session.id.clone(),
            source_observation_ids: Vec::new(),
            last_surface_id: None,
            last_process_generation: None,
            ownership_claim_id: None,
            confidence: session.confidence.clone(),
            observed_at: None,
            recipe: session.resume.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryMode { DryRun, Execute }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryOperationState { Planned, Applying, Unknown, Realized, Failed, Skipped }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryOperation {
    pub id: String,
    pub subject_session_id: String,
    pub plan_revision: String,
    pub principal: String,
    pub mode: RecoveryMode,
    pub state: RecoveryOperationState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_session_does_not_gain_fabricated_generation() {
        let session = AgentSession::codex("legacy", "/tmp");
        let subject = RecoverySubject::from_legacy_session(&session);
        assert!(subject.last_process_generation.is_none());
        assert!(subject.ownership_claim_id.is_none());
        assert!(subject.source_observation_ids.is_empty());
    }
}
