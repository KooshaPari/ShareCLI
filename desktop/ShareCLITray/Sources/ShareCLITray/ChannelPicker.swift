// t-69 — ChannelPicker.swift
//
// Additive file. Channel selection UI for the Sparkle auto-updater
// in ShareCLITray. Defines the release-channel enum
// (stable / beta / alpha), the feed-URL derivation, and a SwiftUI
// picker that surfaces the current channel. Channel choice is
// persisted in UserDefaults so subsequent launches remember the
// user's preference.
//
// Feed URLs follow the convention: appcast-{channel}.xml under the
// sharecli.example origin (placeholder; production feeds are
// published by notarize-tray-macos.sh once the appcast hosts are
// provisioned). The channel is also forwarded to Sparkle's
// SUChannel metadata so the feed itself can disambiguate parallel
// appcast entries that share an origin.

import SwiftUI

/// Release channel for the Sparkle auto-updater.
///
/// Raw values are the on-disk channel names (`stable` / `beta` /
/// `alpha`) and the path component used to derive the per-channel
/// appcast URL.
enum UpdateChannel: String, CaseIterable, Identifiable {
    case stable
    case beta
    case alpha

    var id: String { rawValue }

    /// UserDefaults key for the persisted channel choice.
    static let storageKey = "sharecli.updateChannel"

    /// Origin shared by all appcast feeds. Channels are encoded as
    /// `appcast-{channel}.xml` path components beneath this origin.
    /// Production: replace with the real sharecli.app origin.
    static let feedBase = "https://sharecli.example"

    /// Default channel for first-launch users.
    static let `default`: UpdateChannel = .stable

    /// Derive the Sparkle feed URL for this channel.
    var feedURL: URL {
        // Force-unwrap is safe: feedBase is a hardcoded HTTPS URL
        // and the path component is a known static literal.
        URL(string: "\(Self.feedBase)/appcast-\(rawValue).xml")!
    }

    /// Value to forward to Sparkle's SUChannel metadata.
    var sparkleChannel: String { rawValue }

    /// Human-readable label.
    var displayName: String {
        switch self {
        case .stable: return "Stable"
        case .beta: return "Beta"
        case .alpha: return "Alpha"
        }
    }

    /// Short tagline describing the channel's audience.
    var tagline: String {
        switch self {
        case .stable: return "Production releases."
        case .beta: return "Early access — tested but not blessed."
        case .alpha: return "Nightly build — expect breakage."
        }
    }

    /// Tint color for the channel badge.
    var badgeColor: Color {
        switch self {
        case .stable: return .green
        case .beta: return .orange
        case .alpha: return .purple
        }
    }

    /// Resolve the currently-selected channel from UserDefaults.
    /// Falls back to `.default` for missing or unrecognised values
    /// (e.g. legacy strings written by an earlier app version).
    static func current(defaults: UserDefaults = .standard) -> UpdateChannel {
        guard let raw = defaults.string(forKey: storageKey),
              let ch = UpdateChannel(rawValue: raw) else {
            return .default
        }
        return ch
    }
}

/// SwiftUI picker for the release channel. Renders as a three-button
/// segmented control with a small tagline beneath. The host view
/// owns the binding and is responsible for re-configuring the
/// Sparkle updater and persisting the choice.
struct ChannelPicker: View {
    @Binding var channel: UpdateChannel

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                ForEach(UpdateChannel.allCases) { option in
                    Button {
                        channel = option
                    } label: {
                        Text(option.displayName)
                            .font(.caption.weight(.semibold))
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(channel == option ? option.badgeColor : Color.ctaSecondary.opacity(0.25))
                    .controlSize(.small)
                    .accessibilityLabel("Release channel: \(option.displayName)")
                    .accessibilityAddTraits(channel == option ? [.isSelected] : [])
                }
            }
            Text(channel.tagline)
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }
}
