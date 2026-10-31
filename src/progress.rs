// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C09 L81.7 — indicatif progress bars with ETA for long batch CLI operations.
//!
//! Progress renders to stderr when stderr is a TTY and the batch has at least
//! [`PROGRESS_MIN_ITEMS`] steps. Piped/CI output stays line-oriented.

use std::io::IsTerminal;

use indicatif::{ProgressBar, ProgressStyle};

/// Minimum item count before showing a progress bar (avoids noise for tiny batches).
pub const PROGRESS_MIN_ITEMS: usize = 3;

/// Whether stepped progress should render (stderr TTY).
pub fn progress_enabled() -> bool {
    std::io::stderr().is_terminal()
}

/// Stepped progress with ETA for batch operations (`stop`, `prune`, project group stop).
pub struct StepProgress {
    bar: Option<ProgressBar>,
}

impl StepProgress {
    /// Create progress for `len` steps, or a no-op when disabled / below threshold.
    pub fn new(message: &str, len: usize) -> Self {
        if !progress_enabled() || len < PROGRESS_MIN_ITEMS {
            return Self { bar: None };
        }

        let bar = ProgressBar::new(len as u64);
        bar.set_style(
            ProgressStyle::with_template(
                "{msg} [{bar:40.cyan/blue}] {pos}/{len} {elapsed_precise} ETA {eta}",
            )
            .expect("valid progress template")
            .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        bar.set_message(message.to_string());
        Self { bar: Some(bar) }
    }

    /// Advance one step; optional per-item label updates the bar message.
    pub fn inc(&self, item_label: Option<&str>) {
        if let Some(bar) = &self.bar {
            if let Some(label) = item_label {
                bar.set_message(label.to_string());
            }
            bar.inc(1);
        }
    }

    /// True when progress bar is hidden (non-TTY or small batch) — emit line output instead.
    pub fn uses_line_output(&self) -> bool {
        self.bar.is_none()
    }

    /// Finish and clear the bar.
    pub fn finish(self, message: &str) {
        if let Some(bar) = self.bar {
            bar.finish_with_message(message.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_min_items_is_reasonable() {
        const { assert!(PROGRESS_MIN_ITEMS >= 2) };
    }

    #[test]
    fn step_progress_no_panic_when_disabled() {
        // Non-TTY stderr in CI: bar is None, inc/finish must not panic.
        let progress = StepProgress::new("stopping", 10);
        progress.inc(Some("pid 42"));
        progress.finish("done");
    }

    #[test]
    fn step_progress_skips_small_batches() {
        let progress = StepProgress::new("stopping", PROGRESS_MIN_ITEMS - 1);
        assert!(progress.uses_line_output());
    }

    #[test]
    fn step_progress_uses_line_output_for_zero_length_batch() {
        // Empty batch is a no-op render — `len < PROGRESS_MIN_ITEMS` short-circuits.
        let progress = StepProgress::new("stopping", 0);
        assert!(progress.uses_line_output());
    }

    #[test]
    fn step_progress_uses_line_output_for_one_item_batch() {
        // One-item batch is below the threshold and must use line output.
        let progress = StepProgress::new("stopping", 1);
        assert!(progress.uses_line_output());
    }

    #[test]
    fn step_progress_uses_line_output_at_threshold_minus_two() {
        // Boundary: any value below PROGRESS_MIN_ITEMS uses line output.
        let progress = StepProgress::new("stopping", PROGRESS_MIN_ITEMS - 2);
        assert!(progress.uses_line_output());
    }

    #[test]
    fn step_progress_inc_handles_none_item_label_without_panicking() {
        // In non-TTY (CI) the bar is None; `inc(None)` must be a quiet no-op
        // and not consume the message or move any counter.
        let progress = StepProgress::new("stopping", 10);
        progress.inc(None);
        progress.inc(None);
        // Multiple inc calls do not panic; finish still works.
        progress.finish("done");
    }

    #[test]
    fn step_progress_inc_with_label_is_quiet_no_op_in_non_tty() {
        // Mirror the existing disabled test but with explicit label semantics
        // so the call is exercised even when stderr is a piped stream.
        let progress = StepProgress::new("stopping", 10);
        for i in 0..5 {
            progress.inc(Some(&format!("pid {i}")));
        }
        progress.finish("done");
    }
}
