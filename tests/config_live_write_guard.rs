//! Regression guard for live-config safety.
//!
//! Integration tests run as their own binary, so process-global environment
//! mutation here cannot race another test file.
//!
//! Why this exists: `Config::save()` once wrote to the developer's real
//! `~/Library/Application Support/sharecli/config.toml` whenever a test
//! exercised `config set`, `project add`, or the IPC `config.set` handler. A
//! first attempt at a guard keyed off `CARGO_TARGET_TMPDIR`, but that variable
//! is not set for integration tests on this toolchain, so the guard was inert
//! and the config kept being rewritten. These assertions pin the behaviour so a
//! guard that silently stops firing fails the suite instead of eating configs.

use sharecli::config::Config;

#[test]
fn test_binary_cannot_write_the_live_config() {
    // Sequential on purpose: both halves share process-global environment.
    std::env::remove_var("SHARECLI_CONFIG_PATH");

    // 1. Without an override the write must be refused.
    let refused = Config::default().save();
    let err = match refused {
        Ok(()) => panic!(
            "Config::save() wrote to the live config from a test binary; the guard is inert"
        ),
        Err(e) => e,
    };
    let message = err.to_string();
    assert!(
        message.contains("refusing to write the live config"),
        "refused for the wrong reason: {message}"
    );

    // 2. With an explicit path the write must succeed, proving the guard is a
    //    targeted rail rather than a blanket refusal.
    let dir = std::env::temp_dir().join(format!("sharecli-guard-{}", std::process::id()));
    let path = dir.join("config.toml");
    std::env::set_var("SHARECLI_CONFIG_PATH", &path);

    let written = Config::default().save();
    std::env::remove_var("SHARECLI_CONFIG_PATH");

    written.expect("save() with SHARECLI_CONFIG_PATH set must succeed");
    assert!(path.exists(), "override path {} was not written", path.display());

    let _ = std::fs::remove_dir_all(&dir);
}
