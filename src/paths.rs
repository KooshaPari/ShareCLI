// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Well-known config/state/runtime paths for install/uninstall QOL (C11 L121).

use std::path::{Path, PathBuf};

/// Resolved platform paths used by sharecli for config, state, and runtime locks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WellKnownPaths {
    /// `~/.config/sharecli` (or platform equivalent).
    pub config_dir: PathBuf,
    /// `$XDG_STATE_HOME/sharecli` or `~/.local/state/sharecli`.
    pub state_dir: PathBuf,
    /// `$XDG_RUNTIME_DIR` when set, else system temp (serve lock dir parent).
    pub runtime_dir: PathBuf,
}

/// Collect the directories sharecli uses for persistent and runtime state.
pub fn well_known_paths() -> WellKnownPaths {
    let config_dir = config_dir();
    WellKnownPaths { config_dir, state_dir: state_dir(), runtime_dir: runtime_dir() }
}

fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("sharecli");
        }
    }
    dirs::config_dir()
        .map(|b| b.join("sharecli"))
        .unwrap_or_else(|| PathBuf::from(".config/sharecli"))
}

fn state_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("sharecli");
        }
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".local").join("state").join("sharecli");
    }
    PathBuf::from(".local/state/sharecli")
}

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// Remove sharecli data directories when `purge_data` is true.
///
/// When `dry_run` is true, only prints the paths that would be removed.
pub fn purge_data_dirs(purge_data: bool, dry_run: bool) -> std::io::Result<Vec<PathBuf>> {
    let paths = well_known_paths();
    let mut targets = Vec::new();
    if purge_data {
        targets.push(paths.config_dir.clone());
        targets.push(paths.state_dir.clone());
    }
    // Runtime locks are ephemeral; remove stale lock files only when purging.
    if purge_data {
        let lock_glob_parent = paths.runtime_dir.clone();
        if lock_glob_parent.is_dir() {
            for entry in std::fs::read_dir(&lock_glob_parent)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("sharecli") {
                    targets.push(entry.path());
                }
            }
        }
    }

    if !purge_data {
        return Ok(targets);
    }

    for path in &targets {
        if dry_run {
            continue;
        }
        remove_path(path)?;
    }
    Ok(targets)
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    // FR-003 — uninstall path resolution for C11 L121.
    #[test]
    fn fr003_well_known_paths_include_config_and_state() {
        let paths = well_known_paths();
        assert!(paths.config_dir.ends_with("sharecli"));
        assert!(paths.state_dir.ends_with("sharecli"));
    }

    #[test]
    fn fr003_purge_dry_run_does_not_delete() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = tmp.path().join("sharecli");
        fs::create_dir_all(&cfg).expect("mkdir");
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        }
        let paths = well_known_paths();
        assert_eq!(paths.config_dir, cfg);
        let removed = purge_data_dirs(true, true).expect("dry run");
        assert!(removed.iter().any(|p| p == &cfg));
        assert!(cfg.exists(), "dry_run must not delete");
        match prev {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
    }
}
