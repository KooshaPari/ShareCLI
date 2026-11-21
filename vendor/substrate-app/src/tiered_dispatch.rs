//! Tiered dispatch with deterministic auto-selection and downgrade-on-failure.

use std::future::Future;

use substrate_core::{
    error::{Result, SubstrateError},
    Tier,
};

/// Successful tiered dispatch result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TieredDispatchOutcome {
    /// Tier that produced non-empty output.
    pub succeeded_tier: Tier,
    /// Tiers attempted in order.
    pub attempted_tiers: Vec<Tier>,
    /// Captured dispatch output.
    pub output: String,
}

/// Select an initial tier from prompt text.
///
/// This first-pass heuristic intentionally stays simple and explainable:
/// short prompts under 500 characters use Worker, prompts containing common
/// complex/synthesis cues use Heavy, and everything else uses Main.
pub fn select_auto_tier(prompt: &str) -> Tier {
    let lower = prompt.to_lowercase();
    let complex_cues = [
        "architecture",
        "architect",
        "design",
        "synthesis",
        "synthesize",
        "analyze",
        "analysis",
        "refactor",
        "migration",
        "security",
        "performance",
        "debug",
        "root cause",
        "root-cause",
    ];

    if complex_cues.iter().any(|cue| lower.contains(cue)) {
        Tier::Heavy
    } else if prompt.chars().count() < 500 {
        Tier::Worker
    } else {
        Tier::Main
    }
}

/// Dispatch starting at `start_tier`, retrying once at the next lower tier on failure.
///
/// Failures include closure errors and successful-but-empty output. The function
/// is pure apart from the supplied closure, making retry order easy to unit test.
pub fn dispatch_with_reroute<F>(start_tier: Tier, mut dispatch: F) -> Result<TieredDispatchOutcome>
where
    F: FnMut(Tier) -> Result<String>,
{
    let mut tier = start_tier;
    let mut attempted_tiers = Vec::new();
    let mut retried = false;

    loop {
        attempted_tiers.push(tier);
        let error = match dispatch(tier) {
            Ok(output) if !output.trim().is_empty() => {
                return Ok(TieredDispatchOutcome {
                    succeeded_tier: tier,
                    attempted_tiers,
                    output,
                });
            },
            Ok(_) => SubstrateError::Engine(format!("{tier} dispatch returned empty output")),
            Err(error) => error,
        };

        match (retried, tier.downgrade()) {
            (false, Some(next)) => {
                retried = true;
                tier = next;
            },
            _ => return Err(error),
        }
    }
}

/// Async variant of [`dispatch_with_reroute`] for subprocess or network adapters.
pub async fn dispatch_with_reroute_async<F, Fut>(
    start_tier: Tier,
    mut dispatch: F,
) -> Result<TieredDispatchOutcome>
where
    F: FnMut(Tier) -> Fut,
    Fut: Future<Output = Result<String>>,
{
    let mut tier = start_tier;
    let mut attempted_tiers = Vec::new();
    let mut retried = false;

    loop {
        attempted_tiers.push(tier);
        let error = match dispatch(tier).await {
            Ok(output) if !output.trim().is_empty() => {
                return Ok(TieredDispatchOutcome {
                    succeeded_tier: tier,
                    attempted_tiers,
                    output,
                });
            },
            Ok(_) => SubstrateError::Engine(format!("{tier} dispatch returned empty output")),
            Err(error) => error,
        };

        match (retried, tier.downgrade()) {
            (false, Some(next)) => {
                retried = true;
                tier = next;
            },
            _ => return Err(error),
        }
    }
}
