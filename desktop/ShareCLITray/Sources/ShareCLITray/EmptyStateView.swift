/// EmptyStateView.swift — shared empty-state component for dashboard pages.
///
/// Renders a centered card with an SF Symbol, a title, a subtitle, and up to two
/// action buttons. Used by every dashboard page when its underlying data is empty
/// (no processes spawned yet, no agents observed, no log file written, etc.).
///
/// Variants:
///   - .quiet  — minimal chrome (used for subpage empty state inside a larger frame)
///   - .normal — standard centred card (default)
///   - .hero   — larger icon + accent gradient backdrop, used on cold-start screens

import SwiftUI

struct EmptyStateView: View {
    enum Variant { case quiet, normal, hero }

    let icon: String
    let title: String
    let subtitle: String
    var variant: Variant = .normal

    var primaryTitle: String? = nil
    var primaryIcon: String? = nil
    var primaryAction: (() -> Void)? = nil

    var secondaryTitle: String? = nil
    var secondaryIcon: String? = nil
    var secondaryAction: (() -> Void)? = nil

    @ViewBuilder
    var body: some View {
        VStack(spacing: variant == .quiet ? 8 : 14) {
            ZStack {
                if variant == .hero {
                    Circle()
                        .fill(LinearGradient(colors: [.accentColor.opacity(0.18), .clear], startPoint: .top, endPoint: .bottom))
                        .frame(width: 120, height: 120)
                }
                Image(systemName: icon)
                    .font(.system(size: variant == .hero ? 40 : 28, weight: .semibold))
                    .foregroundStyle(iconStyle)
            }

            VStack(spacing: 4) {
                Text(title)
                    .font(variant == .hero ? .title2 : .title3).bold()
                    .multilineTextAlignment(.center)
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }

            if primaryAction != nil || secondaryAction != nil {
                HStack(spacing: 10) {
                    if let primaryAction, let primaryTitle {
                        Button { primaryAction() } label: {
                            HStack(spacing: 6) {
                                if let primaryIcon { Image(systemName: primaryIcon) }
                                Text(primaryTitle)
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(Color.ctaPrimary)
                        .controlSize(.regular)
                    }
                    if let secondaryAction, let secondaryTitle {
                        Button { secondaryAction() } label: {
                            HStack(spacing: 6) {
                                if let secondaryIcon { Image(systemName: secondaryIcon) }
                                Text(secondaryTitle)
                            }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.regular)
                    }
                }
            }
        }
        .padding(variant == .quiet ? 10 : 24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(backgroundView)
    }

    private var iconStyle: AnyShapeStyle {
        if variant == .hero {
            return AnyShapeStyle(LinearGradient(
                colors: [.accentColor, .accentColor.opacity(0.7)],
                startPoint: .top, endPoint: .bottom
            ))
        }
        return AnyShapeStyle(HierarchicalShapeStyle.secondary)
    }

    @ViewBuilder
    private var backgroundView: some View {
        switch variant {
        case .hero:
            RoundedRectangle(cornerRadius: 12).fill(.quaternary.opacity(0.4))
        case .quiet:
            EmptyView()
        case .normal:
            RoundedRectangle(cornerRadius: 8).fill(.quaternary.opacity(0.25))
        }
    }
}
