// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C10 L101 — designed error / failure states with recovery actions + tier-1 illustration.
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C10 L101 — dashboard ships disconnect error panel with recovery CTA.
#[test]
fn c10_l101_dashboard_disconnect_error_markup() {
    let html =
        fs::read_to_string(repo_root().join("src/dashboard.html")).expect("read dashboard.html");
    for needle in [
        "error-state",
        "data-error-kind=\"disconnect\"",
        "renderDisconnectError",
        "Dashboard disconnected",
        "sharecli serve",
        "error-retry",
        "Retry now",
        "/assets/dashboard/ui/error-states/disconnect.svg",
        "data-illustration=\"disconnect-tier1\"",
    ] {
        assert!(html.contains(needle), "dashboard MUST include {needle} for L101");
    }
    assert!(
        !html.contains("/assets/dashboard/ui/empty-states/error.svg"),
        "disconnect panel must not use abstract empty-states/error.svg decoration"
    );
}

/// FR-003 / C10 L101 — error-state contract documented.
#[test]
fn c10_l101_error_states_doc_present() {
    let doc = fs::read_to_string(repo_root().join("docs/visual/error-states.md"))
        .expect("read error-states.md");
    assert!(
        doc.contains("data-error-kind=\"disconnect\""),
        "error-states.md must document disconnect kind"
    );
    assert!(doc.contains("Retry now"), "error-states.md must document retry CTA");
    assert!(
        doc.contains("sharecli serve --bind"),
        "error-states.md must document serve recovery command"
    );
    assert!(
        doc.contains("error-states/disconnect.svg"),
        "error-states.md must document tier-1 disconnect illustration path"
    );
    assert!(
        doc.contains("tier-1") || doc.contains("Tier-1"),
        "error-states.md must declare illustration provenance tier-1"
    );
}

/// FR-003 / C10 L101 — bespoke disconnect SVG is a real scene (not pack decoration).
#[test]
fn c10_l101_disconnect_illustration_tier1_asset() {
    let svg_path = repo_root().join("assets/dashboard/ui/error-states/disconnect.svg");
    assert!(svg_path.is_file(), "missing {}", svg_path.display());
    let svg = fs::read_to_string(&svg_path).expect("read disconnect.svg");
    for needle in [
        "Feed disconnected",
        "WebSocket",
        "#f85149", // --bb2-error
        "serve :9000",
        "dashboard",
        "tier-1",
    ] {
        assert!(
            svg.contains(needle),
            "disconnect.svg MUST include {needle} (real disconnect scene)"
        );
    }
    // Reject abstract Phenotype pack motif reused as "error illustration"
    assert!(
        !svg.contains("#7ebab5"),
        "disconnect.svg must use Backbone-2 tokens, not Keycap teal pack decoration"
    );
    assert!(
        !svg.contains("Something went wrong"),
        "disconnect.svg must not reuse generic pack copy"
    );

    let provenance = fs::read_to_string(repo_root().join("docs/visual/PROVENANCE.md"))
        .expect("read PROVENANCE.md");
    assert!(
        provenance.contains("error-states/disconnect.svg"),
        "PROVENANCE.md must list disconnect.svg"
    );
    assert!(
        provenance.contains("`assets/dashboard/ui/error-states/disconnect.svg` | 1"),
        "PROVENANCE.md must declare disconnect.svg as tier 1"
    );
}

/// FR-003 / C10 L101 — disconnect illustration is embedded for serve.
#[test]
fn c10_l101_disconnect_illustration_embedded() {
    let assets = fs::read_to_string(repo_root().join("src/dashboard_assets.rs"))
        .expect("read dashboard_assets.rs");
    assert!(
        assets.contains("error-states/disconnect.svg"),
        "dashboard_assets.rs must serve error-states/disconnect.svg"
    );
    assert!(
        assets.contains("include_bytes!(\"../assets/dashboard/ui/error-states/disconnect.svg\")"),
        "dashboard_assets.rs must include_bytes the disconnect SVG"
    );
}
