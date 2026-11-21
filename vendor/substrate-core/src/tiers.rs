//! Tiered model dispatch definitions.
//!
//! The tier table is pure core data so drivers and adapters can agree on the
//! same model/effort mapping without depending on each other.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Heavy tier model id.
pub const HEAVY_MODEL: &str = "gpt-5.5";
/// Heavy tier reasoning effort.
pub const HEAVY_REASONING_EFFORT: &str = "low";
/// Main tier model id.
pub const MAIN_MODEL: &str = "gpt-5.4-mini";
/// Main tier reasoning effort.
pub const MAIN_REASONING_EFFORT: &str = "low";
/// Worker tier model id.
pub const WORKER_MODEL: &str = "gpt-5.3-codex-spark";
/// Worker tier reasoning effort.
pub const WORKER_REASONING_EFFORT: &str = "medium";

/// Tiered model dispatch level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Most capable and most expensive route.
    Heavy,
    /// Default middle route for normal work.
    Main,
    /// Cheapest worker route for delegated/background work.
    Worker,
}

impl Tier {
    /// Return the next higher tier for retry-on-failure routing.
    pub fn escalate(self) -> Option<Tier> {
        match self {
            Tier::Worker => Some(Tier::Main),
            Tier::Main => Some(Tier::Heavy),
            Tier::Heavy => None,
        }
    }

    /// Return the next lower tier for budget-aware retry routing.
    pub fn downgrade(self) -> Option<Tier> {
        match self {
            Tier::Heavy => Some(Tier::Main),
            Tier::Main => Some(Tier::Worker),
            Tier::Worker => None,
        }
    }

    /// Return the model/effort pair for this tier.
    pub fn spec(self) -> TierSpec {
        match self {
            Tier::Heavy => TierSpec {
                model_id: HEAVY_MODEL,
                reasoning_effort: HEAVY_REASONING_EFFORT,
            },
            Tier::Main => TierSpec {
                model_id: MAIN_MODEL,
                reasoning_effort: MAIN_REASONING_EFFORT,
            },
            Tier::Worker => TierSpec {
                model_id: WORKER_MODEL,
                reasoning_effort: WORKER_REASONING_EFFORT,
            },
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Tier::Heavy => "heavy",
            Tier::Main => "main",
            Tier::Worker => "worker",
        };
        f.write_str(value)
    }
}

impl FromStr for Tier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "heavy" => Ok(Tier::Heavy),
            "main" => Ok(Tier::Main),
            "worker" => Ok(Tier::Worker),
            other => Err(format!("invalid tier {other}: use heavy, main, or worker")),
        }
    }
}

/// Concrete model settings for a [`Tier`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierSpec {
    /// Model identifier passed to the engine.
    pub model_id: &'static str,
    /// Reasoning effort passed as `model_reasoning_effort`.
    pub reasoning_effort: &'static str,
}
