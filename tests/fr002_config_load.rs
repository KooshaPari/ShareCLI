// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-002 — TOML Configuration Management (show / load / defaults)
//! FR: FR-002
//!
//! Covers AC-002.3, AC-002.4, AC-002.5.

use std::collections::HashMap;
use std::fs;

use sharecli::config::{Config, RuntimeConfig};

/// FR-002 / AC-002.3 — `config show` prints serialized TOML containing
/// `[projects]` and `[runtime]` tables (mirrors ConfigCmd::Show).
#[test]
fn fr002_show_prints_projects_and_runtime() {
    let cfg = Config::default();
    let serialized = toml::to_string_pretty(&cfg).expect("serialize Config for show");

    assert!(
        serialized.contains("[projects]") || serialized.contains("projects"),
        "show TOML MUST contain projects; got:\n{serialized}"
    );
    assert!(
        serialized.contains("[runtime]") || serialized.contains("runtime"),
        "show TOML MUST contain runtime; got:\n{serialized}"
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    fs::write(&path, &serialized).expect("write show document");
    let from_disk = fs::read_to_string(&path).expect("read show document");
    assert!(from_disk.contains("projects") || from_disk.contains("[projects]"));
    assert!(from_disk.contains("runtime") || from_disk.contains("[runtime]"));
}

/// FR-002 / AC-002.4 — A `Config` deserialized from TOML preserves the
/// `projects` map and `RuntimeConfig` fields.
#[test]
fn fr002_load_roundtrips_projects_map() {
    let mut projects = HashMap::new();
    projects.insert("demo".to_string(), r"C:\Users\example\demo".to_string());
    projects.insert("other".to_string(), "/tmp/other".to_string());

    let cfg = Config {
        projects,
        runtime: RuntimeConfig {
            node_path: Some("node".to_string()),
            bun_path: Some("bun".to_string()),
            max_memory_mb: Some(2048),
            max_processes: Some(42),
        },
        ..Config::default()
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    let toml_text = toml::to_string_pretty(&cfg).expect("serialize");
    fs::write(&path, &toml_text).expect("write");

    let loaded: Config =
        toml::from_str(&fs::read_to_string(&path).expect("read")).expect("deserialize");

    assert_eq!(loaded.projects.len(), 2);
    assert_eq!(loaded.projects.get("demo").map(String::as_str), Some(r"C:\Users\example\demo"));
    assert_eq!(loaded.projects.get("other").map(String::as_str), Some("/tmp/other"));
    assert_eq!(loaded.runtime.node_path.as_deref(), Some("node"));
    assert_eq!(loaded.runtime.bun_path.as_deref(), Some("bun"));
    assert_eq!(loaded.runtime.max_memory_mb, Some(2048));
    assert_eq!(loaded.runtime.max_processes, Some(42));
}

/// FR-002 / AC-002.5 — `RuntimeConfig::default()` returns the documented caps.
#[test]
fn fr002_runtime_config_default_values() {
    let rt = RuntimeConfig::default();
    assert_eq!(rt.max_memory_mb, Some(4096));
    assert_eq!(rt.max_processes, Some(100));
    assert_eq!(rt.node_path, None);
    assert_eq!(rt.bun_path, None);
}
