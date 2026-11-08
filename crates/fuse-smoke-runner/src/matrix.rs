// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FUSE mount-smoke matrix types and loud fail reasons (AC-009.22+).
//!
//! Every selected cell either runs privileged smoke or fails with a structured
//! [`FailReason`] — never a silent skip when the cell is requested.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Structured loud-fail / skip reason codes for matrix cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailReason {
    /// macFUSE Driver Extension / kernel support missing.
    DriverMissing,
    /// WinFsp not installed or not loadable.
    WinfspMissing,
    /// `/dev/fuse` (or platform fuse node) unavailable.
    NoFuseDevice,
    /// Architecture not supported for this cell.
    UnsupportedArch,
    /// Required tooling missing (docker, colima, tart, wsl, …).
    ToolingMissing,
    /// Cell not applicable on this host OS (informational loud fail when forced).
    HostOsMismatch,
    /// Smoke command itself failed.
    SmokeFailed,
}

impl FailReason {
    /// Stable snake_case code for JSON / CLI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DriverMissing => "driver_missing",
            Self::WinfspMissing => "winfsp_missing",
            Self::NoFuseDevice => "no_fuse_device",
            Self::UnsupportedArch => "unsupported_arch",
            Self::ToolingMissing => "tooling_missing",
            Self::HostOsMismatch => "host_os_mismatch",
            Self::SmokeFailed => "smoke_failed",
        }
    }
}

impl fmt::Display for FailReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FailReason {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "driver_missing" => Ok(Self::DriverMissing),
            "winfsp_missing" => Ok(Self::WinfspMissing),
            "no_fuse_device" => Ok(Self::NoFuseDevice),
            "unsupported_arch" => Ok(Self::UnsupportedArch),
            "tooling_missing" => Ok(Self::ToolingMissing),
            "host_os_mismatch" => Ok(Self::HostOsMismatch),
            "smoke_failed" => Ok(Self::SmokeFailed),
            other => Err(format!("unknown fail reason: {other}")),
        }
    }
}

/// Named matrix cell (OS × runtime environment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellId {
    /// Native Linux host with libfuse3.
    LinuxNative,
    /// Docker/Podman Linux container with `/dev/fuse`.
    LinuxContainer,
    /// macOS host running Linux via Colima + container smoke.
    MacHostLinuxColima,
    /// Native macOS with macFUSE.
    MacosNative,
    /// Tart macOS VM with macFUSE pre-enabled in guest.
    MacosVmTart,
    /// WSL2 Linux path (fuse3).
    Wsl2,
    /// Windows native WinFsp.
    WindowsWinfsp,
}

impl CellId {
    /// All cells in stable report order.
    pub const ALL: &'static [CellId] = &[
        Self::LinuxNative,
        Self::LinuxContainer,
        Self::MacHostLinuxColima,
        Self::MacosNative,
        Self::MacosVmTart,
        Self::Wsl2,
        Self::WindowsWinfsp,
    ];

    /// Snake_case id used on CLI (`--cell`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LinuxNative => "linux_native",
            Self::LinuxContainer => "linux_container",
            Self::MacHostLinuxColima => "mac_host_linux_colima",
            Self::MacosNative => "macos_native",
            Self::MacosVmTart => "macos_vm_tart",
            Self::Wsl2 => "wsl2",
            Self::WindowsWinfsp => "windows_winfsp",
        }
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CellId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "linux_native" => Ok(Self::LinuxNative),
            "linux_container" => Ok(Self::LinuxContainer),
            "mac_host_linux_colima" => Ok(Self::MacHostLinuxColima),
            "macos_native" => Ok(Self::MacosNative),
            "macos_vm_tart" => Ok(Self::MacosVmTart),
            "wsl2" => Ok(Self::Wsl2),
            "windows_winfsp" => Ok(Self::WindowsWinfsp),
            "all" => Err("use --all instead of --cell all".into()),
            other => Err(format!(
                "unknown cell `{other}`; expected one of: {}",
                CellId::ALL.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")
            )),
        }
    }
}

/// Outcome of one matrix cell.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CellResult {
    /// Matrix cell id.
    pub cell: CellId,
    /// Host OS string (`linux` / `macos` / `windows`).
    pub host_os: String,
    /// Host CPU arch (`aarch64` / `x86_64`).
    pub host_arch: String,
    /// Whether privileged smoke passed.
    pub ok: bool,
    /// Structured fail reason when `ok` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_reason: Option<FailReason>,
    /// Human-readable detail / command output excerpt.
    pub detail: String,
}

/// Full matrix report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatrixReport {
    /// Per-cell results in run order.
    pub cells: Vec<CellResult>,
    /// True only when every cell in `cells` passed.
    pub ok: bool,
}

impl MatrixReport {
    /// Build a report; `ok` is the conjunction of cell results.
    pub fn from_cells(cells: Vec<CellResult>) -> Self {
        let ok = cells.iter().all(|c| c.ok);
        Self { cells, ok }
    }
}

/// Resolve repo root containing `Containerfile.fuse-smoke` walking up from `start`.
pub fn find_repo_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join("Containerfile.fuse-smoke").is_file() && cur.join("Cargo.toml").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// Default cells for the current host (auto selection when `--all` not forced).
pub fn default_cells_for_host(os: &str, arch: &str) -> Vec<CellId> {
    let _ = arch; // reserved for future arch-specific filtering
    match os {
        "linux" => vec![CellId::LinuxNative, CellId::LinuxContainer],
        "macos" => vec![
            CellId::MacHostLinuxColima,
            CellId::LinuxContainer,
            CellId::MacosNative,
            CellId::MacosVmTart,
        ],
        "windows" => vec![CellId::WindowsWinfsp, CellId::Wsl2],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_009_22_fail_reason_roundtrip() {
        for code in [
            "driver_missing",
            "winfsp_missing",
            "no_fuse_device",
            "unsupported_arch",
            "tooling_missing",
            "host_os_mismatch",
            "smoke_failed",
        ] {
            let r: FailReason = code.parse().expect("parse");
            assert_eq!(r.as_str(), code);
        }
    }

    #[test]
    fn ac_009_22_cell_id_parse_and_all() {
        assert_eq!(CellId::ALL.len(), 7);
        for c in CellId::ALL {
            assert_eq!(c.as_str().parse::<CellId>().unwrap(), *c);
        }
        assert!("bogus".parse::<CellId>().is_err());
    }

    #[test]
    fn ac_009_22_default_cells_macos_prefers_colima_path() {
        let cells = default_cells_for_host("macos", "aarch64");
        assert!(cells.contains(&CellId::MacHostLinuxColima));
        assert!(cells.contains(&CellId::LinuxContainer));
        assert!(cells.contains(&CellId::MacosNative));
    }

    #[test]
    fn ac_009_22_find_repo_root_walks_up() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::write(tmp.path().join("Containerfile.fuse-smoke"), "FROM scratch\n").unwrap();
        let found = find_repo_root(&nested).expect("repo root");
        assert_eq!(found, tmp.path());
    }

    #[test]
    fn ac_009_22_matrix_report_ok_requires_all_cells() {
        let ok = CellResult {
            cell: CellId::LinuxNative,
            host_os: "linux".into(),
            host_arch: "x86_64".into(),
            ok: true,
            fail_reason: None,
            detail: "pass".into(),
        };
        let bad = CellResult {
            cell: CellId::MacosNative,
            host_os: "macos".into(),
            host_arch: "aarch64".into(),
            ok: false,
            fail_reason: Some(FailReason::DriverMissing),
            detail: "no driver".into(),
        };
        assert!(MatrixReport::from_cells(vec![ok.clone()]).ok);
        assert!(!MatrixReport::from_cells(vec![ok, bad]).ok);
    }
}
