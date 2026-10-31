// SPDX-License-Identifier: Apache-2.0 OR MIT
//! In-process HTTP RED metrics for `sharecli serve`.
//!
//! Counters/histograms are process-local atomics updated by the serve
//! observability middleware and scraped via `/metrics/prometheus`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Rate / Errors / Duration snapshot for Prometheus exposition.
#[derive(Debug, Default)]
pub struct HttpRedMetrics {
    pub requests_total: AtomicU64,
    pub errors_total: AtomicU64,
    /// HTTP 401 responses (AuthN failures on protected routes).
    pub unauthorized_total: AtomicU64,
    pub duration_count: AtomicU64,
    pub duration_sum_ms: AtomicU64,
    pub bucket_le_5ms: AtomicU64,
    pub bucket_le_25ms: AtomicU64,
    pub bucket_le_100ms: AtomicU64,
    pub bucket_le_inf: AtomicU64,
}

impl HttpRedMetrics {
    pub fn record(&self, status: u16, elapsed: Duration) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        if status >= 500 {
            self.errors_total.fetch_add(1, Ordering::Relaxed);
        }
        if status == 401 {
            self.unauthorized_total.fetch_add(1, Ordering::Relaxed);
        }
        let ms = elapsed.as_millis() as u64;
        self.duration_count.fetch_add(1, Ordering::Relaxed);
        self.duration_sum_ms.fetch_add(ms, Ordering::Relaxed);
        self.bucket_le_inf.fetch_add(1, Ordering::Relaxed);
        if ms <= 5 {
            self.bucket_le_5ms.fetch_add(1, Ordering::Relaxed);
        }
        if ms <= 25 {
            self.bucket_le_25ms.fetch_add(1, Ordering::Relaxed);
        }
        if ms <= 100 {
            self.bucket_le_100ms.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn snapshot(&self) -> HttpRedSnapshot {
        HttpRedSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            errors_total: self.errors_total.load(Ordering::Relaxed),
            unauthorized_total: self.unauthorized_total.load(Ordering::Relaxed),
            duration_count: self.duration_count.load(Ordering::Relaxed),
            duration_sum_ms: self.duration_sum_ms.load(Ordering::Relaxed),
            bucket_le_5ms: self.bucket_le_5ms.load(Ordering::Relaxed),
            bucket_le_25ms: self.bucket_le_25ms.load(Ordering::Relaxed),
            bucket_le_100ms: self.bucket_le_100ms.load(Ordering::Relaxed),
            bucket_le_inf: self.bucket_le_inf.load(Ordering::Relaxed),
        }
    }
}

/// Immutable copy for pure Prometheus rendering / tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HttpRedSnapshot {
    pub requests_total: u64,
    pub errors_total: u64,
    pub unauthorized_total: u64,
    pub duration_count: u64,
    pub duration_sum_ms: u64,
    pub bucket_le_5ms: u64,
    pub bucket_le_25ms: u64,
    pub bucket_le_100ms: u64,
    pub bucket_le_inf: u64,
}

/// Append RED HTTP series to a Prometheus text buffer.
pub fn render_http_red_metrics(out: &mut String, red: &HttpRedSnapshot) {
    out.push_str(
        "# HELP sharecli_http_requests_total Total HTTP requests handled by sharecli serve\n",
    );
    out.push_str("# TYPE sharecli_http_requests_total counter\n");
    out.push_str(&format!("sharecli_http_requests_total {}\n", red.requests_total));

    out.push_str("# HELP sharecli_http_errors_total HTTP responses with status >= 500\n");
    out.push_str("# TYPE sharecli_http_errors_total counter\n");
    out.push_str(&format!("sharecli_http_errors_total {}\n", red.errors_total));

    out.push_str("# HELP sharecli_http_unauthorized_total HTTP 401 responses (AuthN failures)\n");
    out.push_str("# TYPE sharecli_http_unauthorized_total counter\n");
    out.push_str(&format!("sharecli_http_unauthorized_total {}\n", red.unauthorized_total));

    out.push_str("# HELP sharecli_http_request_duration_ms HTTP request latency in milliseconds\n");
    out.push_str("# TYPE sharecli_http_request_duration_ms histogram\n");
    out.push_str(&format!(
        "sharecli_http_request_duration_ms_bucket{{le=\"5\"}} {}\n",
        red.bucket_le_5ms
    ));
    out.push_str(&format!(
        "sharecli_http_request_duration_ms_bucket{{le=\"25\"}} {}\n",
        red.bucket_le_25ms
    ));
    out.push_str(&format!(
        "sharecli_http_request_duration_ms_bucket{{le=\"100\"}} {}\n",
        red.bucket_le_100ms
    ));
    out.push_str(&format!(
        "sharecli_http_request_duration_ms_bucket{{le=\"+Inf\"}} {}\n",
        red.bucket_le_inf
    ));
    out.push_str(&format!("sharecli_http_request_duration_ms_sum {}\n", red.duration_sum_ms));
    out.push_str(&format!("sharecli_http_request_duration_ms_count {}\n", red.duration_count));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_increments_counters_and_buckets() {
        let m = HttpRedMetrics::default();
        m.record(200, Duration::from_millis(3));
        m.record(503, Duration::from_millis(40));
        m.record(401, Duration::from_millis(2));
        let s = m.snapshot();
        assert_eq!(s.requests_total, 3);
        assert_eq!(s.errors_total, 1);
        assert_eq!(s.unauthorized_total, 1);
        assert_eq!(s.bucket_le_5ms, 2);
        assert_eq!(s.bucket_le_25ms, 2);
        assert_eq!(s.bucket_le_100ms, 3);
        assert_eq!(s.bucket_le_inf, 3);
        assert_eq!(s.duration_sum_ms, 45);
    }

    #[test]
    fn render_includes_red_series() {
        let mut out = String::new();
        render_http_red_metrics(
            &mut out,
            &HttpRedSnapshot {
                requests_total: 9,
                errors_total: 1,
                unauthorized_total: 2,
                duration_count: 9,
                duration_sum_ms: 90,
                bucket_le_5ms: 4,
                bucket_le_25ms: 7,
                bucket_le_100ms: 9,
                bucket_le_inf: 9,
            },
        );
        assert!(out.contains("sharecli_http_requests_total 9"));
        assert!(out.contains("sharecli_http_errors_total 1"));
        assert!(out.contains("sharecli_http_unauthorized_total 2"));
        assert!(out.contains("sharecli_http_request_duration_ms_bucket"));
    }
}
