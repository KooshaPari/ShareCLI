/// PoolEffectivenessPage.swift — Hypervisor coalesce cache + SlotQueue metrics
/// (PR 4 of dashboard expansion plan).
///
/// Pulls live snapshots via the new `pool.effectiveness` IPC method
/// (added in `crates/sharecli-ipc/src/handler.rs`). The page renders:
///
///   ┌────────────────────────────────────────────────────────────┐
///   │ Summary strip (Hits · Misses · Nocache · Hit Rate · ···)  │
///   ├──────────────────────────────┬─────────────────────────────┤
///   │ Coalesce cache panel         │ SlotQueue panel             │
///   │  • hits / misses / nocache   │  • acquires / waits / timeouts│
///   │  • hit-rate gauge (gradient) │  • contention gauge         │
///   │  • hit-rate sparkline        │  • acquires sparkline       │
///   │  • 4-up stat strip           │  • 4-up stat strip          │
///   ├──────────────────────────────┴─────────────────────────────┤
///   │ Glossary (what each metric means)                         │
///   └────────────────────────────────────────────────────────────┘
///
/// Sparkline implementation reuses `Sparkline` from `HealthPage`.

import SwiftUI
import ShareCLICore

struct PoolEffectivenessPage: View {
    @ObservedObject var state: AppState

    @AppStorage("poolEff.lastRefresh") private var lastRefreshRaw: Double = 0
    @State private var nowTick: Date = Date()
    @State private var tickTimer: Timer?

    var body: some View {
        if state.poolEffectiveness == nil {
            EmptyStateView(
                icon: "chart.line.uptrend.xyaxis",
                title: "No effectiveness samples yet",
                subtitle: "Pool effectiveness is reported by the sharecli-fleet hypervisor. Once any worker has made at least one acquire/release/evict, the hit-rate and queue-wait counters will populate here in 5-second sample windows.",
                variant: .hero,
                primaryTitle: "Refresh now",
                primaryIcon: "arrow.clockwise",
                primaryAction: { Task { await state.refresh() } },
                secondaryTitle: "What is this?",
                secondaryIcon: "questionmark.circle",
                secondaryAction: {
                    if let url = URL(string: "https://docs.sharecli.dev/effectiveness") { NSWorkspace.shared.open(url) }
                }
            )
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    summaryStrip
                    HStack(alignment: .top, spacing: 16) {
                        coalescePanel
                        slotQueuePanel
                    }
                    glossaryPanel
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
                    }
                }
                .padding(16)
            }
            .frame(minWidth: 720, minHeight: 460)
            .onAppear { startTick() }
            .onDisappear { stopTick() }
        }
    }

    // MARK: - Summary strip

    private var summaryStrip: some View {
        let eff = state.poolEffectiveness
        let c = eff?.coalesce
        let q = eff?.slot_queue
        return HStack(spacing: 12) {
            MetricCell(
                title: "Cache hits",
                value: c.map { "\($0.hits)" } ?? "—",
                sub: "since sidecar start",
                color: .green,
                icon: "checkmark.icloud.fill"
            )
            MetricCell(
                title: "Cache misses",
                value: c.map { "\($0.misses)" } ?? "—",
                sub: "fell through to command",
                color: .orange,
                icon: "icloud.slash.fill"
            )
            MetricCell(
                title: "Hit rate",
                value: c.map { String(format: "%.1f%%", $0.hitRatePct) } ?? "—",
                sub: "hits / (hits + misses)",
                color: hitRateColor(c?.hitRatePct ?? 0),
                icon: "chart.line.uptrend.xyaxis"
            )
            MetricCell(
                title: "Nocache runs",
                value: c.map { "\($0.nocache_runs)" } ?? "—",
                sub: "mutating argv bypass",
                color: .purple,
                icon: "bolt.fill"
            )
            MetricCell(
                title: "Slot acquires",
                value: q.map { "\($0.acquires)" } ?? "—",
                sub: q.map { "\($0.timeouts) timeout(s)" } ?? "—",
                color: q.map { $0.timeouts > 0 ? .red : .blue } ?? .secondary,
                icon: "rectangle.connected.to.line.below"
            )
        }
        .padding(12)
        .background(.quaternary.opacity(0.5))
    }

    // MARK: - Coalesce cache panel

    private var coalescePanel: some View {
        VStack(alignment: .leading, spacing: 10) {
            panelHeader(
                title: "Coalesce cache",
                subtitle: "Hypervisor pre-lock cache (FR-008 / AC-008.11)",
                icon: "square.stack.3d.up.fill",
                color: .blue
            )
            let c = state.poolEffectiveness?.coalesce
            MetricCard(
                title: "Hits",
                value: c.map { "\($0.hits)" } ?? "—",
                sub: "served from cache",
                icon: "checkmark.icloud.fill",
                color: .green
            )
            MetricCard(
                title: "Misses",
                value: c.map { "\($0.misses)" } ?? "—",
                sub: "executed underlying cmd once",
                icon: "icloud.slash.fill",
                color: .orange
            )
            MetricCard(
                title: "Nocache",
                value: c.map { "\($0.nocache_runs)" } ?? "—",
                sub: "bypass queue",
                icon: "bolt.fill",
                color: .purple
            )
            MetricCard(
                title: "Hit rate",
                value: c.map { String(format: "%.1f%%", $0.hitRatePct) } ?? "—",
                sub: "100% = all served from cache",
                icon: "chart.bar.xaxis",
                color: hitRateColor(c?.hitRatePct ?? 0)
            )

            // Hit rate gauge bar
            VStack(alignment: .leading, spacing: 4) {
                Text("Hit rate").font(.caption).foregroundStyle(.secondary)
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        RoundedRectangle(cornerRadius: 4).fill(.quaternary)
                        RoundedRectangle(cornerRadius: 4)
                            .fill(
                                LinearGradient(
                                    colors: [.red, .orange, .yellow, .green],
                                    startPoint: .leading,
                                    endPoint: .trailing
                                )
                            )
                            .frame(width: Swift.max(0, geo.size.width * CGFloat((c?.hitRatePct ?? 0) / 100.0)))
                        Text(c.map { String(format: "%.1f%%", $0.hitRatePct) } ?? "—")
                            .font(.system(.caption2, design: .monospaced).bold())
                            .foregroundStyle(.white)
                            .padding(.horizontal, 8)
                    }
                }
                .frame(height: 20)
            }

            // Sparkline
            VStack(alignment: .leading, spacing: 2) {
                Text("Hit rate over time").font(.caption).foregroundStyle(.secondary)
                Sparkline(values: state.poolEffectivenessHistory.map { $0.coalesce.hitRatePct })
                    .frame(height: 32)
                Text("\(state.poolEffectivenessHistory.count) sample(s) buffered (cap \(AppState.poolEffectivenessHistoryCap))")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(14)
        .background(.quaternary.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    // MARK: - SlotQueue panel

    private var slotQueuePanel: some View {
        VStack(alignment: .leading, spacing: 10) {
            panelHeader(
                title: "Slot queue",
                subtitle: "Hypervisor SlotQueue acquire / contention (FR-008 / AC-008.12)",
                icon: "rectangle.connected.to.line.below",
                color: .indigo
            )
            let q = state.poolEffectiveness?.slot_queue
            MetricCard(
                title: "Acquires",
                value: q.map { "\($0.acquires)" } ?? "—",
                sub: "closures ran",
                icon: "play.fill",
                color: .blue
            )
            MetricCard(
                title: "Waits",
                value: q.map { "\($0.waits)" } ?? "—",
                sub: "wait-loop iterations",
                icon: "hourglass",
                color: .orange
            )
            MetricCard(
                title: "Timeouts",
                value: q.map { "\($0.timeouts)" } ?? "—",
                sub: "exceeded queue timeout",
                icon: "xmark.octagon.fill",
                color: q.map { $0.timeouts > 0 ? .red : .secondary } ?? .secondary
            )
            MetricCard(
                title: "Wait ratio",
                value: waitRatioString,
                sub: "waits / acquires (lower = less contention)",
                icon: "chart.bar.xaxis.negative",
                color: waitRatioColor
            )

            // Acquire sparkline
            VStack(alignment: .leading, spacing: 2) {
                Text("Acquires over time").font(.caption).foregroundStyle(.secondary)
                Sparkline(values: state.poolEffectivenessHistory.map { Double($0.slot_queue.acquires) })
                    .frame(height: 32)
                Text("\(state.poolEffectivenessHistory.count) sample(s) buffered")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(14)
        .background(.quaternary.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    // MARK: - Glossary

    private var glossaryPanel: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Glossary").font(.headline)
            glossaryRow("Hits", "Cache lookups that returned a result without re-running the underlying command.")
            glossaryRow("Misses", "Cache lookups that fell through to the underlying command (which ran exactly once).")
            glossaryRow("Nocache runs", "Mutating argv routed through the nocache queue bypass (intentional, by design).")
            glossaryRow("Hit rate", "Hits / (hits + misses) × 100%. 100% means every coalesceable call was served from cache.")
            glossaryRow("Acquires", "Slot acquisitions where `with_slot` ran the closure.")
            glossaryRow("Waits", "Wait-loop iterations while a slot was unavailable (contention proxy).")
            glossaryRow("Timeouts", "Acquires that exceeded the queue timeout — should normally be 0 in steady state.")
        }
        .padding(14)
        .background(.quaternary.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    private func glossaryRow(_ key: String, _ desc: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Text(key)
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundStyle(.primary)
                .frame(width: 100, alignment: .leading)
            Text(desc)
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Helpers

    private func panelHeader(title: String, subtitle: String, icon: String, color: Color) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Image(systemName: icon).foregroundStyle(color)
            VStack(alignment: .leading, spacing: 0) {
                Text(title).font(.headline)
                Text(subtitle).font(.caption2).foregroundStyle(.secondary)
            }
        }
    }

    private var waitRatioString: String {
        guard let q = state.poolEffectiveness?.slot_queue, q.acquires > 0 else { return "—" }
        let r = Double(q.waits) / Double(q.acquires)
        return String(format: "%.2f", r)
    }

    private var waitRatioColor: Color {
        guard let q = state.poolEffectiveness?.slot_queue, q.acquires > 0 else { return .secondary }
        let r = Double(q.waits) / Double(q.acquires)
        if r > 1.0 { return .red }
        if r > 0.5 { return .orange }
        if r > 0.1 { return .yellow }
        return .green
    }

    private func hitRateColor(_ pct: Double) -> Color {
        if pct >= 90 { return .green }
        if pct >= 70 { return .yellow }
        if pct >= 40 { return .orange }
        return .red
    }

    private func startTick() {
        // 1Hz tick so the "n seconds since sample" sub-label refreshes.
        stopTick()
        tickTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
            DispatchQueue.main.async { nowTick = Date() }
        }
    }

    private func stopTick() {
        tickTimer?.invalidate()
        tickTimer = nil
    }
}