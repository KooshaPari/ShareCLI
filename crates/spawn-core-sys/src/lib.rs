// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FFI / platform bindings for `spawn_core`.
//!
//! * **Unix** — links the Zig static library (`fork` / `posix_spawn` / pthread).
//! * **Windows** — Rust stub: working counting semaphore; spawn/waitpid return
//!   [`std::io::ErrorKind::Unsupported`].  See crate README and `build.rs`.

#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(unix)]
mod unix_ffi;
#[cfg(unix)]
pub use unix_ffi::*;

#[cfg(windows)]
mod windows_stub;
#[cfg(windows)]
pub use windows_stub::*;
