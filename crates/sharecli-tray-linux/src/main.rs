// SPDX-License-Identifier: Apache-2.0 OR MIT
//! sharecli-tray — Linux system-tray client for sharecli.
//!
//! Renders a StatusNotifierItem tray (via `ksni`, the KDE/freedesktop SNI
//! protocol) that mirrors the macOS Swift tray and Windows WinUI 3 tray: it
//! shows managed-process health and lets the user kill processes. All data
//! comes from the same `sharecli-ipc` daemon over the Unix socket — see `ipc`.
//!
//! Non-Linux targets get a stub `main` so `cargo build --workspace` stays green
//! everywhere; the SNI protocol only exists on freedesktop desktops.

#[cfg(target_os = "linux")]
fn main() {
    linux::run();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("sharecli-tray: the system tray is only supported on Linux (StatusNotifierItem).");
    eprintln!("On macOS use the Swift tray (desktop/), on Windows the WinUI 3 tray (windows/).");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
mod linux {
    use ksni::blocking::{Handle, TrayMethods};
    use sharecli_tray_linux::ipc;
    use sharecli_tray_linux::operator_display;
    use sharecli_tray_linux::poll::tray_poll_interval;

    /// Snapshot of daemon state rendered into the tray. Refreshed by the poll
    /// thread via `handle.update`.
    #[derive(Default)]
    struct ShareCliTray {
        processes: Vec<ipc::ProcessSummary>,
        health: Option<ipc::HealthSnapshot>,
        pool: Option<ipc::PoolSnapshot>,
        status: Option<ipc::StatusSnapshot>,
        connected: bool,
        gate_visual: operator_display::TrayGateVisual,
    }

    impl ksni::Tray for ShareCliTray {
        fn id(&self) -> String {
            "sharecli".into()
        }

        fn title(&self) -> String {
            "ShareCLI".into()
        }

        // Icon reflects thermal gate severity from monitoring.report (AC-007.57).
        fn icon_name(&self) -> String {
            self.gate_visual.linux_icon_name.into()
        }

        // ksni 0.3 renamed IconStatus → Status (StatusNotifierItem status enum).
        fn status(&self) -> ksni::Status {
            use ksni::Status;
            match self.gate_visual.severity {
                operator_display::TrayGateSeverity::Normal => Status::Passive,
                operator_display::TrayGateSeverity::Warning => Status::NeedsAttention,
                operator_display::TrayGateSeverity::Critical => Status::NeedsAttention,
                operator_display::TrayGateSeverity::Offline => Status::NeedsAttention,
            }
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            let description = match (&self.health, self.connected) {
                (Some(h), true) => {
                    let base =
                        operator_display::format_tray_tooltip_summary_line(&self.gate_visual, h);
                    let op =
                        operator_display::format_operator_status_summary(&h.gate, &h.host_watch);
                    let net = operator_display::format_host_net_tray_line(&h.host_watch);
                    format!("{base}\n{op}\n{net}")
                }
                _ => operator_display::format_tray_tooltip_offline_line(&self.gate_visual),
            };
            ksni::ToolTip {
                title: "ShareCLI".into(),
                description,
                icon_name: self.icon_name(),
                icon_pixmap: Vec::new(),
            }
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::*;

            let mut items: Vec<ksni::MenuItem<Self>> = Vec::new();

            let header = match (&self.health, self.connected) {
                (Some(h), true) => {
                    operator_display::format_tray_menu_header_line(&self.gate_visual, h)
                }
                _ => operator_display::format_tray_menu_header_offline_line(&self.gate_visual),
            };
            items.push(StandardItem { label: header, enabled: false, ..Default::default() }.into());
            items.push(MenuItem::Separator);

            if let Some(h) = &self.health {
                if self.connected {
                    items.push(
                        StandardItem {
                            label: format!(
                                "Thermal: {} [{}]",
                                self.gate_visual.badge_label, h.gate.gate_decision
                            ),
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                    );
                    for line in operator_display::format_operator_tray_lines(&h.gate, &h.host_watch)
                    {
                        items.push(
                            StandardItem { label: line, enabled: false, ..Default::default() }
                                .into(),
                        );
                    }
                    items.push(MenuItem::Separator);
                }
            }

            if let (Some(pool), Some(status)) = (&self.pool, &self.status) {
                if self.connected {
                    items.push(MenuItem::Separator);
                    for line in operator_display::format_pool_status_operator_lines(pool, status) {
                        items.push(
                            StandardItem { label: line, enabled: false, ..Default::default() }
                                .into(),
                        );
                    }
                }
            }

            if self.processes.is_empty() {
                let label = if self.connected {
                    "No managed processes".to_string()
                } else {
                    "Start sharecli-ipc to connect".to_string()
                };
                items.push(StandardItem { label, enabled: false, ..Default::default() }.into());
            } else {
                for proc in &self.processes {
                    let submenu = build_process_submenu(proc);
                    let label = format!(
                        "{} [{}]{}",
                        proc.name,
                        proc.pid,
                        proc.project.as_deref().map(|p| format!(" · {p}")).unwrap_or_default(),
                    );
                    items.push(SubMenu { label, submenu, ..Default::default() }.into());
                }
            }

            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Kill All Managed".into(),
                    icon_name: "edit-delete".into(),
                    enabled: !self.processes.is_empty(),
                    activate: Box::new(|_this: &mut Self| {
                        if let Err(e) = ipc::kill_all() {
                            tracing::warn!("kill_all failed: {e}");
                        }
                    }),
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: "Refresh".into(),
                    icon_name: "view-refresh".into(),
                    activate: Box::new(|this: &mut Self| refresh(this)),
                    ..Default::default()
                }
                .into(),
            );
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Quit".into(),
                    icon_name: "application-exit".into(),
                    activate: Box::new(|_this: &mut Self| std::process::exit(0)),
                    ..Default::default()
                }
                .into(),
            );

            items
        }
    }

    fn build_process_submenu(proc: &ipc::ProcessSummary) -> Vec<ksni::MenuItem<ShareCliTray>> {
        use ksni::menu::*;

        let pid = proc.pid;
        vec![
            StandardItem {
                label: format!("Memory: {} MB", proc.memory_mb),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!("Harness: {}", proc.harness.as_deref().unwrap_or("—")),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Kill".into(),
                icon_name: "process-stop".into(),
                activate: Box::new(move |_this: &mut ShareCliTray| {
                    if let Err(e) = ipc::kill(pid) {
                        tracing::warn!("kill pid {pid} failed: {e}");
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    /// Pull the latest state from the IPC daemon into the tray struct.
    ///
    /// Single `monitoring.report` round-trip drives operator gate/host_watch + process
    /// inventory + embedded pool/status (AC-007.48 / AC-007.72); avoids split polls.
    fn refresh(tray: &mut ShareCliTray) {
        match ipc::monitoring_report() {
            Ok(snap) => {
                tray.health = Some(snap.health_snapshot());
                tray.processes = snap.process_summaries();
                tray.pool = Some(snap.pool);
                tray.status = Some(snap.status);
                tray.connected = true;
                tray.gate_visual =
                    operator_display::resolve_tray_gate_visual_from_gate(&snap.gate, true);
            }
            Err(e) => {
                tracing::debug!("monitoring.report poll failed: {e}");
                tray.connected = false;
                tray.health = None;
                tray.processes.clear();
                tray.pool = None;
                tray.status = None;
                tray.gate_visual =
                    operator_display::resolve_tray_gate_visual("UNAVAILABLE", "UNAVAILABLE", false);
            }
        }
    }

    /// Launch the tray binary: configure tracing, spawn the SNI icon, and run the event loop.
    pub fn run() {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "sharecli_tray=info".into()),
            )
            .init();

        let mut initial = ShareCliTray::default();
        refresh(&mut initial);

        let handle: Handle<ShareCliTray> = match initial.spawn() {
            Ok(h) => h,
            Err(e) => {
                eprintln!("sharecli-tray: failed to register StatusNotifierItem: {e}");
                eprintln!("Is a system tray / AppIndicator host running on this desktop?");
                std::process::exit(1);
            }
        };
        let poll_interval = tray_poll_interval();
        tracing::info!("sharecli-tray registered; polling every {}s", poll_interval.as_secs());

        loop {
            std::thread::sleep(poll_interval);
            if handle.is_closed() {
                break;
            }
            // The closure runs on the service thread with exclusive access to
            // the tray struct; returning triggers a menu/icon re-render.
            handle.update(refresh);
        }
    }
}
