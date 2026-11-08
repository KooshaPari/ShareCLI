// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C11 L108 — packaging hard gate (T-990/T-1000/T-1010).
//!
//! FR: FR-001

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-001 / C11 L108 — packaging.yml hard-gate workflow exists and references real installers.
#[test]
fn packaging_workflow_exists() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/packaging.yml"))
        .expect("read packaging.yml");

    assert!(
        workflow.contains("hdiutil"),
        "packaging.yml must reference hdiutil for real DMG creation"
    );
    assert!(
        workflow.contains("dpkg-deb"),
        "packaging.yml must reference dpkg-deb for real DEB packaging"
    );
}

/// FR-001 / C11 L108 — DMG layout script exists for building macOS app bundle.
#[test]
fn dmg_layout_script_exists() {
    let path = repo_root().join("scripts/build_dmg_layout.sh");
    assert!(path.exists(), "scripts/build_dmg_layout.sh must exist (checked at {:?})", path);
}

/// FR-001 / C11 L108 — DEB layout script exists for building Linux .deb packages.
#[test]
fn deb_layout_script_exists() {
    let path = repo_root().join("scripts/build_deb.sh");
    assert!(path.exists(), "scripts/build_deb.sh must exist (checked at {:?})", path);
}
