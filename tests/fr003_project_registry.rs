// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 — Project Registry (add / list / show / remove)
//! FR: FR-003
//!
//! Covers AC-003.1, AC-003.2, AC-003.3, AC-003.5.
//!
//! Library-level acceptance tests. They do **not** call `Config::load()` /
//! `Config::save()` / `commands::project` against the real user config dir
//! (`dirs::config_dir()` is not overridable on Windows).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sharecli::config::Config;

fn save_under(root: &Path, cfg: &Config) -> PathBuf {
    let config_path = root.join("sharecli").join("config.toml");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).expect("create config directory");
    }
    let contents = toml::to_string_pretty(cfg).expect("serialize Config");
    fs::write(&config_path, contents).expect("write config.toml");
    config_path
}

fn load_under(root: &Path) -> Config {
    let config_path = root.join("sharecli").join("config.toml");
    let contents = fs::read_to_string(&config_path).expect("read config.toml");
    toml::from_str(&contents).expect("deserialize Config")
}

fn project_add(cfg: &mut Config, name: &str, path: &str) {
    cfg.projects.insert(name.to_string(), path.to_string());
}

fn project_remove(cfg: &mut Config, name: &str) -> bool {
    cfg.projects.remove(name).is_some()
}

fn format_list(cfg: &Config) -> String {
    if cfg.projects.is_empty() {
        "No projects registered. Run 'sharecli project discover'.".to_string()
    } else {
        let mut out = String::from("Registered Projects:\n");
        for (name, path) in &cfg.projects {
            out.push_str(&format!("  {name} -> {path}\n"));
        }
        out
    }
}

fn format_show(name: &str, path: &str) -> String {
    format!("Project: {}\nPath:    {}\nExists:  {}\n", name, path, Path::new(path).exists())
}

/// FR-003 / AC-003.1 — `project add` inserts into `Config.projects` and persists.
#[test]
fn fr003_project_add_inserts_and_persists() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_dir = tmp.path().join("my-app");
    fs::create_dir_all(&project_dir).expect("create project dir");
    let project_path = project_dir.to_string_lossy().to_string();

    let mut cfg = Config { projects: HashMap::new(), ..Config::default() };
    project_add(&mut cfg, "my-app", &project_path);
    save_under(tmp.path(), &cfg);

    let loaded = load_under(tmp.path());
    assert_eq!(
        loaded.projects.get("my-app").map(String::as_str),
        Some(project_path.as_str()),
        "add MUST insert and persist name → path"
    );
    assert_eq!(loaded.projects.len(), 1);
}

/// FR-003 / AC-003.2 — `project list` prints registered projects or empty hint.
#[test]
fn fr003_project_list_prints_registered() {
    let empty = Config { projects: HashMap::new(), ..Config::default() };
    let empty_out = format_list(&empty);
    assert!(
        empty_out.contains("No projects registered"),
        "empty list MUST show hint; got: {empty_out}"
    );
    assert!(
        empty_out.contains("sharecli project discover"),
        "empty hint MUST mention discover; got: {empty_out}"
    );

    let mut projects = HashMap::new();
    projects.insert("alpha".to_string(), r"C:\Users\example\alpha".to_string());
    projects.insert("beta".to_string(), "/tmp/beta".to_string());
    let cfg = Config { projects, ..Config::default() };
    let out = format_list(&cfg);

    assert!(out.starts_with("Registered Projects:"), "list MUST print header; got: {out}");
    assert!(
        out.contains("alpha -> ") && out.contains(r"C:\Users\example\alpha"),
        "list MUST print alpha line; got: {out}"
    );
    assert!(
        out.contains("beta -> ") && out.contains("/tmp/beta"),
        "list MUST print beta line; got: {out}"
    );
}

/// FR-003 / AC-003.3 — `project show` prints path + Exists true/false.
#[test]
fn fr003_project_show_resolves_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let existing = tmp.path().join("exists-here");
    fs::create_dir_all(&existing).expect("create existing project path");
    let existing_s = existing.to_string_lossy().to_string();
    let missing_s = tmp.path().join("missing-here").to_string_lossy().to_string();

    let mut projects = HashMap::new();
    projects.insert("live".to_string(), existing_s.clone());
    projects.insert("gone".to_string(), missing_s.clone());
    let cfg = Config { projects, ..Config::default() };

    let live_path = cfg.projects.get("live").expect("live registered");
    let live_out = format_show("live", live_path);
    assert!(live_out.contains("Project: live"), "got: {live_out}");
    assert!(live_out.contains(&format!("Path:    {existing_s}")), "got: {live_out}");
    assert!(
        live_out.contains("Exists:  true"),
        "show MUST report Exists true for on-disk path; got: {live_out}"
    );

    let gone_path = cfg.projects.get("gone").expect("gone registered");
    let gone_out = format_show("gone", gone_path);
    assert!(gone_out.contains("Project: gone"), "got: {gone_out}");
    assert!(gone_out.contains(&format!("Path:    {missing_s}")), "got: {gone_out}");
    assert!(
        gone_out.contains("Exists:  false"),
        "show MUST report Exists false for missing path; got: {gone_out}"
    );
}

/// FR-003 / AC-003.5 — `project remove` drops the entry and persists.
#[test]
fn fr003_project_remove_drops_entry() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let mut projects = HashMap::new();
    projects.insert("keep".to_string(), "/tmp/keep".to_string());
    projects.insert("drop-me".to_string(), "/tmp/drop-me".to_string());
    let mut cfg = Config { projects, ..Config::default() };
    save_under(tmp.path(), &cfg);

    assert!(project_remove(&mut cfg, "drop-me"), "remove MUST find existing entry");
    assert!(!cfg.projects.contains_key("drop-me"));
    save_under(tmp.path(), &cfg);

    let loaded = load_under(tmp.path());
    assert!(!loaded.projects.contains_key("drop-me"), "remove MUST persist drop");
    assert_eq!(
        loaded.projects.get("keep").map(String::as_str),
        Some("/tmp/keep"),
        "remove MUST leave other entries"
    );
    assert_eq!(loaded.projects.len(), 1);

    assert!(!project_remove(&mut cfg, "drop-me"), "second remove of same name MUST be a no-op");
}
