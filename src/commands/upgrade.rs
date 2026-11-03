// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli upgrade --check` — soft upgrade probe (FR-003 / C11 L111).
//!
//! **Soft:** does NOT actually download, install, or replace the running
//! binary. Only reports the current version and the configured channel's
//! advertised latest version (read from a deterministic local file that
//! the operator can refresh via `cargo install --force sharecli` /
//! `cargo binstall sharecli` / `brew upgrade sharecli` / GH Releases +
//! `.sha256`).
//!
//! No network egress in this implementation. Real signing + delta
//! updates are deferred to L112 secrets landing.
//!
//! Channels (C11 L111):
//!   - CratesIo   — `cargo install sharecli --force`
//!   - Binstall   — `cargo binstall sharecli`
//!   - Brew       — `brew upgrade sharecli` / `brew install --HEAD sharecli`
//!   - GhReleases — `curl -L https://github.com/KooshaPari/sharecli/releases/latest`

// All public functions in this module are dispatched via the CLI subcommand
// enum, so the compiler sees them as unused. Suppress the warning.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

/// Update channel advertised by the soft probe (no real install).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpgradeChannel {
    /// `cargo install sharecli --force` — crates.io as source of truth.
    CratesIo,
    /// `cargo binstall sharecli` — prebuilt binary installer.
    Binstall,
    /// `brew upgrade sharecli` / `brew install --HEAD sharecli`.
    Brew,
    /// GitHub Releases tarball + `.sha256` (UNSIGNED until L112).
    GhReleases,
}

impl UpgradeChannel {
    /// Operator-facing install hint. No execution; documentation only.
    pub fn install_hint(self) -> &'static str {
        match self {
            UpgradeChannel::CratesIo => "cargo install sharecli --force",
            UpgradeChannel::Binstall => "cargo binstall sharecli",
            UpgradeChannel::Brew => "brew upgrade sharecli",
            UpgradeChannel::GhReleases => {
                "curl -fsSL https://github.com/KooshaPari/sharecli/releases/latest"
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            UpgradeChannel::CratesIo => "crates-io",
            UpgradeChannel::Binstall => "cargo-binstall",
            UpgradeChannel::Brew => "homebrew",
            UpgradeChannel::GhReleases => "github-releases",
        }
    }

    pub fn from_str_loose(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "crates-io" | "crates" | "crates.io" => Ok(Self::CratesIo),
            "cargo-binstall" | "binstall" => Ok(Self::Binstall),
            "homebrew" | "brew" => Ok(Self::Brew),
            "github-releases" | "gh-releases" | "gh" => Ok(Self::GhReleases),
            other => Err(anyhow!("unknown upgrade channel '{}'", other)),
        }
    }
}

/// Reported state of an `sharecli upgrade --check` probe.
///
/// `latest` is **advertised**, not** downloaded; it may be a stale or
/// operator-supplied value from `$XDG_CONFIG_HOME/sharecli/upgrade.json`
/// or the default `~/.config/sharecli/upgrade.json`. No network.
#[derive(Debug, Clone, Serialize)]
pub struct UpgradeReport {
    /// Current binary version (from `CARGO_PKG_VERSION`).
    pub current: String,
    /// Latest version advertised by the configured channel (local file).
    pub latest: Option<String>,
    /// Selected channel.
    pub channel: UpgradeChannel,
    /// True if `latest > current` using semver-major-minor-patch ordering.
    pub update_available: bool,
    /// Operator-visible install command (no execution).
    pub install_hint: &'static str,
    /// Path to the file the probe read the `latest` value from.
    pub source_path: Option<PathBuf>,
}

/// Compare two semver `MAJOR.MINOR.PATCH` strings.
///
/// Returns `Ordering::Greater` if `a > b`, `Less` if `a < b`, `Equal` if
/// equal. **Pre-release suffixes (`-alpha.1`, `+build`) are stripped**
/// before comparison to keep the soft probe deterministic.
pub fn semver_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let pa = parse_semver_tuple(a);
    let pb = parse_semver_tuple(b);
    match pa.cmp(&pb) {
        Ordering::Equal => Ordering::Equal,
        o => o,
    }
}

fn parse_semver_tuple(v: &str) -> (u32, u32, u32) {
    let mut parts = v.trim().split(|c: char| c == '-' || c == '+');
    let head = parts.next().unwrap_or("");
    let mut nums = head.split('.');
    let major = nums.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let minor = nums.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let patch = nums.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// Soft probe — does NOT touch the network. Reads `latest` from
/// `<config_dir>/sharecli/upgrade.json` if present, else returns
/// `latest = None`.
///
/// Caller supplies:
///   - `current_version`: typically `env!("CARGO_PKG_VERSION")`.
///   - `channel`:         one of [`UpgradeChannel`].
///   - `config_dir`:      optional override (tests pass a `TempDir`).
pub fn probe(
    current_version: &str,
    channel: UpgradeChannel,
    config_dir: Option<&Path>,
) -> Result<UpgradeReport> {
    let path = upgrade_json_path(config_dir);
    let (latest, source_path) = match read_latest_from(&path) {
        Ok(v) => (Some(v), Some(path)),
        Err(_) => (None, None),
    };

    let update_available = match &latest {
        Some(v) => semver_cmp(v, current_version).is_gt(),
        None => false,
    };

    Ok(UpgradeReport {
        current: current_version.to_string(),
        latest,
        channel,
        update_available,
        install_hint: channel.install_hint(),
        source_path,
    })
}

fn upgrade_json_path(config_dir: Option<&Path>) -> PathBuf {
    let base = match config_dir {
        Some(p) => p.to_path_buf(),
        None => default_config_dir(),
    };
    base.join("sharecli").join("upgrade.json")
}

fn default_config_dir() -> PathBuf {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(x);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config");
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(profile).join(".config");
    }
    PathBuf::from(".config")
}

fn read_latest_from(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).with_context(|| format!("parse {} as JSON", path.display()))?;
    let latest = parsed
        .get("latest")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing `latest` string in {}", path.display()))?;
    if latest.is_empty() {
        bail!("empty `latest` in {}", path.display());
    }
    Ok(latest.to_string())
}

/// CLI handler for `sharecli upgrade --check [--channel <channel>]`.
pub fn check(channel_name: Option<&str>) -> Result<()> {
    let channel = match channel_name {
        Some(s) => UpgradeChannel::from_str_loose(s)?,
        None => UpgradeChannel::CratesIo,
    };
    let current = env!("CARGO_PKG_VERSION");
    let report = probe(current, channel, None)?;
    let json = serde_json::to_string_pretty(&report).context("serialize upgrade report")?;
    println!("{}", json);
    if report.update_available {
        std::process::exit(0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_cmp_orders_correctly() {
        use std::cmp::Ordering;
        assert_eq!(semver_cmp("1.2.3", "1.2.3"), Ordering::Equal);
        assert_eq!(semver_cmp("1.2.4", "1.2.3"), Ordering::Greater);
        assert_eq!(semver_cmp("1.2.3", "1.2.4"), Ordering::Less);
        assert_eq!(semver_cmp("2.0.0", "1.99.99"), Ordering::Greater);
        assert_eq!(semver_cmp("0.3.0", "0.3.0-alpha.1"), Ordering::Equal);
    }

    #[test]
    fn probe_returns_none_latest_when_file_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let r = probe("0.3.0", UpgradeChannel::CratesIo, Some(dir.path())).expect("probe");
        assert_eq!(r.current, "0.3.0");
        assert_eq!(r.latest, None);
        assert!(!r.update_available);
    }

    #[test]
    fn probe_marks_update_available_when_latest_gt_current() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("sharecli")).expect("mkdir");
        std::fs::write(dir.path().join("sharecli").join("upgrade.json"), r#"{"latest":"0.4.0"}"#)
            .expect("write upgrade.json");
        let r = probe("0.3.0", UpgradeChannel::Binstall, Some(dir.path())).expect("probe");
        assert_eq!(r.latest.as_deref(), Some("0.4.0"));
        assert!(r.update_available);
    }

    #[test]
    fn channel_install_hint_per_channel() {
        assert!(UpgradeChannel::CratesIo.install_hint().contains("cargo install"));
        assert!(UpgradeChannel::Binstall.install_hint().contains("binstall"));
        assert!(UpgradeChannel::Brew.install_hint().contains("brew upgrade"));
        assert!(UpgradeChannel::GhReleases.install_hint().contains("github.com"));
    }
}
