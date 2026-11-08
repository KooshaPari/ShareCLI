// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C00 L9 / FR-003 — SBOM embedded in release archives + CI emission gate.
//!
//! Evidence: `.github/workflows/release.yml`, `.github/workflows/sbom.yml`.

#[test]
fn c00_l9_release_workflow_embeds_sbom_in_archive() {
    let release = include_str!("../.github/workflows/release.yml");
    assert!(
        release.contains("cargo-cyclonedx") || release.contains("cargo cyclonedx"),
        "release.yml must install or invoke cargo-cyclonedx"
    );
    assert!(
        release.contains("sharecli.cdx.json"),
        "release.yml must reference sharecli.cdx.json in release stage"
    );
    assert!(
        release.contains("${STAGE}/sharecli.cdx.json"),
        "release.yml must copy SBOM into release archive staging dir"
    );
}

#[test]
fn c00_l9_sbom_workflow_emits_cyclonedx_on_main() {
    let sbom = include_str!("../.github/workflows/sbom.yml");
    assert!(sbom.contains("cargo cyclonedx"), "sbom.yml must run cargo cyclonedx");
    assert!(sbom.contains("spec-version 1.5"), "sbom.yml must emit CycloneDX 1.5");
    assert!(sbom.contains("sharecli.cdx.json"), "sbom.yml must upload sharecli.cdx.json artifact");
    assert!(
        sbom.contains("if-no-files-found: error"),
        "sbom.yml must hard-fail when SBOM artifact missing"
    );
}

#[test]
fn c00_l9_release_sets_source_date_epoch() {
    let release = include_str!("../.github/workflows/release.yml");
    assert!(
        release.contains("SOURCE_DATE_EPOCH"),
        "release.yml must set SOURCE_DATE_EPOCH for reproducible builds"
    );
}

#[test]
fn c00_l9_release_attestation_job_present() {
    let release = include_str!("../.github/workflows/release.yml");
    assert!(
        release.contains("attest-build-provenance"),
        "release.yml must attest build provenance (SLSA L2)"
    );
    assert!(
        release.contains("github-release"),
        "release.yml must attach unsigned assets to GitHub Release"
    );
}

#[test]
fn c00_l9_deploy_docs_reference_sbom_in_archive() {
    let deploy = include_str!("../docs/deploy.md");
    assert!(
        deploy.contains("sharecli.cdx.json"),
        "deploy.md must document SBOM in release artifacts"
    );
    assert!(deploy.contains("sbom.yml"), "deploy.md must cite sbom.yml CI workflow");
}
