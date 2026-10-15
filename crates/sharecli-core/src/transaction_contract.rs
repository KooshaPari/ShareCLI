//! Pure, actor-neutral admission predicates for the proposed global transaction contract.
//!
//! This module is intentionally not an OS effect broker or a durable ledger.
//! Callers must authenticate actors, resolve scoped grants, serialize conflicting
//! decisions at an authority boundary, and enforce resulting effects at the target.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorKind {
    Human,
    SoftwareAgent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Actor {
    pub id: String,
    pub kind: ActorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grant {
    pub actor_id: String,
    pub scope_id: String,
    pub authority_epoch: u64,
    pub operations: BTreeSet<String>,
    pub max_delegated_units: u64,
}

/// Delegated rights are a subset of the parent grant; caller verifies that the
/// child scope actually descends from the parent scope in the authoritative tree.
pub fn validate_delegation(parent: &Grant, child: &Grant, child_is_descendant: bool) -> bool {
    child_is_descendant
        && parent.authority_epoch == child.authority_epoch
        && child.max_delegated_units <= parent.max_delegated_units
        && child.operations.is_subset(&parent.operations)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardError {
    Duplicate,
    InsufficientCapacity,
    Overflow,
    UnknownReservation,
    UnknownConsumer,
    InvalidUnits,
    NotReclaimed,
    ProducerStopping,
    StaleGeneration,
    IneligibleShare,
    StaleMembership,
    UnfinishedChild,
}

/// A physical reservation pool. Nested logical envelopes must not be summed
/// again as independent physical claims. Methods require serialized callers.
#[derive(Debug)]
pub struct ReservationPool {
    capacity: u64,
    active: BTreeMap<String, u64>,
}

impl ReservationPool {
    pub fn new(capacity: u64) -> Self {
        Self { capacity, active: BTreeMap::new() }
    }

    pub fn admitted(&self) -> u64 {
        self.active.values().copied().sum()
    }

    pub fn reserve(&mut self, reservation_id: &str, units: u64) -> Result<(), GuardError> {
        if self.active.contains_key(reservation_id) {
            return Err(GuardError::Duplicate);
        }
        if units == 0 {
            return Err(GuardError::InvalidUnits);
        }
        let used = self
            .active
            .values()
            .try_fold(0_u64, |sum, value| sum.checked_add(*value).ok_or(GuardError::Overflow))?;
        let next = used.checked_add(units).ok_or(GuardError::Overflow)?;
        if next > self.capacity {
            return Err(GuardError::InsufficientCapacity);
        }
        self.active.insert(reservation_id.to_owned(), units);
        Ok(())
    }

    /// Lease expiry or cancellation is not evidence of actual reclamation.
    pub fn release(&mut self, reservation_id: &str, reclaimed: bool) -> Result<(), GuardError> {
        if !self.active.contains_key(reservation_id) {
            return Err(GuardError::UnknownReservation);
        }
        if !reclaimed {
            return Err(GuardError::NotReclaimed);
        }
        self.active.remove(reservation_id);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducerState {
    Running,
    Stopping,
}

#[derive(Debug)]
pub struct SharedProducer {
    pub generation: u64,
    pub state: ProducerState,
    consumers: BTreeSet<String>,
}

/// In-flight joins and detaches must be serialized on this producer generation.
/// Authorization to execute is separate from the demand expressed by consumers.
impl SharedProducer {
    pub fn new(generation: u64) -> Self {
        Self { generation, state: ProducerState::Running, consumers: BTreeSet::new() }
    }

    pub fn join(
        &mut self,
        generation: u64,
        consumer_id: &str,
        equivalent: bool,
        share_allowed: bool,
        output_authorized: bool,
    ) -> Result<(), GuardError> {
        if generation != self.generation {
            return Err(GuardError::StaleGeneration);
        }
        if self.state != ProducerState::Running {
            return Err(GuardError::ProducerStopping);
        }
        if !(equivalent && share_allowed && output_authorized) {
            return Err(GuardError::IneligibleShare);
        }
        self.consumers.insert(consumer_id.to_owned());
        Ok(())
    }

    /// Returns true only when this generation became empty and is stopping.
    pub fn detach(&mut self, generation: u64, consumer_id: &str) -> Result<bool, GuardError> {
        if generation != self.generation {
            return Err(GuardError::StaleGeneration);
        }
        if !self.consumers.remove(consumer_id) {
            return Err(GuardError::UnknownConsumer);
        }
        if self.consumers.is_empty() && self.state == ProducerState::Running {
            self.state = ProducerState::Stopping;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn consumers(&self) -> usize {
        self.consumers.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectOutcome {
    NotDispatched,
    ConfirmedSuccess,
    ConfirmedFailure,
    Unknown,
}

/// A lost response is not evidence that a non-idempotent effect did not happen.
pub fn can_retry_effect(
    outcome: EffectOutcome,
    same_effect_key_and_payload: bool,
    atomic_target_deduplication: bool,
    proven_absent: bool,
    duplication_explicitly_authorized: bool,
) -> bool {
    match outcome {
        EffectOutcome::ConfirmedSuccess => false,
        EffectOutcome::NotDispatched => true,
        EffectOutcome::ConfirmedFailure | EffectOutcome::Unknown => {
            proven_absent
                || duplication_explicitly_authorized
                || (same_effect_key_and_payload && atomic_target_deduplication)
        }
    }
}

/// Required children are a versioned membership set; no late child may sneak
/// into an accepted revision between checking completion and sealing it.
#[derive(Debug, Default)]
pub struct ParentMembership {
    version: u64,
    sealed: bool,
    required: BTreeMap<String, bool>,
}

impl ParentMembership {
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn add_child(&mut self, id: &str) -> Result<(), GuardError> {
        if self.sealed {
            return Err(GuardError::StaleMembership);
        }
        if self.required.contains_key(id) {
            return Err(GuardError::Duplicate);
        }
        self.version = self.version.checked_add(1).ok_or(GuardError::Overflow)?;
        self.required.insert(id.to_owned(), false);
        Ok(())
    }

    pub fn mark_accepted(&mut self, id: &str) -> Result<(), GuardError> {
        if self.sealed {
            return Err(GuardError::StaleMembership);
        }
        *self.required.get_mut(id).ok_or(GuardError::UnknownReservation)? = true;
        Ok(())
    }

    pub fn seal(&mut self, expected_version: u64) -> Result<(), GuardError> {
        if self.sealed || expected_version != self.version {
            return Err(GuardError::StaleMembership);
        }
        if !self.required.values().all(|complete| *complete) {
            return Err(GuardError::UnfinishedChild);
        }
        self.sealed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_are_attenuated_independent_of_actor_kind() {
        let parent = Grant {
            actor_id: "human-a".into(),
            scope_id: "scope-a".into(),
            authority_epoch: 7,
            operations: ["read", "build"].into_iter().map(str::to_owned).collect(),
            max_delegated_units: 4,
        };
        let mut child = parent.clone();
        child.actor_id = "software-agent-b".into();
        child.scope_id = "scope-b".into();
        child.max_delegated_units = 2;
        assert!(validate_delegation(&parent, &child, true));
        child.operations.insert("publish".into());
        assert!(!validate_delegation(&parent, &child, true));
        child.operations.remove("publish");
        assert!(!validate_delegation(&parent, &child, false));
    }

    #[test]
    fn reservation_never_multiplies_capacity_or_ignores_physical_reclamation() {
        let mut pool = ReservationPool::new(4);
        pool.reserve("a", 3).unwrap();
        assert_eq!(pool.reserve("b", 3), Err(GuardError::InsufficientCapacity));
        assert_eq!(pool.release("a", false), Err(GuardError::NotReclaimed));
        assert_eq!(pool.admitted(), 3);
        pool.release("a", true).unwrap();
        pool.reserve("b", 3).unwrap();
        assert_eq!(pool.reserve("b", 1), Err(GuardError::Duplicate));
    }

    #[test]
    fn producer_late_join_cannot_revive_stopping_generation() {
        let mut producer = SharedProducer::new(4);
        producer.join(4, "human", true, true, true).unwrap();
        assert_eq!(producer.detach(4, "unrelated"), Err(GuardError::UnknownConsumer));
        assert_eq!(producer.state, ProducerState::Running);
        assert!(producer.detach(4, "human").unwrap());
        assert_eq!(producer.join(4, "agent", true, true, true), Err(GuardError::ProducerStopping));
        assert_eq!(producer.join(3, "agent", true, true, true), Err(GuardError::StaleGeneration));
    }

    #[test]
    fn unknown_effect_is_not_automatically_retryable() {
        assert!(!can_retry_effect(EffectOutcome::Unknown, false, false, false, false));
        assert!(!can_retry_effect(EffectOutcome::ConfirmedFailure, false, false, false, false));
        assert!(can_retry_effect(EffectOutcome::NotDispatched, false, false, false, false));
        assert!(can_retry_effect(EffectOutcome::Unknown, true, true, false, false));
        assert!(!can_retry_effect(EffectOutcome::Unknown, false, true, false, false));
        assert!(!can_retry_effect(EffectOutcome::ConfirmedSuccess, true, true, true, true));
    }

    #[test]
    fn parent_acceptance_requires_current_complete_membership() {
        let mut parent = ParentMembership::default();
        parent.add_child("diagnose").unwrap();
        let obsolete = parent.version();
        parent.add_child("verify").unwrap();
        parent.mark_accepted("diagnose").unwrap();
        assert_eq!(parent.seal(obsolete), Err(GuardError::StaleMembership));
        assert_eq!(parent.seal(parent.version()), Err(GuardError::UnfinishedChild));
        parent.mark_accepted("verify").unwrap();
        parent.seal(parent.version()).unwrap();
        assert_eq!(parent.add_child("late"), Err(GuardError::StaleMembership));
    }
}
