//! Root subcommand enum for the sharecli binary.
//!
//! `Commands` is the top-level `#[derive(Subcommand)]` type referenced by
//! `Cli::command`. The other subcommand families (session, surface, fleet,
//! fuse, mesh, cast, proc-compose) live in sibling modules and are referenced
//! here by `super::{name}` to keep the per-file line count manageable.

use clap::Subcommand;

use super::{cast, fleet, fuse, mesh, proc_compose, session, surface};
use crate::config;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inspect durable agent sessions
    Session {
        #[command(subcommand)]
        cmd: session::SessionCmd,
    },
    /// Send shell-free I/O requests to a ShareCLI-enabled Ghostty surface
    Surface {
        #[command(subcommand)]
        cmd: surface::SurfaceCmd,
    },
    /// List managed processes
    Ps {
        /// Filter by project name
        #[arg(short, long)]
        project: Option<String>,

        /// Filter by harness type (claude, forge, node, bun)
        #[arg(long)]
        harness: Option<String>,

        /// Show all processes including system ones
        #[arg(short, long)]
        all: bool,

        /// Emit JSON (requires `--all` for host agent inventory parity; AC-007.43)
        #[arg(long)]
        json: bool,

        /// Emit CSV (requires `--all` for host agent inventory parity; AC-007.83)
        #[arg(long)]
        csv: bool,

        /// Re-render every N seconds until Ctrl-C (live watch mode; AC-007.49 with --all --json)
        #[arg(short, long)]
        watch: Option<u64>,
    },

    /// Start a harness process
    Start {
        /// Project name
        #[arg(required = true)]
        project: String,

        /// Harness type (claude, forge, node, bun)
        #[arg(long, default_value = "claude")]
        harness: String,

        /// Working directory
        #[arg(short, long)]
        cwd: Option<String>,

        /// Arguments to pass
        args: Vec<String>,
    },

    /// Stop managed processes
    #[command(alias = "quit")]
    Stop {
        /// Process ID to stop
        #[arg(long)]
        pid: Option<u32>,

        /// Project to stop all processes for
        #[arg(short, long)]
        project: Option<String>,

        /// Harness type to stop
        #[arg(long)]
        harness: Option<String>,

        /// Stop all managed processes
        #[arg(short, long)]
        all: bool,

        /// Force kill (SIGKILL)
        #[arg(short, long)]
        force: bool,

        /// Confirm destructive force-kill without interactive prompt
        #[arg(long)]
        yes: bool,
    },

    /// Check process health
    Status {
        /// Detailed output
        #[arg(short, long)]
        verbose: bool,

        /// Emit machine-readable JSON (includes detected agent inventory)
        #[arg(long)]
        json: bool,

        /// Emit operator snapshot as CSV (header + rows; AC-007.82)
        #[arg(long)]
        csv: bool,

        /// Re-render every N seconds until Ctrl-C (live watch mode; AC-007.66 with --json)
        #[arg(short, long)]
        watch: Option<u64>,
    },

    /// List host-detected coding agents (proc scan + RSS/FD samples)
    Proc {
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,

        /// Emit flat agent inventory as CSV (header + rows)
        #[arg(long)]
        csv: bool,

        /// Show parent-child process forests rooted at detected agents
        #[arg(long)]
        tree: bool,

        /// Re-render every N seconds until Ctrl-C (live watch mode)
        #[arg(short, long)]
        watch: Option<u64>,

        /// Keep only agents matching this family id (case-insensitive)
        #[arg(long)]
        family: Option<String>,

        /// Drop agents matching this family id (case-insensitive; negates --family)
        #[arg(long)]
        exclude_family: Option<String>,

        /// Keep only agents whose COMM contains this substring (case-insensitive)
        #[arg(long)]
        comm: Option<String>,

        /// Keep only agents whose joined argv/cmdline contains this substring (case-insensitive)
        #[arg(long)]
        cmdline: Option<String>,

        /// Keep only agents in this process state (R|S|D|Z|T|t|…)
        #[arg(long)]
        state: Option<String>,

        /// Keep only agents at or above this RSS (bytes or K/M/G suffix)
        #[arg(long)]
        min_rss: Option<String>,

        /// Keep only agents at or below this RSS (bytes or K/M/G suffix)
        #[arg(long)]
        max_rss: Option<String>,

        /// Keep only agents at or above this open-FD count
        #[arg(long)]
        min_fd: Option<String>,

        /// Keep only agents at or below this open-FD count
        #[arg(long)]
        max_fd: Option<String>,

        /// Sort inventory rows or tree roots: rss (desc), fd (desc), pid (asc), state (asc), cpu (desc), age (asc), name (asc)
        #[arg(long)]
        sort: Option<String>,

        /// Cap inventory rows or tree root forests after filter/sort (N >= 1)
        #[arg(long)]
        limit: Option<u64>,

        /// Show RSS/FD/cmdline/parent detail for one live host PID
        #[arg(long)]
        pid: Option<u32>,

        /// Keep only agents whose parent PID equals N
        #[arg(long)]
        ppid: Option<u32>,
    },

    /// Run a runtime health probe
    Health {
        /// Optional harness type hint (node, bun, etc.)
        #[arg(long)]
        harness: Option<String>,

        /// Emit JSON (gate → host_watch siblings; AC-007.44)
        #[arg(long)]
        json: bool,

        /// Emit operator snapshot as CSV (header + rows; AC-007.82)
        #[arg(long)]
        csv: bool,

        /// Re-render every N seconds until Ctrl-C (live watch mode; AC-007.64 with --json)
        #[arg(short, long)]
        watch: Option<u64>,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        cmd: config::ConfigCmd,
    },

    /// Project management
    Project {
        #[command(subcommand)]
        cmd: config::ProjectCmd,
    },

    /// Optimize resource usage
    Optimize {
        /// Apply optimizations automatically
        #[arg(short, long)]
        apply: bool,
    },

    /// Prune idle processes
    Prune {
        /// Idle time threshold in seconds (default from config if omitted)
        #[arg(short, long)]
        idle_seconds: Option<u64>,

        /// Actually kill processes (dry run by default)
        #[arg(short, long)]
        force: bool,
    },

    /// Show shared runtime pool status
    Pool {
        /// Harness type to check (node, bun)
        #[arg(long)]
        harness: Option<String>,

        /// Emit JSON (gate → host_watch siblings; AC-007.44)
        #[arg(long)]
        json: bool,

        /// Emit operator snapshot as CSV (header + rows; AC-007.82)
        #[arg(long)]
        csv: bool,

        /// Re-render every N seconds until Ctrl-C (live watch mode; AC-007.65 with --json)
        #[arg(short, long)]
        watch: Option<u64>,
    },

    /// Run using pooled runtime
    Run {
        /// Harness type (node, bun)
        #[arg(required = true)]
        harness: String,

        /// Project name
        #[arg(required = true)]
        project: String,
    },

    /// Set project resource limits
    Limits {
        /// Project name
        #[arg(required = true)]
        project: String,

        /// Memory limit in MB
        #[arg(short, long)]
        memory: Option<u64>,

        /// Max process count
        #[arg(short, long)]
        processes: Option<usize>,
    },

    /// Check project resource limits
    Check {
        /// Project name
        #[arg(required = true)]
        project: String,
    },

    /// Live thermal-gate / hypervisor state monitor (TUI)
    ///
    /// Displays current memory pressure level (GREEN/YELLOW/RED), active
    /// build slots, and the gate's ADMIT/DENY decision.
    /// Press `q` or Ctrl-C to exit.
    Thermal {
        /// Build-slot cap (max concurrent cargo build|check|test processes).
        #[arg(short, long, default_value_t = crate::subcommands::root::DEFAULT_SLOT_CAP)]
        cap: u32,
    },

    /// Start the HTTP + WebSocket dashboard server
    Serve {
        /// Address to bind (host:port)
        #[arg(short, long, default_value = "127.0.0.1:9000")]
        bind: String,

        /// Behaviour when a server is already running: abort | attach | replace
        #[arg(long, default_value = "abort")]
        on_conflict: String,
    },

    /// Print a fleet analytics snapshot (one-shot or live watch mode)
    Report {
        /// Output format: text (default), json, or csv
        #[arg(long, default_value = "text")]
        format: String,

        /// Re-render every N seconds (like `watch -n N`); omit for one-shot
        #[arg(short, long)]
        watch: Option<u64>,

        /// Sort top-consumers by: memory (default) or name
        #[arg(long, default_value = "memory")]
        sort: String,
    },

    /// Fleet device management
    Fleet {
        #[command(subcommand)]
        cmd: fleet::FleetCmd,
    },
    /// Maildir mesh task-queue operator surface (status / reclaim)
    Mesh {
        #[command(subcommand)]
        cmd: mesh::MeshCmd,
    },
    /// FUSE IO intercept operator surface (mount, provenance, CoW)
    Fuse {
        #[command(subcommand)]
        cmd: fuse::FuseCmd,
    },
    /// Cross-machine text injection into registered terminal panes
    Cast {
        #[command(subcommand)]
        cmd: cast::CastCmd,
    },

    /// process-compose.yaml integration
    ProcCompose {
        #[command(subcommand)]
        cmd: proc_compose::ProcComposeCmd,
    },

    /// Generate shell completion script
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },

    /// Generate roff man page for sharecli(1) (clap_mangen)
    Man {
        /// Write to share/man/man1/sharecli.1 instead of stdout
        #[arg(long)]
        install: bool,
    },

    /// Enumerate available CLI surfaces (cast modules + utility modules)
    List {
        /// Output as machine-readable JSON instead of a table
        #[arg(long)]
        json: bool,
    },

    /// Run the harbor soak harness -- long-running CLI stability evaluation
    Soak {
        /// Total soak duration in seconds
        #[arg(long)]
        duration: Option<u64>,
        /// Interval between scenario rounds in seconds
        #[arg(long)]
        interval: Option<u64>,
        /// Path to soak.yaml configuration
        #[arg(long, default_value = "soak.yaml")]
        config: std::path::PathBuf,
        /// Path to write the JSON report
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },
    /// Check for or apply an upstream release upgrade
    Upgrade {
        /// Upgrade channel (stable / beta / nightly)
        #[arg(long)]
        channel: Option<String>,
    },
    /// Print the sharecli version
    Version,
    /// Print uninstall guidance and optionally purge local config/state
    Uninstall {
        /// Delete config/state directories (requires explicit consent)
        #[arg(long)]
        purge_data: bool,

        /// List paths that would be removed without deleting
        #[arg(long)]
        dry_run: bool,
    },
    /// Undo / restore model (C09 L81.9): inspect or replay the
    /// operations journal recorded at $XDG_STATE_HOME/sharecli/operations.jsonl
    Undo {
        /// Maximum number of journal entries to display
        #[arg(long, default_value_t = 20)]
        limit: usize,

        /// Emit the journal as JSONL instead of human-readable text
        #[arg(long)]
        json: bool,

        /// Print the restore plan for an entry without executing it (C09 L81.9)
        #[arg(long)]
        restore: bool,

        /// Restore a specific operation by its journal id (C09 L81.9).
        /// When --restore is true and --id is set, the operation plan is
        /// printed; the actual restore executor is an out-of-scope operator
        /// action documented in docs/ops/undo-model.md.
        #[arg(long)]
        id: Option<String>,
    },
}

/// Default build-slot cap for the thermal TUI. The thermal_tui crate defines
/// its own `DEFAULT_SLOT_CAP`; this is a thin alias so the clap default value
/// can be expressed inline in the `#[arg(default_value_t = …)]` attribute
/// above without forcing a direct dependency on the thermal_tui crate from
/// the subcommand module.
pub const DEFAULT_SLOT_CAP: u32 = 16;
