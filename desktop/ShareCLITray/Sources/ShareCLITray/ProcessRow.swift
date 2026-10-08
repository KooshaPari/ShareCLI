/// ProcessRow.swift — extracted grouped row views from ProcessesPage.
///
/// Contains: ProcessGroup, ProjectGroupCard, ProcessRowInline.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore

// MARK: - ProcessGroup data

struct ProcessGroup: Hashable {
    let label: String
    let members: [ProcessSummary]
    let totalRSSBytes: UInt64
    let topHarness: String
    let uniqueHarnessCount: Int
}

// MARK: - ProjectGroupCard (renders one grouped section)

struct ProjectGroupCard: View {
    let group: ProcessGroup
    let keyLabel: String
    let kill: (UInt32) -> Void
    @State private var expanded = true
    /// Per-row kill confirmation gate (closure batch A2 followup). Routes
    /// the inline `ProcessRowInline` kill button through a confirmation
    /// dialog so the destructive action cannot fire silently.
    @State private var killGate = DestructiveKillGate()
    /// Reduce motion (PLAN 1.23): gates the expand/collapse animation.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(keyLabel).font(.caption2).foregroundStyle(.secondary)
                        Text(group.label)
                            .font(.system(.headline, design: .monospaced))
                            .bold()
                    }
                    HStack(spacing: 12) {
                        Label("\(group.members.count)", systemImage: "cpu")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Label(
                            ByteCountFormatter.string(fromByteCount: Int64(group.totalRSSBytes), countStyle: .memory),
                            systemImage: "memorychip"
                        )
                        .font(.caption)
                        .foregroundStyle(.orange)
                        Label("top: \(group.topHarness)", systemImage: "tag")
                            .font(.caption)
                            .foregroundStyle(.purple)
                        if group.uniqueHarnessCount > 1 {
                            Text("+\(group.uniqueHarnessCount - 1) more harness")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Spacer()
                Button {
                    Motion.run(.easeInOut(duration: 0.15), reduceMotion: reduceMotion) { expanded.toggle() }
                } label: {
                    Image(systemName: expanded ? "chevron.up" : "chevron.down")
                }
                .buttonStyle(.borderless)
            }

            // RSS bar — relative to max group in this set
            if expanded {
                Divider()
                rssBarChart
                VStack(spacing: 4) {
                    ForEach(group.members.prefix(5), id: \.pid) { p in
                        ProcessRowInline(p: p, kill: { pid in killGate.request(.selected([pid])) })
                    }
                    if group.members.count > 5 {
                        Text("+\(group.members.count - 5) more (sorted by RSS desc)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
        }
        .padding(12)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 10))
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

    /// Executes the kill for the given scope. `nil` (nothing confirmed) is a
    /// no-op so the inline row kill button can never fire without the dialog.
    private func performKill(_ scope: KillScope?) {
        guard let scope else { return }
        switch scope {
        case .selected(let pids):
            for pid in pids {
                kill(pid)
            }
        case .all:
            for p in group.members {
                kill(p.pid)
            }
        }
    }

    private var rssBarChart: some View {
        let maxBytes = group.members.map { $0.memory_mb }.max() ?? 1
        return VStack(spacing: 2) {
            ForEach(group.members.prefix(8), id: \.pid) { p in
                let frac = Double(p.memory_mb) / Double(max(maxBytes, 1))
                HStack(spacing: 6) {
                    Text(p.name)
                        .font(.system(.caption, design: .monospaced))
                        .frame(width: 110, alignment: .leading)
                        .lineLimit(1)
                    GeometryReader { geo in
                        ZStack(alignment: .leading) {
                            RoundedRectangle(cornerRadius: 3).fill(.quaternary.opacity(0.5))
                            RoundedRectangle(cornerRadius: 3)
                                .fill(rssBarColor(p.memory_mb))
                                .frame(width: max(2, geo.size.width * CGFloat(frac)))
                        }
                    }
                    .frame(height: 10)
                    Text("\(p.memory_mb) MB")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .frame(width: 60, alignment: .trailing)
                }
            }
        }
    }

    private func rssBarColor(_ mb: UInt64) -> Color {
        if mb > 1024 { return .red }
        if mb > 512 { return .orange }
        if mb > 128 { return .yellow }
        return .blue
    }
}

struct ProcessRowInline: View {
    let p: ProcessSummary
    let kill: (UInt32) -> Void
    var body: some View {
        HStack(spacing: 8) {
            Text("\(p.pid)").font(.system(.caption, design: .monospaced)).frame(width: 50, alignment: .leading)
            Text(p.name).font(.system(.caption, design: .monospaced)).frame(width: 140, alignment: .leading).lineLimit(1)
            if let proj = p.project { Badge(text: proj, color: .blue) }
            if let h = p.harness { Badge(text: h, color: .purple) }
            Spacer()
            Text("\(p.memory_mb) MB")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.secondary)
            Button {
                kill(p.pid)
            } label: {
                Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
            }
            .buttonStyle(.borderless)
            .help("Kill PID \(p.pid)")
        }
    }
}
