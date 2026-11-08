// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Opt-in macOS macFUSE MFMount negotiation probe.
//!
//! This intentionally does not implement a FUSE request loop. It only verifies
//! that macFUSE can create a channel and negotiate a mount, then closes the
//! channel so the temporary mount is released. Run explicitly with
//! `SHARECLI_MFMOUNT_PROBE=1 cargo run -p sharecli-fuse --bin mfmount-probe`.

#[cfg(target_os = "macos")]
mod macos {
    use std::{ffi::CString, io, os::raw::c_char, path::PathBuf};

    type Channel = *mut std::ffi::c_void;

    #[repr(i32)]
    #[derive(Debug, Copy, Clone)]
    enum MountResult {
        Success = 0,
        UnsupportedOs = 1,
        HelperToolsInstallationFailed = 2,
        FileSystemExtensionNotFound = 3,
        FileSystemExtensionRequiresApproval = 4,
        UnexpectedFailure = -1,
    }

    #[link(name = "MFMount", kind = "framework")]
    unsafe extern "C" {
        fn MFChannelCreate() -> Channel;
        fn MFChannelClose(channel: Channel) -> bool;
        fn MFRelease(reference: Channel);
        fn MFMount(
            channel: Channel,
            mount_point: *const c_char,
            options: *const c_char,
            quiet: bool,
        ) -> MountResult;
    }

    pub fn run() -> anyhow::Result<()> {
        if std::env::var("SHARECLI_MFMOUNT_PROBE").ok().as_deref() != Some("1") {
            anyhow::bail!("set SHARECLI_MFMOUNT_PROBE=1 to run the opt-in MFMount probe");
        }
        let mountpoint = tempfile::tempdir()?;
        let mountpoint_path: PathBuf = mountpoint.path().to_path_buf();
        let mountpoint_c = CString::new(mountpoint_path.to_string_lossy().as_bytes())?;
        let options = CString::new("fsname=sharecli-mfmount-probe,backend=fskit")?;
        let channel = unsafe { MFChannelCreate() };
        if channel.is_null() {
            anyhow::bail!("MFChannelCreate failed: {}", io::Error::last_os_error());
        }
        let result = unsafe { MFMount(channel, mountpoint_c.as_ptr(), options.as_ptr(), true) };
        let errno = io::Error::last_os_error();
        eprintln!("mfmount-probe: result={result:?} ({}), errno={errno}", result_code(result));
        unsafe {
            let _ = MFChannelClose(channel);
            MFRelease(channel);
        }
        if matches!(result, MountResult::Success) {
            Ok(())
        } else {
            anyhow::bail!("MFMount negotiation failed: {result:?} ({})", result_code(result));
        }
    }

    fn result_code(result: MountResult) -> &'static str {
        match result {
            MountResult::Success => "success",
            MountResult::UnsupportedOs => "unsupported-os",
            MountResult::HelperToolsInstallationFailed => "helper-tools-installation-failed",
            MountResult::FileSystemExtensionNotFound => "filesystem-extension-not-found",
            MountResult::FileSystemExtensionRequiresApproval => {
                "filesystem-extension-requires-approval"
            }
            MountResult::UnexpectedFailure => "unexpected-failure",
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{result_code, MountResult};

        #[test]
        fn result_mapping_is_stable_and_non_runtime() {
            assert_eq!(result_code(MountResult::Success), "success");
            assert_eq!(result_code(MountResult::UnsupportedOs), "unsupported-os");
            assert_eq!(
                result_code(MountResult::HelperToolsInstallationFailed),
                "helper-tools-installation-failed"
            );
            assert_eq!(
                result_code(MountResult::FileSystemExtensionNotFound),
                "filesystem-extension-not-found"
            );
            assert_eq!(
                result_code(MountResult::FileSystemExtensionRequiresApproval),
                "filesystem-extension-requires-approval"
            );
            assert_eq!(result_code(MountResult::UnexpectedFailure), "unexpected-failure");
        }
    }
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    macos::run()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("mfmount-probe: only supported on macOS");
}
