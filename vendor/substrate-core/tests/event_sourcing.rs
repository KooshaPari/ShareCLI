//! Event sourcing: projection replay and task lifecycle fold.

use substrate_core::{
    domain::TaskState,
    event_store_port::{
        replay, EventEnvelope, EventStorePort, TaskLifecycleEvent, TaskLifecycleProjection,
    },
};
use uuid::Uuid;

struct InMemoryEventStore {
    events: std::sync::Mutex<Vec<EventEnvelope<TaskLifecycleEvent>>>,
}

impl InMemoryEventStore {
    fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
struct MemStoreError;

impl std::fmt::Display for MemStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("duplicate seq")
    }
}

impl std::error::Error for MemStoreError {}

impl EventStorePort for InMemoryEventStore {
    type Error = MemStoreError;
    type Event = TaskLifecycleEvent;

    fn append(
        &self,
        aggregate_id: Uuid,
        expected_seq: u64,
        event: &Self::Event,
    ) -> Result<EventEnvelope<Self::Event>, Self::Error> {
        let mut events = self.events.lock().unwrap();
        let current = events
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id)
            .count() as u64;
        if current != expected_seq {
            return Err(MemStoreError);
        }
        let aggregate_seq = expected_seq;
        let global_seq = events.len() as u64;
        let envelope = EventEnvelope {
            aggregate_id,
            aggregate_seq,
            global_seq,
            event: event.clone(),
            occurred_at: 1,
        };
        events.push(envelope.clone());
        Ok(envelope)
    }

    fn load(&self, aggregate_id: Uuid) -> Result<Vec<EventEnvelope<Self::Event>>, Self::Error> {
        let events = self.events.lock().unwrap();
        let mut out: Vec<_> = events
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id)
            .cloned()
            .collect();
        out.sort_by_key(|e| e.aggregate_seq);
        Ok(out)
    }
}

#[test]
fn replay_folds_task_lifecycle_into_final_state() {
    let events = vec![
        TaskLifecycleEvent::Created {
            prompt: "fix bug".into(),
            cwd: "/tmp".into(),
        },
        TaskLifecycleEvent::Advanced {
            to: TaskState::Working,
        },
        TaskLifecycleEvent::Advanced {
            to: TaskState::Completed,
        },
    ];
    let state = replay::<TaskLifecycleProjection>(&events);
    assert_eq!(state.prompt, "fix bug");
    assert_eq!(state.cwd, "/tmp");
    assert_eq!(state.state, TaskState::Completed);
}

#[test]
fn replay_rejects_invalid_fsm_edges() {
    let events = vec![
        TaskLifecycleEvent::Created {
            prompt: "x".into(),
            cwd: ".".into(),
        },
        TaskLifecycleEvent::Advanced {
            to: TaskState::Completed,
        },
    ];
    let state = replay::<TaskLifecycleProjection>(&events);
    assert_eq!(state.state, TaskState::Submitted);
}

#[test]
fn in_memory_append_load_monotonic_seq() {
    let store = InMemoryEventStore::new();
    let id = Uuid::new_v4();
    store
        .append(
            id,
            0,
            &TaskLifecycleEvent::Created {
                prompt: "a".into(),
                cwd: "b".into(),
            },
        )
        .unwrap();
    store
        .append(
            id,
            1,
            &TaskLifecycleEvent::Advanced {
                to: TaskState::Working,
            },
        )
        .unwrap();

    let loaded = store.load(id).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].aggregate_seq, 0);
    assert_eq!(loaded[1].aggregate_seq, 1);
    assert_eq!(loaded[0].global_seq, 0);
    assert_eq!(loaded[1].global_seq, 1);
}

#[test]
fn duplicate_expected_seq_rejected() {
    let store = InMemoryEventStore::new();
    let id = Uuid::new_v4();
    store
        .append(
            id,
            0,
            &TaskLifecycleEvent::Created {
                prompt: "a".into(),
                cwd: "b".into(),
            },
        )
        .unwrap();
    assert!(store
        .append(
            id,
            0,
            &TaskLifecycleEvent::Advanced {
                to: TaskState::Working,
            },
        )
        .is_err());
}
