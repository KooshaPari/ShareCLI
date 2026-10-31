// SPDX-License-Identifier: Apache-2.0 OR MIT
fn main() {
    #[cfg(target_os = "macos")]
    {
        // fuser is built with `macos-no-mount` for CI (no macFUSE on runners).
        // Only emit the MFMount framework link when macFUSE is actually
        // installed, so the crate links without it.
        let macfuse = std::path::Path::new("/Library/Filesystems/macfuse.fs/Contents/Frameworks");
        if macfuse.exists() {
            println!("cargo:rustc-link-search=framework={}", macfuse.display());
            println!("cargo:rustc-link-lib=framework=MFMount");
        } else {
            println!(
                "cargo:warning=macFUSE not installed; building without MFMount (macos-no-mount)"
            );
        }
    }
}
