//! `Mesh` subcommand enum (Maildir task-queue operator surface).

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum MeshCmd {
    /// Show Maildir queue depth (ready / in_flight / pending)
    Status {
        /// Path to Maildir queue root (`tmp/` `new/` `cur/`)
        #[arg(long, short = 'Q')]
        queue: std::path::PathBuf,
        /// Emit JSON [`sharecli_mesh::MaildirStatus`] instead of text
        #[arg(long)]
        json: bool,
    },
    /// Return in-flight (`cur/`) tasks for an owner back to `new/`
    Reclaim {
        /// Path to Maildir queue root
        #[arg(long, short = 'Q')]
        queue: std::path::PathBuf,
        /// Owner string stamped at claim time
        #[arg(long)]
        owner: String,
    },
}
