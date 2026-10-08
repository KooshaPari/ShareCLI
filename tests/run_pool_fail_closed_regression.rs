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
