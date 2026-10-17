//
// HealthPill.swift
//
// Shared "pool healthy / degraded / unhealthy" status pill and the
// abbreviated relative-timestamp formatter that the dashboard's
// "Updated 5s ago" footers use. Both used to live file-scoped inside
// ProcessesPage.swift; lifting them here lets every view (ResourcesView,
// DashboardOverview, future panels) wire them in without re-declaring
// the same SwiftUI body or formatter configuration.
//
// This file is intentionally self-contained: no shared state with
// other files, no SwiftPM dependencies beyond what the rest of
// ShareCLITray already imports (SwiftUI + Foundation).

import SwiftUI
import Foundation

/// Compact "pool healthy / degraded / unhealthy" status pill.
/// Rendered inside TrendChartCard headers, on the poolHealthStrip,
/// and now on the Resources subpage header.
/// Color is derived from `FleetSample.poolHealthy` (latest sample).
/// - true → green (healthy)
/// - false → yellow (degraded — pool exists but reported unhealthy)
/// - nil → red (waiting for first sample)
struct HealthPill: View {
    let healthy: Bool?
    var compact: Bool = false

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: iconName)
                .font(.system(size: compact ? 9 : 10, weight: .semibold))
            if !compact {
                Text(label)
                    .font(.system(size: 10, weight: .semibold))
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, compact ? 5 : 7)
        .padding(.vertical, compact ? 2 : 3)
        .foregroundStyle(foreground)
        .background(background)
        .clipShape(Capsule())
        .overlay(
            Capsule().strokeBorder(stroke, lineWidth: 0.5)
        )
        .help(tooltip)
    }

    private var label: String {
        switch healthy {
        case .some(true): return "healthy"
        case .some(false): return "degraded"
        case .none: return "unhealthy"
        }
    }

    private var iconName: String {
        switch healthy {
        case .some(true): return "checkmark.circle.fill"
        case .some(false): return "exclamationmark.triangle.fill"
        case .none: return StatusIcon.unhealthy.symbolName
        }
    }

    private var foreground: Color {
        switch healthy {
        case .some(true): return .green
        case .some(false): return .yellow
        case .none: return .red
        }
    }

    private var background: Color {
        foreground.opacity(0.15)
    }

    private var stroke: Color {
        foreground.opacity(0.5)
    }

    private var tooltip: String {
        switch healthy {
        case .some(true): return "Pool reports healthy"
        case .some(false): return "Pool reports degraded — check the Pool page"
        case .none: return "No fleet sample yet — pool health unknown"
        }
    }
}

/// Shared `RelativeDateTimeFormatter` for "Updated 5s ago" footers.
/// Configured once for `.abbreviated` style ("5s ago", "2 min. ago").
/// `internal` so any view in the `ShareCLITray` target can reuse it
/// without re-allocating a formatter per render.
let relativeTimestampFormatter: RelativeDateTimeFormatter = {
    let f = RelativeDateTimeFormatter()
    f.unitsStyle = .abbreviated
    return f
}()
