//! Thermal-level transforms (label / color / raw pressure / decision / slot gauge).
//!
//! Pure functions: same input always produces the same output, so each is
//! unit-tested independently of any terminal backend.

use ratatui::style::Color;

use sharecli_fleet::thermal::ThermalLevel;

/// Map a [`ThermalLevel`] to a human-readable label.
pub fn level_label(level: ThermalLevel) -> &'static str {
    match level {
        ThermalLevel::Green => "GREEN",
        ThermalLevel::Yellow => "YELLOW",
        ThermalLevel::Red => "RED",
    }
}

/// Map a [`ThermalLevel`] to a foreground [`Color`].
pub fn level_color(level: ThermalLevel) -> Color {
    match level {
        ThermalLevel::Green => Color::Green,
        ThermalLevel::Yellow => Color::Yellow,
        ThermalLevel::Red => Color::Red,
    }
}

/// Map a [`ThermalLevel`] to the integer pressure value returned by sysctl.
pub fn level_pressure_raw(level: ThermalLevel) -> u8 {
    match level {
        ThermalLevel::Green => 1,
        ThermalLevel::Yellow => 2,
        ThermalLevel::Red => 4,
    }
}

/// The gate's admit/deny decision label given a [`ThermalLevel`].
///
/// Green and Yellow → ADMIT; Red → DENY.
pub fn gate_decision(level: ThermalLevel) -> &'static str {
    match level {
        ThermalLevel::Green | ThermalLevel::Yellow => "ADMIT",
        ThermalLevel::Red => "DENY",
    }
}

/// Color for the gate decision indicator.
pub fn decision_color(level: ThermalLevel) -> Color {
    match level {
        ThermalLevel::Green | ThermalLevel::Yellow => Color::Green,
        ThermalLevel::Red => Color::Red,
    }
}

/// Compute a gauge ratio for the build-slot indicator.
///
/// Returns a value in `[0.0, 1.0]`.  `active` is clamped to `[0, cap]`.
pub fn slot_ratio(active: u32, cap: u32) -> f64 {
    if cap == 0 {
        return 0.0;
    }
    let clamped = active.min(cap) as f64;
    clamped / cap as f64
}

/// Color for the slot-usage gauge: green < 50 %, yellow < 90 %, red otherwise.
pub fn slot_color(active: u32, cap: u32) -> Color {
    let ratio = slot_ratio(active, cap);
    if ratio < 0.5 {
        Color::Green
    } else if ratio < 0.9 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Columns below this width switch the TUI to compact layout (L81.11).
pub const COMPACT_WIDTH: u16 = 80;

/// True when the terminal is narrower than [`COMPACT_WIDTH`].
///
/// Prefer `frame.area().width` (ratatui already reflects `crossterm::terminal::size`)
/// or an explicit `COLUMNS`-derived width in tests.
pub fn is_compact(width: u16) -> bool {
    width < COMPACT_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- level_label ---
    #[test]
    fn test_level_label_green() {
        assert_eq!(level_label(ThermalLevel::Green), "GREEN");
    }

    #[test]
    fn test_level_label_yellow() {
        assert_eq!(level_label(ThermalLevel::Yellow), "YELLOW");
    }

    #[test]
    fn test_level_label_red() {
        assert_eq!(level_label(ThermalLevel::Red), "RED");
    }

    // --- level_color ---
    #[test]
    fn test_level_color_green() {
        assert_eq!(level_color(ThermalLevel::Green), Color::Green);
    }

    #[test]
    fn test_level_color_yellow() {
        assert_eq!(level_color(ThermalLevel::Yellow), Color::Yellow);
    }

    #[test]
    fn test_level_color_red() {
        assert_eq!(level_color(ThermalLevel::Red), Color::Red);
    }

    // --- level_pressure_raw ---
    #[test]
    fn test_pressure_raw_green() {
        assert_eq!(level_pressure_raw(ThermalLevel::Green), 1);
    }

    #[test]
    fn test_pressure_raw_yellow() {
        assert_eq!(level_pressure_raw(ThermalLevel::Yellow), 2);
    }

    #[test]
    fn test_pressure_raw_red() {
        assert_eq!(level_pressure_raw(ThermalLevel::Red), 4);
    }

    // --- gate_decision ---
    #[test]
    fn test_decision_green_admit() {
        assert_eq!(gate_decision(ThermalLevel::Green), "ADMIT");
    }

    #[test]
    fn test_decision_yellow_admit() {
        assert_eq!(gate_decision(ThermalLevel::Yellow), "ADMIT");
    }

    #[test]
    fn test_decision_red_deny() {
        assert_eq!(gate_decision(ThermalLevel::Red), "DENY");
    }

    // --- decision_color ---
    #[test]
    fn test_decision_color_green() {
        assert_eq!(decision_color(ThermalLevel::Green), Color::Green);
    }

    #[test]
    fn test_decision_color_yellow() {
        assert_eq!(decision_color(ThermalLevel::Yellow), Color::Green);
    }

    #[test]
    fn test_decision_color_red() {
        assert_eq!(decision_color(ThermalLevel::Red), Color::Red);
    }

    // --- slot_ratio ---
    #[test]
    fn test_slot_ratio_zero_active() {
        assert!((slot_ratio(0, 4) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_slot_ratio_half() {
        assert!((slot_ratio(2, 4) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_slot_ratio_full() {
        assert!((slot_ratio(4, 4) - 1.0).abs() < 1e-9);
    }

    /// C07 L65 — clamp the upper bound to `cap` (catches a missing `.min`).
    #[test]
    fn test_slot_ratio_overflow_clamped() {
        assert!((slot_ratio(10, 4) - 1.0).abs() < 1e-9);
    }

    /// C07 L65 — degenerate `cap == 0` MUST yield 0.0 (no NaN / panic).
    #[test]
    fn test_slot_ratio_zero_cap() {
        assert!((slot_ratio(5, 0) - 0.0).abs() < 1e-9);
    }

    // --- slot_color ---
    #[test]
    fn test_slot_color_green_below_half() {
        assert_eq!(slot_color(1, 4), Color::Green);
    }

    #[test]
    fn test_slot_color_yellow_between_half_and_90() {
        assert_eq!(slot_color(2, 4), Color::Yellow); // 0.5 → yellow
        assert_eq!(slot_color(3, 4), Color::Yellow); // 0.75 → yellow
    }

    /// FR-003 / C07 L65 — catch `< 0.9` vs `<= 0.9` mutant (exact 90% is red).
    #[test]
    fn test_slot_color_red_at_exact_90_percent() {
        assert_eq!(slot_color(9, 10), Color::Red); // 0.9 → red (not yellow)
    }

    #[test]
    fn test_slot_color_red_at_cap() {
        assert_eq!(slot_color(4, 4), Color::Red); // 1.0 → red
    }

    #[test]
    fn test_slot_color_red_overflow_clamped() {
        assert_eq!(slot_color(10, 4), Color::Red);
    }

    // --- is_compact ---
    #[test]
    fn test_is_compact_columns() {
        assert!(is_compact(40));
        assert!(is_compact(79));
        assert!(!is_compact(80));
        assert!(!is_compact(120));
    }
}
