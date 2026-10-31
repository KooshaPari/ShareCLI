// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Remap FUSE mount paths back to backing paths (FR-009 / AC-009.14).
//!
//! Hypervisor spawns children against an ephemeral intercept mount while
//! coalesce keys and provenance xattrs remain on the original backing tree.
//! Operators and runtime hooks use [`remap_mount_to_backing`] to translate
//! paths observed through the mount into backing paths for inspection.

use std::path::{Component, Path, PathBuf};

use crate::inode_map::abs_under;

/// Remap `path` when it lies under `mountpoint` to the equivalent path under `backing`.
///
/// Relative `path` values are interpreted relative to `mountpoint`. Returns `None`
/// when `path` is outside the mount subtree (prefix-safe — `/tmp/mount` does not
/// match `/tmp/mountextra`).
pub fn remap_mount_to_backing(mountpoint: &Path, backing: &Path, path: &Path) -> Option<PathBuf> {
    let abs = if path.is_absolute() { path.to_path_buf() } else { mountpoint.join(path) };
    let rel = strip_mount_prefix(mountpoint, &abs)?;
    Some(abs_under(backing, rel.as_path()))
}

fn strip_mount_prefix(mountpoint: &Path, path: &Path) -> Option<PathBuf> {
    let mount_comps: Vec<Component<'_>> = mountpoint.components().collect();
    let path_comps: Vec<Component<'_>> = path.components().collect();

    if path_comps.len() < mount_comps.len() {
        return if path == mountpoint { Some(PathBuf::new()) } else { None };
    }

    for (m, p) in mount_comps.iter().zip(path_comps.iter()) {
        if m != p {
            return None;
        }
    }

    let rel_comps = &path_comps[mount_comps.len()..];
    if rel_comps.is_empty() {
        return Some(PathBuf::new());
    }

    let mut rel = PathBuf::new();
    for c in rel_comps {
        rel.push(c.as_os_str());
    }
    Some(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-009 / AC-009.14 — absolute mount subtree remaps to backing.
    #[test]
    fn remap_absolute_under_mount() {
        let mount = Path::new("/tmp/fuse-mp");
        let backing = Path::new("/workspace/proj");
        assert_eq!(
            remap_mount_to_backing(mount, backing, Path::new("/tmp/fuse-mp/src/main.rs")),
            Some(PathBuf::from("/workspace/proj/src/main.rs"))
        );
        assert_eq!(
            remap_mount_to_backing(mount, backing, Path::new("/tmp/fuse-mp")),
            Some(PathBuf::from("/workspace/proj"))
        );
    }

    /// FR-009 / AC-009.14 — relative paths resolve against mountpoint.
    #[test]
    fn remap_relative_under_mount() {
        let mount = Path::new("/tmp/fuse-mp");
        let backing = Path::new("/workspace/proj");
        assert_eq!(
            remap_mount_to_backing(mount, backing, Path::new("src/main.rs")),
            Some(PathBuf::from("/workspace/proj/src/main.rs"))
        );
    }

    /// FR-009 / AC-009.14 — prefix-safe rejection outside mount subtree.
    #[test]
    fn remap_rejects_outside_mount() {
        let mount = Path::new("/tmp/mount");
        let backing = Path::new("/workspace");
        assert!(remap_mount_to_backing(mount, backing, Path::new("/tmp/mountextra/x")).is_none());
        assert!(remap_mount_to_backing(mount, backing, Path::new("/elsewhere/x")).is_none());
    }
}
