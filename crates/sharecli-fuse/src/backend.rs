/// Runtime backend negotiation for macFUSE on macOS.
///
/// The backend only matters on macOS: macOS 15+ prefers the File System Kit
/// backend while older releases rely on the kernel extension. On Linux and
/// Windows the FUSE layer is platform-native (libfuse3 / WinFsp) and the
/// backend is always unavailable.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// macOS FUSE backend selected at mount time (macFUSE only).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FuseBackend {
    /// File System Kit backend (macOS 15+, preferred when available).
    Fskit,
    /// Legacy macFUSE kernel-extension backend.
    Kernel,
    /// No usable backend detected (or non-macOS platform).
    Unavailable,
}

impl FuseBackend {
    /// Stable operator/JSON label for the selected backend.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kext",
            Self::Fskit => "fskit",
            Self::Unavailable => "non-fuse",
        }
    }
}

/// Select the safest available backend. `SHARECLI_FUSE_BACKEND` may force
/// `fskit` or `kernel`; unsupported forcing degrades to `Unavailable`.
pub fn select_backend() -> FuseBackend {
    if let Ok(value) = std::env::var("SHARECLI_FUSE_BACKEND") {
        return match value.to_ascii_lowercase().as_str() {
            "fskit" => FuseBackend::Fskit,
            "kernel" if kernel_backend_loaded() => FuseBackend::Kernel,
            _ => FuseBackend::Unavailable,
        };
    }
    if cfg!(target_os = "macos") {
        // Prefer the loaded macFUSE kext for the mature, lowest-latency path.
        // FSKit remains the explicit fallback when the kext is unavailable.
        return if kernel_backend_loaded() { FuseBackend::Kernel } else { FuseBackend::Fskit };
    }
    if kernel_backend_loaded() {
        FuseBackend::Kernel
    } else {
        FuseBackend::Unavailable
    }
}

/// Host capabilities used to make a deterministic backend decision.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FuseCapabilities {
    /// macFUSE KEXT/VFS backend is loaded and usable.
    pub kernel_loaded: bool,
    /// macFUSE MFMount/FSKit backend has been approved and is usable.
    pub fskit_approved: bool,
}

/// Reason why macOS must continue without filesystem interception.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FuseBackendDiagnostic {
    /// Neither the proven KEXT backend nor an explicitly approved FSKit backend is available.
    NoVerifiedBackend,
    /// FSKit cannot mount a volume outside `/Volumes`.
    FskitRequiresVolumes,
}

impl FuseBackendDiagnostic {
    /// Operator-facing explanation of the unavailable selection.
    pub const fn message(self) -> &'static str {
        match self {
            Self::NoVerifiedBackend => {
                "macFUSE unavailable: no loaded KEXT and no verified FSKit approval; continuing without filesystem interception"
            }
            Self::FskitRequiresVolumes => {
                "macFUSE FSKit supports mount points only under /Volumes; continuing without filesystem interception"
            }
        }
    }
}

/// Backend decision together with a fail-open diagnostic when no mount is safe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FuseBackendSelection {
    /// Backend that may be passed to a FUSE mount call.
    pub backend: FuseBackend,
    /// Why `backend` is unavailable, if it is unavailable.
    pub diagnostic: Option<FuseBackendDiagnostic>,
}

/// Read-only runtime evidence used by the operator probe and diagnostics.
///
/// This deliberately records capability evidence without loading a kext,
/// changing approval state, mounting a volume, or prompting for privilege.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuseRuntimeEvidence {
    /// Platform reported by Rust's target constants.
    pub platform: &'static str,
    /// Mount point used for backend selection.
    pub mountpoint: PathBuf,
    /// Whether `kmutil showloaded` reported a macFUSE KEXT.
    pub kernel_loaded: bool,
    /// Whether the macFUSE MFMount framework is installed.
    pub fskit_framework: bool,
    /// Whether framework presence and explicit operator approval were both verified.
    pub fskit_approved: bool,
    /// Backend selected by the deterministic policy.
    pub selection: FuseBackendSelection,
    /// Non-FUSE execution remains available when no backend is verified.
    pub non_fuse_fallback: bool,
}

/// Gather read-only runtime evidence and apply KEXT -> FSKit -> non-FUSE policy.
pub fn probe_runtime(mountpoint: &Path) -> FuseRuntimeEvidence {
    let kernel_loaded = kernel_backend_loaded();
    let fskit_framework = fskit_framework_available();
    let fskit_approved = fskit_framework && fskit_approval_requested();
    let selection = select_backend_for_mount_with(
        FuseCapabilities { kernel_loaded, fskit_approved },
        mountpoint,
    );
    FuseRuntimeEvidence {
        platform: std::env::consts::OS,
        mountpoint: mountpoint.to_path_buf(),
        kernel_loaded,
        fskit_framework,
        fskit_approved,
        selection,
        non_fuse_fallback: true,
    }
}

/// Select KEXT first, then approved FSKit, otherwise fail open.
pub fn select_backend_with(capabilities: FuseCapabilities) -> FuseBackend {
    if capabilities.kernel_loaded {
        FuseBackend::Kernel
    } else if capabilities.fskit_approved {
        FuseBackend::Fskit
    } else {
        FuseBackend::Unavailable
    }
}

/// Select a macOS backend for `mountpoint` without permitting an invalid FSKit mount.
///
/// macFUSE's FSKit backend supports mount points only under `/Volumes`. A loaded KEXT is
/// deliberately preferred before this restriction is considered, because the KEXT backend can
/// mount at the caller's requested path.
pub fn select_backend_for_mount_with(
    capabilities: FuseCapabilities,
    mountpoint: &Path,
) -> FuseBackendSelection {
    match select_backend_with(capabilities) {
        FuseBackend::Kernel => {
            FuseBackendSelection { backend: FuseBackend::Kernel, diagnostic: None }
        }
        FuseBackend::Fskit if mountpoint.starts_with("/Volumes") => {
            FuseBackendSelection { backend: FuseBackend::Fskit, diagnostic: None }
        }
        FuseBackend::Fskit => FuseBackendSelection {
            backend: FuseBackend::Unavailable,
            diagnostic: Some(FuseBackendDiagnostic::FskitRequiresVolumes),
        },
        FuseBackend::Unavailable => FuseBackendSelection {
            backend: FuseBackend::Unavailable,
            diagnostic: Some(FuseBackendDiagnostic::NoVerifiedBackend),
        },
    }
}

/// Select the safest available backend for a specific mountpoint.
pub fn select_backend_for_mount(mountpoint: &Path) -> FuseBackendSelection {
    if cfg!(target_os = "macos") {
        return select_backend_for_mount_with(
            FuseCapabilities {
                kernel_loaded: kernel_backend_loaded(),
                fskit_approved: fskit_backend_approved(),
            },
            mountpoint,
        );
    }
    FuseBackendSelection { backend: select_backend(), diagnostic: None }
}

fn fskit_backend_approved() -> bool {
    fskit_framework_available() && fskit_approval_requested()
}

fn fskit_framework_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        Path::new("/Library/Filesystems/macfuse.fs/Contents/Frameworks/MFMount.framework").is_dir()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn fskit_approval_requested() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::env::var("SHARECLI_FUSE_FSKIT_APPROVED")
            .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn kernel_backend_loaded() -> bool {
    Command::new("kmutil")
        .args(["showloaded"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout).to_ascii_lowercase().contains("macfuse")
        })
        .unwrap_or(false)
}

/// Collect non-sensitive host state useful when a macFUSE mount negotiation fails.
///
/// This is intentionally executed only on an error path by callers. It does not
/// alter backend selection and avoids shelling out through a user-controlled shell.
///
/// Only reachable from `#[cfg(target_os = "macos")]` mount code; on other
/// platforms the body is a no-op string, so keep the function compiled for the
/// cross-platform diagnostics path but silence the dead-code lint there.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn runtime_diagnostics() -> String {
    #[cfg(target_os = "macos")]
    {
        let kext = Command::new("kmutil")
            .args(["showloaded"])
            .output()
            .map(|output| {
                if output.status.success() {
                    let loaded = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .find(|line| line.to_ascii_lowercase().contains("macfuse"))
                        .map(str::trim)
                        .unwrap_or("not found")
                        .to_string();
                    format!("kext={loaded}")
                } else {
                    format!("kext=kmutil exit {}", output.status)
                }
            })
            .unwrap_or_else(|error| format!("kext=unavailable ({error})"));
        let version =
            std::fs::read_to_string("/Library/Filesystems/macfuse.fs/Contents/version.plist")
                .ok()
                .and_then(|contents| parse_bundle_version(&contents))
                .unwrap_or_else(|| "unknown".to_string());
        let fskit = Command::new("launchctl")
            .arg("list")
            .output()
            .map(|output| {
                if String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line.contains("com.apple.fskit.fskit_agent"))
                {
                    "running"
                } else {
                    "not-running"
                }
            })
            .unwrap_or("unavailable");
        format!("macFUSE version-entry={version}; {kext}; fskit_agent={fskit}")
    }
    #[cfg(not(target_os = "macos"))]
    {
        "macFUSE diagnostics unavailable on this platform".to_string()
    }
}

/// Extract the `CFBundleShortVersionString` value from a macFUSE version plist.
///
/// Only called from [`runtime_diagnostics`], which is macOS-only at runtime.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_bundle_version(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    while let Some(line) = lines.next() {
        if line.contains("CFBundleShortVersionString") {
            return lines
                .find(|value| !value.trim().is_empty())
                .map(str::trim)
                .map(|value| value.trim_start_matches("<string>").trim_end_matches("</string>"))
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_override_degrades_closed() {
        std::env::set_var("SHARECLI_FUSE_BACKEND", "invalid");
        assert_eq!(select_backend(), FuseBackend::Unavailable);
        std::env::remove_var("SHARECLI_FUSE_BACKEND");
    }

    #[test]
    fn bundle_version_probe_is_deterministic() {
        let plist = "<key>CFBundleShortVersionString</key>\n<string>5.3.3</string>";
        assert_eq!(parse_bundle_version(plist).as_deref(), Some("5.3.3"));
        assert_eq!(parse_bundle_version("<key>Other</key>\n<string>x</string>"), None);
    }

    #[test]
    fn backend_selection_is_kext_first_then_approved_fskit() {
        assert_eq!(
            select_backend_with(FuseCapabilities { kernel_loaded: true, fskit_approved: true }),
            FuseBackend::Kernel
        );
        assert_eq!(
            select_backend_with(FuseCapabilities { kernel_loaded: false, fskit_approved: true }),
            FuseBackend::Fskit
        );
        assert_eq!(select_backend_with(FuseCapabilities::default()), FuseBackend::Unavailable);
    }

    #[test]
    fn approved_fskit_outside_volumes_fails_open_with_a_specific_diagnostic() {
        let selection = select_backend_for_mount_with(
            FuseCapabilities { kernel_loaded: false, fskit_approved: true },
            std::path::Path::new("/tmp/sharecli-fuse"),
        );

        assert_eq!(selection.backend, FuseBackend::Unavailable);
        assert_eq!(selection.diagnostic, Some(FuseBackendDiagnostic::FskitRequiresVolumes));
    }

    #[test]
    fn approved_fskit_under_volumes_is_selected() {
        let selection = select_backend_for_mount_with(
            FuseCapabilities { kernel_loaded: false, fskit_approved: true },
            std::path::Path::new("/Volumes/sharecli-fuse"),
        );

        assert_eq!(selection.backend, FuseBackend::Fskit);
        assert_eq!(selection.diagnostic, None);
    }

    #[test]
    fn loaded_kernel_remains_first_choice_outside_volumes() {
        let selection = select_backend_for_mount_with(
            FuseCapabilities { kernel_loaded: true, fskit_approved: true },
            std::path::Path::new("/tmp/sharecli-fuse"),
        );

        assert_eq!(selection.backend, FuseBackend::Kernel);
        assert_eq!(selection.diagnostic, None);
    }

    #[test]
    fn no_verified_backend_has_a_fail_open_diagnostic() {
        let selection = select_backend_for_mount_with(
            FuseCapabilities::default(),
            std::path::Path::new("/Volumes/sharecli-fuse"),
        );

        assert_eq!(selection.backend, FuseBackend::Unavailable);
        assert_eq!(selection.diagnostic, Some(FuseBackendDiagnostic::NoVerifiedBackend));
    }

    #[test]
    fn runtime_probe_always_advertises_non_fuse_fallback() {
        let evidence = probe_runtime(Path::new("/tmp/sharecli-fuse-probe"));
        assert!(evidence.non_fuse_fallback);
        assert!(!evidence.selection.backend.as_str().is_empty());
    }

    #[test]
    fn backend_labels_are_stable_operator_strings() {
        assert_eq!(FuseBackend::Kernel.as_str(), "kext");
        assert_eq!(FuseBackend::Fskit.as_str(), "fskit");
        assert_eq!(FuseBackend::Unavailable.as_str(), "non-fuse");
    }

    #[test]
    fn diagnostics_are_distinct_and_operator_facing() {
        let no_backend = FuseBackendDiagnostic::NoVerifiedBackend.message();
        let volumes = FuseBackendDiagnostic::FskitRequiresVolumes.message();
        assert!(!no_backend.is_empty());
        assert!(!volumes.is_empty());
        assert_ne!(no_backend, volumes);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn runtime_diagnostics_is_non_empty_off_macos() {
        // Exact match: String::new() / "xyzzy" mutants both differ.
        assert_eq!(runtime_diagnostics(), "macFUSE diagnostics unavailable on this platform");
    }

    // --- Additional coverage tests ---

    #[test]
    fn fuse_capabilities_default_values() {
        let caps = FuseCapabilities::default();
        assert!(!caps.kernel_loaded);
        assert!(!caps.fskit_approved);
    }

    #[test]
    fn fuse_backend_selection_clone_eq() {
        let sel = FuseBackendSelection { backend: FuseBackend::Kernel, diagnostic: None };
        let cloned = sel.clone();
        assert_eq!(sel, cloned);
    }

    #[test]
    fn select_backend_for_mount_with_neither_available() {
        let caps = FuseCapabilities { kernel_loaded: false, fskit_approved: false };
        let sel = select_backend_for_mount_with(caps, Path::new("/Volumes/test"));
        assert_eq!(sel.backend, FuseBackend::Unavailable);
        assert_eq!(sel.diagnostic, Some(FuseBackendDiagnostic::NoVerifiedBackend));
    }

    #[test]
    fn select_backend_for_mount_with_unavailable_fskit_not_checked_for_volumes() {
        // When neither is available, the mountpoint path is irrelevant.
        let caps = FuseCapabilities { kernel_loaded: false, fskit_approved: false };
        let sel = select_backend_for_mount_with(caps, Path::new("/other/path"));
        assert_eq!(sel.backend, FuseBackend::Unavailable);
    }

    #[test]
    fn fuse_runtime_evidence_field_access() {
        let ev = FuseRuntimeEvidence {
            platform: "macos",
            mountpoint: PathBuf::from("/Volumes/test"),
            kernel_loaded: true,
            fskit_framework: false,
            fskit_approved: false,
            selection: FuseBackendSelection { backend: FuseBackend::Kernel, diagnostic: None },
            non_fuse_fallback: true,
        };
        assert_eq!(ev.platform, "macos");
        assert!(ev.kernel_loaded);
        assert!(!ev.fskit_framework);
        assert!(!ev.fskit_approved);
        assert!(ev.non_fuse_fallback);
    }
}
