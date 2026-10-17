//! Operator-panel line builders (level / gate / runtime / resource / IO).
//!
//! Each submodule owns a single panel family's pure line builder. The `render`
//! module wires them into the TUI layout.

pub mod gate;
pub mod io;
pub mod level;
pub mod resource;
pub mod runtime;

pub use gate::gate_panel_lines;
pub use io::{
    fuse_coalesce_lines, fuse_neg_dentry_lines, fuse_write_serialize_lines,
    hypervisor_coalesce_lines, hypervisor_slot_queue_lines, mesh_maildir_lines,
};
pub use level::{
    decision_color, gate_decision, is_compact, level_color, level_label, level_pressure_raw,
    slot_color, slot_ratio, COMPACT_WIDTH,
};
pub use resource::{host_agent_lines, resource_watch_lines};
pub use runtime::{pool_panel_lines, status_panel_lines};
