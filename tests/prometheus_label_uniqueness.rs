//! FR: FR-004
// FR/AC ids: UNKNOWN (plan 1.12 names none)
// Tests for task 1.12: label uniqueness (pid labels, no duplicate series)
// Strict TDD: these tests are written BEFORE the implementation changes.

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

/// Two pids with the same name must render distinct pid= labels,
/// not duplicate process= labels (FINDINGS:105 defect).
#[test]
fn two_pids_same_name_render_distinct_pid_labels() {
    let p1 = make_process_with_pid("worker", 111, 100);
    let p2 = make_process_with_pid("worker", 222, 200);
    let processes = vec![p1, p2];
    let hmap = std::collections::HashMap::new();
    let out = sharecli::commands::serve::render_prometheus_metrics(&processes, &hmap);

    // Must contain pid= labels, not process= labels for per-process metrics
    assert!(
        out.contains("sharecli_process_memory_mb{pid=\"111\"}"),
        "missing pid=111 label for memory: {out}"
    );
    assert!(
        out.contains("sharecli_process_memory_mb{pid=\"222\"}"),
        "missing pid=222 label for memory: {out}"
    );
    assert!(
        out.contains("sharecli_process_up{pid=\"111\"}"),
        "missing pid=111 label for process_up: {out}"
    );
    assert!(
        out.contains("sharecli_process_up{pid=\"222\"}"),
        "missing pid=222 label for process_up: {out}"
    );
}

/// Per-process metrics must use pid= label, not process= label.
/// The process= label is reserved for health-series (name-keyed store).
#[test]
fn per_process_metrics_use_pid_label_not_process() {
    let p = make_process_with_pid("myapp", 42, 256);
    let processes = vec![p];
    let hmap = std::collections::HashMap::new();
    let out = sharecli::commands::serve::render_prometheus_metrics(&processes, &hmap);

    // Must have pid= label
    assert!(
        out.contains("sharecli_process_memory_mb{pid=\"42\"}"),
        "memory metric must use pid= label: {out}"
    );
    assert!(
        out.contains("sharecli_process_up{pid=\"42\"}"),
        "process_up metric must use pid= label: {out}"
    );

    // Must NOT have process= label for per-process metrics
    assert!(
        !out.contains("sharecli_process_memory_mb{process="),
        "memory metric must NOT use process= label: {out}"
    );
    assert!(
        !out.contains("sharecli_process_up{process="),
        "process_up metric must NOT use process= label: {out}"
    );
}

/// Rendered output must have no duplicate series within a family.
/// Parse and check that no identical label-set appears twice.
#[test]
fn rendered_body_has_no_duplicate_series() {
    let p1 = make_process_with_pid("svc", 100, 100);
    let p2 = make_process_with_pid("svc", 200, 200);
    let processes = vec![p1, p2];
    let mut hmap = std::collections::HashMap::new();
    hmap.insert("svc".to_string(), make_health(true, 0));
    let out = sharecli::commands::serve::render_prometheus_metrics(&processes, &hmap);

    // Collect all metric lines (skip HELP/TYPE comments)
    let metric_lines: Vec<&str> =
        out.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();

    // Check for duplicates
    let mut seen = std::collections::HashSet::new();
    for line in &metric_lines {
        // Extract just the metric name + labels (before the value)
        if let Some(metric_part) = line.rsplit_once(' ') {
            let metric_name = metric_part.0;
            assert!(seen.insert(metric_name), "duplicate series found: {line}");
        }
    }
}
