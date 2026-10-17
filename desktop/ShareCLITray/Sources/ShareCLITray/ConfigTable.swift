/// ConfigTable.swift — extracted runtime, pool, and monitoring subpages.
///
/// Contains: RuntimeSubpage, PoolSubpage, MonitoringSubpage.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore


// MARK: - Subpage: Runtime

struct RuntimeSubpage: View {
    @ObservedObject var state: AppState
    let apply: (String, AnyCodable) -> Void

    @State private var maxMemoryMB: String = ""
    @State private var maxProcesses: String = ""
    @State private var didLoad = false

    /// Prefill editors from the live config via the existing IPC
    /// `config.get` plumbing so the hard validator doesn't fire on the
    /// empty initial value (parsed as 0).
    private func loadCurrentValues() {
        Task { @MainActor in
            guard let data = await state.getConfig() else { return }
            guard let obj = try? JSONDecoder().decode([String: AnyCodable].self, from: data) else { return }
            guard case .object(let runtime) = obj["runtime"] else { return }
            if maxMemoryMB.isEmpty, case .int(let mb)? = runtime["max_memory_mb"] {
                maxMemoryMB = "\(mb)"
            } else if maxMemoryMB.isEmpty, case .double(let mb)? = runtime["max_memory_mb"] {
                maxMemoryMB = isIntegerLike(mb) ? "\(Int(mb))" : "\(mb)"
            }
            if maxProcesses.isEmpty, case .int(let mp)? = runtime["max_processes"] {
                maxProcesses = "\(mp)"
            } else if maxProcesses.isEmpty, case .double(let mp)? = runtime["max_processes"] {
                maxProcesses = isIntegerLike(mp) ? "\(Int(mp))" : "\(mp)"
            }
        }
    }

    private func isIntegerLike(_ d: Double) -> Bool {
        d == d.rounded() && abs(d) < 1e12
    }

    var body: some View {
        Group {
            sectionHeader(
                title: "Runtime",
                subtitle: "Per-process resource caps. Affects gate decisions."
            )

            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    NumericEditorRow(
                        label: "runtime.max_memory_mb",
                        key: "runtime.max_memory_mb",
                        value: $maxMemoryMB,
                        apply: apply,
                        range: 64...65536,
                        step: 64,
                        isInteger: true,
                        softWarning: { v in
                            if let h = state.health, v > Double(h.total_memory_mb) {
                                return "Currently in effect total memory is \(h.total_memory_mb) MB — this cap exceeds host memory"
                            }
                            return nil
                        },
                        hardError: { v in
                            if v <= 0 { return "must be greater than 0" }
                            return nil
                        }
                    )

                    NumericEditorRow(
                        label: "runtime.max_processes",
                        key: "runtime.max_processes",
                        value: $maxProcesses,
                        apply: apply,
                        range: 1...1000,
                        step: 1,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            if v <= 0 { return "must be greater than 0" }
                            return nil
                        }
                    )
                }
                .padding(8)
            } label: {
                Label("Resource caps", systemImage: "memorychip")
                    .font(.headline)
            }

            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Currently in effect")
                        .font(.caption).foregroundStyle(.secondary)
                    if let h = state.health {
                        HStack(spacing: 24) {
                            previewStat(
                                title: "Used memory",
                                value: "\(h.used_memory_mb) MB",
                                color: h.used_memory_mb > h.total_memory_mb / 2 ? .orange : .blue
                            )
                            previewStat(
                                title: "Total memory",
                                value: "\(h.total_memory_mb) MB",
                                color: .secondary
                            )
                            previewStat(
                                title: "Managed processes",
                                value: "\(h.managed_processes)",
                                color: .purple
                            )
                            previewStat(
                                title: "Cap (current)",
                                value: maxMemoryMB.isEmpty ? "—" : "\(maxMemoryMB) MB",
                                color: .green
                            )
                        }
                        // Utilisation bar
                        GeometryReader { geo in
                            ZStack(alignment: .leading) {
                                RoundedRectangle(cornerRadius: 4).fill(.quaternary)
                                RoundedRectangle(cornerRadius: 4)
                                    .fill(h.used_memory_mb > h.total_memory_mb / 2 ? Color.orange : Color.blue)
                                    .frame(width: geo.size.width * CGFloat(h.used_memory_mb) / CGFloat(max(h.total_memory_mb, 1)))
                            }
                        }
                        .frame(height: 10)
                    } else if state.isConnected {
                        Text("Loading…")
                            .foregroundStyle(.secondary)
                    } else {
                        FailedActionBanner(
                            title: RetryCopy.disconnectedTitle,
                            detail: state.lastError,
                            systemImage: "wifi.slash",
                            actionTitle: RetryCopy.disconnectedStartLabel,
                            actionSystemImage: "play.fill",
                            actionHint: RetryCopy.startHint,
                            action: {
                                Task {
                                    await SidecarSupervisor.shared.ensureRunning()
                                    await state.refresh()
                                }
                            }
                        )
                    }
                }
                .padding(8)
            } label: {
                Label("Live preview (from monitoring.report)", systemImage: "eye")
                    .font(.headline)
            }
        }
        .onAppear {
            if !didLoad {
                didLoad = true
                loadCurrentValues()
            }
        }
    }

    private func previewStat(title: String, value: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.system(.body, design: .monospaced)).bold().foregroundStyle(color)
        }
    }
}

// MARK: - Subpage: Pool

struct PoolSubpage: View {
    @ObservedObject var state: AppState
    let apply: (String, AnyCodable) -> Void

    @State private var poolEnabled: Bool = true
    @State private var maxPerType: String = "5"
    @State private var idleTimeoutSecs: String = "300"
    @State private var maxAgeSecs: String = "3600"
    @State private var spawnDelayMs: String = "100"

    var body: some View {
        Group {
            sectionHeader(
                title: "Process Pool",
                subtitle: "Shared pool of node/bun processes — caps, lifetimes, spawn pacing."
            )

            GroupBox {
                VStack(alignment: .leading, spacing: 14) {
                    HStack {
                        Text("pool.enabled")
                            .font(.system(.body, design: .monospaced))
                            .frame(width: 240, alignment: .leading)
                        Toggle("", isOn: $poolEnabled)
                            .labelsHidden()
                            .onChange(of: poolEnabled) { _, v in
                                apply("pool.enabled", .bool(v))
                            }
                        Spacer()
                    }

                    NumericEditorRow(
                        label: "pool.max_per_type",
                        key: "pool.max_per_type",
                        value: $maxPerType,
                        apply: apply,
                        range: 1...100,
                        step: 1,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            v <= 0 ? "must be greater than 0" : nil
                        }
                    )

                    NumericEditorRow(
                        label: "pool.idle_timeout_secs",
                        key: "pool.idle_timeout_secs",
                        value: $idleTimeoutSecs,
                        apply: apply,
                        range: 1...3600,
                        step: 30,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            if v <= 0 { return "must be greater than 0" }
                            if v > 3600 { return "must be <= 3600 seconds" }
                            return nil
                        }
                    )

                    NumericEditorRow(
                        label: "pool.max_age_secs",
                        key: "pool.max_age_secs",
                        value: $maxAgeSecs,
                        apply: apply,
                        range: 60...86400,
                        step: 60,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            if v <= 0 { return "must be greater than 0" }
                            if v > 86400 { return "must be <= 86400 seconds (24 h)" }
                            return nil
                        }
                    )

                    NumericEditorRow(
                        label: "pool.spawn_delay_ms",
                        key: "pool.spawn_delay_ms",
                        value: $spawnDelayMs,
                        apply: apply,
                        range: 1...10000,
                        step: 10,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            v <= 0 ? "must be greater than 0" : nil
                        }
                    )
                }
                .padding(8)
            } label: {
                Label("Pool settings", systemImage: "rectangle.stack")
                    .font(.headline)
            }
        }
    }
}

// MARK: - Subpage: Monitoring

struct MonitoringSubpage: View {
    @ObservedObject var state: AppState
    let apply: (String, AnyCodable) -> Void

    @State private var healthCheckInterval: String = "30"
    @State private var idleThresholdSecs: String = "300"
    @State private var highMemThreshold: String = "4096"

    var body: some View {
        Group {
            sectionHeader(
                title: "Monitoring",
                subtitle: "Health-check cadence + thresholds that drive warnings and gate decisions."
            )

            GroupBox {
                VStack(alignment: .leading, spacing: 14) {
                    NumericEditorRow(
                        label: "monitoring.health_check_interval_secs",
                        key: "monitoring.health_check_interval_secs",
                        value: $healthCheckInterval,
                        apply: apply,
                        range: 1...3600,
                        step: 5,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            if v <= 0 { return "must be greater than 0" }
                            if v > 3600 { return "must be <= 3600 seconds" }
                            return nil
                        }
                    )

                    NumericEditorRow(
                        label: "monitoring.idle_threshold_secs",
                        key: "monitoring.idle_threshold_secs",
                        value: $idleThresholdSecs,
                        apply: apply,
                        range: 1...86400,
                        step: 30,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            v <= 0 ? "must be greater than 0" : nil
                        }
                    )

                    NumericEditorRow(
                        label: "monitoring.high_memory_threshold_mb",
                        key: "monitoring.high_memory_threshold_mb",
                        value: $highMemThreshold,
                        apply: apply,
                        range: 64...65536,
                        step: 64,
                        isInteger: true,
                        softWarning: { _ in nil },
                        hardError: { v in
                            v <= 0 ? "must be greater than 0" : nil
                        }
                    )
                }
                .padding(8)
            } label: {
                Label("Monitoring thresholds", systemImage: "waveform.path.ecg")
                    .font(.headline)
            }
        }
    }
}

