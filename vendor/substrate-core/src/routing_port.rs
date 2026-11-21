//! Routing superset — load-balancing strategies, circuit breakers, weighted fallback.
//!
//! Pure types and selection logic live here (no I/O). Adapters such as
//! `routing-phenotype-router` and the legacy `omniroute-adapter` wire these
//! primitives to real providers.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    domain::RoutingDecision,
    error::{Result, SubstrateError},
};

/// Load-balancing strategy over a pool of candidate targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Cycle through healthy targets in order.
    RoundRobin,
    /// Select proportionally to each target's `weight`.
    Weighted,
    /// Pick the healthy target with the lowest `in_flight` count.
    LeastUsed,
    /// Sample two healthy targets and pick the one with lower load.
    PowerOfTwoChoices,
}

/// Circuit breaker state for a single target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Requests flow normally.
    Closed,
    /// Requests are rejected until the reset timeout elapses.
    Open,
    /// A single probe request is allowed after the reset timeout.
    HalfOpen,
}

/// Thresholds controlling circuit-breaker transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures in `Closed` before opening the circuit.
    pub failure_threshold: u32,
    /// Seconds the circuit stays `Open` before lazy recovery to `HalfOpen`.
    pub reset_timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        CircuitBreakerConfig {
            failure_threshold: 5,
            reset_timeout_secs: 30,
        }
    }
}

/// Per-target circuit breaker with lazy recovery on read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircuitBreaker {
    state: CircuitState,
    consecutive_failures: u32,
    opened_at_secs: Option<u64>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Create a breaker in the `Closed` state.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        CircuitBreaker {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            opened_at_secs: None,
            config,
        }
    }

    /// Current stored state (does not apply lazy timeout recovery).
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Consecutive failure count (for tests and diagnostics).
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Effective state after applying lazy open→half-open recovery.
    pub fn effective_state(&self, now_secs: u64) -> CircuitState {
        match self.state {
            CircuitState::Open => {
                if let Some(opened) = self.opened_at_secs {
                    if now_secs.saturating_sub(opened) >= self.config.reset_timeout_secs {
                        CircuitState::HalfOpen
                    } else {
                        CircuitState::Open
                    }
                } else {
                    CircuitState::Open
                }
            },
            other => other,
        }
    }

    /// Whether a request may be sent to this target at `now_secs`.
    pub fn allow_request(&self, now_secs: u64) -> bool {
        matches!(
            self.effective_state(now_secs),
            CircuitState::Closed | CircuitState::HalfOpen
        )
    }

    /// Record a successful response.
    pub fn record_success(&mut self, now_secs: u64) {
        match self.effective_state(now_secs) {
            CircuitState::Closed | CircuitState::HalfOpen => {
                self.state = CircuitState::Closed;
                self.consecutive_failures = 0;
                self.opened_at_secs = None;
            },
            CircuitState::Open => {},
        }
    }

    /// Record a failed response.
    pub fn record_failure(&mut self, now_secs: u64) {
        match self.effective_state(now_secs) {
            CircuitState::Closed => {
                self.consecutive_failures += 1;
                if self.consecutive_failures >= self.config.failure_threshold {
                    self.state = CircuitState::Open;
                    self.opened_at_secs = Some(now_secs);
                }
            },
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.opened_at_secs = Some(now_secs);
                self.consecutive_failures = self.config.failure_threshold;
            },
            CircuitState::Open => {},
        }
    }
}

/// A routable engine/model target in a load-balanced pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingTarget {
    /// Stable identifier for health tracking.
    pub id: String,
    /// Engine name (e.g. `"forge"`).
    pub engine: String,
    /// Model identifier passed to the engine.
    pub model: String,
    /// Relative weight for [`RoutingStrategy::Weighted`].
    pub weight: u32,
}

/// One entry in an ordered fallback chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FallbackEntry {
    /// Lower rank = higher priority. Entries at the same rank compete by weight.
    pub rank: u32,
    /// The target to route to when this entry is selected.
    pub target: RoutingTarget,
    /// Weight among healthy entries sharing the same `rank`.
    pub weight: u32,
}

/// Health and load counters for a single target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetHealth {
    /// Circuit breaker for this target.
    pub breaker: CircuitBreaker,
    /// In-flight request count (for least-used / P2C).
    pub in_flight: u64,
}

impl Default for TargetHealth {
    fn default() -> Self {
        TargetHealth {
            breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            in_flight: 0,
        }
    }
}

/// Mutable routing pool state (cursors, counters, per-target health).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoutingPoolState {
    /// Cursor for round-robin among healthy indices.
    pub round_robin_cursor: usize,
    /// Monotonic counter driving weighted / P2C selection.
    pub selection_counter: u64,
    /// Per-target health keyed by target id.
    pub health: HashMap<String, TargetHealth>,
}

impl RoutingPoolState {
    /// Ensure every target in `pool` has a health entry.
    pub fn ensure_targets(&mut self, pool: &[RoutingTarget], config: CircuitBreakerConfig) {
        for t in pool {
            self.health
                .entry(t.id.clone())
                .or_insert_with(|| TargetHealth {
                    breaker: CircuitBreaker::new(config),
                    in_flight: 0,
                });
        }
    }

    fn health_for<'a>(&'a self, id: &str) -> Option<&'a TargetHealth> {
        self.health.get(id)
    }
}

/// Pure selection helpers — deterministic given inputs and mutable state.
#[derive(Debug, Clone, Copy, Default)]
pub struct RoutingSelector;

impl RoutingSelector {
    /// Select a pool index using `strategy`. Returns `None` if no healthy target exists.
    pub fn select(
        strategy: RoutingStrategy,
        pool: &[RoutingTarget],
        state: &mut RoutingPoolState,
        now_secs: u64,
        p2c_seed: u64,
    ) -> Option<usize> {
        let healthy: Vec<usize> = pool
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                state
                    .health_for(&t.id)
                    .map(|h| h.breaker.allow_request(now_secs))
                    .unwrap_or(true)
            })
            .map(|(i, _)| i)
            .collect();

        if healthy.is_empty() {
            return None;
        }

        let pick = match strategy {
            RoutingStrategy::RoundRobin => {
                let cursor = state.round_robin_cursor % healthy.len();
                state.round_robin_cursor = state.round_robin_cursor.wrapping_add(1);
                healthy[cursor]
            },
            RoutingStrategy::Weighted => {
                let weights: Vec<u32> = healthy.iter().map(|&i| pool[i].weight.max(1)).collect();
                let total: u32 = weights.iter().sum();
                let mut pick_weight = (state.selection_counter % total as u64) as u32;
                state.selection_counter = state.selection_counter.wrapping_add(1);
                let mut chosen = healthy[0];
                for (idx, &w) in weights.iter().enumerate() {
                    if pick_weight < w {
                        chosen = healthy[idx];
                        break;
                    }
                    pick_weight -= w;
                }
                chosen
            },
            RoutingStrategy::LeastUsed => {
                let mut best = healthy[0];
                let mut best_load = u64::MAX;
                for &i in &healthy {
                    let load = state
                        .health_for(&pool[i].id)
                        .map(|h| h.in_flight)
                        .unwrap_or(0);
                    if load < best_load {
                        best_load = load;
                        best = i;
                    }
                }
                best
            },
            RoutingStrategy::PowerOfTwoChoices => {
                let n = healthy.len();
                if n == 1 {
                    healthy[0]
                } else {
                    let seed = state.selection_counter.wrapping_add(p2c_seed);
                    state.selection_counter = state.selection_counter.wrapping_add(1);
                    let i = (seed as usize) % n;
                    let j = (seed.wrapping_mul(31).wrapping_add(17) as usize) % n;
                    let (a, b) = if i == j {
                        (healthy[i], healthy[(i + 1) % n])
                    } else {
                        (healthy[i], healthy[j])
                    };
                    let load_a = state
                        .health_for(&pool[a].id)
                        .map(|h| h.in_flight)
                        .unwrap_or(0);
                    let load_b = state
                        .health_for(&pool[b].id)
                        .map(|h| h.in_flight)
                        .unwrap_or(0);
                    if load_a <= load_b {
                        a
                    } else {
                        b
                    }
                }
            },
        };

        Some(pick)
    }

    /// Walk the fallback chain by rank; within each rank, weighted-select among healthy entries.
    pub fn select_fallback<'a>(
        chain: &'a [FallbackEntry],
        health: &HashMap<String, TargetHealth>,
        counter: &mut u64,
        now_secs: u64,
    ) -> Option<&'a RoutingTarget> {
        if chain.is_empty() {
            return None;
        }
        let min_rank = chain.iter().map(|e| e.rank).min().unwrap();
        let max_rank = chain.iter().map(|e| e.rank).max().unwrap();

        for rank in min_rank..=max_rank {
            let tier: Vec<&FallbackEntry> = chain
                .iter()
                .filter(|e| e.rank == rank)
                .filter(|e| {
                    health
                        .get(&e.target.id)
                        .map(|h| h.breaker.allow_request(now_secs))
                        .unwrap_or(true)
                })
                .collect();
            if tier.is_empty() {
                continue;
            }
            if tier.len() == 1 {
                return Some(&tier[0].target);
            }
            let weights: Vec<u32> = tier.iter().map(|e| e.weight.max(1)).collect();
            let total: u32 = weights.iter().sum();
            let mut pick = (*counter % total as u64) as u32;
            *counter = counter.wrapping_add(1);
            for (entry, w) in tier.iter().zip(weights.iter()) {
                if pick < *w {
                    return Some(&entry.target);
                }
                pick -= w;
            }
            return Some(&tier[0].target);
        }
        None
    }
}

/// A routing decision enriched with the chosen target id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupersetRoutingDecision {
    /// Chosen target id (for outcome recording).
    pub target_id: String,
    /// Engine + model decision for downstream engines.
    pub decision: RoutingDecision,
}

impl SupersetRoutingDecision {
    fn from_target(target: &RoutingTarget, reason: impl Into<String>) -> Self {
        SupersetRoutingDecision {
            target_id: target.id.clone(),
            decision: RoutingDecision {
                engine: target.engine.clone(),
                model: target.model.clone(),
                reason: Some(reason.into()),
            },
        }
    }
}

/// Bundled routing superset: pool load-balancing with fallback chain and health tracking.
#[derive(Debug, Clone)]
pub struct RoutingSuperset {
    pool: Vec<RoutingTarget>,
    fallback: Vec<FallbackEntry>,
    strategy: RoutingStrategy,
    breaker_config: CircuitBreakerConfig,
    state: RoutingPoolState,
}

impl RoutingSuperset {
    /// Create a new superset router.
    pub fn new(
        pool: Vec<RoutingTarget>,
        fallback: Vec<FallbackEntry>,
        strategy: RoutingStrategy,
        breaker_config: CircuitBreakerConfig,
    ) -> Self {
        let mut state = RoutingPoolState::default();
        state.ensure_targets(&pool, breaker_config);
        for entry in &fallback {
            state
                .health
                .entry(entry.target.id.clone())
                .or_insert_with(|| TargetHealth {
                    breaker: CircuitBreaker::new(breaker_config),
                    in_flight: 0,
                });
        }
        RoutingSuperset {
            pool,
            fallback,
            strategy,
            breaker_config,
            state,
        }
    }

    /// Select a target and return a routing decision. Increments in-flight for the chosen target.
    pub fn route(&mut self, now_secs: u64) -> Result<SupersetRoutingDecision> {
        if let Some(idx) =
            RoutingSelector::select(self.strategy, &self.pool, &mut self.state, now_secs, 0)
        {
            let target = &self.pool[idx];
            if let Some(h) = self.state.health.get_mut(&target.id) {
                h.in_flight += 1;
            }
            return Ok(SupersetRoutingDecision::from_target(
                target,
                format!("routing-superset:{strategy:?}", strategy = self.strategy),
            ));
        }

        if let Some(target) = RoutingSelector::select_fallback(
            &self.fallback,
            &self.state.health,
            &mut self.state.selection_counter,
            now_secs,
        ) {
            if let Some(h) = self.state.health.get_mut(&target.id) {
                h.in_flight += 1;
            }
            return Ok(SupersetRoutingDecision::from_target(
                target,
                "routing-superset:fallback",
            ));
        }

        Err(SubstrateError::Routing(
            "no healthy routing target available".to_string(),
        ))
    }

    /// Record success or failure for a previously routed target.
    pub fn record_outcome(&mut self, target_id: &str, success: bool, now_secs: u64) {
        if let Some(h) = self.state.health.get_mut(target_id) {
            h.in_flight = h.in_flight.saturating_sub(1);
            if success {
                h.breaker.record_success(now_secs);
            } else {
                h.breaker.record_failure(now_secs);
            }
        }
    }

    /// Current load-balancing strategy.
    pub fn strategy(&self) -> RoutingStrategy {
        self.strategy
    }

    /// Primary target pool.
    pub fn pool(&self) -> &[RoutingTarget] {
        &self.pool
    }

    /// Fallback chain entries.
    pub fn fallback(&self) -> &[FallbackEntry] {
        &self.fallback
    }

    /// Breaker configuration applied to new health entries.
    pub fn breaker_config(&self) -> CircuitBreakerConfig {
        self.breaker_config
    }
}
