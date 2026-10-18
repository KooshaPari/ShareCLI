// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Semantic argv normalization (Feb harness `harness::semantic::normalize`).
//!
//! When enabled, lint-tool path arguments collapse so equivalent invocations share
//! a coalesce cache entry (e.g. `ruff check .` vs `ruff check src/` when `src/`
//! is the only Python tree — path tokens normalize to canonical absolutes and
//! `.` becomes `__PROJECT_ROOT__`).

use std::path::Path;

/// Normalize `argv` for semantic cache-key comparison.
///
/// Port of Feb `core.sh` `harness::semantic::normalize`: only the basename of
/// `argv[0]` is inspected; remaining tokens are path-normalized for known lint
/// tools.
pub fn semantic_normalize_argv(argv: &[String], cwd: &Path) -> Vec<String> {
    if argv.is_empty() {
        return Vec::new();
    }

    let cmd = Path::new(&argv[0]).file_name().and_then(|s| s.to_str()).unwrap_or(argv[0].as_str());

    match cmd {
        "ruff" | "mypy" | "pylint" | "flake8" => {
            let mut out = vec![argv[0].clone()];
            for arg in &argv[1..] {
                out.push(normalize_lint_path_arg(arg, cwd));
            }
            out
        }
        "git" => argv.to_vec(),
        _ => argv.to_vec(),
    }
}

fn normalize_lint_path_arg(arg: &str, cwd: &Path) -> String {
    if arg == "." {
        return "__PROJECT_ROOT__".to_string();
    }
    let candidate = cwd.join(arg);
    if candidate.is_dir() {
        return canonical_dir(&candidate).unwrap_or_else(|| arg.to_string());
    }
    arg.to_string()
}

fn canonical_dir(path: &Path) -> Option<String> {
    std::fs::canonicalize(path).ok().map(|p| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// FR-008 / AC-008.20 — semantic normalize maps `.` to project-root token.
    #[test]
    fn semantic_normalize_dot_to_project_root() {
        let dir = TempDir::new().expect("tempdir");
        let argv = vec!["ruff".into(), "check".into(), ".".into()];
        let norm = semantic_normalize_argv(&argv, dir.path());
        assert_eq!(norm[2], "__PROJECT_ROOT__");
    }

    /// FR-008 / AC-008.20 — semantic normalize canonicalizes directory paths.
    #[test]
    fn semantic_normalize_canonicalizes_directory() {
        let dir = TempDir::new().expect("tempdir");
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).expect("mkdir src");
        let canonical = std::fs::canonicalize(&sub).expect("canonicalize");

        let argv = vec!["mypy".into(), sub.file_name().unwrap().to_string_lossy().into()];
        let norm = semantic_normalize_argv(&argv, dir.path());
        assert_eq!(norm[1], canonical.to_string_lossy());
    }

    /// FR-008 / AC-008.20 — unknown commands pass argv through unchanged.
    #[test]
    fn semantic_normalize_passthrough_unknown_cmd() {
        let argv = vec!["cargo".into(), "check".into()];
        let norm = semantic_normalize_argv(&argv, Path::new("/tmp"));
        assert_eq!(norm, argv);
    }
}
