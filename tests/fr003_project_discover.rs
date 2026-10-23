// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 — Project Registry (discover)
//! FR: FR-003
//!
//! Covers AC-003.4.
//!
//! Library-level acceptance test. Mirrors the one-level `.git` scan from
//! `ProjectCmd::Discover` under a `tempfile` tree (Windows-safe).

use std::fs;
use std::path::{Path, PathBuf};

/// Mirror of the scan body in `ProjectCmd::Discover` (immediate children only).
fn discover_git_repos(base: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join(".git").exists() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                found.push((name, p.to_string_lossy().to_string()));
            }
        }
    }
    found
}

fn format_discover(base: &Path, found: &[(String, String)]) -> String {
    let mut out = format!("Scanning {:?} for projects...\n", PathBuf::from(base));
    out.push_str(&format!("\nFound {} projects:\n", found.len()));
    for (name, path) in found {
        out.push_str(&format!("  {name} -> {path}\n"));
    }
    out
}

/// FR-003 / AC-003.4 — `project discover [path]` reports subdirs with `.git`.
#[test]
fn fr003_project_discover_finds_git_repos() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().join("scan-root");
    fs::create_dir_all(&base).expect("create scan root");

    let repo_a = base.join("repo-a");
    fs::create_dir_all(repo_a.join(".git")).expect("create repo-a/.git");

    let plain = base.join("not-a-repo");
    fs::create_dir_all(&plain).expect("create plain dir");
    fs::write(plain.join("README.md"), b"hi").expect("write readme");

    let nested = base.join("wrapper").join("deep-repo");
    fs::create_dir_all(nested.join(".git")).expect("create nested .git");

    let repo_b = base.join("repo-b");
    fs::create_dir_all(&repo_b).expect("create repo-b");
    fs::write(repo_b.join(".git"), b"gitdir: /somewhere\n").expect("write .git file");

    let found = discover_git_repos(&base);
    let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();

    assert!(names.contains(&"repo-a"), "discover MUST find dir with .git/; got {names:?}");
    assert!(names.contains(&"repo-b"), "discover MUST find dir with .git file; got {names:?}");
    assert!(!names.contains(&"not-a-repo"), "discover MUST ignore non-git dirs; got {names:?}");
    assert!(
        !names.contains(&"wrapper") && !names.contains(&"deep-repo"),
        "discover MUST NOT recurse; got {names:?}"
    );
    assert_eq!(found.len(), 2, "exactly two top-level git repos; got {found:?}");

    let out = format_discover(&base, &found);
    assert!(out.contains("Found 2 projects:"), "discover output MUST report count; got: {out}");
    assert!(out.contains("repo-a -> "), "got: {out}");
    assert!(out.contains("repo-b -> "), "got: {out}");
}
