//! FR: FR-001
//!
//! Integration tests for the public API of `sharecli_core::transaction_contract`.
//!
//! The module is intentionally not an OS effect broker or a durable ledger — it
//! exposes pure, actor-neutral admission predicates. These tests cover the
//! public surface (constructors, validation helpers, and error variants) and
//! the error-mapping rules that other crates depend on.
//!
//! Mirrors the test style of `crates/sharecli-ipc/tests/handler_dispatch.rs`.

use std::collections::BTreeSet;

use sharecli_core::transaction_contract::{
    can_retry_effect, validate_delegation, Actor, ActorKind, EffectOutcome, Grant, GuardError,
    ParentMembership, ReservationPool, SharedProducer,
};

// ---------------------------------------------------------------------------
// Actor / ActorKind
// ---------------------------------------------------------------------------

#[test]
fn actor_kind_constructs_with_expected_variants() {
    // Exhaustiveness check — both variants are constructible and copy-comparable.
    assert_eq!(ActorKind::Human, ActorKind::Human);
    assert_eq!(ActorKind::SoftwareAgent, ActorKind::SoftwareAgent);
    assert_ne!(ActorKind::Human, ActorKind::SoftwareAgent);
}

#[test]
fn actor_construction_preserves_id_and_kind() {
    let human = Actor { id: "human-a".into(), kind: ActorKind::Human };
    let agent = Actor { id: "agent-b".into(), kind: ActorKind::SoftwareAgent };
    assert_eq!(human.id, "human-a");
    assert_eq!(human.kind, ActorKind::Human);
    assert_eq!(agent.id, "agent-b");
    assert_eq!(agent.kind, ActorKind::SoftwareAgent);
}

// ---------------------------------------------------------------------------
// Grant + validate_delegation
// ---------------------------------------------------------------------------

fn make_grant(
    actor_id: &str,
    scope_id: &str,
    epoch: u64,
    operations: &[&str],
    max_units: u64,
) -> Grant {
    Grant {
        actor_id: actor_id.into(),
        scope_id: scope_id.into(),
        authority_epoch: epoch,
        operations: operations.iter().map(|s| (*s).to_string()).collect(),
        max_delegated_units: max_units,
    }
}

#[test]
fn validate_delegation_accepts_attenuated_descendant() {
    let parent = make_grant("human-a", "scope-a", 7, &["read", "build"], 4);
    let child = make_grant("agent-b", "scope-b", 7, &["read"], 2);
    assert!(validate_delegation(&parent, &child, true));
}

#[test]
fn validate_delegation_rejects_non_descendant_scope() {
    let parent = make_grant("human-a", "scope-a", 7, &["read"], 4);
    let mut child = parent.clone();
    child.scope_id = "scope-x".into();
    child.max_delegated_units = 2;
    assert!(!validate_delegation(&parent, &child, false));
}

#[test]
fn validate_delegation_rejects_authority_epoch_mismatch() {
    let parent = make_grant("human-a", "scope-a", 7, &["read"], 4);
    let mut child = parent.clone();
    child.authority_epoch = 8;
    child.max_delegated_units = 2;
    assert!(!validate_delegation(&parent, &child, true));
}

#[test]
fn validate_delegation_rejects_units_exceeding_parent() {
    let parent = make_grant("human-a", "scope-a", 7, &["read"], 4);
    let mut child = parent.clone();
    child.max_delegated_units = 8;
    assert!(!validate_delegation(&parent, &child, true));
}

#[test]
fn validate_delegation_rejects_operation_not_in_parent() {
    let parent = make_grant("human-a", "scope-a", 7, &["read"], 4);
    let mut child = parent.clone();
    child.operations.insert("publish".into());
    assert!(!validate_delegation(&parent, &child, true));
}

#[test]
fn validate_delegation_accepts_identical_grant_when_descendant() {
    let grant = make_grant("human-a", "scope-a", 7, &["read", "build"], 4);
    assert!(validate_delegation(&grant, &grant, true));
}

// ---------------------------------------------------------------------------
// ReservationPool
// ---------------------------------------------------------------------------

#[test]
fn reservation_pool_new_starts_empty() {
    let pool = ReservationPool::new(16);
    assert_eq!(pool.admitted(), 0);
}

#[test]
fn reservation_pool_admitted_sums_active_reservations() {
    let mut pool = ReservationPool::new(10);
    pool.reserve("a", 3).expect("reserve a");
    pool.reserve("b", 4).expect("reserve b");
    assert_eq!(pool.admitted(), 7);
}

#[test]
fn reservation_pool_rejects_duplicate_reservation_id() {
    let mut pool = ReservationPool::new(10);
    pool.reserve("a", 3).expect("first reserve");
    assert_eq!(pool.reserve("a", 2), Err(GuardError::Duplicate));
}

#[test]
fn reservation_pool_rejects_zero_units() {
    let mut pool = ReservationPool::new(10);
    assert_eq!(pool.reserve("a", 0), Err(GuardError::InvalidUnits));
}

#[test]
fn reservation_pool_rejects_capacity_overflow() {
    let mut pool = ReservationPool::new(4);
    pool.reserve("a", 3).expect("first reserve");
    assert_eq!(pool.reserve("b", 2), Err(GuardError::InsufficientCapacity));
}

#[test]
fn reservation_pool_release_refuses_when_not_reclaimed() {
    let mut pool = ReservationPool::new(4);
    pool.reserve("a", 2).expect("reserve");
    assert_eq!(pool.release("a", false), Err(GuardError::NotReclaimed));
    // Still admitted because the release did not happen.
    assert_eq!(pool.admitted(), 2);
}

#[test]
fn reservation_pool_release_refuses_unknown_reservation() {
    let mut pool = ReservationPool::new(4);
    assert_eq!(pool.release("missing", true), Err(GuardError::UnknownReservation));
}

#[test]
fn reservation_pool_release_frees_capacity_for_reuse() {
    let mut pool = ReservationPool::new(5);
    pool.reserve("a", 3).expect("reserve a");
    pool.release("a", true).expect("release a");
    assert_eq!(pool.admitted(), 0);
    pool.reserve("b", 5).expect("reserve b after release");
    assert_eq!(pool.admitted(), 5);
}

// ---------------------------------------------------------------------------
// SharedProducer
// ---------------------------------------------------------------------------

#[test]
fn shared_producer_new_starts_running_and_empty() {
    let producer = SharedProducer::new(3);
    assert_eq!(producer.generation, 3);
    assert_eq!(producer.state, sharecli_core::transaction_contract::ProducerState::Running);
    assert_eq!(producer.consumers(), 0);
}

#[test]
fn shared_producer_join_inserts_consumer_when_eligible() {
    let mut producer = SharedProducer::new(4);
    producer.join(4, "human-a", true, true, true).expect("join human");
    producer.join(4, "agent-b", true, true, true).expect("join agent");
    assert_eq!(producer.consumers(), 2);
}

#[test]
fn shared_producer_join_rejects_stale_generation() {
    let mut producer = SharedProducer::new(4);
    assert_eq!(
        producer.join(3, "human-a", true, true, true),
        Err(GuardError::StaleGeneration)
    );
}

#[test]
fn shared_producer_join_rejects_stopping_state() {
    let mut producer = SharedProducer::new(4);
    producer.join(4, "human-a", true, true, true).expect("first join");
    // Detach to drive the producer to Stopping (single consumer).
    let _ = producer.detach(4, "human-a").expect("detach");
    assert_eq!(
        producer.join(4, "late-agent", true, true, true),
        Err(GuardError::ProducerStopping)
    );
}

#[test]
fn shared_producer_join_rejects_when_share_not_eligible() {
    let mut producer = SharedProducer::new(4);
    // equivalent=false should fail even with share_allowed + output_authorized.
    assert_eq!(
        producer.join(4, "human", false, true, true),
        Err(GuardError::IneligibleShare)
    );
    assert_eq!(
        producer.join(4, "human", true, false, true),
        Err(GuardError::IneligibleShare)
    );
    assert_eq!(
        producer.join(4, "human", true, true, false),
        Err(GuardError::IneligibleShare)
    );
}

#[test]
fn shared_producer_detach_returns_false_when_others_remain() {
    let mut producer = SharedProducer::new(4);
    producer.join(4, "human", true, true, true).unwrap();
    producer.join(4, "agent", true, true, true).unwrap();
    assert!(!producer.detach(4, "human").unwrap());
    assert_eq!(producer.consumers(), 1);
}

#[test]
fn shared_producer_detach_returns_true_when_becoming_empty() {
    let mut producer = SharedProducer::new(4);
    producer.join(4, "human", true, true, true).unwrap();
    assert!(producer.detach(4, "human").unwrap());
    assert_eq!(
        producer.state,
        sharecli_core::transaction_contract::ProducerState::Stopping
    );
}

#[test]
fn shared_producer_detach_rejects_unknown_consumer() {
    let mut producer = SharedProducer::new(4);
    producer.join(4, "human", true, true, true).unwrap();
    assert_eq!(producer.detach(4, "ghost"), Err(GuardError::UnknownConsumer));
}

#[test]
fn shared_producer_detach_rejects_stale_generation() {
    let mut producer = SharedProducer::new(4);
    producer.join(4, "human", true, true, true).unwrap();
    assert_eq!(producer.detach(3, "human"), Err(GuardError::StaleGeneration));
}

// ---------------------------------------------------------------------------
// EffectOutcome + can_retry_effect
// ---------------------------------------------------------------------------

#[test]
fn can_retry_effect_confirmed_success_never_retries() {
    assert!(!can_retry_effect(
        EffectOutcome::ConfirmedSuccess,
        /* same_effect_key_and_payload = */ true,
        /* atomic_target_deduplication = */ true,
        /* proven_absent = */ true,
        /* duplication_explicitly_authorized = */ true,
    ));
}

#[test]
fn can_retry_effect_not_dispatched_always_retries() {
    assert!(can_retry_effect(EffectOutcome::NotDispatched, false, false, false, false));
    assert!(can_retry_effect(EffectOutcome::NotDispatched, true, true, true, true));
}

#[test]
fn can_retry_effect_confirmed_failure_requires_proof_or_auth() {
    assert!(!can_retry_effect(EffectOutcome::ConfirmedFailure, false, false, false, false));
    assert!(can_retry_effect(EffectOutcome::ConfirmedFailure, false, false, true, false));
    assert!(can_retry_effect(EffectOutcome::ConfirmedFailure, false, false, false, true));
}

#[test]
fn can_retry_effect_unknown_requires_dedup_or_explicit_proof() {
    assert!(!can_retry_effect(EffectOutcome::Unknown, false, false, false, false));
    assert!(can_retry_effect(EffectOutcome::Unknown, true, true, false, false));
    assert!(!can_retry_effect(EffectOutcome::Unknown, false, true, false, false));
    assert!(can_retry_effect(EffectOutcome::Unknown, false, false, true, false));
    assert!(can_retry_effect(EffectOutcome::Unknown, false, false, false, true));
}

// ---------------------------------------------------------------------------
// ParentMembership
// ---------------------------------------------------------------------------

#[test]
fn parent_membership_default_starts_empty_and_unsealed() {
    let parent = ParentMembership::default();
    assert_eq!(parent.version(), 0);
}

#[test]
fn parent_membership_add_child_bumps_version_and_inserts() {
    let mut parent = ParentMembership::default();
    parent.add_child("diagnose").unwrap();
    assert_eq!(parent.version(), 1);
    parent.add_child("verify").unwrap();
    assert_eq!(parent.version(), 2);
}

#[test]
fn parent_membership_add_child_rejects_duplicate_id() {
    let mut parent = ParentMembership::default();
    parent.add_child("diagnose").unwrap();
    assert_eq!(parent.add_child("diagnose"), Err(GuardError::Duplicate));
}

#[test]
fn parent_membership_add_child_rejects_after_seal() {
    let mut parent = ParentMembership::default();
    parent.add_child("a").unwrap();
    parent.mark_accepted("a").unwrap();
    parent.seal(parent.version()).unwrap();
    assert_eq!(parent.add_child("late"), Err(GuardError::StaleMembership));
}

#[test]
fn parent_membership_mark_accepted_rejects_unknown_id() {
    let mut parent = ParentMembership::default();
    assert_eq!(parent.mark_accepted("ghost"), Err(GuardError::UnknownReservation));
}

#[test]
fn parent_membership_seal_rejects_stale_expected_version() {
    let mut parent = ParentMembership::default();
    parent.add_child("a").unwrap();
    let observed = parent.version();
    parent.add_child("b").unwrap();
    parent.mark_accepted("a").unwrap();
    parent.mark_accepted("b").unwrap();
    assert_eq!(parent.seal(observed), Err(GuardError::StaleMembership));
}

#[test]
fn parent_membership_seal_rejects_unfinished_children() {
    let mut parent = ParentMembership::default();
    parent.add_child("a").unwrap();
    parent.add_child("b").unwrap();
    parent.mark_accepted("a").unwrap();
    assert_eq!(parent.seal(parent.version()), Err(GuardError::UnfinishedChild));
}

#[test]
fn parent_membership_seal_succeeds_when_all_children_accepted() {
    let mut parent = ParentMembership::default();
    parent.add_child("a").unwrap();
    parent.add_child("b").unwrap();
    parent.mark_accepted("a").unwrap();
    parent.mark_accepted("b").unwrap();
    parent.seal(parent.version()).expect("complete parent seals cleanly");
}

#[test]
fn parent_membership_cannot_be_sealed_twice() {
    let mut parent = ParentMembership::default();
    parent.add_child("a").unwrap();
    parent.mark_accepted("a").unwrap();
    parent.seal(parent.version()).expect("first seal");
    // Second seal — already sealed → StaleMembership.
    assert_eq!(parent.seal(parent.version()), Err(GuardError::StaleMembership));
}

// ---------------------------------------------------------------------------
// Cross-cutting: error variant equality + Clone for serialization ergonomics
// ---------------------------------------------------------------------------

#[test]
fn guard_error_is_copy_eq_and_distinct() {
    let a = GuardError::Duplicate;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(GuardError::Duplicate, GuardError::InvalidUnits);
    assert_ne!(GuardError::Overflow, GuardError::StaleGeneration);
}

#[test]
fn effect_outcome_is_copy_eq_and_distinct() {
    let a = EffectOutcome::Unknown;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(EffectOutcome::NotDispatched, EffectOutcome::ConfirmedSuccess);
}

#[test]
fn grant_operations_use_btreeset_for_deterministic_iteration() {
    // BTreeSet guarantees ascending iteration, so JSON serialization of the
    // operations field is byte-stable across runs (regression guard).
    let mut ops: BTreeSet<String> = BTreeSet::new();
    ops.insert("zeta".into());
    ops.insert("alpha".into());
    ops.insert("mu".into());
    let ordered: Vec<&String> = ops.iter().collect();
    let alpha = String::from("alpha");
    let mu = String::from("mu");
    let zeta = String::from("zeta");
    assert_eq!(ordered, vec![&alpha, &mu, &zeta]);
}