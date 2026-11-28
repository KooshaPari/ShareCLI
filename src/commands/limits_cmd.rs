// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `limits` / `limits set` / `limits check` / `optimize` operator commands.
//!
//! These are the only surfaces that read or write the persisted limits
//! document, so what an operator sets in one invocation is what the next
//! invocation checks. Detection goes through
//! [`crate::runtime::limits_store::detect`] — the same host path
//! `sharecli proc` uses — and every verdict is either a measurement
//! (`OK` / `OVER`) or an explicit [`LimitVerdict::NoData`].

use anyhow::{bail, Result};
use serde_json::json;

use crate::config::{Config, ProjectLimitsConfig};
use crate::runtime::{
    detect, evaluate, resolve, suggested_entry, suggestions, LimitEntry, LimitRow, LimitSource,
    LimitsFile, LimitsStore,
};

/// Every project name the table should cover: registered projects first, then
/// projects that only exist in the limits file.
fn table_projects(cfg: &Config, file: &LimitsFile) -> Vec<(String, Option<String>)> {
    let mut pairs: Vec<(String, Option<String>)> =
        cfg.projects.iter().map(|(name, path)| (name.clone(), Some(path.clone()))).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    for name in file.projects.keys() {
        if !pairs.iter().any(|(existing, _)| existing == name) {
            pairs.push((name.clone(), None));
        }
    }
    pairs
}

fn defaults_from(cfg: &Config) -> ProjectLimitsConfig {
    cfg.project_limits.clone()
}

/// Print the effective limit table (defaults + overrides). Never fails on a
/// missing file: an absent `limits.json` is simply "no overrides yet".
pub fn show_effective_limits() -> Result<()> {
    let cfg = Config::load()?;
    let defaults = defaults_from(&cfg);
    let store = LimitsStore::new();
    let file = store.load()?;

    println!("EFFECTIVE LIMITS  (source: {})", store.path().display());
    match file.global {
        Some(entry) => println!(
            "{:<24} {:>9} {:>11}  {}",
            "(global)",
            entry.memory_limit_mb,
            entry.max_processes,
            LimitSource::GlobalOverride.label()
        ),
        None => println!(
            "{:<24} {:>9} {:>11}  {}",
            "(global)",
            defaults.memory_limit_mb,
            defaults.max_processes,
            LimitSource::Default.label()
        ),
    }

    let pairs = table_projects(&cfg, &file);
    if pairs.is_empty() {
        println!("no projects registered (sharecli project add <name> <path>)");
        return Ok(());
    }
    println!("{:<24} {:>9} {:>11}  {}", "PROJECT", "MEM(MB)", "MAX_PROCS", "SOURCE");
    for (name, _path) in &pairs {
        let (entry, source) = resolve(&file, name, &defaults);
        println!(
            "{:<24} {:>9} {:>11}  {}",
            name,
            entry.memory_limit_mb,
            entry.max_processes,
            source.label()
        );
    }
    Ok(())
}

/// Show one project's effective limit and where it came from (read-only).
pub fn show_one(project: &str) -> Result<()> {
    let cfg = Config::load()?;
    let defaults = defaults_from(&cfg);
    let store = LimitsStore::new();
    let file = store.load()?;
    let (entry, source) = resolve(&file, project, &defaults);
    println!("Project: {}", project);
    println!("  Memory limit: {} MB", entry.memory_limit_mb);
    println!("  Max processes: {}", entry.max_processes);
    println!("  Source: {} ({})", source.label(), store.path().display());
    Ok(())
}

/// Write one limit target (a project or the global default) and persist it.
pub fn set_limit(
    project: Option<&str>,
    global: bool,
    mem_mb: Option<u64>,
    max_procs: Option<usize>,
) -> Result<()> {
    if global && project.is_some() {
        bail!("choose one target: pass a project name OR --global, not both");
    }
    if !global && project.is_none() {
        bail!("limits set needs a project name or --global");
    }
    if mem_mb.is_none() && max_procs.is_none() {
        bail!("limits set needs --mem-mb and/or --max-procs");
    }

    let cfg = Config::load()?;
    let defaults = defaults_from(&cfg);
    let store = LimitsStore::new();
    let mut file = store.load()?;

    let base = if global {
        file.global.unwrap_or_else(|| LimitEntry::from_defaults(&defaults))
    } else {
        resolve(&file, project.expect("project checked above"), &defaults).0
    };
    let entry = LimitEntry {
        memory_limit_mb: mem_mb.unwrap_or(base.memory_limit_mb),
        max_processes: max_procs.unwrap_or(base.max_processes),
    };

    let target = if global {
        file.global = Some(entry);
        "(global)".to_string()
    } else {
        let name = project.expect("project checked above").to_string();
        file.projects.insert(name.clone(), entry);
        name
    };

    store.save(&file)?;
    println!(
        "Set limits for {}: {} MB / {} procs  → {}",
        target,
        entry.memory_limit_mb,
        entry.max_processes,
        store.path().display()
    );
    Ok(())
}

/// One-line measurement footer shared by `limits check` and `optimize`.
fn detection_footer(snapshot: &crate::runtime::DetectionSnapshot) -> String {
    if !snapshot.is_available() {
        return "detection: UNAVAILABLE (host process table could not be read)".to_string();
    }
    format!(
        "detection: {} agents, {} MB total, {} families",
        snapshot.agent_count,
        snapshot.total_memory_mb,
        snapshot.families.len()
    )
}

/// Compare detected usage against effective limits. Returns true when any row
/// is `OVER` (the caller decides the exit code via `--fail-on-over`).
pub fn check(project: Option<&str>, as_json: bool) -> Result<bool> {
    let cfg = Config::load()?;
    let defaults = defaults_from(&cfg);
    let store = LimitsStore::new();
    let file = store.load()?;

    let pairs = table_projects(&cfg, &file);
    let wanted: Vec<String> = match project {
        Some(one) => vec![one.to_string()],
        None => pairs.iter().map(|(name, _)| name.clone()).collect(),
    };
    let detect_pairs: Vec<(String, Option<String>)> = match project {
        Some(one) => match pairs.iter().find(|(name, _)| name == one) {
            Some(pair) => vec![pair.clone()],
            None => vec![(one.to_string(), None)],
        },
        None => pairs.clone(),
    };
    let snapshot = detect(&detect_pairs);
    let rows = evaluate(&wanted, &file, &defaults, &snapshot);
    let any_over = rows.iter().any(|row| row.verdict == crate::runtime::LimitVerdict::Over);

    if as_json {
        let payload = json!({
            "limits_file": store.path().display().to_string(),
            "detection": if snapshot.is_available() { "available" } else { "unavailable" },
            "agents": snapshot.agent_count,
            "total_memory_mb": snapshot.total_memory_mb,
            "families": snapshot.families.iter().map(|f| json!({
                "family": f.family,
                "procs": f.procs,
                "memory_mb": f.memory_mb,
            })).collect::<Vec<_>>(),
            "rows": rows.iter().map(row_json).collect::<Vec<_>>(),
            "any_over": any_over,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(any_over);
    }

    println!("LIMITS CHECK  (limits: {})", store.path().display());
    println!(
        "{:<24} {:>9} {:>11} {:>9} {:>9}  {:<8} {}",
        "PROJECT", "LIMIT_MB", "LIMIT_PROCS", "DET_MB", "DET_PROCS", "VERDICT", "SOURCE"
    );
    for row in &rows {
        println!(
            "{:<24} {:>9} {:>11} {:>9} {:>9}  {:<8} {}",
            row.project,
            row.limit.memory_limit_mb,
            row.limit.max_processes,
            row.usage.as_ref().map(|u| u.memory_mb.to_string()).unwrap_or_else(|| "-".into()),
            row.usage.as_ref().map(|u| u.procs.to_string()).unwrap_or_else(|| "-".into()),
            row.verdict.label(),
            row.source.label()
        );
    }
    println!();
    println!("{}", detection_footer(&snapshot));
    if rows.iter().all(|r| r.verdict == crate::runtime::LimitVerdict::NoData) {
        println!("note: NO_DATA means detection attributed no process to the row; it is not a pass");
        if snapshot.is_available() {
            println!(
                "note: attribution matches a project name (3+ chars) or project path (6+ chars) in an agent's argv"
            );
        }
    }
    Ok(any_over)
}

fn row_json(row: &LimitRow) -> serde_json::Value {
    json!({
        "project": row.project,
        "limit_memory_mb": row.limit.memory_limit_mb,
        "limit_max_processes": row.limit.max_processes,
        "detected_memory_mb": row.usage.as_ref().map(|u| u.memory_mb),
        "detected_procs": row.usage.as_ref().map(|u| u.procs),
        "verdict": row.verdict.label(),
        "source": row.source.label(),
        "over_memory_mb": row.over_memory_mb(),
        "over_processes": row.over_processes(),
    })
}

/// `optimize`: emit suggestions derived from the measurement, or exactly
/// `no suggestions`. With `apply`, persist the measured global default when
/// none is persisted yet (the only write this command ever makes).
pub fn optimize(apply: bool) -> Result<()> {
    let cfg = Config::load()?;
    let defaults = defaults_from(&cfg);
    let store = LimitsStore::new();
    let mut file = store.load()?;

    let pairs = table_projects(&cfg, &file);
    let wanted: Vec<String> = pairs.iter().map(|(name, _)| name.clone()).collect();
    let snapshot = detect(&pairs);
    let rows = evaluate(&wanted, &file, &defaults, &snapshot);
    let list = suggestions(&file, &defaults, &snapshot, &rows);

    println!("{}", detection_footer(&snapshot));
    if list.is_empty() {
        println!("no suggestions");
    } else {
        for suggestion in &list {
            println!("- {}", suggestion);
        }
    }

    if apply {
        if file.global.is_none() && snapshot.agent_count > 0 {
            let entry = suggested_entry(snapshot.total_memory_mb, snapshot.agent_count);
            file.global = Some(entry);
            store.save(&file)?;
            println!(
                "applied: global limit {} MB / {} procs → {}",
                entry.memory_limit_mb,
                entry.max_processes,
                store.path().display()
            );
        } else {
            println!("applied: nothing to write (no measured global suggestion)");
        }
    }
    Ok(())
}
