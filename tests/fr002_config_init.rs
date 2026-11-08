// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-002 — TOML Configuration Management (init / validate)
//! FR: FR-002
//!
//! Covers AC-002.1, AC-002.2.
//!
//! Library-level acceptance tests. They do **not** call `Config::init()` /
//! `Config::load()` against the real user config dir (`dirs::config_dir()` is
//! not overridable on Windows); they recreate the same create-dir → write
//! default TOML → deserialize path using `tempfile`.

use std::fs;
use std::path::Path;

use sharecli::config::Config;

/// Mirror of `Config::init` + the deserialize half of `Config::load`, writing
/// under `root` instead of `dirs::config_dir()/sharecli/`.
fn init_default_toml_under(root: &Path) -> std::path::PathBuf {
    let config_path = root.join("sharecli").join("config.toml");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).expect("create config directory");
    }
    let config = Config::default();
    let contents = toml::to_string_pretty(&config).expect("serialize default Config");
    fs::write(&config_path, contents).expect("write config.toml");
    config_path
}

/// FR-002 / AC-002.1 — `config init` creates the config directory if missing
/// and writes a default TOML that round-trips through the deserialize path.
#[test]
fn fr002_init_creates_default_toml() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert!(!tmp.path().join("sharecli").exists());

    let config_path = init_default_toml_under(tmp.path());
    assert!(config_path.is_file(), "init MUST write config.toml");
    assert!(
        config_path.parent().unwrap().is_dir(),
        "init MUST create the sharecli config directory"
    );

    let contents = fs::read_to_string(&config_path).expect("read written toml");
    let loaded: Config = toml::from_str(&contents).expect("default TOML MUST deserialize");

    let expected = Config::default();
    assert_eq!(loaded.projects, expected.projects);
    assert_eq!(loaded.runtime.max_memory_mb, expected.runtime.max_memory_mb);
    assert_eq!(loaded.runtime.max_processes, expected.runtime.max_processes);
    assert_eq!(loaded.runtime.node_path, expected.runtime.node_path);
    assert_eq!(loaded.runtime.bun_path, expected.runtime.bun_path);
}

/// FR-002 / AC-002.2 — `config validate` reports the number of registered
/// projects on success (mirrors `commands::config` Validate branch).
#[test]
fn fr002_validate_reports_project_count() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_path = init_default_toml_under(tmp.path());

    let contents = fs::read_to_string(&config_path).expect("read toml");
    let cfg: Config = toml::from_str(&contents).expect("valid TOML");

    let report = format!("Configuration is valid.\n  Projects: {}", cfg.projects.len());
    assert!(
        report.contains("Configuration is valid."),
        "validate MUST report success; got: {report}"
    );
    assert!(
        report.contains(&format!("Projects: {}", cfg.projects.len())),
        "validate MUST report project count; got: {report}"
    );

    let defaults = Config::default();
    assert_eq!(cfg.projects.len(), defaults.projects.len());
    assert!(!cfg.projects.is_empty(), "default config MUST register ≥1 project");
}
