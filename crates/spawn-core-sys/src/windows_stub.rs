//! Windows stub for spawn-core — no Zig / POSIX.
//!
//! Provides the same public surface as the Unix Zig FFI path so
//! `spawn_policy` and dependents compile.  The counting semaphore is a real
//! `Mutex` + `Condvar` implementation.  `zig_spawn` / `zig_waitpid` return
//! `ErrorKind::Unsupported` (Zig hot core is POSIX-only).

use std::ffi::CStr;
use std::io::{Error, ErrorKind};
use std::os::raw::{c_char, c_int, c_uchar};
use std::sync::{Condvar, Mutex};

/// Opaque handle type kept for API parity with the Unix FFI path.
#[repr(transparent)]
pub struct SemaphoreHandle(*mut std::ffi::c_void);

// SAFETY: unused on Windows; present for type parity.
unsafe impl Send for SemaphoreHandle {}
unsafe impl Sync for SemaphoreHandle {}

/// Spawn parameters — layout matches Unix `SpawnParams` for API parity.
#[repr(C)]
pub struct SpawnParams {
    pub program: *const c_char,
    pub argv: *const *const c_char,
    pub envp: *const *const c_char,
    pub cwd: *const c_char,
    pub nice_delta: c_int,
    pub background_qos: c_uchar,
}

struct SemInner {
    available: usize,
    max: usize,
}

/// Counting semaphore (Rust `Mutex` + `Condvar` stand-in for Zig pthread).
pub struct ZigSemaphore {
    inner: Mutex<SemInner>,
    cond: Condvar,
}

impl ZigSemaphore {
    /// Construct a semaphore with `max` permits (clamped to at least 1).
    pub fn new(max: usize) -> Self {
        let max = max.max(1);
        Self { inner: Mutex::new(SemInner { available: max, max }), cond: Condvar::new() }
    }

    /// Acquire one permit, blocking until a slot is free.
    pub fn acquire(&self) -> Result<(), Error> {
        let mut guard = self.inner.lock().map_err(|_| Error::other("semaphore mutex poisoned"))?;
        while guard.available == 0 {
            guard =
                self.cond.wait(guard).map_err(|_| Error::other("semaphore condvar poisoned"))?;
        }
        guard.available -= 1;
        Ok(())
    }

    /// Try to acquire one permit without blocking.
    /// Returns `Ok(true)` on success, `Ok(false)` if the semaphore is full.
    pub fn try_acquire(&self) -> Result<bool, Error> {
        let mut guard = self.inner.lock().map_err(|_| Error::other("semaphore mutex poisoned"))?;
        if guard.available == 0 {
            return Ok(false);
        }
        guard.available -= 1;
        Ok(true)
    }

    /// Release one permit, waking a waiter if any. No-op when already at max.
    pub fn release(&self) -> Result<(), Error> {
        let mut guard = self.inner.lock().map_err(|_| Error::other("semaphore mutex poisoned"))?;
        if guard.available < guard.max {
            guard.available += 1;
        }
        self.cond.notify_one();
        Ok(())
    }

    /// Return the approximate number of available permits (0 on poisoned lock).
    pub fn available(&self) -> usize {
        self.inner.lock().map(|g| g.available).unwrap_or(0)
    }
}

// SAFETY: Mutex + Condvar are Sync/Send.
unsafe impl Send for ZigSemaphore {}
unsafe impl Sync for ZigSemaphore {}

fn unsupported(op: &str) -> Error {
    Error::new(
        ErrorKind::Unsupported,
        format!("{op}: Zig spawn-core is POSIX-only; not available on Windows"),
    )
}

/// Stub: Zig `spc_spawn` is not available on Windows.
pub fn zig_spawn(
    _program: &CStr,
    _args: &[*const c_char],
    _envp: Option<&[*const c_char]>,
    _cwd: Option<&CStr>,
    _nice_delta: i32,
    _background_qos: bool,
) -> Result<i32, Error> {
    Err(unsupported("zig_spawn"))
}

/// Stub: Zig `spc_waitpid` is not available on Windows.
pub fn zig_waitpid(_pid: i32) -> Result<i32, Error> {
    Err(unsupported("zig_waitpid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semaphore_acquire_release() {
        let sem = ZigSemaphore::new(2);
        assert_eq!(sem.available(), 2);

        sem.acquire().unwrap();
        assert_eq!(sem.available(), 1);

        sem.acquire().unwrap();
        assert_eq!(sem.available(), 0);

        assert!(!sem.try_acquire().unwrap());

        sem.release().unwrap();
        assert_eq!(sem.available(), 1);

        assert!(sem.try_acquire().unwrap());
        assert_eq!(sem.available(), 0);
    }

    #[test]
    fn semaphore_cap_enforced() {
        let sem = ZigSemaphore::new(3);
        sem.acquire().unwrap();
        sem.acquire().unwrap();
        sem.acquire().unwrap();
        assert!(!sem.try_acquire().unwrap(), "semaphore must block at cap");
        sem.release().unwrap();
        assert!(sem.try_acquire().unwrap());
    }

    #[test]
    fn spawn_and_waitpid_unsupported() {
        use std::ffi::CString;
        let prog = CString::new("cmd.exe").unwrap();
        let err = zig_spawn(&prog, &[], None, None, 0, false).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unsupported);
        let err = zig_waitpid(1).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unsupported);
    }
}
