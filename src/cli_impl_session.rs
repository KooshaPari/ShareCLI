//! Concrete impls for the `session` and `surface` subcommand families.
//!
//! These are the only `Commands::Session { cmd }` / `Commands::Surface { cmd }`
//! handlers — they own the Ghostty control socket plumbing, durable ledger
//! reads, and the sidecar-format IO that the rest of the CLI does not need
//! to know about.

use anyhow::{Context, Result};
use sharecli::session::GhosttyControlClient;
use sharecli_session::{
    LayoutSnapshot, RecoveryExecutor, SessionObservation, SessionService, SessionStore,
    SidecarStateProvider, SurfaceObservationScanner,
};
use std::io::Write;

use crate::subcommands::session::SessionCmd;
use crate::subcommands::surface::SurfaceCmd;

pub fn session_cmd(cmd: &SessionCmd) -> Result<()> {
    let (db, operation) = match cmd {
        SessionCmd::List { db } => (db.clone(), None),
        SessionCmd::Inspect { id, db } => (db.clone(), Some(id.as_str())),
        SessionCmd::RecoveryPlan { db, .. } => (db.clone(), None),
        SessionCmd::Observe { db, .. } => (db.clone(), None),
        SessionCmd::Observations { db, .. } => (db.clone(), None),
        SessionCmd::Compact { db } => (db.clone(), None),
        SessionCmd::Recover { db, .. } => (db.clone(), None),
        SessionCmd::LayoutList { db } => (db.clone(), None),
        SessionCmd::LayoutInspect { id, db } => (db.clone(), Some(id.as_str())),
        SessionCmd::LayoutSave { db, .. } => (db.clone(), None),
        SessionCmd::Register { .. } => (None, None),
        SessionCmd::Watch { db, .. } => (db.clone(), None),
    };
    let path = db.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("sharecli")
            .join("sessions.sqlite")
    });
    let store = SessionStore::open(path)?;
    if let SessionCmd::Observe { input, .. } = cmd {
        let observation: SessionObservation =
            serde_json::from_str(&std::fs::read_to_string(input)?)?;
        let sequence = store.append_observation(&observation)?;
        println!(
            "{}",
            serde_json::json!({"sequence": sequence, "surface_id": observation.surface.id})
        );
        return Ok(());
    }
    if let SessionCmd::LayoutSave { input, .. } = cmd {
        let snapshot: LayoutSnapshot = serde_json::from_str(&std::fs::read_to_string(input)?)?;
        store.save_layout(&snapshot)?;
        println!("{}", serde_json::json!({"id": snapshot.id}));
        return Ok(());
    }
    if let SessionCmd::LayoutList { .. } = cmd {
        println!("{}", serde_json::to_string_pretty(&store.list_layouts()?)?);
        return Ok(());
    }
    if let SessionCmd::LayoutInspect { id, .. } = cmd {
        println!("{}", serde_json::to_string_pretty(&store.get_layout(id)?)?);
        return Ok(());
    }
    if let SessionCmd::Register { surface_id, harness, session_id, pid, state_sidecar } = cmd {
        let surface_id = surface_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("--surface-id or GHOSTTY_SURFACE_ID is required"))?;
        if harness.trim().is_empty() {
            anyhow::bail!("--harness must not be empty");
        }
        if session_id.trim().is_empty() {
            anyhow::bail!("--session-id must not be empty");
        }
        let path = state_sidecar
            .as_deref()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(default_state_sidecar);
        let record = sharecli_session::SidecarRecord {
            surface_id: surface_id.to_owned(),
            harness: harness.clone(),
            session_id: session_id.clone(),
            pid: *pid,
        };
        sharecli_session::append_record(&path, &record)?;
        println!("{}", serde_json::to_string(&record)?);
        return Ok(());
    }
    if let SessionCmd::Watch { interval_seconds, once, socket, token, state_sidecar, .. } = cmd {
        run_session_watch(
            &store,
            socket,
            token.as_deref(),
            state_sidecar.as_deref(),
            *interval_seconds,
            *once,
        )?;
        return Ok(());
    }
    let service = SessionService::new(store);
    let value = match cmd {
        SessionCmd::List { .. } => serde_json::to_value(service.list()?)?,
        SessionCmd::Inspect { .. } => {
            serde_json::to_value(service.inspect(operation.expect("id"))?)?
        }
        SessionCmd::RecoveryPlan { max_age_seconds, .. } => serde_json::to_value(
            service.recovery_plan(chrono::Duration::seconds(*max_age_seconds as i64))?,
        )?,
        SessionCmd::Observations { surface_id, .. } => {
            serde_json::to_value(service.observations(surface_id.as_deref())?)?
        }
        SessionCmd::Compact { .. } => serde_json::json!({
            "removed": service.compact_observations()?
        }),
        SessionCmd::Recover { execute, max_parallel, max_age_seconds, .. } => {
            let sessions =
                service.recovery_plan(chrono::Duration::seconds(*max_age_seconds as i64))?;
            let executor = RecoveryExecutor::new(*max_parallel);
            let results =
                if *execute { executor.execute(&sessions) } else { executor.dry_run(&sessions) };
            serde_json::to_value(results)?
        }
        SessionCmd::Observe { .. } => unreachable!("observe handled before service dispatch"),
        SessionCmd::LayoutSave { .. } => {
            unreachable!("layout save handled before service dispatch")
        }
        SessionCmd::LayoutList { .. } | SessionCmd::LayoutInspect { .. } => {
            unreachable!("layout operation handled before service dispatch")
        }
        SessionCmd::Register { .. } => unreachable!("register handled before service dispatch"),
        SessionCmd::Watch { .. } => unreachable!("watch handled before service dispatch"),
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn run_session_watch(
    store: &SessionStore,
    socket: &std::path::Path,
    token: Option<&str>,
    state_sidecar: Option<&std::path::Path>,
    interval_seconds: u64,
    once: bool,
) -> Result<()> {
    let client = GhosttyControlClient::new(socket, token.map(str::to_owned));
    let sidecar = SidecarStateProvider::new(
        state_sidecar.map(std::path::Path::to_path_buf).unwrap_or_else(default_state_sidecar),
    );
    let interval = std::time::Duration::from_secs(interval_seconds.clamp(1, 3600));
    loop {
        match observe_ghostty_surfaces(store, &client, &sidecar) {
            Ok(count) => eprintln!("sharecli session watch: recorded {count} surface(s)"),
            Err(error) => eprintln!("sharecli session watch: degraded: {error:#}"),
        }
        if once {
            return Ok(());
        }
        std::thread::sleep(interval);
    }
}

fn observe_ghostty_surfaces(
    store: &SessionStore,
    client: &GhosttyControlClient,
    state: &SidecarStateProvider,
) -> Result<usize> {
    let observed_at = chrono::Utc::now().to_rfc3339();
    let report = SurfaceObservationScanner::new(client, state, store).scan(&observed_at)?;
    for failure in &report.failures {
        eprintln!("sharecli session watch: {} unavailable: {}", failure.surface_id, failure.error);
    }
    Ok(report.recorded)
}

pub fn default_state_sidecar() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sharecli/session-sidecar.jsonl")
}

pub fn surface_cmd(cmd: &SurfaceCmd) -> Result<()> {
    match cmd {
        SurfaceCmd::Send { socket, surface_id, text, bytes, token } => {
            let client = GhosttyControlClient::new(socket, token.clone());
            if let Some(text) = text {
                client.send_text(surface_id, text)?;
            } else if let Some(bytes) = bytes {
                let bytes = bytes
                    .split(',')
                    .map(|value| value.trim().parse::<u8>())
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("--bytes must be a comma-separated list of u8 values")?;
                client.request(
                    "surface.io.send",
                    serde_json::json!({"surface_id": surface_id, "bytes": bytes}),
                )?;
            } else {
                anyhow::bail!("surface send requires --text or --bytes");
            }
            println!("{{\"ok\":true}}");
        }
        SurfaceCmd::Read { socket, surface_id, max_bytes, token } => {
            let client = GhosttyControlClient::new(socket, token.clone());
            println!(
                "{}",
                serde_json::to_string_pretty(&client.read_surface(surface_id, *max_bytes)?)?
            );
        }
        SurfaceCmd::Resize { socket, surface_id, rows, cols, token } => {
            let client = GhosttyControlClient::new(socket, token.clone());
            client.resize(surface_id, *rows, *cols)?;
            println!("{{\"ok\":true}}");
        }
        SurfaceCmd::Watch {
            socket,
            surface_id,
            from_seq,
            max_chunk_bytes,
            queue_capacity,
            once,
            token,
        } => {
            #[cfg(unix)]
            {
                let client = GhosttyControlClient::new(socket, token.clone());
                let mut subscription = client.subscribe_surface(
                    surface_id.as_deref(),
                    *from_seq,
                    *max_chunk_bytes,
                    *queue_capacity,
                )?;
                loop {
                    let event = subscription.next_event()?;
                    println!("{}", serde_json::to_string(&event)?);
                    std::io::stdout().flush().ok();
                    if *once {
                        subscription.unsubscribe()?;
                        break;
                    }
                }
            }
            #[cfg(not(unix))]
            {
                let _ =
                    (socket, surface_id, from_seq, max_chunk_bytes, queue_capacity, once, token);
                anyhow::bail!("surface watch requires a Unix control socket (macOS/Linux)")
            }
        }
        SurfaceCmd::LayoutSnapshot { socket, output, token } => {
            let client = GhosttyControlClient::new(socket, token.clone());
            let snapshot = client.snapshot_layout()?;
            let encoded = serde_json::to_string_pretty(&snapshot)?;
            if let Some(output) = output {
                write_json_file(output, &encoded)?;
                println!(
                    "{{\"ok\":true,\"path\":{}}}",
                    serde_json::to_string(&output.display().to_string())?
                );
            } else {
                println!("{encoded}");
            }
        }
        SurfaceCmd::LayoutRestore { input, socket, token } => {
            let snapshot: LayoutSnapshot = serde_json::from_str(&std::fs::read_to_string(input)?)?;
            let client = GhosttyControlClient::new(socket, token.clone());
            let report = client.restore_layout(&snapshot)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}

/// Write a JSON artifact without exposing a shell or partially replacing an
/// existing snapshot. The rename is atomic on the same filesystem.
pub fn write_json_file(path: &std::path::Path, contents: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_extension(format!("sharecli-tmp-{}", std::process::id()));
    std::fs::write(&tmp, format!("{contents}\n"))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("atomically replace layout snapshot at {}", path.display()))?;
    Ok(())
}
