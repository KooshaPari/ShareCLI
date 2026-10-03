//! Pressure-response policy reference for SC-WP-B06.
//!
//! Pure decision model only. A pressure observation never manufactures an
//! enforcement capability and this module performs no process side effects.

use crate::pressure::PressureSnapshot;
use crate::resource_control::ResourceControlCapabilities;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PressureResponse {
    Continue,
    DeferNewAdmissions,
    ReduceCpuWeight,
    PauseWork,
    CancelWork,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PressurePolicy {
    pub some_avg10_defer_threshold: f64,
    pub full_avg10_control_threshold: f64,
    pub cancellation_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureDecision {
    pub response: PressureResponse,
    pub enforceable: bool,
    pub reason: String,
}

pub fn decide_pressure_response(
    snapshot: &PressureSnapshot,
    controls: &ResourceControlCapabilities,
    policy: &PressurePolicy,
) -> PressureDecision {
    if !policy.some_avg10_defer_threshold.is_finite()
        || policy.some_avg10_defer_threshold < 0.0
        || !policy.full_avg10_control_threshold.is_finite()
        || policy.full_avg10_control_threshold < 0.0
    {
        return PressureDecision {
            response: PressureResponse::DeferNewAdmissions,
            enforceable: false,
            reason: "invalid pressure policy thresholds; fail closed at admission".into(),
        };
    }

    let full_avg10 = snapshot.full.as_ref().map(|w| w.avg10).unwrap_or(0.0);

    if full_avg10 >= policy.full_avg10_control_threshold {
        if controls.pause_resume {
            return PressureDecision {
                response: PressureResponse::PauseWork,
                enforceable: true,
                reason: "full-stall pressure crossed control threshold; pause/resume available".into(),
            };
        }
        if controls.cpu_weight {
            return PressureDecision {
                response: PressureResponse::ReduceCpuWeight,
                enforceable: true,
                reason: "full-stall pressure crossed control threshold; cpu weight available".into(),
            };
        }
        if policy.cancellation_allowed && controls.terminate {
            return PressureDecision {
                response: PressureResponse::CancelWork,
                enforceable: true,
                reason: "full-stall pressure crossed control threshold; explicit cancellation policy and terminate capability available".into(),
            };
        }
        return PressureDecision {
            response: PressureResponse::DeferNewAdmissions,
            enforceable: false,
            reason: "pressure observed but no matching enforcement capability; only admission policy can change".into(),
        };
    }

    if snapshot.some.avg10 >= policy.some_avg10_defer_threshold {
        return PressureDecision {
            response: PressureResponse::DeferNewAdmissions,
            enforceable: false,
            reason: "some-stall pressure crossed admission threshold".into(),
        };
    }

    PressureDecision {
        response: PressureResponse::Continue,
        enforceable: false,
        reason: "pressure below configured response thresholds".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pressure::StallWindow;

    fn snapshot(some: f64, full: Option<f64>) -> PressureSnapshot {
        PressureSnapshot {
            resource: "memory".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            some: StallWindow { avg10: some, avg60: 0.0, avg300: 0.0, total_micros: 1 },
            full: full.map(|avg10| StallWindow {
                avg10,
                avg60: 0.0,
                avg300: 0.0,
                total_micros: 1,
            }),
        }
    }

    fn policy() -> PressurePolicy {
        PressurePolicy {
            some_avg10_defer_threshold: 2.0,
            full_avg10_control_threshold: 1.0,
            cancellation_allowed: false,
        }
    }

    #[test]
    fn invalid_policy_thresholds_fail_closed_at_admission() {
        let controls = ResourceControlCapabilities::observe_only("linux-psi");
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            let mut p = policy();
            p.some_avg10_defer_threshold = invalid;
            let d = decide_pressure_response(&snapshot(0.0, Some(0.0)), &controls, &p);
            assert_eq!(d.response, PressureResponse::DeferNewAdmissions);
            assert!(!d.enforceable);

            let mut p = policy();
            p.full_avg10_control_threshold = invalid;
            let d = decide_pressure_response(&snapshot(0.0, Some(0.0)), &controls, &p);
            assert_eq!(d.response, PressureResponse::DeferNewAdmissions);
            assert!(!d.enforceable);
        }
    }

    #[test]
    fn observation_only_pressure_cannot_claim_enforcement() {
        let controls = ResourceControlCapabilities::observe_only("linux-psi");
        let d = decide_pressure_response(&snapshot(4.0, Some(2.0)), &controls, &policy());
        assert_eq!(d.response, PressureResponse::DeferNewAdmissions);
        assert!(!d.enforceable);
    }

    #[test]
    fn pause_is_selected_only_when_capability_exists() {
        let mut controls = ResourceControlCapabilities::observe_only("fixture");
        controls.pause_resume = true;
        let d = decide_pressure_response(&snapshot(4.0, Some(2.0)), &controls, &policy());
        assert_eq!(d.response, PressureResponse::PauseWork);
        assert!(d.enforceable);
    }

    #[test]
    fn termination_is_not_selected_without_explicit_cancellation_policy() {
        let mut controls = ResourceControlCapabilities::observe_only("fixture");
        controls.terminate = true;
        let d = decide_pressure_response(&snapshot(4.0, Some(2.0)), &controls, &policy());
        assert_eq!(d.response, PressureResponse::DeferNewAdmissions);
        assert!(!d.enforceable);
    }

    #[test]
    fn admission_deferral_needs_no_fake_process_control() {
        let controls = ResourceControlCapabilities::observe_only("linux-psi");
        let d = decide_pressure_response(&snapshot(3.0, Some(0.1)), &controls, &policy());
        assert_eq!(d.response, PressureResponse::DeferNewAdmissions);
        assert!(!d.enforceable);
    }
}
