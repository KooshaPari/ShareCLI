// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Config file watcher with debounced hot-reload.
//!
//! # Usage
//!
//! ```no_run
//! use std::path::PathBuf;
//! use tokio::sync::watch;
//! use sharecli::config::Config;
//! use sharecli::config_watcher::ConfigWatcher;
//!
//! let path = PathBuf::from("/etc/sharecli/config.toml");
//! let initial = Config::load().unwrap_or_default();
//! let (tx, rx) = watch::channel(initial);
//! let _watcher = ConfigWatcher::new(path, tx).expect("failed to start watcher");
//! // rx now receives updated Config values on every valid save.
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::watch;
use tracing::{error, info};

use crate::config::Config;

/// Debounce window: coalesce file-system events within this duration.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Watches a config file path and sends a new [`Config`] on `reload_tx`
/// whenever the file is created or modified with a valid TOML payload.
///
/// Parse errors are logged and the previous config is kept — the watcher
/// never crashes on bad input.
pub struct ConfigWatcher {
    /// Keep the watcher alive; dropped when `ConfigWatcher` is dropped.
    _watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    /// Start watching `path`.  Sends the initial (or reloaded) config on
    /// `reload_tx` for every valid `Create` / `Modify` event.
    pub fn new(path: PathBuf, reload_tx: watch::Sender<Config>) -> Result<Self> {
        // Shared last-event timestamp for debouncing.
        let last_event: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

        let path_clone = path.clone();
        let last_clone = Arc::clone(&last_event);

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    let relevant =
                        matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_));
                    if !relevant {
                        return;
                    }

                    // Debounce: record the event time; only act if ≥200 ms has
                    // elapsed since the *previous* action.
                    let now = Instant::now();
                    {
                        let mut guard = last_clone.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(prev) = *guard {
                            if now.duration_since(prev) < DEBOUNCE {
                                // Still within debounce window — skip.
                                return;
                            }
                        }
                        *guard = Some(now);
                    }

                    // Attempt to reload.
                    match reload_config(&path_clone) {
                        Ok(cfg) => {
                            info!("config_watcher: reloaded {:?}", path_clone);
                            // send() only errors when all receivers are gone; treat
                            // that as a no-op (the process is shutting down).
                            let _ = reload_tx.send(cfg);
                        }
                        Err(e) => {
                            error!(
                                "config_watcher: parse error in {:?} — keeping old config: {e}",
                                path_clone
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("config_watcher: watch error: {e}");
                }
            }
        })?;

        // Watch the file's parent directory so we also catch atomic rename-saves
        // (editors like vim, helix, and `sed -i` write to a temp file then rename).
        let watch_target = path.parent().unwrap_or(&path);
        watcher.watch(watch_target, RecursiveMode::NonRecursive)?;

        Ok(Self { _watcher: watcher })
    }
}

/// Re-read and parse the config file at `path`.
fn reload_config(path: &PathBuf) -> Result<Config> {
    let contents = std::fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&contents)?;
    Ok(cfg)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;
    use tokio::sync::watch;

    use super::*;

    /// Minimal valid TOML that round-trips through `Config`.
    fn minimal_toml() -> &'static str {
        // An empty document is valid because every field has `#[serde(default)]`.
        ""
    }

    // --- reload_config ---

    #[test]
    fn reload_config_returns_ok_for_valid_toml() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", minimal_toml()).unwrap();
        let result = reload_config(&f.path().to_path_buf());
        assert!(result.is_ok(), "expected Ok for valid TOML, got {result:?}");
    }

    #[test]
    fn reload_config_returns_err_for_invalid_toml() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "NOT = [valid toml}}}}").unwrap();
        let result = reload_config(&f.path().to_path_buf());
        assert!(result.is_err(), "expected Err for invalid TOML");
    }

    #[test]
    fn reload_config_returns_err_for_missing_file() {
        let path = PathBuf::from("/nonexistent/sharecli-test-config.toml");
        let result = reload_config(&path);
        assert!(result.is_err(), "expected Err for missing file");
    }

    // --- debounce logic ---

    #[test]
    fn debounce_suppresses_rapid_events() {
        // Simulate two events with zero elapsed time between them.
        let last_event: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

        let now = Instant::now();
        // First event: always passes.
        let first_passed = {
            let mut guard = last_event.lock().unwrap();
            match *guard {
                Some(prev) if now.duration_since(prev) < DEBOUNCE => false,
                _ => {
                    *guard = Some(now);
                    true
                }
            }
        };

        // Second event at the *same* instant — should be suppressed.
        let second_passed = {
            let mut guard = last_event.lock().unwrap();
            match *guard {
                Some(prev) if now.duration_since(prev) < DEBOUNCE => false,
                _ => {
                    *guard = Some(now);
                    true
                }
            }
        };

        assert!(first_passed, "first event should pass");
        assert!(!second_passed, "second event within debounce window should be suppressed");
    }

    #[test]
    fn debounce_allows_event_after_window_expires() {
        let last_event: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

        // Seed with a timestamp well in the past.
        {
            let past = Instant::now() - Duration::from_millis(500);
            *last_event.lock().unwrap() = Some(past);
        }

        let now = Instant::now();
        let passed = {
            let mut guard = last_event.lock().unwrap();
            match *guard {
                Some(prev) if now.duration_since(prev) < DEBOUNCE => false,
                _ => {
                    *guard = Some(now);
                    true
                }
            }
        };

        assert!(passed, "event after debounce window should be allowed through");
    }

    // --- ConfigWatcher::new wires up without panicking ---

    #[test]
    fn watcher_new_does_not_panic_on_existing_file() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", minimal_toml()).unwrap();

        let initial = Config::default();
        let (tx, _rx) = watch::channel(initial);

        let result = ConfigWatcher::new(f.path().to_path_buf(), tx);
        assert!(result.is_ok(), "ConfigWatcher::new should succeed for an existing file");
    }

    #[test]
    fn watcher_new_succeeds_for_nonexistent_file_path() {
        // The watcher watches the *parent* dir; the file itself need not exist yet.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let initial = Config::default();
        let (tx, _rx) = watch::channel(initial);

        let result = ConfigWatcher::new(path, tx);
        assert!(
            result.is_ok(),
            "ConfigWatcher::new should succeed even if the file doesn't exist yet"
        );
    }
}
