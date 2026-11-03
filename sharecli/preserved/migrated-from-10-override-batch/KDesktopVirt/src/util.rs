// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Generic utility helpers used across `kvirtualstage`.
//!
//! Provides a `time::Duration` parser that accepts human-friendly strings such as
//! `"500ms"`, `"2s"`, `"1m30s"`, `"1h"`, or plain integer seconds (`"60"`).
//! This is useful for configuration values, CLI flags, and any other place
//! where a duration arrives as a string from outside the binary.

use std::time::{Duration, Instant};

use thiserror::Error;

/// Errors returned by [`parse_duration`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseDurationError {
    /// Input was empty.
    #[error("duration string is empty")]
    Empty,
    /// Input did not match any recognized duration form.
    #[error("invalid duration string: {0:?}")]
    Invalid(String),
    /// A numeric component could not be parsed.
    #[error("invalid numeric component {component:?} in duration string {input:?}")]
    InvalidNumber {
        component: String,
        input: String,
    },
    /// Computed duration overflowed `time::Duration` (u64::MAX seconds).
    #[error("duration overflow: {0:?}")]
    Overflow(String),
}

/// Parse a `time::Duration` from a human-friendly string.
///
/// Accepted forms (case-insensitive unit suffixes, optional whitespace):
///
/// * Plain integer seconds: `"60"` -> 60s
/// * Milliseconds: `"500ms"` / `"500MS"`
/// * Seconds: `"30s"`
/// * Minutes: `"5m"`
/// * Hours: `"2h"`
/// * Days: `"1d"`
/// * Composite (sum, in this order): `"1h30m"`, `"1d 2h 30m 15s"`,
///   `"1h30m15s500ms"`
///
/// Whitespace between components is allowed and ignored. An empty string and
/// an unknown suffix are reported as [`ParseDurationError::Empty`] and
/// [`ParseDurationError::Invalid`] respectively.
pub fn parse_duration(input: &str) -> Result<Duration, ParseDurationError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseDurationError::Empty);
    }

    // Fast path: bare integer seconds (e.g. "60" or "-1"). The whole input
    // must be consumed so we don't accidentally accept "60abc" here.
    if let Ok(secs) = trimmed.parse::<i64>() {
        return i64_to_duration(secs, trimmed);
    }

    // Composite path: walk the string, pulling off (number, unit) pairs.
    // Units are recognised in this fixed order so that the longest match wins
    // even if the input is concatenated without spaces.
    let units: &[(&str, u128)] = &[
        ("ms", 1),
        ("millisecond", 1),
        ("milliseconds", 1),
        ("s", 1_000),
        ("sec", 1_000),
        ("secs", 1_000),
        ("second", 1_000),
        ("seconds", 1_000),
        ("m", 60_000),
        ("min", 60_000),
        ("mins", 60_000),
        ("minute", 60_000),
        ("minutes", 60_000),
        ("h", 3_600_000),
        ("hr", 3_600_000),
        ("hrs", 3_600_000),
        ("hour", 3_600_000),
        ("hours", 3_600_000),
        ("d", 86_400_000),
        ("day", 86_400_000),
        ("days", 86_400_000),
    ];

    let lower = trimmed.to_ascii_lowercase();
    let mut total_ms: u128 = 0;
    let mut consumed_all = true;
    let mut idx = 0usize;
    let bytes = lower.as_bytes();

    while idx < bytes.len() {
        // Skip whitespace between components.
        while idx < bytes.len() && (bytes[idx] as char).is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() {
            break;
        }

        // Read the numeric part (integer, optional leading sign).
        let num_start = idx;
        if bytes[idx] == b'+' || bytes[idx] == b'-' {
            idx += 1;
        }
        let digit_start = idx;
        while idx < bytes.len() && (bytes[idx] as char).is_ascii_digit() {
            idx += 1;
        }
        if idx == digit_start {
            consumed_all = false;
            break;
        }
        let num_str = &lower[num_start..idx];

        // Read the unit suffix.
        let unit_start = idx;
        while idx < bytes.len() && (bytes[idx] as char).is_ascii_alphabetic() {
            idx += 1;
        }
        if unit_start == idx {
            // Number without a unit suffix isn't valid in the composite form.
            return Err(ParseDurationError::Invalid(trimmed.to_string()));
        }
        let unit_str = &lower[unit_start..idx];

        let multiplier = match units.iter().find(|(name, _)| *name == unit_str) {
            Some((_, m)) => *m,
            None => return Err(ParseDurationError::Invalid(trimmed.to_string())),
        };

        let value: i128 = num_str.parse().map_err(|_| ParseDurationError::InvalidNumber {
            component: num_str.to_string(),
            input: trimmed.to_string(),
        })?;

        if value < 0 {
            return Err(ParseDurationError::Overflow(trimmed.to_string()));
        }
        let value = value as u128;

        total_ms = total_ms
            .checked_add(value.checked_mul(multiplier).ok_or_else(|| {
                ParseDurationError::Overflow(trimmed.to_string())
            })?)
            .ok_or_else(|| ParseDurationError::Overflow(trimmed.to_string()))?;
    }

    if !consumed_all && total_ms == 0 {
        return Err(ParseDurationError::Invalid(trimmed.to_string()));
    }

    let millis: u64 = total_ms.try_into().map_err(|_| ParseDurationError::Overflow(trimmed.to_string()))?;
    Ok(Duration::from_millis(millis))
}

fn i64_to_duration(secs: i64, original: &str) -> Result<Duration, ParseDurationError> {
    if secs < 0 {
        return Err(ParseDurationError::Overflow(original.to_string()));
    }
    u64::try_from(secs)
        .map(Duration::from_secs)
        .map_err(|_| ParseDurationError::Overflow(original.to_string()))
}

// ---------------------------------------------------------------------------
// Token-bucket rate limiter
// ---------------------------------------------------------------------------

/// Errors returned by [`RateLimiter::try_acquire_many`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RateLimitError {
    /// `n` was zero; acquiring zero tokens is not a valid operation.
    #[error("invalid token count: requested 0 tokens")]
    InvalidCount,
    /// The bucket did not contain enough tokens to satisfy the request.
    #[error("insufficient tokens: requested {requested}, available {available}")]
    InsufficientTokens { requested: u32, available: u32 },
}

/// A simple token-bucket rate limiter.
///
/// The bucket holds at most `capacity` tokens and refills at
/// `refill_per_sec` tokens per second of wall-clock time. Time is driven
/// externally via [`RateLimiter::refill_at`] so the limiter is deterministic
/// when the caller controls the clock.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    capacity: u32,
    tokens: u32,
    refill_per_sec: u32,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter that starts full at `now`.
    pub fn new(capacity: u32, refill_per_sec: u32, now: Instant) -> Self {
        Self {
            capacity,
            tokens: capacity,
            refill_per_sec,
            last_refill: now,
        }
    }

    /// Create a new rate limiter that starts empty at `now`.
    pub fn new_empty(capacity: u32, refill_per_sec: u32, now: Instant) -> Self {
        Self {
            capacity,
            tokens: 0,
            refill_per_sec,
            last_refill: now,
        }
    }

    /// Maximum number of tokens the bucket can hold.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Tokens currently in the bucket (not refilled by this call).
    pub fn available(&self) -> u32 {
        self.tokens
    }

    /// Refill the bucket based on the time elapsed since the last refill.
    ///
    /// `now` must be greater than or equal to the last refill time; if it is
    /// not, the call is a no-op and returns `0`. The bucket is capped at
    /// `capacity`, and the number of tokens actually added is returned.
    pub fn refill_at(&mut self, now: Instant) -> u32 {
        if now <= self.last_refill {
            return 0;
        }
        let elapsed = now.saturating_duration_since(self.last_refill);
        // Saturate-cast seconds to u32; if the elapsed time is huge we cap
        // the addition at the bucket capacity below.
        let elapsed_secs = elapsed.as_secs().saturating_add(if elapsed.subsec_nanos() > 0 { 1 } else { 0 });
        let added = elapsed_secs
            .saturating_mul(u64::from(self.refill_per_sec))
            .min(u64::from(self.capacity));
        let added_u32 = u32::try_from(added).unwrap_or(self.capacity);
        let before = self.tokens;
        self.tokens = self.tokens.saturating_add(added_u32).min(self.capacity);
        self.last_refill = now;
        self.tokens - before
    }

    /// Try to acquire `n` tokens in a single operation.
    ///
    /// On success returns `Ok(n)` and the bucket's token count is reduced by
    /// `n`. On failure the bucket is left untouched and the variant explains
    /// why.
    pub fn try_acquire_many(&mut self, n: u32) -> Result<u32, RateLimitError> {
        if n == 0 {
            return Err(RateLimitError::InvalidCount);
        }
        if n > self.tokens {
            return Err(RateLimitError::InsufficientTokens {
                requested: n,
                available: self.tokens,
            });
        }
        self.tokens -= n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_units() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(5 * 60));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(2 * 3_600));
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(86_400));
    }

    #[test]
    fn parses_composite_with_mixed_case_and_spaces() {
        let got = parse_duration("1H 30m 15s 500MS").unwrap();
        let want = Duration::from_secs(3_600)
            + Duration::from_secs(30 * 60)
            + Duration::from_secs(15)
            + Duration::from_millis(500);
        assert_eq!(got, want);
    }

    #[test]
    fn parses_plain_integer_seconds() {
        assert_eq!(parse_duration("60").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_duration("0").unwrap(), Duration::ZERO);
    }

    #[test]
    fn rejects_empty_and_invalid() {
        assert_eq!(parse_duration(""), Err(ParseDurationError::Empty));
        assert_eq!(parse_duration("   "), Err(ParseDurationError::Empty));
        assert!(matches!(parse_duration("5x"), Err(ParseDurationError::Invalid(_))));
        assert!(matches!(parse_duration("abc"), Err(ParseDurationError::Invalid(_))));
        assert!(matches!(parse_duration("-5s"), Err(ParseDurationError::Overflow(_))));
    }

    #[test]
    fn try_acquire_many_succeeds_and_fails() {
        let t0 = Instant::now();
        let mut rl = RateLimiter::new(5, 10, t0);

        // Drain 3 of the 5 starting tokens.
        assert_eq!(rl.try_acquire_many(3), Ok(3));
        assert_eq!(rl.available(), 2);

        // Asking for 3 when only 2 remain fails without mutating state.
        assert_eq!(
            rl.try_acquire_many(3),
            Err(RateLimitError::InsufficientTokens { requested: 3, available: 2 })
        );
        assert_eq!(rl.available(), 2);

        // A zero-sized request is rejected.
        assert_eq!(rl.try_acquire_many(0), Err(RateLimitError::InvalidCount));
    }

    #[test]
    fn refill_at_adds_tokens_and_caps_at_capacity() {
        let t0 = Instant::now();
        let mut rl = RateLimiter::new(10, 4, t0);

        // Drain the bucket.
        assert_eq!(rl.try_acquire_many(10), Ok(10));
        assert_eq!(rl.available(), 0);

        // After 2 seconds at 4 tokens/sec, exactly 8 tokens should be added.
        let added = rl.refill_at(t0 + Duration::from_secs(2));
        assert_eq!(added, 8);
        assert_eq!(rl.available(), 8);

        // After a further 10 seconds we'd add 40 tokens, but the bucket
        // caps at capacity (10). The reported `added` is the actual delta.
        let added = rl.refill_at(t0 + Duration::from_secs(12));
        assert_eq!(added, 2);
        assert_eq!(rl.available(), 10);
    }
}
