// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 acceptance gates for C09 L81.12 (Recognition Over Recall) and
//! L81.15 (Aesthetic & Minimalist Design — CTA token system).
//!
//! Tests the `sharecli history` subcommand and CTA token presence in
//! the dashboard CSS and theme source.

use std::fs;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tokens_css() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("assets");
    p.push("tokens.css");
    p
}

fn dashboard_html() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("src");
    p.push("dashboard.html");
    p
}

// ---------------------------------------------------------------------------
// C09 L81.12 — Recognition Over Recall
// ---------------------------------------------------------------------------

#[test]
fn fr003_history_module_exists() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join("commands").join("history.rs");
    assert!(path.exists(), "history.rs must exist at {}", path.display());
}

#[test]
fn fr003_history_append_and_read_roundtrip() {
    use sharecli::commands::history::{append_to, clear, read_recent, HistoryEntry};

    let dir = std::env::temp_dir().join("sharecli_hist_gate_test");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("history.jsonl");
    let _ = clear(&path);

    // Append 3 entries using the explicit path variant.
    append_to(&HistoryEntry::now("ps", "--json", 0), &path);
    append_to(&HistoryEntry::now("status", "", 1), &path);
    append_to(&HistoryEntry::now("serve", "--port 9000", 0), &path);

    let entries = read_recent(&path, 100).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].command, "ps");
    assert_eq!(entries[1].command, "status");
    assert_eq!(entries[2].exit_code, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fr003_history_recent_limit_works() {
    use sharecli::commands::history::{append_to, clear, read_recent, HistoryEntry};

    let dir = std::env::temp_dir().join("sharecli_hist_limit_test");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("history.jsonl");
    let _ = clear(&path);

    for i in 0..50 {
        append_to(&HistoryEntry::now(&format!("cmd{}", i), "", 0), &path);
    }

    let entries = read_recent(&path, 5).unwrap();
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0].command, "cmd45");
    assert_eq!(entries[4].command, "cmd49");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fr003_history_source_has_deserialize_derive() {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join("commands").join("history.rs"),
    )
    .expect("read history.rs");
    assert!(source.contains("Deserialize"), "HistoryEntry must derive Deserialize");
}

#[test]
fn fr003_history_source_defines_append_to() {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join("commands").join("history.rs"),
    )
    .expect("read history.rs");
    assert!(source.contains("pub fn append_to"), "history.rs must define pub fn append_to");
}

#[test]
fn fr003_history_fresh_install_returns_empty() {
    use sharecli::commands::history::{clear, read_recent};

    let dir = std::env::temp_dir().join("sharecli_hist_fresh_test");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("nonexistent_history.jsonl");
    // Ensure file does not exist
    let _ = fs::remove_file(&path);

    // read_recent should return empty vec on missing file (fresh install)
    let entries = read_recent(&path, 20).unwrap();
    assert!(entries.is_empty(), "fresh install must return empty history");

    // clear should succeed on missing file (creates parent dirs + empty file)
    clear(&path).expect("clear must succeed on missing file");
    assert!(path.exists(), "clear must create the file");

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// C09 L81.15 — CTA Token System
// ---------------------------------------------------------------------------

#[test]
fn fr003_cta_primary_token_in_css() {
    let css = fs::read_to_string(tokens_css()).expect("read tokens.css");
    assert!(css.contains("--bb2-cta-primary"), "tokens.css must define --bb2-cta-primary");
    assert!(
        css.contains("--bb2-cta-primary-text"),
        "tokens.css must define --bb2-cta-primary-text"
    );
}

#[test]
fn fr003_cta_secondary_token_in_css() {
    let css = fs::read_to_string(tokens_css()).expect("read tokens.css");
    assert!(css.contains("--bb2-cta-secondary"), "tokens.css must define --bb2-cta-secondary");
    assert!(
        css.contains("--bb2-cta-secondary-text"),
        "tokens.css must define --bb2-cta-secondary-text"
    );
}

#[test]
fn fr003_cta_tokens_present_in_dark_and_light_themes() {
    let css = fs::read_to_string(tokens_css()).expect("read tokens.css");
    let count = css.matches("--bb2-cta-primary").count();
    assert!(
        count >= 3,
        "--bb2-cta-primary must appear in dark + light + media query blocks (found {})",
        count
    );
}

#[test]
fn fr003_cta_primary_matches_pulse_green_in_dark() {
    let css = fs::read_to_string(tokens_css()).expect("read tokens.css");
    let first_cta = css
        .lines()
        .find(|l| l.contains("--bb2-cta-primary:"))
        .expect("must have --bb2-cta-primary line");
    assert!(
        first_cta.contains("#3fb950"),
        "dark CTA primary should be #3fb950 (pulse-green), got: {}",
        first_cta.trim()
    );
}

#[test]
fn fr003_cta_button_classes_in_dashboard() {
    let html = fs::read_to_string(dashboard_html()).expect("read dashboard.html");
    assert!(html.contains("cta-primary"), "dashboard.html must define .cta-primary class");
    assert!(html.contains("cta-secondary"), "dashboard.html must define .cta-secondary class");
    assert!(
        html.contains("var(--bb2-cta-primary)"),
        ".cta-primary must use var(--bb2-cta-primary)"
    );
    assert!(
        html.contains("var(--bb2-cta-secondary)"),
        ".cta-secondary must use var(--bb2-cta-secondary)"
    );
}
