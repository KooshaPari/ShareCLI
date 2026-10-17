// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shared CoW mount handle (Linux/macOS InterceptFs + Windows WinFsp).
//!
//! Per-agent staging via [`AgentCowStore`]; commit stamps write provenance.
//! Used by the session registry so `fuse commit|discard` works on all supported OS.

use std::path::{Path, PathBuf};

use crate::agent_cow::AgentCowStore;
use crate::inode_map::abs_under;
use crate::provenance::annotate_write;
use crate::write_serialize::WriteSerializeError;
use crate::InterceptFsOptions;

/// CoW + provenance session shared by FUSE and WinFsp mounts.
#[derive(Debug)]
pub struct CowMountHandle {
    backing: PathBuf,
    session_id: String,
    cow_enabled: bool,
    cow: AgentCowStore,
}

impl CowMountHandle {
    /// Build from CLI / registry mount options over `backing`.
    pub fn from_options(backing: &Path, opts: &InterceptFsOptions) -> Self {
        let session_id = if opts.session_id.is_empty() {
            crate::default_session_id()
        } else {
            opts.session_id.clone()
        };
        let default_agent = opts.agent.clone().unwrap_or_else(|| session_id.clone());
        let cow_root = opts.cow_dir.clone().unwrap_or_else(|| {
            if opts.cow {
                backing.join(".sharecli-cow")
            } else {
                backing.join(".sharecli-cow-staging")
            }
        });
        Self {
            backing: backing.to_path_buf(),
            session_id,
            cow_enabled: opts.cow,
            cow: AgentCowStore::new(cow_root, default_agent, opts.serialize),
        }
    }

    /// Backing root mirrored by the mount.
    pub fn backing(&self) -> &Path {
        &self.backing
    }

    /// Write-provenance session id.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Whether CoW overlays are enabled.
    pub fn cow_enabled(&self) -> bool {
        self.cow_enabled
    }

    /// CoW overlay root.
    pub fn cow_root(&self) -> &Path {
        self.cow.cow_root()
    }

    /// Default agent id.
    pub fn default_agent(&self) -> &str {
        self.cow.default_agent()
    }

    /// Pending relative paths for the default agent.
    pub fn pending_rel_paths(&self) -> Result<Vec<PathBuf>, WriteSerializeError> {
        self.pending_rel_paths_for_agent(None)
    }

    /// Pending relative paths for `agent` (default when `None`).
    pub fn pending_rel_paths_for_agent(
        &self,
        agent: Option<&str>,
    ) -> Result<Vec<PathBuf>, WriteSerializeError> {
        let abs = self.cow.pending_for_agent(agent)?;
        Ok(abs
            .into_iter()
            .filter_map(|p| p.strip_prefix(&self.backing).ok().map(Path::to_path_buf))
            .collect())
    }

    /// Pending paths grouped by agent.
    pub fn pending_by_agent(&self) -> Result<Vec<(String, Vec<PathBuf>)>, WriteSerializeError> {
        let raw = self.cow.list_agent_pending()?;
        Ok(raw
            .into_iter()
            .map(|ap| {
                let rels = ap
                    .backing_paths
                    .into_iter()
                    .filter_map(|p| p.strip_prefix(&self.backing).ok().map(Path::to_path_buf))
                    .collect();
                (ap.agent, rels)
            })
            .collect())
    }

    /// Stage CoW bytes for `agent` at relative `rel`.
    pub fn stage_rel_for_agent(
        &self,
        agent: Option<&str>,
        rel: &Path,
        contents: &[u8],
    ) -> Result<(), WriteSerializeError> {
        if !self.cow_enabled {
            return Err(WriteSerializeError::Io(std::io::Error::other(
                "sharecli-fuse: CoW staging requires fuse mount --cow",
            )));
        }
        let abs = abs_under(&self.backing, rel);
        self.cow.stage_bytes(agent, &abs, contents)
    }

    /// Commit pending CoW for `agent` at relative `rel`.
    pub fn commit_rel_for_agent(
        &self,
        agent: Option<&str>,
        rel: &Path,
    ) -> Result<(), WriteSerializeError> {
        let abs = abs_under(&self.backing, rel);
        self.cow.commit_pending(agent, &abs)?;
        annotate_write(&abs, &self.session_id).map_err(WriteSerializeError::Io)?;
        Ok(())
    }

    /// Discard pending CoW for `agent` at relative `rel`.
    pub fn discard_rel_for_agent(
        &self,
        agent: Option<&str>,
        rel: &Path,
    ) -> Result<(), WriteSerializeError> {
        let abs = abs_under(&self.backing, rel);
        self.cow.discard_pending(agent, &abs)
    }

    /// Commit all pending for `agent`; returns relative paths.
    pub fn commit_all_for_agent(
        &self,
        agent: Option<&str>,
    ) -> Result<Vec<PathBuf>, WriteSerializeError> {
        let abs_paths = self.cow.commit_all_for_agent(agent)?;
        let mut rels = Vec::new();
        for abs in abs_paths {
            annotate_write(&abs, &self.session_id).map_err(WriteSerializeError::Io)?;
            if let Ok(rel) = abs.strip_prefix(&self.backing) {
                rels.push(rel.to_path_buf());
            }
        }
        Ok(rels)
    }

    /// Discard all pending for `agent`; returns relative paths.
    pub fn discard_all_for_agent(
        &self,
        agent: Option<&str>,
    ) -> Result<Vec<PathBuf>, WriteSerializeError> {
        let abs_paths = self.cow.discard_all_for_agent(agent)?;
        Ok(abs_paths
            .into_iter()
            .filter_map(|abs| abs.strip_prefix(&self.backing).ok().map(Path::to_path_buf))
            .collect())
    }

    /// Run `f` under per-path lock when serialize is enabled (WinFsp / FUSE writes).
    pub fn with_locked_path<R, F: FnOnce() -> R>(
        &self,
        agent: Option<&str>,
        path: &Path,
        f: F,
    ) -> Result<R, WriteSerializeError> {
        self.cow.with_locked_path(agent, path, f)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::InterceptFsOptions;

    #[test]
    fn ac_009_27_cow_handle_stage_commit_round_trip() {
        let dir = TempDir::new().unwrap();
        let backing = dir.path().join("back");
        fs::create_dir_all(&backing).unwrap();
        let target = backing.join("f.txt");
        fs::write(&target, b"old").unwrap();

        let opts = InterceptFsOptions {
            session_id: "sess-27".into(),
            cow: true,
            cow_dir: Some(dir.path().join("cow")),
            agent: Some("agent-a".into()),
            serialize: true,
            agents_conf: None,
        };
        let h = CowMountHandle::from_options(&backing, &opts);
        assert!(h.cow_enabled());
        h.stage_rel_for_agent(Some("agent-a"), Path::new("f.txt"), b"new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old");
        h.commit_rel_for_agent(Some("agent-a"), Path::new("f.txt")).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        let prov = crate::read_provenance(&target).unwrap().expect("provenance");
        assert_eq!(prov.session_id, "sess-27");
    }

    /// The derived CoW root must follow the documented InterceptFsOptions
    /// contract on every platform: `{backing}/.sharecli-cow` when CoW is
    /// enabled, `{backing}/.sharecli-cow-staging` otherwise (the WinFsp path
    /// previously always used `.sharecli-cow`, diverging from the Unix path).
    #[test]
    fn cow_handle_default_root_follows_documented_contract() {
        let dir = TempDir::new().unwrap();
        let backing = dir.path().join("back");
        fs::create_dir_all(&backing).unwrap();

        let cow_on = CowMountHandle::from_options(
            &backing,
            &InterceptFsOptions { session_id: "s".into(), cow: true, ..Default::default() },
        );
        assert_eq!(cow_on.cow_root(), backing.join(".sharecli-cow"));

        let cow_off = CowMountHandle::from_options(
            &backing,
            &InterceptFsOptions { session_id: "s".into(), cow: false, ..Default::default() },
        );
        assert_eq!(cow_off.cow_root(), backing.join(".sharecli-cow-staging"));
    }
}
