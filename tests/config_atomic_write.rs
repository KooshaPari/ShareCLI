//! FR-002 — Atomic config write and `.bak` recovery
//! FR: FR-002
//!
//! Phase 1 task 1.4 (lane 8 MEDIUM-HIGH): `Config::save`/`init` used
//! `std::fs::write`, which is open / truncate / write-in-place. A crash or kill
//! mid-write leaves a half-written file, and there was no backup to fall back
//! to, so the next `Config::load()` either failed to parse or — when the file
//! was missing entirely — silently handed back `Config::default()`, losing every
//! registered project without an error.
//!
//! Contract under test:
//! - `save` writes through a temporary file in the *same directory* (so the
//!   final `rename` is atomic on the same file system), keeps the previous
//!   generation as `.bak`, and leaves no temporary file behind.
//! - `load` prefers the primary, and recovers from `.bak` when the primary is
//!   missing or unparsable.
//!
//! Crash simulation: an in-process kill cannot be staged portably, so the test
//! constructs exactly the state a crash under the old write-in-place scheme
//! leaves behind — primary truncated or gone, `.bak` holding the last good
//! generation — and asserts recovery.

use std::fs;
use std::path::{Path, PathBuf};

use sharecli::config::Config;

/// The backup path this suite expects `save` to maintain.
fn backup_path(config_path: &Path) -> PathBuf {
    let mut s = config_path.as_os_str().to_owned();
    s.push(".bak");
    PathBuf::from(s)
}

/// Point the process at an isolated config file and return a guard that
/// restores the environment when dropped, so a failing assertion cannot leak
/// the override into sibling tests.
struct EnvGuard(Option<std::ffi::OsString>);

impl EnvGuard {
    fn set(path: &Path) -> Self {
        let prev = std::env::var_os("SHARECLI_CONFIG_PATH");
        // SAFETY: serialised by `#[serial_test::serial]`; scoped to this test.
        unsafe { std::env::set_var("SHARECLI_CONFIG_PATH", path) };
        Self(prev)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: serialised by `#[serial_test::serial]`; scoped to this test.
        unsafe {
            match self.0.take() {
                Some(v) => std::env::set_var("SHARECLI_CONFIG_PATH", v),
                None => std::env::remove_var("SHARECLI_CONFIG_PATH"),
            }
        }
    }
}

fn read_config(path: &Path) -> Config {
    let raw = fs::read_to_string(path).expect("read config file");
    toml::from_str(&raw).expect("parse config file")
}

/// AC — `save` must preserve the previous generation as `.bak`.
#[test]
#[serial_test::serial]
fn save_keeps_the_previous_generation_as_a_backup() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    let _env = EnvGuard::set(&path);

    let mut cfg = Config::default();
    cfg.projects.insert("first".into(), "/a".into());
    cfg.save().expect("save first generation");

    cfg.projects.insert("second".into(), "/b".into());
    cfg.save().expect("save second generation");

    let bak = backup_path(&path);
    assert!(bak.exists(), "save must keep a .bak of the previous config");

    let previous = read_config(&bak);
    assert_eq!(previous.projects.get("first").map(String::as_str), Some("/a"));
    assert!(
        !previous.projects.contains_key("second"),
        ".bak must hold the generation *before* this save, not this one"
    );
}

/// AC — after a crash the last good generation is still loadable when the
/// primary file is gone.
#[test]
#[serial_test::serial]
fn load_recovers_from_backup_when_the_primary_is_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    let bak = backup_path(&path);

    let mut good = Config::default();
    good.projects.insert("rescued".into(), "/rescued".into());
    fs::write(&bak, toml::to_string_pretty(&good).expect("serialize backup"))
        .expect("write backup");
    // Primary deliberately absent — the state a crash mid-write leaves behind.

    let _env = EnvGuard::set(&path);
    let loaded = Config::load().expect("load must succeed against .bak");

    assert_eq!(
        loaded.projects.get("rescued").map(String::as_str),
        Some("/rescued"),
        "a missing primary must not silently degrade to Config::default()"
    );
}

/// AC — a truncated primary (a half-written file) is recovered from `.bak`
/// instead of failing the load.
#[test]
#[serial_test::serial]
fn load_recovers_from_backup_when_the_primary_is_truncated() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    let bak = backup_path(&path);

    let mut good = Config::default();
    good.projects.insert("rescued".into(), "/rescued".into());
    fs::write(&bak, toml::to_string_pretty(&good).expect("serialize backup"))
        .expect("write backup");
    // Half of a valid document: exactly what write-in-place leaves after a kill.
    fs::write(&path, "[projects\nrescued = \"/res").expect("write truncated primary");

    let _env = EnvGuard::set(&path);
    let loaded = Config::load().expect("load must succeed against .bak");

    assert_eq!(loaded.projects.get("rescued").map(String::as_str), Some("/rescued"));
}

/// AC — the temporary file used for the atomic swap must not survive the save.
#[test]
#[serial_test::serial]
fn save_leaves_no_temporary_files_behind() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    let _env = EnvGuard::set(&path);

    Config::default().save().expect("first save");
    Config::default().save().expect("second save");

    let leftovers: Vec<String> = fs::read_dir(tmp.path())
        .expect("read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "config.toml" && name != "config.toml.bak")
        .collect();

    assert!(
        leftovers.is_empty(),
        "no stray temporary files may be left next to the config, found {leftovers:?}"
    );
}
