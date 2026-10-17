/// HealthHostWatch.swift — extracted host watch subpage from HealthPage.
///
/// Contains: HostWatchSubpage, SparklineCard, Sparkline.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore

// MARK: - Host watch subpage

struct HostWatchSubpage: View {
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
struct SparklineCard: View {
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
