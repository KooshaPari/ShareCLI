//! Regression test for source issue #886: false success from unimplemented pool.
use sharecli::runtime::SharedRuntime;

#[tokio::test]
async fn unimplemented_pool_never_reports_success() {
    let runtime = SharedRuntime::new(2);
    let result = runtime.run_with_pool("node", "test-project", "process.exit(7)").await;
    assert!(result.is_err(), "unimplemented pool must fail closed");
    let message = result.unwrap_err().to_string();
    assert!(message.contains("not implemented"));
    assert!(message.contains("no script executed"));
}

#[tokio::test]
async fn unknown_pid_is_not_reported_as_terminated() {
    let pool = sharecli::runtime::ProcessPool::new();
    let terminated = pool.kill_verified(u32::MAX).await.unwrap();
    assert!(!terminated);
    // Preserve legacy idempotent kill semantics for existing callers.
    assert!(pool.kill(u32::MAX).await.is_ok());
}

#[cfg(unix)]
#[tokio::test]
#[serial_test::serial]
async fn build_permit_stays_held_until_natural_exit() {
    use sharecli::config::SpawnPolicyConfig;
    use sharecli::runtime::ProcessPool;
    use sharecli::spawn_policy::SpawnPolicy;
    use std::sync::Arc;
    use std::time::Duration;

    let config = SpawnPolicyConfig { nice_level: 0, max_concurrent_builds: 1, use_sccache: false };
    let policy = Arc::new(SpawnPolicy::new(config));
    let pool = ProcessPool::with_spawn_policy(Arc::clone(&policy));
    let _child = pool
        .spawn("sleep", &["1".to_string()], None, None, Some("cargo".to_string()))
        .await
        .unwrap();
    assert_eq!(policy.available_permits(), 0, "permit released at spawn");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(policy.available_permits(), 0, "permit released before exit");

    for _ in 0..30 {
        if policy.available_permits() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(policy.available_permits(), 1, "permit not released after exit");
}

#[cfg(unix)]
#[tokio::test]
#[serial_test::serial]
async fn build_permit_released_after_explicit_kill() {
    use sharecli::config::SpawnPolicyConfig;
    use sharecli::runtime::ProcessPool;
    use sharecli::spawn_policy::SpawnPolicy;
    use std::sync::Arc;
    use std::time::Duration;

    let config = SpawnPolicyConfig { nice_level: 0, max_concurrent_builds: 1, use_sccache: false };
    let policy = Arc::new(SpawnPolicy::new(config));
    let pool = ProcessPool::with_spawn_policy(Arc::clone(&policy));
    let child = pool
        .spawn("sleep", &["30".to_string()], None, None, Some("cargo".to_string()))
        .await
        .unwrap();
    assert_eq!(policy.available_permits(), 0);
    pool.kill(child.pid).await.unwrap();
    for _ in 0..20 {
        if policy.available_permits() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(policy.available_permits(), 1, "kill must release build slot");
}
