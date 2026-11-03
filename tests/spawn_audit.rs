// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C02 L28 — spawn/stop audit JSONL rows when `SHARECLI_AUDIT_LOG` is set.
//! FR: FR-004

use std::fs;
use std::sync::Mutex;

use sharecli::audit_log;
use sharecli::runtime::ProcessPool;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Spawn + stop emit `spawn` / `stop` rows with project, capability, outcome.
#[tokio::test]
async fn spawn_audit_emits_jsonl_when_configured() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spawn-audit.jsonl");
    {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("SHARECLI_AUDIT_LOG", &path);
        }
    }

    let pool = ProcessPool::new();

    #[cfg(unix)]
    let info = pool
        .spawn(
            "sleep",
            &["1".to_string()],
            None,
            Some("alpha".to_string()),
            Some("claude".to_string()),
        )
        .await
        .expect("spawn ok");

    #[cfg(windows)]
    let info = pool
        .spawn(
            "cmd",
            &[
                "/C".to_string(),
                "ping".to_string(),
                "127.0.0.1".to_string(),
                "-n".to_string(),
                "2".to_string(),
            ],
            None,
            Some("alpha".to_string()),
            Some("claude".to_string()),
        )
        .await
        .expect("spawn ok");

    pool.kill(info.pid).await.expect("kill ok");

    let body = fs::read_to_string(&path).expect("audit file exists");
    let lines: Vec<_> = body.lines().collect();
    assert_eq!(lines.len(), 2, "expected spawn + stop rows; body={body}");
    assert!(lines[0].contains("\"event\":\"spawn\""));
    assert!(lines[0].contains("\"project\":\"alpha\""));
    assert!(lines[0].contains("\"capability\":\"claude\""));
    assert!(lines[0].contains("\"outcome\":\"ok\""));
    assert!(lines[0].contains(&format!("\"pid\":{}", info.pid)));
    assert!(lines[1].contains("\"event\":\"stop\""));
    assert!(lines[1].contains("\"outcome\":\"ok\""));

    {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var("SHARECLI_AUDIT_LOG");
        }
    }
}

#[test]
fn spawn_audit_skipped_without_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::remove_var("SHARECLI_AUDIT_LOG");
    }
    assert!(!audit_log::is_configured());
}
