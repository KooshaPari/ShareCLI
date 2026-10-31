// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-009 — Hypervisor FUSE write-provenance session wiring
//! FR: FR-009
//!
//! AC-009.12 Hypervisor cache-miss FUSE mounts derive session id from coalesce CommandKey
//! AC-009.13 Hypervisor SpawnOutcome exposes fuse_session_id when intercept is active
//! AC-009.14 SpawnOutcome exposes backing/mount remap pair; remap_mount_to_backing path translate
//!
//! GATED: Requires FUSE kernel support (Linux/macOS only).

#![cfg(not(target_os = "windows"))]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sharecli_core::{
    fuse_session_id_for_command_key, FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest,
    ThermalDecision,
};
use sharecli_fuse::remap_mount_to_backing;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use sharecli_fuse::{read_provenance, InterceptFs};
use sharecli_ipc::command_key;
use tempfile::TempDir;

/// FR-009 / AC-009.12 — coalesce key maps to FUSE session id stamped on writes.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn fr009_hypervisor_fuse_session_stamps_provenance() {
    use std::fs;
    use std::io::Write;

    let dir = TempDir::new().expect("tempdir");
    let file = dir.path().join("artifact.txt");
    {
        let mut f = fs::File::create(&file).expect("create");
        f.write_all(b"seed").expect("write");
    }

    let key = command_key(&["cargo".into(), "build".into()], dir.path(), &[]);
    let session = fuse_session_id_for_command_key(&key);
    assert!(session.starts_with("hv-"));
    assert_eq!(session.len(), 19, "hv- + 16 hex chars from CommandKey");

    let fs = InterceptFs::with_session(dir.path(), &session);
    assert_eq!(fs.session_id(), session);

    fs.write_rel(Path::new("artifact.txt"), 0, b"built").expect("write_rel");
    let prov = read_provenance(&file).expect("read_provenance").expect("provenance xattrs");
    assert_eq!(prov.session_id, session);
}

/// FR-009 / AC-009.12 — different argv → different FUSE session ids.
#[test]
fn fr009_hypervisor_fuse_session_differs_by_command_key() {
    let cwd = Path::new("/workspace");
    let k1 = command_key(&["a".into()], cwd, &[]);
    let k2 = command_key(&["b".into()], cwd, &[]);
    assert_ne!(fuse_session_id_for_command_key(&k1), fuse_session_id_for_command_key(&k2));
}

/// FR-009 / AC-009.13 — cache-miss Hypervisor run surfaces fuse_session_id when FUSE active.
#[tokio::test]
async fn fr009_hypervisor_spawn_outcome_fuse_session_id() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "fr009-fuse-spawn-outcome".to_string()];
    #[cfg(windows)]
    let argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        "echo".to_string(),
        "fr009-fuse-spawn-outcome".to_string(),
    ];

    let req = SpawnRequest {
        argv: argv.clone(),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };
    let key = command_key(&argv, dir.path(), &[]);
    let expected_session = fuse_session_id_for_command_key(&key);

    let outcome = hv.run(req).await.expect("Hypervisor cache-miss run");

    assert!(!outcome.from_cache, "first run must be cache miss");
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if outcome.fuse_intercept_active() {
            assert_eq!(
                outcome.fuse_session_id.as_deref(),
                Some(expected_session.as_str()),
                "active FUSE intercept MUST expose coalesce-derived session id"
            );
        } else {
            assert!(
                outcome.fuse_session_id.is_none(),
                "inactive FUSE MUST leave fuse_session_id None"
            );
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        assert!(
            outcome.fuse_session_id.is_none(),
            "non-FUSE platforms MUST leave fuse_session_id None"
        );
    }

    // Cache hit must never carry a FUSE session id.
    let hit = hv
        .run(SpawnRequest {
            argv,
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect("Hypervisor cache hit");
    assert!(hit.from_cache);
    assert!(hit.fuse_session_id.is_none());
    assert!(!hit.fuse_intercept_active());
}

/// FR-009 / AC-009.14 — remap_mount_to_backing translates mount subtree to backing.
#[test]
fn fr009_remap_mount_to_backing_subtree() {
    let mount = Path::new("/tmp/fuse-mp");
    let backing = Path::new("/workspace/proj");
    assert_eq!(
        remap_mount_to_backing(mount, backing, Path::new("/tmp/fuse-mp/src/main.rs")),
        Some(PathBuf::from("/workspace/proj/src/main.rs"))
    );
    assert_eq!(
        remap_mount_to_backing(mount, backing, Path::new("src/main.rs")),
        Some(PathBuf::from("/workspace/proj/src/main.rs"))
    );
    assert!(remap_mount_to_backing(mount, backing, Path::new("/tmp/mountextra/x")).is_none());
}

/// FR-009 / AC-009.14 — active intercept SpawnOutcome carries backing + mount remap pair.
#[tokio::test]
async fn fr009_hypervisor_spawn_outcome_fuse_path_remap() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "fr009-fuse-remap".to_string()];
    #[cfg(windows)]
    let argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        "echo".to_string(),
        "fr009-fuse-remap".to_string(),
    ];

    let outcome = hv
        .run(SpawnRequest {
            argv,
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect("Hypervisor cache-miss run");

    assert!(!outcome.from_cache);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if outcome.fuse_intercept_active() {
        assert_eq!(outcome.fuse_backing.as_deref(), Some(dir.path()));
        let mount = outcome.fuse_mountpoint.as_ref().expect("mountpoint when intercept active");
        assert_ne!(mount, dir.path());
        assert_eq!(outcome.remap_fuse_path(mount), Some(dir.path().to_path_buf()));
    } else {
        assert!(outcome.fuse_backing.is_none());
        assert!(outcome.fuse_mountpoint.is_none());
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        assert!(outcome.fuse_backing.is_none());
        assert!(outcome.fuse_mountpoint.is_none());
    }
}
