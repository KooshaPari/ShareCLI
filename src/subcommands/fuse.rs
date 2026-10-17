//! `Fuse` subcommand enum (FUSE IO intercept + CoW overlays + provenance).

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum FuseCmd {
    /// Report read-only KEXT -> FSKit -> non-FUSE backend evidence
    Probe {
        /// Mountpoint used for selection (not created or mounted)
        mountpoint: std::path::PathBuf,
        /// Emit JSON instead of operator-readable text
        #[arg(long)]
        json: bool,
    },
    /// Mount intercept layer over a backing directory
    Mount {
        /// Backing filesystem root to mirror
        backing: std::path::PathBuf,
        /// Mountpoint path (created if missing)
        mountpoint: std::path::PathBuf,
        /// Write-provenance session id (default: process-local)
        #[arg(long)]
        session: Option<String>,
        /// Enable per-agent CoW overlays (Feb `--cow`)
        #[arg(long)]
        cow: bool,
        /// CoW overlay root (default: `{backing}/.sharecli-cow` when `--cow`)
        #[arg(long)]
        cow_dir: Option<std::path::PathBuf>,
        /// Default agent id for unscoped commit/discard (default: session id)
        #[arg(long)]
        agent: Option<String>,
        /// Disable per-path write locks (Feb `--no-serialize`)
        #[arg(long)]
        no_serialize: bool,
        /// Path to Feb-format `agents.conf`
        #[arg(long)]
        agents_conf: Option<std::path::PathBuf>,
        /// Block in foreground until unmounted (default: background daemon thread)
        #[arg(long)]
        foreground: bool,
    },
    /// Unmount a registered intercept mount
    Unmount {
        /// Mountpoint path
        mountpoint: std::path::PathBuf,
    },
    /// Print FUSE read-cache + write-serialize global meters
    Status {
        /// Emit JSON instead of text sections
        #[arg(long)]
        json: bool,
    },
    /// Commit staged CoW (path and/or all pending for `--agent`)
    Commit {
        /// Path relative to backing root (optional when `--agent` commits all)
        relpath: Option<std::path::PathBuf>,
        /// Mountpoint when multiple mounts are registered
        #[arg(long)]
        mountpoint: Option<std::path::PathBuf>,
        /// Agent id (Feb `harness-fuse commit <mp> <agent>`)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Discard staged CoW (path and/or all pending for `--agent`)
    Discard {
        /// Path relative to backing root (optional when `--agent` discards all)
        relpath: Option<std::path::PathBuf>,
        /// Mountpoint when multiple mounts are registered
        #[arg(long)]
        mountpoint: Option<std::path::PathBuf>,
        /// Agent id
        #[arg(long)]
        agent: Option<String>,
    },
    /// List registered mounts and pending CoW paths
    List {
        /// Emit JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// Read write-provenance xattrs (`user.sharecli.session`, `user.sharecli.written_at`)
    Provenance {
        /// Backing file path (need not be under a live FUSE mount)
        path: std::path::PathBuf,
        /// Emit JSON instead of text (`null` when attrs absent)
        #[arg(long)]
        json: bool,
    },
}
