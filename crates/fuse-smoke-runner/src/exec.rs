// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Host probes and cell executors for the FUSE smoke matrix.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::matrix::{find_repo_root, CellId, CellResult, FailReason, MatrixReport};

fn host_os() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

fn host_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "unknown"
    }
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn path_exists(p: &str) -> bool {
    Path::new(p).exists()
}

fn fuse_dev_present() -> bool {
    path_exists("/dev/fuse")
        || path_exists("/dev/macfuse0")
        || std::fs::read_dir("/dev")
            .ok()
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().starts_with("macfuse"))
            })
            .unwrap_or(false)
}

fn macfuse_fs_present() -> bool {
    path_exists("/Library/Filesystems/macfuse.fs")
}

fn fail(cell: CellId, reason: FailReason, detail: impl Into<String>) -> CellResult {
    CellResult {
        cell,
        host_os: host_os().into(),
        host_arch: host_arch().into(),
        ok: false,
        fail_reason: Some(reason),
        detail: detail.into(),
    }
}

fn pass(cell: CellId, detail: impl Into<String>) -> CellResult {
    CellResult {
        cell,
        host_os: host_os().into(),
        host_arch: host_arch().into(),
        ok: true,
        fail_reason: None,
        detail: detail.into(),
    }
}

fn repo_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("cwd")?;
    find_repo_root(&cwd).context(
        "Containerfile.fuse-smoke + Cargo.toml not found walking up from cwd; run from sharecli repo",
    )
}

/// Ensure Colima is running (starts if needed). Loud-fails if `colima` missing.
fn ensure_colima() -> Result<(), (FailReason, String)> {
    if !which("colima") {
        return Err((
            FailReason::ToolingMissing,
            "colima not found on PATH (brew install colima)".into(),
        ));
    }
    let status = Command::new("colima")
        .args(["status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let running = status.map(|s| s.success()).unwrap_or(false);
    if running {
        return Ok(());
    }
    // Prefer QEMU when available — VZ often ENOSYS for FUSE mounts (AC-009.22/23).
    let vm_type = if which("qemu-img") || which("qemu-system-aarch64") { "qemu" } else { "vz" };
    let out = Command::new("colima")
        .args(["start", "--cpu", "2", "--memory", "4", "--vm-type", vm_type, "--runtime", "docker"])
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("colima start spawn failed: {e}")))?;
    if !out.status.success() {
        return Err((
            FailReason::ToolingMissing,
            format!(
                "colima start failed (vm-type={vm_type}): {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn ensure_docker() -> Result<(), (FailReason, String)> {
    if !which("docker") {
        return Err((FailReason::ToolingMissing, "docker not found on PATH".into()));
    }
    let out = Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("docker info: {e}")))?;
    if !out.status.success() {
        return Err((
            FailReason::ToolingMissing,
            format!("docker daemon not reachable: {}", String::from_utf8_lossy(&out.stderr).trim()),
        ));
    }
    Ok(())
}

/// Build + run privileged smoke inside Containerfile.fuse-smoke.
fn run_linux_container_smoke(root: &Path) -> Result<(), (FailReason, String)> {
    ensure_docker()?;
    // Prefer Colima Linux VM host smoke when `colima` is present: nested
    // Docker-on-Colima often exposes /dev/fuse without a working FUSE FS
    // (ENOSYS). Running on the VM itself exercises real libfuse.
    if which("colima") {
        return run_colima_vm_smoke(root);
    }
    let cf = root.join("Containerfile.fuse-smoke");
    if !cf.is_file() {
        return Err((FailReason::ToolingMissing, format!("missing {}", cf.display())));
    }
    let tag = "sharecli-fuse-smoke:local";
    let build = Command::new("docker")
        .args(["build", "-f", "Containerfile.fuse-smoke", "-t", tag, "."])
        .current_dir(root)
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("docker build: {e}")))?;
    if !build.status.success() {
        return Err((
            FailReason::SmokeFailed,
            format!("docker build failed:\n{}", String::from_utf8_lossy(&build.stderr)),
        ));
    }
    let run = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--privileged",
            "--device",
            "/dev/fuse",
            "--cap-add",
            "SYS_ADMIN",
            "--security-opt",
            "apparmor=unconfined",
            tag,
        ])
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("docker run: {e}")))?;
    if !run.status.success() {
        let combined = format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
        if combined.contains("no_fuse_device") || combined.contains("/dev/fuse") {
            return Err((FailReason::NoFuseDevice, combined));
        }
        return Err((FailReason::SmokeFailed, combined));
    }
    Ok(())
}

/// Build smoke image, extract binary, run on Colima Linux VM (real FUSE).
fn run_colima_vm_smoke(root: &Path) -> Result<(), (FailReason, String)> {
    ensure_colima()?;
    ensure_docker()?;
    let tag = "sharecli-fuse-smoke:local";
    let build = Command::new("docker")
        .args(["build", "-f", "Containerfile.fuse-smoke", "-t", tag, "."])
        .current_dir(root)
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("docker build: {e}")))?;
    if !build.status.success() {
        return Err((
            FailReason::SmokeFailed,
            format!("docker build failed:\n{}", String::from_utf8_lossy(&build.stderr)),
        ));
    }
    let script = format!(
        "set -euo pipefail; \
         sudo rm -f /var/tmp/fuse-mount-smoke; \
         docker run --rm -v /var/tmp:/out {tag} cp /usr/local/bin/fuse-mount-smoke /out/fuse-mount-smoke; \
         sudo chmod +x /var/tmp/fuse-mount-smoke; \
         export TMPDIR=/var/tmp; \
         export SHARECLI_FUSE_MOUNT_SMOKE=1; \
         sudo -E /var/tmp/fuse-mount-smoke"
    );
    let out = Command::new("colima")
        .args(["ssh", "--", "bash", "-lc", &script])
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("colima ssh: {e}")))?;
    if !out.status.success() {
        let combined = format!(
            "colima VM smoke failed:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if combined.contains("Function not implemented")
            || combined.contains("os error 38")
            || combined.contains("ENOSYS")
        {
            return Err((
                FailReason::NoFuseDevice,
                format!(
                    "{combined}\n\nHint: Apple Virtualization (vz) Colima VMs often return ENOSYS for \
                     FUSE mounts. Recreate with QEMU: `colima delete -f && \
                     colima start --vm-type qemu --runtime docker` (requires `brew install qemu`)."
                ),
            ));
        }
        return Err((FailReason::SmokeFailed, combined));
    }
    Ok(())
}

/// Native privileged smoke via cargo test (linux/macos).
fn run_native_cargo_smoke(root: &Path) -> Result<(), (FailReason, String)> {
    if !fuse_dev_present() && cfg!(target_os = "linux") {
        return Err((FailReason::NoFuseDevice, "/dev/fuse missing on Linux host".into()));
    }
    if cfg!(target_os = "macos") && !macfuse_fs_present() {
        return Err((
            FailReason::DriverMissing,
            "macFUSE filesystem not installed under /Library/Filesystems/macfuse.fs".into(),
        ));
    }
    if cfg!(target_os = "macos") && !fuse_dev_present() {
        return Err((
            FailReason::DriverMissing,
            "macFUSE Driver Extension not loaded (no /dev/macfuse*); enable in System Settings or reboot after install"
                .into(),
        ));
    }
    let out = Command::new("cargo")
        .args([
            "test",
            "-p",
            "sharecli",
            "--test",
            "fr009_fuse_intercept",
            "fr009_privileged_mount_smoke",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .current_dir(root)
        .env("SHARECLI_FUSE_MOUNT_SMOKE", "1")
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("cargo test: {e}")))?;
    if !out.status.success() {
        return Err((
            FailReason::SmokeFailed,
            format!(
                "cargo smoke failed:\n{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        ));
    }
    Ok(())
}

fn run_wsl2_smoke(root: &Path) -> Result<(), (FailReason, String)> {
    if !which("wsl") && !which("wsl.exe") {
        return Err((
            FailReason::ToolingMissing,
            "wsl/wsl.exe not found (WSL2 required for this cell)".into(),
        ));
    }
    let wsl = if which("wsl") { "wsl" } else { "wsl.exe" };
    // Convert Windows path if needed — on Unix hosts this cell is host_os_mismatch unless forced.
    if host_os() != "windows" {
        return Err((
            FailReason::HostOsMismatch,
            "wsl2 cell requires a Windows host with WSL2".into(),
        ));
    }
    let root_disp = root.display().to_string();
    let script = format!(
        "set -euo pipefail; cd \"$(wslpath '{root_disp}' 2>/dev/null || echo '{root_disp}')\"; \
         command -v fusermount3 >/dev/null || (sudo apt-get update && sudo apt-get install -y fuse3 libfuse3-dev); \
         test -e /dev/fuse || exit 42; \
         SHARECLI_FUSE_MOUNT_SMOKE=1 cargo test -p sharecli --test fr009_fuse_intercept \
           fr009_privileged_mount_smoke -- --nocapture --test-threads=1"
    );
    let out = Command::new(wsl)
        .args(["-e", "bash", "-lc", &script])
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("wsl spawn: {e}")))?;
    if out.status.code() == Some(42) {
        return Err((FailReason::NoFuseDevice, "/dev/fuse missing inside WSL2".into()));
    }
    if !out.status.success() {
        return Err((
            FailReason::SmokeFailed,
            format!(
                "wsl smoke failed:\n{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        ));
    }
    Ok(())
}

fn run_windows_winfsp_smoke(root: &Path) -> Result<(), (FailReason, String)> {
    if host_os() != "windows" {
        return Err((
            FailReason::HostOsMismatch,
            "windows_winfsp cell requires Windows host".into(),
        ));
    }
    // Presence probe: WinFsp launcher / install dir.
    let winfsp_ok = path_exists(r"C:\Program Files (x86)\WinFsp\bin\winfsp-x64.dll")
        || path_exists(r"C:\Program Files\WinFsp\bin\winfsp-x64.dll")
        || which("launchctl-winfsp"); // unlikely; keep probe soft
    if !winfsp_ok {
        return Err((
            FailReason::WinfspMissing,
            "WinFsp not found under Program Files; install from https://winfsp.dev with Developer files"
                .into(),
        ));
    }
    run_native_cargo_smoke(root)
}

fn run_macos_vm_tart(root: &Path) -> Result<(), (FailReason, String)> {
    if host_os() != "macos" {
        return Err((FailReason::HostOsMismatch, "macos_vm_tart requires macOS host".into()));
    }
    if !which("tart") {
        return Err((
            FailReason::ToolingMissing,
            "tart not found (brew install cirruslabs/cli/tart); bake a macOS VM image with macFUSE Driver Extension enabled — see docs/ops/fuse-mount-smoke-matrix.md"
                .into(),
        ));
    }
    let image =
        std::env::var("SHARECLI_TART_FUSE_IMAGE").unwrap_or_else(|_| "sharecli-fuse-macos".into());
    let list = Command::new("tart")
        .args(["list"])
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("tart list: {e}")))?;
    let listing = String::from_utf8_lossy(&list.stdout);
    if !listing.contains(&image) {
        return Err((
            FailReason::ToolingMissing,
            format!(
                "tart image `{image}` not found; set SHARECLI_TART_FUSE_IMAGE or create VM per docs/ops/fuse-mount-smoke-matrix.md"
            ),
        ));
    }
    // IP + SSH smoke — image must have sharecli checkout + macFUSE.
    let ip_out = Command::new("tart")
        .args(["ip", &image, "--wait", "30"])
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("tart ip: {e}")))?;
    if !ip_out.status.success() {
        return Err((
            FailReason::ToolingMissing,
            format!("tart ip failed: {}", String::from_utf8_lossy(&ip_out.stderr)),
        ));
    }
    let ip = String::from_utf8_lossy(&ip_out.stdout).trim().to_string();
    let remote = format!(
        "cd {} && SHARECLI_FUSE_MOUNT_SMOKE=1 cargo test -p sharecli --test fr009_fuse_intercept fr009_privileged_mount_smoke -- --nocapture --test-threads=1",
        root.display()
    );
    let user = std::env::var("SHARECLI_TART_SSH_USER").unwrap_or_else(|_| "admin".into());
    let ssh = Command::new("ssh")
        .args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "ConnectTimeout=15",
            &format!("{user}@{ip}"),
            &remote,
        ])
        .output()
        .map_err(|e| (FailReason::ToolingMissing, format!("ssh: {e}")))?;
    if !ssh.status.success() {
        return Err((
            FailReason::SmokeFailed,
            format!(
                "tart guest smoke failed:\n{}\n{}",
                String::from_utf8_lossy(&ssh.stdout),
                String::from_utf8_lossy(&ssh.stderr)
            ),
        ));
    }
    Ok(())
}

fn execute_cell(cell: CellId, root: &Path) -> CellResult {
    let mapped = match cell {
        CellId::LinuxNative => {
            if host_os() != "linux" {
                Err((FailReason::HostOsMismatch, "linux_native requires Linux host".into()))
            } else {
                run_native_cargo_smoke(root)
            }
        }
        CellId::LinuxContainer => run_linux_container_smoke(root),
        CellId::MacHostLinuxColima => {
            if host_os() != "macos" {
                Err((
                    FailReason::HostOsMismatch,
                    "mac_host_linux_colima requires macOS host".into(),
                ))
            } else {
                match ensure_colima() {
                    Ok(()) => run_linux_container_smoke(root),
                    Err(e) => Err(e),
                }
            }
        }
        CellId::MacosNative => {
            if host_os() != "macos" {
                Err((FailReason::HostOsMismatch, "macos_native requires macOS host".into()))
            } else {
                run_native_cargo_smoke(root)
            }
        }
        CellId::MacosVmTart => run_macos_vm_tart(root),
        CellId::Wsl2 => run_wsl2_smoke(root),
        CellId::WindowsWinfsp => run_windows_winfsp_smoke(root),
    };
    match mapped {
        Ok(()) => pass(cell, "privileged mount smoke passed"),
        Err((reason, detail)) => fail(cell, reason, detail),
    }
}

/// Run selected cells and build a matrix report.
pub fn run_matrix(cells: &[CellId]) -> Result<MatrixReport> {
    if cells.is_empty() {
        bail!("no cells selected");
    }
    let root = repo_root()?;
    let results: Vec<CellResult> = cells.iter().map(|c| execute_cell(*c, &root)).collect();
    Ok(MatrixReport::from_cells(results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::FailReason;

    #[test]
    fn ac_009_22_fail_helper_sets_reason() {
        let r = fail(CellId::MacosNative, FailReason::DriverMissing, "test");
        assert!(!r.ok);
        assert_eq!(r.fail_reason, Some(FailReason::DriverMissing));
    }
}
