/// MiniCompositeHealthCard.swift — compact glanceable composite-health tile.
///
/// Companion to `CompositeHealthCard` (T-95). Renders the same 0–100
/// score + four-band classification in a single dense row suitable for
/// the sidebar footer, narrow page headers, and other tight surfaces
/// where the full breakdown row would not fit.
///
/// Layout:
///   ┌──────────────────────────────────────────────────┐
///   │ ♥ Composite  72 / 100  [WATCH]                   │
///   │ Updated 3s ago                                   │
///   └──────────────────────────────────────────────────┘
///
/// Cold-start layout (no fleet sample yet):
///   ┌──────────────────────────────────────────────────┐
///   │ ♥ Composite  Waiting…                            │
///   └──────────────────────────────────────────────────┘
///
/// Reuses the same band color palette as the full card so a glance
/// across the dashboard (sidebar footer + dashboard overview + page
/// banners) is visually consistent.

import SwiftUI
import ShareCLICore

struct MiniCompositeHealthCard: View {
    let fleet: FleetSample?
    let host: HostWatchSample?

    private var metric: CompositeHealthMetric? {
        CompositeHealthMetric.compute(fleet: fleet, host: host)
    }

    var body: some View {
        Group {
            if let metric {
                content(metric)
            } else {
                placeholder
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color(nsColor: .controlBackgroundColor))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(borderColor, lineWidth: 1)
        )
    }

    private func content(_ metric: CompositeHealthMetric) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Image(systemName: StatusIcon.compositeHealth.symbolName)
                    .font(.system(size: 11))
                    .foregroundStyle(bandColor(metric.band))
                Text("Composite")
                    .font(.caption.weight(.semibold))
                Spacer()
                Text("\(metric.score)")
                    .font(.system(.body, design: .rounded).weight(.bold))
                    .monospacedDigit()
                    .foregroundStyle(bandColor(metric.band))
                Text("/ \(CompositeHealthMetric.maxScore)")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(.tertiary)
                bandPill(metric.band)
            }
            if let ts = fleet?.timestamp {
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

    private var placeholder: some View {
        HStack(spacing: 6) {
            Image(systemName: "heart.text.square")
                .font(.system(size: 11))
                .foregroundStyle(.tertiary)
            Text("Composite")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Spacer()
            Text("Waiting…")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
    }

    private func bandPill(_ band: CompositeHealthMetric.HealthBand) -> some View {
        Text(band.displayName.uppercased())
            .font(.system(size: 9, weight: .bold))
            .padding(.horizontal, 5)
            .padding(.vertical, 1)
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

    private var borderColor: Color {
        guard let metric else { return .clear }
        return bandColor(metric.band).opacity(0.4)
    }
}
