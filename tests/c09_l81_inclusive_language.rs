// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-004 NFR — C09 L81.10 inclusive language + golden help.
//! FR: FR-004

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn fr004_l81_10_vale_and_style_guide_present() {
    let root = repo_root();
    let vale_ini = fs::read_to_string(root.join(".vale.ini")).expect(".vale.ini");
    assert!(
        vale_ini.contains("Microsoft"),
        ".vale.ini must enable Microsoft inclusive-language rules"
    );
    let style = fs::read_to_string(root.join("docs/style-guide.md")).expect("style-guide");
    assert!(style.contains("Inclusive"), "style-guide must document inclusive language");
    assert!(style.contains("Vale"), "style-guide must reference Vale automation");
}

#[test]
fn fr004_l81_10_help_golden_present() {
    let help = repo_root().join("tests/golden/help.txt");
    let cli_help = repo_root().join("tests/golden/cli_help.txt");
    let help_body = fs::read_to_string(&help).expect("tests/golden/help.txt");
    let cli_body = fs::read_to_string(&cli_help).expect("tests/golden/cli_help.txt");
    assert_eq!(help_body, cli_body, "help.txt must mirror cli_help.txt (canonical --help golden)");
    assert!(help_body.contains("Usage: sharecli"));
}

#[test]
fn fr004_l81_10_no_ableist_user_visible_strings() {
    let root = repo_root();
    let banned = ["blindly", "cripple", "idiot", "stupid"];
    for rel in ["src", "crates"] {
        scan_tree_for_banned(&root.join(rel), &banned);
    }
}

fn scan_tree_for_banned(dir: &Path, banned: &[&str]) {
    if !dir.is_dir() {
        return;
    }
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "bip39_wordlist.rs" {
                continue;
            }
            let body = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            for word in banned {
                if body.to_ascii_lowercase().contains(word) {
                    panic!("banned term {word:?} in {}", path.display());
                }
            }
        } else if path.is_dir() {
            scan_tree_for_banned(&path, banned);
        }
    }
}

#[test]
fn fr004_l81_10_vale_script_registered() {
    let script = repo_root().join("scripts/lint/vale.sh");
    assert!(script.is_file(), "scripts/lint/vale.sh must exist");
    let body = fs::read_to_string(&script).expect("read vale.sh");
    assert!(body.contains("vale"), "vale.sh must invoke vale");
}

/// Smoke: run Vale when installed (skipped in minimal CI agents without vale binary).
#[test]
fn fr004_l81_10_vale_smoke_when_installed() {
    if Command::new("vale").arg("--version").output().is_err() {
        eprintln!("skip: vale not installed");
        return;
    }
    let out = Command::new("bash")
        .arg(repo_root().join("scripts/lint/vale.sh"))
        .output()
        .expect("vale.sh");
    assert!(out.status.success(), "vale: {}", String::from_utf8_lossy(&out.stderr));
}
