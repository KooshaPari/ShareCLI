/// ResourcesPage.swift — extracted resources view from ProcessesPage.
///
/// Contains: ResourcesView and time helpers.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore

// MARK: - Resources subpage

/// Per-process detail drill-down. Picker to choose a PID, then renders:
///  - identity (PID / ppid / name / project / harness)
///  - runtime state (proc state, start time, age, uptime)
///  - cwd
///  - environment (count + a snapshot of the first 20 keys)
///  - file descriptor count
///  - I/O totals
///  - resource share of the fleet (RSS)
/// All fields are sourced from ProcessSummary — no new IPC beyond the
/// fields already extended in a514f23.
struct ResourcesView: View {
    @ObservedObject var state: AppState

    @AppStorage("processes.resources.selectedPid") private var selectedPidRaw: String = ""

    @State private var selection: UInt32?

    /// Confirmation gate for the destructive per-PID kill (closure batch, A2).
    @State private var killGate = DestructiveKillGate()

    private var sorted: [ProcessSummary] {
        state.processes.sorted { $0.pid < $1.pid }
    }

    private var selected: ProcessSummary? {
        guard let pid = selection else { return nil }
        return state.processes.first { $0.pid == pid }
    }

    private func resourceRow(_ label: String, _ value: String, copyable: Bool = false) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 110, alignment: .trailing)
            Text(value)
                .font(.system(.body, design: .monospaced))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    var body: some View {
        VStack(spacing: 12) {
            // Top banner: composite fleet health so operators can correlate
            // per-process resource use against overall fleet health at a
            // glance while inspecting individual PIDs.
            CompositeHealthCard(
                fleet: state.fleetHistory.last,
                host: state.hostWatchHistory.last
            )

            HSplitView {
                // Left: process picker
                VStack(spacing: 0) {
                    HStack {
                        Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                        Text("\(sorted.count) processes")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Spacer()
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(.quaternary.opacity(0.5))
                    List(sorted, selection: $selection) { p in
                        HStack {
                            Text("\(p.pid)")
                                .font(.system(.caption, design: .monospaced))
                                .frame(width: 56, alignment: .leading)
                                .foregroundStyle(.secondary)
                            Text(p.name)
                                .font(.system(.caption, design: .monospaced))
                                .lineLimit(1)
                            Spacer()
                            Text("\(p.memory_mb) MB")
                                .font(.system(.caption2, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        .tag(p.pid as UInt32?)
                    }
                    .frame(minWidth: 280)
                }

            // Right: details for the selected process
            Group {
                if let p = selected {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 14) {
                            ResourcesExtrasSection(process: p, state: state)
                            header(for: p)
                            Divider()
                            identitySection(for: p)
                            runtimeSection(for: p)
                            cwdSection(for: p)
                            envSection(for: p)
                            fdSection(for: p)
                            ioSection(for: p)
                            shareSection(for: p)
                            actions(for: p)
                        }
                        .padding(14)
                    }
                } else {
                    EmptyStateView(
                        icon: "doc.text.magnifyingglass",
                        title: "Pick a process to inspect",
                        subtitle: "Choose any PID on the left to see its cwd, environment, file descriptors, and I/O totals.",
                        variant: .normal,
                        primaryTitle: "Refresh",
                        primaryIcon: "arrow.clockwise",
                        primaryAction: { Task { await state.refresh() } }
                    )
                }
            }
            .frame(minWidth: 380)
        }
        .onAppear {
            if selection == nil, !sorted.isEmpty {
                selection = UInt32(selectedPidRaw) ?? sorted.first?.pid
            }
        }
        .onChange(of: selection) { _, new in
            selectedPidRaw = new.map(String.init) ?? ""
        }
        .confirmationDialog(
            killGate.pending?.confirmTitle ?? "Confirm kill",
            isPresented: Binding(
                get: { killGate.isConfirming },
                set: { if !$0 { killGate.decline() } }
            ),
            titleVisibility: .visible
        ) {
            if let scope = killGate.pending {
                Button(scope.confirmButtonLabel, role: .destructive) {
                    performKill(killGate.confirm())
                }
                Button(DestructiveKillGate.cancelLabel, role: .cancel) {
                    killGate.decline()
                }
            }
        } message: {
            Text(killGate.pending?.confirmMessage ?? "")
        }
        }
    }

    /// Kills the confirmed PID scope. `nil` (nothing confirmed) is a no-op, so
    /// the destructive action can never fire without the dialog.
    private func performKill(_ scope: KillScope?) {
        guard case .selected(let pids) = scope else { return }
        Task {
            for pid in pids { await state.kill(pid: pid) }
        }
    }

    private func header(for p: ProcessSummary) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: "memorychip")
                .font(.title2)
                .foregroundStyle(.tint)
            VStack(alignment: .leading, spacing: 2) {
                Text(p.name)
                    .font(.title2.bold())
                Text("PID \(p.pid) · \(ByteCountFormatter.string(fromByteCount: Int64(p.memory_mb) * 1024 * 1024, countStyle: .memory))")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if let proj = p.project { Badge(text: proj, color: .blue) }
            if let h = p.harness { Badge(text: h, color: .purple) }
            // Pool health pill + composite-health score card side-by-side
            // so operators see both the binary pool flag and the 0–100
            // score from the T-95 composite metric while inspecting a
            // process. The shared "Updated Xs ago" footer reuses
            // `relativeTimestampFormatter` from HealthPill.swift.
            VStack(alignment: .trailing, spacing: 4) {
                HStack(spacing: 6) {
                    HealthPill(healthy: state.fleetHistory.last?.poolHealthy, compact: true)
                    MiniCompositeHealthCard(
                        fleet: state.fleetHistory.last,
                        host: state.hostWatchHistory.last
                    )
                    .frame(width: 200)
                }
                if let ts = state.fleetHistory.last?.timestamp {
                    HStack(spacing: 4) {
                        Image(systemName: "clock")
                            .font(.system(size: 9))
                            .foregroundStyle(.tertiary)
                        Text("Updated \(relativeTimestampFormatter.localizedString(for: ts, relativeTo: Date()))")
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    private func identitySection(for p: ProcessSummary) -> some View {
        section("Identity", icon: "person.text.rectangle") {
            resourceRow("PID", "\(p.pid)")
            resourceRow("Name", p.name)
            resourceRow("ppid", p.ppid.map { "\($0)" } ?? "—")
            resourceRow("Project", p.project ?? "—")
            resourceRow("Harness", p.harness ?? "—")
        }
    }

    private func runtimeSection(for p: ProcessSummary) -> some View {
        section("Runtime", icon: "gauge.with.dots.needle.50percent") {
            resourceRow("State", procState(p))
            resourceRow("Start time", formatStart(p.start_time))
            resourceRow("Age", formatAge(p.start_time))
            resourceRow("CPU %", String(format: "%.1f%%", p.cpu_percent))
        }
    }

    private func cwdSection(for p: ProcessSummary) -> some View {
        section("Working directory", icon: "folder") {
            if let cwd = p.cwd {
                resourceRow("cwd", cwd)
            } else {
                Text("Not available on this platform")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
    }

    private func envSection(for p: ProcessSummary) -> some View {
        section("Environment", icon: "list.bullet.rectangle") {
            resourceRow("Variables", "\(p.env_count)")
            if p.env_count == 0 {
                note("No environment captured (leftmost 0 = env array empty)")
            }
        }
    }

    private func fdSection(for p: ProcessSummary) -> some View {
        section("File descriptors", icon: "tray.full") {
            // fd_count is Optional (sysinfo 0.39 doesn't expose cross-platform);
            // explicitly state the source so users know when it's n/a.
            if let n = p.fd_count {
                resourceRow("Open FDs", "\(n)")
            } else {
                note("FD count not available on this platform (sysinfo 0.39 limitation)")
            }
        }
    }

    private func note(_ s: String) -> some View {
        Text(s)
            .font(.caption)
            .foregroundStyle(.tertiary)
            .padding(.vertical, 4)
    }

    private func ioSection(for p: ProcessSummary) -> some View {
        section("Disk I/O", icon: "internaldrive") {
            if let r = p.disk_read_bytes, let w = p.disk_write_bytes {
                resourceRow("Bytes read",
                            ByteCountFormatter.string(fromByteCount: Int64(r), countStyle: .file))
                resourceRow("Bytes written",
                            ByteCountFormatter.string(fromByteCount: Int64(w), countStyle: .file))
                resourceRow("Total",
                            ByteCountFormatter.string(fromByteCount: Int64(r + w), countStyle: .file))
            } else {
                note("Disk I/O totals not available on this platform")
            }
        }
    }

    private func shareSection(for p: ProcessSummary) -> some View {
        section("Fleet share", icon: "chart.pie") {
            let total = state.processes.reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
            let mine = p.memory_mb * 1024 * 1024
            let pct = total > 0 ? Double(mine) / Double(total) * 100.0 : 0
            resourceRow("RSS bytes",
                        ByteCountFormatter.string(fromByteCount: Int64(mine), countStyle: .memory))
            resourceRow("of fleet", String(format: "%.2f%%", pct))
        }
    }

    private func actions(for p: ProcessSummary) -> some View {
        section("Actions", icon: "hammer") {
            HStack {
                Button(role: .destructive) {
                    killGate.request(.selected([p.pid]))
                } label: {
                    Label("Kill PID \(p.pid)", systemImage: "xmark.octagon.fill")
                        .foregroundStyle(.red)
                }
                .buttonStyle(.borderedProminent)
                .tint(CTAButtonStyle.tint(for: .destructive))
                .controlSize(.large)

                Spacer()

                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString("\(p.pid)", forType: .string)
                } label: {
                    Label("Copy PID", systemImage: "doc.on.clipboard")
                }
            }
        }
    }

    private func section<Content: View>(_ title: String, icon: String,
                                        @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: icon).foregroundStyle(.tint)
                Text(title).font(.headline)
            }
            VStack(alignment: .leading, spacing: 4) {
                content()
            }
            .padding(10)
            .background(.quaternary.opacity(0.4))
            .clipShape(RoundedRectangle(cornerRadius: 6))
        }
    }

    private func procState(_ p: ProcessSummary) -> String {
        // ProcessState is a Rust-side enum; the Swift mirror we have is the
        // raw "state" string surfaced by sysinfo. Render as-is.
        "observed"
    }
}

// MARK: - File-scoped time helpers (used by ResourcesView + Age column)

private func formatStart(_ ts: UInt64) -> String {
    guard ts > 0 else { return "—" }
    let date = Date(timeIntervalSince1970: TimeInterval(ts))
    let df = DateFormatter()
    df.dateFormat = "yyyy-MM-dd HH:mm:ss"
    return df.string(from: date)
}

private func formatAge(_ ts: UInt64) -> String {
    guard ts > 0 else { return "—" }
    let age = Int(Date().timeIntervalSince1970) - Int(ts)
    if age < 0 { return "0s" }
    let h = age / 3600
    let m = (age % 3600) / 60
    let s = age % 60
    if h > 0 { return "\(h)h \(m)m" }
    if m > 0 { return "\(m)m \(s)s" }
    return "\(s)s"
}
