// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Level A landmark + screen-reader structure checks for the embedded serve dashboard.
//! FR: FR-004 NFR (C09 L81.1 / L81.4 / L81.5)

use std::fs;
use std::path::PathBuf;

fn dashboard_html() -> String {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("src/dashboard.html");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn dashboard_has_lang_and_landmarks() {
    let html = dashboard_html();
    assert!(html.contains(r#"<html lang="en">"#), "missing lang=en");
    assert!(html.contains("<main"), "missing <main> landmark");
    assert!(html.contains("<nav"), "missing <nav> landmark");
    assert!(
        html.contains(r#"aria-label="Dashboard status""#)
            || html.contains("aria-label='Dashboard status'"),
        "nav should have aria-label"
    );
    assert!(html.contains("id=\"main-content\""), "main needs skiplink target id");
    assert!(
        html.contains(r#"id="main-content" tabindex="-1""#),
        "main must be focusable skip-link target"
    );
}

#[test]
fn dashboard_announces_live_status() {
    let html = dashboard_html();
    assert!(html.contains("aria-live"), "connection/status updates should use aria-live");
}

#[test]
fn dashboard_sr_table_and_skip_link() {
    // L81.4 acceptance: labeled table + skip link target for SR/keyboard entry.
    let html = dashboard_html();
    assert!(
        html.contains(r#"aria-label="Managed processes""#),
        "process table needs aria-label for SR announcement"
    );
    assert!(
        html.contains(r#"aria-labelledby="dashboard-title""#),
        "main should be labelled by the page title"
    );
    assert!(html.matches("scope=\"col\"").count() >= 5, "every column header needs scope=col");
    assert!(
        html.contains("href=\"#main-content\"") && html.contains("Skip to process table"),
        "skip link must target main content"
    );
    // L81.4: every <img> opening tag must carry a standalone alt attribute
    // (bare imgs without alt are banned; data-alt= does not satisfy the rule).
    let every_img_has_alt =
        html.split("<img").skip(1).filter_map(|frag| frag.split('>').next()).all(|tag| {
            tag.split_whitespace().any(|attr| attr == "alt" || attr.starts_with("alt="))
        });
    assert!(every_img_has_alt, "dashboard must not ship <img> without an alt attribute");
}

#[test]
fn dashboard_has_responsive_breakpoints() {
    let html = dashboard_html();
    assert!(html.contains(r#"name="viewport""#), "missing viewport meta for adaptive layout");
    assert!(html.contains("@media (max-width: 768px)"), "missing tablet breakpoint smoke (768)");
    assert!(html.contains("@media (max-width: 375px)"), "missing phone breakpoint smoke (375)");
}
