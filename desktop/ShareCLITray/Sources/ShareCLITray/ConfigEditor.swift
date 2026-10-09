/// ConfigEditor.swift — extracted spawn and defaults subpages.
///
/// Contains: SpawnSubpage, DefaultsSubpage, and the shared sectionHeader helper.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore

// MARK: - Subpage: Spawn

struct SpawnSubpage: View {
    @ObservedObject var state: AppState
    let apply: (String, AnyCodable) -> Void

    @State private var defaultHarness: String = "claude"
    @State private var pruneIdleSeconds: String = "300"

    private let harnesses = ["claude", "forge", "node", "bun", "custom"]

    var body: some View {
        Group {
            sectionHeader(
                title: "Spawn",
                subtitle: "Default harness for new spawns + idle prune threshold."
            )

            GroupBox {
                VStack(alignment: .leading, spacing: 14) {
                    HStack(spacing: 12) {
                        Text("spawn.default_harness")
                            .font(.system(.body, design: .monospaced))
                            .frame(width: 240, alignment: .leading)
                        Picker("", selection: $defaultHarness) {
                            ForEach(harnesses, id: \.self) { h in
                                Text(h).tag(h)
                            }
                        }
                        .labelsHidden()
                        .frame(width: 160)
                        .onChange(of: defaultHarness) { _, v in
                            apply("spawn.default_harness", .string(v))
                        }
                        Spacer()
                    }

                    NumericEditorRow(
                        label: "spawn.prune_idle_seconds",
                        key: "spawn.prune_idle_seconds",
                        value: $pruneIdleSeconds,
                        apply: apply,
                        range: 1...86400,
                        step: 30,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            v <= 0 ? "must be greater than 0" : nil
                        }
                    )
                }
                .padding(8)
            } label: {
                Label("Spawn defaults", systemImage: "arrow.up.forward.app")
                    .font(.headline)
            }
        }
    }
}

// MARK: - Subpage: Defaults (per-harness config.defaults.*)

struct DefaultsSubpage: View {
    @ObservedObject var state: AppState
    let apply: (String, AnyCodable) -> Void

    @AppStorage("config.defaults.harness") private var harnessRaw: String = "claude"
    @State private var harness: String = "claude"
    @State private var didLoadHarness = false

    @State private var maxInstances: String = "10"
    @State private var memoryLimitMB: String = "256"

    private let harnesses = ["claude", "forge", "node", "bun", "custom"]

    var body: some View {
        Group {
            sectionHeader(
                title: "Per-Harness Defaults",
                subtitle: "config.defaults.{harness}.* — spawned harnesses inherit these caps."
            )

            GroupBox {
                VStack(alignment: .leading, spacing: 14) {
                    HStack(spacing: 12) {
                        Text("harness")
                            .font(.system(.body, design: .monospaced))
                            .frame(width: 240, alignment: .leading)
                        Picker("", selection: $harness) {
                            ForEach(harnesses, id: \.self) { h in
                                Text(h).tag(h)
                            }
                        }
                        .labelsHidden()
                        .frame(width: 200)
                        .onChange(of: harness) { _, v in
                            harnessRaw = v
                        }
                        Spacer()
                    }

                    Divider()

                    HStack(spacing: 12) {
                        Text("key namespace")
                            .font(.caption).foregroundStyle(.secondary)
                            .frame(width: 240, alignment: .leading)
                        Text("config.defaults.\(harness).*")
                            .font(.system(.body, design: .monospaced))
                            .foregroundStyle(.secondary)
                        Spacer()
                    }

                    NumericEditorRow(
                        label: "max_instances",
                        key: "defaults.\(harness).max_instances",
                        value: $maxInstances,
                        apply: apply,
                        range: 1...200,
                        step: 1,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            v <= 0 ? "must be greater than 0" : nil
                        }
                    )

                    NumericEditorRow(
                        label: "memory_limit_mb",
                        key: "defaults.\(harness).memory_limit_mb",
                        value: $memoryLimitMB,
                        apply: apply,
                        range: 64...16384,
                        step: 64,
                        isInteger: true,
                        softWarning: { v in
                            if let h = state.health, v > Double(h.total_memory_mb) {
                                return "Total host memory is \(h.total_memory_mb) MB — this cap exceeds host memory"
                            }
                            return nil
                        },
                        hardError: { v in
                            v <= 0 ? "must be greater than 0" : nil
                        }
                    )

                    HStack(spacing: 12) {
                        Spacer().frame(width: 240)
                        Button {
                            resetHarness()
                        } label: {
                            Label("Reset \(harness) to default", systemImage: "arrow.counterclockwise")
                        }
                        .buttonStyle(.bordered)
                        .help("Reload defaults for \(harness) from the sidecar's config-defaults template")
                    }
                }
                .padding(8)
            } label: {
                Label("Per-harness defaults", systemImage: "wrench.and.screwdriver")
                    .font(.headline)
            }
        }
        .onAppear {
            if !didLoadHarness {
                harness = harnessRaw
                didLoadHarness = true
            }
        }
    }

    private func resetHarness() {
        // Apply the sharecli-side default by writing back to the configured
        // fallback. Defaults from `default_harness_configs()` in
        // src/config.rs:331 are encoded here so we can clear user overrides
        // without a new IPC round-trip.
        let defaults: [String: (maxInstances: Int, memoryLimitMB: Int)] = [
            "claude":  (11, 512),
            "forge":   (20, 256),
            "node":    (30, 256),
            "bun":     (10, 384),
            "custom":  (10, 256),
        ]
        if let d = defaults[harness] {
            maxInstances = String(d.maxInstances)
            memoryLimitMB = String(d.memoryLimitMB)
            apply("defaults.\(harness).max_instances", .int(d.maxInstances))
            apply("defaults.\(harness).memory_limit_mb", .int(d.memoryLimitMB))
        }
    }
}

// MARK: - Helpers

func sectionHeader(title: String, subtitle: String) -> some View {
    VStack(alignment: .leading, spacing: 2) {
        Text(title).font(.title2).bold()
        Text(subtitle).font(.caption).foregroundStyle(.secondary)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
}