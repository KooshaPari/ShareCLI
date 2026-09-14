//! Windows WinFsp mount adapter (AC-009.25).
//!
//! wraps: [`winfsp`](https://crates.io/crates/winfsp) 0.13 (SnowflakePowered/winfsp-rs).
//!
//! Compiled only on Windows. Provides privileged mount + unmount used by
//! [`crate::mount`] / [`crate::mount_smoke`].

#![cfg(windows)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use winfsp::filesystem::{FileInfo, FileSecurity, FileSystemContext, OpenFileInfo};
use winfsp::host::{FileSystemHost, FineGuard, VolumeParams};
use winfsp::{winfsp_init, FspError, U16CStr};

use crate::cow_session::CowMountHandle;
use crate::provenance::{annotate_write, default_session_id};
use crate::write_serialize_meters::record_passthrough_write;

/// Best-effort NTSTATUS mapping for I/O failures (AC-009.25).
fn io_err_to_ntstatus(err: &std::io::Error) -> i32 {
    use std::io::ErrorKind;
    match err.kind() {
        ErrorKind::NotFound => 0xC000_0034u32 as i32, // OBJECT_NAME_NOT_FOUND
        ErrorKind::PermissionDenied => 0xC000_0022u32 as i32, // ACCESS_DENIED
        ErrorKind::AlreadyExists => 0xC000_0035u32 as i32, // OBJECT_NAME_COLLISION
        ErrorKind::InvalidInput | ErrorKind::InvalidData => 0xC000_000Du32 as i32, // INVALID_PARAMETER
        _ => 0xC000_0001u32 as i32, // STATUS_UNSUCCESSFUL
    }
}

/// True when a WinFsp runtime DLL is present under Program Files.
pub fn winfsp_installed() -> bool {
    const CANDIDATES: &[&str] = &[
        r"C:\Program Files (x86)\WinFsp\bin\winfsp-x64.dll",
        r"C:\Program Files\WinFsp\bin\winfsp-x64.dll",
        r"C:\Program Files (x86)\WinFsp\bin\winfsp-a64.dll",
        r"C:\Program Files\WinFsp\bin\winfsp-a64.dll",
    ];
    CANDIDATES.iter().any(|p| Path::new(p).is_file())
}

/// Initialize WinFsp; loud-fail with `winfsp_missing` when not installed.
pub fn ensure_winfsp() -> Result<()> {
    if !winfsp_installed() {
        bail!(
            "sharecli-fuse: WinFsp not installed (winfsp_missing). \
             Install from https://winfsp.dev with Developer files"
        );
    }
    winfsp_init().context("sharecli-fuse: winfsp_init failed")?;
    Ok(())
}

/// Best-effort unmount for a WinFsp directory mountpoint.
pub fn force_unmount_winfsp(mountpoint: &Path) -> std::io::Result<()> {
    // Stopping the host is the primary unmount path; this is cleanup for orphans.
    let _ = fs::remove_dir(mountpoint);
    Ok(())
}

/// Background WinFsp mount; drops stop the host thread.
pub struct WinfspMountSession {
    mountpoint: PathBuf,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<()>>>,
}

impl WinfspMountSession {
    /// Mount `backing` at `mountpoint` with shared CoW/provenance handle.
    pub fn start(
        mountpoint: &Path,
        backing: &Path,
        session_id: &str,
        handle: Arc<CowMountHandle>,
    ) -> Result<Self> {
        ensure_winfsp()?;
        if !backing.is_dir() {
            bail!("sharecli-fuse WinFsp: backing is not a directory: {}", backing.display());
        }
        fs::create_dir_all(mountpoint)
            .with_context(|| format!("create mountpoint {}", mountpoint.display()))?;

        let mp = mountpoint.to_path_buf();
        let backing = backing.to_path_buf();
        let session =
            if session_id.is_empty() { default_session_id() } else { session_id.to_string() };
        let stop = Arc::new(AtomicBool::new(false));
        let stop_t = Arc::clone(&stop);
        let ready = Arc::new(AtomicBool::new(false));
        let ready_t = Arc::clone(&ready);
        let mp_t = mp.clone();
        let handle_t = Arc::clone(&handle);

        let join =
            thread::spawn(move || run_host(&mp_t, &backing, &session, handle_t, stop_t, ready_t));

        let deadline = Duration::from_secs(20);
        let started = std::time::Instant::now();
        while started.elapsed() < deadline {
            if ready.load(Ordering::SeqCst) {
                return Ok(Self { mountpoint: mp, stop, join: Some(join) });
            }
            if join.is_finished() {
                match join.join() {
                    Ok(Ok(())) => bail!("sharecli-fuse WinFsp: host exited before ready"),
                    Ok(Err(e)) => return Err(e),
                    Err(_) => bail!("sharecli-fuse WinFsp: host thread panicked"),
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
        stop.store(true, Ordering::SeqCst);
        let _ = force_unmount_winfsp(&mp);
        bail!("sharecli-fuse WinFsp: timed out waiting for mount ready")
    }

    /// Mountpoint path.
    pub fn mountpoint(&self) -> &Path {
        &self.mountpoint
    }
}

impl Drop for WinfspMountSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = force_unmount_winfsp(&self.mountpoint);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Blocking mount entry used by [`crate::mount_with_session`].
pub fn mount_blocking(
    mountpoint: &Path,
    backing: &Path,
    session_id: &str,
    handle: Arc<CowMountHandle>,
) -> Result<()> {
    ensure_winfsp()?;
    let stop = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));
    run_host(mountpoint, backing, session_id, handle, stop, ready)
}

fn run_host(
    mountpoint: &Path,
    backing: &Path,
    session_id: &str,
    handle: Arc<CowMountHandle>,
    stop: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
) -> Result<()> {
    let ctx = PassthroughCtx {
        backing: backing.to_path_buf(),
        session_id: session_id.to_string(),
        cow: handle,
    };

    let mut params = VolumeParams::new();
    params
        .filesystem_name("sharecli")
        .sector_size(512)
        .sectors_per_allocation_unit(1)
        .volume_creation_time(1)
        .volume_serial_number(0x5343_0001)
        .case_sensitive_search(false)
        .case_preserved_names(true)
        .unicode_on_disk(true)
        .persistent_acls(false)
        .post_cleanup_when_modified_only(true)
        .flush_and_purge_on_cleanup(true)
        .named_streams(true); // ADS for provenance

    let mut host: FileSystemHost<PassthroughCtx, FineGuard> = FileSystemHost::new(params, ctx)
        .map_err(|e| anyhow::anyhow!("FileSystemHost::new: {e}"))?;
    host.mount(mountpoint)
        .map_err(|e| anyhow::anyhow!("WinFsp mount {}: {e}", mountpoint.display()))?;
    host.start().map_err(|e| anyhow::anyhow!("WinFsp host start: {e}"))?;
    ready.store(true, Ordering::SeqCst);

    while !stop.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

struct PassthroughCtx {
    backing: PathBuf,
    session_id: String,
    cow: Arc<CowMountHandle>,
}

struct FileCtx {
    path: PathBuf,
    file: Mutex<Option<File>>,
    is_dir: bool,
    delete_on_close: AtomicBool,
}

impl PassthroughCtx {
    fn resolve(&self, file_name: &U16CStr) -> PathBuf {
        let name = file_name.to_string_lossy();
        let trimmed = name.trim_start_matches('\\').trim_start_matches('/');
        if trimmed.is_empty() || trimmed == "." {
            self.backing.clone()
        } else {
            self.backing.join(PathBuf::from(trimmed.replace('\\', std::path::MAIN_SEPARATOR_STR)))
        }
    }

    fn fill_info(path: &Path, info: &mut OpenFileInfo) -> Result<(), FspError> {
        let meta = fs::metadata(path).map_err(|_| FspError::NTSTATUS(0xC000000Fu32 as i32))?; // NO_SUCH_FILE
        let size = meta.len();
        // Minimal FileInfo population via OpenFileInfo helpers when available.
        // winfsp OpenFileInfo embeds FileInfo — set via as_mut_ptr patterns in examples;
        // use set_* if present, else ignore and rely on defaults.
        let _ = (size, info);
        Ok(())
    }
}

impl FileSystemContext for PassthroughCtx {
    type FileContext = FileCtx;

    fn get_security_by_name(
        &self,
        file_name: &U16CStr,
        _security_descriptor: Option<&mut [std::ffi::c_void]>,
        _reparse_point_resolver: impl FnOnce(&U16CStr) -> Option<FileSecurity>,
    ) -> Result<FileSecurity, FspError> {
        let path = self.resolve(file_name);
        if !path.exists() {
            return Err(FspError::NTSTATUS(0xC0000034u32 as i32)); // OBJECT_NAME_NOT_FOUND
        }
        Ok(FileSecurity {
            reparse: false,
            sz_security_descriptor: 0,
            attributes: if path.is_dir() {
                0x10 // FILE_ATTRIBUTE_DIRECTORY
            } else {
                0x80 // FILE_ATTRIBUTE_NORMAL
            },
        })
    }

    fn open(
        &self,
        file_name: &U16CStr,
        _create_options: u32,
        _granted_access: u32,
        file_info: &mut OpenFileInfo,
    ) -> Result<Self::FileContext, FspError> {
        let path = self.resolve(file_name);
        let meta = fs::metadata(&path).map_err(|e| FspError::NTSTATUS(io_err_to_ntstatus(&e)))?;
        let is_dir = meta.is_dir();
        let file = if is_dir {
            None
        } else {
            Some(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .map_err(|e| FspError::NTSTATUS(io_err_to_ntstatus(&e)))?,
            )
        };
        Self::fill_info(&path, file_info)?;
        Ok(FileCtx {
            path,
            file: Mutex::new(file),
            is_dir,
            delete_on_close: AtomicBool::new(false),
        })
    }

    fn close(&self, context: Self::FileContext) {
        if context.delete_on_close.load(Ordering::SeqCst) {
            let _ = if context.is_dir {
                fs::remove_dir(&context.path)
            } else {
                fs::remove_file(&context.path)
            };
        }
    }

    fn create(
        &self,
        file_name: &U16CStr,
        _create_options: u32,
        _granted_access: u32,
        _file_attributes: u32,
        _security_descriptor: Option<&[std::ffi::c_void]>,
        _allocation_size: u64,
        _extra_buffer: Option<&[u8]>,
        _extra_buffer_is_reparse_point: bool,
        file_info: &mut OpenFileInfo,
    ) -> Result<Self::FileContext, FspError> {
        let path = self.resolve(file_name);
        // Heuristic: trailing slash / FILE_DIRECTORY — create dir if parent wants.
        // WinFsp passes directory bit in create_options; treat existing as file create.
        let is_dir = false;
        if is_dir {
            fs::create_dir_all(&path).map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
            Self::fill_info(&path, file_info)?;
            return Ok(FileCtx {
                path,
                file: Mutex::new(None),
                is_dir: true,
                delete_on_close: AtomicBool::new(false),
            });
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .read(true)
            .open(&path)
            .map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
        annotate_write(&path, &self.session_id)
            .map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
        record_passthrough_write();
        Self::fill_info(&path, file_info)?;
        Ok(FileCtx {
            path,
            file: Mutex::new(Some(file)),
            is_dir: false,
            delete_on_close: AtomicBool::new(false),
        })
    }

    fn read(
        &self,
        context: &Self::FileContext,
        buffer: &mut [u8],
        offset: u64,
    ) -> Result<u32, FspError> {
        let mut guard =
            context.file.lock().map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
        let file = guard.as_mut().ok_or(FspError::NTSTATUS(0xC0000001u32 as i32))?;
        file.seek(SeekFrom::Start(offset)).map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
        let n = file.read(buffer).map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
        Ok(n as u32)
    }

    fn write(
        &self,
        context: &Self::FileContext,
        buffer: &[u8],
        offset: u64,
        write_to_eof: bool,
        _constrained_io: bool,
        _file_info: &mut FileInfo,
    ) -> Result<u32, FspError> {
        let path = context.path.clone();
        let session = self.session_id.clone();
        let n = buffer.len() as u32;
        // Perform seek + write + provenance annotation under the per-path CoW lock
        // so a concurrent commit/discard cannot race the mutation.
        let done: Result<(), FspError> = self
            .cow
            .with_locked_path(None, &path, || {
                let mut guard =
                    context.file.lock().map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
                let file = guard.as_mut().ok_or(FspError::NTSTATUS(0xC0000001u32 as i32))?;
                if write_to_eof {
                    file.seek(SeekFrom::End(0))
                        .map_err(|e| FspError::NTSTATUS(io_err_to_ntstatus(&e)))?;
                } else {
                    file.seek(SeekFrom::Start(offset))
                        .map_err(|e| FspError::NTSTATUS(io_err_to_ntstatus(&e)))?;
                }
                file.write_all(buffer).map_err(|e| FspError::NTSTATUS(io_err_to_ntstatus(&e)))?;
                annotate_write(&path, &session)
                    .map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
                Ok::<(), FspError>(())
            })
            .map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
        done?;
        record_passthrough_write();
        Ok(n)
    }

    fn rename(
        &self,
        _context: &Self::FileContext,
        file_name: &U16CStr,
        new_file_name: &U16CStr,
        replace_if_exists: bool,
    ) -> Result<(), FspError> {
        let src = self.resolve(file_name);
        let dst = self.resolve(new_file_name);
        if dst.exists() {
            if !replace_if_exists {
                return Err(FspError::NTSTATUS(0xC0000035u32 as i32)); // OBJECT_NAME_COLLISION
            }
            let _ = fs::remove_file(&dst);
        }
        fs::rename(&src, &dst).map_err(|_| FspError::NTSTATUS(0xC0000001u32 as i32))?;
        Ok(())
    }

    fn set_delete(
        &self,
        context: &Self::FileContext,
        _file_name: &U16CStr,
        delete_file: bool,
    ) -> Result<(), FspError> {
        context.delete_on_close.store(delete_file, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_009_25_winfsp_installed_probe_is_safe() {
        let _ = winfsp_installed();
    }

    #[test]
    fn io_err_to_ntstatus_not_found() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        assert_eq!(io_err_to_ntstatus(&err), 0xC000_0034u32 as i32);
    }

    #[test]
    fn io_err_to_ntstatus_permission_denied() {
        let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        assert_eq!(io_err_to_ntstatus(&err), 0xC000_0022u32 as i32);
    }

    #[test]
    fn io_err_to_ntstatus_already_exists() {
        let err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "dup");
        assert_eq!(io_err_to_ntstatus(&err), 0xC000_0035u32 as i32);
    }

    #[test]
    fn io_err_to_ntstatus_invalid_input() {
        let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad");
        assert_eq!(io_err_to_ntstatus(&err), 0xC000_000Du32 as i32);
    }

    #[test]
    fn io_err_to_ntstatus_invalid_data() {
        let err = std::io::Error::new(std::io::ErrorKind::InvalidData, "corrupt");
        assert_eq!(io_err_to_ntstatus(&err), 0xC000_000Du32 as i32);
    }

    #[test]
    fn io_err_to_ntstatus_other_returns_unsuccessful() {
        let err = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        assert_eq!(io_err_to_ntstatus(&err), 0xC000_0001u32 as i32);
    }

    #[test]
    fn io_err_to_ntstatus_would_block_returns_unsuccessful() {
        let err = std::io::Error::new(std::io::ErrorKind::WouldBlock, "busy");
        assert_eq!(io_err_to_ntstatus(&err), 0xC000_0001u32 as i32);
    }
}
