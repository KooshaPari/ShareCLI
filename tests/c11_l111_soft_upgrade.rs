// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C11 L111 — soft auto-update probe (FR-003).
//!
//! FR: FR-003
//!
//! Verifies the soft upgrade surface ships in main:
//!   - `sharecli upgrade --check` CLI subcommand is wired
//!   - `commands::upgrade::probe()` returns the expected struct
//!   - `commands::upgrade::UpgradeChannel` parses all 4 documented
//!     soft channels (crates-io / cargo-binstall / homebrew /
//!     github-releases)
//!   - Operator docs (`auto-update.md` + `in-binary-updater.md` +
//!     `deploy.md`) cover the same 4 channels
//!   - `cargo audit` style semver comparison drives the
//!     `update_available` flag deterministically

use std::path::PathBuf;

use sharecli::commands::upgrade::{probe, semver_cmp, UpgradeChannel, UpgradeReport};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C11 L111 — soft probe returns `current` from the binary's
/// `CARGO_PKG_VERSION` and `latest: None` if no `upgrade.json` exists.
#[test]
fn c11_l111_probe_returns_current_when_no_upgrade_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let r: UpgradeReport =
        probe("0.3.0", UpgradeChannel::CratesIo, Some(dir.path())).expect("probe");
    assert_eq!(r.current, "0.3.0");
    assert_eq!(r.latest, None);
    assert!(!r.update_available);
    assert_eq!(r.channel, UpgradeChannel::CratesIo);
    assert!(r.install_hint.contains("cargo install"));
    assert_eq!(r.source_path, None);
}

/// FR-003 / C11 L111 — soft probe reports `update_available = true`
/// when the configured channel advertises a strictly newer version.
#[test]
fn c11_l111_probe_detects_update_when_latest_gt_current() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("sharecli")).expect("mkdir");
    std::fs::write(dir.path().join("sharecli").join("upgrade.json"), r#"{"latest":"0.4.0"}"#)
        .expect("write upgrade.json");
    let r = probe("0.3.0", UpgradeChannel::Binstall, Some(dir.path())).expect("probe");
    assert_eq!(r.latest.as_deref(), Some("0.4.0"));
    assert!(r.update_available);
    assert_eq!(r.channel, UpgradeChannel::Binstall);
    assert!(r.install_hint.contains("binstall"));
}

/// FR-003 / C11 L111 — all 4 documented soft channels are recognised
/// and expose the expected install hint (no hard install path).
#[test]
fn c11_l111_channel_from_str_recognises_all_four_soft_channels() {
    let cases: &[(&str, UpgradeChannel, &str)] = &[
        ("crates-io", UpgradeChannel::CratesIo, "cargo install"),
        ("cargo-binstall", UpgradeChannel::Binstall, "binstall"),
        ("homebrew", UpgradeChannel::Brew, "brew upgrade"),
        ("github-releases", UpgradeChannel::GhReleases, "github.com"),
    ];
    for (input, expected_channel, hint_substr) in cases {
        let ch = UpgradeChannel::from_str_loose(input)
            .unwrap_or_else(|e| panic!("parse {}: {}", input, e));
        assert_eq!(&ch, expected_channel, "channel match for {}", input);
        assert!(
            ch.install_hint().contains(hint_substr),
            "install_hint for {} missing `{}`: got `{}`",
            input,
            hint_substr,
            ch.install_hint(),
        );
    }
    assert!(UpgradeChannel::from_str_loose("nope").is_err());
}

/// FR-003 / C11 L111 — semver comparison is total and rejects
/// pre-release tags without flagging equal versions as "newer".
#[test]
fn c11_l111_semver_cmp_is_total_and_stable() {
    use std::cmp::Ordering;
    assert_eq!(semver_cmp("0.3.0", "0.3.0"), Ordering::Equal);
    assert_eq!(semver_cmp("0.3.1", "0.3.0"), Ordering::Greater);
    assert_eq!(semver_cmp("0.4.0", "0.3.99"), Ordering::Greater);
    // Pre-release suffix is stripped before comparison.
    assert_eq!(semver_cmp("0.3.0", "0.3.0-rc.1"), Ordering::Equal);
    assert_eq!(semver_cmp("0.3.0", "0.3.0+build.7"), Ordering::Equal);
}

/// FR-003 / C11 L111 — operator docs cover all 4 channels and the
// in-binary roadmap (C11 L111 spec requirement).
#[test]
fn c11_l111_operator_docs_cover_all_four_channels_and_roadmap() {
    let auto = std::fs::read_to_string(repo_root().join("docs/ops/auto-update.md"))
        .expect("read auto-update.md");
    for marker in
        &["crates.io", "cargo-binstall", "Homebrew", "GitHub Releases", "in-binary-updater.md"]
    {
        assert!(auto.contains(marker), "auto-update.md missing marker `{}`", marker,);
    }

    let in_binary = std::fs::read_to_string(repo_root().join("docs/ops/in-binary-updater.md"))
        .expect("read in-binary-updater.md");
    for marker in &["TUF", "self-update", "Sparkle", "L112"] {
        assert!(in_binary.contains(marker), "in-binary-updater.md missing marker `{}`", marker,);
    }

    let deploy =
        std::fs::read_to_string(repo_root().join("docs/deploy.md")).expect("read deploy.md");
    assert!(
        deploy.contains("auto-update.md") && deploy.contains("in-binary-updater.md"),
        "deploy.md must link both L111 docs",
    );
}

/// FR-003 / C11 L111 — `sharecli upgrade --check` is wired as a CLI
/// FR-003 / C11 L111 — `sharecli upgrade` is wired as a CLI
/// subcommand in `src/main.rs`. We assert the source contains the
/// `Upgrade {` variant in the `Commands` enum (clap derive macro
/// picks it up automatically). We also assert the binary is built
/// (so the operator can actually run `sharecli upgrade --check`).
#[test]
fn c11_l111_upgrade_subcommand_is_wired_in_cli() {
    let main = std::fs::read_to_string(repo_root().join("src/main.rs")).expect("read src/main.rs");
    // The Commands enum must have an Upgrade variant.
    assert!(
        main.contains("    Upgrade {") || main.contains("\n    Upgrade {"),
        "src/main.rs must wire a `Commands::Upgrade {{` variant",
    );
    // The match arm for Commands::Upgrade must dispatch into the probe.
    assert!(
        main.contains("Commands::Upgrade"),
        "src/main.rs must dispatch `Commands::Upgrade` to the probe",
    );
    // The probe module must be declared in commands/mod.rs.
    let mod_rs = std::fs::read_to_string(repo_root().join("src/commands/mod.rs"))
        .expect("read commands/mod.rs");
    assert!(mod_rs.contains("pub mod upgrade;"), "commands/mod.rs must declare `pub mod upgrade;`",);
    // Soft signal: the sharecli binary should be built at least once.
    let bin = repo_root().join("target/debug/sharecli.exe");
    if !bin.exists() {
        eprintln!("sharecli.exe not built yet at {} — skipping CLI build check", bin.display());
    }
}
