//! `Surface` subcommand enum (Ghostty shell-free I/O).

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum SurfaceCmd {
    /// Write bytes or UTF-8 text to one terminal surface
    Send {
        #[arg(long, env = "SHARECLI_GHOSTTY_SOCKET", default_value = "/tmp/sharecli-ghostty.sock")]
        socket: std::path::PathBuf,
        #[arg(long)]
        surface_id: String,
        #[arg(long, conflicts_with = "bytes")]
        text: Option<String>,
        #[arg(long, conflicts_with = "text")]
        bytes: Option<String>,
        #[arg(long, env = "SHARECLI_GHOSTTY_TOKEN")]
        token: Option<String>,
    },
    /// Read a bounded output snapshot from one terminal surface
    Read {
        #[arg(long, env = "SHARECLI_GHOSTTY_SOCKET", default_value = "/tmp/sharecli-ghostty.sock")]
        socket: std::path::PathBuf,
        #[arg(long)]
        surface_id: String,
        #[arg(long, default_value_t = 4096)]
        max_bytes: usize,
        #[arg(long, env = "SHARECLI_GHOSTTY_TOKEN")]
        token: Option<String>,
    },
    /// Resize one terminal surface
    Resize {
        #[arg(long, env = "SHARECLI_GHOSTTY_SOCKET", default_value = "/tmp/sharecli-ghostty.sock")]
        socket: std::path::PathBuf,
        #[arg(long)]
        surface_id: String,
        #[arg(long)]
        rows: u16,
        #[arg(long)]
        cols: u16,
        #[arg(long, env = "SHARECLI_GHOSTTY_TOKEN")]
        token: Option<String>,
    },
    /// Follow bounded server-originated output/events from one surface (or all surfaces)
    Watch {
        #[arg(long, env = "SHARECLI_GHOSTTY_SOCKET", default_value = "/tmp/sharecli-ghostty.sock")]
        socket: std::path::PathBuf,
        #[arg(long)]
        surface_id: Option<String>,
        #[arg(long)]
        from_seq: Option<u64>,
        #[arg(long, default_value_t = 65536)]
        max_chunk_bytes: usize,
        #[arg(long, default_value_t = 64)]
        queue_capacity: usize,
        /// Read one event, unsubscribe, and exit.
        #[arg(long)]
        once: bool,
        #[arg(long, env = "SHARECLI_GHOSTTY_TOKEN")]
        token: Option<String>,
    },
    /// Capture the native Ghostty pane topology as a validated LayoutSnapshot
    LayoutSnapshot {
        #[arg(long, env = "SHARECLI_GHOSTTY_SOCKET", default_value = "/tmp/sharecli-ghostty.sock")]
        socket: std::path::PathBuf,
        /// Write the snapshot to this path instead of stdout
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,
        #[arg(long, env = "SHARECLI_GHOSTTY_TOKEN")]
        token: Option<String>,
    },
    /// Apply a validated LayoutSnapshot through the native Ghostty provider
    LayoutRestore {
        /// JSON file containing a LayoutSnapshot
        input: std::path::PathBuf,
        #[arg(long, env = "SHARECLI_GHOSTTY_SOCKET", default_value = "/tmp/sharecli-ghostty.sock")]
        socket: std::path::PathBuf,
        #[arg(long, env = "SHARECLI_GHOSTTY_TOKEN")]
        token: Option<String>,
    },
}
