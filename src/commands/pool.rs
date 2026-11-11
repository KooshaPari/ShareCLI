//! `sharecli pool` operator command (FR-007 / AC-007.44, 007.65, 007.77).
//!
//! Reports the shared runtime's node/bun pool counts, health status, and
//! issues. Supports text / JSON / CSV output with optional `--watch` refresh.
//! The JSON path embeds a nested `status` sibling (AC-007.77) and the live
//! gate / host-watch / pool-status sections appear on every text and CSV tick.
//!
//! `build_pool_json` is the canonical JSON envelope builder for the pool
//! panel; it is consumed by `ps` (via `live_sections::fetch_operator_pool_status_siblings`),
//! `status` (renders as the `pool` sibling of `status --json`), `health`
//! (renders as the `pool` sibling of `health --json`), `serve`, and `main`.

use std::io::Write;
use std::time::Duration;

use anyhow::Result;

use crate::commands::live_sections::{
    capture_live_gate_host_watch, eprint_live_gate_host_watch_sections, print_live_gate_section,
    print_live_host_watch_section, print_live_pool_status_operator_sections,
};
use crate::commands::render_csv;
use crate::commands::runtime_helpers::get_shared_runtime;
use crate::commands::types::{
    emit_operator_csv_watch_frame, emit_operator_watch_footer, emit_ps_ndjson_line,
    ps_unix_ts_secs, PoolJson, PoolNdjsonLine, POOL_CSV_WATCH_FRAME_MARKER,
};

/// Build the `pool --json` payload (FR-007 / AC-007.44, status sibling AC-007.77).
pub(crate) async fn build_pool_json() -> Result<PoolJson> {
    let runtime = get_shared_runtime();
    let status = runtime.status().await;
    let health = runtime.health_check().await;
    let (gate, host_watch) = capture_live_gate_host_watch()?;
    Ok(PoolJson {
        node_total: status.node_total,
        node_idle: status.node_idle,
        bun_total: status.bun_total,
        bun_idle: status.bun_idle,
        max_per_type: status.max_per_type,
        healthy: health.healthy,
        issues: health.issues,
        gate,
        host_watch,
        status: None,
    })
}

/// Render one pool snapshot (one-shot or watch cycle).
///
/// One-shot `pool --json` MUST NOT print gate/host_watch stderr companions (AC-007.44).
async fn render_pool_once(json: bool, csv: bool, ndjson: bool) -> Result<()> {
    if json {
        let mut payload = build_pool_json().await?;
        payload.status = Some(Box::new(crate::commands::ps::build_status_json().await?));
        if ndjson {
            let line = PoolNdjsonLine { ts: ps_unix_ts_secs(), snapshot: payload };
            emit_ps_ndjson_line(&line)?;
            eprint_live_gate_host_watch_sections()?;
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        return Ok(());
    }

    if csv {
        let payload = build_pool_json().await?;
        let body = render_csv::render_pool_csv_body(&payload);
        let csv_out = render_csv::append_operator_csv_companions(body, &payload.gate).await?;
        print!("{csv_out}");
        return Ok(());
    }

    let runtime = get_shared_runtime();
    let status = runtime.status().await;
    let health = runtime.health_check().await;

    println!("=== Shared Runtime Pool Status ===\n");
    println!("{:<10} {:<10} {:<10} {:<10}", "TYPE", "TOTAL", "IDLE", "MAX");
    println!("{}", "-".repeat(40));
    println!(
        "{:<10} {:<10} {:<10} {:<10}",
        "node", status.node_total, status.node_idle, status.max_per_type
    );
    println!(
        "{:<10} {:<10} {:<10} {:<10}",
        "bun", status.bun_total, status.bun_idle, status.max_per_type
    );
    println!("\nMax per type: {}", status.max_per_type);

    // Health check
    println!("\n=== Health Check ===");
    if health.healthy {
        println!("Status: HEALTHY");
    } else {
        println!("Status: DEGRADED");
    }
    if !health.issues.is_empty() {
        println!("\nIssues:");
        for issue in &health.issues {
            println!("  - {}", issue);
        }
    }

    print_live_gate_section()?;
    print_live_host_watch_section()?;
    print_live_pool_status_operator_sections().await?;

    Ok(())
}

/// Show pool status (FR-007 / AC-007.44, 007.65).
pub async fn pool_status(json: bool, csv: bool, watch: Option<u64>) -> Result<()> {
    if csv && json {
        anyhow::bail!("--csv cannot be combined with --json");
    }
    match watch {
        None => render_pool_once(json, csv, false).await,
        Some(interval_secs) => {
            if interval_secs == 0 {
                anyhow::bail!("--watch interval must be >= 1 second");
            }
            let ndjson = json;
            let csv_watch = csv;
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    emit_operator_csv_watch_frame(POOL_CSV_WATCH_FRAME_MARKER);
                }
                render_pool_once(json, csv, ndjson).await?;
                if !ndjson {
                    std::io::stdout().flush()?;
                }
                emit_operator_watch_footer(interval_secs, ndjson, csv_watch);
                let idle = cycle_start.elapsed();
                let period = Duration::from_secs(interval_secs);
                let sleep_for = period.saturating_sub(idle);
                tokio::select! {
                    _ = tokio::time::sleep(sleep_for) => {},
                    _ = tokio::signal::ctrl_c() => {
                        if ndjson {
                            eprintln!("\nExiting watch mode.");
                        } else {
                            println!("\nExiting watch mode.");
                        }
                        break;
                    }
                }
            }
            Ok(())
        }
    }
}
