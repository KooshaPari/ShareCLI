// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 / C10 L107 — Phenotype UI pack wired into dashboard + serve static assets.
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn c10_dashboard_html_references_ui_pack_assets() {
    let html =
        fs::read_to_string(repo_root().join("src/dashboard.html")).expect("read dashboard.html");
    for needle in [
        "/assets/dashboard/ui/favicons/phenotype.ico",
        "/assets/dashboard/ui/favicons/phenotype_32.png",
        "/assets/dashboard/ui/favicons/phenotype_128.png",
        "/assets/dashboard/ui/banners/dashboard_1280x320.png",
        "/assets/dashboard/ui/empty-states/no-data.svg",
        "/assets/dashboard/ui/empty-states/no-results.svg",
        "/assets/dashboard/ui/error-states/disconnect.svg",
    ] {
        assert!(html.contains(needle), "dashboard.html must reference {needle}");
    }
}

#[test]
fn c10_dashboard_ui_pack_files_present_on_disk() {
    let base = repo_root().join("assets/dashboard/ui");
    for rel in [
        "favicons/phenotype.ico",
        "favicons/phenotype_32.png",
        "banners/dashboard_1280x320.png",
        "empty-states/no-data.svg",
        "empty-states/no-results.svg",
        "empty-states/error.svg",
        "error-states/disconnect.svg",
    ] {
        let path = base.join(rel);
        assert!(path.is_file(), "missing ui-pack file: {}", path.display());
    }
}

#[test]
fn c10_dashboard_assets_module_embeds_served_paths() {
    use sharecli::dashboard_assets;

    assert!(dashboard_assets::is_dashboard_asset_path(
        "/assets/dashboard/ui/favicons/phenotype_32.png"
    ));
    assert_eq!(dashboard_assets::URL_PREFIX, "/assets/dashboard/ui");
}
