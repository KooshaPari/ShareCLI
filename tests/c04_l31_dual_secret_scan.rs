//! C04 L31 — dual secret scanners (gitleaks + trufflehog) + pre-commit hook (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C04 L31 — `security.yml` runs gitleaks and trufflehog on push/PR.
#[test]
fn fr003_security_yml_dual_secret_scanners() {
    let security = fs::read_to_string(repo_root().join(".github/workflows/security.yml"))
        .expect("read security.yml");

    // gitleaks runs as a pinned binary: the gitleaks-action `args` input is
    // ignored by the action, so `--config gitleaks.toml` never applied and the
    // lane scanned with the built-in rule set. Assert the full command contract
    // of the direct invocation.
    let run_gitleaks = security
        .split_once("name: Run Gitleaks")
        .map(|(_, rest)| rest.split_once("- name:").map(|(block, _)| block).unwrap_or(rest))
        .expect("security.yml must define a Run Gitleaks step");
    assert!(
        run_gitleaks.contains("GITLEAKS_VERSION: 8.24.3"),
        "Run Gitleaks must pin gitleaks 8.24.3"
    );
    assert!(run_gitleaks.contains("GITLEAKS_SHA256:"), "Run Gitleaks must pin a binary checksum");
    assert!(
        run_gitleaks.contains("sha256sum -c -"),
        "Run Gitleaks must verify the downloaded binary checksum"
    );
    assert!(run_gitleaks.contains("tar -xzf"), "Run Gitleaks must extract the archive");
    assert!(
        run_gitleaks.contains("--proto '=https'"),
        "Run Gitleaks must enforce HTTPS-only downloads"
    );
    for flag in ["--redact", "--verbose", "--exit-code=2", "--config .gitleaks.toml"] {
        assert!(run_gitleaks.contains(flag), "Run Gitleaks must pass {flag}");
    }
    assert!(
        !run_gitleaks.contains("gitleaks/gitleaks-action@"),
        "Run Gitleaks must not use gitleaks-action (its args input is ignored)"
    );
    assert!(
        security.contains("fetch-depth: 0"),
        "security.yml must fetch full history so older commits are scanned"
    );
    assert!(
        security.contains("trufflesecurity/trufflehog@"),
        "security.yml must run trufflehog action"
    );
    assert!(
        security.contains("--only-verified"),
        "security.yml trufflehog must use --only-verified"
    );
}

/// FR-003 / C04 L31 — committed pre-commit config wires gitleaks + trufflehog.
#[test]
fn fr003_pre_commit_secret_hooks_present() {
    let pre_commit =
        fs::read_to_string(repo_root().join(".pre-commit-config.yaml")).expect("pre-commit config");

    assert!(pre_commit.contains("gitleaks"), "pre-commit must include gitleaks hook");
    assert!(pre_commit.contains("trufflehog"), "pre-commit must include trufflehog hook");
    assert!(pre_commit.contains(".gitleaks.toml"), "gitleaks hook must reference .gitleaks.toml");
}

/// FR-003 / C04 L31 — trufflehog exclusions + local scan script exist.
#[test]
fn fr003_trufflehog_config_and_local_script() {
    let trufflehog =
        fs::read_to_string(repo_root().join(".trufflehog.yml")).expect(".trufflehog.yml");

    assert!(trufflehog.contains("target/**"), ".trufflehog.yml must exclude target/");

    let script =
        fs::read_to_string(repo_root().join("scripts/ci/secret_scan.sh")).expect("secret_scan.sh");
    assert!(script.contains("gitleaks"), "secret_scan.sh must run gitleaks");
    assert!(script.contains("trufflehog"), "secret_scan.sh must run trufflehog");
}
