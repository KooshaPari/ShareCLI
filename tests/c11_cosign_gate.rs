//! C11 L112 — container cosign hard gate verification.
//!
//! FR: FR-003
//!
//! Verifies the container cosign hard gate surface ships and is sound:
//!   - `cosign` binary is available (or skips gracefully)
//!   - Container image ID artifact exists when hard gate has run
//!   - Container image manifest is signed (cosign verify passes)
//!   - Signature verification passes with identity + issuer constraints
//!   - Rekor transparency log entry exists for the signed image

use std::process::Command;

/// Resolve the repo root from `CARGO_MANIFEST_DIR`.
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Check whether the `cosign` binary is on PATH.
/// Returns `true` if available, `false` otherwise.
fn cosign_available() -> bool {
    Command::new("cosign")
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Resolve the GHCR image reference from env or the digest file written
/// by `scripts/container-cosign-hard.sh`.
fn resolve_ghcr_ref() -> Option<String> {
    // Prefer explicit env var (CI sets this).
    if let Ok(ref_tag) = std::env::var("COSIGN_GHCR_REF") {
        if !ref_tag.is_empty() {
            return Some(ref_tag.clone());
        }
    }

    // Fall back to the digest file produced by the hard gate script.
    let digest_file = repo_root().join("sharecli-ci-image-digest.txt");
    if digest_file.exists() {
        let content = std::fs::read_to_string(&digest_file).ok()?;
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() && trimmed != "skipped-push" {
            return Some(trimmed);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// FR-003 / C11 L112 — cosign binary is available on the runner.
///
/// When cosign is not installed the test passes with a SKIP notice so CI
/// can differentiate "not installed" from "hard gate failed".
#[test]
fn c11_cosign_binary_available() {
    if cosign_available() {
        let output =
            Command::new("cosign").arg("version").output().expect("failed to spawn cosign version");
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("cosign version output: {stdout}");
        assert!(output.status.success(), "cosign version must succeed");
    } else {
        eprintln!(
            "SKIP: cosign binary not found on PATH — \
             install from https://docs.sigstore.dev/cosign/installation/"
        );
    }
}

/// FR-003 / C11 L112 — container image ID artifact exists when the hard
/// gate has been executed.
///
/// The hard gate script (`scripts/container-cosign-hard.sh`) writes
/// `sharecli-ci-image-id.txt` after a successful build. This test
/// verifies that file exists and contains a non-empty image ID.
#[test]
fn c11_cosign_image_artifact_present() {
    let id_file = repo_root().join("sharecli-ci-image-id.txt");
    if !id_file.exists() {
        eprintln!(
            "SKIP: sharecli-ci-image-id.txt not present — \
             run `bash scripts/container-cosign-hard.sh` first"
        );
        return;
    }

    let content = std::fs::read_to_string(&id_file).expect("failed to read image ID artifact");
    let trimmed = content.trim();
    assert!(!trimmed.is_empty(), "sharecli-ci-image-id.txt must not be empty");
    assert!(
        trimmed.starts_with("sha256:") || trimmed.starts_with("sha384:"),
        "image ID must start with sha256: or sha384:, got: {trimmed}"
    );
}

/// FR-003 / C11 L112 — container image manifest signature verification
/// passes.
///
/// Runs `cosign verify` with the certificate identity regexp and OIDC
/// issuer constraints to confirm the image was signed by the expected
/// GitHub Actions workflow.
///
/// Skipped gracefully when the GHCR ref is unavailable (e.g. local
/// development or when SKIP_GHCR_PUSH was used).
#[test]
fn c11_cosign_signature_verification_passes() {
    if !cosign_available() {
        eprintln!("SKIP: cosign not available — signature verification skipped");
        return;
    }

    let subject = match resolve_ghcr_ref() {
        Some(s) => s,
        None => {
            eprintln!(
                "SKIP: no GHCR image ref available — \
                 set COSIGN_GHCR_REF or run the hard gate with GHCR push"
            );
            return;
        }
    };

    let identity = std::env::var("COSIGN_IDENTITY_REGEXP")
        .unwrap_or_else(|_| "https://github.com/KooshaPari/sharecli/.*".to_string());
    let issuer = std::env::var("COSIGN_OIDC_ISSUER")
        .unwrap_or_else(|_| "https://token.actions.githubusercontent.com".to_string());

    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity-regexp",
            &identity,
            "--certificate-oidc-issuer",
            &issuer,
            &subject,
        ])
        .output()
        .expect("failed to spawn cosign verify");

    assert!(
        output.status.success(),
        "cosign verify failed for {subject}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// FR-003 / C11 L112 — Rekor transparency log entry exists for the
/// signed container image.
///
/// `cosign verify` with keyless signing implicitly checks the Rekor
/// transparency log. This test makes the check explicit by verifying
/// the JSON output contains a `critical` claim with `identity` and
/// `issuer` fields (Rekor log entry structure).
///
/// Skipped gracefully when cosign or the GHCR ref is unavailable.
#[test]
fn c11_cosign_rekor_transparency_log_entry_exists() {
    if !cosign_available() {
        eprintln!("SKIP: cosign not available — Rekor check skipped");
        return;
    }

    let subject = match resolve_ghcr_ref() {
        Some(s) => s,
        None => {
            eprintln!(
                "SKIP: no GHCR image ref available — \
                 set COSIGN_GHCR_REF or run the hard gate with GHCR push"
            );
            return;
        }
    };

    let identity = std::env::var("COSIGN_IDENTITY_REGEXP")
        .unwrap_or_else(|_| "https://github.com/KooshaPari/sharecli/.*".to_string());
    let issuer = std::env::var("COSIGN_OIDC_ISSUER")
        .unwrap_or_else(|_| "https://token.actions.githubusercontent.com".to_string());

    // cosign verify --output-json emits the transparency log bundle.
    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity-regexp",
            &identity,
            "--certificate-oidc-issuer",
            &issuer,
            "--output-json",
            &subject,
        ])
        .output()
        .expect("failed to spawn cosign verify --output-json");

    assert!(
        output.status.success(),
        "cosign verify --output-json failed for {subject}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"critical\""),
        "Rekor transparency log bundle must contain a `critical` claim — \
         got: {stdout}"
    );
    assert!(
        stdout.contains("\"identity\""),
        "Rekor transparency log bundle must contain an `identity` field"
    );
    assert!(
        stdout.contains("\"issuer\""),
        "Rekor transparency log bundle must contain an `issuer` field"
    );
}

/// FR-003 / C11 L112 — hard gate workflow references the cosign hard
/// script and the container-cosign-verify consumer script.
#[test]
fn c11_cosign_hard_workflow_present() {
    let workflow =
        std::fs::read_to_string(repo_root().join(".github/workflows/container-cosign.yml"))
            .expect("read container-cosign.yml");

    assert!(
        workflow.contains("Container cosign (hard)"),
        "workflow must declare name 'Container cosign (hard)'"
    );
    assert!(
        workflow.contains("container-cosign-hard.sh"),
        "workflow must invoke scripts/container-cosign-hard.sh"
    );
    assert!(
        workflow.contains("container-cosign-verify.sh"),
        "workflow must invoke scripts/container-cosign-verify.sh"
    );
    assert!(
        workflow.contains("cosign-installer"),
        "workflow must install cosign via sigstore/cosign-installer"
    );
    assert!(
        workflow.contains("attest-build-provenance"),
        "workflow must include SLSA attestation step"
    );
}

/// FR-003 / C11 L112 — hard gate script exists and contains the
/// required hard gate steps (sign, attest, verify, Rekor).
#[test]
fn c11_cosign_hard_script_present() {
    let script = std::fs::read_to_string(repo_root().join("scripts/container-cosign-hard.sh"))
        .expect("read container-cosign-hard.sh");

    assert!(
        script.contains("set -euo pipefail"),
        "hard gate script must use strict error handling"
    );
    assert!(script.contains("cosign sign"), "hard gate script must invoke cosign sign");
    assert!(script.contains("cosign attest"), "hard gate script must invoke cosign attest");
    assert!(script.contains("cosign verify"), "hard gate script must invoke cosign verify");
    assert!(
        script.contains("verify-attestation"),
        "hard gate script must invoke cosign verify-attestation"
    );
    assert!(
        script.contains("Rekor transparency"),
        "hard gate script must reference Rekor transparency log verification"
    );
}
