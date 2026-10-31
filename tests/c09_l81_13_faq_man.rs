// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-004 NFR — C09 L81.13 help & documentation accessibility (FAQ + man page).
//! FR: FR-004

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn fr004_l81_13_faq_covers_top_five_questions() {
    let body = fs::read_to_string(repo_root().join("docs/faq.md")).expect("docs/faq.md");
    assert!(body.contains("Frequently Asked"), "faq must declare FAQ section");
    for heading in [
        "How do I install",
        "Where is configuration",
        "How do I start the dashboard",
        "How do I list, stop, or prune",
        "Shell completions and man page",
    ] {
        assert!(body.contains(heading), "faq must cover: {heading}");
    }
}

#[test]
fn fr004_l81_13_committed_man_page_present() {
    let path = repo_root().join("share/man/man1/sharecli.1");
    let body = fs::read_to_string(&path).expect("share/man/man1/sharecli.1");
    assert!(
        body.contains(".TH sharecli") || body.contains(".TH \"sharecli\""),
        "committed man page must be roff for sharecli(1)"
    );
    assert!(body.contains("serve"), "man page must document serve subcommand");
}

#[test]
fn fr004_l81_13_man_subcommand_matches_committed_page() {
    let out =
        Command::new(env!("CARGO_BIN_EXE_sharecli")).arg("man").output().expect("sharecli man");
    assert!(out.status.success(), "sharecli man: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let committed =
        fs::read_to_string(repo_root().join("share/man/man1/sharecli.1")).expect("committed man");
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        committed.replace("\r\n", "\n"),
        "stdout from `sharecli man` must match committed share/man/man1/sharecli.1"
    );
}

#[test]
fn fr004_l81_13_help_mentions_faq() {
    let out = Command::new(env!("CARGO_BIN_EXE_sharecli"))
        .arg("--help")
        .output()
        .expect("sharecli --help");
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("docs/faq.md"), "--help after_long_help must link docs/faq.md");
}

#[test]
fn fr004_l81_13_just_man_recipe_registered() {
    let body = fs::read_to_string(repo_root().join("justfile")).expect("justfile");
    assert!(body.contains("man:"), "justfile must expose `man` recipe");
    assert!(body.contains("share/man/man1/sharecli.1"), "just man must target man path");
}
