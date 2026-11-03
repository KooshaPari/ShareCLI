// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FUSE write-serialize / CoW operator meters (FR-009 / AC-009.10).

use std::sync::atomic::{AtomicU64, Ordering};

/// Snapshot of write-serialize / CoW counters across all FUSE intercepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WriteSerializeMeters {
    /// Passthrough writes serialized per path (`write_rel` / FUSE write).
    pub passthrough_writes: u64,
    /// CoW staging operations (`stage_bytes` / `stage_rel`).
    pub stages: u64,
    /// Staging promotions to backing (`commit_pending` / `commit_rel`).
    pub commits: u64,
    /// Staging drops without touching backing (`discard_pending` / `discard_rel`).
    pub discards: u64,
}

impl WriteSerializeMeters {
    /// Operator-facing status block for `sharecli status` (FR-009 / AC-009.10).
    pub fn format_status_section(self) -> String {
        let mut out = String::from("\n=== FUSE Write Serialize ===\n\n");
        out.push_str(&format!(
            "Passthrough:  {}\nStages:       {}\nCommits:      {}\nDiscards:     {}\n",
            self.passthrough_writes, self.stages, self.commits, self.discards
        ));
        out
    }
}

static GLOBAL_PASSTHROUGH: AtomicU64 = AtomicU64::new(0);
static GLOBAL_STAGES: AtomicU64 = AtomicU64::new(0);
static GLOBAL_COMMITS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_DISCARDS: AtomicU64 = AtomicU64::new(0);

/// Process-wide aggregate of write-serialize / CoW events.
pub fn global_write_serialize_meters() -> WriteSerializeMeters {
    WriteSerializeMeters {
        passthrough_writes: GLOBAL_PASSTHROUGH.load(Ordering::Relaxed),
        stages: GLOBAL_STAGES.load(Ordering::Relaxed),
        commits: GLOBAL_COMMITS.load(Ordering::Relaxed),
        discards: GLOBAL_DISCARDS.load(Ordering::Relaxed),
    }
}

/// Record a serialized passthrough write.
pub fn record_passthrough_write() {
    GLOBAL_PASSTHROUGH.fetch_add(1, Ordering::Relaxed);
}

/// Record a CoW stage operation.
pub fn record_stage() {
    GLOBAL_STAGES.fetch_add(1, Ordering::Relaxed);
}

/// Record a successful CoW commit.
pub fn record_commit() {
    GLOBAL_COMMITS.fetch_add(1, Ordering::Relaxed);
}

/// Record a successful CoW discard.
pub fn record_discard() {
    GLOBAL_DISCARDS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_write_serialize_meters_record_all_kinds() {
        let before = global_write_serialize_meters();
        record_passthrough_write();
        record_stage();
        record_commit();
        record_discard();
        let after = global_write_serialize_meters();
        assert_eq!(after.passthrough_writes, before.passthrough_writes + 1);
        assert_eq!(after.stages, before.stages + 1);
        assert_eq!(after.commits, before.commits + 1);
        assert_eq!(after.discards, before.discards + 1);
        let section = after.format_status_section();
        assert!(section.contains("=== FUSE Write Serialize ==="));
        assert!(section.contains("Passthrough:"));
        assert!(section.contains("Stages:"));
    }
}
