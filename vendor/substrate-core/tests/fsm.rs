//! Lifecycle FSM tests: valid transitions accepted, invalid rejected.

use substrate_core::{
    domain::{Task, TaskState},
    SubstrateError,
};

#[test]
fn happy_path_transitions_are_legal() {
    use TaskState::*;
    assert!(TaskState::can_transition(Submitted, Working));
    assert!(TaskState::can_transition(Working, InputRequired));
    assert!(TaskState::can_transition(InputRequired, Working));
    assert!(TaskState::can_transition(Working, Completed));
}

#[test]
fn any_live_state_may_fail_or_cancel() {
    use TaskState::*;
    for s in [Submitted, Working, InputRequired] {
        assert!(TaskState::can_transition(s, Failed), "{s:?}->Failed");
        assert!(TaskState::can_transition(s, Cancelled), "{s:?}->Cancelled");
    }
}

#[test]
fn terminal_states_have_no_outgoing_edges() {
    use TaskState::*;
    for from in [Completed, Failed, Cancelled] {
        for to in [
            Submitted,
            Working,
            InputRequired,
            Completed,
            Failed,
            Cancelled,
        ] {
            assert!(
                !TaskState::can_transition(from, to),
                "terminal {from:?} must not -> {to:?}"
            );
        }
    }
}

#[test]
fn illegal_skips_are_rejected() {
    use TaskState::*;
    // Cannot jump straight from Submitted to Completed.
    assert!(!TaskState::can_transition(Submitted, Completed));
    // Cannot go Submitted -> InputRequired without Working.
    assert!(!TaskState::can_transition(Submitted, InputRequired));
    // No self-loops.
    assert!(!TaskState::can_transition(Working, Working));
}

#[test]
fn advance_mutates_on_legal_and_errors_on_illegal() {
    let mut t = Task::new("do the thing", "/tmp");
    assert_eq!(t.state, TaskState::Submitted);

    t.advance(TaskState::Working).unwrap();
    assert_eq!(t.state, TaskState::Working);

    // Illegal: Working -> Submitted.
    let err = t.advance(TaskState::Submitted).unwrap_err();
    assert!(matches!(err, SubstrateError::InvalidTransition { .. }));
    // State unchanged after a rejected transition.
    assert_eq!(t.state, TaskState::Working);

    t.advance(TaskState::Completed).unwrap();
    assert!(t.state.is_terminal());

    // No transitions out of a terminal state.
    assert!(t.advance(TaskState::Working).is_err());
}
