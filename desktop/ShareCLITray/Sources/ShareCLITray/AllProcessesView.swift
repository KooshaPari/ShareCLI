/// AllProcessesView.swift — extracted main table view from ProcessesPage.
///
/// Contains: AllProcessesView (filter bar, table, bulk actions, export).
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore
import AppKit
import UniformTypeIdentifiers

// MARK: - AllProcessesView

struct AllProcessesView: View {
    @ObservedObject var state: AppState

    @State private var filterText: String = ""
    @FocusState private var filterFocused: Bool
    @State private var minRSS: Double = 0  // MB threshold
    @State private var sortOrder: [KeyPathComparator<ProcessSummary>] = [
        KeyPathComparator(\ProcessSummary.memory_mb, order: .reverse)
    ]
    @State private var selection: Set<UInt32> = []
    @State private var bulkStatus: String = ""
    /// Confirmation gate for the destructive bulk kill buttons (PLAN 1.24).
    @State private var killGate = DestructiveKillGate()

    private var filtered: [ProcessSummary] {
        let q = filterText.lowercased()
        let minBytes = UInt64(max(0, minRSS)) * 1024 * 1024
        return state.processes.filter { p in
            if minBytes > 0 && p.memory_mb * 1024 * 1024 < minBytes { return false }
            if q.isEmpty { return true }
            return p.name.lowercased().contains(q)
                || (p.project?.lowercased().contains(q) ?? false)
                || (p.harness?.lowercased().contains(q) ?? false)
                || String(p.pid).contains(q)
        }
        .sorted(using: sortOrder)
    }

    var body: some View {
        VStack(spacing: 0) {
            summaryStrip
            filterBar
            bulkActionBar
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
            Divider()
            if filtered.isEmpty {
                EmptyStateView(
                    icon: "tray",
                    title: state.processes.isEmpty
                        ? "No processes yet"
                        : "No processes match your filter",
                    subtitle: state.processes.isEmpty
                        ? "The fleet pool will list registered processes here once the host directory contains bun sockets or any host-tracked CLI processes."
                        : "Try widening the text filter, lowering the minimum RSS slider, or clearing the filter entirely.",
                    variant: .hero,
                    primaryTitle: state.processes.isEmpty ? "Refresh now" : "Clear filter",
                    primaryIcon: state.processes.isEmpty ? "arrow.clockwise" : "xmark.circle",
                    primaryAction: {
                        if state.processes.isEmpty {
                            Task { await state.refresh() }
                        } else {
                            filterText = ""
                            minRSS = 0
                        }
                    },
                    secondaryTitle: state.processes.isEmpty ? "How does this work?" : nil,
                    secondaryIcon: "questionmark.circle",
                    secondaryAction: state.processes.isEmpty ? {
                        if let url = URL(string: "https://docs.sharecli.dev/processes") { NSWorkspace.shared.open(url) }
                    } : nil
                )
            } else {
                Table(filtered, selection: $selection, sortOrder: $sortOrder) {
                TableColumn("PID", value: \.pid) { p in
                    Text("\(p.pid)").font(.system(.body, design: .monospaced))
                }
                .width(60)

                TableColumn("Name", value: \.name) { p in
                    Text(p.name).font(.system(.body, design: .monospaced)).lineLimit(1)
                }

                TableColumn("Project") { p in
                    if let proj = p.project { Badge(text: proj, color: .blue) }
                }
                .width(100)

                TableColumn("Harness") { p in
                    if let h = p.harness { Badge(text: h, color: .purple) }
                }
                .width(80)

                TableColumn("Memory (MB)", value: \.memory_mb) { p in
                    Text("\(p.memory_mb)").font(.system(.body, design: .monospaced))
                        .foregroundStyle(rssColor(p.memory_mb))
                }
                .width(110)

                TableColumn("CPU %", value: \.cpu_percent) { p in
                    Text(String(format: "%.1f%%", p.cpu_percent))
                        .font(.system(.body, design: .monospaced))
                        .foregroundStyle(cpuColor(p.cpu_percent))
                        .frame(width: 56, alignment: .trailing)
                    cpuBar(p.cpu_percent)
                }
                .width(140)

                TableColumn("FDs", value: \.fdCountValue) { p in
                    if let fd = p.fd_count {
                        HStack(spacing: 4) {
                            Text("\(fd)")
                                .font(.system(.body, design: .monospaced))
                                .foregroundStyle(fdColor(fd))
                                .frame(width: 38, alignment: .trailing)
                            fdBar(fd)
                        }
                    } else {
                        Text("n/a")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.tertiary)
                    }
                }
                .width(96)

                TableColumn("I/O", value: \.ioReadValue) { p in
                    if let r = p.disk_read_bytes, let w = p.disk_write_bytes {
                        VStack(alignment: .trailing, spacing: 1) {
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.down.circle.fill")
                                    .font(.caption2)
                                    .foregroundStyle(.blue)
                                Text(ioBytes(r))
                                    .font(.system(.caption, design: .monospaced))
                            }
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.up.circle.fill")
                                    .font(.caption2)
                                    .foregroundStyle(.purple)
                                Text(ioBytes(w))
                                    .font(.system(.caption, design: .monospaced))
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .trailing)
                    } else {
                        Text("n/a")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.tertiary)
                            .frame(maxWidth: .infinity, alignment: .trailing)
                    }
                }
                .width(120)

                TableColumn("Age") { p in
                    Text(formatAge(p.start_time))
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
                .width(80)

                TableColumn("Actions") { p in
                    Button {
                        Task { await state.kill(pid: p.pid) }
                    } label: {
                        Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
                    }
                    .buttonStyle(.borderless)
                    .help("Kill PID \(p.pid) (\(p.name))")
                }
                .width(40)
            }
            .frame(minHeight: 240)
            } // end Table
        }
    }

    // MARK: - Summary

    private var summaryStrip: some View {
        let total = state.processes.count
        let filteredCount = filtered.count
        let totalRSSBytes = state.processes.reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
        let selectedRSS = filtered
            .filter { selection.contains($0.pid) }
            .reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
        let topCPU = state.processes
            .max(by: { $0.cpu_percent < $1.cpu_percent })?.cpu_percent ?? 0
        return HStack(spacing: 12) {
            summaryCard("Total", "\(total)", filteredCount == total ? "processes" : "\(filteredCount) shown", .blue)
            summaryCard("Total RSS", ByteCountFormatter.string(fromByteCount: Int64(totalRSSBytes), countStyle: .memory), "fleet", .orange)
            summaryCard("Selected", "\(selection.count)", selection.isEmpty ? "—" : ByteCountFormatter.string(fromByteCount: Int64(selectedRSS), countStyle: .memory), .purple)
            summaryCard("Top CPU", String(format: "%.1f%%", topCPU), "fleet peak", cpuColor(topCPU))
            summaryCard("Filtered", "\(filteredCount)", "of \(total)", .green)
        }
        .padding(10)
        .background(.quaternary.opacity(0.5))
    }

    private func summaryCard(_ title: String, _ value: String, _ sub: String, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.system(.title3, design: .monospaced)).bold().foregroundStyle(color)
            Text(sub).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }

    // MARK: - Filter bar

    private var filterBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Filter by name / project / harness / pid", text: $filterText)
                .textFieldStyle(.plain)
                .filterFieldFocus($filterFocused)
            if !filterText.isEmpty {
                Button { filterText = "" } label: {
                    Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                }
                .buttonStyle(.borderless)
            }
            Divider().frame(height: 16)
            Text("Min RSS \(Int(minRSS)) MB").font(.caption).foregroundStyle(.secondary)
            Slider(value: $minRSS, in: 0...4096, step: 64) {
                Text("Min RSS")
            }
            .frame(width: 160)
            Spacer()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.quaternary.opacity(0.3))
    }

    // MARK: - Bulk action bar

    private var bulkActionBar: some View {
        HStack(spacing: 10) {
            Button(role: .destructive) {
                killGate.request(.selected(selection))
            } label: {
                Label("Kill selected (\(selection.count))", systemImage: "xmark.circle")
            }
            .disabled(selection.isEmpty)

            Button(role: .destructive) {
                killGate.request(.all)
            } label: {
                Label("Kill all", systemImage: "xmark.octagon")
            }
            .tint(.red)

            Spacer()

            Button {
                exportJSON(filtered: filtered)
            } label: {
                Label("Export JSON", systemImage: "square.and.arrow.up")
            }

            Button {
                exportCSV(filtered: filtered)
            } label: {
                Label("Export CSV", systemImage: "tablecells")
            }

            // P2-11: export only the rows currently selected in the Table
            Button {
                exportSelectedJSON()
            } label: {
                Label("Export selected JSON (\(selection.count))", systemImage: "square.and.arrow.down.on.square")
            }
            .disabled(selection.isEmpty)

            Button {
                exportSelectedCSV()
            } label: {
                Label("Export selected CSV (\(selection.count))", systemImage: "tablecells.badge.ellipsis")
            }
            .disabled(selection.isEmpty)

            if !bulkStatus.isEmpty {
                Text(bulkStatus)
                    .font(.caption)
                    .foregroundStyle(bulkStatus.lowercased().contains("error") ? .red : .green)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.quaternary.opacity(0.3))
    }

    // MARK: - Destructive kill (confirmed only)

    /// Performs a kill scope handed back by `killGate.confirm()`. Never called
    /// directly from a button — a `nil` scope (nothing confirmed) is a no-op.
    private func performKill(_ scope: KillScope?) {
        guard let scope else { return }
        switch scope {
        case .selected(let pids):
            Task {
                var killed = 0
                for pid in pids {
                    await state.kill(pid: pid)
                    killed += 1
                }
                bulkStatus = "Killed \(killed) selected"
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                bulkStatus = ""
            }
        case .all:
            Task {
                await state.killAll()
                bulkStatus = "Kill-all requested"
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                bulkStatus = ""
            }
        }
    }

    // MARK: - Export

    private func exportJSON(filtered: [ProcessSummary]) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [UTType.json]
        panel.nameFieldStringValue = "sharecli-processes.json"
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url else { return }

        struct Out: Codable { let exported_at: UInt64; let count: Int; let processes: [ProcessSummary] }
        let payload = Out(
            exported_at: UInt64(Date().timeIntervalSince1970),
            count: filtered.count,
            processes: filtered
        )
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(payload) {
            try? data.write(to: url)
        }
    }

    private func exportCSV(filtered: [ProcessSummary]) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [UTType.commaSeparatedText]
        panel.nameFieldStringValue = "sharecli-processes.csv"
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url else { return }

        var lines: [String] = ["pid,name,memory_mb,project,harness,start_time,age_seconds,cpu_percent"]
        let now = UInt64(Date().timeIntervalSince1970)
        for p in filtered {
            let age = p.start_time > 0 ? (now >= p.start_time ? now - p.start_time : 0) : 0
            let cells: [String] = [
                "\(p.pid)",
                csvEscape(p.name),
                "\(p.memory_mb)",
                csvEscape(p.project ?? ""),
                csvEscape(p.harness ?? ""),
                "\(p.start_time)",
                "\(age)",
                String(format: "%.2f", p.cpu_percent)
            ]
            lines.append(cells.joined(separator: ","))
        }
        let body = lines.joined(separator: "\n") + "\n"
        try? body.write(to: url, atomically: true, encoding: .utf8)
    }

    private func exportSelectedJSON() {
        let sel = selectedRows()
        if sel.isEmpty { return }
        exportJSON(filtered: sel)
    }

    private func exportSelectedCSV() {
        let sel = selectedRows()
        if sel.isEmpty { return }
        exportCSV(filtered: sel)
    }

    private func selectedRows() -> [ProcessSummary] {
        // `selection` is Set<ProcessSummary.ID> (UInt32 pid); map back to rows.
        let pidSet = selection
        return filtered.filter { pidSet.contains($0.pid) }
    }

    private func csvEscape(_ s: String) -> String {
        if s.contains(",") || s.contains("\"") || s.contains("\n") {
            return "\"" + s.replacingOccurrences(of: "\"", with: "\"\"") + "\""
        }
        return s
    }

    // MARK: - Helpers

    private func formatAge(_ startTime: UInt64) -> String {
        guard startTime > 0 else { return "—" }
        let now = UInt64(Date().timeIntervalSince1970)
        guard now >= startTime else { return "?" }
        let secs = now - startTime
        if secs < 60 { return "\(secs)s" }
        if secs < 3600 { return "\(secs / 60)m" }
        if secs < 86400 { return "\(secs / 3600)h \(secs % 3600 / 60)m" }
        return "\(secs / 86400)d"
    }

    private func rssColor(_ mb: UInt64) -> Color {
        if mb > 1024 { return .red }
        if mb > 512 { return .orange }
        if mb > 128 { return .yellow }
        return .primary
    }

    private func cpuColor(_ pct: Float) -> Color {
        if pct > 90 { return .red }
        if pct > 60 { return .orange }
        if pct > 25 { return .yellow }
        return .secondary
    }

    private func cpuBar(_ pct: Float) -> some View {
        let width = max(0, min(1, pct / 100.0))
        return GeometryReader { geo in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(.quaternary)
                RoundedRectangle(cornerRadius: 2)
                    .fill(cpuColor(pct).opacity(0.85))
                    .frame(width: geo.size.width * CGFloat(width))
            }
        }
        .frame(height: 6)
    }

    private func fdColor(_ fd: UInt32) -> Color {
        if fd > 1024 { return .red }
        if fd > 256 { return .orange }
        if fd > 64 { return .yellow }
        return .secondary
    }

    private func fdBar(_ fd: UInt32) -> some View {
        // Map 0..2048 onto 0..1 (log-ish). 2048+ clamps to full bar.
        let n = Double(min(fd, 2048))
        let width = n / 2048.0
        return GeometryReader { geo in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2).fill(.quaternary)
                RoundedRectangle(cornerRadius: 2)
                    .fill(fdColor(fd).opacity(0.85))
                    .frame(width: geo.size.width * CGFloat(width))
            }
        }
        .frame(height: 6)
    }

    private func ioBytes(_ b: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(b), countStyle: .file)
    }
}
