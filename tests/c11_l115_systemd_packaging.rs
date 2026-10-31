// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C11 L115 — traditional server packaged unit in release artifacts (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C11 L115 — `.deb` build installs systemd unit alongside binary.
#[test]
fn c11_l115_build_deb_installs_systemd_unit() {
    let script = fs::read_to_string(repo_root().join("scripts/packaging/build_deb.sh"))
        .expect("read build_deb.sh");

    assert!(script.contains("sharecli.service"), "build_deb.sh must install sharecli.service unit");
    assert!(
        script.contains("/lib/systemd/system"),
        "build_deb.sh must place unit under /lib/systemd/system"
    );
}

/// FR-003 / C11 L115 — packaging CI asserts systemd unit inside `.deb`.
#[test]
fn c11_l115_packaging_soft_asserts_systemd_unit() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/packaging-soft.yml"))
        .expect("read packaging-soft.yml");

    assert!(
        workflow.contains("lib/systemd/system/sharecli.service"),
        "packaging-soft.yml must assert systemd unit in .deb contents"
    );
}

/// FR-003 / C11 L115 — deploy docs tie sample unit to packaged artifact path.
#[test]
fn c11_l115_systemd_doc_packaged_unit() {
    let doc = fs::read_to_string(repo_root().join("docs/deploy/systemd/sharecli.service.md"))
        .expect("read sharecli.service.md");

    assert!(doc.contains("sharecli_"), "systemd doc must reference packaged .deb artifact");
    assert!(doc.contains("dpkg-deb"), "systemd doc must document dpkg-deb inspection path");
}
