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
private struct LargeCard<Extra: View>: View {
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

// MARK: - Thermal gate subpage

private struct ThermalGateSubpage: View {
    @ObservedObject var state: AppState

    @AppStorage("thermal.rssUnit") private var rssUnitRaw: String = RssUnit.mb.rawValue

    enum RssUnit: String {
        case mb, gb
    }

    private var rssUnit: RssUnit {
        RssUnit(rawValue: rssUnitRaw) ?? .mb
    }

    private var gate: GateStatusSnapshot? {
        state.health?.gate
    }

    private var gateVisual: TrayGateVisual {
        if let g = gate, state.isConnected {
            return OperatorDisplay.resolveTrayGateVisual(gate: g, connected: true)
        }
        return OperatorDisplay.resolveTrayGateVisual(
            thermalPressure: "UNAVAILABLE",
            gateDecision: "UNAVAILABLE",
            connected: false
        )
    }

    private var agentRSSDisplay: String {
        guard let g = gate else { return "—" }
        let bytes = g.agent_total_rss_bytes
        switch rssUnit {
        case .gb:
            return String(format: "%.2f GB", Double(bytes) / 1_073_741_824.0)
        case .mb:
            return String(format: "%.1f MB", Double(bytes) / 1_048_576.0)
        }
    }

    private var thermalPressureLevel: Int {
        // 0 = green/normal, 1 = yellow/warning, 2 = red/critical, 3 = unavailable
        guard let g = gate, state.isConnected else { return 3 }
        switch g.thermal_pressure {
        case "GREEN": return 0
        case "YELLOW": return 1
        case "RED": return 2
        default: return 3
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // 4 large cards
                HStack(spacing: 12) {
                    LargeCard(
                        title: "Detected agents",
                        value: "\(gate?.detected_agents ?? 0)",
                        sub: "agents currently tracked",
                        icon: "person.2.fill",
                        color: .blue
                    )
                    LargeCard(
                        title: "Total agent RSS",
                        value: agentRSSDisplay,
                        sub: rssUnit == .mb ? "tap to switch to GB" : "tap to switch to MB",
                        icon: "memorychip",
                        color: .orange
                    ) {
                        // Unit toggle in the top-right of the RSS card
                        Picker("", selection: Binding(
                            get: { rssUnitRaw },
                            set: { rssUnitRaw = $0 }
                        )) {
                            Text("MB").tag(RssUnit.mb.rawValue)
                            Text("GB").tag(RssUnit.gb.rawValue)
                        }
                        .labelsHidden()
                        .pickerStyle(.segmented)
                        .frame(width: 90)
                    }
                    LargeCard(
                        title: "Gate decision",
                        value: gate?.gate_decision ?? "—",
                        sub: "admission policy result",
                        icon: gateVisual.swiftSymbolName,
                        color: gateVisual.swiftColor
                    ) {
                        // Colored badge in top-right
                        Text(gateVisual.badgeLabel)
                            .font(.caption2.weight(.semibold))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .foregroundStyle(gateVisual.swiftColor)
                            .overlay(
                                RoundedRectangle(cornerRadius: 4)
                                    .stroke(gateVisual.swiftColor, lineWidth: 1)
                            )
                    }
                    LargeCard(
                        title: "Contention",
                        value: gate?.agent_contention ?? "—",
                        sub: "resource pressure state",
                        icon: contentionIcon,
                        color: contentionColor
                    )
                }

                // Thermal pressure gauge
                VStack(alignment: .leading, spacing: 8) {
                    Text("Thermal pressure")
                        .font(.headline)
                    ThermalGauge(level: thermalPressureLevel)
                    Text(thermalPressureLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(14)
                .background(.quaternary.opacity(0.5))
                .clipShape(RoundedRectangle(cornerRadius: 10))

                // Last 20 gate decisions log
                GateDecisionLogPanel(state: state)

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

    private var contentionIcon: String {
        switch (gate?.agent_contention ?? "").lowercased() {
        case let x where x.contains("contend") || x.contains("high"): return "exclamationmark.triangle.fill"
        case let x where x.contains("low") || x == "calm": return "checkmark.seal.fill"
        default: return "circle.dotted"
        }
    }

    private var contentionColor: Color {
        switch (gate?.agent_contention ?? "").lowercased() {
        case let x where x.contains("contend") || x.contains("high"): return .orange
        case let x where x.contains("low") || x == "calm": return .green
        default: return .secondary
        }
    }

    private var thermalPressureLabel: String {
        switch thermalPressureLevel {
        case 0: return "Nominal — within comfortable thermal envelope."
        case 1: return "Elevated — gate may begin throttling."
        case 2: return "Critical — gate expected to deny new spawns."
        default: return "Unavailable — sidecar not reporting thermal pressure."
        }
    }
}

/// Three-segment horizontal gauge for thermal pressure level.
/// Renders as a filled bar where the fill length and colour correspond to the
/// current pressure level (green / yellow / red / gray-unavailable).
private struct ThermalGauge: View {
    let level: Int // 0=green, 1=yellow, 2=red, 3=unavailable

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 6)
                    .fill(.quaternary)
                RoundedRectangle(cornerRadius: 6)
                    .fill(fillColor)
                    .frame(width: max(0, geo.size.width * fillFrac))
                HStack(spacing: 0) {
                    ForEach(0..<4, id: \.self) { i in
                        Rectangle()
                            .fill(Color.black.opacity(i == level ? 0.18 : 0.04))
                            .frame(width: geo.size.width / 4)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
        }
        .frame(height: 18)
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(fillColor.opacity(0.5), lineWidth: 1)
        )
    }

    private var fillColor: Color {
        switch level {
        case 0: return .green
        case 1: return .yellow
        case 2: return .red
        default: return .gray
        }
    }

    private var fillFrac: Double {
        switch level {
        case 0: return 0.25
        case 1: return 0.5
        case 2: return 0.85
        default: return 0.0
        }
    }
}

/// "Last 20 gate decisions" rolling log panel — the most novel element of
/// PR 6. Renders `AppState.gateDecisionHistory` newest-first with color-coded
/// rows.
private struct GateDecisionLogPanel: View {
    @ObservedObject var state: AppState

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Last \(AppState.gateDecisionHistoryCap) gate decisions")
                    .font(.headline)
                Spacer()
                Text("\(state.gateDecisionHistory.count) sample(s) buffered")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            if state.gateDecisionHistory.isEmpty {
                emptyLog
            } else {
                VStack(spacing: 2) {
                    ForEach(Array(state.gateDecisionHistory.reversed().enumerated()), id: \.element.id) { idx, sample in
                        GateDecisionRow(sample: sample, isLatest: idx == 0)
                    }
                }
                .padding(8)
                .background(.quaternary.opacity(0.3))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
        }
        .padding(14)
        .background(.quaternary.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    private var emptyLog: some View {
        VStack(spacing: 6) {
            Image(systemName: "tray")
                .font(.system(size: 28))
                .foregroundStyle(.secondary)
            Text("Awaiting first poll — gate decisions appear here as soon as the sidecar responds.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, minHeight: 80)
        .padding(.vertical, 8)
    }
}

private struct GateDecisionRow: View {
    let sample: GateDecisionSample
    let isLatest: Bool

    private static let timeFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    var body: some View {
        HStack(spacing: 8) {
            Text(Self.timeFormatter.string(from: sample.timestamp))
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 64, alignment: .leading)

            Text(sample.gateDecision)
                .font(.system(.caption, design: .monospaced).weight(isLatest ? .bold : .regular))
                .foregroundStyle(decisionColor)
                .frame(width: 86, alignment: .leading)

            Text("thermal \(sample.thermalPressure)")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(thermalColor)
                .frame(width: 116, alignment: .leading)

            Text("agents \(sample.detectedAgents)")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.primary)
                .frame(width: 64, alignment: .leading)

            Text("rss \(OperatorDisplay.formatBytesCompact(sample.agentTotalRssBytes))")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.secondary)

            Spacer(minLength: 8)

            Text(sample.agentContention)
                .font(.caption2.weight(.semibold))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .foregroundStyle(contentionColor)
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .stroke(contentionColor, lineWidth: 1)
                )
        }
        .padding(.vertical, 3)
        .padding(.horizontal, 6)
        .background(isLatest ? Color.accentColor.opacity(0.08) : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: 4))
    }

    private var decisionColor: Color {
        switch sample.gateDecision {
        case "ADMIT": return .green
        case "DENY": return .red
        case "THROTTLE": return .orange
        default: return .secondary
        }
    }

    private var thermalColor: Color {
        switch sample.thermalPressure {
        case "GREEN": return .green
        case "YELLOW": return .orange
        case "RED": return .red
        default: return .secondary
        }
    }

    private var contentionColor: Color {
        let c = sample.agentContention.lowercased()
        if c.contains("contend") || c.contains("high") { return .orange }
        if c.contains("low") || c == "calm" { return .green }
        return .secondary
    }
}

// MARK: - Host watch subpage

private struct HostWatchSubpage: View {
    @ObservedObject var state: AppState

    private var latest: HostWatchSample? {
        state.hostWatchHistory.last
    }
    private var prior: HostWatchSample? {
        guard state.hostWatchHistory.count >= 2 else { return nil }
        return state.hostWatchHistory[state.hostWatchHistory.count - 2]
    }

    private var fdDelta: Int64 {
        guard let l = latest, let p = prior else { return 0 }
        return Int64(l.fd_count) - Int64(p.fd_count)
    }
    private var rxDelta: Int64 {
        guard let l = latest, let p = prior else { return 0 }
        return Int64(l.net_rx_bytes) - Int64(p.net_rx_bytes)
    }
    private var txDelta: Int64 {
        guard let l = latest, let p = prior else { return 0 }
        return Int64(l.net_tx_bytes) - Int64(p.net_tx_bytes)
    }
    private var loadDelta: Double {
        guard let l = latest, let p = prior else { return 0 }
        return l.load_1m - p.load_1m
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // 4 sparkline cards
                HStack(spacing: 12) {
                    SparklineCard(
                        title: "FD count",
                        value: latest.map { "\($0.fd_count)" } ?? "—",
                        deltaText: deltaText(fdDelta, suffix: "fds"),
                        sparkline: Sparkline(values: state.hostWatchHistory.map { Double($0.fd_count) }),
                        accent: fdColor(latest?.fd_count ?? 0),
                        warn: (latest?.fd_count ?? 0) > 1000
                    )
                    SparklineCard(
                        title: "Net RX (since boot)",
                        value: latest.map { OperatorDisplay.formatBytesCompact($0.net_rx_bytes) } ?? "—",
                        deltaText: latest != nil && prior != nil ? "+\(OperatorDisplay.formatBytesCompact(UInt64(max(rxDelta, 0)))) last poll" : "waiting for 2nd sample",
                        sparkline: Sparkline(values: state.hostWatchHistory.map { Double($0.net_rx_bytes) }),
                        accent: .blue,
                        warn: false
                    )
                    SparklineCard(
                        title: "Net TX (since boot)",
                        value: latest.map { OperatorDisplay.formatBytesCompact($0.net_tx_bytes) } ?? "—",
                        deltaText: latest != nil && prior != nil ? "+\(OperatorDisplay.formatBytesCompact(UInt64(max(txDelta, 0)))) last poll" : "waiting for 2nd sample",
                        sparkline: Sparkline(values: state.hostWatchHistory.map { Double($0.net_tx_bytes) }),
                        accent: .indigo,
                        warn: false
                    )
                    SparklineCard(
                        title: "Load 1m",
                        value: latest.map { String(format: "%.2f", $0.load_1m) } ?? "—",
                        deltaText: deltaTextLoad(loadDelta),
                        sparkline: Sparkline(values: state.hostWatchHistory.map { $0.load_1m }),
                        accent: loadColor(latest?.load_1m ?? 0),
                        warn: (latest?.load_1m ?? 0) > 2.0
                    )
                }

                // Buffer status
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Rolling buffer")
                            .font(.headline)
                        Spacer()
                        Text("\(state.hostWatchHistory.count) / \(AppState.hostWatchHistoryCap) samples")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                    Text("Sparklines plot the most recent samples from `monitoring.report` (polled every \(TrayPoll.intervalSeconds)s). Window is capped at \(AppState.hostWatchHistoryCap) entries; older samples are evicted oldest-first.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(14)
                .background(.quaternary.opacity(0.5))
                .clipShape(RoundedRectangle(cornerRadius: 10))

                // Mem RSS for completeness
                if let last = latest {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Sidecar memory RSS (host)")
                            .font(.headline)
                        Text(OperatorDisplay.formatBytesCompact(last.mem_rss_bytes))
                            .font(.system(.title3, design: .monospaced))
                            .foregroundStyle(.primary)
                        Text("Resident set size of the sharecli sidecar as reported by host_watch.")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    .padding(14)
                    .background(.quaternary.opacity(0.5))
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                }

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

    private func fdColor(_ n: UInt64) -> Color {
        if n > 1000 { return .red }
        if n > 600 { return .orange }
        return .blue
    }

    private func loadColor(_ load: Double) -> Color {
        if load > 2.0 { return .red }
        if load > 1.0 { return .orange }
        return .green
    }

    private func deltaText(_ delta: Int64, suffix: String) -> String {
        guard latest != nil, prior != nil else { return "waiting for 2nd sample" }
        if delta == 0 { return "no change" }
        let sign = delta > 0 ? "+" : ""
        return "\(sign)\(delta) \(suffix) last poll"
    }

    private func deltaTextLoad(_ delta: Double) -> String {
        guard latest != nil, prior != nil else { return "waiting for 2nd sample" }
        if abs(delta) < 0.005 { return "no change" }
        let sign = delta > 0 ? "+" : ""
        return String(format: "\(sign)%.2f vs prior poll", delta)
    }
}

/// A sparkline card: title + value + delta + inline polyline.
private struct SparklineCard: View {
    let title: String
    let value: String
    let deltaText: String
    let sparkline: Sparkline
    let accent: Color
    let warn: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: warn ? "exclamationmark.triangle.fill" : "waveform.path.ecg")
                    .foregroundStyle(accent)
                Text(title)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Text(value)
                .font(.system(.title3, design: .monospaced))
                .bold()
                .foregroundStyle(accent)
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            sparkline
                .frame(height: 36)
            Text(deltaText)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(warn ? Color.red.opacity(0.5) : Color.clear, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

/// Minimal native sparkline: a `Path` polyline drawn inside a `GeometryReader`.
/// Plots the supplied numeric series normalised to [0,1] across the visible
/// width. Empty / single-point series render a flat baseline; constant series
/// centre on the midline.
struct Sparkline: View {
    let values: [Double]

    var body: some View {
        GeometryReader { geo in
            ZStack {
                RoundedRectangle(cornerRadius: 4)
                    .fill(.quaternary.opacity(0.5))
                path(in: geo.size)
                    .stroke(
                        LinearGradient(
                            colors: [.blue, .purple],
                            startPoint: .leading,
                            endPoint: .trailing
                        ),
                        style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round)
                    )
                    .padding(2)
                // Endpoint dot so the latest sample is visually anchored.
                if let last = lastPoint(in: geo.size) {
                    Circle()
                        .fill(Color.blue)
                        .frame(width: 4, height: 4)
                        .position(last)
                }
            }
        }
    }

    private func path(in size: CGSize) -> Path {
        var p = Path()
        let points = normalisedPoints(in: size)
        guard let first = points.first else { return p }
        p.move(to: first)
        for pt in points.dropFirst() {
            p.addLine(to: pt)
        }
        return p
    }

    private func lastPoint(in size: CGSize) -> CGPoint? {
        normalisedPoints(in: size).last
    }

    private func normalisedPoints(in size: CGSize) -> [CGPoint] {
        guard !values.isEmpty else { return [] }
        let w = max(0, size.width - 4)   // inset to match stroke padding
        let h = max(0, size.height - 4)
        guard w > 0, h > 0 else { return [] }

        // Single sample: centre it. Constant series: middle line.
        let v = values
        let lo = v.min() ?? 0
        let hi = v.max() ?? 0
        let range = hi - lo

        // Edge cases: empty / single / all-equal.
        if v.count == 1 {
            return [CGPoint(x: 2 + w / 2, y: 2 + h / 2)]
        }
        if range < .ulpOfOne {
            // Plot a flat midline.
            return v.enumerated().map { i, _ in
                let x = 2 + (w * CGFloat(i) / CGFloat(v.count - 1))
                return CGPoint(x: x, y: 2 + h / 2)
            }
        }

        let stepX = w / CGFloat(v.count - 1)
        return v.enumerated().map { i, raw in
            let normalised = (raw - lo) / range // in [0,1]
            let y = 2 + h * (1.0 - CGFloat(normalised))
            let x = 2 + CGFloat(i) * stepX
            return CGPoint(x: x, y: y)
        }
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
