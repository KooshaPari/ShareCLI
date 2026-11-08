// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Sliding-window HTTP rate limiting for `sharecli serve` (C02 L25).

use std::sync::Mutex;
use std::time::Duration;

use crate::config::ServeConfig;
use crate::rate_limiter::RateLimiter;

/// Probe routes stay unlimited so orchestrators can scrape liveness/readiness.
pub fn is_probe_path(path: &str) -> bool {
    matches!(path, "/healthz" | "/readyz")
}

/// Resolved serve HTTP rate limit (disabled when `max_per_window` is 0).
#[derive(Debug)]
pub struct ServeRateLimit {
    limiter: RateLimiter,
    window: Duration,
    max_per_window: usize,
}

impl ServeRateLimit {
    pub fn new(max_per_window: usize, window: Duration) -> Self {
        Self { limiter: RateLimiter::new(max_per_window, window), window, max_per_window }
    }

    /// Build from `[serve]` config with optional env overrides.
    pub fn from_env_or_config(cfg: &ServeConfig) -> Option<Self> {
        let max = std::env::var("SHARECLI_SERVE_RATE_LIMIT_MAX")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .or(cfg.rate_limit_max)?;
        if max == 0 {
            return None;
        }
        let window_secs = std::env::var("SHARECLI_SERVE_RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .or(cfg.rate_limit_window_secs)
            .unwrap_or(60);
        Some(Self::new(max, Duration::from_secs(window_secs)))
    }

    pub fn try_acquire(&mut self) -> bool {
        self.limiter.try_acquire()
    }

    /// Seconds until the oldest hit in the window expires (for `Retry-After`).
    pub fn retry_after_secs(&self) -> u64 {
        self.limiter.retry_after_secs().max(1)
    }

    pub fn window(&self) -> Duration {
        self.window
    }

    pub fn max_per_window(&self) -> usize {
        self.max_per_window
    }
}

/// Shared optional limiter for axum middleware (`None` = pass-through).
pub type ServeRateLimitState = Mutex<Option<ServeRateLimit>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_paths_exempt() {
        assert!(is_probe_path("/healthz"));
        assert!(is_probe_path("/readyz"));
        assert!(!is_probe_path("/"));
        assert!(!is_probe_path("/metrics/prometheus"));
    }

    #[test]
    fn zero_max_disables_limiter() {
        let cfg = ServeConfig {
            rate_limit_max: Some(0),
            rate_limit_window_secs: Some(60),
            ..ServeConfig::default()
        };
        assert!(ServeRateLimit::from_env_or_config(&cfg).is_none());
    }

    #[test]
    fn config_builds_limiter() {
        let cfg = ServeConfig {
            rate_limit_max: Some(10),
            rate_limit_window_secs: Some(30),
            ..ServeConfig::default()
        };
        let lim = ServeRateLimit::from_env_or_config(&cfg).expect("limiter");
        assert_eq!(lim.window(), Duration::from_secs(30));
    }
}
