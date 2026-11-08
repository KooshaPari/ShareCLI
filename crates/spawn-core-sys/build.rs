// SPDX-License-Identifier: Apache-2.0 OR MIT
//! build.rs — compile the Zig spawn-core static library and link it.
//!
//! # Why Zig for the hot core
//!
//! The semaphore (POSIX mutex+condvar), `posix_spawn`, `setpriority`, and
//! `waitpid` are all single libc calls.  Zig reaches them with zero-overhead
//! `std.c` wrappers and zero-ceremony `extern "c"` linkage, no bindgen, no
//! proc-macro, no async runtime.  The `SpawnParams` struct is `extern struct`
//! in Zig which guarantees C ABI layout; Rust declares it as `#[repr(C)]` —
//! the boundary is a plain C struct + a handful of `i32`-returning functions.
//!
//! # Build protocol (Unix)
//!
//! 1. Linux: `zig build` in `crates/spawn-core/` → `zig-out/lib/libspawn_core.a`
//! 2. macOS: `zig build-obj` + `ar` (Zig 0.14 `zig build` static lib fails to
//!    resolve libSystem on Darwin GHA runners)
//! 3. Cargo links via `cargo:rustc-link-lib=static=spawn_core`
//!
//! The `links = "spawn_core"` key in Cargo.toml ensures at most one copy of the
//! lib is linked in a dependency graph and that build metadata is propagated
//! to downstream crates.
//!
//! # Windows (no Zig)
//!
//! Zig `spawn_core` uses `fork` / `waitpid` / pthread — it does not build on
//! Windows.  When `CARGO_CFG_TARGET_OS=windows`, this script skips Zig entirely;
//! `src/lib.rs` (`cfg(windows)`) supplies a Rust stub with the same public API
//! (working counting semaphore; `zig_spawn` / `zig_waitpid` return
//! `ErrorKind::Unsupported`).
//!
//! Gotcha: use `CARGO_CFG_TARGET_OS`, not `cfg!(windows)` in this file — the
//! latter is the *host* OS of the build script, which breaks cross-compiles.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        // No Zig static lib on Windows — Rust stub in lib.rs provides symbols.
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let spawn_core_dir = manifest_dir.join("..").join("spawn-core");
    let spawn_core_dir = spawn_core_dir.canonicalize()?;
    let lib_out = spawn_core_dir.join("zig-out").join("lib");

    if target_os == "macos" {
        build_macos_archive(&spawn_core_dir, &lib_out)?;
    } else {
        build_unix_via_zig_build(&spawn_core_dir)?;
    }

    // --- Tell Cargo where to find libspawn_core.a ---
    println!("cargo:rustc-link-search=native={}", lib_out.display());
    println!("cargo:rustc-link-lib=static=spawn_core");

    // macOS: link libc (libSystem contains all the POSIX symbols we need).
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=System");
    }

    // Linux: link libc + libpthread.
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=dylib=c");
        println!("cargo:rustc-link-lib=dylib=pthread");
    }

    // Re-run if any Zig source changes.
    println!("cargo:rerun-if-changed={}/src/spawn_core.zig", spawn_core_dir.display());
    println!("cargo:rerun-if-changed={}/build.zig", spawn_core_dir.display());

    Ok(())
}

fn build_unix_via_zig_build(spawn_core_dir: &Path) -> anyhow::Result<()> {
    let status = Command::new("zig")
        .args(["build", "-Doptimize=ReleaseSafe"])
        .current_dir(spawn_core_dir)
        .status()
        .map_err(|e| {
            anyhow::anyhow!("failed to run `zig build`: {e}\nIs zig installed and on PATH?")
        })?;

    if !status.success() {
        anyhow::bail!("`zig build` exited with status {status}");
    }
    Ok(())
}

/// Zig 0.14 `addLibrary` static linkage on Darwin tries to resolve libc into the
/// archive step and fails with undefined `_getcwd` / `_fork` / …. Compile a
/// single object with stack-check off, then `ar` it into `libspawn_core.a`.
fn build_macos_archive(spawn_core_dir: &Path, lib_out: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(lib_out)?;
    let obj = lib_out.join("spawn_core.o");
    let archive = lib_out.join("libspawn_core.a");

    let mut cmd = Command::new("zig");
    cmd.args(["build-obj", "src/spawn_core.zig", "-OReleaseSafe", "-lc", "-fno-stack-check"])
        .arg(format!("-femit-bin={}", obj.display()))
        .current_dir(spawn_core_dir);

    if let Ok(sdk) = env::var("SDKROOT") {
        if !sdk.is_empty() {
            cmd.env("SDKROOT", sdk);
        }
    }

    let status = cmd.status().map_err(|e| {
        anyhow::anyhow!("failed to run `zig build-obj`: {e}\nIs zig installed and on PATH?")
    })?;
    if !status.success() {
        anyhow::bail!("`zig build-obj` exited with status {status}");
    }

    let ar_status = Command::new("ar")
        .args(["rcs"])
        .arg(&archive)
        .arg(&obj)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run `ar`: {e}"))?;
    if !ar_status.success() {
        anyhow::bail!("`ar` exited with status {ar_status}");
    }
    Ok(())
}
