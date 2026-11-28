// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integration tests for the first-class `sharecli queue` verbs (FR-010).
//!
//! Every test drives the real binary through `env!("CARGO_BIN_EXE_sharecli")`
//! against a fresh temporary Maildir queue, so both the CLI contract and the
//! on-disk lifecycle (`new/` → `cur/`) are asserted end to end.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn run(args: &[&str], env: &[(&str, &Path)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sharecli"));
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output().expect("spawn sharecli")
}

fn out_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn err_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn json(out: &Output) -> Value {
    serde_json::from_str(&out_text(out)).unwrap_or_else(|e| {
        panic!("stdout is not JSON ({e}); stdout: {}; stderr: {}", out_text(out), err_text(out))
    })
}

fn qstr(queue: &Path) -> String {
    queue.to_string_lossy().into_owned()
}

fn count_files(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).filter(|e| e.path().is_file()).count())
        .unwrap_or(0)
}

fn enqueue(queue: &Path, payload: &str, priority: u8) -> String {
    let out = run(
        &[
            "queue",
            "enqueue",
            "--queue",
            &qstr(queue),
            "--json",
            payload,
            "--priority",
            &priority.to_string(),
        ],
        &[],
    );
    assert!(out.status.success(), "enqueue should exit 0; stderr: {}", err_text(&out));
    out_text(&out).trim().to_string()
}

#[test]
fn round_trip_enqueue_list_claim() {
    let dir = TempDir::new().unwrap();
    let queue: PathBuf = dir.path().to_path_buf();

    let id = enqueue(&queue, r#"{"cmd":"lint"}"#, 5);
    assert!(!id.is_empty(), "enqueue should print the task id");
    assert!(queue.join("new").join(&id).exists(), "enqueue must land in new/");

    let out = run(&["queue", "list", "--queue", &qstr(&queue), "--json"], &[]);
    assert!(out.status.success(), "list should exit 0; stderr: {}", err_text(&out));
    let listed = json(&out);
    assert_eq!(listed["ok"], true);
    assert_eq!(listed["count"], 1);
    assert_eq!(listed["tasks"][0]["id"], id.as_str());
    assert_eq!(listed["tasks"][0]["location"], "new");

    let out = run(&["queue", "claim", "--queue", &qstr(&queue), "--json"], &[]);
    assert!(out.status.success(), "claim should exit 0; stderr: {}", err_text(&out));
    let claimed = json(&out);
    assert_eq!(claimed["ok"], true);
    assert_eq!(claimed["claimed"], true);
    assert_eq!(claimed["task"]["id"], id.as_str());
    assert_eq!(claimed["task"]["location"], "cur");
    assert_eq!(claimed["task"]["attempts"], 1);

    assert!(queue.join("cur").join(&id).exists(), "claim must move the file to cur/");
    assert!(!queue.join("new").join(&id).exists(), "claim must remove it from new/");
}

#[test]
fn priority_ordering_claims_lowest_number_first() {
    let dir = TempDir::new().unwrap();
    let queue = dir.path();

    let _low = enqueue(queue, r#"{"op":"background"}"#, 9);
    let high = enqueue(queue, r#"{"op":"urgent"}"#, 1);

    let out = run(&["queue", "claim", "--queue", &qstr(queue), "--json"], &[]);
    assert!(out.status.success(), "claim should exit 0; stderr: {}", err_text(&out));
    let claimed = json(&out);
    assert_eq!(
        claimed["task"]["id"],
        high.as_str(),
        "priority 1 must be claimed before priority 9"
    );
    assert_eq!(claimed["task"]["priority"], 1);
}

#[test]
fn empty_queue_claim_exits_zero() {
    let dir = TempDir::new().unwrap();
    let out = run(&["queue", "claim", "--queue", &qstr(dir.path())], &[]);
    assert!(
        out.status.success(),
        "claim on an empty queue must exit 0; stderr: {}",
        err_text(&out)
    );
    assert!(out_text(&out).contains("no ready task"), "got: {}", out_text(&out));

    let out = run(&["queue", "claim", "--queue", &qstr(dir.path()), "--json"], &[]);
    assert!(out.status.success(), "json claim on an empty queue must exit 0");
    let value = json(&out);
    assert_eq!(value["ok"], true);
    assert_eq!(value["claimed"], false);
    assert!(value["task"].is_null());
}

#[test]
fn drain_dry_run_is_side_effect_free() {
    let dir = TempDir::new().unwrap();
    let queue = dir.path();
    let first = enqueue(queue, r#"{"op":"one"}"#, 7);
    let second = enqueue(queue, r#"{"op":"two"}"#, 2);

    let new_before = count_files(&queue.join("new"));
    let cur_before = count_files(&queue.join("cur"));

    let out = run(&["queue", "drain", "--queue", &qstr(queue)], &[]);
    assert!(out.status.success(), "drain dry-run should exit 0; stderr: {}", err_text(&out));

    assert_eq!(
        count_files(&queue.join("new")),
        new_before,
        "dry-run must not move files out of new/"
    );
    assert_eq!(count_files(&queue.join("cur")), cur_before, "dry-run must not add files to cur/");
    assert!(queue.join("new").join(&first).exists());
    assert!(queue.join("new").join(&second).exists());

    let out = run(&["queue", "drain", "--queue", &qstr(queue), "--json"], &[]);
    assert!(out.status.success(), "drain --json should exit 0; stderr: {}", err_text(&out));
    let value = json(&out);
    assert_eq!(value["ok"], true);
    assert_eq!(value["applied"], false);
    assert_eq!(value["count"], 2);
    assert_eq!(value["tasks"][0]["id"], second.as_str(), "priority 2 must be reported first");
    assert_eq!(value["tasks"][1]["id"], first.as_str(), "priority 7 must be reported second");
    assert_eq!(count_files(&queue.join("new")), new_before, "json dry-run must not move files");
}

#[test]
fn drain_apply_claims_every_ready_task() {
    let dir = TempDir::new().unwrap();
    let queue = dir.path();
    enqueue(queue, r#"{"op":"one"}"#, 5);
    enqueue(queue, r#"{"op":"two"}"#, 5);

    let out = run(&["queue", "drain", "--queue", &qstr(queue), "--apply", "--json"], &[]);
    assert!(out.status.success(), "drain --apply should exit 0; stderr: {}", err_text(&out));
    let value = json(&out);
    assert_eq!(value["applied"], true);
    assert_eq!(value["count"], 2);
    assert_eq!(count_files(&queue.join("new")), 0);
    assert_eq!(count_files(&queue.join("cur")), 2);
}

#[test]
fn json_envelopes_carry_documented_keys() {
    let dir = TempDir::new().unwrap();
    let queue = dir.path();
    enqueue(queue, r#"{"cmd":"lint"}"#, 5);

    let stats = json(&run(&["queue", "stats", "--queue", &qstr(queue), "--json"], &[]));
    assert_eq!(stats["ok"], true);
    assert_eq!(stats["queue"]["ready"], 1);
    assert_eq!(stats["queue"]["in_flight"], 0);
    assert_eq!(stats["queue"]["pending"], 1);
    assert!(stats["queue"]["path"].is_string());

    let list = json(&run(&["queue", "list", "--queue", &qstr(queue), "--json"], &[]));
    assert_eq!(list["ok"], true);
    assert_eq!(list["ready"], 1);
    assert_eq!(list["in_flight"], 0);
    let task = &list["tasks"][0];
    for key in [
        "id",
        "priority",
        "created_at",
        "created",
        "attempts",
        "owner",
        "payload_preview",
        "payload",
        "location",
    ] {
        assert!(task.get(key).is_some(), "list task JSON must carry `{key}`; got: {task}");
    }
    assert!(task["payload_preview"].as_str().unwrap().chars().count() <= 60);

    let enq = json(&run(
        &["queue", "enqueue", "--queue", &qstr(queue), "--json", r#"{"cmd":"x"}"#, "--emit-json"],
        &[],
    ));
    assert_eq!(enq["ok"], true);
    assert!(enq["id"].is_string());
}

#[test]
fn priority_out_of_range_is_a_clean_usage_error() {
    let dir = TempDir::new().unwrap();
    let out = run(
        &[
            "queue",
            "enqueue",
            "--queue",
            &qstr(dir.path()),
            "--json",
            r#"{"cmd":"lint"}"#,
            "--priority",
            "10",
        ],
        &[],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "out-of-range --priority must exit 2; stderr: {}",
        err_text(&out)
    );
    assert!(
        err_text(&out).contains("priority"),
        "error should name the offending flag; got: {}",
        err_text(&out)
    );
}

#[test]
fn queue_and_mesh_share_one_queue_root() {
    let dir = TempDir::new().unwrap();
    let queue = dir.path();

    // No --queue: `queue stats` resolves SHARECLI_MESH_QUEUE, the same knob `mesh` honours.
    let out = run(&["queue", "stats"], &[("SHARECLI_MESH_QUEUE", queue)]);
    assert!(out.status.success(), "queue stats should exit 0; stderr: {}", err_text(&out));
    let queue_path = out_text(&out);
    assert!(
        queue_path.contains(&qstr(queue)),
        "queue stats must print the shared root; got: {queue_path}"
    );

    let out = run(&["mesh", "status", "--queue", &qstr(queue)], &[]);
    assert!(out.status.success(), "mesh status should exit 0; stderr: {}", err_text(&out));
    let mesh_path = out_text(&out);
    assert!(
        mesh_path.contains(&qstr(queue)),
        "mesh status must print the same root; got: {mesh_path}"
    );
}
