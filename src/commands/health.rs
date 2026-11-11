//! `sharecli health` operator command (FR-007 / AC-007.44, 007.64, 007.77).
//!
//! Reports the shared runtime's health status, pool counts, and any current
//! issues. Supports text / JSON / CSV output with optional `--watch` refresh.
//! The JSON envelope embeds nested `pool` and `status` siblings (AC-007.77) for
//! parity with `report --format json` (AC-007.73).
//!
//! `build_health_json` is the canonical JSON envelope builder; it is consumed
//! only by `health` itself (the dashboard's `GET /health` reads runtime state
//! directly via `serve`, not through this function).

use std::io::Write;
use std::time::Duration;

use anyhow::Result;

use crate::commands::live_sections::{
    capture_live_gate_host_watch, eprint_live_gate_host_watch_sections,
    fetch_operator_pool_status_siblings, print_live_gate_section, print_live_host_watch_section,
    print_live_pool_status_operator_sections,
};
use crate::commands::render_csv;
use crate::commands::runtime_helpers::get_shared_runtime;
use crate::commands::types::{
    emit_operator_csv_watch_frame, emit_operator_watch_footer, emit_ps_ndjson_line,
    ps_unix_ts_secs, HealthJson, HealthNdjsonLine, HEALTH_CSV_WATCH_FRAME_MARKER,
};

/// Build the `health --json` payload (FR-007 / AC-007.44, pool/status AC-007.77).
async fn build_health_json() -> Result<HealthJson> {
    let runtime = get_shared_runtime();
    let pool_status = runtime.status().await;
    let health = runtime.health_check().await;
    let (gate, host_watch) = capture_live_gate_host_watch()?;
    let (pool_panel, status_panel) = fetch_operator_pool_status_siblings().await?;
    Ok(HealthJson {
        healthy: health.healthy,
        issues: health.issues,
        node_total: pool_status.node_total,
        node_idle: pool_status.node_idle,
        node_in_use: health.node_in_use,
        bun_total: pool_status.bun_total,
        bun_idle: pool_status.bun_idle,
        bun_in_use: health.bun_in_use,
        max_per_type: pool_status.max_per_type,
        gate,
        host_watch,
        pool: pool_panel,
        status: status_panel,
    })
}

/// Render one health snapshot (one-shot or watch cycle).
///
/// One-shot `health --json` MUST NOT print gate/host_watch stderr companions (AC-007.44).
async fn render_health_once(
    harness: Option<&str>,
    json: bool,
    csv: bool,
    ndjson: bool,
) -> Result<()> {
    if json {
        let payload = build_health_json().await?;
        if ndjson {
            let line = HealthNdjsonLine { ts: ps_unix_ts_secs(), snapshot: payload };
            emit_ps_ndjson_line(&line)?;
            eprint_live_gate_host_watch_sections()?;
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        return Ok(());
    }

    if csv {
        let payload = build_health_json().await?;
        let body = render_csv::render_health_csv_body(&payload);
        let csv_out = render_csv::append_operator_csv_companions(body, &payload.gate).await?;
        print!("{csv_out}");
        return Ok(());
    }

    if let Some(h) = harness {
        println!("Health probe requested for harness '{}'.", h);
        if h != "node" && h != "bun" {
            println!("Note: only the pooled node/bun runtimes are tracked currently.");
        }
    }

    let runtime = get_shared_runtime();
    let pool_status = runtime.status().await;
    let health = runtime.health_check().await;

    println!("\nShared runtime health: {}", if health.healthy { "HEALTHY" } else { "DEGRADED" });

    if !health.issues.is_empty() {
        println!("\nIssues detected:");
        for issue in &health.issues {
            println!("  - {}", issue);
        }
    } else {
        println!("No runtime issues detected.");
    }

    println!("\nPool summary:");
    println!(
        "  node: {} total, {} idle, {} in use",
        pool_status.node_total, pool_status.node_idle, health.node_in_use
    );
    println!(
        "  bun:  {} total, {} idle, {} in use",
        pool_status.bun_total, pool_status.bun_idle, health.bun_in_use
    );
    println!("\nMax per harness type: {}", pool_status.max_per_type);

    print_live_gate_section()?;
    print_live_host_watch_section()?;
    print_live_pool_status_operator_sections().await?;

    Ok(())
}

/// Run health probe for shared runtime (FR-007 / AC-007.44, 007.64).
pub async fn health(
    harness: Option<&str>,
    json: bool,
    csv: bool,
    watch: Option<u64>,
) -> Result<()> {
    if csv && json {
        anyhow::bail!("--csv cannot be combined with --json");
    }
    match watch {
        None => render_health_once(harness, json, csv, false).await,
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
                    emit_operator_csv_watch_frame(HEALTH_CSV_WATCH_FRAME_MARKER);
                }
                render_health_once(harness, json, csv, ndjson).await?;
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
