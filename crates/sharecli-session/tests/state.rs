// SPDX-License-Identifier: Apache-2.0 OR MIT
// FR:003 — Sidecar state provider tests
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use sharecli_session::{
    append_record, ProcessEvidence, SessionStateProvider, SidecarRecord, SidecarStateProvider,
    SurfaceRecord,
};

static SIDECARE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn surface(id: &str, pid: Option<u32>) -> SurfaceRecord {
    SurfaceRecord {
        id: id.into(),
        terminal: "ghostty".into(),
        title: None,
        cwd: PathBuf::from("/tmp/project"),
        process: Some(ProcessEvidence {
            pid,
            tty: None,
            cwd: PathBuf::from("/tmp/project"),
            argv: vec!["codex".into()],
            started_at: None,
        }),
    }
}

fn temp_sidecar() -> PathBuf {
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let ordinal = SIDECARE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir()
        .join(format!("sharecli-sidecar-{}-{suffix}-{ordinal}.jsonl", std::process::id()))
}

#[test]
fn sidecar_returns_latest_exact_surface_and_pid_match() {
    let path = temp_sidecar();
    fs::write(
        &path,
        "{\"surface_id\":\"pane-1\",\"harness\":\"codex\",\"session_id\":\"old\",\"pid\":41}\n{\"surface_id\":\"pane-1\",\"harness\":\"codex\",\"session_id\":\"new\",\"pid\":42}\n",
    )
    .unwrap();
    let provider = SidecarStateProvider::new(&path);
    assert_eq!(
        provider.session_id(&surface("pane-1", Some(42)), "codex").unwrap(),
        Some("new".into())
    );
    let _ = fs::remove_file(path);
}

#[test]
fn sidecar_refuses_stale_pid_or_harness_matches() {
    let path = temp_sidecar();
    fs::write(
        &path,
        "{\"surface_id\":\"pane-1\",\"harness\":\"codex\",\"session_id\":\"id\",\"pid\":42}\n",
    )
    .unwrap();
    let provider = SidecarStateProvider::new(&path);
    assert_eq!(provider.session_id(&surface("pane-1", Some(7)), "codex").unwrap(), None);
    assert_eq!(provider.session_id(&surface("pane-1", Some(42)), "forge").unwrap(), None);
    let _ = fs::remove_file(path);
}

#[test]
fn newer_pid_mismatch_supersedes_an_older_matching_record() {
    let path = temp_sidecar();
    fs::write(
        &path,
        "{\"surface_id\":\"pane-1\",\"harness\":\"codex\",\"session_id\":\"old\",\"pid\":42}\n{\"surface_id\":\"pane-1\",\"harness\":\"codex\",\"session_id\":\"recycled\",\"pid\":99}\n",
    )
    .unwrap();
    let provider = SidecarStateProvider::new(&path);
    assert_eq!(provider.session_id(&surface("pane-1", Some(42)), "codex").unwrap(), None);
    let _ = fs::remove_file(path);
}

#[test]
fn malformed_sidecar_fails_closed() {
    let path = temp_sidecar();
    fs::write(&path, "not-json\n").unwrap();
    let error = SidecarStateProvider::new(&path)
        .session_id(&surface("pane-1", Some(42)), "codex")
        .unwrap_err();
    assert!(error.to_string().contains("parse sidecar"));
    let _ = fs::remove_file(path);
}

#[test]
fn append_record_writes_jsonl_that_provider_reads() {
    let path = temp_sidecar();
    append_record(
        &path,
        &SidecarRecord {
            surface_id: "pane-1".into(),
            harness: "codex".into(),
            session_id: "thread-1".into(),
            pid: Some(42),
        },
    )
    .unwrap();
    let provider = SidecarStateProvider::new(&path);
    assert_eq!(
        provider.session_id(&surface("pane-1", Some(42)), "codex").unwrap(),
        Some("thread-1".into())
    );
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.ends_with('\n'));
    let _ = fs::remove_file(path);
}
