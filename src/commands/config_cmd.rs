//! `sharecli config` and `sharecli project` operator commands.
//!
//! `config` covers init / validate / show / get / set against the global TOML
//! config; `project` covers project-registry add / remove / list / show /
//! discover / generate / start / stop / restart / status. The project-group
//! start / stop / status helpers are also kept here because they share the
//! same `ProcessFilter::ByProject` resolution path and the project-group
//! test suite (now in `commands::tests`).

use std::path::PathBuf;

use anyhow::Result;

use crate::commands::config_edit;
use crate::commands::process_ops::{
    expand_path, force_kill_preview, force_kill_requires_confirmation,
};
use crate::config::{self, Config, ConfigCmd, ProjectCmd};
use crate::progress::StepProgress;
use crate::runtime::{ProcessFilter, ProcessPool};

/// Configuration management
pub fn config(cfg_cmd: &ConfigCmd) -> Result<()> {
    match cfg_cmd {
        ConfigCmd::Init => {
            Config::init()?;
            println!("Configuration initialized.");
        }
        ConfigCmd::Validate => {
            let cfg = Config::load()?;
            println!("Configuration is valid.");
            println!("  Projects: {}", cfg.projects.len());
        }
        ConfigCmd::Show => {
            let cfg = Config::load()?;
            let serialized = toml::to_string_pretty(&cfg)?;
            println!("{}", serialized);
        }
        ConfigCmd::Get { key } => {
            let cfg = Config::load()?;
            let root = serde_json::to_value(&cfg)?;
            let parts = config_edit::split_path(key)?;
            match config_edit::lookup(&root, &parts) {
                Some(value) => println!("{}", config_edit::render(value)),
                None => anyhow::bail!("unknown configuration key: {key}"),
            }
        }
        ConfigCmd::Set { key, value } => {
            let cfg = Config::load()?;
            let mut root = serde_json::to_value(&cfg)?;
            let parts = config_edit::split_path(key)?;

            let existing = config_edit::lookup(&root, &parts).cloned();
            let new = config_edit::coerce(existing.as_ref(), value);
            let rendered = config_edit::render(&new);
            config_edit::assign(&mut root, &parts, new)?;

            let candidate: Config = serde_json::from_value(root)?;

            // Never persist a configuration the startup gate would reject.
            let errors = crate::config_validator::validate_config(&candidate);
            if !errors.is_empty() {
                crate::config_validator::report_and_exit(&errors);
            }

            candidate.save()?;
            println!("{key} = {rendered}");
        }
    }
    Ok(())
}

/// Project management (async — bulk ops need an async runtime)
pub async fn project(proj_cmd: &ProjectCmd) -> Result<()> {
    match proj_cmd {
        ProjectCmd::Add { name, path } => {
            let mut cfg = Config::load()?;
            cfg.projects.insert(name.clone(), expand_path(path));
            cfg.save()?;
            println!("Added project '{}'", name);
        }
        ProjectCmd::Remove { name } => {
            let mut cfg = Config::load()?;
            if cfg.projects.remove(name).is_some() {
                cfg.save()?;
                println!("Removed project '{}'", name);
            }
        }
        ProjectCmd::List => {
            let cfg = Config::load()?;
            if cfg.projects.is_empty() {
                println!("No projects registered. Run 'sharecli project discover'.");
            } else {
                println!("Registered Projects:");
                for (name, path) in &cfg.projects {
                    println!("  {} -> {}", name, path);
                }
            }
        }
        ProjectCmd::Show { name } => {
            let cfg = Config::load()?;
            if let Some(path) = cfg.projects.get(name) {
                println!("Project: {}", name);
                println!("Path:    {}", path);
                println!("Exists:  {}", std::path::Path::new(path).exists());
            }
        }
        ProjectCmd::Discover { path } => {
            let cfg = config::global();
            let base =
                PathBuf::from(expand_path(path.as_deref().unwrap_or(&cfg.paths.discovery_path)));
            println!("Scanning {:?} for projects...", base);

            let mut found = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && p.join(".git").exists() {
                        let name =
                            p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                        found.push((name, p.to_string_lossy().to_string()));
                    }
                }
            }

            println!("\nFound {} projects:", found.len());
            for (name, path) in &found {
                println!("  {} -> {}", name, path);
            }
        }
        ProjectCmd::Generate { output } => {
            let cfg = config::global();
            let out_path = PathBuf::from(expand_path(
                output.as_deref().unwrap_or(&cfg.paths.default_compose_output),
            ));

            let sharewei_port = cfg.port.sharewei_port;
            let mut yaml = format!("# Generated by sharecli\nversion: \"0.5\"\n\nenv:\n  - SHAREWEI_PORT={}\n\nservices:\n", sharewei_port);

            for name in cfg.projects.keys() {
                yaml.push_str(&format!(
                    r#"  {}-agent:
    command: sharecli run --harness {} --project {}
    depends_on: {{}}
    log_location: .sharecli/logs/{}.log
    readiness_probe:
      exec:
        command: sharecli health --harness {}
        initial_delay_seconds: 5
        period_seconds: 10
        failure_threshold: 3

"#,
                    name, name, name, name, name
                ));
            }

            std::fs::write(&out_path, &yaml)?;
            println!("Generated process-compose.yml with {} services", cfg.projects.len());
            println!("Written to: {:?}", out_path);
        }
        ProjectCmd::Start { name, harness } => {
            project_group_start(name, harness.as_deref()).await?;
        }
        ProjectCmd::Stop { name, force, yes } => {
            project_group_stop(name, *force, *yes).await?;
        }
        ProjectCmd::Restart { name, harness, force, yes } => {
            project_group_stop(name, *force, *yes).await?;
            project_group_start(name, harness.as_deref()).await?;
        }
        ProjectCmd::Status { name, json } => {
            project_group_status(name, *json).await?;
        }
    }
    Ok(())
}

/// Start all stopped processes for a project group.
///
/// Spawns a process in the project's configured directory.  If `harness` is
/// `None` the function defaults to `"sh"` so that there is always something
/// runnable without additional flags.
async fn project_group_start(name: &str, harness: Option<&str>) -> Result<()> {
    let cfg = Config::load()?;
    let project_path = if let Some(path) = cfg.projects.get(name) {
        PathBuf::from(expand_path(path))
    } else {
        anyhow::bail!("Unknown project: '{}'. Add with 'sharecli project add <name> <path>'", name);
    };

    if !project_path.exists() {
        anyhow::bail!("Project path does not exist: {:?}", project_path);
    }

    let harness_name = harness.unwrap_or("sh");
    let pool = ProcessPool::new();

    println!("Starting '{}' harness for project group '{}'...", harness_name, name);
    let info = pool
        .spawn(
            harness_name,
            &[],
            Some(project_path),
            Some(name.to_string()),
            Some(harness_name.to_string()),
        )
        .await?;

    println!("Affected: 1 process started. PID {} ({})", info.pid, info.name);
    Ok(())
}

/// Stop all running processes in a project group.
///
/// Returns the number of processes killed.  Collects failures and reports
/// them after attempting every process so that a single bad PID does not
/// prevent the rest from being stopped.
async fn project_group_stop(name: &str, force: bool, yes: bool) -> Result<()> {
    let pool = ProcessPool::new();
    let processes = pool.find(ProcessFilter::ByProject(name.to_string())).await;

    if processes.is_empty() {
        println!("No running processes found for project '{}'.", name);
        return Ok(());
    }

    let total = processes.len();
    if force_kill_requires_confirmation(force, yes) {
        force_kill_preview(&format!("project '{name}'"), total);
        return Ok(());
    }
    let mut stopped = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let progress = StepProgress::new(&format!("Stopping project '{name}'"), total);
    let line_mode = progress.uses_line_output();

    for proc in &processes {
        match pool.kill(proc.pid).await {
            Ok(()) => {
                progress.inc(Some(&format!("{} ({})", proc.pid, proc.name)));
                if line_mode {
                    println!("Stopped {} ({})", proc.pid, proc.name);
                }
                stopped += 1;
            }
            Err(e) => {
                failures.push(format!("PID {} ({}): {}", proc.pid, proc.name, e));
            }
        }
    }
    progress.finish(&format!("Affected: {stopped}/{total} processes stopped"));
    if line_mode {
        println!("\nAffected: {}/{} processes stopped.", stopped, total);
    }
    if !failures.is_empty() {
        println!("Failures:");
        for f in &failures {
            println!("  - {}", f);
        }
        anyhow::bail!("{} process(es) could not be stopped", failures.len());
    }
    Ok(())
}

/// Show a status table for all processes in a project group.
async fn project_group_status(name: &str, json: bool) -> Result<()> {
    let pool = ProcessPool::new();
    let processes = pool.find(ProcessFilter::ByProject(name.to_string())).await;

    if json {
        // Emit a JSON array of process objects.
        let items: Vec<serde_json::Value> = processes
            .iter()
            .map(|p| {
                serde_json::json!({
                    "pid": p.pid,
                    "name": p.name,
                    "memory_mb": p.memory_mb,
                    "project": p.project,
                    "harness": p.harness,
                    "cmd": p.cmd,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    println!("=== Project '{}' — {} process(es) ===\n", name, processes.len());
    println!("{:<8} {:<20} {:<12} {:<15}", "PID", "NAME", "MEM(MB)", "HARNESS");
    println!("{}", "-".repeat(58));

    for proc in &processes {
        let harness = proc.harness.as_deref().unwrap_or("-");
        println!(
            "{:<8} {:<20} {:<12.1} {:<15}",
            proc.pid, proc.name, proc.memory_mb as f64, harness
        );
    }

    let total_mem: u64 = processes.iter().map(|p| p.memory_mb).sum();
    println!("\nTotal: {} processes, {} MB memory", processes.len(), total_mem);
    Ok(())
}
