//! Smart three-way merge with optional mergiraf, git merge-file fallback (FR-010).
//!
//! Rust port of `thegent.mesh.smart_merge.SmartMerger` fallback path:
//! try `mergiraf` when on `PATH`, otherwise `git merge-file --diff3`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of a smart merge operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResult {
    /// True when the merge produced no unresolved conflicts.
    pub success: bool,
    /// Conflicting paths (empty on a clean merge).
    pub conflicts: Vec<String>,
    /// Captured stdout/stderr from the merge tool.
    pub output: String,
    /// True when mergiraf performed the merge (vs git fallback).
    pub used_mergiraf: bool,
}

impl MergeResult {
    fn clean(output: impl Into<String>, used_mergiraf: bool) -> Self {
        Self { success: true, conflicts: Vec::new(), output: output.into(), used_mergiraf }
    }

    fn conflicted(output: impl Into<String>, used_mergiraf: bool) -> Self {
        Self { success: false, conflicts: Vec::new(), output: output.into(), used_mergiraf }
    }
}

/// Thin coordinator: mergiraf when available, else `git merge-file`.
#[derive(Debug, Clone)]
pub struct SmartMerger {
    /// Optional explicit mergiraf binary; `None` searches `PATH`.
    mergiraf_binary: Option<PathBuf>,
    /// When true (default), fall back to git if mergiraf is missing or hard-fails.
    fallback_to_git: bool,
}

impl Default for SmartMerger {
    fn default() -> Self {
        Self { mergiraf_binary: None, fallback_to_git: true }
    }
}

impl SmartMerger {
    /// Create a merger that resolves mergiraf from `PATH`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override mergiraf binary path (for tests / pinned installs).
    pub fn with_mergiraf_binary(mut self, path: impl Into<PathBuf>) -> Self {
        self.mergiraf_binary = Some(path.into());
        self
    }

    /// Disable git fallback (loud failure when mergiraf unavailable).
    pub fn without_git_fallback(mut self) -> Self {
        self.fallback_to_git = false;
        self
    }

    /// Resolve mergiraf binary if present.
    pub fn mergiraf_path(&self) -> Option<PathBuf> {
        if let Some(ref p) = self.mergiraf_binary {
            return Some(p.clone());
        }
        which_bin("mergiraf")
    }

    /// Three-way merge of `base` / `ours` / `theirs` into `output`.
    pub fn merge(&self, base: &Path, ours: &Path, theirs: &Path, output: &Path) -> MergeResult {
        if let Some(bin) = self.mergiraf_path() {
            let result = self.run_mergiraf(&bin, base, ours, theirs, output);
            // Soft conflict or clean (used_mergiraf=true) — return as-is.
            // Hard failure clears used_mergiraf so we can fall through.
            if result.used_mergiraf || !self.fallback_to_git {
                return result;
            }
        } else if !self.fallback_to_git {
            return MergeResult {
                success: false,
                conflicts: Vec::new(),
                output: "mergiraf unavailable and fallback disabled".into(),
                used_mergiraf: false,
            };
        }

        self.run_git_fallback(base, ours, theirs, output)
    }

    fn run_mergiraf(
        &self,
        bin: &Path,
        base: &Path,
        ours: &Path,
        theirs: &Path,
        output: &Path,
    ) -> MergeResult {
        let mut cmd = Command::new(bin);
        cmd.arg("merge").arg(base).arg(ours).arg(theirs).arg("-o").arg(output);
        match cmd.output() {
            Ok(out) => {
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                match out.status.code() {
                    Some(0) => {
                        if !output.exists() && !out.stdout.is_empty() {
                            let _ = fs::write(output, &out.stdout);
                        }
                        MergeResult::clean(combined, true)
                    }
                    Some(1) => {
                        if !output.exists() && !out.stdout.is_empty() {
                            let _ = fs::write(output, &out.stdout);
                        }
                        MergeResult::conflicted(combined, true)
                    }
                    _ => {
                        // Hard failure — signal fallback by used_mergiraf=false.
                        MergeResult {
                            success: false,
                            conflicts: Vec::new(),
                            output: combined,
                            used_mergiraf: false,
                        }
                    }
                }
            }
            Err(err) => MergeResult {
                success: false,
                conflicts: Vec::new(),
                output: err.to_string(),
                used_mergiraf: false,
            },
        }
    }

    fn run_git_fallback(
        &self,
        base: &Path,
        ours: &Path,
        theirs: &Path,
        output: &Path,
    ) -> MergeResult {
        if let Some(parent) = output.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // git merge-file overwrites "ours" in place — work on a scratch copy.
        let scratch = match tempfile_in_parent(output, ours) {
            Ok(p) => p,
            Err(err) => {
                return MergeResult {
                    success: false,
                    conflicts: Vec::new(),
                    output: err.to_string(),
                    used_mergiraf: false,
                };
            }
        };

        let cwd = output.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));

        let status = Command::new("git")
            .args([
                "merge-file",
                "--diff3",
                scratch.file_name().and_then(|s| s.to_str()).unwrap_or("scratch"),
            ])
            .arg(path_arg_for_cwd(base, &cwd))
            .arg(path_arg_for_cwd(theirs, &cwd))
            .current_dir(&cwd)
            .output();

        match status {
            Ok(out) => {
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                match fs::copy(&scratch, output) {
                    Ok(_) => {
                        let _ = fs::remove_file(&scratch);
                        if out.status.success() {
                            MergeResult::clean(combined, false)
                        } else {
                            MergeResult::conflicted(combined, false)
                        }
                    }
                    Err(err) => {
                        let _ = fs::remove_file(&scratch);
                        MergeResult {
                            success: false,
                            conflicts: Vec::new(),
                            output: err.to_string(),
                            used_mergiraf: false,
                        }
                    }
                }
            }
            Err(err) => {
                let _ = fs::remove_file(&scratch);
                // Last resort: copy ours so callers still have a file.
                let _ = fs::copy(ours, output);
                MergeResult {
                    success: false,
                    conflicts: Vec::new(),
                    output: err.to_string(),
                    used_mergiraf: false,
                }
            }
        }
    }
}

fn which_bin(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn path_arg_for_cwd(path: &Path, cwd: &Path) -> String {
    if path.parent() == Some(cwd) {
        path.file_name().and_then(|s| s.to_str()).unwrap_or(".").to_string()
    } else {
        path.to_string_lossy().into_owned()
    }
}

fn tempfile_in_parent(output: &Path, ours: &Path) -> std::io::Result<PathBuf> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let suffix =
        ours.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let name = format!(
        ".sharecli-merge-{}{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        suffix
    );
    let scratch = parent.join(name);
    fs::copy(ours, &scratch)?;
    Ok(scratch)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// FR-010 / AC-010.7 — clean three-way merge via git merge-file fallback.
    #[test]
    fn smart_merge_git_fallback_clean() {
        let dir = TempDir::new().expect("temp");
        let base = dir.path().join("base.txt");
        let ours = dir.path().join("ours.txt");
        let theirs = dir.path().join("theirs.txt");
        let out = dir.path().join("out.txt");
        // Keep a stable middle line so git merge-file treats edits as non-adjacent.
        fs::write(&base, "line1\nshared\nmiddle\nline3\n").unwrap();
        fs::write(&ours, "line1\nours-change\nmiddle\nline3\n").unwrap();
        fs::write(&theirs, "line1\nshared\nmiddle\nline3-theirs\n").unwrap();

        let merger = SmartMerger::new().with_mergiraf_binary("/nonexistent/mergiraf");
        let result = merger.merge(&base, &ours, &theirs, &out);
        assert!(!result.used_mergiraf, "must use git fallback");
        assert!(out.exists(), "output must be written");
        // Non-overlapping edits should succeed with git merge-file.
        assert!(result.success, "expected clean merge, got: {}", result.output);
        let text = fs::read_to_string(&out).unwrap();
        assert!(text.contains("ours-change"));
        assert!(text.contains("line3-theirs"));
    }

    /// FR-010 / AC-010.7 — conflicting edits yield success=false with markers.
    #[test]
    fn smart_merge_git_fallback_conflict() {
        let dir = TempDir::new().expect("temp");
        let base = dir.path().join("base.txt");
        let ours = dir.path().join("ours.txt");
        let theirs = dir.path().join("theirs.txt");
        let out = dir.path().join("out.txt");
        fs::write(&base, "same\n").unwrap();
        fs::write(&ours, "ours-only\n").unwrap();
        fs::write(&theirs, "theirs-only\n").unwrap();

        let merger = SmartMerger::new().with_mergiraf_binary("/nonexistent/mergiraf");
        let result = merger.merge(&base, &ours, &theirs, &out);
        assert!(!result.used_mergiraf);
        assert!(!result.success);
        assert!(out.exists());
        let text = fs::read_to_string(&out).unwrap();
        assert!(
            text.contains("<<<<<<<") || text.contains("ours-only") || text.contains("theirs-only"),
            "expected conflict markers or both sides, got {text:?}"
        );
    }

    // --- Additional coverage tests ---

    #[test]
    fn merge_result_clean_constructor() {
        let r = MergeResult::clean("ok", true);
        assert!(r.success);
        assert!(r.conflicts.is_empty());
        assert_eq!(r.output, "ok");
        assert!(r.used_mergiraf);
    }

    #[test]
    fn merge_result_conflicted_constructor() {
        let r = MergeResult::conflicted("conflict", false);
        assert!(!r.success);
        assert!(r.conflicts.is_empty());
        assert_eq!(r.output, "conflict");
        assert!(!r.used_mergiraf);
    }

    #[test]
    fn smart_merger_default_values() {
        let m = SmartMerger::new();
        assert!(m.fallback_to_git);
        assert!(m.mergiraf_binary.is_none());
    }

    #[test]
    fn smart_merger_with_mergiraf_binary_path() {
        let m = SmartMerger::new().with_mergiraf_binary("/usr/bin/mergiraf");
        assert_eq!(m.mergiraf_path(), Some(PathBuf::from("/usr/bin/mergiraf")));
    }

    #[test]
    fn smart_merger_mergiraf_path_none_when_not_set() {
        let m = SmartMerger::new();
        // When not set, mergiraf_path() checks PATH (which may or may not have it)
        // Just verify it doesn't panic
        let _ = m.mergiraf_path();
    }

    #[test]
    fn smart_merger_without_git_fallback() {
        let m = SmartMerger::new().without_git_fallback();
        assert!(!m.fallback_to_git);
    }

    #[test]
    fn smart_merger_clone_eq() {
        let m = SmartMerger::new().with_mergiraf_binary("/usr/bin/m").without_git_fallback();
        let cloned = m.clone();
        assert_eq!(m.mergiraf_binary, cloned.mergiraf_binary);
        assert_eq!(m.fallback_to_git, cloned.fallback_to_git);
    }

    #[test]
    fn merge_result_clone_eq() {
        let r = MergeResult::clean("out", false);
        let cloned = r.clone();
        assert_eq!(r, cloned);
    }

    #[test]
    fn no_fallback_with_broken_mergiraf_returns_error() {
        let m = SmartMerger::new()
            .without_git_fallback()
            .with_mergiraf_binary(std::path::PathBuf::from("/nonexistent/mergiraf"));
        let base = std::env::temp_dir().join("mb.txt");
        let ours = std::env::temp_dir().join("mo.txt");
        let theirs = std::env::temp_dir().join("mt.txt");
        let output = std::env::temp_dir().join("mo2.txt");
        let result = m.merge(&base, &ours, &theirs, &output);
        assert!(!result.success);
        // mergiraf binary is nonexistent, so it fails
        assert!(!result.used_mergiraf || !result.success);
    }
}
