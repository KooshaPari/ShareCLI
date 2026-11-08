// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C10 L99 — designed loading / skeleton states for async dashboard views.
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C10 L99 — dashboard ships content-shaped skeleton rows + busy region.
#[test]
fn c10_l99_dashboard_skeleton_markup() {
    let html =
        fs::read_to_string(repo_root().join("src/dashboard.html")).expect("read dashboard.html");
    for needle in [
        "renderSkeletonRows",
        "renderOperatorPanelSkeletons",
        "skeleton-row",
        "skeleton-bar",
        "data-loading-kind=\"table-row\"",
        "data-loading-kind=\"panel-value\"",
        "skeleton-shimmer",
        "aria-busy",
        "#status-dot.connecting",
        "connected — loading processes…",
        "#operator-panels",
    ] {
        assert!(html.contains(needle), "dashboard MUST include {needle} for L99");
    }
}

/// FR-003 / C10 L99 — loading-state contract documented.
#[test]
fn c10_l99_loading_states_doc_present() {
    let doc = fs::read_to_string(repo_root().join("docs/visual/loading-states.md"))
        .expect("read loading-states.md");
    assert!(
        doc.contains("data-loading-kind=\"table-row\""),
        "loading-states.md must document skeleton row kind"
    );
    assert!(
        doc.contains("data-loading-kind=\"panel-value\""),
        "loading-states.md must document operator panel skeleton kind"
    );
    assert!(
        doc.contains("renderOperatorPanelSkeletons"),
        "loading-states.md must reference operator panel skeleton implementation"
    );
    assert!(doc.contains("aria-busy"), "loading-states.md must document busy region");
    assert!(doc.contains("renderSkeletonRows"), "loading-states.md must reference implementation");
    assert!(
        doc.contains("prefers-reduced-motion"),
        "loading-states.md must document reduced-motion collapse"
    );
}
