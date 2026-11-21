//! Process superset: core port contracts compile and round-trip.

use substrate_core::{
    process_port::{ProcessSpawnSpec, ProcessState},
    watcher_port::{WatchEvent, WatchEventKind},
};

#[test]
fn process_spawn_spec_round_trips_json() {
    let spec = ProcessSpawnSpec {
        program: "echo".into(),
        args: vec!["hi".into()],
        cwd: None,
    };
    let json = serde_json::to_string(&spec).unwrap();
    let back: ProcessSpawnSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(spec, back);
}

#[test]
fn process_state_variants_are_distinct() {
    let running = ProcessState::Running { pid: 42 };
    let exited = ProcessState::Exited {
        pid: 42,
        code: Some(0),
    };
    assert_ne!(running, exited);
}

#[test]
fn watch_event_kind_maps_create_modify() {
    let ev = WatchEvent {
        path: std::path::PathBuf::from("x.txt"),
        kind: WatchEventKind::Create,
    };
    let json = serde_json::to_string(&ev).unwrap();
    let back: WatchEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(ev, back);
    assert_eq!(back.kind, WatchEventKind::Create);
}
