// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-001
//!
//! Integration tests for the value-level public API of `sharecli_core`.
//!
//! These tests focus on pure value construction, conversion, and helper
//! functions that other crates depend on. Process-spawning and FUSE mount
//! paths are exercised by the unit tests inside `lib.rs` (they require
//! `tempfile` + `tokio::test_util` and live next to the implementation).
//!
//! Mirrors the test style of `crates/sharecli-ipc/tests/handler_dispatch.rs`.

use std::path::{Path, PathBuf};

use sharecli_core::{
    combine_thermal_agent_decision, fuse_session_id_for_command_key, SpawnOutcome, SpawnRequest,
    ThermalDecision,
};
use sharecli_fleet::AgentContentionTier;
use sharecli_ipc::{command_key, CachedResult, CommandKey, QueuePriority};

// ---------------------------------------------------------------------------
// SpawnRequest constructors
// ---------------------------------------------------------------------------

#[test]
fn spawn_request_new_defaults_to_normal_priority() {
    let req =
        SpawnRequest::new(vec!["cargo".into(), "build".into()], PathBuf::from("/repo"), vec![]);
    assert_eq!(req.argv, vec!["cargo", "build"]);
    assert_eq!(req.cwd, PathBuf::from("/repo"));
    assert!(req.env.is_empty());
    assert_eq!(req.queue_priority, QueuePriority::Normal);
}

#[test]
fn spawn_request_with_queue_priority_overrides_normal() {
    let req = SpawnRequest::new(vec!["echo".into()], PathBuf::from("."), vec![])
        .with_queue_priority(QueuePriority::High);
    assert_eq!(req.queue_priority, QueuePriority::High);
}

#[test]
fn spawn_request_with_queue_priority_can_downgrade_to_low() {
    let req = SpawnRequest::new(vec!["echo".into()], PathBuf::from("."), vec![])
        .with_queue_priority(QueuePriority::High)
        .with_queue_priority(QueuePriority::Low);
    assert_eq!(req.queue_priority, QueuePriority::Low);
}

#[test]
fn spawn_request_from_operator_uses_normal_when_no_rule_priority() {
    let req = SpawnRequest::from_operator(
        vec!["make".into()],
        PathBuf::from("/build"),
        vec![("CC".into(), "clang".into())],
        None,
    );
    assert_eq!(req.argv, vec!["make"]);
    assert_eq!(req.env, vec![("CC".into(), "clang".into())]);
    assert_eq!(req.queue_priority, QueuePriority::Normal);
}

// ---------------------------------------------------------------------------
// SpawnOutcome helpers
// ---------------------------------------------------------------------------

fn make_outcome_with_fuse() -> SpawnOutcome {
    SpawnOutcome {
        exit_code: 0,
        stdout: vec![],
        stderr: vec![],
        from_cache: false,
        resource_watch: Default::default(),
        detected_agent: None,
        fuse_session_id: Some("hv-0123456789abcdef".into()),
        fuse_backing: Some(PathBuf::from("/repo")),
        fuse_mountpoint: Some(PathBuf::from("/tmp/sharecli-mount")),
    }
}

fn make_outcome_without_fuse() -> SpawnOutcome {
    SpawnOutcome {
        exit_code: 0,
        stdout: vec![],
        stderr: vec![],
        from_cache: false,
        resource_watch: Default::default(),
        detected_agent: None,
        fuse_session_id: None,
        fuse_backing: None,
        fuse_mountpoint: None,
    }
}

#[test]
fn spawn_outcome_agent_family_is_none_when_no_detected_agent() {
    let outcome = make_outcome_without_fuse();
    assert_eq!(outcome.agent_family(), None);
}

#[test]
fn spawn_outcome_fuse_intercept_active_is_false_without_session() {
    let outcome = make_outcome_without_fuse();
    assert!(!outcome.fuse_intercept_active());
}

#[test]
fn spawn_outcome_fuse_intercept_active_is_true_with_session() {
    let outcome = make_outcome_with_fuse();
    assert!(outcome.fuse_intercept_active());
}

#[test]
fn spawn_outcome_remap_fuse_path_returns_none_without_intercept() {
    let outcome = make_outcome_without_fuse();
    assert_eq!(outcome.remap_fuse_path(Path::new("/anywhere")), None);
}

// ---------------------------------------------------------------------------
// CachedResult ↔ SpawnOutcome conversions
// ---------------------------------------------------------------------------

#[test]
fn cached_result_into_spawn_outcome_sets_from_cache_true() {
    let cached =
        CachedResult { exit_code: 42, stdout: b"hello".to_vec(), stderr: b"warn".to_vec() };
    let outcome: SpawnOutcome = cached.into();
    assert_eq!(outcome.exit_code, 42);
    assert_eq!(outcome.stdout, b"hello");
    assert_eq!(outcome.stderr, b"warn");
    assert!(outcome.from_cache, "From<CachedResult> must set from_cache=true");
    assert!(outcome.detected_agent.is_none());
    assert!(outcome.fuse_session_id.is_none());
}

#[test]
fn spawn_outcome_into_cached_result_preserves_output() {
    let outcome = SpawnOutcome {
        exit_code: 7,
        stdout: b"out".to_vec(),
        stderr: b"err".to_vec(),
        from_cache: false,
        resource_watch: Default::default(),
        detected_agent: None,
        fuse_session_id: None,
        fuse_backing: None,
        fuse_mountpoint: None,
    };
    let cached: CachedResult = outcome.into();
    assert_eq!(cached.exit_code, 7);
    assert_eq!(cached.stdout, b"out");
    assert_eq!(cached.stderr, b"err");
}

// ---------------------------------------------------------------------------
// ThermalDecision
// ---------------------------------------------------------------------------

#[test]
fn thermal_decision_is_copy_and_equality_distinguishes_variants() {
    let allow = ThermalDecision::Allow;
    let copied = allow;
    assert_eq!(copied, allow);
    assert_ne!(ThermalDecision::Allow, ThermalDecision::Warn);
    assert_ne!(ThermalDecision::Warn, ThermalDecision::Refuse);
    assert_ne!(ThermalDecision::Allow, ThermalDecision::Refuse);
}

#[test]
fn combine_thermal_agent_decision_allow_plus_ok_is_allow() {
    assert_eq!(
        combine_thermal_agent_decision(ThermalDecision::Allow, AgentContentionTier::Ok),
        ThermalDecision::Allow
    );
}

#[test]
fn combine_thermal_agent_decision_warn_stays_warn() {
    assert_eq!(
        combine_thermal_agent_decision(ThermalDecision::Warn, AgentContentionTier::Ok),
        ThermalDecision::Warn
    );
}

#[test]
fn combine_thermal_agent_decision_thermal_refuse_wins_over_agent_ok() {
    assert_eq!(
        combine_thermal_agent_decision(ThermalDecision::Refuse, AgentContentionTier::Ok),
        ThermalDecision::Refuse
    );
}

#[test]
fn combine_thermal_agent_decision_agent_refuse_overrides_allow() {
    assert_eq!(
        combine_thermal_agent_decision(ThermalDecision::Allow, AgentContentionTier::Refuse),
        ThermalDecision::Refuse
    );
}

#[test]
fn combine_thermal_agent_decision_agent_warn_promotes_allow_to_warn() {
    assert_eq!(
        combine_thermal_agent_decision(ThermalDecision::Allow, AgentContentionTier::Warn),
        ThermalDecision::Warn
    );
}

// ---------------------------------------------------------------------------
// fuse_session_id_for_command_key
// ---------------------------------------------------------------------------

#[test]
fn fuse_session_id_uses_short_key_in_full() {
    let key = CommandKey("abc".into());
    let id = fuse_session_id_for_command_key(&key);
    assert_eq!(id, "hv-abc");
}

#[test]
fn fuse_session_id_truncates_long_key_to_16_chars() {
    let key = CommandKey("0123456789abcdef12345678".into());
    let id = fuse_session_id_for_command_key(&key);
    assert_eq!(id, "hv-0123456789abcdef");
    assert_eq!(id.len(), 19);
}

#[test]
fn fuse_session_id_is_stable_across_calls() {
    let key = command_key(&["cargo".into(), "build".into()], Path::new("/repo"), &[]);
    let a = fuse_session_id_for_command_key(&key);
    let b = fuse_session_id_for_command_key(&key);
    assert_eq!(a, b);
    assert!(a.starts_with("hv-"));
}
