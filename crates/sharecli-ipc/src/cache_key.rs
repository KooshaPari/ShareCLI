//! Cache-key modes from the Feb agent-harness (`harness::cache_key`).
//!
//! | Mode   | Inputs hashed                                      |
//! |--------|----------------------------------------------------|
//! | `Time` | argv + cwd + env_subset (default / legacy)         |
//! | `Args` | argv only (CWD-independent)                        |
//! | `Git`  | argv + cwd + git porcelain + HEAD                  |

use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::CommandKey;

/// How a coalesce cache key is derived from a spawn request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheKeyMode {
    /// argv + cwd + env_subset (legacy [`command_key`] behaviour).
    #[default]
    Time,
    /// argv only — identical commands in different directories share a key.
    Args,
    /// argv + cwd + git working-tree fingerprint (HEAD + porcelain).
    Git,
}

impl CacheKeyMode {
    /// Parse rules.conf `cache_key=` values (`time`, `args`, `git`).
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "args" => Self::Args,
            "git" => Self::Git,
            _ => Self::Time,
        }
    }
}

/// Compute a [`CommandKey`] using the requested mode.
pub fn command_key_with_mode(
    mode: CacheKeyMode,
    argv: &[String],
    cwd: &Path,
    env_subset: &[(String, String)],
) -> CommandKey {
    let mut hasher = Sha256::new();
    hash_argv(&mut hasher, argv);

    match mode {
        CacheKeyMode::Args => {}
        CacheKeyMode::Time => {
            hasher.update(b"\x01");
            hasher.update(cwd.to_string_lossy().as_bytes());
            hasher.update(b"\x01");
            hash_env_subset(&mut hasher, env_subset);
        }
        CacheKeyMode::Git => {
            hasher.update(b"\x01");
            hasher.update(cwd.to_string_lossy().as_bytes());
            hasher.update(b"\x01");
            hasher.update(git_fingerprint(cwd).as_bytes());
        }
    }

    CommandKey(hex::encode(hasher.finalize()))
}

/// Legacy entry point — equivalent to [`command_key_with_mode`] with [`CacheKeyMode::Time`].
pub fn command_key(argv: &[String], cwd: &Path, env_subset: &[(String, String)]) -> CommandKey {
    command_key_with_mode(CacheKeyMode::Time, argv, cwd, env_subset)
}

fn hash_argv(hasher: &mut Sha256, argv: &[String]) {
    for arg in argv {
        hasher.update(arg.as_bytes());
        hasher.update(b"\x00");
    }
}

fn hash_env_subset(hasher: &mut Sha256, env_subset: &[(String, String)]) {
    let mut sorted_env: Vec<&(String, String)> = env_subset.iter().collect();
    sorted_env.sort_by_key(|(k, _)| k.as_str());
    for (k, v) in sorted_env {
        hasher.update(k.as_bytes());
        hasher.update(b"=");
        hasher.update(v.as_bytes());
        hasher.update(b"\x00");
    }
}

/// Git working-tree fingerprint: porcelain status concatenated with HEAD rev.
///
/// Port of Feb `core.sh` `harness::cache_key` git branch (~1865–1888).
fn git_fingerprint(cwd: &Path) -> String {
    let cwd_str = cwd.to_string_lossy();
    let mut state = String::new();

    if let Ok(out) =
        Command::new("git").args(["-C", cwd_str.as_ref(), "status", "--porcelain"]).output()
    {
        if out.status.success() {
            state.push_str(&String::from_utf8_lossy(&out.stdout));
        }
    }

    if let Ok(out) =
        Command::new("git").args(["-C", cwd_str.as_ref(), "rev-parse", "HEAD"]).output()
    {
        if out.status.success() {
            state.push_str(&String::from_utf8_lossy(&out.stdout));
        }
    }

    state
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// FR-008 / AC-008.19 — Args mode ignores cwd/env dimensions.
    #[test]
    fn cache_key_args_mode_ignores_cwd_and_env() {
        let argv = vec!["echo".into(), "x".into()];
        let env_a = vec![("A".into(), "1".into())];
        let env_b = vec![("B".into(), "2".into())];
        let k1 = command_key_with_mode(CacheKeyMode::Args, &argv, Path::new("/a"), &env_a);
        let k2 = command_key_with_mode(CacheKeyMode::Args, &argv, Path::new("/b"), &env_b);
        assert_eq!(k1, k2, "Args mode MUST hash argv only");
    }

    /// FR-008 / AC-008.19 — Time mode differs on cwd/env (legacy command_key).
    #[test]
    fn cache_key_time_mode_includes_cwd_and_env() {
        let argv = vec!["echo".into(), "x".into()];
        let env = vec![("K".into(), "v".into())];
        let k1 = command_key_with_mode(CacheKeyMode::Time, &argv, Path::new("/a"), &env);
        let k2 = command_key_with_mode(CacheKeyMode::Time, &argv, Path::new("/b"), &env);
        assert_ne!(k1, k2, "Time mode MUST incorporate cwd");
    }

    /// FR-008 / AC-008.19 — Git mode differs from Time when git state present.
    #[test]
    fn cache_key_git_mode_parse_and_differs_from_args() {
        let argv = vec!["tool".into()];
        let env: Vec<(String, String)> = vec![];
        let args_key = command_key_with_mode(CacheKeyMode::Args, &argv, Path::new("/tmp"), &env);
        let git_key = command_key_with_mode(CacheKeyMode::Git, &argv, Path::new("/tmp"), &env);
        // Git mode adds cwd + fingerprint even when git is absent; differs from Args.
        assert_ne!(args_key, git_key);
        assert_eq!(CacheKeyMode::parse("git"), CacheKeyMode::Git);
        assert_eq!(CacheKeyMode::parse("ARGS"), CacheKeyMode::Args);
        assert_eq!(CacheKeyMode::parse("time"), CacheKeyMode::Time);
    }

    #[test]
    fn command_key_legacy_matches_time_mode() {
        let argv = vec!["a".into(), "b".into()];
        let cwd = Path::new("/w");
        let env = vec![("X".into(), "1".into())];
        assert_eq!(
            command_key(&argv, cwd, &env),
            command_key_with_mode(CacheKeyMode::Time, &argv, cwd, &env)
        );
    }
}
