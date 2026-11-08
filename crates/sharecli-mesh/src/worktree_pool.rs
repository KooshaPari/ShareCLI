// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Lightweight git worktree pool for parallel agent checkouts (FR-010).
//!
//! Allocates and releases worktrees under a pool root via
//! `git worktree add` / `git worktree remove`. Non-git roots fail loudly.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// Git variables that bind a subprocess to the caller's repository.
///
/// Git exports these to hooks. Every command in this module intentionally
/// operates on an explicit foreign checkout, so inheriting them would let a
/// hook test inspect or mutate the hook's repository instead.
const GIT_LOCAL_ENV_VARS: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    // `git rev-parse --local-env-vars` does not report this ref namespace,
    // but foreign worktree operations must never inherit the caller's scope.
    "GIT_NAMESPACE",
];

/// Error from worktree pool operations.
#[derive(Debug, thiserror::Error)]
pub enum WorktreePoolError {
    /// Underlying IO failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Path is not a git repository.
    #[error("worktree pool: not a git repository: {0}")]
    NotGitRepo(PathBuf),
    /// `git` subprocess failed.
    #[error("worktree pool: git failed: {0}")]
    Git(String),
    /// Slot was never allocated (or already released).
    #[error("worktree pool: no allocation for slot {0}")]
    NoAllocation(String),
    /// Internal lock poisoning.
    #[error("worktree pool lock poisoned")]
    Poisoned,
}

/// Allocated worktree slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeLease {
    /// Caller-provided slot id (agent id).
    pub slot_id: String,
    /// Absolute path of the worktree checkout.
    pub path: PathBuf,
    /// Branch name checked out in the worktree.
    pub branch: String,
}

/// Pool of git worktrees under `pool_root`, rooted at `repo_root`.
#[derive(Debug)]
pub struct WorktreePool {
    repo_root: PathBuf,
    pool_root: PathBuf,
    /// Optional base ref for new branches (default: HEAD).
    base_ref: String,
    leases: Mutex<HashMap<String, WorktreeLease>>,
}

impl WorktreePool {
    /// Open a pool for `repo_root`, placing worktrees under `pool_root`.
    ///
    /// Fails loudly when `repo_root` is not a git repository.
    pub fn open(
        repo_root: impl Into<PathBuf>,
        pool_root: impl Into<PathBuf>,
    ) -> Result<Self, WorktreePoolError> {
        let repo_root = repo_root.into();
        let pool_root = pool_root.into();
        ensure_git_repo(&repo_root)?;
        fs::create_dir_all(&pool_root)?;
        Ok(Self {
            repo_root,
            pool_root,
            base_ref: "HEAD".into(),
            leases: Mutex::new(HashMap::new()),
        })
    }

    /// Override the base ref used when creating slot branches.
    pub fn with_base_ref(mut self, base_ref: impl Into<String>) -> Self {
        self.base_ref = base_ref.into();
        self
    }

    /// Repository root.
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Pool directory root.
    pub fn pool_root(&self) -> &Path {
        &self.pool_root
    }

    /// Allocate (or return existing) worktree for `slot_id`.
    pub fn allocate(&self, slot_id: &str) -> Result<WorktreeLease, WorktreePoolError> {
        let mut leases = self.leases.lock().map_err(|_| WorktreePoolError::Poisoned)?;
        if let Some(existing) = leases.get(slot_id) {
            if existing.path.is_dir() {
                return Ok(existing.clone());
            }
            leases.remove(slot_id);
        }

        let branch = format!("sharecli/pool/{slot_id}");
        let path = self.pool_root.join(slot_id);

        if path.exists() {
            // Stale path from a prior crash — force-remove before re-add.
            let _ =
                git(&self.repo_root, &["worktree", "remove", "--force", &path.to_string_lossy()]);
            if path.exists() {
                fs::remove_dir_all(&path)?;
            }
        }

        // Ensure branch exists from base_ref.
        let list = git(&self.repo_root, &["branch", "--list", &branch])?;
        if list.trim().is_empty() {
            git(&self.repo_root, &["branch", &branch, &self.base_ref])?;
        }

        git(&self.repo_root, &["worktree", "add", &path.to_string_lossy(), &branch])?;

        let lease = WorktreeLease { slot_id: slot_id.to_string(), path, branch };
        leases.insert(slot_id.to_string(), lease.clone());
        Ok(lease)
    }

    /// Release a previously allocated slot (`git worktree remove --force`).
    pub fn release(&self, slot_id: &str) -> Result<(), WorktreePoolError> {
        let mut leases = self.leases.lock().map_err(|_| WorktreePoolError::Poisoned)?;
        let lease = leases
            .remove(slot_id)
            .ok_or_else(|| WorktreePoolError::NoAllocation(slot_id.to_string()))?;

        let path_str = lease.path.to_string_lossy().into_owned();
        let remove = git(&self.repo_root, &["worktree", "remove", "--force", &path_str]);
        if remove.is_err() && lease.path.exists() {
            fs::remove_dir_all(&lease.path)?;
            let _ = git(&self.repo_root, &["worktree", "prune"]);
        } else {
            remove?;
        }

        // Drop the ephemeral branch (best-effort).
        let _ = git(&self.repo_root, &["branch", "-D", &lease.branch]);
        Ok(())
    }

    /// Currently tracked slot ids.
    pub fn active_slots(&self) -> Result<Vec<String>, WorktreePoolError> {
        let leases = self.leases.lock().map_err(|_| WorktreePoolError::Poisoned)?;
        Ok(leases.keys().cloned().collect())
    }
}

fn ensure_git_repo(path: &Path) -> Result<(), WorktreePoolError> {
    let status = git_command(path)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map_err(|e| WorktreePoolError::Git(e.to_string()))?;
    if status.status.success() {
        Ok(())
    } else {
        Err(WorktreePoolError::NotGitRepo(path.to_path_buf()))
    }
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, WorktreePoolError> {
    let out =
        git_command(cwd).args(args).output().map_err(|e| WorktreePoolError::Git(e.to_string()))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let msg = format!(
            "git {} => {}: {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Err(WorktreePoolError::Git(msg))
    }
}

fn git_command(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(cwd);
    for variable in GIT_LOCAL_ENV_VARS {
        command.env_remove(variable);
    }
    command
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn init_repo(dir: &Path) {
        git(dir, &["init"]).expect("init");
        git(dir, &["config", "user.email", "test@example.com"]).unwrap();
        git(dir, &["config", "user.name", "Test"]).unwrap();
        fs::write(dir.join("README"), "hi").unwrap();
        git(dir, &["add", "README"]).unwrap();
        git(dir, &["commit", "-m", "init"]).unwrap();
    }

    /// FR-010 / AC-010.8 — allocate then release a worktree slot.
    #[test]
    fn worktree_pool_allocate_and_release() {
        let repo = TempDir::new().expect("repo");
        init_repo(repo.path());
        let pool_dir = TempDir::new().expect("pool");
        let pool = WorktreePool::open(repo.path(), pool_dir.path()).expect("open");

        let lease = pool.allocate("agent-a").expect("alloc");
        assert!(lease.path.is_dir());
        assert!(lease.path.join("README").exists());
        assert_eq!(pool.active_slots().unwrap(), vec!["agent-a".to_string()]);

        pool.release("agent-a").expect("release");
        assert!(pool.active_slots().unwrap().is_empty());
        assert!(!lease.path.exists(), "worktree path must be removed after release");
    }

    /// FR-010 / AC-010.8 — non-git root fails loudly.
    #[test]
    fn worktree_pool_rejects_non_git() {
        let dir = TempDir::new().expect("dir");
        let pool_dir = TempDir::new().expect("pool");
        let err = WorktreePool::open(dir.path(), pool_dir.path()).expect_err("must fail");
        assert!(matches!(err, WorktreePoolError::NotGitRepo(_)));
    }

    #[test]
    fn git_command_clears_hook_repository_context() {
        let command = git_command(Path::new("/tmp"));
        for variable in GIT_LOCAL_ENV_VARS {
            assert!(
                command.get_envs().any(|(key, value)| key == *variable && value.is_none()),
                "{variable} must not leak into a foreign git command"
            );
        }
        assert!(
            command.get_envs().any(|(key, value)| key == "GIT_NAMESPACE" && value.is_none()),
            "GIT_NAMESPACE must not leak into a foreign git command"
        );
    }
}
