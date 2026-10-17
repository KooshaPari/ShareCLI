//! Speculation-policy reference for SC-WP-B06.
//!
//! Pure planning/accounting semantics. No execution provider consumes this yet.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeculationPolicy {
    pub max_extra_attempts: u32,
    pub max_extra_work_ms: u64,
    pub launch_delay_ms: u64,
    pub cancellation_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeculationCandidate {
    pub work_item_id: String,
    pub equivalence_proven: bool,
    pub side_effect_free: bool,
    pub estimated_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeculationDecision {
    Eligible { extra_attempts: u32, launch_delay_ms: u64 },
    Ineligible(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptOutcome {
    Selected,
    CancelledLoser,
    CompletedLoser,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeculationReceipt {
    pub selected_attempt_id: String,
    pub outcomes: Vec<(String, AttemptOutcome)>,
    pub wasted_work_ms: u64,
}

pub fn evaluate_speculation(
    candidate: &SpeculationCandidate,
    policy: &SpeculationPolicy,
) -> SpeculationDecision {
    if !candidate.equivalence_proven {
        return SpeculationDecision::Ineligible("equivalence is not proven".into());
    }
    if !candidate.side_effect_free {
        return SpeculationDecision::Ineligible("work is not side-effect free".into());
    }
    if policy.max_extra_attempts == 0 {
        return SpeculationDecision::Ineligible("extra-attempt budget is zero".into());
    }
    let Some(duration_ms) = candidate.estimated_duration_ms else {
        return SpeculationDecision::Ineligible("duration estimate is unknown".into());
    };
    if duration_ms > policy.max_extra_work_ms {
        return SpeculationDecision::Ineligible("estimated extra work exceeds budget".into());
    }

    SpeculationDecision::Eligible {
        extra_attempts: policy.max_extra_attempts,
        launch_delay_ms: policy.launch_delay_ms,
    }
}

pub fn account_speculation(
    selected_attempt_id: impl Into<String>,
    attempts: &[(String, AttemptOutcome, u64)],
) -> Result<SpeculationReceipt, String> {
    let selected_attempt_id = selected_attempt_id.into();
    let selected_count = attempts
        .iter()
        .filter(|(id, outcome, _)| id == &selected_attempt_id && *outcome == AttemptOutcome::Selected)
        .count();
    if selected_count != 1 {
        return Err("exactly one selected attempt must be identified".into());
    }

    let mut wasted_work_ms = 0u64;
    let mut outcomes = Vec::with_capacity(attempts.len());
    for (id, outcome, work_ms) in attempts {
        if *outcome != AttemptOutcome::Selected {
            wasted_work_ms = wasted_work_ms.saturating_add(*work_ms);
        }
        outcomes.push((id.clone(), outcome.clone()));
    }

    Ok(SpeculationReceipt { selected_attempt_id, outcomes, wasted_work_ms })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SpeculationPolicy {
        SpeculationPolicy {
            max_extra_attempts: 1,
            max_extra_work_ms: 1_000,
            launch_delay_ms: 50,
            cancellation_required: true,
        }
    }

    #[test]
    fn opaque_or_side_effecting_work_cannot_speculate() {
        let opaque = SpeculationCandidate {
            work_item_id: "a".into(),
            equivalence_proven: false,
            side_effect_free: true,
            estimated_duration_ms: Some(100),
        };
        assert!(matches!(
            evaluate_speculation(&opaque, &policy()),
            SpeculationDecision::Ineligible(_)
        ));

        let mut side_effecting = opaque;
        side_effecting.equivalence_proven = true;
        side_effecting.side_effect_free = false;
        assert!(matches!(
            evaluate_speculation(&side_effecting, &policy()),
            SpeculationDecision::Ineligible(_)
        ));
    }

    #[test]
    fn explicit_budget_and_delay_are_preserved() {
        let c = SpeculationCandidate {
            work_item_id: "safe".into(),
            equivalence_proven: true,
            side_effect_free: true,
            estimated_duration_ms: Some(500),
        };
        assert_eq!(
            evaluate_speculation(&c, &policy()),
            SpeculationDecision::Eligible { extra_attempts: 1, launch_delay_ms: 50 }
        );
    }

    #[test]
    fn unknown_or_over_budget_duration_fails_closed() {
        let mut c = SpeculationCandidate {
            work_item_id: "safe".into(),
            equivalence_proven: true,
            side_effect_free: true,
            estimated_duration_ms: None,
        };
        assert!(matches!(evaluate_speculation(&c, &policy()), SpeculationDecision::Ineligible(_)));
        c.estimated_duration_ms = Some(1_001);
        assert!(matches!(evaluate_speculation(&c, &policy()), SpeculationDecision::Ineligible(_)));
    }

    #[test]
    fn losing_attempt_work_is_never_erased_from_receipt() {
        let receipt = account_speculation(
            "attempt-a",
            &[
                ("attempt-a".into(), AttemptOutcome::Selected, 400),
                ("attempt-b".into(), AttemptOutcome::CancelledLoser, 120),
            ],
        )
        .unwrap();
        assert_eq!(receipt.wasted_work_ms, 120);
        assert_eq!(receipt.outcomes.len(), 2);
    }

    #[test]
    fn ambiguous_winner_selection_is_rejected() {
        assert!(account_speculation(
            "attempt-a",
            &[("attempt-b".into(), AttemptOutcome::CompletedLoser, 100)],
        )
        .is_err());
    }
}
