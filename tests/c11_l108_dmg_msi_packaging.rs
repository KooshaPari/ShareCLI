// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C11 L108 — unsigned dmg/msi soft scaffolds (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C11 L108 — WiX soft source stages per-user unsigned MSI product.
#[test]
fn c11_l108_wix_soft_product_present() {
    let wxs = fs::read_to_string(repo_root().join("scripts/packaging/wix/sharecli.wxs"))
        .expect("read sharecli.wxs");

    assert!(wxs.contains("InstallScope=\"perUser\""), "WiX must declare perUser InstallScope");
    assert!(wxs.contains("sharecli.exe"), "WiX must reference sharecli.exe payload");
    assert!(
        wxs.contains("unsigned soft MSI") || wxs.contains("L112"),
        "WiX soft scaffold must document unsigned / L112 deferral"
    );
}

/// FR-003 / C11 L108 — DMG layout script produces .app Contents tree.
#[test]
fn c11_l108_dmg_layout_script_stages_app_bundle() {
    let script = fs::read_to_string(repo_root().join("scripts/packaging/build_dmg_layout.sh"))
        .expect("read build_dmg_layout.sh");

    assert!(script.contains("Contents/MacOS"), "dmg layout must stage Contents/MacOS");
    assert!(script.contains("Info.plist"), "dmg layout must write Info.plist");
    assert!(script.contains("UNSIGNED_SOFT.txt"), "dmg layout must mark unsigned soft");
    assert!(
        !script.contains("codesign") && !script.contains("notarytool"),
        "dmg soft layout must not invoke codesign/notarytool (L112 blocked)"
    );
}

/// FR-003 / C11 L108 — MSI layout script stages WiX + payload without Authenticode.
#[test]
fn c11_l108_msi_layout_script_stages_wix() {
    let script = fs::read_to_string(repo_root().join("scripts/packaging/build_msi_layout.sh"))
        .expect("read build_msi_layout.sh");

    assert!(script.contains("sharecli.wxs"), "msi layout must install WiX source");
    assert!(script.contains("payload/sharecli.exe"), "msi layout must stage payload exe");
    assert!(script.contains("UNSIGNED_SOFT.txt"), "msi layout must mark unsigned soft");
    assert!(
        !script.to_lowercase().contains("signtool"),
        "msi soft layout must not invoke signtool (L112 blocked)"
    );
}

/// FR-003 / C11 L108 — packaging-soft CI runs dmg/msi soft assert job.
#[test]
fn c11_l108_packaging_soft_asserts_dmg_msi() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/packaging-soft.yml"))
        .expect("read packaging-soft.yml");

    assert!(
        workflow.contains("assert_dmg_msi_soft.sh"),
        "packaging-soft.yml must run assert_dmg_msi_soft.sh"
    );
    assert!(
        workflow.contains("dmg-msi-soft") || workflow.contains("unsigned dmg/msi"),
        "packaging-soft.yml must name a dmg/msi soft job"
    );
}

/// FR-003 / C11 L108 — soft assert script succeeds with stub binaries.
#[test]
fn c11_l108_assert_dmg_msi_soft_script_runs() {
    let status = Command::new("bash")
        .arg(repo_root().join("scripts/packaging/assert_dmg_msi_soft.sh"))
        .current_dir(repo_root())
        .status()
        .expect("spawn assert_dmg_msi_soft.sh");

    assert!(status.success(), "assert_dmg_msi_soft.sh must exit 0");
}

/// FR-003 / C11 L108 — ops doc covers phase 3.5 soft layouts.
#[test]
fn c11_l108_dmg_msi_doc_covers_soft_layouts() {
    let doc = fs::read_to_string(repo_root().join("docs/ops/dmg-msi-packaging.md"))
        .expect("read dmg-msi-packaging.md");

    assert!(doc.contains("build_dmg_layout.sh"), "doc must reference dmg layout script");
    assert!(doc.contains("build_msi_layout.sh"), "doc must reference msi layout script");
    assert!(doc.contains("L112"), "doc must cross-ref L112 codesign block");
}
