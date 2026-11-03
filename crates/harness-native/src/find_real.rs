// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Resolve real binary from .real cache or PATH scan.
//! Equivalent to harness::find_real in core.sh (FR-INTC-001).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Canonicalize path; returns fallback on failure (matches bash readlink -f fallback).
fn canonicalize_or(p: &Path, fallback: PathBuf) -> PathBuf {
    dunce::canonicalize(p).unwrap_or(fallback)
}

/// Find the real binary for `cmd`. First checks .real cache, then scans PATH
/// skipping proxy dir and anything that resolves to the harness dispatcher.
pub fn find_real(
    proxy_dir: &Path,
    _harness_home: &Path,
    harness_bin_path: Option<&Path>,
    cmd: &str,
) -> Option<PathBuf> {
    // 1. Check .real cache
    let cache_file = proxy_dir.join(format!(".{}.real", cmd));
    if cache_file.is_file() {
        if let Ok(content) = fs::read_to_string(&cache_file) {
            let path = content.trim();
            if !path.is_empty() {
                let p = PathBuf::from(path);
                if p.is_file() && is_executable(&p) {
                    return Some(p);
                }
            }
        }
    }

    // 2. Scan PATH, skipping proxy dir
    let path_var = env::var("PATH").unwrap_or_default();
    let proxy_real = canonicalize_or(proxy_dir, proxy_dir.to_path_buf());
    let harness_real = harness_bin_path
        .map(|p| canonicalize_or(p, p.to_path_buf()))
        .filter(|p| !p.as_os_str().is_empty());

    for dir in path_var.split(':') {
        let dir_path = Path::new(dir);
        let dir_real = canonicalize_or(dir_path, dir_path.to_path_buf());
        if dir_real == proxy_real {
            continue;
        }
        let candidate = dir_path.join(cmd);
        if candidate.is_file() && is_executable(&candidate) {
            let cand_real = canonicalize_or(&candidate, candidate.clone());
            if let Some(ref hr) = harness_real {
                if cand_real == *hr {
                    continue;
                }
            }
            return Some(candidate);
        }
    }
    None
}
