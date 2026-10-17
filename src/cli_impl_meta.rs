//! Concrete impls for the meta (man / list / version / NO_COLOR), fleet
//! (status / register), and proc-compose subcommand families.

use anyhow::Result;
use clap::CommandFactory;

use crate::subcommands::proc_compose::ProcComposeCmd;
use crate::subcommands::root::Cli;
use crate::theme;

/// Returns true when the NO_COLOR environment variable is set (per https://no-color.org).
pub fn is_no_color() -> bool {
    std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty())
}

/// `sharecli man` — emit sharecli(1) via clap_mangen (C09 L81.13).
pub fn cli_man(install: bool) -> Result<()> {
    use std::io::Write;

    use clap_mangen::Man;

    let man = Man::new(Cli::command());
    let mut buffer: Vec<u8> = Vec::new();
    man.render(&mut buffer).map_err(|e| anyhow::anyhow!("man page render failed: {e}"))?;
    let rendered = String::from_utf8(buffer).map_err(|e| anyhow::anyhow!("man page utf-8: {e}"))?;

    if install {
        let path = std::path::Path::new("share/man/man1/sharecli.1");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &rendered)?;
        println!("Wrote {}", path.display());
        return Ok(());
    }

    std::io::stdout()
        .write_all(rendered.as_bytes())
        .map_err(|e| anyhow::anyhow!("man page stdout: {e}"))?;
    Ok(())
}

/// `sharecli list` — enumerate CLI surfaces (cast subcommands + utility modules).
///
/// Backbone-2 family: pulse-green (#3fb950) for headers, amber (#d29922) for
/// accent markers. No external deps; pure introspection of the typed subcommand
pub fn cli_list(as_json: bool) -> Result<()> {
    let cast_modules: &[(&str, &str)] = &[
        ("register", "Register a pane: `cast register <name> <address>`"),
        ("unregister", "Unregister a pane by name"),
        ("list", "List all registered panes"),
        ("send", "Send text to a registered pane (`<name> [file]`)"),
        ("where", "Show the on-disk path of the pane-map file"),
    ];

    let mesh_modules: &[(&str, &str)] = &[
        ("status", "Show Maildir queue depth (`mesh status --queue <path>`)"),
        (
            "reclaim",
            "Return in-flight tasks for an owner (`mesh reclaim --queue <path> --owner <id>`)",
        ),
    ];

    let fuse_modules: &[(&str, &str)] = &[
        ("probe", "Read-only KEXT -> FSKit -> non-FUSE backend evidence"),
        ("mount", "Mount intercept over backing (`fuse mount <backing> <mountpoint> [--cow]`)"),
        ("unmount", "Unmount registered intercept (`fuse unmount <mountpoint>`)"),
        ("status", "FUSE read-cache + write-serialize meters"),
        ("commit", "Commit staged CoW (`fuse commit [relpath] [--agent]`)"),
        ("discard", "Discard staged CoW (`fuse discard [relpath] [--agent]`)"),
        ("list", "List mounts + pending CoW by agent"),
        ("provenance", "Read FUSE write xattrs on a backing file (`fuse provenance <path>`)"),
    ];

    let util_modules: &[(&str, &str)] = &[
        ("base85", "Base85 encode / decode"),
        ("csv", "Build a CSV row from --row entries"),
        ("crc", "CRC64 checksum"),
        ("hash", "xxhash3 / xxtea digest"),
        ("json", "JSON pretty-print / validate"),
        ("md-table", "Render markdown table"),
        ("sha", "SHA1 / SHA256 digest"),
        ("skiplist", "Walk the bundled skiplist"),
        ("trie", "Radix-trie lookup"),
        ("url", "URL percent-encode / decode"),
        ("uuid", "APFS UUID helper"),
        ("xml", "XML escape / unescape"),
    ];

    if as_json {
        let payload = serde_json::json!({
            "cast": cast_modules.iter().map(|(n, d)| serde_json::json!({"name": n, "desc": d})).collect::<Vec<_>>(),
            "mesh": mesh_modules.iter().map(|(n, d)| serde_json::json!({"name": n, "desc": d})).collect::<Vec<_>>(),
            "fuse": fuse_modules.iter().map(|(n, d)| serde_json::json!({"name": n, "desc": d})).collect::<Vec<_>>(),
            "util": util_modules.iter().map(|(n, d)| serde_json::json!({"name": n, "desc": d})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("sharecli CLI surfaces");
    println!();
    println!("cast <subcommand>  -- pane casting ({} subcommands)", cast_modules.len());
    for (n, d) in cast_modules {
        println!("  - {:<11} {}", n, d);
    }
    println!();
    println!("mesh <subcommand>  -- Maildir task queue ({} subcommands)", mesh_modules.len());
    for (n, d) in mesh_modules {
        println!("  - {:<11} {}", n, d);
    }
    println!();
    println!("fuse <subcommand>  -- FUSE IO intercept ({} subcommands)", fuse_modules.len());
    for (n, d) in fuse_modules {
        println!("  - {:<11} {}", n, d);
    }
    println!();
    println!("util <subcommand>  -- utility modules ({} subcommands)", util_modules.len());
    for (n, d) in util_modules {
        println!("  - {:<11} {}", n, d);
    }
    Ok(())
}

/// `sharecli version` — emit Backbone-2 ASCII splash + version + author.
///
/// Respects `--theme` (via `Tokens::from_name`) and `NO_COLOR`.
pub fn cli_version() -> Result<()> {
    let cli = Cli::command();
    let version = cli.get_version().unwrap_or("0.0.0");

    let tokens = theme::Tokens::from_name("backbone-2")
        .ok_or_else(|| anyhow::anyhow!("backbone-2 theme tokens missing"))?;

    let pulse = tokens.pulse_green.ansi_fg();
    let amber = tokens.warm_amber.ansi_fg();
    let panel = tokens.panel.ansi_fg();
    let reset = "\x1b[0m";

    let splash_large = include_str!("../assets/brand/sharecli-ascii.txt");
    let splash_small = include_str!("../assets/brand/sharecli-ascii-small.txt");

    if is_no_color() {
        println!("{splash_large}");
        println!("sharecli {version}");
        println!("multi-agent process manager");
        println!("(NO_COLOR set — ANSI palette disabled)");
    } else {
        println!("{pulse}{splash_large}{reset}");
        println!("{amber}sharecli {version}{reset}");
        println!("{panel}multi-agent process manager{reset}");
        println!("{panel}{splash_small}{reset}");
    }

    Ok(())
}

pub async fn fleet_status() -> Result<()> {
    use sharecli_fleet::{ThermalGovernor, DEFAULT_COORDINATOR};

    let _gov = ThermalGovernor::new();
    println!("Thermal governor: ready");

    match sharecli_fleet::connect(DEFAULT_COORDINATOR).await {
        Ok(_client) => {
            println!("Fleet registry: connected to {DEFAULT_COORDINATOR}");
        }
        Err(e) => {
            println!("Fleet registry: not connected ({e})");
            println!("  Run `sharecli fleet register` to join the fleet.");
        }
    }
    Ok(())
}

pub async fn fleet_register(name: Option<&str>, coordinator: &str) -> Result<()> {
    let hostname = name.unwrap_or("local");

    println!("Registering device '{hostname}' with coordinator '{coordinator}'");

    match sharecli_fleet::connect(coordinator).await {
        Ok(client) => {
            let record = sharecli_fleet::DeviceRecord {
                device_id: format!("{hostname}-{}", std::process::id()),
                hostname: hostname.to_string(),
                os: std::env::consts::OS.to_string(),
                available_slots: 4,
            };
            sharecli_fleet::announce(&client, &record).await?;
            println!(
                "Registered device '{}' (os={}, slots={})",
                record.device_id, record.os, record.available_slots
            );
        }
        Err(e) => {
            println!("Registration failed: {e}");
            println!("  Is the NATS coordinator running at '{coordinator}'?");
        }
    }
    Ok(())
}

pub fn proc_compose_cmd(cmd: &ProcComposeCmd) -> Result<()> {
    let resolve_path = |file: &Option<String>| -> Result<std::path::PathBuf> {
        if let Some(f) = file {
            let p = std::path::PathBuf::from(f);
            if !p.exists() {
                anyhow::bail!("File not found: {}", p.display());
            }
            Ok(p)
        } else {
            let cwd = std::env::current_dir()?;
            crate::proc_compose::find_config(&cwd).ok_or_else(|| {
                anyhow::anyhow!("No process-compose.yaml found in {cwd:?} or any parent directory")
            })
        }
    };

    match cmd {
        ProcComposeCmd::Status { file } => {
            let path = resolve_path(file)?;
            println!("Using: {}", path.display());
            let cfg = crate::proc_compose::load_config(&path)?;
            let defs = cfg.to_process_defs();
            crate::proc_compose::print_status(&defs);
        }
        ProcComposeCmd::List { file } => {
            let path = resolve_path(file)?;
            let cfg = crate::proc_compose::load_config(&path)?;
            for d in cfg.to_process_defs() {
                println!("{}", d.name);
            }
        }
    }
    Ok(())
}
