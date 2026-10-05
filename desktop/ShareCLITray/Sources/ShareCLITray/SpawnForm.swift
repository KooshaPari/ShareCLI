/// SpawnForm.swift — extracted spawn and presets views from ProcessesPage.
///
/// Contains: SpawnView, PresetsView, SpawnHistoryRow.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore
import AppKit

// MARK: - SpawnView (process.spawn IPC)

struct SpawnView: View {
    let state: AppState

    @State private var workingDirectory: String = NSHomeDirectory()
    @State private var binary: String = "/usr/bin/env"
    @State private var argsCSV: String = ""
    @State private var project: String = ""
    @State private var harness: String = ""
    @State private var spawning: Bool = false
    @State private var lastResult: ProcessSpawnResult?
    @State private var lastError: String?
    @AppStorage("spawn.lastArgs") private var lastArgsJSON: String = "[]"

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack {
                    Image(systemName: "wand.and.stars")
                        .font(.title2)
                        .foregroundStyle(.tint)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Spawn a new sidecar process").font(.headline)
                        Text("Calls process.spawn on the sidecar; pool will absorb it under harness/project as configured.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                GroupBox("Command") {
                    Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 10, verticalSpacing: 8) {
                        GridRow {
                            Text("Working dir").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("Working directory", text: $workingDirectory)
                                .textFieldStyle(.roundedBorder)
                        }
                        GridRow {
                            Text("Binary").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("/usr/bin/env", text: $binary)
                                .textFieldStyle(.roundedBorder)
                        }
                        GridRow {
                            Text("Args (CSV)").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("e.g. node,index.js,--inspect=0", text: $argsCSV)
                                .textFieldStyle(.roundedBorder)
                        }
                    }
                    .padding(8)
                }

                GroupBox("Pool tags") {
                    Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 10, verticalSpacing: 8) {
                        GridRow {
                            Text("Project").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("(optional)", text: $project).textFieldStyle(.roundedBorder)
                        }
                        GridRow {
                            Text("Harness").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("(optional)", text: $harness).textFieldStyle(.roundedBorder)
                        }
                    }
                    .padding(8)
                }

                HStack {
                    Button {
                        Task { await spawn() }
                    } label: {
                        if spawning { ProgressView().controlSize(.small) }
                        else { Label("Spawn", systemImage: "play.fill") }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(spawning || binary.isEmpty)
                    .keyboardShortcut(.return, modifiers: [.command])

                    Spacer()

                    Button("Save args as preset") { savePreset() }
                        .buttonStyle(.bordered)
                        .disabled(argsCSV.isEmpty)
                }

                if let result = lastResult {
                    GroupBox(result.success ? "Spawned" : "Failed") {
                        HStack(spacing: 14) {
                            Image(systemName: result.success ? "checkmark.circle.fill" : "xmark.octagon.fill")
                                .foregroundStyle(result.success ? .green : .red)
                            Text("PID: \(result.pid)").monospacedDigit()
                            if let err = result.error {
                                Text(err).font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        .padding(8)
                    }
                } else if let err = lastError {
                    Text(err).font(.caption).foregroundStyle(.red)
                }

                if !state.spawnHistory.isEmpty {
                    GroupBox("Recent spawn history (last \(min(state.spawnHistory.count, 10)))") {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(state.spawnHistory.prefix(10)) { entry in
                                SpawnHistoryRow(entry: entry)
                            }
                            if state.spawnHistory.count > 10 {
                                Text("…and \(state.spawnHistory.count - 10) more persisted entries")
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        .padding(8)
                    }
                }

                Text("Tip: Pool absorbs the new process under the harness/project you tag it with — convenient for testing pool effectiveness metrics (⌘4).")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(20)
        }
    }

    private func spawn() async {
        spawning = true
        defer { spawning = false }
        lastResult = nil
        lastError = nil

        let argv: [String] = argsCSV
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }

        do {
            let result = try await state.client.spawn(payload: ProcessSpawnPayload(
                name: binary,
                command: binary,
                args: argv,
                project: project.isEmpty ? nil : project,
                harness: harness.isEmpty ? nil : harness,
                cwd: workingDirectory.isEmpty ? nil : workingDirectory
            ))
            lastResult = result
            // Save only the argv so the preset recalls a clean spawn.
            if let data = try? JSONSerialization.data(withJSONObject: argv),
               let json = String(data: data, encoding: .utf8) {
                lastArgsJSON = json
            }
            // Record the attempt in the persistent spawn history (P1-7)
            let entry = SpawnHistoryEntry(
                command: binary.isEmpty ? "(empty)" : binary,
                args: argv,
                project: project.isEmpty ? nil : project,
                harness: harness.isEmpty ? nil : harness,
                workingDir: workingDirectory,
                succeeded: result.success,
                spawnedPID: result.pid,
                errorMessage: result.error
            )
            state.recordSpawn(entry)
        } catch {
            lastError = "\(error)"
            let entry = SpawnHistoryEntry(
                command: binary.isEmpty ? "(empty)" : binary,
                args: argv,
                project: project.isEmpty ? nil : project,
                harness: harness.isEmpty ? nil : harness,
                workingDir: workingDirectory,
                succeeded: false,
                spawnedPID: nil,
                errorMessage: "\(error)"
            )
            state.recordSpawn(entry)
        }
    }

    private func savePreset() {
        let argv: [String] = argsCSV
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
        if let data = try? JSONSerialization.data(withJSONObject: argv),
           let json = String(data: data, encoding: .utf8) {
            lastArgsJSON = json
        }
    }
}

// MARK: - PresetsView (filter saved-presets)

struct PresetsView: View {
    let state: AppState

    @AppStorage("processes.presets") private var presetsJSON: String = "[]"
    @AppStorage("processes.subpage") private var subpageRaw: String = ProcessesPage.Subpage.all.rawValue
    @AppStorage("processes.allFilterText") private var filterText: String = ""
    @AppStorage("processes.allMinRSS") private var minRSS: Double = 0
    @AppStorage("processes.allSortKey") private var sortKey: String = "pid"

    @State private var newPresetName: String = ""

    private struct Preset: Codable, Identifiable {
        let id: UUID
        var name: String
        var filterText: String
        var minRSS: Double
        var sortKey: String
        var createdAt: Date
    }

    private var presets: [Preset] {
        guard let data = presetsJSON.data(using: .utf8),
              let decoded = try? JSONDecoder().decode([Preset].self, from: data) else {
            return []
        }
        return decoded.sorted(by: { $0.createdAt > $1.createdAt })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: "bookmark.fill").font(.title2).foregroundStyle(.tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Filter presets").font(.headline)
                    Text("Save and recall the current All-subpage filter as a named preset.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }

            HStack {
                TextField("Preset name", text: $newPresetName)
                    .textFieldStyle(.roundedBorder)
                Button("Save current filter") { saveCurrent() }
                    .buttonStyle(.borderedProminent)
                    .disabled(newPresetName.isEmpty)
                    .keyboardShortcut(.return, modifiers: [.command])
            }

            if presets.isEmpty {
                EmptyStateView(
                    icon: "bookmark.slash",
                    title: "No presets saved yet",
                    subtitle: "Configure a filter on the All subpage (text + min-RSS slider + sort), name it, and save. Click any preset to recall it.",
                    primaryAction: nil,
                    secondaryAction: nil
                )
            } else {
                ForEach(presets) { preset in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(preset.name).font(.subheadline).fontWeight(.medium)
                            HStack(spacing: 10) {
                                if !preset.filterText.isEmpty {
                                    Label(preset.filterText, systemImage: "text.magnifyingglass")
                                }
                                if preset.minRSS > 0 {
                                    Label("≥ \(Int(preset.minRSS)) MB", systemImage: "memorychip")
                                }
                                Label("sort: \(preset.sortKey)", systemImage: "arrow.up.arrow.down")
                            }
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text(preset.createdAt.formatted(date: .abbreviated, time: .shortened))
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                        Button("Apply") { apply(preset) }.buttonStyle(.bordered)
                        Button(role: .destructive) { delete(preset) } label: { Image(systemName: "trash") }
                            .buttonStyle(.borderless)
                    }
                    .padding(10)
                    .background(.quaternary.opacity(0.3))
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                }
            }

            Spacer()
        }
        .padding(20)
    }

    private func saveCurrent() {
        let preset = Preset(
            id: UUID(),
            name: newPresetName,
            filterText: filterText,
            minRSS: minRSS,
            sortKey: sortKey,
            createdAt: Date()
        )
        var updated = presets
        updated.append(preset)
        if let data = try? JSONEncoder().encode(updated),
           let json = String(data: data, encoding: .utf8) {
            presetsJSON = json
            newPresetName = ""
        }
    }

    private func apply(_ preset: Preset) {
        filterText = preset.filterText
        minRSS = preset.minRSS
        sortKey = preset.sortKey
        subpageRaw = ProcessesPage.Subpage.all.rawValue
    }

    private func delete(_ preset: Preset) {
        var updated = presets
        updated.removeAll { $0.id == preset.id }
        if let data = try? JSONEncoder().encode(updated),
           let json = String(data: data, encoding: .utf8) {
            presetsJSON = json
        }
    }
}
// MARK: - Spawn History (P1-7)

/// A single spawn attempt surfaced in the Spawn subpage's recent-history list.
struct SpawnHistoryRow: View {
    let entry: SpawnHistoryEntry
    static let timestampFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    private var renderedArgv: String {
        ([entry.command] + entry.args).joined(separator: " ")
    }

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: entry.succeeded ? "checkmark.circle.fill" : "xmark.octagon.fill")
                .foregroundStyle(entry.succeeded ? Color.green : Color.red)
                .font(.caption)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(Self.timestampFormatter.string(from: entry.timestamp))
                        .font(.caption.monospacedDigit())
                    if entry.succeeded, let pid = entry.spawnedPID {
                        Text("pid \(pid)").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                    } else if let err = entry.errorMessage {
                        Text(err).font(.caption).foregroundStyle(.red).lineLimit(2)
                    }
                    Spacer(minLength: 0)
                }
                Text(renderedArgv).font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 2)
    }
}
