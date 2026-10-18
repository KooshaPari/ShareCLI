// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-009 — fuse CLI provenance inspect (operator surface)
//! FR: FR-009
//!
//! AC-009.11 CLI `fuse provenance <path>` reads write xattrs via read_provenance
//! AC-009.17 CLI fuse mount/unmount/status/list/commit/discard operator surface
//! AC-009.21 CLI loud-rejects invalid `--agent` / missing `--agents-conf`; help documents Feb flags
//!
//! GATED: All tests in this file require Linux/macOS (FUSE kernel support).
//! On Windows these tests are skipped via `cfg` to keep `--workspace` measurement unblocked.

#![cfg(not(target_os = "windows"))]

use std::process::Command;

use sharecli_fuse::{annotate_write_at, read_provenance};
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// FR-009 / AC-009.11 — `sharecli fuse provenance --json` mirrors read_provenance.
#[test]
fn fr009_cli_fuse_provenance_json() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("tracked.txt");
    std::fs::write(&path, b"payload").expect("write");
    annotate_write_at(&path, "cli-session-7", 1_750_000_000).expect("stamp");

    let out = bin()
        .args(["fuse", "provenance"])
        .arg(&path)
        .arg("--json")
        .output()
        .expect("spawn fuse provenance");
    assert!(out.status.success(), "fuse provenance MUST exit 0; stderr={}", stderr(&out));
    let body = stdout(&out);
    let v: serde_json::Value = serde_json::from_str(&body).expect("json provenance");
    assert_eq!(v["session_id"], "cli-session-7");
    assert_eq!(v["written_at_unix"], 1_750_000_000);
}

/// FR-009 / AC-009.11 — absent xattrs emit JSON null.
#[test]
fn fr009_cli_fuse_provenance_json_null() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("plain.txt");
    std::fs::write(&path, b"plain").expect("write");

    let out = bin()
        .args(["fuse", "provenance"])
        .arg(&path)
        .arg("--json")
        .output()
        .expect("spawn fuse provenance");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "null");
}

/// FR-009 / AC-009.11 — missing path fails loudly.
#[test]
fn fr009_cli_fuse_provenance_missing_path() {
    let out = bin()
        .args(["fuse", "provenance", "/no/such/sharecli-fuse-file"])
        .output()
        .expect("spawn fuse provenance");
    assert!(!out.status.success(), "missing path MUST fail");
    assert!(stderr(&out).contains("does not exist"), "stderr={}", stderr(&out));
}

/// FR-009 / AC-009.11 — library round-trip matches CLI JSON (no mount).
#[test]
fn fr009_cli_fuse_provenance_matches_read_provenance() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("lib.txt");
    std::fs::write(&path, b"x").expect("write");
    annotate_write_at(&path, "lib-session", 42).expect("stamp");
    let lib = read_provenance(&path).expect("read").expect("present");

    let out = bin().args(["fuse", "provenance"]).arg(&path).arg("--json").output().expect("spawn");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json");
    assert_eq!(v["session_id"], lib.session_id);
    assert_eq!(v["written_at_unix"], lib.written_at_unix);
}

/// FR-009 / AC-009.17 — `fuse status --json` exposes read-cache + write-serialize meters.
#[test]
fn fr009_cli_fuse_status_json() {
    let out = bin().args(["fuse", "status", "--json"]).output().expect("spawn fuse status");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json status");
    assert!(v.get("read_cache").is_some(), "must include read_cache");
    assert!(v.get("write_serialize").is_some(), "must include write_serialize");
}

/// FR-009 / AC-009.17 — `fuse list` with no mounts returns success.
#[test]
fn fr009_cli_fuse_list_empty() {
    let out = bin().args(["fuse", "list"]).output().expect("spawn fuse list");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert!(stdout(&out).contains("no registered mounts"), "stdout={}", stdout(&out));
}

/// FR-009 / AC-009.17 — commit without a registered mount fails loudly.
#[test]
fn fr009_cli_fuse_commit_requires_mount() {
    let out = bin().args(["fuse", "commit", "some/path.txt"]).output().expect("spawn fuse commit");
    assert!(!out.status.success(), "commit without mount MUST fail");
    assert!(
        stderr(&out).contains("no active FUSE mounts") || stderr(&out).contains("registered mount"),
        "stderr={}",
        stderr(&out)
    );
}

/// FR-009 / AC-009.21 — `fuse mount --agent` with path separators fails before mount.
#[test]
fn fr009_cli_fuse_mount_rejects_invalid_agent() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("backing");
    let mountpoint = dir.path().join("mnt");
    std::fs::create_dir_all(&backing).expect("backing");
    std::fs::create_dir_all(&mountpoint).expect("mnt");

    let out = bin()
        .args([
            "fuse",
            "mount",
            backing.to_str().unwrap(),
            mountpoint.to_str().unwrap(),
            "--agent",
            "bad/id",
        ])
        .output()
        .expect("spawn fuse mount");
    assert!(!out.status.success(), "invalid --agent MUST fail (AC-009.21)");
    assert!(
        stderr(&out).contains("invalid --agent") || stderr(&out).contains("alnum"),
        "stderr={}",
        stderr(&out)
    );
}

/// FR-009 / AC-009.21 — missing `--agents-conf` path fails loudly before mount.
#[test]
fn fr009_cli_fuse_mount_rejects_missing_agents_conf() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("backing");
    let mountpoint = dir.path().join("mnt");
    let missing = dir.path().join("no-such-agents.conf");
    std::fs::create_dir_all(&backing).expect("backing");
    std::fs::create_dir_all(&mountpoint).expect("mnt");

    let out = bin()
        .args([
            "fuse",
            "mount",
            backing.to_str().unwrap(),
            mountpoint.to_str().unwrap(),
            "--agents-conf",
            missing.to_str().unwrap(),
        ])
        .output()
        .expect("spawn fuse mount");
    assert!(
        !out.status.success(),
        "missing agents.conf MUST fail (AC-009.21); stderr={}",
        stderr(&out)
    );
}

/// FR-009 / AC-009.21 — `fuse mount --help` documents Feb `--cow` / `--no-serialize` / `--agents-conf`.
#[test]
fn fr009_cli_fuse_mount_help_documents_feb_flags() {
    let out = bin().args(["fuse", "mount", "--help"]).output().expect("spawn fuse mount --help");
    assert!(out.status.success(), "help MUST exit 0");
    let text = format!("{}{}", stdout(&out), stderr(&out));
    for flag in ["--cow", "--no-serialize", "--agents-conf", "--agent", "--cow-dir"] {
        assert!(
            text.contains(flag),
            "fuse mount --help MUST document {flag} (AC-009.21); got: {text}"
        );
    }
}
