//! Spawn / stop audit emission and `ProcState` value type.
//!
//! `ProcState` is the stable, serialisable mapping of `sysinfo::ProcessStatus`
//! used by tray / dashboard IPC payloads — the wire format must be platform-
//! independent, so we collapse platform-specific variants to a small enum.
//!
//! `spawn_capability` / `emit_spawn_audit` / `emit_stop_audit` are the
//! `crate::audit_log` wrappers used by `ProcessPool` whenever a substrate
//! `ProcessPort` operation succeeds, fails, or is denied.

use serde_json::json;
use sysinfo::ProcessStatus;

use crate::audit_log;

/// Project + harness name -> audit-log "capability" string. Used to identify
/// which logical subsystem invoked a spawn (so audit consumers can group by
/// project or harness without parsing argv).
pub(crate) fn spawn_capability(cmd: &str, harness: &Option<String>) -> String {
    harness.clone().unwrap_or_else(|| cmd.to_string())
}

/// Emit a `spawn` audit-log record. `outcome` is one of `"ok"` / `"denied"`.
/// When `pid` is `Some`, it is included in the JSON payload.
pub(crate) fn emit_spawn_audit(
    project: &Option<String>,
    capability: &str,
    outcome: &str,
    pid: Option<u32>,
) {
    let mut fields = json!({
        "project": project,
        "capability": capability,
        "outcome": outcome,
    });
    if let Some(pid) = pid {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("pid".to_string(), json!(pid));
        }
    }
    audit_log::emit_if_configured("spawn", fields);
}

/// Emit a `stop` audit-log record. `outcome` is one of `"ok"` / `"denied"`.
pub(crate) fn emit_stop_audit(project: &Option<String>, capability: &str, pid: u32, outcome: &str) {
    audit_log::emit_if_configured(
        "stop",
        json!({
            "project": project,
            "pid": pid,
            "capability": capability,
            "outcome": outcome,
        }),
    );
}

/// Lightweight `ProcessStatus` value that survives serialisation. sysinfo's enum
/// has many variants gated per-platform — for IPC purposes we only need the
/// handful users ever see on the dashboard, mapped through this enum so the
/// wire format is stable across the Linux/macOS/Windows tray crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcState {
    Idle,
    Run,
    Sleep,
    Stop,
    Zombie,
    Tracing,
    Dead,
    #[default]
    Unknown,
}

impl From<ProcessStatus> for ProcState {
    fn from(s: ProcessStatus) -> Self {
        match s {
            ProcessStatus::Idle => Self::Idle,
            ProcessStatus::Run => Self::Run,
            ProcessStatus::Sleep => Self::Sleep,
            ProcessStatus::Stop => Self::Stop,
            ProcessStatus::Zombie => Self::Zombie,
            ProcessStatus::Tracing => Self::Tracing,
            ProcessStatus::Dead => Self::Dead,
            // Wakekill / Waking / Parked / Blocked / Unknown collapse to Unknown
            // on the dashboard. The exact variant is rarely useful to the user.
            _ => Self::Unknown,
        }
    }
}
