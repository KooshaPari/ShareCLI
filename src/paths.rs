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

    /// Helper: install an env override for the duration of one test, restore on drop.
    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let previous = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn xdg_config_home_takes_precedence_over_default() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("XDG_CONFIG_HOME", tmp.path());
        let paths = well_known_paths();
        assert_eq!(paths.config_dir, tmp.path().join("sharecli"));
    }

    #[test]
    fn xdg_state_home_takes_precedence_over_default() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("XDG_STATE_HOME", tmp.path());
        let paths = well_known_paths();
        assert_eq!(paths.state_dir, tmp.path().join("sharecli"));
    }

    #[test]
    fn xdg_runtime_dir_takes_precedence_over_temp() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _g = EnvGuard::set("XDG_RUNTIME_DIR", tmp.path());
        let paths = well_known_paths();
        assert_eq!(paths.runtime_dir, tmp.path());
    }

    #[test]
    fn empty_xdg_var_falls_back_to_default() {
        // XDG_CONFIG_HOME="" must NOT be honored — empty strings are treated as unset.
        let _g = EnvGuard::set("XDG_CONFIG_HOME", std::path::Path::new(""));
        let paths = well_known_paths();
        // Should not equal "" / "sharecli"; must use the default fallback path.
        assert!(paths.config_dir.ends_with("sharecli"));
        assert_ne!(paths.config_dir, std::path::PathBuf::from("sharecli"));
    }

    #[test]
    fn purge_data_when_purge_false_returns_no_targets() {
        // !purge_data short-circuits the target list to empty.
        let removed = purge_data_dirs(false, false).expect("no-op");
        assert!(removed.is_empty(), "purge_data=false must return no targets");
    }

    #[test]
    fn purge_data_dry_run_collects_sharecli_lock_entries() {
        let cfg_tmp = tempfile::tempdir().expect("tempdir");
        let state_tmp = tempfile::tempdir().expect("tempdir");
        let run_tmp = tempfile::tempdir().expect("tempdir");
        let cfg_g = EnvGuard::set("XDG_CONFIG_HOME", cfg_tmp.path());
        let state_g = EnvGuard::set("XDG_STATE_HOME", state_tmp.path());
        let run_g = EnvGuard::set("XDG_RUNTIME_DIR", run_tmp.path());
        // Drop only after the assertion so XDG_RUNTIME_DIR is still set when
        // purge_data_dirs reads it.
        let cfg_dir = cfg_tmp.path().join("sharecli");
        let state_dir = state_tmp.path().join("sharecli");
        fs::create_dir_all(&cfg_dir).expect("mkdir config");
        fs::create_dir_all(&state_dir).expect("mkdir state");
        let stale_lock = run_tmp.path().join("sharecli.lock");
        let fresh_lock = run_tmp.path().join("sharecli-stale.sock");
        let unrelated = run_tmp.path().join("not-sharecli.lock");
        fs::write(&stale_lock, b"").expect("write lock");
        fs::write(&fresh_lock, b"").expect("write sock");
        fs::write(&unrelated, b"").expect("write unrelated");
        let removed = purge_data_dirs(true, true).expect("dry run");
        // Both config and state dirs appear in targets.
        assert!(removed.contains(&cfg_dir), "config dir missing from targets: {removed:?}");
        assert!(removed.contains(&state_dir), "state dir missing from targets: {removed:?}");
        // Lock files prefixed with `sharecli` are picked up; unrelated names are not.
        assert!(removed.contains(&stale_lock), "stale lock missing: {removed:?}");
        assert!(removed.contains(&fresh_lock), "sharecli- prefix lock missing: {removed:?}");
        assert!(
            !removed.contains(&unrelated),
            "unrelated lock should not be purged: {removed:?}"
        );
        // Dry run: nothing was actually deleted.
        assert!(stale_lock.exists());
        assert!(unrelated.exists());
        drop(run_g);
        drop(state_g);
        drop(cfg_g);
    }
}
