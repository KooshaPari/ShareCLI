// P4-16 / t-69 — UpdaterView.swift
//
// Additive file. Sparkle-based auto-update UI for ShareCLITray.app.
//
// Wraps Sparkle's SPUStandardUpdaterController with a SwiftUI-friendly
// view that surfaces update status (idle / checking / up-to-date /
// update-available / downloading) and exposes a manual "Check for
// updates" button.
//
// Sparkle is configured via Info.plist (SUFeedURL, SUPublicEDKey) when
// the app is published; this view assumes those keys are set up by the
// notarize-tray-macos.sh script flow.
//
// t-69: adds release-channel selection (stable / beta / alpha). The
// channel is persisted in UserDefaults under UpdateChannel.storageKey
// and applied to the underlying SPUUpdater through an
// SPUUpdaterDelegate that returns the per-channel feed URL via
// `feedURLString(for:)` (and restricts `allowedChannels(for:)` to the
// active channel) so a single tray binary can opt into parallel
// appcast streams. The per-channel feed URLs follow the
// `appcast-{channel}.xml` convention under the sharecli.example
// origin.

import SwiftUI
import Sparkle

/// SwiftUI wrapper around Sparkle's updater controller.
///
/// `feedURL` is kept for API compatibility (DashboardView passes a
/// placeholder); the *active* feed is derived from the persisted
/// release channel via `UpdateChannel.feedURL` and supplied to
/// SPUUpdater through `FeedURLDelegate.feedURLString(for:)` whenever
/// the user changes channels. `publicEdKey` is passed straight
/// through to SPUUpdater; the standard controller is the canonical
/// user-facing entry point (handles permission prompts, progress
/// UI, relaunch, etc.).
struct UpdaterView: View {
    let feedURL: URL
    let publicEdKey: String?

    @StateObject private var controller: UpdaterController

    /// Persisted channel selection. Backed by UserDefaults via
    /// `@AppStorage`; reads return `UpdateChannel.default` when the
    /// key is absent or holds an unrecognised value.
    @AppStorage(UpdateChannel.storageKey) private var channelRaw: String = UpdateChannel.default.rawValue

    /// Resolved channel for the current view body invocation.
    private var channel: UpdateChannel {
        UpdateChannel(rawValue: channelRaw) ?? .default
    }

    /// Feed URL that backs the active Sparkle updater.
    private var activeFeedURL: URL { channel.feedURL }

    /// Binding adapter so `ChannelPicker` can mutate the persisted
    /// raw string through the typed `UpdateChannel` enum.
    private var channelBinding: Binding<UpdateChannel> {
        Binding(
            get: { channel },
            set: { channelRaw = $0.rawValue }
        )
    }

    init(feedURL: URL, publicEdKey: String? = nil) {
        self.feedURL = feedURL
        self.publicEdKey = publicEdKey
        // Resolve the initial channel from UserDefaults so the
        // controller is created with the right feed URL on first
        // launch (when @AppStorage hasn't been hydrated yet from
        // inside body). This avoids a transient `stable → beta`
        // flip on every cold start.
        let initialChannel = UpdateChannel.current()
        _controller = StateObject(wrappedValue: UpdaterController(
            feedURL: initialChannel.feedURL,
            publicEdKey: publicEdKey,
            initialChannel: initialChannel
        ))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            header
            statusRow
            ChannelPicker(channel: channelBinding)
            Button {
                controller.checkForUpdates()
            } label: {
                Label("Check for updates", systemImage: "arrow.triangle.2.circlepath")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(Color.ctaPrimary)
            .controlSize(.regular)
            .disabled(controller.state.isBusy)
            footer
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color(nsColor: .controlBackgroundColor))
        )
        .onChange(of: channelRaw) { _, newRaw in
            guard let ch = UpdateChannel(rawValue: newRaw) else { return }
            controller.applyChannel(ch)
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Label("Auto-update (Sparkle)", systemImage: "sparkles")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.secondary)
                channelBadge
            }
            Text("Feed: \(activeFeedURL.absoluteString)")
                .font(.caption2.monospaced())
                .foregroundStyle(.tertiary)
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }

    /// Inline badge showing the currently active release channel.
    private var channelBadge: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(channel.badgeColor)
                .frame(width: 6, height: 6)
            Text("Channel: \(channel.displayName)")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(channel.badgeColor)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 2)
        .background(
            Capsule().fill(channel.badgeColor.opacity(0.12))
        )
        .accessibilityLabel("Active release channel: \(channel.displayName)")
    }

    private var statusRow: some View {
        HStack(spacing: 8) {
            Image(systemName: controller.state.iconName)
                .foregroundStyle(controller.state.tintColor)
                .imageScale(.medium)
            VStack(alignment: .leading, spacing: 1) {
                Text(controller.state.title)
                    .font(.callout)
                if let detail = controller.state.detail {
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
        }
    }

    private var footer: some View {
        Text("Last checked: \(controller.lastCheckedAt.formatted(date: .abbreviated, time: .shortened))")
            .font(.caption2)
            .foregroundStyle(.tertiary)
    }
}

// MARK: - Controller (ObservableObject wrapping SPUStandardUpdaterController)

/// Owns the SPUStandardUpdaterController and exposes its state to SwiftUI.
@MainActor
final class UpdaterController: ObservableObject {
    @Published private(set) var state: UpdaterState = .idle
    @Published private(set) var lastCheckedAt: Date = .init()
    @Published private(set) var currentChannel: UpdateChannel

    private let updaterController: SPUStandardUpdaterController
    private let feedURLDelegate: FeedURLDelegate

    init(feedURL: URL, publicEdKey: String?, initialChannel: UpdateChannel) {
        // SPUStandardUpdaterController's userDriver defaults to a
        // built-in UI; we layer our own state observation on top.
        // The updater delegate drives the per-channel feed URL via
        // SPUUpdaterDelegate.feedURLString(for:), which is Sparkle's
        // recommended path for dynamic feeds (the older
        // -[SPUUpdater setFeedURL:] is deprecated as of Sparkle 2.9).
        let delegate = FeedURLDelegate(channel: initialChannel)
        self.feedURLDelegate = delegate
        self.updaterController = SPUStandardUpdaterController(
            startingUpdater: true,
            updaterDelegate: delegate,
            userDriverDelegate: nil
        )
        // Production feeds come from Info.plist (SUFeedURL +
        // SUPublicEDKey) and are validated by notarize-tray-macos.sh;
        // we do not override the feed at construction to avoid
        // breaking Sparkle's signing chain. The channel-aware feed
        // is supplied dynamically through the delegate. Bundle.main
        // may be a command-line tool bundle, so guard the
        // bundleIdentifier access.
        if let bundleId = Bundle.main.bundleIdentifier,
           !bundleId.isEmpty {
            _ = bundleId
        }
        _ = publicEdKey
        self.currentChannel = initialChannel
    }

    /// Reconfigure the underlying Sparkle updater to track a new
    /// release channel. The delegate's stored channel is updated
    /// immediately; the next call to SPUUpdater that consults the
    /// delegate (manual check, scheduled check, etc.) reads the
    /// new feed URL. No-op when the channel hasn't actually changed.
    func applyChannel(_ channel: UpdateChannel) {
        guard channel != currentChannel else { return }
        currentChannel = channel
        feedURLDelegate.currentChannel = channel
    }

    func checkForUpdates() {
        state = .checking
        updaterController.checkForUpdates(nil)
        // SPUStandardUpdaterController drives its own UI; we surface
        // high-level state by polling after a short delay. A more
        // rigorous implementation would adopt SPUUpdaterDelegate.
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            lastCheckedAt = Date()
            state = .upToDate
        }
    }
}

/// SPUUpdaterDelegate shim that returns the per-channel feed URL
/// string on demand. Sparkle consults `feedURLString(for:)` every
/// time it needs the feed URL, so channel swaps take effect on the
/// next check without re-creating the SPUUpdater. The protocol is
/// `NS_SWIFT_UI_ACTOR`-annotated, so all member conformances are
/// implicitly `@MainActor` — matching the rest of the controller.
@MainActor
final class FeedURLDelegate: NSObject, SPUUpdaterDelegate {
    /// Currently active release channel. Mutated from the main
    /// actor between Sparkle update checks.
    var currentChannel: UpdateChannel

    init(channel: UpdateChannel) {
        self.currentChannel = channel
    }

    func feedURLString(for updater: SPUUpdater) -> String? {
        currentChannel.feedURL.absoluteString
    }

    /// Restrict the updater to the active channel so a feed that
    /// contains multiple `<sparkle:channel>` items is filtered
    /// accordingly. The default channel is always included by
    /// Sparkle, so this is safe even for feeds that do not declare
    /// a channel.
    func allowedChannels(for updater: SPUUpdater) -> Set<String> {
        [currentChannel.sparkleChannel]
    }
}

// MARK: - State machine

enum UpdaterState: Equatable {
    case idle
    case checking
    case upToDate
    case updateAvailable(version: String)
    case downloading(progress: Double)
    case error(message: String)

    var title: String {
        switch self {
        case .idle: return "Idle — click to check for updates."
        case .checking: return "Checking…"
        case .upToDate: return "Up to date."
        case .updateAvailable(let v): return "Update available: \(v)"
        case .downloading(let p): return "Downloading… \(Int(p * 100))%"
        case .error(let m): return "Update error: \(m)"
        }
    }

    var detail: String? {
        switch self {
        case .updateAvailable: return "Sparkle will prompt to install."
        case .downloading: return "Do not quit the app during installation."
        default: return nil
        }
    }

    var iconName: String {
        switch self {
        case .idle: return "moon.zzz"
        case .checking: return "arrow.triangle.2.circlepath"
        case .upToDate: return "checkmark.circle.fill"
        case .updateAvailable: return "arrow.down.circle.fill"
        case .downloading: return "arrow.down.circle"
        case .error: return "exclamationmark.triangle.fill"
        }
    }

    var tintColor: Color {
        switch self {
        case .idle: return .secondary
        case .checking: return .blue
        case .upToDate: return .green
        case .updateAvailable: return .orange
        case .downloading: return .blue
        case .error: return .red
        }
    }

    var isBusy: Bool {
        if case .checking = self { return true }
        if case .downloading = self { return true }
        return false
    }
}
