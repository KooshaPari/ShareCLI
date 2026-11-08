// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Hypervisor SlotQueue operator meters (FR-008 / AC-008.12).

use std::sync::atomic::{AtomicU64, Ordering};

/// Snapshot of nocache [`SlotQueue`] acquire / contention counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SlotQueueMeters {
    /// Successful slot acquisitions (`with_slot` ran the closure).
    pub acquires: u64,
    /// Wait-loop iterations while a slot was unavailable (contention proxy).
    pub waits: u64,
    /// Acquire attempts that exceeded the queue timeout (loud fail).
    pub timeouts: u64,
}

impl SlotQueueMeters {
    /// Operator-facing status block for `sharecli status` (FR-008 / AC-008.12).
    pub fn format_status_section(self) -> String {
        let mut out = String::from("\n=== Hypervisor SlotQueue ===\n\n");
        out.push_str(&format!(
            "Acquires:  {}\nWaits:     {}\nTimeouts:  {}\n",
            self.acquires, self.waits, self.timeouts
        ));
        out
    }
}

static GLOBAL_SLOT_ACQUIRES: AtomicU64 = AtomicU64::new(0);
static GLOBAL_SLOT_WAITS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_SLOT_TIMEOUTS: AtomicU64 = AtomicU64::new(0);

/// Process-wide aggregate of Hypervisor SlotQueue events.
pub fn global_slot_queue_meters() -> SlotQueueMeters {
    SlotQueueMeters {
        acquires: GLOBAL_SLOT_ACQUIRES.load(Ordering::Relaxed),
        waits: GLOBAL_SLOT_WAITS.load(Ordering::Relaxed),
        timeouts: GLOBAL_SLOT_TIMEOUTS.load(Ordering::Relaxed),
    }
}

/// Record a successful slot acquisition.
pub fn record_slot_acquire() {
    GLOBAL_SLOT_ACQUIRES.fetch_add(1, Ordering::Relaxed);
}

/// Record a wait-loop iteration before acquiring a slot.
pub fn record_slot_wait() {
    GLOBAL_SLOT_WAITS.fetch_add(1, Ordering::Relaxed);
}

/// Record a queue timeout (no free slot within configured deadline).
pub fn record_slot_timeout() {
    GLOBAL_SLOT_TIMEOUTS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_slot_queue_meters_record_acquire_wait_timeout() {
        let before = global_slot_queue_meters();
        record_slot_acquire();
        record_slot_wait();
        record_slot_timeout();
        let after = global_slot_queue_meters();
        assert_eq!(after.acquires, before.acquires + 1);
        assert_eq!(after.waits, before.waits + 1);
        assert_eq!(after.timeouts, before.timeouts + 1);
        let section = after.format_status_section();
        assert!(section.contains("=== Hypervisor SlotQueue ==="));
        assert!(section.contains("Acquires:"));
        assert!(section.contains("Waits:"));
        assert!(section.contains("Timeouts:"));
    }
}
