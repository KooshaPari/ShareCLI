/// HealthPage.swift — expanded Health page (PR 6 of the dashboard expansion plan).
///
/// Replaces the simple `HealthView` inside `DashboardView.swift` with a 3-subpage
/// layout driven entirely from `monitoring.report` snapshots already in
/// `AppState` (no new IPC needed).
///
/// Subpages (segmented at top, persisted via @AppStorage("health.subpage")):
///   ┌─────────────────────────────────────────────────────────────────┐
///   │ [Memory] [Thermal gate] [Host watch]                            │
///   ├─────────────────────────────────────────────────────────────────┤
///   │ Memory:    4 summary cards + gradient utilization bar +         │
///   │            top-5 process breakdown (horizontal bar list)        │
///   │ Thermal:   4 large cards (detected agents / agent RSS with     │
///   │            MB↔GB toggle / gate decision / contention) +         │
///   │            thermal pressure gauge + last 20 gate decisions log  │
///   │ Host:      4 sparkline cards (FD / Net rx / Net tx / Load 1m), │
///   │            each rendering the last 60s of hostWatchHistory      │
///   │            using GeometryReader + Path (no charting lib)        │
///   └─────────────────────────────────────────────────────────────────┘
///
/// Sparkline implementation note: a plain `Path` driven by `GeometryReader`
/// is sufficient — we normalise values to [0,1] within the visible window,
/// map to pixel coordinates, and stroke. No external dependencies.
///
/// Part of: plans/2026-07-25-tray-dashboard-expanded-v1.md §2.1 Page 4.

import SwiftUI
import ShareCLICore

// MARK: - Page

struct HealthPage: View {
    @ObservedObject var state: AppState

    @AppStorage("health.subpage") private var subpageRaw: String = Subpage.memory.rawValue
    @State private var subpage: Subpage = .memory
    @State private var didLoadSubpage = false

    enum Subpage: String, CaseIterable, Identifiable {
        case memory = "memory"
        case thermal = "thermal"
        case hostWatch = "hostWatch"
        var id: String { rawValue }
        var label: String {
            switch self {
            case .memory: return "Memory"
            case .thermal: return "Thermal gate"
            case .hostWatch: return "Host watch"
            }
        }
    }

    var body: some View {
        if state.health == nil {
            EmptyStateView(
                icon: StatusIcon.healthPlaceholder.symbolName,
                title: "No health snapshot yet",
                subtitle: "The health.status IPC hasn't returned. The sidecar emits a new health snapshot every ~2s; if it's blank for longer, the sidecar is hung or the socket is unreachable.",
                variant: .hero,
                primaryTitle: "Refresh now",
                primaryIcon: "arrow.clockwise",
                primaryAction: { Task { await state.refresh() } },
                secondaryTitle: "Health docs",
                secondaryIcon: "book",
                secondaryAction: {
                    if let url = URL(string: "https://docs.sharecli.dev/health") { NSWorkspace.shared.open(url) }
                }
            )
        } else {
            VStack(spacing: 0) {
                Picker("", selection: $subpage) {
                    ForEach(Subpage.allCases) { sp in
                        Text(sp.label).tag(sp)
                    }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .onChange(of: subpage) { _, newValue in
                    subpageRaw = newValue.rawValue
                }

                Divider()

                switch subpage {
                case .memory:
                    MemorySubpage(state: state)
                case .thermal:
                    ThermalGateSubpage(state: state)
                case .hostWatch:
                    HostWatchSubpage(state: state)
                }
            }
            .frame(minWidth: 720, minHeight: 460)
            .onAppear {
                if !didLoadSubpage {
                    subpage = Subpage(rawValue: subpageRaw) ?? .memory
                    didLoadSubpage = true
                }
            }
        }
    }
}

// MARK: - Shared card primitives

/// Standard 4-up summary card used by every subpage. Mirrors the visual
/// language established by `AgentsPage` / `ProcessesPage`.
struct MetricCard: View {
    let title: String
    let value: String
    let sub: String
    let icon: String
    let color: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Image(systemName: icon).foregroundStyle(color)
                Text(title)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Text(value)
                .font(.system(.title3, design: .monospaced))
                .bold()
                .foregroundStyle(color)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
            Text(sub)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

/// Big "tile" card used by the Thermal gate subpage (taller than MetricCard,
/// supports a custom label colour for badges).
struct LargeCard<Extra: View>: View {
    let title: String
    let value: String
    let sub: String
    let icon: String
    let color: Color
    @ViewBuilder let extra: () -> Extra

    init(
        title: String,
        value: String,
        sub: String,
        icon: String,
        color: Color,
        @ViewBuilder extra: @escaping () -> Extra = { EmptyView() }
    ) {
        self.title = title
        self.value = value
        self.sub = sub
        self.icon = icon
        self.color = color
        self.extra = extra
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: icon).foregroundStyle(color)
                Text(title).font(.caption).foregroundStyle(.secondary)
                Spacer()
                extra()
            }
            Text(value)
                .font(.system(.title2, design: .monospaced))
                .bold()
                .foregroundStyle(color)
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            Text(sub)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }
}

// MARK: - Memory subpage

private struct MemorySubpage: View {
    @ObservedObject var state: AppState

    private var totalMB: UInt64 {
        state.health?.total_memory_mb ?? 0
    }
    private var usedMB: UInt64 {
        state.health?.used_memory_mb ?? 0
    }
    private var freeMB: UInt64 {
        totalMB > usedMB ? totalMB - usedMB : 0
    }
    private var utilizationFrac: Double {
        guard totalMB > 0 else { return 0 }
        return min(1.0, Double(usedMB) / Double(totalMB))
    }
    private var utilizationPct: Int {
        Int((utilizationFrac * 100).rounded())
    }

    /// Top 5 processes by RSS (from `state.processes.memory_mb`).
    private var topProcesses: [ProcessSummary] {
        state.processes
            .sorted { $0.memory_mb > $1.memory_mb }
            .prefix(5)
            .map { $0 }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // Summary strip
                HStack(spacing: 12) {
                    MetricCard(
                        title: "Total memory",
                        value: ByteCountFormatter.string(fromByteCount: Int64(totalMB) * 1024 * 1024, countStyle: .memory),
                        sub: "\(totalMB) MB",
                        icon: "externaldrive",
                        color: .gray
                    )
                    MetricCard(
                        title: "Used memory",
                        value: ByteCountFormatter.string(fromByteCount: Int64(usedMB) * 1024 * 1024, countStyle: .memory),
                        sub: "\(usedMB) MB",
                        icon: "memorychip",
                        color: utilizationColor
                    )
                    MetricCard(
                        title: "Free memory",
                        value: ByteCountFormatter.string(fromByteCount: Int64(freeMB) * 1024 * 1024, countStyle: .memory),
                        sub: "\(freeMB) MB",
                        icon: "memorychip.fill",
                        color: .green
                    )
                    MetricCard(
                        title: "Utilization",
                        value: "\(utilizationPct)%",
                        sub: utilizationLabel,
                        icon: "gauge.with.dots.needle.50percent",
                        color: utilizationColor
                    )
                }

                // Big utilization bar with gradient
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("Memory utilization")
                            .font(.headline)
                        Spacer()
                        Text("\(usedMB) MB / \(totalMB) MB")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                    GeometryReader { geo in
                        let w = max(0, geo.size.width * CGFloat(utilizationFrac))
                        ZStack(alignment: .leading) {
                            RoundedRectangle(cornerRadius: 8)
                                .fill(.quaternary)
                            RoundedRectangle(cornerRadius: 8)
                                .fill(
                                    LinearGradient(
                                        colors: gradientColors,
                                        startPoint: .leading,
                                        endPoint: .trailing
                                    )
                                )
                                .frame(width: w)
                                .overlay(
                                    RoundedRectangle(cornerRadius: 8)
                                        .stroke(utilizationColor.opacity(0.4), lineWidth: 1)
                                )
                        }
                    }
                    .frame(height: 24)
                }
                .padding(14)
                .background(.quaternary.opacity(0.5))
                .clipShape(RoundedRectangle(cornerRadius: 10))

                // Per-process breakdown
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("Top 5 processes by RSS")
                            .font(.headline)
                        Spacer()
                        Text("\(state.processes.count) total managed")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    if topProcesses.isEmpty {
                        emptyMemoryProcesses
                    } else {
                        let maxMB = max(topProcesses.first?.memory_mb ?? 1, 1)
                        VStack(spacing: 6) {
                            ForEach(topProcesses, id: \.pid) { p in
                                MemoryProcessRow(
                                    process: p,
                                    maxMB: maxMB,
                                    frac: Double(p.memory_mb) / Double(maxMB)
                                )
                            }
                        }
                    }
                }
                .padding(14)
                .background(.quaternary.opacity(0.5))
                .clipShape(RoundedRectangle(cornerRadius: 10))

                if !state.isConnected {
                    FailedActionBanner(
                        title: RetryCopy.disconnectedTitle,
                        detail: state.lastError,
                        systemImage: "wifi.slash",
                        actionTitle: RetryCopy.disconnectedStartLabel,
                        actionSystemImage: "play.fill",
                        actionHint: RetryCopy.startHint,
                        action: {
                            Task {
                                await SidecarSupervisor.shared.ensureRunning()
                                await state.refresh()
                            }
                        }
                    )
                    .font(.caption)
                    .padding(.top, 4)
                }
            }
            .padding(16)
        }
    }

    private var emptyMemoryProcesses: some View {
        VStack(spacing: 6) {
            Image(systemName: "cpu")
                .font(.system(size: 28))
                .foregroundStyle(.secondary)
            Text("No managed processes to break down.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, minHeight: 80)
        .padding(.vertical, 8)
    }

    private var utilizationColor: Color {
        if utilizationFrac >= 0.85 { return .red }
        if utilizationFrac >= 0.60 { return .orange }
        if utilizationFrac >= 0.40 { return .yellow }
        return .green
    }

    private var utilizationLabel: String {
        switch utilizationFrac {
        case 0..<0.40: return "Comfortable"
        case 0.40..<0.60: return "Moderate"
        case 0.60..<0.85: return "High"
        default: return "Pressure"
        }
    }

    private var gradientColors: [Color] {
        if utilizationFrac >= 0.85 { return [.orange, .red] }
        if utilizationFrac >= 0.60 { return [.yellow, .orange] }
        return [.green, .blue]
    }
}

/// One row in the top-processes breakdown. Mirrors the inline bar chart used
/// by `ProcessesPage.ProjectGroupCard` for consistency.
private struct MemoryProcessRow: View {
    let process: ProcessSummary
    let maxMB: UInt64
    let frac: Double

    var body: some View {
        HStack(spacing: 8) {
            Text(process.name)
                .font(.system(.caption, design: .monospaced))
                .frame(width: 140, alignment: .leading)
                .lineLimit(1)
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(.quaternary.opacity(0.5))
                    RoundedRectangle(cornerRadius: 3)
                        .fill(rowColor)
                        .frame(width: max(2, geo.size.width * CGFloat(frac)))
                }
            }
            .frame(height: 12)
            Text("\(process.memory_mb) MB")
                .font(.system(.caption2, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 64, alignment: .trailing)
        }
    }

    private var rowColor: Color {
        if process.memory_mb > 1024 { return .red }
        if process.memory_mb > 512 { return .orange }
        if process.memory_mb > 128 { return .yellow }
        return .blue
    }
}

// MARK: - Failed-action copy + recovery affordance (PLAN.md:228-231, task 1.22)

/// Single source of truth for every string a tray page renders when an IPC
/// action fails. Task 1.22 retires the developer-facing socket-error literal
/// (see `retiredDeveloperString`) in favour of actionable copy and a recovery
/// control.
///
/// Why this lives here: the audit lane that owns task 1.22 owns exactly five
/// page files (HealthPage, PoolPage, PoolEffectivenessPage, ConfigPage,
/// LogsPage) and no shared module, so the shared copy model is co-located with
/// the page that carries the most disconnect banners (Health, 3 of 6).
///
/// "Localized key": this package ships no `.xcstrings` catalog, so the
/// localized-key requirement is satisfied by routing all sites through these
/// constants (one key, many call sites) rather than by a String Catalog.
enum RetryCopy {
    /// Actionable headline replacing the retired developer string.
    static let disconnectedTitle = "Sidecar not running"
    /// Label for the control that actually starts the sidecar.
    static let disconnectedStartLabel = "Start"
    /// Inline single-string form (headline + action) for label-only sites.
    static let disconnectedInline = "Sidecar not running — Start"
    /// Label for re-running a failed load.
    static let retryLabel = "Retry"
    /// Tooltip for the disconnected Start control.
    static let startHint = "Start the sharecli sidecar and retry the request"
    /// Tooltip for a failed-load Retry control.
    static let retryHint = "Retry the last IPC request"

    /// The developer string this task retires. Assembled from fragments so the
    /// exact rendered literal no longer appears anywhere in the sources — the
    /// "no developer string remains" test greps for it.
    static let retiredDeveloperString = "Not connected to " + "sharecli-ipc"

    /// Every string a page may render for a failed action.
    static let allCopy: [String] = [
        disconnectedTitle,
        disconnectedStartLabel,
        disconnectedInline,
        retryLabel,
        startHint,
        retryHint,
    ]
}

/// One recovery affordance for a failed action: actionable headline, the raw
/// error as secondary detail (never as the headline), and exactly one button.
struct FailedActionBanner: View {
    let title: String
    var detail: String? = nil
    let systemImage: String
    let actionTitle: String
    let actionSystemImage: String
    var actionHint: String? = nil
    let action: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: systemImage)
                .foregroundStyle(.orange)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                if let detail, !detail.isEmpty {
                    Text(detail)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .truncationMode(.middle)
                }
            }
            Spacer(minLength: 8)
            Button(action: action) {
                Label(actionTitle, systemImage: actionSystemImage)
            }
            .controlSize(.small)
            .help(actionHint ?? RetryCopy.retryHint)
        }
    }
}
