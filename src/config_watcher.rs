//! Config file watcher with trailing-edge debounced hot-reload.
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
//!
//! # Debounce policy
//!
//! The debouncer is **trailing**: it waits for a quiet window of `DEBOUNCE`
//! (see [`debounce`]) measured from the *latest* event, then reloads once.
//! A leading-edge scheme fires on the first event and drops everything inside
//! the window; for a burst of saves that produces several reloads of
//! intermediate states and can lose the final save entirely. See
//! `trailing_debounce_coalesces_a_burst_into_one_reload_of_final_content`.

mod debounce;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::watch;
use tracing::{error, info};

use crate::config::Config;
use debounce::{lock, wait, Debouncer, Shared};

/// Watches a config file path and sends a new [`Config`] on `reload_tx`
/// whenever the file is created or modified with a valid TOML payload.
///
/// Parse errors are logged and the previous config is kept — the watcher
/// never crashes on bad input.
pub struct ConfigWatcher {
    /// Keep the file watcher alive; dropped when `ConfigWatcher` is dropped.
    _watcher: RecommendedWatcher,
    /// Coordination shared with the notify callback and debounce thread.
    shared: Arc<Shared>,
    /// The trailing-edge debounce thread; joined on drop.
    debounce_thread: Option<std::thread::JoinHandle<()>>,
}

impl ConfigWatcher {
    /// Start watching `path`. Sends a config on `reload_tx` for each settled
    /// batch of `Create` / `Modify` events.
    pub fn new(path: PathBuf, reload_tx: watch::Sender<Config>) -> Result<Self> {
        let shared = Arc::new(Shared::new());

        // The notify callback only records events; all reloading happens on the
        // debounce thread so the trailing deadline can be extended safely.
        let callback_shared = Arc::clone(&shared);
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        return;
                    }
                    let mut guard = callback_shared.state.lock().unwrap_or_else(|e| e.into_inner());
                    guard.record(Instant::now());
                    drop(guard);
                    callback_shared.cv.notify_one();
                }
                Err(e) => {
                    error!("config_watcher: watch error: {e}");
                }
            })?;

        // Watch the file's parent directory so we also catch atomic rename-saves
        // (editors like vim, helix, and `sed -i` write to a temp file then rename).
        let watch_target = path.parent().unwrap_or(&path).to_path_buf();
        watcher.watch(&watch_target, RecursiveMode::NonRecursive)?;

        let thread_shared = Arc::clone(&shared);
        let thread_path = path.clone();
        let debounce_thread =
            std::thread::Builder::new().name("config-debounce".into()).spawn(move || {
                run_debounced_reload(thread_path, thread_shared, reload_tx);
            })?;

        Ok(Self { _watcher: watcher, shared, debounce_thread: Some(debounce_thread) })
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        {
            let mut guard = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.shutdown = true;
        }
        self.shared.cv.notify_all();
        if let Some(handle) = self.debounce_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Wait for `DEBOUNCE` of quiet after the latest event, then reload once.
///
/// Runs on its own thread so the notify callback stays cheap and so the
/// deadline can be extended when further events arrive mid-wait.
fn run_debounced_reload(path: PathBuf, shared: Arc<Shared>, reload_tx: watch::Sender<Config>) {
    loop {
        // 1. Wait for an outstanding batch.
        let (mut generation, mut due) = {
            let mut guard = lock(&shared);
            while !guard.pending && !guard.shutdown {
                guard = wait(&shared, guard);
            }
            if guard.shutdown {
                return;
            }
            (guard.generation, guard.due_at().expect("pending implies an event"))
        };

        // 2. Sleep to the trailing deadline, following any newer event.
        loop {
            let now = Instant::now();
            if now >= due {
                break;
            }
            std::thread::sleep(due - now);
            let guard = lock(&shared);
            if guard.shutdown {
                return;
            }
            if guard.generation != generation {
                generation = guard.generation;
                due = guard.due_at().expect("pending implies an event");
                continue;
            }
            drop(guard);
            if Instant::now() >= due {
                break;
            }
            // Woke early; sleep out the remainder.
        }

        // 3. Claim the batch. A newer event arriving here means we wait again
        //    rather than reloading a file that is still being written.
        if !lock(&shared).claim(generation) {
            continue;
        }

        // 4. Reload the settled file.
        match reload_config(&path) {
            Ok(cfg) => {
                info!("config_watcher: reloaded {path:?}");
                // send() only errors when all receivers are gone; treat that as
                // a no-op (the process is shutting down).
                let _ = reload_tx.send(cfg);
            }
            Err(e) => {
                error!("config_watcher: parse error in {path:?} — keeping old config: {e}");
            }
        }
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
    use std::time::Duration;

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
