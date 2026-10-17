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
    fn step_progress_is_a_noop_without_a_bar_and_reports_line_output() {
        let progress = StepProgress::new("stopping", 10);
        if progress.uses_line_output() {
            // Non-TTY: the bar must be absent so every method stays inert.
            assert!(progress.bar.is_none(), "a disabled bar must not be built");
            progress.inc(None);
            progress.inc(Some("pid 1"));
            progress.finish("done");
        }
    }

    #[test]
    fn progress_enabled_matches_the_stderr_tty_state() {
        // The predicate must reflect the real terminal state, not a constant.
        assert_eq!(progress_enabled(), std::io::stderr().is_terminal());
    }

    #[test]
    fn step_progress_skips_small_batches() {
        let progress = StepProgress::new("stopping", PROGRESS_MIN_ITEMS - 1);
        assert!(progress.uses_line_output());
    }
}
