//! `Fleet` subcommand enum (cross-device registry + thermal telemetry).

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum FleetCmd {
    /// Show fleet registry status and thermal level
    Status,
    /// Register this device into the fleet
    Register {
        /// Friendly device name (defaults to "local")
        #[arg(short, long)]
        name: Option<String>,
        /// Fleet coordinator address (e.g. nats://localhost:4222)
        #[arg(short, long, default_value = "nats://localhost:4222")]
        coordinator: String,
    },
}
