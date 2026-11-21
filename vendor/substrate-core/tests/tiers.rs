use substrate_core::{Tier, HEAVY_MODEL, HEAVY_REASONING_EFFORT, MAIN_MODEL};

#[test]
fn tier_escalates_worker_to_main_to_heavy_to_none() {
    assert_eq!(Tier::Worker.escalate(), Some(Tier::Main));
    assert_eq!(Tier::Main.escalate(), Some(Tier::Heavy));
    assert_eq!(Tier::Heavy.escalate(), None);
}

#[test]
fn tier_downgrades_heavy_to_main_to_worker_to_none() {
    assert_eq!(Tier::Heavy.downgrade(), Some(Tier::Main));
    assert_eq!(Tier::Main.downgrade(), Some(Tier::Worker));
    assert_eq!(Tier::Worker.downgrade(), None);
}

#[test]
fn tier_specs_map_to_model_and_reasoning_effort() {
    let heavy = Tier::Heavy.spec();
    assert_eq!(heavy.model_id, HEAVY_MODEL);
    assert_eq!(heavy.reasoning_effort, HEAVY_REASONING_EFFORT);

    let main = Tier::Main.spec();
    assert_eq!(main.model_id, MAIN_MODEL);
    assert_eq!(main.reasoning_effort, "low");

    let worker = Tier::Worker.spec();
    assert_eq!(worker.model_id, "gpt-5.3-codex-spark");
    assert_eq!(worker.reasoning_effort, "medium");
}

#[test]
fn tier_parses_cli_values() {
    assert_eq!("worker".parse::<Tier>().unwrap(), Tier::Worker);
    assert_eq!("main".parse::<Tier>().unwrap(), Tier::Main);
    assert_eq!("heavy".parse::<Tier>().unwrap(), Tier::Heavy);
    assert!("center".parse::<Tier>().is_err());
}
