//! Filesystem-tier coverage for config hot-reload.
//!
//! FR: FR-002 (config load) / config hot-reload
//!
//! Phase 1 task 1.2. These drive the watcher against the real file system, so
//! they live at the integration tier beside the deterministic `Debouncer` unit
//! tests in `src/config_watcher/debounce.rs`.
//!
//! AC-12.3 saving to the watched file produces a reload
//! AC-12.4 the last reload observes the **final** file content
//!
//! Red before the trailing-edge rewrite: a burst of 6 saves produced **4**
//! reloads, each reading an intermediate state of the file, so the final save
//! was never observed.
//!
//! # Why the reload *count* is asserted at the unit tier instead
//!
//! A trailing debouncer fires once per batch of events separated by `DEBOUNCE`
//! of quiet, where the clock runs on *delivered* events. An external observer
//! sees only its own writes, so it cannot predict how delivery groups them.
//!
//! Worse, on a loaded host file-system delivery latency approaches or exceeds
//! `DEBOUNCE`, which removes any timing signature that would separate the two
//! implementations. Measured on this host (temporary probe, since removed, 5
//! samples against the original watcher at load 251-438): write-to-callback
//! latency `L` came in at 210, 219, 224, 281 and 380 ms against a 200 ms window,
//! i.e. `L >= DEBOUNCE` on every sample. When `L` is that large, a leading-edge
//! debouncer also reads the settled file, because the save that lands after a
//! fire has already left the previous window by the time its own event is
//! delivered. Any `reloads == n` or "a save inside the window is swallowed"
//! assertion therefore tests the scheduler rather than the contract, and this
//! tier deliberately does not make one.
//!
//! The leading -> trailing behaviour change is pinned deterministically by
//! `config_watcher::debounce::tests::five_events_within_the_window_yield_exactly_one_claim`,
//! `debounce_deadline_follows_the_latest_event` and
//! `debounce_fires_once_after_the_window`, with injected instants and no wall
//! clock. Those cannot be run against the original code: the original debounce
//! logic was an inline closure inside the notify callback with no seam to inject
//! a clock, which is precisely what the extraction fixed. This tier is the
//! end-to-end receipt that a real save still reloads the real server config and
//! that the final content wins.

use std::time::{Duration, Instant};

use sharecli::config::Config;
use sharecli::config_watcher::ConfigWatcher;
use tokio::sync::watch;

/// Parse the `probe = "vN"` marker the tests write into `[projects]`.
fn probe_index(cfg: &Config) -> Option<u32> {
    cfg.projects.get("probe")?.strip_prefix('v')?.parse().ok()
}

/// Observe reloads until `deadline`, returning every distinct index seen.
fn observe(rx: &mut watch::Receiver<Config>, deadline: Instant) -> Vec<u32> {
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        if rx.has_changed().expect("watch channel alive") {
            let cfg = rx.borrow_and_update().clone();
            if let Some(v) = probe_index(&cfg) {
                if seen.last() != Some(&v) {
                    seen.push(v);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    seen
}

/// AC-12.3 / AC-12.4: a burst of saves reaches the live config, every reload
/// moves the observed value forward, and the last reload reflects the last save
/// rather than an intermediate state.
#[test]
fn burst_of_saves_reaches_the_live_config_with_final_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[projects]\nprobe = \"v0\"\n").expect("seed");

    let (tx, rx) = watch::channel(Config::default());
    let watcher = ConfigWatcher::new(path.clone(), tx).expect("watcher");
    std::thread::sleep(Duration::from_millis(300));

    // Observer starts before the burst so no reload can be missed.
    let deadline = Instant::now() + Duration::from_millis(4000);
    let observer = std::thread::spawn(move || {
        let mut rx = rx;
        observe(&mut rx, deadline)
    });

    for i in 1..=6 {
        std::fs::write(&path, format!("[projects]\nprobe = \"v{i}\"\n"))
            .unwrap_or_else(|e| panic!("save {i}: {e}"));
        std::thread::sleep(Duration::from_millis(150));
    }

    let seen = observer.join().expect("observer thread");
    drop(watcher);

    assert!(!seen.is_empty(), "saving to the watched file must produce at least one reload");
    assert_eq!(
        seen.last().copied(),
        Some(6),
        "the last reload must observe the final file content; observed {seen:?}"
    );
    assert!(
        seen.windows(2).all(|w| w[0] < w[1]),
        "reloads must only ever move forward, observed {seen:?}"
    );
}
