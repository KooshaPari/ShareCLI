//! Filesystem-timing coverage for config hot-reload.
//!
//! FR: FR-002 (config load) / config hot-reload
//!
//! Phase 1 task 1.2. These exercise the watcher against the real file system,
//! so they live at the integration tier rather than beside the deterministic
//! `Debouncer` unit tests in `src/config_watcher/debounce.rs`.
//!
//! AC-12.3 five edits inside the 200 ms debounce window → exactly one reload
//! AC-12.4 a burst of saves spanning the window coalesces into one reload that
//!            observes the **final** file content
//!
//! Red before the trailing-edge rewrite: AC-12.4 observed 4 reloads for a burst
//! of 6 saves, each reading an intermediate state of the file.

use std::time::{Duration, Instant};

use sharecli::config::Config;
use sharecli::config_watcher::ConfigWatcher;
use tokio::sync::watch;

/// Phase 1 task 1.2: a burst of saves must coalesce into **one** reload
/// that observes the **final** file content.
///
/// Under a leading-edge debouncer this produced 4 reloads for a burst of 6
/// saves, each reading an intermediate state of the file.
#[test]
fn trailing_debounce_coalesces_a_burst_into_one_reload_of_final_content() {
    use std::sync::mpsc;
    use std::thread;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[projects]\nprobe = \"v0\"\n").expect("seed");

    let (tx, mut rx) = watch::channel(Config::default());
    let watcher = ConfigWatcher::new(path.clone(), tx).expect("watcher");

    // Observer starts before the burst so no reload can be missed.
    let (done_tx, done_rx) = mpsc::channel::<(usize, Option<String>)>();
    let observer = thread::spawn(move || {
        let mut reloads = 0usize;
        let mut last = None;
        let deadline = Instant::now() + Duration::from_millis(3000);
        while Instant::now() < deadline {
            if rx.has_changed().expect("watch channel alive") {
                let cfg = rx.borrow_and_update().clone();
                last = cfg.projects.get("probe").cloned();
                reloads += 1;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = done_tx.send((reloads, last));
    });

    // Six saves 150 ms apart: each falls outside the previous event's
    // debounce window under a leading-edge scheme, but none is followed by
    // DEBOUNCE of quiet until the sixth.
    for i in 1..=6 {
        std::fs::write(&path, format!("[projects]\nprobe = \"v{i}\"\n"))
            .unwrap_or_else(|e| panic!("save {i}: {e}"));
        thread::sleep(Duration::from_millis(150));
    }

    let (reloads, last) = done_rx.recv().expect("observer finished");
    drop(watcher);
    observer.join().expect("observer thread");

    assert_eq!(
        reloads, 1,
        "a burst of 6 saves must coalesce into exactly one reload, got {reloads}"
    );
    assert_eq!(
        last.as_deref(),
        Some("v6"),
        "the single reload must observe the final file content, got {last:?}"
    );
}

/// The plan's stated acceptance for task 1.2: five rapid edits inside the
/// 200 ms window must produce exactly one reload.
///
/// The burst test above covers a stronger case (saves spanning the window);
/// this pins the literal requirement.
#[test]
fn five_edits_within_the_window_produce_exactly_one_reload() {
    use std::sync::mpsc;
    use std::thread;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[projects]\nprobe = \"v0\"\n").expect("seed");

    let (tx, mut rx) = watch::channel(Config::default());
    let watcher = ConfigWatcher::new(path.clone(), tx).expect("watcher");
    thread::sleep(Duration::from_millis(300));

    for i in 1..=5 {
        std::fs::write(&path, format!("[projects]\nprobe = \"v{i}\"\n"))
            .unwrap_or_else(|e| panic!("edit {i}: {e}"));
    }

    let deadline = Instant::now() + Duration::from_millis(900);
    let mut reloads = 0usize;
    while Instant::now() < deadline {
        if rx.has_changed().expect("watch channel alive") {
            let _ = rx.borrow_and_update();
            reloads += 1;
        }
        thread::sleep(Duration::from_millis(20));
    }
    drop(watcher);

    assert_eq!(
        reloads, 1,
        "5 edits inside the debounce window must yield exactly one reload, got {reloads}"
    );
}
