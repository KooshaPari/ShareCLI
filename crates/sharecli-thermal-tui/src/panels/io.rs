//! Hypervisor + FUSE + mesh IO meter panels (FR-007 / FR-008 / FR-009 / FR-010).
//!
//! Six small line builders stacked by the `render` module into the IO block.

use ratatui::text::Line;

use sharecli_fleet::{CoalesceMeters, SlotQueueMeters};
use sharecli_fuse::{NegDentryMeters, ReadCacheMeters, WriteSerializeMeters};
use sharecli_mesh::MaildirStatus;

/// Lines for the FUSE read-coalesce panel (FR-007 / AC-007.9 TUI slice).
pub fn fuse_coalesce_lines(meters: ReadCacheMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " hits:{} miss:{} {}%",
            meters.hits,
            meters.misses,
            meters.hit_rate_pct()
        ))];
    }

    vec![
        Line::from(format!("  Cache hits:   {}", meters.hits)),
        Line::from(format!("  Cache misses: {}", meters.misses)),
        Line::from(format!("  Hit rate:     {}%", meters.hit_rate_pct())),
    ]
}

/// Lines for the FUSE negative-dentry panel (FR-009 / AC-009.9 TUI slice).
pub fn fuse_neg_dentry_lines(meters: NegDentryMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " neg:{} miss:{} {}%",
            meters.hits,
            meters.misses,
            meters.hit_rate_pct()
        ))];
    }

    vec![
        Line::from(format!("  Neg hits:     {}", meters.hits)),
        Line::from(format!("  Neg misses:   {}", meters.misses)),
        Line::from(format!("  Hit rate:     {}%", meters.hit_rate_pct())),
    ]
}

/// Lines for the Hypervisor coalesce cache panel (FR-008 / AC-008.11 TUI slice).
pub fn hypervisor_coalesce_lines(meters: CoalesceMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " coalesce:{} miss:{} nocache:{} {}%",
            meters.hits,
            meters.misses,
            meters.nocache_runs,
            meters.hit_rate_pct()
        ))];
    }

    vec![
        Line::from(format!("  Coalesce hits:   {}", meters.hits)),
        Line::from(format!("  Coalesce misses: {}", meters.misses)),
        Line::from(format!("  Nocache runs:    {}", meters.nocache_runs)),
        Line::from(format!("  Hit rate:        {}%", meters.hit_rate_pct())),
    ]
}

/// Lines for the Hypervisor SlotQueue panel (FR-008 / AC-008.12 TUI slice).
pub fn hypervisor_slot_queue_lines(meters: SlotQueueMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " slot:{} wait:{} to:{}",
            meters.acquires, meters.waits, meters.timeouts
        ))];
    }

    vec![
        Line::from(format!("  Slot acquires: {}", meters.acquires)),
        Line::from(format!("  Slot waits:    {}", meters.waits)),
        Line::from(format!("  Slot timeouts: {}", meters.timeouts)),
    ]
}

/// Lines for the mesh Maildir queue depth panel (FR-010 / AC-010.11 TUI slice).
pub fn mesh_maildir_lines(status: Option<MaildirStatus>, compact: bool) -> Vec<Line<'static>> {
    match status {
        Some(st) if compact => {
            vec![Line::from(format!(" mesh r:{} f:{} p:{}", st.ready, st.in_flight, st.pending))]
        }
        Some(st) => vec![
            Line::from(format!("  Mesh ready:     {}", st.ready)),
            Line::from(format!("  Mesh in-flight: {}", st.in_flight)),
            Line::from(format!("  Mesh pending:   {}", st.pending)),
        ],
        None => vec![Line::from("  Mesh queue:     unavailable")],
    }
}

/// Lines for the FUSE write-serialize / CoW panel (FR-009 / AC-009.10 TUI slice).
pub fn fuse_write_serialize_lines(
    meters: WriteSerializeMeters,
    compact: bool,
) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " wr:{} st:{} cm:{} ds:{}",
            meters.passthrough_writes, meters.stages, meters.commits, meters.discards
        ))];
    }

    vec![
        Line::from(format!("  Passthrough:  {}", meters.passthrough_writes)),
        Line::from(format!("  Stages:       {}", meters.stages)),
        Line::from(format!("  Commits:      {}", meters.commits)),
        Line::from(format!("  Discards:     {}", meters.discards)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuse_coalesce_lines_compact() {
        let meters = ReadCacheMeters { hits: 10, misses: 5, ..Default::default() };
        let lines = fuse_coalesce_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("hits:10"));
        assert!(text.contains("miss:5"));
    }

    #[test]
    fn test_fuse_coalesce_lines_full() {
        let meters = ReadCacheMeters { hits: 10, misses: 5, ..Default::default() };
        let lines = fuse_coalesce_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Cache hits:   10"));
        assert!(text.contains("Cache misses: 5"));
    }

    #[test]
    fn test_fuse_neg_dentry_lines_compact() {
        let meters = NegDentryMeters { hits: 8, misses: 2, ..Default::default() };
        let lines = fuse_neg_dentry_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("neg:8"));
        assert!(text.contains("miss:2"));
    }

    #[test]
    fn test_hypervisor_coalesce_lines_full() {
        let meters = CoalesceMeters { hits: 100, misses: 10, nocache_runs: 3 };
        let lines = hypervisor_coalesce_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Coalesce hits:   100"));
        assert!(text.contains("Nocache runs:    3"));
    }

    #[test]
    fn test_hypervisor_slot_queue_lines_compact() {
        let meters = SlotQueueMeters { acquires: 5, waits: 1, timeouts: 0 };
        let lines = hypervisor_slot_queue_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("slot:5"));
        assert!(text.contains("wait:1"));
    }

    #[test]
    fn test_mesh_maildir_lines_unavailable() {
        let lines = mesh_maildir_lines(None, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Mesh queue:     unavailable"));
    }

    #[test]
    fn test_mesh_maildir_lines_compact_some() {
        let status = MaildirStatus {
            path: std::path::PathBuf::from("/tmp/m"),
            ready: 3,
            in_flight: 1,
            pending: 2,
        };
        let lines = mesh_maildir_lines(Some(status), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("mesh r:3"));
        assert!(text.contains("f:1"));
        assert!(text.contains("p:2"));
    }

    #[test]
    fn test_fuse_write_serialize_lines_full() {
        let meters = WriteSerializeMeters {
            passthrough_writes: 4,
            stages: 6,
            commits: 2,
            discards: 1,
        };
        let lines = fuse_write_serialize_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Passthrough:  4"));
        assert!(text.contains("Stages:       6"));
        assert!(text.contains("Commits:      2"));
        assert!(text.contains("Discards:     1"));
    }
}
