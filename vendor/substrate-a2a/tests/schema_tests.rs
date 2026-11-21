use a2a::{
    message::{Artifact, Message, MessageKind, MsgState, Part},
    task::{Task, TaskState},
};

#[test]
fn task_serde_round_trip() {
    let task = Task::new("team-1", "Write a PR", "agent-lead");
    let json = serde_json::to_string(&task).expect("serialize");
    let back: Task = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(task, back);
}

#[test]
fn message_serde_round_trip() {
    let msg = Message::new(
        "team-1",
        "lead",
        "worker",
        MessageKind::Task,
        vec![Part::Text {
            text: "do the thing".to_string(),
        }],
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn artifact_serde_round_trip() {
    let art = Artifact {
        kind: "diff".to_string(),
        content: "--- a\n+++ b".to_string(),
        name: Some("patch.diff".to_string()),
    };
    let json = serde_json::to_string(&art).expect("serialize");
    let back: Artifact = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(art, back);
}

#[test]
fn task_state_submitted_to_working_is_legal() {
    assert!(TaskState::can_transition(
        TaskState::Submitted,
        TaskState::Working
    ));
    assert!(!TaskState::Working.is_terminal());
    assert!(TaskState::Completed.is_terminal());
}

#[test]
fn task_state_completed_to_working_is_illegal() {
    // Completed is terminal — no outgoing transitions
    assert!(!TaskState::can_transition(
        TaskState::Completed,
        TaskState::Working
    ));
}

#[test]
fn msg_state_variants_serialize() {
    let states = [MsgState::Unread, MsgState::Delivered, MsgState::Consumed];
    for s in states {
        let json = serde_json::to_string(&s).expect("serialize MsgState");
        let back: MsgState = serde_json::from_str(&json).expect("deserialize MsgState");
        assert_eq!(s, back);
    }
}

#[test]
fn part_file_round_trip() {
    let part = Part::File {
        uri: "file:///tmp/foo.diff".to_string(),
    };
    let json = serde_json::to_string(&part).expect("serialize Part::File");
    let back: Part = serde_json::from_str(&json).expect("deserialize Part::File");
    assert_eq!(part, back);
}

#[test]
fn part_data_round_trip() {
    let part = Part::Data {
        data: serde_json::json!({"key": "value", "num": 42}),
    };
    let json = serde_json::to_string(&part).expect("serialize Part::Data");
    let back: Part = serde_json::from_str(&json).expect("deserialize Part::Data");
    assert_eq!(part, back);
}
