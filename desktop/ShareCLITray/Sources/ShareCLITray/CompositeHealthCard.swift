/// CompositeHealthCard.swift — SwiftUI card for the composite health metric (T-95).
///
/// Surfaces `CompositeHealthMetric` (pool health + CPU + memory + load)
/// as a single glanceable tile for `DashboardOverview`. Read-only — no
/// actions, no buttons; this is a "what is the fleet doing right now"
/// indicator, not a control surface.
///
/// Layout (collapsed):
///   ┌──────────────────────────────────────────────────────────────┐
///   │  Composite health   72 / 100   [WATCH]                       │
///   │  Pool 30/30  CPU 18/25  Memory 22/25  Load 12/20             │
///   └──────────────────────────────────────────────────────────────┘
///
/// Layout (cold-start placeholder when no fleet sample has arrived yet):
///   ┌──────────────────────────────────────────────────────────────┐
///   │  Composite health   Waiting for first fleet sample…         │
///   └──────────────────────────────────────────────────────────────┘

import SwiftUI
import ShareCLICore

struct CompositeHealthCard: View {
    let fleet: FleetSample?
    let host: HostWatchSample?

    private var metric: CompositeHealthMetric? {
        CompositeHealthMetric.compute(fleet: fleet, host: host)
    }

    var body: some View {
        if let metric {
            content(metric)
        } else {
            placeholder
        }
    }

    private func content(_ metric: CompositeHealthMetric) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            header(metric)
            breakdownRow(metric)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color(nsColor: .controlBackgroundColor))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(bandColor(metric.band).opacity(0.4), lineWidth: 1)
        )
    }

    private func header(_ metric: CompositeHealthMetric) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Image(systemName: StatusIcon.compositeHealth.symbolName)
                        .foregroundStyle(bandColor(metric.band))
                    Text("Composite health")
                        .font(.headline)
                }
                Text("Pool + CPU + memory + load, scaled 0–\(CompositeHealthMetric.maxScore)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Text("\(metric.score)")
                .font(.system(size: 38, weight: .bold, design: .rounded))
                .monospacedDigit()
                .foregroundStyle(bandColor(metric.band))
            Text("/ \(CompositeHealthMetric.maxScore)")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.tertiary)
            bandPill(metric.band)
        }
    }

    private func breakdownRow(_ metric: CompositeHealthMetric) -> some View {
        HStack(alignment: .top, spacing: 14) {
            componentStat(
                label: "Pool",
                value: "\(metric.breakdown.poolPoints) / 30",
                detail: metric.poolHealthy ? "healthy" : "degraded",
                color: metric.poolHealthy ? .green : .red,
                icon: metric.poolHealthy ? "checkmark.seal.fill" : "exclamationmark.triangle.fill"
            )
            componentStat(
                label: "CPU",
                value: String(
                    format: "%d / 25",
                    metric.breakdown.cpuPoints
                ),
                detail: String(format: "%.1f%%", metric.breakdown.cpuPercent),
                color: cpuColor(metric.breakdown.cpuPercent),
                icon: "cpu"
            )
            componentStat(
                label: "Memory",
                value: String(
                    format: "%d / 25",
                    metric.breakdown.memoryPoints
                ),
                detail: "\(metric.breakdown.memUsedMB)/\(metric.breakdown.memTotalMB) MB",
                color: memColor(
                    used: metric.breakdown.memUsedMB,
                    total: metric.breakdown.memTotalMB
                ),
                icon: "memorychip"
            )
            componentStat(
                label: "Load",
                value: String(
                    format: "%d / 20",
                    metric.breakdown.loadPoints
                ),
                detail: metric.breakdown.hasHostSample
                    ? String(format: "%.2f 1m", metric.breakdown.load1m)
                    : "no host sample",
                color: loadColor(metric.breakdown.load1m, hasSample: metric.breakdown.hasHostSample),
                icon: "speedometer"
            )
        }
    }

    private func componentStat(
        label: String,
        value: String,
        detail: String,
        color: Color,
        icon: String
    ) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                Image(systemName: icon)
                    .font(.system(size: 10))
                Text(label)
                    .font(.caption2)
            }
            .foregroundStyle(color)
            Text(value)
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.primary)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
            Text(detail)
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(.tertiary)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func bandPill(_ band: CompositeHealthMetric.HealthBand) -> some View {
        Text(band.displayName.uppercased())
            .font(.system(size: 10, weight: .bold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .foregroundStyle(bandColor(band))
            .overlay(
                Capsule().stroke(bandColor(band), lineWidth: 1)
            )
    }

    private func bandColor(_ band: CompositeHealthMetric.HealthBand) -> Color {
        switch band {
        case .healthy: return .green
        case .watch: return .yellow
        case .degraded: return .orange
        case .critical: return .red
        }
    }

    private func cpuColor(_ percent: Float) -> Color {
        if percent >= 80 { return .red }
        if percent >= 50 { return .orange }
        return .blue
    }

    private func memColor(used: UInt64, total: UInt64) -> Color {
        guard total > 0 else { return .blue }
        let frac = Double(used) / Double(total)
        if frac >= 0.85 { return .red }
        if frac >= 0.65 { return .orange }
        return .blue
    }

    private func loadColor(_ load: Double, hasSample: Bool) -> Color {
        guard hasSample else { return .secondary }
        if load >= 4.0 { return .red }
        if load >= 2.0 { return .orange }
        return .purple
    }

    private var placeholder: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: StatusIcon.healthPlaceholder.symbolName)
                    .foregroundStyle(.tertiary)
                Text("Composite health")
                    .font(.headline)
            }
            Text("Waiting for first fleet sample…")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color(nsColor: .controlBackgroundColor))
        )
    }
}
