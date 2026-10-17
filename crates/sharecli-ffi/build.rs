//! Build script for sharecli-ffi.
//!
//! Emits the absolute path of the freshly-built `libsharecli_ffi.{dylib,so}`
//! into `OUT_DIR` so the integration tests can `dlopen` it deterministically.
//! We deliberately avoid hard-coding `target/debug/...` because the path
//! depends on the build profile and `CARGO_TARGET_DIR` may relocate it.
//!
//! Strategy: parse `CARGO_MANIFEST_DIR` (known at script time) and write a
//! generated `dylib_path.rs` into `OUT_DIR` that the test `include!`s. The
//! generated code exposes two constants (debug, release) and a
//! `resolve_dylib()` helper that picks whichever exists at runtime.

use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // CARGO_MANIFEST_DIR points at crates/sharecli-ffi; the workspace target
    // dir lives two levels up at <workspace>/target.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_target = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target"))
        .expect("sharecli-ffi must live at <workspace>/crates/sharecli-ffi");

    let debug_dylib = workspace_target.join("debug").join("deps").join(dylib_name());
    let release_dylib = workspace_target.join("release").join("deps").join(dylib_name());

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
    let gen = out_dir.join("dylib_path.rs");
    let body = format!(
        r#"pub const DEBUG_DYLIB_PATH: &str = "{debug}";
pub const RELEASE_DYLIB_PATH: &str = "{release}";
pub fn resolve_dylib() -> Option<std::path::PathBuf> {{
    use std::path::Path;
    if Path::new(DEBUG_DYLIB_PATH).exists() {{
        Some(PathBuf::from(DEBUG_DYLIB_PATH))
    }} else if Path::new(RELEASE_DYLIB_PATH).exists() {{
        Some(PathBuf::from(RELEASE_DYLIB_PATH))
    }} else {{
        None
    }}
}}
"#,
        debug = debug_dylib.display().to_string(),
        release = release_dylib.display().to_string(),
    );
    fs::write(&gen, body).expect("write generated dylib_path.rs");
    println!("cargo:rerun-if-changed={}", gen.display());
}

#[cfg(target_os = "macos")]
fn dylib_name() -> &'static str {
    "libsharecli_ffi.dylib"
}

#[cfg(target_os = "linux")]
fn dylib_name() -> &'static str {
    "libsharecli_ffi.so"
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn dylib_name() -> &'static str {
    compile_error!("sharecli-ffi integration tests are unix-only (macos, linux)")
}
