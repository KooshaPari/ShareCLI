// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Library surface for the Windows WinUI tray (wire types + mapping helpers).
//!
//! The WinUI binary lives under `windows/ShareCLITray/`; this crate compiles
//! cross-platform so integration tests can prove AC-007.51 mapping parity with
//! Linux/Swift tray refresh via `monitoring.report`, AC-007.52 poll cadence, and
//! AC-007.54 kill / kill_all operator actions.

pub mod ipc;
pub mod operator_display;
pub mod poll;
