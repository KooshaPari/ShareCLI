// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FUSE inode ↔ relative-path map for passthrough InterceptFs.

use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// Inode number for the FUSE root (libfuse convention).
pub const ROOT_INO: u64 = 1;

/// Bidirectional map from FUSE inodes to paths relative to the backing root.
///
/// Root (`ROOT_INO`) maps to an empty relative path (the backing directory itself).
#[derive(Debug, Default)]
pub struct InodeMap {
    next_ino: u64,
    ino_to_rel: HashMap<u64, PathBuf>,
    rel_to_ino: HashMap<PathBuf, u64>,
}

impl InodeMap {
    /// Create a map with only the root inode registered.
    pub fn new() -> Self {
        let mut map =
            Self { next_ino: ROOT_INO + 1, ino_to_rel: HashMap::new(), rel_to_ino: HashMap::new() };
        map.ino_to_rel.insert(ROOT_INO, PathBuf::new());
        map.rel_to_ino.insert(PathBuf::new(), ROOT_INO);
        map
    }

    /// Resolve `ino` to a relative path (empty for root).
    pub fn resolve(&self, ino: u64) -> Option<&Path> {
        self.ino_to_rel.get(&ino).map(PathBuf::as_path)
    }

    /// Look up an existing inode for `rel`, or allocate a new one.
    pub fn alloc_or_get(&mut self, rel: PathBuf) -> u64 {
        if let Some(&ino) = self.rel_to_ino.get(&rel) {
            return ino;
        }
        let ino = self.next_ino;
        self.next_ino = self.next_ino.saturating_add(1);
        self.rel_to_ino.insert(rel.clone(), ino);
        self.ino_to_rel.insert(ino, rel);
        ino
    }

    /// Build the child relative path under `parent` ino for `name`.
    ///
    /// Returns `None` if `parent` is unknown.
    pub fn child_rel(&self, parent: u64, name: &OsStr) -> Option<PathBuf> {
        let parent_rel = self.resolve(parent)?;
        Some(join_rel(parent_rel, name))
    }

    /// Resolve parent+name to (ino, relative path), allocating if needed.
    pub fn lookup_or_alloc(&mut self, parent: u64, name: &OsStr) -> Option<(u64, PathBuf)> {
        let rel = self.child_rel(parent, name)?;
        let ino = self.alloc_or_get(rel.clone());
        Some((ino, rel))
    }

    /// Remove a relative path mapping (e.g. after unlink/rmdir).
    pub fn remove_rel(&mut self, rel: &Path) {
        if let Some(ino) = self.rel_to_ino.remove(rel) {
            self.ino_to_rel.remove(&ino);
        }
    }

    /// Remap an existing relative path after rename (keeps inode number).
    pub fn rename_rel(&mut self, old: &Path, new: PathBuf) {
        let Some(ino) = self.rel_to_ino.remove(old) else {
            // Destination may still need a mapping on next lookup.
            let _ = new;
            return;
        };
        self.ino_to_rel.insert(ino, new.clone());
        self.rel_to_ino.insert(new, ino);
    }

    /// Absolute path under `backing` for `ino`.
    pub fn abs_path(&self, backing: &Path, ino: u64) -> Option<PathBuf> {
        let rel = self.resolve(ino)?;
        Some(abs_under(backing, rel))
    }
}

/// Join a relative parent path with a child name.
pub fn join_rel(parent_rel: &Path, name: &OsStr) -> PathBuf {
    if parent_rel.as_os_str().is_empty() {
        PathBuf::from(name)
    } else {
        parent_rel.join(name)
    }
}

/// Absolute path = `backing` when `rel` is empty, else `backing/rel`.
pub fn abs_under(backing: &Path, rel: &Path) -> PathBuf {
    if rel.as_os_str().is_empty() {
        backing.to_path_buf()
    } else {
        backing.join(rel)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    /// FR-009 / AC-009.3 — root resolves; nested lookup allocates stable inodes.
    #[test]
    fn inode_map_root_and_nested_lookup() {
        let mut map = InodeMap::new();
        assert_eq!(map.resolve(ROOT_INO), Some(Path::new("")));
        let (ino_a, rel_a) = map.lookup_or_alloc(ROOT_INO, OsStr::new("dir")).expect("root child");
        assert_eq!(rel_a, PathBuf::from("dir"));
        assert!(ino_a > ROOT_INO);
        let (ino_b, rel_b) = map.lookup_or_alloc(ino_a, OsStr::new("file.txt")).expect("nested");
        assert_eq!(rel_b, PathBuf::from("dir/file.txt"));
        let (ino_b2, _) = map.lookup_or_alloc(ino_a, OsStr::new("file.txt")).expect("stable");
        assert_eq!(ino_b, ino_b2);
    }

    /// FR-009 / AC-009.3 — unknown parent yields None (no panic).
    #[test]
    fn inode_map_unknown_parent_is_none() {
        let mut map = InodeMap::new();
        assert!(map.lookup_or_alloc(999, OsStr::new("x")).is_none());
    }

    /// FR-009 / AC-009.3 — remove + rename keep map coherent.
    #[test]
    fn inode_map_remove_and_rename() {
        let mut map = InodeMap::new();
        let (ino, _) = map.lookup_or_alloc(ROOT_INO, OsStr::new("a")).expect("alloc");
        map.rename_rel(Path::new("a"), PathBuf::from("b"));
        assert_eq!(map.resolve(ino), Some(Path::new("b")));
        map.remove_rel(Path::new("b"));
        assert!(map.resolve(ino).is_none());
        let _ = OsString::new();
    }
}
