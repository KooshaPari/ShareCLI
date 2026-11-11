//! CLI commands for sharecli.
//!
//! Module structure (post-decomposition):
//!
//! - [`types`] — JSON envelope structs (`PoolJson`, `StatusJson`, `HealthJson`,
//!   `PsAllJson`, `*NdjsonLine`) plus CSV watch markers and footer emitters.
//! - [`live_sections`] — `print_live_*` / `eprint_live_*` /
//!   `capture_live_gate_host_watch` / `fetch_operator_pool_status_siblings`
//!   used by every operator command's text and JSON paths.
//! - [`runtime_helpers`] — `get_shared_runtime` / `get_project_resources`
//!   `OnceLock` singletons.
//! - [`ps`] — `ps` and `status` operator commands, plus `build_ps_all_json`
//!   and `build_status_json` (also called by `serve`, `main`, and other
//!   subcommands via `live_sections`).
//! - [`pool`] — `pool_status` operator command, plus `build_pool_json`
//!   (called by `ps` / `status` / `health` / `serve` / `main`).
//! - [`health`] — `health` operator command, plus `build_health_json`.
//! - [`process_ops`] — `start` / `stop` / `run_pool` / `set_limits` /
//!   `check_limits`, plus the `force_kill_*` confirmation guard and
//!   `expand_path`.
//! - [`config_cmd`] — `config` and `project` operator commands, plus the
//!   `project_group_start` / `project_group_stop` / `project_group_status`
//!   bulk helpers.
//! - [`tests`] — unit tests for `filter_by_project` and
//!   `force_kill_requires_confirmation`.
//!
//! The `commands::` module root re-exports the JSON envelope structs and the
//! subcommand entry points so that the existing
//! `sharecli::commands::ps` / `commands::PoolJson` / `commands::serve_run`
//! paths keep resolving from `main.rs`, `serve.rs`, `report.rs`, and the
//! `tests/fr00X_*.rs` integration suite.

pub mod cast;
pub mod config_cmd;
pub mod config_edit;
pub mod fuse;
pub mod health;
pub mod history;
pub mod live_sections;
pub mod mesh;
pub mod pool;
pub mod proc;
pub mod process_ops;
pub mod ps;
pub mod render_csv;
pub mod report;
pub mod runtime_helpers;
pub mod serve;
pub mod soak;
pub mod types;
pub mod undo;
pub mod uninstall;
pub mod upgrade;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

// --- JSON envelope re-exports (FR-007 / AC-007.43, 007.44, 007.49, 007.64–66, 007.77, 007.83).
// External test files and `main.rs` / `serve.rs` / `report.rs` resolve these
// from `sharecli::commands::*` directly. ---
pub use types::{
    HealthJson, HealthNdjsonLine, PoolJson, PoolNdjsonLine, PsAllJson, PsAllNdjsonLine,
    PsManagedProcessRow, StatusJson, StatusNdjsonLine,
};

// --- CSV document builders (FR-007 / AC-007.82 / AC-007.83). ---
pub use render_csv::{
    render_health_csv_body, render_pool_csv_body, render_ps_all_csv_body, render_status_csv_body,
};

// --- `serve::run` re-export (used by `main.rs`). ---
pub use serve::run as serve_run;

// --- Subcommand entry points (used by `main.rs`).
//     `cast`, `fuse`, `mesh` are already declared as `pub mod` above and
//     accessible as `commands::cast`, `commands::fuse`, `commands::mesh`
//     (main.rs aliases them as `cast_cmd` / `fuse_cmd` / `mesh_cmd`).
pub use config_cmd::{config, project};
pub use health::health;
pub use pool::pool_status;
pub use process_ops::{check_limits, run_pool, set_limits, start, stop};
pub use ps::{ps, status};

// --- JSON envelope builder re-exports (used by `main.rs`, `serve.rs`). ---
pub(crate) use pool::build_pool_json;
pub(crate) use ps::build_status_json;

// --- `live_sections` re-exports (used by `proc`, `render_csv`, `report`,
//     and any other sibling that used to call these via `super::X` from
//     the now-slim `commands` module root). ---
pub(crate) use live_sections::{
    capture_live_gate_host_watch, eprint_live_gate_host_watch_sections,
    fetch_operator_pool_status_siblings, print_live_gate_section, print_live_host_watch_section,
    print_live_pool_status_operator_sections,
};
