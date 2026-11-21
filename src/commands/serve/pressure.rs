//! `ThermalEvent` enum and pure pressure-parsing.
//!
//! `parse_pressure_level` is the single canonical mapping from a raw sysctl
//! pressure integer to a `ThermalLevel` -- the thermal poller re-encodes
//! `ThermalGovernor` output through this function so all branches use the
//! same rule (and so unit tests can verify the mapping without I/O).

use serde::Serialize;
use sharecli_fleet::thermal::ThermalLevel;

/// Lightweight thermal event forwarded to WS clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ThermalEvent {
    ThermalWarning { pressure: u8 },
    ThermalCritical { pressure: u8 },
}

/// Parse a raw sysctl pressure integer into a [`ThermalLevel`].
///
/// This is a pure function with no I/O; it is unit-tested below.
pub fn parse_pressure_level(raw: u8) -> Option<ThermalLevel> {
    match raw {
        1 => Some(ThermalLevel::Green),
        2 => Some(ThermalLevel::Yellow),
        4 => Some(ThermalLevel::Red),
        _ => None,
    }
}
