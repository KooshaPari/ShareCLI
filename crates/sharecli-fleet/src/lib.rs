//! `sharecli-fleet` — fleet registry and thermal governor.
//!
//! Provides the device registry (NATS-backed) and thermal-aware scheduling
//! primitives for sharecli's fleet runtime.

pub mod agent_contention;
pub mod coalesce_meters;
pub mod detect;
pub mod operator_pool_status;
pub mod proc_scan;
pub mod registry;
pub mod resource_watch;
pub mod slot_queue_meters;
pub mod thermal;

pub use agent_contention::{
    agent_contention_tier, agent_resource_contention_tier, combined_agent_contention_tier,
    count_host_agents, effective_gate_decision, effective_gate_decision_for_tier,
    format_gate_status_from_snapshot, format_gate_status_section, gate_status_snapshot,
    gate_status_snapshot_with_rss, live_agent_contention_tier, total_watched_agent_rss_bytes,
    AgentContentionThresholds, AgentContentionTier, AgentResourceThresholds, GateStatusSnapshot,
};
use async_nats::Client;
pub use coalesce_meters::{
    global_coalesce_meters, record_coalesce_hit_kind, record_coalesce_lookup_hit,
    record_nocache_run, CoalesceHitKind, CoalesceMeters,
};
pub use detect::{
    clear_runtime_patterns, load_default_runtime_patterns, load_patterns, load_runtime_patterns,
    match_known_agent, match_system_tool, RuntimePattern, RuntimeSystemTool, KNOWN_AGENT_FAMILIES,
};
pub use operator_pool_status::{
    format_pool_operator_line, format_status_operator_line, PoolOperatorPanel, StatusOperatorPanel,
};
pub use proc_scan::{
    agent_label_for_pid, build_agent_forests, build_agent_state_map, build_forest_state_map,
    build_host_agent_forests, build_host_agent_state_map, build_host_forest_state_map,
    collect_forest_pids, detect_caller_agent, is_under_agent, lookup_host_proc, lookup_proc,
    scan_agents, scan_host_agents, scan_host_system_tools, scan_system_tools, state_text_for_pid,
    walk_agent_ancestors, AgentTreeNode, DetectedAgent, FakeProcSource, HostProcSource,
    ProcSnapshot, ProcSource, SystemToolScan,
};
pub use registry::{DeviceRecord, FleetRegistry, DEFAULT_SUBJECT_PREFIX};
pub use resource_watch::{
    format_rss_bytes, parse_rss_bytes, sample_host_load_1m, sample_host_net, sample_pid_fds,
    sample_pid_rss_bytes, sample_self_fds, sample_self_rss_bytes, sum_detected_agent_rss_bytes,
    watch_detected_agents, watch_host_agents, AgentResourceSample, DetectedAgentWatch,
    ResourceWatchSample,
};
pub use slot_queue_meters::{
    global_slot_queue_meters, record_slot_acquire, record_slot_timeout, record_slot_wait,
    SlotQueueMeters,
};
pub use thermal::{ThermalGovernor, ThermalLevel};

/// Default NATS coordinator URL used when none is specified.
pub const DEFAULT_COORDINATOR: &str = "nats://localhost:4222";

/// NATS subject for fleet-wide device announcements.
pub const FLEET_SUBJECT: &str = "sharecli.fleet.devices";

/// Connect to the NATS coordinator and return a client.
pub async fn connect(coordinator: &str) -> anyhow::Result<Client> {
    let client = async_nats::connect(coordinator)
        .await
        .map_err(|e| anyhow::anyhow!("NATS connect to {coordinator} failed: {e}"))?;
    tracing::info!(coordinator, "sharecli-fleet: connected to NATS coordinator");
    Ok(client)
}

/// Publish this device's [`DeviceRecord`] to the fleet subject.
pub async fn announce(client: &Client, record: &DeviceRecord) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(record)?;
    client.publish(FLEET_SUBJECT, payload.into()).await?;
    Ok(())
}

/// Subscribe to fleet device announcements and return a subscriber.
pub async fn subscribe(client: &Client) -> anyhow::Result<async_nats::Subscriber> {
    Ok(client.subscribe(FLEET_SUBJECT).await?)
}

/// Publish a DeviceRecord health-beat every `interval` until the token is cancelled.
///
/// Runs in the background — spawn with `tokio::spawn(health_beat(...))`.
/// Stops cleanly when the `async_nats::Client` is dropped or the interval is cancelled.
pub async fn health_beat(client: Client, record: DeviceRecord, interval: std::time::Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        if let Err(e) = announce(&client, &record).await {
            tracing::warn!("health_beat: announce failed: {e}");
        }
    }
}
