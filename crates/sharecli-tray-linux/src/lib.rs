//! Library surface for sharecli-tray-linux (IPC client + wire types).
//!
//! The binary target uses this module; integration tests import mapping helpers
//! for AC-007.48 tray refresh via `monitoring.report`.

pub mod ipc;
/// Operator panel formatting helpers (gate/host/pool lines, visuals, badges).
pub mod operator_display;
/// Background poll loop that refreshes tray state from IPC.
pub mod poll;
