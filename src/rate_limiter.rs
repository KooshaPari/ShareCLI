// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct RateLimiter {
    max_per_window: usize,
    window: Duration,
    hits: Vec<Instant>,
}
impl RateLimiter {
    pub fn new(max_per_window: usize, window: Duration) -> Self {
        Self { max_per_window, window, hits: Vec::new() }
    }
    pub fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let cutoff = now - self.window;
        self.hits.retain(|t| *t > cutoff);
        if self.hits.len() < self.max_per_window {
            self.hits.push(now);
            true
        } else {
            false
        }
    }
    #[allow(dead_code)]
    pub fn available(&self) -> usize {
        self.max_per_window.saturating_sub(self.hits.len())
    }
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.hits.clear();
    }

    /// Seconds until the oldest hit in the current window expires (0 when under cap).
    pub fn retry_after_secs(&self) -> u64 {
        if self.hits.len() < self.max_per_window {
            return 0;
        }
        let now = Instant::now();
        let cutoff = now - self.window;
        let oldest = self.hits.iter().filter(|t| **t > cutoff).min().copied();
        match oldest {
            Some(t) => (self.window.saturating_sub(now.duration_since(t))).as_secs(),
            None => 0,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allows_within() {
        let mut r = RateLimiter::new(3, Duration::from_secs(60));
        assert!(r.try_acquire());
        assert!(r.try_acquire());
        assert!(r.try_acquire());
    }
    #[test]
    fn blocks_over() {
        let mut r = RateLimiter::new(2, Duration::from_secs(60));
        r.try_acquire();
        r.try_acquire();
        assert!(!r.try_acquire());
    }
    #[test]
    fn available_decrements() {
        let mut r = RateLimiter::new(5, Duration::from_secs(60));
        r.try_acquire();
        r.try_acquire();
        assert_eq!(r.available(), 3);
    }
    #[test]
    fn reset() {
        let mut r = RateLimiter::new(1, Duration::from_secs(60));
        r.try_acquire();
        r.reset();
        assert!(r.try_acquire());
    }

    #[test]
    fn retry_after_zero_when_under_cap() {
        let r = RateLimiter::new(3, Duration::from_secs(60));
        assert_eq!(r.retry_after_secs(), 0);
    }

    #[test]
    fn retry_after_positive_when_saturated() {
        let mut r = RateLimiter::new(1, Duration::from_secs(60));
        r.try_acquire();
        assert!(r.retry_after_secs() > 0);
    }
}
