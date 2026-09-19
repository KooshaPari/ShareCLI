//! CSV rendering for operator commands (FR-007 / AC-007.79 / AC-007.82 / AC-007.83).
//!
//! Text builders that turn the `health`, `pool`, `status`, and `ps --all`
//! snapshots into the `--csv` document printed on stdout, plus the
//! gate → host_watch → pool → status companion rows appended after each body.

use anyhow::Result;
use sharecli_fleet::{agent_label_for_pid, HostProcSource};

use crate::monitoring::HostResourceWatchJson;
use crate::runtime::ProcessInfo;

use super::proc::{csv_escape_field, AgentProcRow};
use super::{fetch_operator_pool_status_siblings, HealthJson, PoolJson};

/// Append gate → host_watch → pool → status CSV companion blocks (FR-007 / AC-007.79 / AC-007.82).
pub(crate) async fn append_operator_csv_companions(
    csv: String,
    gate: &sharecli_fleet::GateStatusSnapshot,
) -> Result<String> {
    use sharecli_fleet::{PoolOperatorPanel, StatusOperatorPanel};

    let mut out = csv;
    out.push_str(&gate.format_csv_companion());
    out.push_str(&HostResourceWatchJson::capture()?.format_csv_companion());
    let (pool_json, status_json) = fetch_operator_pool_status_siblings().await?;
    let pool: PoolOperatorPanel = pool_json.into();
    let status: StatusOperatorPanel = status_json.into();
    out.push_str(&pool.format_csv_companion());
    out.push_str(&status.format_csv_companion());
    Ok(out)
}

/// Primary CSV body for `sharecli health --csv` (FR-007 / AC-007.82).
pub fn render_health_csv_body(health: &HealthJson) -> String {
    let issues = csv_escape_field(&health.issues.join(";"));
    format!(
        "record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues\n\
         health,{},{},{},{},{},{},{},{},{}\n",
        health.healthy,
        health.node_total,
        health.node_idle,
        health.node_in_use,
        health.bun_total,
        health.bun_idle,
        health.bun_in_use,
        health.max_per_type,
        issues,
    )
}

/// Primary CSV body for `sharecli pool --csv` (FR-007 / AC-007.82).
pub fn render_pool_csv_body(pool: &PoolJson) -> String {
    let issues = csv_escape_field(&pool.issues.join(";"));
    format!(
        "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues\n\
         pool,{},{},{},{},{},{},{}\n",
        pool.node_total,
        pool.node_idle,
        pool.bun_total,
        pool.bun_idle,
        pool.max_per_type,
        pool.healthy,
        issues,
    )
}

/// Primary CSV body for `sharecli status --csv` (FR-007 / AC-007.82).
pub fn render_status_csv_body(
    summary: &sharecli_fleet::StatusOperatorPanel,
    harness_rows: &[(String, usize, u64)],
    pool_status: &crate::runtime::PoolStatus,
    used_mb: u64,
    total_mb: u64,
) -> String {
    let mut out = format!(
        "record,total_processes,scanned,watched,agent_rows\n\
         status,{},{},{},{}\n",
        summary.total_processes, summary.scanned, summary.watched, summary.agent_rows,
    );
    out.push_str("\nrecord,harness,count,memory_mb\n");
    for (h, count, mem) in harness_rows {
        out.push_str(&format!("harness,{},{},{mem}\n", csv_escape_field(h), count,));
    }
    out.push_str("\nrecord,type,total,idle,max_per_type\n");
    out.push_str(&format!(
        "runtime_pool,node,{},{},{}\n",
        pool_status.node_total, pool_status.node_idle, pool_status.max_per_type,
    ));
    out.push_str(&format!(
        "runtime_pool,bun,{},{},{}\n",
        pool_status.bun_total, pool_status.bun_idle, pool_status.max_per_type,
    ));
    let pct = (used_mb * 100).checked_div(total_mb).unwrap_or(0);
    out.push_str("\nrecord,used_mb,total_mb,used_pct\n");
    out.push_str(&format!("system_memory,{used_mb},{total_mb},{pct}\n"));
    out
}

/// Primary CSV body for `sharecli ps --all --csv` (FR-007 / AC-007.83).
pub fn render_ps_all_csv_body(
    processes: &[ProcessInfo],
    proc_source: &HostProcSource,
    agents: &[AgentProcRow],
    scanned: usize,
    watched: usize,
) -> String {
    let mut out = String::from("record,pid,name,memory_mb,project,harness,agent\n");
    for proc in processes {
        out.push_str(&format!(
            "process,{},{},{},{},{},{}\n",
            proc.pid,
            csv_escape_field(&proc.name),
            proc.memory_mb,
            csv_escape_field(proc.project.as_deref().unwrap_or("-")),
            csv_escape_field(proc.harness.as_deref().unwrap_or("-")),
            csv_escape_field(agent_label_for_pid(proc_source, proc.pid)),
        ));
    }
    let total_mem: u64 = processes.iter().map(|p| p.memory_mb).sum();
    out.push_str("\nrecord,process_count,total_memory_mb\n");
    out.push_str(&format!("summary,{},{total_mem}\n", processes.len()));
    out.push_str("\nrecord,scanned,watched\n");
    out.push_str(&format!("agent_inventory,{scanned},{watched}\n"));
    out.push_str("\npid,family,comm,state,mem_rss_bytes,mem_rss,fd_count\n");
    for row in agents {
        let fd = row.fd_count.map(|n| n.to_string()).unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            row.pid,
            csv_escape_field(&row.family),
            csv_escape_field(&row.comm),
            csv_escape_field(&row.state),
            row.mem_rss_bytes,
            csv_escape_field(&row.mem_rss),
            fd,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{HealthJson, PoolJson, StatusJson};
    use crate::monitoring::HostResourceWatchJson;

    // --- render_health_csv_body ---

    #[test]
    fn render_health_csv_body_healthy() {
        let gate_snap = sharecli_fleet::GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let hw = HostResourceWatchJson::default();
        let pool = PoolJson {
            node_total: 0,
            node_idle: 0,
            bun_total: 0,
            bun_idle: 0,
            max_per_type: 0,
            healthy: true,
            issues: vec![],
            gate: gate_snap.clone(),
            host_watch: hw,
            status: None,
        };
        let status = StatusJson {
            total_processes: 0,
            agents: vec![],
            scanned: 0,
            watched: 0,
            gate: gate_snap.clone(),
            host_watch: hw,
            pool: None,
            log_location: None,
        };
        let health = HealthJson {
            healthy: true,
            node_total: 3,
            node_idle: 1,
            node_in_use: 2,
            bun_total: 5,
            bun_idle: 3,
            bun_in_use: 2,
            max_per_type: 5,
            issues: vec![],
            gate: gate_snap,
            host_watch: hw,
            pool,
            status,
        };
        let csv = render_health_csv_body(&health);
        assert!(csv.starts_with("record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues"));
        assert!(csv.contains("health,true,3,1,2,5,3,2,5,"));
    }

    #[test]
    fn render_health_csv_body_unhealthy_with_issues() {
        let gate_snap = sharecli_fleet::GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let hw = HostResourceWatchJson::default();
        let health = HealthJson {
            healthy: false,
            node_total: 0,
            node_idle: 0,
            node_in_use: 0,
            bun_total: 0,
            bun_idle: 0,
            bun_in_use: 0,
            max_per_type: 5,
            issues: vec!["port conflict".into(), "OOM".into()],
            gate: gate_snap.clone(),
            host_watch: hw,
            pool: PoolJson {
                node_total: 0,
                node_idle: 0,
                bun_total: 0,
                bun_idle: 0,
                max_per_type: 0,
                healthy: true,
                issues: vec![],
                gate: gate_snap.clone(),
                host_watch: hw,
                status: None,
            },
            status: StatusJson {
                total_processes: 0,
                agents: vec![],
                scanned: 0,
                watched: 0,
                gate: gate_snap,
                host_watch: hw,
                pool: None,
                log_location: None,
            },
        };
        let csv = render_health_csv_body(&health);
        assert!(csv.contains("health,false,"));
        assert!(csv.contains("port conflict;OOM"));
    }

    // --- render_pool_csv_body ---

    #[test]
    fn render_pool_csv_body_basic() {
        let pool = PoolJson {
            node_total: 4,
            node_idle: 2,
            bun_total: 6,
            bun_idle: 4,
            max_per_type: 10,
            healthy: true,
            issues: vec![],
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            status: None,
        };
        let csv = render_pool_csv_body(&pool);
        assert!(csv.starts_with(
            "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues"
        ));
        assert!(csv.contains("pool,4,2,6,4,10,true,"));
    }

    #[test]
    fn render_pool_csv_body_with_issues() {
        let pool = PoolJson {
            node_total: 0,
            node_idle: 0,
            bun_total: 0,
            bun_idle: 0,
            max_per_type: 5,
            healthy: false,
            issues: vec!["stale lock".into()],
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            status: None,
        };
        let csv = render_pool_csv_body(&pool);
        assert!(csv.contains("pool,0,0,0,0,5,false,"));
        assert!(csv.contains("stale lock"));
    }
}
