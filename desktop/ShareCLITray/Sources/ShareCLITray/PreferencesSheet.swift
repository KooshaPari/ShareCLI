/// PreferencesSheet.swift — Cmd+, preferences sheet for the dashboard.
///
/// Lightweight settings that don't belong in the sidecar config:
///  - Refresh interval (polling cadence, in seconds, 1-30)
///  - Log buffer cap (line cap for LogsPage, 100-20000)
///  - Dashboard sidebar column width (just shown as read-only — we don't
///    offer a way to change it from inside the sheet because the user
///    can already drag the NavigationSplitView column)
///  - IPC endpoint (read-only, tilde-expanded display of the socket path
///    the Swift tray connects to)
import SwiftUI
import ShareCLICore
import AppKit

struct PreferencesSheet: View {
    @Binding var isVisible: Bool
    @ObservedObject var state: AppState

    @AppStorage("dashboard.refreshIntervalSecs") private var refreshInterval: Double = 5
    @AppStorage("dashboard.logBufferCap") private var logBufferCap: Int = 5000
    @AppStorage("dashboard.sidebar.columnWidth") private var sidebarWidth: Double = 168

    private var socketPath: String {
        IPCClient.defaultSocketPath()
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Label("Preferences", systemImage: "gearshape")
                    .font(.title2).bold()
                Spacer()
                Button {
                    isVisible = false
                } label: {
                    Image(systemName: StatusIcon.dismiss.symbolName)
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.pressable)
            }
            .padding(20)
            Divider()
            Form {
                Section("Polling") {
                    HStack {
                        Text("Refresh interval")
                        Spacer()
                        Text(String(format: "%.0fs", refreshInterval))
                            .monospaced()
                            .foregroundStyle(.secondary)
                            .frame(width: 32, alignment: .trailing)
                    }
                    Slider(value: $refreshInterval, in: 1...30, step: 1)
                    Text("How often the dashboard re-fetches IPC data when the sidecar is connected.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Section("Logs page") {
                    HStack {
                        Text("Log buffer cap")
                        Spacer()
                        Text("\(logBufferCap) lines")
                            .monospaced()
                            .foregroundStyle(.secondary)
                            .frame(width: 80, alignment: .trailing)
                    }
                    Stepper("Buffer cap", value: $logBufferCap, in: 100...20000, step: 500)
                    Text("Maximum lines kept in the SwiftUI virtualised list. Older lines roll off the top.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Section("Layout (read-only)") {
                    LabeledContent("Sidebar width") {
                        Text("\(Int(sidebarWidth)) pt")
                            .monospaced()
                            .foregroundStyle(.secondary)
                    }
                    LabeledContent("IPC endpoint") {
                        Text(socketPath)
                            .font(.system(.caption, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .foregroundStyle(.secondary)
                    }
                    LabeledContent("Log file") {
                        Text(state.statusSnapshot?.live_log_path?.path ?? "—")
                            .font(.system(.caption, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .foregroundStyle(.secondary)
                            .help(socketPath)
                    }
                }
            }
            .formStyle(.grouped)
            .frame(width: 480, height: 440)
        }
        .frame(width: 520, height: 540)
    }
}
