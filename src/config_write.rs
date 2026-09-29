//! Atomic configuration file writes.
//!
//! Phase 1 task 1.4. `std::fs::write` is open / truncate / write-in-place: a
//! process killed mid-write leaves a half-written config, and the next
//! `Config::load()` either fails to parse or silently returns
//! `Config::default()`, losing every registered project with no error.
//!
//! The swap follows the standard sequence:
//!
//! 1. stage `contents` in a temporary file **in the same directory** — `rename`
//!    is only atomic within a single file system, so the staging file must not
//!    be created in a temp directory elsewhere;
//! 2. `fsync` the staged data, so the bytes are durable before anything points
//!    at them;
//! 3. preserve the outgoing generation as `.bak`, itself staged and renamed so
//!    an interrupted backup cannot publish a half-written `.bak`;
//! 4. `rename` the staged file over the target — atomic, so readers see either
//!    the whole previous file or the whole new one, never a truncate window;
//! 5. `fsync` the directory so the rename itself is durable.
//!
//! Every failure path removes the staged file, so a failed save leaves the
//! previous config and no debris.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Sibling backup path holding the previous generation of `path`.
pub fn backup_path(path: &Path) -> PathBuf {
    sibling(path, ".bak")
}

/// Sibling path used to stage a write before it is swapped into place.
fn staging_path(path: &Path) -> PathBuf {
    // The pid keeps two concurrent writers from picking the same staging file.
    sibling(path, &format!(".tmp-{}", std::process::id()))
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Atomically replace `path` with `contents`, retaining the previous generation
/// as [`backup_path`].
pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).context("create config directory")?;
        }
    }

    let staged = staging_path(path);
    if let Err(e) = stage(&staged, contents) {
        let _ = std::fs::remove_file(&staged);
        return Err(e);
    }

    if path.exists() {
        if let Err(e) = preserve_backup(path) {
            let _ = std::fs::remove_file(&staged);
            return Err(e);
        }
    }

    if let Err(e) = std::fs::rename(&staged, path) {
        let _ = std::fs::remove_file(&staged);
        return Err(anyhow::Error::new(e).context(format!("replace {}", path.display())));
    }

    sync_directory(path);
    Ok(())
}

/// Write `contents` to `dest` and flush it to stable storage.
fn stage(dest: &Path, contents: &str) -> Result<()> {
    let mut file = std::fs::File::create(dest)
        .with_context(|| format!("create staging file {}", dest.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("write staging file {}", dest.display()))?;
    file.sync_all().with_context(|| format!("fsync staging file {}", dest.display()))?;
    Ok(())
}

/// Copy the current config to `.bak` through a staging file, so an interrupted
/// backup leaves the previous `.bak` intact rather than a truncated one.
fn preserve_backup(primary: &Path) -> Result<()> {
    let backup = backup_path(primary);
    let staged = staging_path(&backup);

    let result = (|| -> Result<()> {
        let contents =
            std::fs::read(primary).with_context(|| format!("read {}", primary.display()))?;
        stage(&staged, String::from_utf8_lossy(&contents).as_ref())?;
        std::fs::rename(&staged, &backup)
            .with_context(|| format!("publish backup {}", backup.display()))
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    result
}

/// Best-effort directory fsync so the rename survives a power loss. Directory
/// handles are a Unix concept; failure here degrades durability, not
/// correctness of the in-flight write, so it is never fatal.
#[cfg(unix)]
fn sync_directory(path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Ok(dir) = std::fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }
    }
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) {}
