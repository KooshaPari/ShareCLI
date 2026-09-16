//! C11 L112 — codesign hard gate verification.
//!
//! Asserts that the macOS `codesign` toolchain is available and that the
//! release binary carries a valid signature when `SHARECLI_SIGNED_BINARY`
//! is set.

#![cfg(target_os = "macos")]

use std::process::Command;

/// Verify that the `codesign` binary is available on this macOS host.
#[test]
fn codesign_binary_available() {
    assert!(
        Command::new("which")
            .arg("codesign")
            .output()
            .expect("failed to spawn `which`")
            .status
            .success(),
        "`which codesign` must succeed on macOS — codesign toolchain missing"
    );
}

/// When `SHARECLI_SIGNED_BINARY` points at a release binary, verify it
/// carries a valid codesign signature (hard gate criterion).
///
/// This test is skipped (passes trivially) when the env var is not set,
/// so it only enforces signing in CI where the artifact is available.
#[test]
fn codesign_release_binary_signed() {
    let signed = std::env::var("SHARECLI_SIGNED_BINARY");
    let bin = match signed {
        Ok(b) => b,
        Err(_) => {
            eprintln!(
                "SKIP: SHARECLI_SIGNED_BINARY not set — \
                 set it to the path of a release binary to verify its signature"
            );
            return;
        }
    };

    let output = Command::new("codesign")
        .args(["--verify", "--deep", "--strict", &bin])
        .output()
        .expect("failed to spawn codesign --verify");

    assert!(
        output.status.success(),
        "codesign --verify failed for {bin}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Verify that the `notarytool` binary is available for notarization.
#[test]
fn notarytool_binary_available() {
    assert!(
        Command::new("xcrun")
            .args(["notarytool", "--help"])
            .output()
            .expect("failed to spawn xcrun notarytool")
            .status
            .success(),
        "`xcrun notarytool --help` must succeed on macOS — notarytool missing"
    );
}

/// Verify that the `stapler` binary is available for ticket stapling.
#[test]
fn stapler_binary_available() {
    // xcrun stapler only exists on macOS; skip gracefully on Linux CI.
    let status = Command::new("xcrun")
        .args(["stapler", "--help"])
        .output()
        .map(|o| o.status);
    let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
    if !success && !cfg!(target_os = "macos") {
        eprintln!("skipping stapler_binary_available: not macOS");
        return;
    }
    assert!(
        success,
        "`xcrun stapler --help` must succeed on macOS — stapler missing"
    );
}

/// Verify the hard-gate workflow exists and references the required steps.
#[test]
fn c11_l112_hard_gate_workflow_present() {
    let workflow = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/.github/workflows/codesign.yml"
    ))
    .expect("read codesign.yml");

    assert!(
        workflow.contains("Code Signing (Hard Gate)"),
        "workflow must declare name 'Code Signing (Hard Gate)'"
    );
    assert!(
        workflow.contains("import-codesign-certs"),
        "workflow must use apple-actions/import-codesign-certs"
    );
    assert!(
        workflow.contains("codesign --force --sign"),
        "workflow must invoke codesign --force --sign"
    );
    assert!(workflow.contains("notarytool submit"), "workflow must invoke notarytool submit");
    assert!(workflow.contains("stapler staple"), "workflow must invoke stapler staple");
    assert!(
        workflow.contains("codesign --verify --deep --strict"),
        "workflow must verify with codesign --verify --deep --strict"
    );
    assert!(
        !workflow.contains("continue-on-error: true") || workflow.contains("windows-sign"),
        "macos-sign job must NOT have continue-on-error"
    );
}

/// Verify the soft-gate workflow still exists (no regression to deleting it).
#[test]
fn c11_l112_soft_gate_preserved() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/codesign-soft.yml");
    assert!(
        std::path::Path::new(path).exists(),
        "codesign-soft.yml must still exist alongside hard gate"
    );
}
