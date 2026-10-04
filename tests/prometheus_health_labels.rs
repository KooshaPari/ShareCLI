//! FR: FR-004
// FR/AC ids: UNKNOWN (plan 1.12 names none)
// Tests for task 1.12: health-series label policy
// Health-series use process= (name-keyed store), not pid=.

use sharecli::runtime::ProcState;

fn make_process_with_pid(name: &str, pid: u32, memory_mb: u64) -> sharecli::runtime::ProcessInfo {
    sharecli::runtime::ProcessInfo {
        pid,
        name: name.to_string(),
        cmd: vec!["fake".to_string()],
        memory_mb,
        start_time: 0,
        cpu_percent: 0.0,
        project: None,
        harness: None,
        ppid: None,
        cwd: None,
        env_count: 0,
        state: ProcState::default(),
        disk_read_bytes: None,
        disk_write_bytes: None,
        fd_count: None,
        thread_count: None,
    }
}

fn make_health(healthy: bool, failures: u32) -> sharecli::health_check::HealthStatus {
    sharecli::health_check::HealthStatus {
        healthy,
        consecutive_failures: failures,
        last_error: None,
        last_check: std::time::Instant::now(),
    }
}

/// Health-series (sharecli_health_check_*) must render exactly once per store entry.
/// Health store is name-keyed, so labels use process= (not pid=).
#[test]
fn health_series_render_once_per_store_entry() {
    let processes =
        vec![make_process_with_pid("svc-a", 100, 100), make_process_with_pid("svc-b", 200, 200)];
    let mut hmap = std::collections::HashMap::new();
    hmap.insert("svc-a".to_string(), make_health(true, 0));
    hmap.insert("svc-b".to_string(), make_health(false, 3));
    let out = sharecli::commands::serve::render_prometheus_metrics(&processes, &hmap);

    // Health-series must use process= label (name-keyed store)
    assert!(
        out.contains("sharecli_health_check_consecutive_failures{process=\"svc-a\"} 0"),
        "missing health failures for svc-a: {out}"
    );
    assert!(
        out.contains("sharecli_health_check_consecutive_failures{process=\"svc-b\"} 3"),
        "missing health failures for svc-b: {out}"
    );
    assert!(
        out.contains("sharecli_health_check_status{process=\"svc-a\"} 1"),
        "missing health status for svc-a: {out}"
    );
    assert!(
        out.contains("sharecli_health_check_status{process=\"svc-b\"} 0"),
        "missing health status for svc-b: {out}"
    );

    // Count occurrences of each health-series line
    let failures_a_count =
        out.matches("sharecli_health_check_consecutive_failures{process=\"svc-a\"}").count();
    let failures_b_count =
        out.matches("sharecli_health_check_consecutive_failures{process=\"svc-b\"}").count();
    assert_eq!(failures_a_count, 1, "health failures for svc-a should appear exactly once");
    assert_eq!(failures_b_count, 1, "health failures for svc-b should appear exactly once");
}

/// Health-series must use process= label, not pid=.
/// Health store is keyed by process name, not pid.
#[test]
fn health_series_use_process_label_not_pid() {
    let processes = vec![make_process_with_pid("myapp", 42, 256)];
    let mut hmap = std::collections::HashMap::new();
    hmap.insert("myapp".to_string(), make_health(true, 0));
    let out = sharecli::commands::serve::render_prometheus_metrics(&processes, &hmap);

    // Must have process= label for health-series
    assert!(
        out.contains("sharecli_health_check_consecutive_failures{process=\"myapp\"}"),
        "health failures must use process= label: {out}"
    );
    assert!(
        out.contains("sharecli_health_check_status{process=\"myapp\"}"),
        "health status must use process= label: {out}"
    );

    // Must NOT have pid= label for health-series
    assert!(
        !out.contains("sharecli_health_check_consecutive_failures{pid="),
        "health failures must NOT use pid= label: {out}"
    );
    assert!(
        !out.contains("sharecli_health_check_status{pid="),
        "health status must NOT use pid= label: {out}"
    );
}
