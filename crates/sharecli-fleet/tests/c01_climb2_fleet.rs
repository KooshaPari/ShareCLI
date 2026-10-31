// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003
//!
//! C01 climb-2 — fleet operator formatters + thermal governor.

use sharecli_fleet::{
    format_rss_bytes, global_slot_queue_meters, record_slot_acquire, record_slot_timeout,
    record_slot_wait, SlotQueueMeters, ThermalGovernor, ThermalLevel,
};

/// FR-003 / C01 — SlotQueueMeters format + global record counters.
#[test]
fn fr003_fleet_slot_queue_meters_format() {
    let before = global_slot_queue_meters();
    record_slot_acquire();
    record_slot_wait();
    record_slot_timeout();
    let after = global_slot_queue_meters();
    assert!(after.acquires >= before.acquires + 1);
    assert!(after.waits >= before.waits + 1);
    assert!(after.timeouts >= before.timeouts + 1);

    let meters = SlotQueueMeters { acquires: 4, waits: 2, timeouts: 1 };
    let section = meters.format_status_section();
    assert!(section.contains("SlotQueue"));
    assert!(section.contains("Acquires:"));
}

/// FR-003 / C01 — format_rss_bytes edges.
#[test]
fn fr003_fleet_format_rss_bytes() {
    assert!(!format_rss_bytes(0).is_empty());
    assert!(!format_rss_bytes(1024).is_empty());
    assert!(!format_rss_bytes(1_048_576).is_empty());
}

/// FR-003 / C01 — ThermalGovernor mock poll levels.
#[test]
fn fr003_fleet_thermal_governor_mock_levels() {
    for level in [ThermalLevel::Green, ThermalLevel::Yellow, ThermalLevel::Red] {
        let gov = ThermalGovernor::with_mock(level);
        let polled = gov.poll().expect("poll");
        assert_eq!(polled, level);
    }
    let _ = ThermalGovernor::new();
}
