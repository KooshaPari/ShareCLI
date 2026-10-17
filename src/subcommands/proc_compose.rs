//! `ProcCompose` subcommand enum (process-compose.yaml integration).

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum ProcComposeCmd {
    /// Pretty-print all services from process-compose.yaml with their current status.
    Status {
        /// Path to process-compose.yaml (auto-discovered from cwd if omitted)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// List services defined in process-compose.yaml (names only).
    List {
        /// Path to process-compose.yaml (auto-discovered from cwd if omitted)
        #[arg(short, long)]
        file: Option<String>,
    },
}
