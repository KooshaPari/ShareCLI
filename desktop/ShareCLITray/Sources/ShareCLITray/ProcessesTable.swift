/// ProcessesTable.swift — extracted tree and trends views from ProcessesPage.
///
/// Contains: TreeNode, ProcessesTreeCanvasView, TrendsView, TrendStats, TrendChartCard.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore

// MARK: - Tree subpage

/// In-memory tree model for the .tree subpage. Built lazily from
/// `state.processes` by `TreeView`; nodes whose `ppid` either is nil or
/// points at a PID not present in the current process set are treated as
/// roots (this also handles "cycle" cases where a parent was missing or
/// out-of-band). To keep the model robust against malformed `ppid`
/// cycles (parent → child → parent), `TreeNode` is built by walking the
/// forest top-down and skipping any parent link that would revisit a
/// node already on the active ancestor chain.
struct TreeNode: Identifiable, Hashable {
    let process: ProcessSummary
    var children: [TreeNode]
    var id: UInt32 { process.pid }
    var depth: Int
}

// MARK: - Trends subpage

/// Fleet-wide time series view. Reads from `AppState.fleetHistory`
/// (a 60-sample rolling window captured per refresh), renders memory +
/// CPU + process-count sparklines, and exposes min/avg/max summary cards.
///
/// No IPC additions — all data is already collected by `AppState.refresh()`.
struct TrendsView: View {
    @ObservedObject var state: AppState

    private var samples: [FleetSample] {
        state.fleetHistory.sorted { $0.timestamp < $1.timestamp }
    }

    var body: some View {
        VStack(spacing: 0) {
            summaryStrip
            Divider()
            if samples.count < 2 {
                EmptyStateView(
                    icon: "chart.xyaxis.line",
                    title: "Waiting for first sample…",
                    subtitle: "Fleet telemetry populates this view as monitoring.report snapshots arrive (typically every ~5s).",
                    variant: .quiet
                )
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        TrendChartCard(
                            title: "Total memory",
                            subtitle: "Fleet-wide MB across all processes",
                            series: samples.map { Double($0.totalMemoryMB) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "MB",
                            color: .orange,
                            stats: stats(\.totalMemoryMB),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        TrendChartCard(
                            title: "Used memory",
                            subtitle: "Fleet-wide used MB (excluding caches, free)",
                            series: samples.map { Double($0.usedMemoryMB) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "MB",
                            color: .red,
                            stats: stats(\.usedMemoryMB),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        TrendChartCard(
                            title: "Avg CPU %",
                            subtitle: "Per-process average across fleet",
                            series: samples.map { Double($0.cpuAvgPercent) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "%",
                            color: .blue,
                            stats: stats(\.cpuAvgPercent),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        TrendChartCard(
                            title: "Process count",
                            subtitle: "Total processes tracked by the fleet pool",
                            series: samples.map { Double($0.totalProcesses) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "procs",
                            color: .green,
                            stats: stats(\.totalProcesses),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        poolHealthStrip
                    }
                    .padding(16)
                }
            }
        }
    }

    private var summaryStrip: some View {
        let count = samples.count
        let span = samples.count >= 2
            ? "\(Int(samples.last!.timestamp.timeIntervalSince(samples.first!.timestamp)))s"
            : "—"
        let latestRSS = samples.last?.totalMemoryMB ?? 0
        let latestCPU = samples.last?.cpuAvgPercent ?? 0
        return HStack(spacing: 12) {
            card("Samples", "\(count)", "of \(AppState.fleetHistoryCap)", .blue)
            card("Span", span, "rolling window", .purple)
            card("Total RSS", "\(latestRSS) MB", "now", .orange)
            card("Avg CPU", String(format: "%.1f%%", latestCPU), "now", latestCPU > 60 ? .red : .green)
        }
        .padding(10)
        .background(.quaternary.opacity(0.5))
    }

    private func card(_ title: String, _ value: String, _ sub: String, _ color: Color) -> some View {
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

    private func stats(_ keyPath: KeyPath<FleetSample, UInt64>) -> TrendStats {
        let values = samples.map { Double($0[keyPath: keyPath]) }
        return TrendStats(
            min: values.min() ?? 0,
            max: values.max() ?? 0,
            avg: values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count),
            current: values.last ?? 0
        )
    }

    private func stats(_ keyPath: KeyPath<FleetSample, Float>) -> TrendStats {
        let values = samples.map { Double($0[keyPath: keyPath]) }
        return TrendStats(
            min: values.min() ?? 0,
            max: values.max() ?? 0,
            avg: values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count),
            current: values.last ?? 0
        )
    }

    private func stats(_ keyPath: KeyPath<FleetSample, Int>) -> TrendStats {
        let values = samples.map { Double($0[keyPath: keyPath]) }
        return TrendStats(
            min: values.min() ?? 0,
            max: values.max() ?? 0,
            avg: values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count),
            current: values.last ?? 0
        )
    }

    private var poolHealthStrip: some View {
        let healthy = samples.filter { $0.poolHealthy }.count
        let pct = samples.isEmpty ? 0 : Double(healthy) / Double(samples.count)
        let latest = samples.last
        return HStack(spacing: 12) {
            Image(systemName: pct > 0.95 ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                .font(.title2)
                .foregroundStyle(pct > 0.95 ? .green : .orange)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text("Pool health").font(.headline)
                    HealthPill(healthy: latest?.poolHealthy, compact: true)
                }
                Text("\(healthy) / \(samples.count) samples were pool-healthy (\(Int(pct * 100))%)")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if let latest {
                HStack(spacing: 4) {
                    Image(systemName: "clock")
                        .font(.system(size: 10))
                        .foregroundStyle(.tertiary)
                    Text("Updated \(relativeTimestampFormatter.localizedString(for: latest.timestamp, relativeTo: Date()))")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(12)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct TrendStats: Hashable {
    let min: Double
    let max: Double
    let avg: Double
    let current: Double
}

// HealthPill and relativeTimestampFormatter were extracted to
// `HealthPill.swift` so ResourcesView and other panels can reuse
// them without duplicating the same SwiftUI body / formatter.

/// Single trend chart card. Renders a Path-based polyline with a gradient
/// fill below it, axis labels, and min/avg/max summary stats.
struct TrendChartCard: View {
    let title: String
    let subtitle: String
    let series: [Double]
    let timestamps: [Date]
    let unit: String
    let color: Color
    let stats: TrendStats
    /// Latest `FleetSample.poolHealthy` value. Drives the header pill
    /// color and a subtle border tint. Nil means no data yet.
    let poolHealthy: Bool?
    /// Timestamp of the latest sample; rendered as a "Updated Xs ago"
    /// footer using `RelativeDateTimeFormatter` (abbreviated style).
    let lastUpdated: Date?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.headline)
                    Text(subtitle).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                HealthPill(healthy: poolHealthy, compact: false)
                stat("min", stats.min)
                stat("avg", stats.avg)
                stat("max", stats.max)
                stat("now", stats.current)
            }
            chart
                .frame(height: 90)
                .background(.quaternary.opacity(0.3))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            footer
        }
        .padding(12)
        .background(cardBackground)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(borderTint, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    /// "Updated 5s ago" footer. Hidden when no timestamp is supplied
    /// (e.g. caller has not yet wired a sample).
    @ViewBuilder
    private var footer: some View {
        if let lastUpdated {
            HStack {
                Spacer()
                Image(systemName: "clock")
                    .font(.system(size: 9))
                    .foregroundStyle(.tertiary)
                Text("Updated \(relativeTimestampFormatter.localizedString(for: lastUpdated, relativeTo: Date()))")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(.secondary)
            }
            .padding(.top, 2)
        }
    }

    /// Tint the card background very subtly with the health color so
    /// the entire tile reads as healthy / degraded / unhealthy at a
    /// glance. Falls back to neutral `.quaternary` when no sample exists.
    private var cardBackground: AnyShapeStyle {
        guard let poolHealthy else { return AnyShapeStyle(HierarchicalShapeStyle.quaternary) }
        switch poolHealthy {
        case true: return AnyShapeStyle(Color.green.opacity(0.06))
        case false: return AnyShapeStyle(Color.yellow.opacity(0.08))
        }
    }

    private var borderTint: Color {
        guard let poolHealthy else { return .clear }
        switch poolHealthy {
        case true: return Color.green.opacity(0.35)
        case false: return Color.yellow.opacity(0.45)
        }
    }

    private func stat(_ label: String, _ v: Double) -> some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text(label.uppercased()).font(.system(size: 9, design: .monospaced)).foregroundStyle(.secondary)
            Text(formatValue(v))
                .font(.system(.caption, design: .monospaced)).bold()
                .foregroundStyle(color)
        }
        .frame(width: 64, alignment: .trailing)
    }

    private func formatValue(_ v: Double) -> String {
        if unit == "%" { return String(format: "%.1f%%", v) }
        if unit == "procs" { return "\(Int(v))" }
        if v >= 1024 { return String(format: "%.1f GB", v / 1024) }
        return String(format: "%.0f MB", v)
    }

    private var chart: some View {
        GeometryReader { geo in
            let minV = stats.min
            let maxV = Swift.max(stats.max, minV + 1)
            let range = maxV - minV
            let w = geo.size.width
            let h = geo.size.height
            let step = series.count > 1 ? w / CGFloat(series.count - 1) : 0
            ZStack(alignment: .leading) {
                // Gradient fill below the line
                Path { path in
                    guard series.count >= 2 else { return }
                    path.move(to: CGPoint(x: 0, y: h))
                    for (i, v) in series.enumerated() {
                        let x = CGFloat(i) * step
                        let y = h - CGFloat((v - minV) / range) * h
                        path.addLine(to: CGPoint(x: x, y: y))
                    }
                    path.addLine(to: CGPoint(x: w, y: h))
                    path.closeSubpath()
                }
                .fill(LinearGradient(
                    colors: [color.opacity(0.45), color.opacity(0.05)],
                    startPoint: .top,
                    endPoint: .bottom
                ))
                // The line itself
                Path { path in
                    guard series.count >= 2 else { return }
                    for (i, v) in series.enumerated() {
                        let x = CGFloat(i) * step
                        let y = h - CGFloat((v - minV) / range) * h
                        if i == 0 { path.move(to: CGPoint(x: x, y: y)) }
                        else { path.addLine(to: CGPoint(x: x, y: y)) }
                    }
                }
                .stroke(color, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))

                // Latest-value dot
                if let last = series.last {
                    let x = CGFloat(series.count - 1) * step
                    let y = h - CGFloat((last - minV) / range) * h
                    Circle().fill(color).frame(width: 6, height: 6)
                        .position(x: x, y: y)
                }

                // Time axis labels (first / mid / last timestamps)
                VStack {
                    Spacer()
                    HStack {
                        if let first = timestamps.first {
                            Text(relativeTime(first)).font(.system(size: 9, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        if timestamps.count >= 3 {
                            Text(relativeTime(timestamps[timestamps.count / 2]))
                                .font(.system(size: 9, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        if let last = timestamps.last {
                            Text(relativeTime(last)).font(.system(size: 9, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(.horizontal, 6)
                    .padding(.bottom, 2)
                }
            }
        }
    }

    private func relativeTime(_ date: Date) -> String {
        let secs = Int(Date().timeIntervalSince(date))
        if secs < 60 { return "-\(secs)s" }
        if secs < 3600 { return "-\(secs / 60)m" }
        return "-\(secs / 3600)h"
    }
}

