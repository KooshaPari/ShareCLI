// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc` host agent inventory CLI.
//!
//! Module structure (post-decomposition):
//!
//! - [`types`] — JSON envelope structs (AgentProcRow, AgentProcSnapshot,
//!   AgentTreeSnapshot, ProcDetailSnapshot, *NdjsonLine, AgentTreeNodeJson).
//! - [`filter`] — ProcFilter + match helpers + parse_proc_state/parse_fd_count.
//! - [`sort`] — ProcSort + parse_proc_limit + limit_watched/limit_agent_forests.
//! - [`scan`] — filter_watched/forest + sort_watched/forest + apply_sort + RSS/FD maps.
//! - [`build`] — AgentProcSnapshot::capture + agent_row_from_watch +
//!   agent_tree_node_to_json + build_proc_detail + state helpers + build_agent_ppid/cmdline_map.
//! - [`render`] — text / CSV renderers (render_agent_inventory, render_agent_tree,
//!   render_proc_detail_text, render_proc_detail_csv, etc.) + csv_escape_field.
//! - [`ndjson`] — NDJSON watch helpers + CSV/stderr companion emission
//!   (unix_ts_secs, emit_ndjson_line, append_proc_csv_companions, etc.).
//! - [`run`] — `run` entry point + `render_once` dispatcher +
//!   `render_pid_detail[_once]` + `reject_pid_inventory_combos`.
//! - [`platform`] — Linux `/proc/{pid}/stat` helpers (CPU%, age).
//! - [`tests`] — unit tests (loaded via `#[path]`).
//!
//! `commands::proc::run` and every JSON envelope type are re-exported at the
//! module root so existing `crate::commands::proc::*` paths keep resolving
//! from `main.rs`, `report.rs`, `serve.rs`, and the `tests/fr00X_*.rs` suite.

pub mod build;
pub mod filter;
pub mod ndjson;
pub mod platform;
pub mod render;
pub mod run;
pub mod scan;
pub mod sort;
pub mod types;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

// Public re-exports for `crate::commands::proc::*` callers.
pub use build::{
    agent_row_from_watch, agent_tree_node_to_json, build_agent_cmdline_map, build_agent_ppid_map,
    build_proc_detail,
};
pub use filter::{
    cmdline_matches_pattern, comm_matches_pattern, parse_fd_count, parse_proc_state, ProcFilter,
};
pub use render::{
    csv_escape_field, render_agent_inventory, render_agent_inventory_csv, render_agent_tree,
    render_agent_tree_csv, render_proc_detail_csv, render_proc_detail_text,
};
pub use run::render_once;
pub use run::run;
pub use scan::{
    filter_agent_forests, filter_watched_agents, sort_agent_forests, sort_watched_agents,
};
pub use sort::{limit_agent_forests, limit_watched_agents, parse_proc_limit, ProcSort};
pub use types::{
    AgentAncestorRef, AgentProcNdjsonLine, AgentProcRow, AgentProcSnapshot, AgentTreeNdjsonLine,
    AgentTreeNodeJson, AgentTreeSnapshot, ProcDetailNdjsonLine, ProcDetailSnapshot,
};

// Re-export watch frame marker constants (used by other commands / tests).
pub use ndjson::{PROC_CSV_WATCH_FRAME_MARKER, PROC_PID_CSV_WATCH_FRAME_MARKER};

// Re-exports for sibling commands (`crate::commands::ps` etc.) that need
// `crate::commands::proc::build_agent_state_map` / `build_forest_state_map` /
// `state_text_for_pid` without taking a direct `sharecli_fleet` dependency.
pub use sharecli_fleet::{build_agent_state_map, build_forest_state_map, state_text_for_pid};

// Private use aliases — required so the glob `use crate::commands::proc::*;`
// inside the unit-test module (proc/tests.rs) can reach `DetectedAgentWatch`,
// `watch_detected_agents`, `HashMap`, `HostResourceWatchJson`, etc. The
// original monolithic proc/mod.rs carried the same private `use` statements;
// we preserve that surface so the 30+ test fns keep compiling unchanged.
use crate::monitoring::HostResourceWatchJson;
use sharecli_fleet::{
    build_host_agent_forests, format_gate_status_from_snapshot, format_gate_status_section,
    format_rss_bytes, gate_status_snapshot, lookup_proc, match_known_agent, parse_rss_bytes,
    scan_host_agents, walk_agent_ancestors, watch_detected_agents, AgentResourceSample,
    AgentTreeNode, DetectedAgentWatch, HostProcSource, ProcSource, ThermalLevel,
};
use std::collections::HashMap;

// Re-export (within the crate) helper fns that live in submodules but are
// referenced directly from `proc/tests.rs` via the `crate::commands::proc::*`
// glob import. The original 1518L proc/mod.rs held these as private free
// fns; after decomposition they live in `build.rs` / `ndjson.rs` as
// `pub(crate)` and are surfaced here so the test surface is unchanged.
pub(crate) use build::{
    format_cmdline, state_json_from_char, state_letter_for_pid, state_text_from_detail_state,
};
pub(crate) use ndjson::append_gate_csv_companion;
