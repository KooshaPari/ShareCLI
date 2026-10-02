#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use sharecli_core::{
    ChildCommandConfigurator, FakeThermalGate, Hypervisor, HypervisorConfig, SpawnRequest,
    ThermalDecision,
};
use sharecli_ipc::{
    extra_tokens_for_total_parallelism, CacheKeyMode, NativeJobserverClient, QueuePriority,
};
use tempfile::TempDir;

struct JobserverConfigurator<'a>(&'a NativeJobserverClient);

impl ChildCommandConfigurator for JobserverConfigurator<'_> {
    fn configure(&self, command: &mut std::process::Command) {
        self.0.configure_make_child(command);
    }
}

fn hypervisor(dir: &TempDir) -> Hypervisor {
    Hypervisor::from_config_with_gate(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: Duration::from_secs(60),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: CacheKeyMode::Time,
            semantic: false,
        },
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
    )
}

#[tokio::test]
async fn native_jobserver_token_and_child_propagation_compose_with_hypervisor() {
    let dir = TempDir::new().expect("tempdir");
    let hv = hypervisor(&dir);
    let client = NativeJobserverClient::new_owned(2).expect("owned jobserver");

    assert_eq!(extra_tokens_for_total_parallelism(2), 1);
    assert_eq!(client.available_tokens().expect("available before"), 2);

    let lease = client.acquire_extra_tokens(1).expect("acquire one extra token");
    assert_eq!(lease.token_count(), 1);
    assert_eq!(client.available_tokens().expect("available while leased"), 1);

    let request = SpawnRequest::new(
        vec![
            "sh".into(),
            "-c".into(),
            r#"printf '%s|%s' "$MAKEFLAGS" "$CARGO_MAKEFLAGS""#.into(),
        ],
        std::env::current_dir().expect("cwd"),
        vec![],
    )
    .with_queue_priority(QueuePriority::Normal);

    let configurator = JobserverConfigurator(&client);
    let outcome = hv
        .run_queued_with_command_configurator(
            request,
            "native-jobserver-composition-fixture",
            &configurator,
        )
        .await
        .expect("Hypervisor spawn with native jobserver configurator");

    assert_eq!(outcome.exit_code, 0);
    assert!(!outcome.from_cache);
    let inherited = String::from_utf8(outcome.stdout).expect("jobserver environment UTF-8");
    assert!(
        inherited.contains("jobserver") || inherited.contains("-j"),
        "child did not receive make-compatible jobserver state: {inherited:?}"
    );

    drop(lease);
    assert_eq!(client.available_tokens().expect("available after lease drop"), 2);
}
