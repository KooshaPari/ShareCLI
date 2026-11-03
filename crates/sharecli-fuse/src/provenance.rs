// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Write provenance via extended attributes (`user.sharecli.*`).
//!
//! Every InterceptFs write / CoW commit records `(session-id, timestamp)` on the
//! backing file without altering file contents. Failures are loud: never
//! silently skip provenance when a write succeeds.
//!
//! Platform backends:
//! - **Unix** — `xattr` crate (`setxattr` / `getxattr`).
//! - **Windows** — NTFS alternate data streams (ADS) via `std::fs`; the WinFsp
//!   adapter enables `named_streams(true)` (AC-009.25).
//!
//! Attribute names:
//! - [`ATTR_SESSION`] — opaque session id (UTF-8)
//! - [`ATTR_WRITTEN_AT`] — Unix epoch seconds as decimal ASCII

use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Extended-attribute name for the writer session id.
pub const ATTR_SESSION: &str = "user.sharecli.session";
/// Extended-attribute name for the write timestamp (Unix seconds, decimal).
pub const ATTR_WRITTEN_AT: &str = "user.sharecli.written_at";

/// NTFS alternate-data-stream name for the session id (Windows backend).
#[cfg(windows)]
const ADS_SESSION: &str = "sharecli_session";
/// NTFS alternate-data-stream name for the write timestamp (Windows backend).
#[cfg(windows)]
const ADS_WRITTEN_AT: &str = "sharecli_written_at";

/// Provenance annotation read back from a backing path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteProvenance {
    /// Session that performed the write.
    pub session_id: String,
    /// Unix epoch seconds when the annotation was written.
    pub written_at_unix: u64,
}

/// Default session id when callers do not supply one (`sharecli-<pid>`).
pub fn default_session_id() -> String {
    format!("sharecli-{}", std::process::id())
}

/// Current Unix epoch seconds (0 if the clock is before the epoch).
pub fn now_unix_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Annotate `path` with session + timestamp attrs (loud fail on IO errors).
pub fn annotate_write(path: &Path, session_id: &str) -> std::io::Result<()> {
    annotate_write_at(path, session_id, now_unix_secs())
}

/// Annotate with an explicit timestamp (tests / deterministic clocks).
pub fn annotate_write_at(
    path: &Path,
    session_id: &str,
    written_at_unix: u64,
) -> std::io::Result<()> {
    if session_id.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "sharecli-fuse provenance: session_id must not be empty",
        ));
    }
    set_attr(path, ATTR_SESSION, session_id.as_bytes())?;
    set_attr(path, ATTR_WRITTEN_AT, written_at_unix.to_string().as_bytes())?;
    Ok(())
}

/// Read provenance attrs from `path`. Returns `Ok(None)` when either attr is missing.
pub fn read_provenance(path: &Path) -> std::io::Result<Option<WriteProvenance>> {
    let session = match get_attr(path, ATTR_SESSION)? {
        Some(bytes) => String::from_utf8(bytes).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("sharecli-fuse provenance: session utf-8: {e}"),
            )
        })?,
        None => return Ok(None),
    };
    let written_raw = match get_attr(path, ATTR_WRITTEN_AT)? {
        Some(bytes) => String::from_utf8(bytes).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("sharecli-fuse provenance: written_at utf-8: {e}"),
            )
        })?,
        None => return Ok(None),
    };
    let written_at_unix = written_raw.parse::<u64>().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("sharecli-fuse provenance: written_at parse: {e}"),
        )
    })?;
    Ok(Some(WriteProvenance { session_id: session, written_at_unix }))
}

/// Unix backend: store via `xattr` (wraps `setxattr` / `getxattr`).
#[cfg(unix)]
fn set_attr(path: &Path, name: &str, value: &[u8]) -> std::io::Result<()> {
    xattr::set(path, name, value).map_err(map_attr_err)
}

#[cfg(unix)]
fn get_attr(path: &Path, name: &str) -> std::io::Result<Option<Vec<u8>>> {
    xattr::get(path, name).map_err(map_attr_err)
}

/// Windows backend: store via NTFS alternate data streams (ADS).
#[cfg(windows)]
fn ads_path(path: &Path, stream: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(":");
    os.push(stream);
    PathBuf::from(os)
}

#[cfg(windows)]
fn set_attr(path: &Path, name: &str, value: &[u8]) -> std::io::Result<()> {
    let stream = match name {
        ATTR_SESSION => ADS_SESSION,
        ATTR_WRITTEN_AT => ADS_WRITTEN_AT,
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("sharecli-fuse provenance: unsupported attr {name:?} on Windows"),
            ))
        }
    };
    std::fs::write(ads_path(path, stream), value)
}

#[cfg(windows)]
fn get_attr(path: &Path, name: &str) -> std::io::Result<Option<Vec<u8>>> {
    let stream = match name {
        ATTR_SESSION => ADS_SESSION,
        ATTR_WRITTEN_AT => ADS_WRITTEN_AT,
        _ => return Ok(None),
    };
    match std::fs::read(ads_path(path, stream)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn map_attr_err(err: std::io::Error) -> std::io::Error {
    std::io::Error::new(err.kind(), format!("sharecli-fuse provenance attr: {err}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn annotate_and_read_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("tracked.txt");
        fs::write(&path, b"payload").unwrap();

        annotate_write_at(&path, "agent-session-1", 1_700_000_000).unwrap();
        let got = read_provenance(&path).unwrap().expect("provenance present");
        assert_eq!(got.session_id, "agent-session-1");
        assert_eq!(got.written_at_unix, 1_700_000_000);
    }

    #[test]
    fn empty_session_fails_loudly() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("x.txt");
        fs::write(&path, b"x").unwrap();
        let err = annotate_write(&path, "").expect_err("empty session");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn missing_attrs_yield_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("plain.txt");
        fs::write(&path, b"plain").unwrap();
        assert!(read_provenance(&path).unwrap().is_none());
    }
}
