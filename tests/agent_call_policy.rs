// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::path::PathBuf;
use std::time::Duration;

use sharecli::agent_call_policy::{AgentCallPolicy, PauseCode};

fn policy() -> AgentCallPolicy {
    AgentCallPolicy::new(PathBuf::from("/workspace/project"))
}

#[test]
fn rewrites_recursive_grep_to_a_bounded_ripgrep_command() {
    let decision = policy().admit("grep -R TODO .");

    assert_eq!(
        decision.command(),
        "rg --hidden --glob '!target' --glob '!node_modules' TODO /workspace/project"
    );
}

#[test]
fn leaves_nonrecursive_grep_unchanged() {
    let decision = policy().admit("grep TODO README.md");

    assert_eq!(decision.command(), "grep TODO README.md");
}

#[test]
fn pauses_searches_targeting_a_hazardous_root() {
    let decision = policy().admit("rg TODO /System");

    assert_eq!(decision.pause_code(), Some(PauseCode::HazardousRoot));
    assert!(decision.resume_condition().is_some());
}

#[test]
fn pauses_recursive_grep_targeting_filesystem_root() {
    let decision = policy().admit("grep -R TODO /");

    assert_eq!(decision.pause_code(), Some(PauseCode::HazardousRoot));
    assert!(decision.resume_condition().is_some());
}

#[test]
fn pauses_when_the_project_concurrency_limit_is_reached() {
    let policy = policy().with_project_limit(1);
    let _first = policy.admit("rg TODO src");
    let decision = policy.admit("rg FIXME src");

    assert_eq!(decision.pause_code(), Some(PauseCode::ProjectLimit));
}

#[test]
fn pauses_when_thermal_headroom_is_unavailable() {
    let decision = policy().with_thermal_headroom(false).admit("rg TODO src");

    assert_eq!(decision.pause_code(), Some(PauseCode::Thermal));
}

#[test]
fn pauses_builds_when_no_build_slot_is_available() {
    let policy = policy().with_build_slots(1);
    let _first = policy.admit("cargo test");
    let decision = policy.admit("cargo build");

    assert_eq!(decision.pause_code(), Some(PauseCode::BuildSlot));
}

#[test]
fn attaches_a_nonzero_deadline_to_admitted_calls() {
    let decision = policy().admit("rg TODO src");

    assert!(decision.deadline() > Duration::ZERO);
}
