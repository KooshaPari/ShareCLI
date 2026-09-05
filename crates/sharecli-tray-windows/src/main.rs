//! Native system-tray client for sharecli.
//!
//! Replaces the WinUI 3 XAML `windows/ShareCLITray` dashboard window (which
//! rendered a black/uncomposable content root in self-contained mode) with a
//! real Win32 `NotificationIcon` tray icon + menu — the same native pattern the
//! Linux tray (`sharecli-tray-linux`, ksni) already follows.
//!
//! FR-007 AC-007.51/AC-007.52/AC-007.54: tray presence, IPC sidecar launch,
//! and "open dashboard" affordance.

use std::io::Write;
use std::process::{Child, Command};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// NoFX version fix: MouseButton/MouseButtonState are unused in this build;
/// keep the import lean (`tray_icon::Icon`, `TrayIcon`, `TrayIconBuilder`).

const IPC_SIDECAR: &str = "sharecli-ipc";
const IPC_SIDECAR_EXE: &str = "sharecli-ipc.exe";
const DASHBOARD_URL: &str = "http://127.0.0.1:9000";

/// A shared error type so the FFI-style callers see a uniform failure signal.
#[derive(Debug)]
pub enum TrayError {
    Icon(std::io::Error),
    Menu(String),
    Sidecar(std::io::Error),
    Beam(String),
}

impl std::fmt::Display for TrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrayError::Icon(e) => write!(f, "tray icon error: {e}"),
            TrayError::Menu(e) => write!(f, "tray menu error: {e}"),
            TrayError::Sidecar(e) => write!(f, "sidecar spawn error: {e}"),
            TrayError::Beam(e) => write!(f, "tray-icon runtime error: {e}"),
        }
    }
}

impl std::error::Error for TrayError {}

/// Locate the IPC sidecar executable. Looks next to the current executable
/// first (where a self-contained install stages it), then on PATH.
fn find_sidecar() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    for dir in [exe_dir, std::env::current_dir().ok()].into_iter().flatten() {
        for name in [IPC_SIDECAR_EXE, IPC_SIDECAR] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Last resort: resolve on PATH (works on Unix where the bare name is right).
    for name in [IPC_SIDECAR, IPC_SIDECAR_EXE] {
        if let Ok(resolved) = which(name) {
            if resolved.is_file() {
                return Some(resolved);
            }
        }
    }

    None
}

/// Minimal PATH resolver (no external crate) honoring PATHEXT on Windows.
fn which(bin: &str) -> std::io::Result<std::path::PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let exts: Vec<String> = std::env::var_os("PATHEXT")
        .map(|v| v.to_string_lossy().split(';').map(|s| s.to_string()).collect())
        .unwrap_or_else(|| vec!["".to_string()]);

    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{bin}{ext}"));
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{bin} not found on PATH"),
    ))
}

/// Spawn the IPC sidecar daemon (the process that actually serves the tray and
/// the dashboard's live data). Returns the child handle on success.
fn spawn_ipc_sidecar() -> Result<Child, TrayError> {
    // If it's already running (as detected by a live loopback listener we can
    // at least keep reference-tracked), treat as already up. For tray purposes,
    // spawning a daemon that self-exits on socket-exists is the CLI's job; here
    // we just attempt to launch it idempotently.
    let path = find_sidecar().ok_or_else(|| {
        TrayError::Sidecar(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "sharecli-ipc sidecar not found next to tray or on PATH",
        ))
    })?;

    let dir = path.parent().map(|d| d.to_path_buf());
    #[allow(clippy::zombie_processes)]
    let child = Command::new(&path)
        .arg("serve") // idempotent: refuses/ignores if already bound
        .current_dir(dir.unwrap_or_default())
        .spawn()
        .map_err(TrayError::Sidecar)?;

    Ok(child)
}

/// Builds the menu model (before the icon) so we can clone menu item handles.
/// Returns the `Menu` plus the event-id strings that identify each item.
fn build_menu() -> Result<(Menu, Vec<String>), TrayError> {
    let open = MenuItem::new("Open Dashboard", true, None);
    let health = MenuItem::new("Health Check", true, None);
    let status = MenuItem::new("Status", true, None);
    let sep = PredefinedMenuItem::separator();
    let quit = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append_items(&[&open, &health, &status, &sep, &quit])
        .map_err(|e| TrayError::Menu(e.to_string()))?;

    Ok((menu, vec![
        open.id().0.clone(),
        health.id().0.clone(),
        status.id().0.clone(),
        quit.id().0.clone(),
    ]))
}

/// Show a one-shot desktop notification via PowerShell (fire-and-forget).
fn toast(title: &str, body: &str) {
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "[System.Windows.Forms.MessageBox]::Show('{}','{}','Information')",
                body.replace('\'', "''"),
                title.replace('\'', "''")
            ),
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
}

fn open_dashboard() {
    // Use the tray's expected dashboard port. `sharecli serve` binds 9000 by
    // default; opening the browser is the tray's main affordance.
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd").args(["/C", "start", DASHBOARD_URL]).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(DASHBOARD_URL).spawn();
    }
}

fn main() {
    // Detached IPC sidecar (we don't want to block the tray message loop on it).
    let mut sidecar: Option<Child> = match spawn_ipc_sidecar() {
        Ok(child) => {
            eprintln!("[tray] spawned ipc sidecar pid {}", child.id());
            Some(child)
        }
        Err(e) => {
            toast("sharecli tray", &format!("IPC sidecar not started: {e}"));
            None
        }
    };

    // Build the tray menu and icon.
    let (menu, ids) = match build_menu() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[tray] menu build failed: {e}");
            toast("sharecli tray", "failed to build menu");
            return;
        }
    };
    let (id_open, id_health, id_status, id_quit) =
        (ids[0].clone(), ids[1].clone(), ids[2].clone(), ids[3].clone());

    let icon_rgba = icon_rgba();
    let icon = match Icon::from_rgba(icon_rgba.0, icon_rgba.1, icon_rgba.2) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("[tray] icon error: {e}");
            return;
        }
    };

    let tray: TrayIcon = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("sharecli — distributed filesystem agent")
        .with_icon(icon)
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[tray] icon build failed: {e:?}");
            return;
        }
    };
    let _ = &tray; // keep alive for the duration of main

    let (tx, rx) = mpsc::channel::<String>();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = tx.send(event.id().0.clone());
    }));

    // Poll menu events + health in a worker thread; the tray-icon event loop
    // keeps the process alive.
    thread::spawn(move || loop {
        if let Ok(cmd) = rx.recv_timeout(Duration::from_millis(1000)) {
            if cmd == id_open {
                open_dashboard();
            } else if cmd == id_health {
                let _ = Command::new("sharecli").arg("health").creation_flags(0x08000000).spawn();
                toast("sharecli", "health: see dashboard / terminal");
            } else if cmd == id_status {
                let _ = Command::new("sharecli").arg("status").creation_flags(0x08000000).spawn();
                toast("sharecli", "status: see dashboard / terminal");
            } else if cmd == id_quit {
                if let Some(mut c) = sidecar.take() {
                    let _ = c.kill();
                }
                break;
            }
        }
    });

    // Keep the main thread alive servicing tray-icon's internal message loop.
    loop {
        thread::sleep(Duration::from_millis(2000));
    }
}

/// A simple 16x16 RGBA glyph (a filled rounded square with a lighter center
/// dot) used as the tray icon so we don't depend on embed-resource tooling.
fn icon_rgba() -> (Vec<u8>, u32, u32) {
    const W: u32 = 16;
    const H: u32 = 16;
    let mut px = vec![0u8; (W * H * 4) as usize];
    let mut set = |x: i32, y: i32, rgba: [u8; 4]| {
        if x >= 0 && x < W as i32 && y >= 0 && y < H as i32 {
            let i = ((y as u32 * W + x as u32) * 4) as usize;
            px[i..i + 4].copy_from_slice(&rgba);
        }
    };
    // Fill body.
    for y in 2..(H as i32 - 2) {
        for x in 2..(W as i32 - 2) {
            set(x, y, [16, 96, 200, 255]);
        }
    }
    // Center dot.
    for y in 6..10 {
        for x in 6..10 {
            set(x, y, [210, 230, 255, 255]);
        }
    }
    (px, W, H)
}

#[allow(unused_must_use)]
fn _write_stderr(msg: &str) {
    let _ = std::io::stderr().write_all(msg.as_bytes());
}