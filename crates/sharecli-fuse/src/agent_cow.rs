//! Per-agent CoW overlay store (Feb `harness-fuse --cow` parity).
//!
//! Staging lives under `{cow_root}/{agent}/` as independent [`WriteSerialize`]
//! instances so two agents can hold pending edits for the same backing path.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::agents_conf::sanitize_agent_id;
use crate::write_serialize::{WriteSerialize, WriteSerializeError};

/// One agent's pending relative paths (relative to a backing root, when provided).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPending {
    /// Sanitized agent id.
    pub agent: String,
    /// Absolute backing paths with pending staging.
    pub backing_paths: Vec<PathBuf>,
}

/// Per-agent CoW staging rooted at `cow_root`.
#[derive(Debug)]
pub struct AgentCowStore {
    cow_root: PathBuf,
    default_agent: String,
    serialize: bool,
    agents: Mutex<HashMap<String, WriteSerialize>>,
}

impl AgentCowStore {
    /// Create a store under `cow_root` with `default_agent` for unscoped ops.
    pub fn new(
        cow_root: impl Into<PathBuf>,
        default_agent: impl Into<String>,
        serialize: bool,
    ) -> Self {
        let cow_root = cow_root.into();
        let _ = std::fs::create_dir_all(&cow_root);
        Self {
            cow_root,
            default_agent: sanitize_agent_id(&default_agent.into()),
            serialize,
            agents: Mutex::new(HashMap::new()),
        }
    }

    /// CoW overlay root directory.
    pub fn cow_root(&self) -> &Path {
        &self.cow_root
    }

    /// Default agent id used when callers omit `--agent`.
    pub fn default_agent(&self) -> &str {
        &self.default_agent
    }

    /// Whether per-path write locks are enabled (`false` ≡ Feb `--no-serialize`).
    pub fn serialize(&self) -> bool {
        self.serialize
    }

    fn agent_key(&self, agent: Option<&str>) -> String {
        sanitize_agent_id(agent.unwrap_or(self.default_agent.as_str()))
    }

    fn store_for(
        &self,
        agent: &str,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, WriteSerialize>>, WriteSerializeError>
    {
        let mut map = self.agents.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        if !map.contains_key(agent) {
            let root = self.cow_root.join(agent);
            map.insert(agent.to_string(), WriteSerialize::with_staging_root(root));
        }
        Ok(map)
    }

    /// Stage bytes for `backing` under `agent` (or default agent).
    pub fn stage_bytes(
        &self,
        agent: Option<&str>,
        backing: &Path,
        contents: &[u8],
    ) -> Result<(), WriteSerializeError> {
        let key = self.agent_key(agent);
        let map = self.store_for(&key)?;
        let ws = map.get(&key).expect("agent store just inserted");
        ws.stage_bytes(backing, contents)
    }

    /// Commit one pending path for an agent.
    pub fn commit_pending(
        &self,
        agent: Option<&str>,
        backing: &Path,
    ) -> Result<(), WriteSerializeError> {
        let key = self.agent_key(agent);
        let map = self.store_for(&key)?;
        let ws = map.get(&key).expect("agent store just inserted");
        ws.commit_pending(backing)
    }

    /// Discard one pending path for an agent.
    pub fn discard_pending(
        &self,
        agent: Option<&str>,
        backing: &Path,
    ) -> Result<(), WriteSerializeError> {
        let key = self.agent_key(agent);
        let map = self.store_for(&key)?;
        let ws = map.get(&key).expect("agent store just inserted");
        ws.discard_pending(backing)
    }

    /// Commit every pending path for an agent. Returns absolute backing paths committed.
    pub fn commit_all_for_agent(
        &self,
        agent: Option<&str>,
    ) -> Result<Vec<PathBuf>, WriteSerializeError> {
        let key = self.agent_key(agent);
        let paths = {
            let map = self.store_for(&key)?;
            let ws = map.get(&key).expect("agent store just inserted");
            ws.pending_backing_paths()?
        };
        let mut done = Vec::new();
        for p in paths {
            self.commit_pending(Some(&key), &p)?;
            done.push(p);
        }
        Ok(done)
    }

    /// Discard every pending path for an agent. Returns absolute backing paths discarded.
    pub fn discard_all_for_agent(
        &self,
        agent: Option<&str>,
    ) -> Result<Vec<PathBuf>, WriteSerializeError> {
        let key = self.agent_key(agent);
        let paths = {
            let map = self.store_for(&key)?;
            let ws = map.get(&key).expect("agent store just inserted");
            ws.pending_backing_paths()?
        };
        let mut done = Vec::new();
        for p in paths {
            self.discard_pending(Some(&key), &p)?;
            done.push(p);
        }
        Ok(done)
    }

    /// Run `f` under the per-path lock when serialize is enabled.
    pub fn with_locked_path<R, F: FnOnce() -> R>(
        &self,
        agent: Option<&str>,
        path: &Path,
        f: F,
    ) -> Result<R, WriteSerializeError> {
        if !self.serialize {
            return Ok(f());
        }
        let key = self.agent_key(agent);
        let map = self.store_for(&key)?;
        let ws = map.get(&key).expect("agent store just inserted");
        ws.with_locked_path(path, f)
    }

    /// Pending absolute backing paths for one agent.
    pub fn pending_for_agent(
        &self,
        agent: Option<&str>,
    ) -> Result<Vec<PathBuf>, WriteSerializeError> {
        let key = self.agent_key(agent);
        let map = self.store_for(&key)?;
        let ws = map.get(&key).expect("agent store just inserted");
        ws.pending_backing_paths()
    }

    /// Snapshot of all agents that currently have a store (may include empty pending).
    pub fn list_agent_pending(&self) -> Result<Vec<AgentPending>, WriteSerializeError> {
        let map = self.agents.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        let mut out = Vec::new();
        for (agent, ws) in map.iter() {
            out.push(AgentPending {
                agent: agent.clone(),
                backing_paths: ws.pending_backing_paths()?,
            });
        }
        out.sort_by(|a, b| a.agent.cmp(&b.agent));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    /// FR-009 / AC-009.19 — two agents stage the same path independently; commit one leaves the other.
    #[test]
    fn ac_009_19_per_agent_cow_isolation() {
        let dir = tempdir().unwrap();
        let backing = dir.path().join("file.txt");
        fs::write(&backing, b"base").unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);

        cow.stage_bytes(Some("claude"), &backing, b"from-claude").unwrap();
        cow.stage_bytes(Some("cursor"), &backing, b"from-cursor").unwrap();

        cow.commit_pending(Some("claude"), &backing).unwrap();
        assert_eq!(fs::read(&backing).unwrap(), b"from-claude");

        // cursor still pending; discard restores no further change to backing
        cow.discard_pending(Some("cursor"), &backing).unwrap();
        assert_eq!(fs::read(&backing).unwrap(), b"from-claude");
    }

    /// FR-009 / AC-009.19 — commit_all / discard_all for an agent.
    #[test]
    fn ac_009_19_commit_all_and_discard_all() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"a0").unwrap();
        fs::write(&b, b"b0").unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "agent-x", true);

        cow.stage_bytes(None, &a, b"a1").unwrap();
        cow.stage_bytes(None, &b, b"b1").unwrap();
        let committed = cow.commit_all_for_agent(None).unwrap();
        assert_eq!(committed.len(), 2);
        assert_eq!(fs::read(&a).unwrap(), b"a1");
        assert_eq!(fs::read(&b).unwrap(), b"b1");

        cow.stage_bytes(Some("agent-x"), &a, b"a2").unwrap();
        let discarded = cow.discard_all_for_agent(Some("agent-x")).unwrap();
        assert_eq!(discarded.len(), 1);
        assert_eq!(fs::read(&a).unwrap(), b"a1");
    }

    /// FR-009 / AC-009.20 — `--no-serialize` skips lock acquisition path.
    #[test]
    fn ac_009_20_no_serialize_runs_callback() {
        let dir = tempdir().unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", false);
        let path = dir.path().join("x");
        let mut hit = false;
        cow.with_locked_path(None, &path, || {
            hit = true;
        })
        .unwrap();
        assert!(hit);
    }

    // --- Additional coverage tests ---

    #[test]
    fn agent_cow_store_root_accessor() {
        let dir = tempdir().unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);
        assert_eq!(cow.cow_root(), dir.path().join("cow"));
    }

    #[test]
    fn agent_cow_default_agent_accessor() {
        let dir = tempdir().unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "my-agent", true);
        assert_eq!(cow.default_agent(), "my-agent");
    }

    #[test]
    fn agent_cow_serialize_accessor() {
        let dir = tempdir().unwrap();
        let cow_t = AgentCowStore::new(dir.path().join("a"), "x", true);
        assert!(cow_t.serialize());
        let cow_f = AgentCowStore::new(dir.path().join("b"), "y", false);
        assert!(!cow_f.serialize());
    }

    #[test]
    fn agent_cow_with_locked_path_enabled() {
        let dir = tempdir().unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "a", true);
        let result = cow.with_locked_path(None, Path::new("/test"), || 42).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn agent_cow_pending_for_agent_empty() {
        let dir = tempdir().unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);
        assert!(cow.pending_for_agent(None).unwrap().is_empty());
    }

    #[test]
    fn agent_cow_commit_all_for_agent() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);
        cow.stage_bytes(Some("x"), &a, b"a1").unwrap();
        cow.stage_bytes(Some("x"), &b, b"b1").unwrap();
        let mut committed = cow.commit_all_for_agent(Some("x")).unwrap();
        committed.sort();
        assert_eq!(committed.len(), 2);
        assert_eq!(fs::read(&a).unwrap(), b"a1");
        assert_eq!(fs::read(&b).unwrap(), b"b1");
    }

    #[test]
    fn agent_cow_discard_all_for_agent() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("file.txt");
        fs::write(&a, b"keep").unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);
        cow.stage_bytes(None, &a, b"discard").unwrap();
        let discarded = cow.discard_all_for_agent(None).unwrap();
        assert_eq!(discarded.len(), 1);
        assert_eq!(fs::read(&a).unwrap(), b"keep");
    }

    #[test]
    fn agent_cow_list_agent_pending_sorted() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("f.txt");
        fs::write(&a, b"x").unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);
        cow.stage_bytes(Some("z-agent"), &a, b"v").unwrap();
        cow.stage_bytes(Some("a-agent"), &a, b"v").unwrap();
        let list = cow.list_agent_pending().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].agent, "a-agent");
        assert_eq!(list[1].agent, "z-agent");
    }

    #[test]
    fn agent_cow_default_agent_sanitize() {
        let dir = tempdir().unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "a/b c", true);
        assert_eq!(cow.default_agent(), "a_b_c");
    }

    #[test]
    fn agent_cow_stage_creates_agent_dir() {
        let dir = tempdir().unwrap();
        let backing = dir.path().join("f.txt");
        fs::write(&backing, b"x").unwrap();
        let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);
        cow.stage_bytes(Some("brand-new"), &backing, b"y").unwrap();
        assert!(dir.path().join("cow").join("brand-new").exists());
    }
}
