// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C07 L70 — reproducible local dev seed fixture + verify gate (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

use sharecli::config::Config;
use sharecli::config_validator::validate_config;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C07 L70 — committed dev seed parses and passes validate_config.
#[test]
fn c07_l70_dev_seed_fixture_valid() {
    let fixture = repo_root().join("fixtures/dev-seed/config.toml");
    assert!(fixture.is_file(), "fixtures/dev-seed/config.toml must exist for L70 seed-data");

    let contents = fs::read_to_string(&fixture).expect("read dev seed fixture");
    let cfg: Config = toml::from_str(&contents).expect("dev seed fixture must parse as Config");

    let errs = validate_config(&cfg);
    assert!(errs.is_empty(), "dev seed fixture must pass validate_config: {errs:?}");
    assert!(
        cfg.project_limits.max_processes >= 1,
        "seed fixture must set a positive max_processes"
    );
}

/// FR-003 / C07 L70 — `just dev` must invoke seed verify script.
#[test]
fn c07_l70_just_dev_wires_seed_verify() {
    let justfile = fs::read_to_string(repo_root().join("justfile")).expect("read justfile");
    assert!(
        justfile.contains("scripts/dev/verify_seed.sh"),
        "just dev must run scripts/dev/verify_seed.sh"
    );
}

/// FR-003 / C07 L70 — dev seed contract documented.
#[test]
fn c07_l70_dev_seed_doc_present() {
    let doc =
        fs::read_to_string(repo_root().join("docs/ops/dev-seed.md")).expect("read dev-seed.md");
    assert!(
        doc.contains("fixtures/dev-seed/config.toml"),
        "docs/ops/dev-seed.md must reference the seed fixture"
    );
    assert!(doc.contains("just dev"), "docs/ops/dev-seed.md must document just dev entrypoint");
}
