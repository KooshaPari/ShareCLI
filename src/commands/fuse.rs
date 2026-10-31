// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli fuse …` — FUSE IO intercept operator surface (FR-009).
//!
//! Exposes mount control, staged CoW commit/discard (per-agent), global FUSE
//! meters, and write-provenance inspection without reaching into xattr(1).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sharecli_fuse::{
    default_session_id, global_read_cache_meters, global_write_serialize_meters, probe_runtime,
    read_provenance, AgentsConf, FuseMountOptions, FuseSessionRegistry,
};

/// Mount options from CLI flags.
#[derive(Debug, Clone, Default)]
pub struct FuseMountCliOpts {
    /// Write-provenance session id.
    pub session_id: Option<String>,
    /// Enable per-agent CoW.
    pub cow: bool,
    /// CoW directory root.
    pub cow_dir: Option<PathBuf>,
    /// Default agent id.
    pub agent: Option<String>,
    /// Disable per-path write locks.
    pub no_serialize: bool,
    /// Path to agents.conf.
    pub agents_conf: Option<PathBuf>,
    /// Foreground mount.
    pub foreground: bool,
}

/// Report read-only backend evidence without loading, mounting, or prompting.
pub fn probe(mountpoint: &Path, json: bool) -> Result<()> {
    let evidence = probe_runtime(mountpoint);
    if json {
        let body = serde_json::json!({
            "platform": evidence.platform,
            "mountpoint": evidence.mountpoint.display().to_string(),
            "kernel_loaded": evidence.kernel_loaded,
            "fskit_framework": evidence.fskit_framework,
            "fskit_approved": evidence.fskit_approved,
            "selected_backend": evidence.selection.backend.as_str(),
            "diagnostic": evidence.selection.diagnostic.map(|diagnostic| diagnostic.message()),
            "non_fuse_fallback": evidence.non_fuse_fallback,
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }

    println!("platform:          {}", evidence.platform);
    println!("mountpoint:        {}", evidence.mountpoint.display());
    println!("kext_loaded:       {}", evidence.kernel_loaded);
    println!("fskit_framework:   {}", evidence.fskit_framework);
    println!("fskit_approved:    {}", evidence.fskit_approved);
    println!("selected_backend:  {}", evidence.selection.backend.as_str());
    if let Some(diagnostic) = evidence.selection.diagnostic {
        println!("diagnostic:        {}", diagnostic.message());
    }
    println!("non_fuse_fallback: {}", evidence.non_fuse_fallback);
    Ok(())
}

/// Read FUSE write-provenance xattrs from a backing file path.
///
/// When `json` is true, emit a JSON object or `null` when attrs are absent.
pub fn provenance(path: &Path, json: bool) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("fuse provenance: path does not exist: {}", path.display());
    }
    if path.is_dir() {
        anyhow::bail!("fuse provenance: path must be a file, not a directory: {}", path.display());
    }

    let prov = read_provenance(path)
        .with_context(|| format!("read provenance xattrs on {}", path.display()))?;

    if json {
        match prov {
            Some(p) => {
                let body = serde_json::json!({
                    "path": path.display().to_string(),
                    "session_id": p.session_id,
                    "written_at_unix": p.written_at_unix,
                });
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            None => println!("null"),
        }
        return Ok(());
    }

    match prov {
        Some(p) => {
            println!("path:       {}", path.display());
            println!("session_id: {}", p.session_id);
            println!("written_at: {} (unix)", p.written_at_unix);
        }
        None => {
            println!("path:       {}", path.display());
            println!("provenance: (none — no sharecli write xattrs)");
        }
    }
    Ok(())
}

/// Mount the intercept layer over `backing` at `mountpoint`.
pub fn mount(backing: &Path, mountpoint: &Path, opts: FuseMountCliOpts) -> Result<()> {
    if let Some(ref agent) = opts.agent {
        if !AgentsConf::is_valid_agent_id(agent) {
            anyhow::bail!("fuse mount: invalid --agent {agent:?} (use alnum, '_' or '-')");
        }
    }
    if let Some(ref conf) = opts.agents_conf {
        AgentsConf::load(conf).with_context(|| format!("load agents.conf {}", conf.display()))?;
    }

    let session = opts.session_id.clone().unwrap_or_else(default_session_id);
    let mount_opts = FuseMountOptions {
        session_id: Some(session.clone()),
        cow: opts.cow,
        cow_dir: opts.cow_dir.clone(),
        agent: opts.agent.clone(),
        serialize: !opts.no_serialize,
        agents_conf: opts.agents_conf.clone(),
    };
    let registry = FuseSessionRegistry::global();
    if opts.foreground {
        registry.mount_foreground_with(mountpoint, backing, &mount_opts)?;
    } else {
        registry.mount_background_with(mountpoint, backing, &mount_opts)?;
        println!(
            "fuse mount: {} over {} (session {}; cow={}; agent={})",
            mountpoint.display(),
            backing.display(),
            session,
            opts.cow,
            opts.agent.as_deref().unwrap_or(&session)
        );
    }
    Ok(())
}

/// Unmount a registered intercept mount.
pub fn unmount(mountpoint: &Path) -> Result<()> {
    FuseSessionRegistry::global().unmount(mountpoint)?;
    println!("fuse unmount: {}", mountpoint.display());
    Ok(())
}

/// Print global FUSE read-cache and write-serialize meter sections.
pub fn status(json: bool) -> Result<()> {
    let read = global_read_cache_meters();
    let write = global_write_serialize_meters();
    if json {
        let body = serde_json::json!({
            "read_cache": {
                "hits": read.hits,
                "misses": read.misses,
            },
            "write_serialize": {
                "passthrough_writes": write.passthrough_writes,
                "stages": write.stages,
                "commits": write.commits,
                "discards": write.discards,
            },
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    print!("{}", read.format_status_section());
    print!("{}", write.format_status_section());
    Ok(())
}

/// Commit staged CoW — one relative path and/or all pending for `--agent`.
pub fn commit(
    relpath: Option<&Path>,
    mountpoint: Option<&Path>,
    agent: Option<&str>,
) -> Result<()> {
    commit_or_discard(relpath, mountpoint, agent, true)
}

/// Discard staged CoW — one relative path and/or all pending for `--agent`.
pub fn discard(
    relpath: Option<&Path>,
    mountpoint: Option<&Path>,
    agent: Option<&str>,
) -> Result<()> {
    commit_or_discard(relpath, mountpoint, agent, false)
}

fn commit_or_discard(
    relpath: Option<&Path>,
    mountpoint: Option<&Path>,
    agent: Option<&str>,
    commit: bool,
) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        use sharecli_fuse::WriteSerializeError;

        if let Some(a) = agent {
            if !AgentsConf::is_valid_agent_id(a) {
                anyhow::bail!("fuse: invalid --agent {a:?} (use alnum, '_' or '-')");
            }
        }

        let fs = FuseSessionRegistry::global()
            .resolve_fs(mountpoint)
            .context("fuse commit/discard requires a registered mount")?;
        let verb = if commit { "commit" } else { "discard" };

        match (relpath, agent) {
            (None, None) => {
                anyhow::bail!("fuse {verb}: pass <relpath> and/or --agent <id> (see `fuse list`)");
            }
            (Some(rel), _) => {
                let result = if commit {
                    fs.commit_rel_for_agent(agent, rel)
                } else {
                    fs.discard_rel_for_agent(agent, rel)
                };
                match result {
                    Ok(()) => {
                        println!(
                            "fuse {verb}: {}{}",
                            rel.display(),
                            agent.map(|a| format!(" (agent {a})")).unwrap_or_default()
                        );
                        Ok(())
                    }
                    Err(WriteSerializeError::NoPending(p)) => {
                        anyhow::bail!("fuse {verb}: no pending CoW staging for {}", p.display());
                    }
                    Err(e) => Err(e.into()),
                }
            }
            (None, Some(_)) => {
                let paths = if commit {
                    fs.commit_all_for_agent(agent)?
                } else {
                    fs.discard_all_for_agent(agent)?
                };
                if paths.is_empty() {
                    println!("fuse {verb}: (no pending for agent {})", agent.unwrap_or("default"));
                } else {
                    for p in &paths {
                        println!("fuse {verb}: {}", p.display());
                    }
                }
                Ok(())
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (relpath, mountpoint, agent, commit);
        anyhow::bail!("sharecli-fuse is only supported on Linux, macOS, and Windows (WinFsp)")
    }
}

/// List registered mounts and pending CoW relative paths.
pub fn list(json: bool) -> Result<()> {
    let mounts = FuseSessionRegistry::global().list();
    if json {
        let body: Vec<serde_json::Value> = mounts
            .iter()
            .map(|m| {
                serde_json::json!({
                    "mountpoint": m.mountpoint.display().to_string(),
                    "backing": m.backing.display().to_string(),
                    "session_id": m.session_id,
                    "cow_enabled": m.cow_enabled,
                    "cow_root": m.cow_root.display().to_string(),
                    "default_agent": m.default_agent,
                    "pending_relpaths": m.pending_relpaths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    "pending_by_agent": m.pending_by_agent.iter().map(|(a, paths)| {
                        serde_json::json!({
                            "agent": a,
                            "relpaths": paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    if mounts.is_empty() {
        println!("fuse list: (no registered mounts)");
        return Ok(());
    }
    for m in &mounts {
        println!("mountpoint: {}", m.mountpoint.display());
        println!("  backing:       {}", m.backing.display());
        println!("  session_id:    {}", m.session_id);
        println!("  cow:           {}", m.cow_enabled);
        println!("  cow_root:      {}", m.cow_root.display());
        println!("  default_agent: {}", m.default_agent);
        if m.pending_by_agent.is_empty() {
            println!("  pending:       (none)");
        } else {
            println!("  pending:");
            for (agent, paths) in &m.pending_by_agent {
                for p in paths {
                    println!("    - [{agent}] {}", p.display());
                }
            }
        }
        println!();
    }
    Ok(())
}
