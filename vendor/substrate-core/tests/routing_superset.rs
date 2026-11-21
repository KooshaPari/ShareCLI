//! Routing superset: strategy selection, circuit breaker, weighted fallback.

use std::collections::HashMap;

use substrate_core::routing_port::{
    CircuitBreaker, CircuitBreakerConfig, CircuitState, FallbackEntry, RoutingPoolState,
    RoutingSelector, RoutingStrategy, RoutingSuperset, RoutingTarget, TargetHealth,
};

fn target(id: &str, weight: u32) -> RoutingTarget {
    RoutingTarget {
        id: id.to_string(),
        engine: "forge".to_string(),
        model: format!("model-{id}"),
        weight,
    }
}

fn breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout_secs: 60,
    }
}

#[test]
fn round_robin_cycles_through_healthy_targets() {
    let pool = vec![target("a", 1), target("b", 1), target("c", 1)];
    let mut state = RoutingPoolState::default();
    let now = 1_000;

    let i0 =
        RoutingSelector::select(RoutingStrategy::RoundRobin, &pool, &mut state, now, 0).unwrap();
    let i1 =
        RoutingSelector::select(RoutingStrategy::RoundRobin, &pool, &mut state, now, 0).unwrap();
    let i2 =
        RoutingSelector::select(RoutingStrategy::RoundRobin, &pool, &mut state, now, 0).unwrap();
    let i3 =
        RoutingSelector::select(RoutingStrategy::RoundRobin, &pool, &mut state, now, 0).unwrap();

    assert_eq!(pool[i0].id, "a");
    assert_eq!(pool[i1].id, "b");
    assert_eq!(pool[i2].id, "c");
    assert_eq!(pool[i3].id, "a");
}

#[test]
fn weighted_distribution_respects_weights() {
    let pool = vec![target("heavy", 3), target("light", 1)];
    let mut state = RoutingPoolState::default();
    let now = 1_000;

    let picks: Vec<String> = (0..8)
        .map(|_| {
            let idx = RoutingSelector::select(RoutingStrategy::Weighted, &pool, &mut state, now, 0)
                .unwrap();
            pool[idx].id.clone()
        })
        .collect();

    let heavy = picks.iter().filter(|id| id.as_str() == "heavy").count();
    let light = picks.iter().filter(|id| id.as_str() == "light").count();
    assert_eq!(heavy, 6);
    assert_eq!(light, 2);
}

#[test]
fn least_used_picks_minimum_in_flight() {
    let pool = vec![target("a", 1), target("b", 1), target("c", 1)];
    let mut state = RoutingPoolState::default();
    state.ensure_targets(&pool, breaker_config());
    state.health.get_mut("a").unwrap().in_flight = 5;
    state.health.get_mut("b").unwrap().in_flight = 1;
    state.health.get_mut("c").unwrap().in_flight = 3;
    let now = 1_000;

    let idx =
        RoutingSelector::select(RoutingStrategy::LeastUsed, &pool, &mut state, now, 0).unwrap();
    assert_eq!(pool[idx].id, "b");
}

#[test]
fn power_of_two_choices_picks_lower_load() {
    let pool = vec![target("a", 1), target("b", 1), target("c", 1)];
    let mut state = RoutingPoolState::default();
    state.ensure_targets(&pool, breaker_config());
    state.health.get_mut("a").unwrap().in_flight = 10;
    state.health.get_mut("b").unwrap().in_flight = 2;
    state.health.get_mut("c").unwrap().in_flight = 8;
    let now = 1_000;

    // counter=1 -> indices (1,2) -> b vs c -> pick b (lower load)
    let idx = RoutingSelector::select(
        RoutingStrategy::PowerOfTwoChoices,
        &pool,
        &mut state,
        now,
        1,
    )
    .unwrap();
    assert_eq!(pool[idx].id, "b");
}

#[test]
fn circuit_breaker_closed_to_open_after_n_failures() {
    let config = breaker_config();
    let mut cb = CircuitBreaker::new(config);
    let t0 = 100;

    assert_eq!(cb.effective_state(t0), CircuitState::Closed);
    cb.record_failure(t0);
    cb.record_failure(t0);
    assert_eq!(cb.effective_state(t0), CircuitState::Closed);
    cb.record_failure(t0);
    assert_eq!(cb.effective_state(t0), CircuitState::Open);
    assert!(!cb.allow_request(t0));
}

#[test]
fn circuit_breaker_open_to_half_open_after_timeout() {
    let config = breaker_config();
    let mut cb = CircuitBreaker::new(config);
    let t0 = 100;
    for _ in 0..3 {
        cb.record_failure(t0);
    }
    assert_eq!(cb.effective_state(t0), CircuitState::Open);
    assert_eq!(cb.effective_state(t0 + 59), CircuitState::Open);
    assert_eq!(cb.effective_state(t0 + 60), CircuitState::HalfOpen);
    assert!(cb.allow_request(t0 + 60));
}

#[test]
fn circuit_breaker_half_open_to_closed_on_success() {
    let config = breaker_config();
    let mut cb = CircuitBreaker::new(config);
    let t0 = 100;
    for _ in 0..3 {
        cb.record_failure(t0);
    }
    let t1 = t0 + 60;
    assert_eq!(cb.effective_state(t1), CircuitState::HalfOpen);
    cb.record_success(t1);
    assert_eq!(cb.effective_state(t1), CircuitState::Closed);
    assert_eq!(cb.consecutive_failures(), 0);
}

#[test]
fn circuit_breaker_half_open_to_open_on_failure() {
    let config = breaker_config();
    let mut cb = CircuitBreaker::new(config);
    let t0 = 100;
    for _ in 0..3 {
        cb.record_failure(t0);
    }
    let t1 = t0 + 60;
    assert_eq!(cb.effective_state(t1), CircuitState::HalfOpen);
    cb.record_failure(t1);
    assert_eq!(cb.effective_state(t1), CircuitState::Open);
}

#[test]
fn fallback_skips_open_targets_in_order() {
    let chain = vec![
        FallbackEntry {
            rank: 0,
            target: target("primary", 1),
            weight: 1,
        },
        FallbackEntry {
            rank: 1,
            target: target("secondary", 1),
            weight: 1,
        },
    ];
    let mut health: HashMap<String, TargetHealth> = HashMap::new();
    health.insert(
        "primary".to_string(),
        TargetHealth {
            breaker: {
                let mut cb = CircuitBreaker::new(breaker_config());
                for _ in 0..3 {
                    cb.record_failure(100);
                }
                cb
            },
            in_flight: 0,
        },
    );
    health.insert("secondary".to_string(), TargetHealth::default());

    let picked = RoutingSelector::select_fallback(&chain, &health, &mut 0, 150).unwrap();
    assert_eq!(picked.id, "secondary");
}

#[test]
fn routing_superset_route_and_record_outcome() {
    let pool = vec![target("a", 1), target("b", 1)];
    let fallback = vec![FallbackEntry {
        rank: 1,
        target: target("backup", 1),
        weight: 1,
    }];
    let mut superset = RoutingSuperset::new(
        pool,
        fallback,
        RoutingStrategy::RoundRobin,
        breaker_config(),
    );

    let decision = superset.route(1_000).unwrap();
    assert!(decision.decision.model.starts_with("model-"));
    superset.record_outcome(&decision.target_id, true, 1_000);
}
