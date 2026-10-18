//! Library surface for the Windows WinUI tray (wire types + mapping helpers).
//!
//! The WinUI binary lives under `windows/ShareCLITray/`; this crate compiles
//! cross-platform so integration tests can prove AC-007.51 mapping parity with
//! Linux/Swift tray refresh via `monitoring.report`, AC-007.52 poll cadence, and
//! AC-007.54 kill / kill_all operator actions.

pub mod ipc;
/// Operator panel formatting helpers (gate/host/pool lines, visuals, badges).
pub mod operator_display;
/// Background poll loop that refreshes tray state from IPC.
pub mod poll;
