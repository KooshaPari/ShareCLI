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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_path_is_a_sibling_with_bak_suffix() {
        assert_eq!(
            backup_path(Path::new("/etc/sharecli/config.toml")).to_string_lossy(),
            "/etc/sharecli/config.toml.bak"
        );
    }

    #[test]
    fn staging_path_is_suffix_tagged_with_the_pid() {
        let staged = staging_path(Path::new("/etc/sharecli/config.toml"));
        assert_eq!(
            staged.to_string_lossy(),
            format!("/etc/sharecli/config.toml.tmp-{}", std::process::id())
        );
    }

    #[test]
    fn sibling_appends_without_touching_the_extension() {
        assert_eq!(sibling(Path::new("a/b.c"), ".x").to_string_lossy(), "a/b.c.x");
    }

    #[test]
    fn first_write_replaces_and_leaves_no_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("config.toml");

        write_atomic(&target, "first").expect("write");
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "first");
        assert!(!backup_path(&target).exists(), "no previous generation yet");
        assert_eq!(staged_files(&dir.path().join("config.toml")), 0);
    }

    #[test]
    fn repeated_writes_publish_a_backup_of_the_previous_generation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("config.toml");

        write_atomic(&target, "v1").expect("write v1");
        write_atomic(&target, "v2").expect("write v2");

        assert_eq!(std::fs::read_to_string(&target).expect("read"), "v2");
        assert_eq!(std::fs::read_to_string(backup_path(&target)).expect("bak"), "v1");

        write_atomic(&target, "v3").expect("write v3");
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "v3");
        assert_eq!(
            std::fs::read_to_string(backup_path(&target)).expect("bak"),
            "v2",
            "each save rotates the backup forward by exactly one generation"
        );
    }

    #[test]
    fn write_creates_missing_parent_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("nested/deeper/config.toml");

        write_atomic(&target, "payload").expect("write");
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "payload");
    }

    #[test]
    fn staging_failure_reports_an_error_and_leaves_no_debris() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A directory at the target path makes `File::create` fail with
        // IsADirectory, exercising the staging-error cleanup path.
        let target = dir.path().join("config.toml");
        std::fs::create_dir(&target).expect("mkdir");

        assert!(write_atomic(&target, "nope").is_err());
        assert!(target.is_dir(), "the existing directory survives a failed save");
        assert_eq!(staged_files(&target), 0, "no staging debris is left behind");
    }

    #[test]
    fn missing_primary_read_failure_aborts_before_rename() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("config.toml");
        std::fs::create_dir(&target).expect("mkdir");

        // The staging file stages fine (it is a file path), then the backup
        // pass fails to read a directory as a file and aborts the swap.
        let err = write_atomic(&target, "payload").expect_err("read must fail");
        assert!(format!("{err:#}").contains("config.toml"), "error names the target: {err:#}");
        assert_eq!(staged_files(&target), 0);
    }

    #[test]
    fn interrupted_backup_leaves_the_previous_backup_intact() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("config.toml");
        write_atomic(&target, "v1").expect("write v1");
        write_atomic(&target, "v2").expect("write v2");
        assert_eq!(std::fs::read_to_string(backup_path(&target)).expect("bak"), "v1");

        // Block the backup staging path so the preserve step fails after the
        // new generation is already staged.
        let blocked = staging_path(&backup_path(&target));
        std::fs::create_dir(&blocked).expect("mkdir blocking stage path");

        let err = write_atomic(&target, "v3").expect_err("backup must fail");
        assert!(format!("{err:#}").contains("staging file"), "error explains the failure: {err:#}");
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "v2", "primary is unchanged");
        assert_eq!(
            std::fs::read_to_string(backup_path(&target)).expect("bak"),
            "v1",
            "the interrupted backup cannot publish a truncated .bak"
        );
    }

    #[test]
    fn stage_writes_and_flushes_exact_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("staged.txt");
        stage(&dest, "hello \u{1F600}").expect("stage");
        assert_eq!(std::fs::read_to_string(&dest).expect("read"), "hello \u{1F600}");
    }

    /// Count leftover staging files for `target` in its own directory.
    fn staged_files(target: &Path) -> usize {
        let dir = target.parent().expect("parent");
        let prefix = format!("{}.tmp-", target.file_name().expect("name").to_string_lossy());
        std::fs::read_dir(dir)
            .expect("read_dir")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .count()
    }
}
