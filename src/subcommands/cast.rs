//! `Cast` subcommand enum (cross-machine text injection into panes).

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum CastCmd {
    /// Register a pane: `cast register <name> <address>`
    Register {
        /// Friendly pane name (e.g. `civis-1`)
        name: String,
        /// Address in the form `machine:host[:window[:pane]]`
        address: String,
    },
    /// Unregister a pane
    Unregister { name: String },
    /// List all registered panes
    List,
    /// Send text to a registered pane
    Send {
        /// Registered pane name
        name: String,
        /// File to read; pass `-` (or omit) to read from stdin
        file: Option<String>,
    },
    /// Show the on-disk path of the pane-map file
    Where,
}
