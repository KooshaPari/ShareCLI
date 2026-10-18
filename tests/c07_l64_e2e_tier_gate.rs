// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C07 L64 — e2e/chaos test pyramid tier (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C07 L64 — dedicated e2e integration tests exist with `_e2e_` naming.
#[test]
fn c07_l64_e2e_test_files_present() {
    let tests = repo_root().join("tests");
    for file in ["e2e_serve_healthz.rs", "e2e_chaos_recovery.rs"] {
        assert!(tests.join(file).is_file(), "tests/{file} must exist for e2e tier");
    }
}

/// FR-003 / C07 L64 — nextest e2e profile + `_e2e_` override wired.
#[test]
fn c07_l64_nextest_e2e_profile_wired() {
    let cfg =
        fs::read_to_string(repo_root().join(".config/nextest.toml")).expect("read nextest.toml");
    assert!(cfg.contains("[profile.e2e]"), "nextest must define [profile.e2e]");
    assert!(cfg.contains("test(/_e2e_/)"), "nextest ci overrides must target _e2e_ tests");
}

/// FR-003 / C07 L64 — `just test-e2e` recipe runs e2e profile.
#[test]
fn c07_l64_just_test_e2e_recipe() {
    let justfile = fs::read_to_string(repo_root().join("justfile")).expect("read justfile");
    assert!(justfile.contains("test-e2e:"), "justfile must define test-e2e recipe");
    assert!(justfile.contains("--profile e2e"), "test-e2e must use nextest e2e profile");
    assert!(justfile.contains("_e2e_"), "test-e2e must filter _e2e_ tests");
}

/// FR-003 / C07 L64 — e2e tier contract documented.
#[test]
fn c07_l64_e2e_tier_doc_present() {
    let doc =
        fs::read_to_string(repo_root().join("docs/testing/e2e-tier.md")).expect("read e2e-tier.md");
    assert!(doc.contains("e2e_serve_healthz"), "doc must cite healthz e2e");
    assert!(doc.contains("e2e_chaos_recovery"), "doc must cite chaos e2e");
    assert!(doc.contains("just test-e2e"), "doc must cite just test-e2e");
}
