// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Build script for sharecli-ffi.
//!
//! Emits compile-time env vars and a generated `dylib_path.rs` so the
//! integration tests can locate `libsharecli_ffi.{dylib,so,dll}` reliably
//! under `cargo test`, even when `CARGO_TARGET_DIR` is set or artifacts live
//! in the profile root rather than `deps/`.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
    let (profile_dir, target_dir) = ancestor_dirs(&out_dir);

    // Fallback: derive <workspace>/target from manifest layout when OUT_DIR
    // doesn't give us a usable profile dir (unexpected, but defensive).
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_target = manifest_dir.parent().and_then(|p| p.parent()).map(|p| p.join("target"));

    let dylib = dylib_name();

    // Expose locations for the test crate (mechanism b): derived from OUT_DIR.
    if let Some(ref p) = profile_dir {
        println!("cargo:rustc-env=SHARECLI_FFI_PROFILE_DIR={}", path_str(p));
    } else {
        println!("cargo:rustc-env=SHARECLI_FFI_PROFILE_DIR=");
    }
    if let Some(ref p) = target_dir {
        println!("cargo:rustc-env=SHARECLI_FFI_TARGET_DIR={}", path_str(p));
    } else if let Some(ref p) = workspace_target {
        println!("cargo:rustc-env=SHARECLI_FFI_TARGET_DIR={}", path_str(p));
    } else {
        println!("cargo:rustc-env=SHARECLI_FFI_TARGET_DIR=");
    }
    println!("cargo:rustc-env=SHARECLI_FFI_DYLIB_NAME={dylib}");

    let profile_dir_s = profile_dir.as_ref().map(|p| path_str(p)).unwrap_or_default();
    let target_dir_s =
        target_dir.as_ref().or(workspace_target.as_ref()).map(|p| path_str(p)).unwrap_or_default();

    let gen = out_dir.join("dylib_path.rs");
    let body = format!(
        r#"pub const SHARECLI_FFI_DYLIB_NAME: &str = "{dylib}";
pub const SHARECLI_FFI_PROFILE_DIR: &str = "{profile_dir_s}";
pub const SHARECLI_FFI_TARGET_DIR: &str = "{target_dir_s}";

/// Candidate locations for the cdylib produced for this crate.
/// Tries (b) OUT_DIR-derived profile/target dirs, then (c) the test binary's
/// directory (where cargo places test executables and often dylibs), then the
/// legacy <workspace>/target/{{debug,release}}/{{,deps/}} guesses.
pub fn searched_dylib_paths() -> Vec<std::path::PathBuf> {{
    use std::path::PathBuf;
    let name = SHARECLI_FFI_DYLIB_NAME;
    let mut dirs: Vec<PathBuf> = Vec::new();

    if !SHARECLI_FFI_PROFILE_DIR.is_empty() {{
        let p = PathBuf::from(SHARECLI_FFI_PROFILE_DIR);
        dirs.push(p.clone());
        dirs.push(p.join("deps"));
    }}

    if !SHARECLI_FFI_TARGET_DIR.is_empty() {{
        let t = PathBuf::from(SHARECLI_FFI_TARGET_DIR);
        for profile in ["debug", "release"] {{
            dirs.push(t.join(profile));
            dirs.push(t.join(profile).join("deps"));
        }}
    }}

    // (c) Fallback: cargo test binaries live under target/<profile>/deps;
    // the cdylib is commonly alongside the test binary or in the profile root.
    if let Ok(exe) = std::env::current_exe() {{
        if let Some(exe_dir) = exe.parent() {{
            dirs.push(exe_dir.to_path_buf());
            if let Some(parent) = exe_dir.parent() {{
                dirs.push(parent.to_path_buf());
                dirs.push(parent.join("deps"));
            }}
        }}
    }}

    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in dirs {{
        let c = dir.join(name);
        if !candidates.iter().any(|x| x == &c) {{
            candidates.push(c);
        }}
    }}
    candidates
}}

pub fn resolve_dylib() -> Option<std::path::PathBuf> {{
    searched_dylib_paths().into_iter().find(|p| p.exists())
}}
"#,
        dylib = dylib,
        profile_dir_s = profile_dir_s,
        target_dir_s = target_dir_s,
    );
    fs::write(&gen, body).expect("write generated dylib_path.rs");
}

/// Derives `(profile_dir, target_dir)` from `OUT_DIR` without magic ancestor
/// counts. Cargo lays build scripts out as
/// `<target>/<triple?>/<profile>/build/<crate>-<hash>/out`, so anchoring on
/// the `build` component keeps working if the layout gains or loses levels
/// (e.g. `--target <triple>` adds one, future cargo layouts may differ).
/// Falls back to `CARGO_TARGET_DIR` when no `build` component is present;
/// `CARGO_PROFILE_DIR` is not a cargo-provided build-script variable.
fn ancestor_dirs(out_dir: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    for anc in out_dir.ancestors() {
        if anc.file_name() == Some(OsStr::new("build")) {
            let profile = anc.parent().map(Path::to_path_buf);
            let target = profile.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf);
            return (profile, target);
        }
    }
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    (None, target)
}

fn path_str(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

#[cfg(target_os = "macos")]
fn dylib_name() -> &'static str {
    "libsharecli_ffi.dylib"
}

#[cfg(target_os = "linux")]
fn dylib_name() -> &'static str {
    "libsharecli_ffi.so"
}

#[cfg(target_os = "windows")]
fn dylib_name() -> &'static str {
    "sharecli_ffi.dll"
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn dylib_name() -> &'static str {
    compile_error!("sharecli-ffi integration tests are unsupported on this target")
}
