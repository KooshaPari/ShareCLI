//! Top-level `dispatch` for the sharecli binary.
//!
//! This module owns the `match cli.command { ... }` switch that maps each
//! `Commands` variant to a concrete handler. Each arm is a thin
//! `?`-propagating call — the real work lives in `crate::commands::*` and the
//! `cli_impl_*` modules. `dispatch` is a pure switch and should not perform
//! I/O of its own (other than calling the concrete handlers).

use anyhow::Result;
use clap::CommandFactory;
use sharecli_fleet::PoolOperatorPanel;
use sharecli_fleet::StatusOperatorPanel;
use sharecli_thermal_tui as thermal_tui;
use std::str::FromStr as _;

use crate::commands;
use crate::commands::{
    cast as cast_cmd, check_limits, config as config_cmd, fuse as fuse_cmd, health,
    mesh as mesh_cmd, pool_status, project as project_cmd, ps, run_pool, serve_run, set_limits,
    start, status, stop,
};
use crate::config;
use crate::serve_lock::OnConflict;
use crate::subcommands::cast::CastCmd;
use crate::subcommands::fleet::FleetCmd;
use crate::subcommands::fuse::FuseCmd;
use crate::subcommands::mesh::MeshCmd;
use crate::subcommands::proc_compose::ProcComposeCmd;
use crate::subcommands::root::{Cli, Commands};
use crate::{cli_impl_meta, cli_impl_optimize, cli_impl_session};

pub async fn dispatch(cli: &Cli) -> Result<()> {
    match &cli.command {
        Commands::Session { cmd } => cli_impl_session::session_cmd(cmd)?,
        Commands::Surface { cmd } => cli_impl_session::surface_cmd(cmd)?,
        Commands::Ps { project, harness, all, json, csv, watch } => {
            ps(project.as_deref(), harness.as_deref(), *all, *json, *csv, *watch).await?
        }
        Commands::Start { project, harness, cwd, args } => {
            start(project, harness, cwd.as_deref(), args).await?
        }
        Commands::Stop { pid, project, harness, all, force, yes } => {
            stop(*pid, project.as_deref(), harness.as_deref(), *all, *force, *yes).await?
        }
        Commands::Status { verbose, json, csv, watch } => {
            status(*verbose, *json, *csv, *watch).await?
        }
        Commands::Proc {
            json,
            csv,
            tree,
            watch,
            family,
            exclude_family,
            comm,
            cmdline,
            state,
            min_rss,
            max_rss,
            min_fd,
            max_fd,
            sort,
            limit,
            pid,
            ppid,
        } => {
            commands::proc::run(
                *json,
                *csv,
                *tree,
                *watch,
                family.clone(),
                exclude_family.clone(),
                comm.clone(),
                cmdline.clone(),
                state.clone(),
                min_rss.clone(),
                max_rss.clone(),
                min_fd.clone(),
                max_fd.clone(),
                sort.clone(),
                *limit,
                *pid,
                *ppid,
            )
            .await?
        }
        Commands::Config { cmd } => config_cmd(cmd)?,
        Commands::Project { cmd } => project_cmd(cmd).await?,
        Commands::Optimize { apply } => cli_impl_optimize::optimize(*apply).await?,
        Commands::Prune { idle_seconds, force } => {
            cli_impl_optimize::prune(
                idle_seconds.unwrap_or(config::global().spawn.prune_idle_seconds),
                *force,
            )
            .await?
        }
        Commands::Pool { harness: _, json, csv, watch } => pool_status(*json, *csv, *watch).await?,
        Commands::Health { harness, json, csv, watch } => {
            health(harness.as_deref(), *json, *csv, *watch).await?
        }
        Commands::Run { harness, project } => run_pool(harness, project).await?,
        Commands::Limits { project, memory, processes } => {
            set_limits(project, *memory, *processes).await?
        }
        Commands::Check { project } => check_limits(project).await?,
        Commands::Report { format, watch, sort } => {
            let fmt = commands::report::ReportFormat::from_str(format)?;
            let sort_key = commands::report::SortBy::from_str(sort)?;
            commands::report::run(fmt, *watch, sort_key).await?
        }
        Commands::Serve { bind, on_conflict } => {
            let policy = match on_conflict.as_str() {
                "attach" => OnConflict::Attach,
                "replace" => OnConflict::Replace,
                _ => OnConflict::Abort,
            };
            serve_run(bind, policy).await?
        }
        Commands::Thermal { cap } => {
            let gov = sharecli_fleet::thermal::ThermalGovernor::new();
            // Drive pool/status polling from the main tokio runtime via a
            // channel. `Handle::current().block_on(...)` and
            // `Runtime::block_on(...)` both panic when the calling thread is
            // already inside another tokio runtime. We avoid that by
            // dispatching the work to the existing runtime and synchronizing
            // via a std::sync::mpsc::channel.
            let (tx, rx) = std::sync::mpsc::channel::<(Option<PoolOperatorPanel>, Option<StatusOperatorPanel>)>();
            let pool_handle = tokio::runtime::Handle::current();
            std::thread::spawn(move || loop {
                let (pool, status) = pool_handle.block_on(async {
                    let pool_panel = crate::commands::build_pool_json().await.ok().map(Into::into);
                    let status_panel = crate::commands::build_status_json().await.ok().map(Into::into);
                    (pool_panel, status_panel)
                });
                if tx.send((pool, status)).is_err() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            });
            let poll_pool_status = move || rx.recv().unwrap_or((None, None));
            thermal_tui::run_with_pool_status(&gov, *cap, Some(Box::new(poll_pool_status)))?;
        }
        Commands::Fleet { cmd } => match cmd {
            FleetCmd::Status => cli_impl_meta::fleet_status().await?,
            FleetCmd::Register { name, coordinator } => {
                cli_impl_meta::fleet_register(name.as_deref(), coordinator).await?
            }
        },
        Commands::Mesh { cmd } => match cmd {
            MeshCmd::Status { queue, json } => mesh_cmd::status(queue, *json)?,
            MeshCmd::Reclaim { queue, owner } => mesh_cmd::reclaim(queue, owner)?,
        },
        Commands::Fuse { cmd } => match cmd {
            FuseCmd::Probe { mountpoint, json } => fuse_cmd::probe(mountpoint, *json)?,
            FuseCmd::Mount {
                backing,
                mountpoint,
                session,
                cow,
                cow_dir,
                agent,
                no_serialize,
                agents_conf,
                foreground,
            } => {
                fuse_cmd::mount(
                    backing,
                    mountpoint,
                    fuse_cmd::FuseMountCliOpts {
                        session_id: session.clone(),
                        cow: *cow,
                        cow_dir: cow_dir.clone(),
                        agent: agent.clone(),
                        no_serialize: *no_serialize,
                        agents_conf: agents_conf.clone(),
                        foreground: *foreground,
                    },
                )?;
            }
            FuseCmd::Unmount { mountpoint } => fuse_cmd::unmount(mountpoint)?,
            FuseCmd::Status { json } => fuse_cmd::status(*json)?,
            FuseCmd::Commit { relpath, mountpoint, agent } => {
                fuse_cmd::commit(relpath.as_deref(), mountpoint.as_deref(), agent.as_deref())?
            }
            FuseCmd::Discard { relpath, mountpoint, agent } => {
                fuse_cmd::discard(relpath.as_deref(), mountpoint.as_deref(), agent.as_deref())?
            }
            FuseCmd::List { json } => fuse_cmd::list(*json)?,
            FuseCmd::Provenance { path, json } => fuse_cmd::provenance(path, *json)?,
        },
        Commands::Cast { cmd } => match cmd {
            CastCmd::Register { name, address } => cast_cmd::register(name, address)?,
            CastCmd::Unregister { name } => cast_cmd::unregister(name)?,
            CastCmd::List => cast_cmd::list()?,
            CastCmd::Send { name, file } => cast_cmd::send(name, file.as_deref())?,
            CastCmd::Where => cast_cmd::where_file()?,
        },
        Commands::ProcCompose { cmd } => cli_impl_meta::proc_compose_cmd(cmd)?,
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "sharecli", &mut std::io::stdout());
        }
        Commands::Man { install } => cli_impl_meta::cli_man(*install)?,
        Commands::List { json } => cli_impl_meta::cli_list(*json)?,
        Commands::Undo { limit, json, restore, id } => {
            commands::undo::run(*limit, *json, *restore, id.clone())?
        }
        Commands::Soak { duration, interval, config, output } => {
            let report = commands::soak::run(*duration, *interval, config, output.as_deref())?;
            println!(
                "soak: {} scenarios, {:.1}% errors",
                report.scenario_results.len(),
                report.error_rate * 100.0
            );
        }
        Commands::Upgrade { channel } => {
            commands::upgrade::check(channel.as_deref())?;
        }
        Commands::Version => cli_impl_meta::cli_version()?,
        Commands::Uninstall { purge_data, dry_run } => {
            commands::uninstall::run(*purge_data, *dry_run)?
        }
    }

    Ok(())
}

// Re-exports preserved for legacy test access
pub type _SessionCmdReexport = crate::subcommands::session::SessionCmd;
pub type _SurfaceCmdReexport = crate::subcommands::surface::SurfaceCmd;
pub type _ProcComposeCmdReexport = ProcComposeCmd;
