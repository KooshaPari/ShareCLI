use substrate_app::tiered_dispatch::{
    dispatch_with_reroute, select_auto_tier, TieredDispatchOutcome,
};
use substrate_core::{Result, SubstrateError, Tier};

#[test]
fn auto_tier_routes_short_prompt_to_worker() {
    assert_eq!(select_auto_tier("echo hi"), Tier::Worker);
}

#[test]
fn auto_tier_routes_complex_prompt_to_heavy() {
    assert_eq!(
        select_auto_tier("Analyze the architecture and root cause the failure"),
        Tier::Heavy
    );
}

#[test]
fn reroute_downgrades_once_until_dispatch_succeeds() {
    let mut attempts = Vec::new();

    let outcome = dispatch_with_reroute(Tier::Heavy, |tier| {
        attempts.push(tier);
        match tier {
            Tier::Heavy => Err(SubstrateError::Engine("heavy failed".into())),
            Tier::Main => Ok("main output".to_string()),
            Tier::Worker => Ok("worker output".to_string()),
        }
    })
    .unwrap();

    assert_eq!(attempts, vec![Tier::Heavy, Tier::Main]);
    assert_eq!(
        outcome,
        TieredDispatchOutcome {
            succeeded_tier: Tier::Main,
            attempted_tiers: vec![Tier::Heavy, Tier::Main],
            output: "main output".to_string(),
        }
    );
}

#[test]
fn reroute_treats_empty_output_as_failure() {
    let mut attempts = Vec::new();

    let outcome = dispatch_with_reroute(Tier::Main, |tier| {
        attempts.push(tier);
        match tier {
            Tier::Main => Ok("   ".to_string()),
            Tier::Worker => Ok("worker output".to_string()),
            Tier::Heavy => Ok("heavy output".to_string()),
        }
    })
    .unwrap();

    assert_eq!(attempts, vec![Tier::Main, Tier::Worker]);
    assert_eq!(outcome.succeeded_tier, Tier::Worker);
    assert_eq!(outcome.output, "worker output");
}

#[test]
fn reroute_returns_last_failure_when_all_tiers_fail() {
    let result: Result<TieredDispatchOutcome> = dispatch_with_reroute(Tier::Main, |tier| {
        Err(SubstrateError::Engine(format!("{tier} failed")))
    });

    let error = result.unwrap_err().to_string();
    assert!(error.contains("worker failed"));
}
