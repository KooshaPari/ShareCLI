/// HelpSheet.swift — keyboard shortcut reference sheet (Cmd+/).
import SwiftUI

struct HelpSheet: View {
    @Binding var isVisible: Bool

    struct Shortcut: Identifiable, Hashable {
        let id: String
        let key: String
        let label: String
    }
    static let shortcuts: [Shortcut] = [
        .init(id: "k1", key: "⌘1", label: "Overview page"),
        .init(id: "k2", key: "⌘2", label: "Processes page"),
        .init(id: "k3", key: "⌘3", label: "Agents page"),
        .init(id: "k4", key: "⌘4", label: "Pool page"),
        .init(id: "k5", key: "⌘5", label: "Pool effectiveness page"),
        .init(id: "k6", key: "⌘6", label: "Config page"),
        .init(id: "k7", key: "⌘7", label: "Health page"),
        .init(id: "k8", key: "⌘8", label: "Logs page"),
        .init(id: "kr", key: "⌘R", label: "Refresh all panels"),
        .init(id: "kk", key: "⌘K", label: "Open command palette"),
        .init(id: "kw", key: "⌘W", label: "Close window"),
        .init(id: "kcomma", key: "⌘,", label: "Open preferences"),
        .init(id: "kslash", key: "⌘/", label: "Show this help"),
        .init(id: "kesc", key: "esc", label: "Dismiss overlays"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Label("Keyboard shortcuts", systemImage: "keyboard")
                    .font(.title2).bold()
                Spacer()
                Button {
                    isVisible = false
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.pressable)
            }
            .padding(20)
            Divider()
            Table(Self.shortcuts) {
                TableColumn("Key") { row in
                    Text(row.key)
                        .font(.system(.body, design: .monospaced))
                        .bold()
                        .frame(minWidth: 64, alignment: .leading)
                }
                .width(min: 64, ideal: 72)
                TableColumn("Action") { row in
                    Text(row.label)
                }
            }
            .frame(width: 380, height: 360)
        }
        .frame(width: 420, height: 440)
    }
}
