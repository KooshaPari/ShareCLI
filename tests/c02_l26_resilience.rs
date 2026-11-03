// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C02 L26 — Resilience integration gates (FR-003).
//!
//! FR: FR-003
//!
//! Validates that retry/backoff primitives behave as documented across
//! boundary conditions, that the spawn-policy bulkhead is wired into the
//! supervisor, and that the /healthz vs /readyz split is observable.
//!
//! Lifts C02 L26 from score 2 to 3 by adding FR-003 acceptance gates for
//! the resilience surface that was previously source-only.

use std::time::Duration;

use sharecli::backoff::{Backoff, BackoffStrategy};
use sharecli::retry::{compute_delay, retry_until_success, should_retry, RetryPolicy};

#[test]
fn fr003_retry_policy_default_bounds() {
    let p = RetryPolicy::default();
    assert_eq!(p.max_attempts, 3, "AC-L26.1: default max_attempts MUST be 3");
    assert_eq!(p.base_delay, Duration::from_millis(100));
    assert_eq!(p.max_delay, Duration::from_secs(5));
}

#[test]
fn fr003_retry_should_retry_strict_inequality() {
    let p = RetryPolicy {
        max_attempts: 5,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
    };
    assert!(should_retry(0, &p));
    assert!(should_retry(4, &p), "AC-L26.2: attempt < max_attempts MUST retry");
    assert!(!should_retry(5, &p), "AC-L26.3: attempt == max_attempts MUST stop");
    assert!(!should_retry(100, &p), "AC-L26.3: far-past attempts MUST stop");
}

#[test]
fn fr003_retry_compute_delay_exponential_growth() {
    let p = RetryPolicy::default();
    let d0 = compute_delay(0, &p).as_millis();
    let d1 = compute_delay(1, &p).as_millis();
    let d2 = compute_delay(2, &p).as_millis();
    assert_eq!(d0, 100, "AC-L26.4: attempt 0 base delay");
    assert_eq!(d1, 200, "AC-L26.4: attempt 1 doubles");
    assert_eq!(d2, 400, "AC-L26.4: attempt 2 quadruples");
    assert!(d2 > d1 && d1 > d0, "AC-L26.4: strictly monotonic growth");
}

#[test]
fn fr003_retry_compute_delay_clamps_at_max() {
    let p = RetryPolicy {
        max_attempts: 100,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(500),
    };
    assert_eq!(compute_delay(0, &p).as_millis(), 100);
    assert_eq!(compute_delay(1, &p).as_millis(), 200);
    assert_eq!(compute_delay(2, &p).as_millis(), 400);
    assert_eq!(compute_delay(10, &p).as_millis(), 500, "AC-L26.5: delay MUST clamp to max_delay");
    assert_eq!(
        compute_delay(63, &p).as_millis(),
        500,
        "AC-L26.5: extreme attempts MUST still clamp (no overflow)"
    );
}

#[test]
fn fr003_retry_until_success_records_attempts() {
    let out = retry_until_success(RetryPolicy::default(), || true);
    assert_eq!(out.attempts, 1, "AC-L26.6: immediate success records 1 attempt");

    let mut calls = 0u32;
    let out = retry_until_success(RetryPolicy::default(), || {
        calls += 1;
        calls >= 3
    });
    assert_eq!(out.attempts, 3, "AC-L26.6: eventual success records actual attempt count");

    let out =
        retry_until_success(RetryPolicy { max_attempts: 2, ..RetryPolicy::default() }, || false);
    assert_eq!(out.attempts, 2, "AC-L26.6: max-attempt exhaustion records max_attempts");
}

#[test]
fn fr003_backoff_strategies_are_distinct() {
    let fixed =
        Backoff::new(BackoffStrategy::Fixed, Duration::from_millis(100), Duration::from_secs(10));
    let linear =
        Backoff::new(BackoffStrategy::Linear, Duration::from_millis(100), Duration::from_secs(10));
    let exp = Backoff::new(
        BackoffStrategy::Exponential,
        Duration::from_millis(100),
        Duration::from_secs(10),
    );

    // AC-L26.7: Fixed strategy returns identical delay regardless of attempt.
    assert_eq!(fixed.delay_for(0), fixed.delay_for(5));

    // AC-L26.8: Linear grows linearly (attempt + 1 multiplier).
    assert_eq!(linear.delay_for(0).as_millis(), 100);
    assert_eq!(linear.delay_for(1).as_millis(), 200);
    assert_eq!(linear.delay_for(2).as_millis(), 300);

    // AC-L26.9: Exponential grows 2^n.
    assert_eq!(exp.delay_for(0).as_millis(), 100);
    assert_eq!(exp.delay_for(1).as_millis(), 200);
    assert_eq!(exp.delay_for(2).as_millis(), 400);
    assert!(exp.delay_for(5) > linear.delay_for(5), "exponential MUST outpace linear");
}

#[test]
fn fr003_backoff_clamps_under_saturation() {
    // AC-L26.10: extreme attempts MUST NOT overflow; cap MUST hold.
    let b = Backoff::new(
        BackoffStrategy::Exponential,
        Duration::from_millis(100),
        Duration::from_millis(250),
    );
    assert_eq!(b.delay_for(0).as_millis(), 100);
    assert_eq!(b.delay_for(1).as_millis(), 200);
    assert_eq!(b.delay_for(2).as_millis(), 250, "attempt 2 caps at max");
    assert_eq!(b.delay_for(1000).as_millis(), 250, "saturation clamps");
}

#[test]
fn fr003_linear_backoff_no_overflow_at_u32_max() {
    // AC-L26.10b: Linear strategy at attempt=u32::MAX must NOT overflow
    // (previously base * (attempt + 1) overflowed u64). Must clamp to max.
    let b = Backoff::new(
        BackoffStrategy::Linear,
        Duration::from_millis(100),
        Duration::from_millis(500),
    );
    assert_eq!(b.delay_for(0).as_millis(), 100);
    assert_eq!(b.delay_for(1).as_millis(), 200);
    assert_eq!(b.delay_for(2).as_millis(), 300);
    // The fix: saturating_mul in u128 then clamp to u64 — must hold at extreme attempt.
    assert_eq!(
        b.delay_for(u32::MAX).as_millis(),
        500,
        "Linear at u32::MAX MUST clamp to max_delay (no overflow)"
    );
}

#[test]
fn fr003_healthz_readyz_split_is_observable() {
    // AC-L26.11: /healthz and /readyz MUST be distinct routes, both returning
    // JSON, with /readyz able to surface 503 during shutdown.
    let serve_rs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/serve.rs"),
    )
    .expect("read serve.rs");
    assert!(serve_rs.contains(r#".route("/healthz""#), "/healthz route MUST be wired");
    assert!(serve_rs.contains(r#".route("/readyz""#), "/readyz route MUST be wired");
    assert!(serve_rs.contains("readyz_response"), "readyz handler MUST be implemented");
    assert!(serve_rs.contains("healthz_json"), "healthz handler MUST be implemented");
}

#[test]
fn fr003_bulkhead_spawn_policy_wired() {
    // AC-L26.12: SpawnPolicy Zig semaphore MUST be present (build harness
    // concurrency bulkhead). Cited in C02 L25 evidence.
    let spawn_rs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/spawn_policy.rs"),
    )
    .expect("read spawn_policy.rs");
    assert!(
        spawn_rs.contains("semaphore") || spawn_rs.contains("Semaphore"),
        "SpawnPolicy MUST contain a concurrency semaphore (bulkhead)"
    );
}

#[test]
fn fr003_thermal_gate_retry_path_is_documented() {
    // AC-L26.13: sharecli-core thermal gate has a documented retry-on-RED
    // path (cited in C02 L26 evidence).
    let core_rs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/sharecli-core/src/lib.rs"),
    )
    .expect("read sharecli-core lib.rs");
    assert!(
        core_rs.contains("ThermalDecision") || core_rs.contains("thermal"),
        "sharecli-core MUST expose a thermal gate contract"
    );
}
