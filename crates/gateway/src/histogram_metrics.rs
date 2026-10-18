//! In-memory histogram with a fixed bucket boundary list. Records a stream of
//! observations and produces `(bucket_boundary, count_of_obs_le_boundary)`
//! snapshots for the Prometheus exposition layer.

use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct Histogram {
    boundaries: Vec<f64>,
    counts: Vec<u64>,
    /// All recorded observations retained so that exact-quantile estimates can
    /// be computed in `p50`. For the MVP, the histogram's `snapshot()` only
    /// needs the bucket counts, but a full copy is also stored so the demo
    /// gauge can report a real p50 latency.
    observations: Vec<f64>,
}

impl Histogram {
    /// Create a new histogram. `boundaries` must be strictly increasing and
    /// non-empty; otherwise the result is the same shape but every `record`
    /// will fall into the implicit `+Inf` bucket.
    pub fn with_buckets(boundaries: &[f64]) -> Self {
        let mut sorted: Vec<f64> = boundaries.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let n = sorted.len();
        Histogram { boundaries: sorted, counts: vec![0; n], observations: Vec::new() }
    }

    pub fn record(&mut self, value: f64) {
        self.observations.push(value);
        let n = self.boundaries.len();
        for i in 0..n {
            if value <= self.boundaries[i] {
                self.counts[i] += 1;
            }
        }
    }

    /// Iterator over `(bucket_upper_bound, cumulative_count_of_obs_le_bound)`.
    /// Used by the Prometheus exposition layer to emit one `_bucket{le=…}`
    /// sample per boundary, plus the `+Inf` bucket which equals the total.
    pub fn snapshot(&self) -> impl Iterator<Item = (f64, u64)> + '_ {
        self.boundaries.iter().copied().zip(self.counts.iter().copied())
    }

    /// Total observations (including those that fell into `+Inf`).
    pub fn count(&self) -> u64 {
        self.observations.len() as u64
    }

    /// p50 estimate via the "nearest rank" method on the recorded observations.
    /// Returns 0.0 when the histogram is empty so the demo gauge can be
    /// emitted unconditionally.
    pub fn p50(&self) -> f64 {
        if self.observations.is_empty() {
            return 0.0;
        }
        let mut sorted = self.observations.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let rank = ((sorted.len() as f64) * 0.5).ceil() as usize;
        let idx = rank.saturating_sub(1).min(sorted.len() - 1);
        sorted[idx]
    }
}
