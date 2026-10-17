/// FailedActionBanner.swift — the shared one-action recovery banner.
///
/// Extracted from `HealthPage.swift` (audit: a component shared by five pages
/// was declared inside one of its consumers). Every page that surfaces a failed
/// IPC action renders this; keeping it in `HealthPage` coupled the component to
/// a caller whose concerns it does not own.

import SwiftUI

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
