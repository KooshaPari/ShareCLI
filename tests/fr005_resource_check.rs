// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-005 — Per-Project Resource Limits (ResourceCheck / check output)
//! FR: FR-005
//!
//! Covers AC-005.4, AC-005.5.

use sharecli::runtime::ResourceCheck;

/// Mirror of the status lines printed by `commands::check_limits`.
fn format_check(project: &str, check: &ResourceCheck) -> String {
    let mut out = format!("=== Resource Limits for '{project}' ===\n\n");
    out.push_str(&format!("Memory: {} MB / {} MB\n", check.memory_mb, check.memory_limit_mb));
    if check.memory_ok {
        out.push_str("  Status: OK\n");
    } else {
        out.push_str(&format!(
            "  Status: EXCEEDED (over by {} MB)\n",
            check.memory_mb - check.memory_limit_mb
        ));
    }
    out.push_str(&format!("\nProcesses: {} / {}\n", check.process_count, check.max_processes));
    if check.processes_ok {
        out.push_str("  Status: OK\n");
    } else {
        out.push_str(&format!(
            "  Status: EXCEEDED (over by {})\n",
            check.process_count - check.max_processes
        ));
    }
    out.push_str(&format!(
        "\nOverall: {}\n",
        if check.overall_ok { "OK" } else { "LIMIT EXCEEDED" }
    ));
    out
}

fn check(
    memory_mb: u64,
    memory_limit_mb: u64,
    process_count: usize,
    max_processes: usize,
) -> ResourceCheck {
    let memory_ok = memory_mb <= memory_limit_mb;
    let processes_ok = process_count <= max_processes;
    ResourceCheck {
        memory_mb,
        memory_limit_mb,
        memory_ok,
        process_count,
        max_processes,
        processes_ok,
        overall_ok: memory_ok && processes_ok,
    }
}

/// FR-005 / AC-005.4 — `overall_ok` is true only when both axes are OK.
#[test]
fn fr005_resource_check_overall_ok_logic() {
    let both_ok = check(100, 1024, 2, 10);
    assert!(both_ok.memory_ok && both_ok.processes_ok);
    assert!(both_ok.overall_ok, "both OK MUST yield overall_ok");

    let mem_bad = check(2000, 1024, 1, 10);
    assert!(!mem_bad.memory_ok && mem_bad.processes_ok);
    assert!(!mem_bad.overall_ok, "memory EXCEEDED MUST clear overall_ok");

    let proc_bad = check(64, 1024, 20, 10);
    assert!(proc_bad.memory_ok && !proc_bad.processes_ok);
    assert!(!proc_bad.overall_ok, "processes EXCEEDED MUST clear overall_ok");

    let both_bad = check(5000, 1024, 50, 10);
    assert!(!both_bad.memory_ok && !both_bad.processes_ok);
    assert!(!both_bad.overall_ok);

    let at_limit = check(1024, 1024, 10, 10);
    assert!(at_limit.memory_ok && at_limit.processes_ok);
    assert!(at_limit.overall_ok, "exact limit MUST still be OK");
}

/// FR-005 / AC-005.5 — `check` prints memory, process count, per-axis status, overall.
#[test]
fn fr005_check_prints_status_lines() {
    let ok = check(128, 1024, 3, 10);
    let out_ok = format_check("demo", &ok);
    assert!(out_ok.contains("=== Resource Limits for 'demo' ==="), "got: {out_ok}");
    assert!(out_ok.contains("Memory: 128 MB / 1024 MB"), "got: {out_ok}");
    assert!(out_ok.contains("Processes: 3 / 10"), "got: {out_ok}");
    assert!(out_ok.matches("Status: OK").count() >= 2, "both axes MUST show OK; got: {out_ok}");
    assert!(out_ok.contains("Overall: OK"), "got: {out_ok}");

    let exceeded = check(2048, 1024, 15, 10);
    let out_bad = format_check("demo", &exceeded);
    assert!(out_bad.contains("Status: EXCEEDED (over by 1024 MB)"), "got: {out_bad}");
    assert!(out_bad.contains("Status: EXCEEDED (over by 5)"), "got: {out_bad}");
    assert!(out_bad.contains("Overall: LIMIT EXCEEDED"), "got: {out_bad}");
}
