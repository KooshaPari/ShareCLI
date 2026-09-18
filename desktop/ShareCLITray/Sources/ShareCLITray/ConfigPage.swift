/// ConfigPage.swift — expanded Config page (PR 7 of dashboard expansion plan).
///
/// Replaces the single `ConfigEditorView` inside `DashboardView` with a
/// segmented 5-subpage layout:
///
///   ┌─────────────────────────────────────────────────────────────────────┐
///   │ [Runtime] [Pool] [Monitoring] [Spawn] [Defaults]                   │
///   ├─────────────────────────────────────────────────────────────────────┤
///   │ Runtime:    max_memory_mb + max_processes (slider+input+preview)   │
///   │             live "Currently in effect" preview from monitoring     │
///   │ Pool:       enabled toggle + max_per_type / idle_timeout_secs /    │
///   │             max_age_secs / spawn_delay_ms                          │
///   │ Monitoring: health_check_interval_secs / idle_threshold_secs /     │
///   │             high_memory_threshold_mb                               │
///   │ Spawn:      default_harness picker + prune_idle_seconds            │
///   │ Defaults:   per-harness editor for `config.defaults.{harness}.*`   │
///   │             (max_instances + memory_limit_mb + reset button)       │
///   └─────────────────────────────────────────────────────────────────────┘
///
/// Each numeric editor is a labelled row containing a slider and a numeric
/// input. Edits call `state.setConfig(key:value:)` via the existing IPC
/// `config.set` method (no IPC changes). A success/failure toast is shown
/// for ~2 s after every apply.
///
/// Validation rules mirror `src/config_validator.rs` (hard validator) and
/// surface as a non-blocking warning label under the offending field. We do
/// NOT block the save — the Rust validator does that on `sharecli validate`.
/// We DO warn users about obvious context issues (e.g. max_memory_mb >
/// total_memory_mb reported by `monitoring.report`).
///
/// Persistence:
///   - `config.subpage` (String: "runtime" | "pool" | "monitoring" |
///     "spawn" | "defaults")
///   - `config.defaults.harness` (String: selected harness in Defaults tab)
///
/// Part of: plans/2026-07-25-tray-dashboard-expanded-v1.md §2.1 Page 5
/// (Config), Subpanels 5a–5e.

import SwiftUI
import ShareCLICore

// MARK: - Top-level page

struct ConfigPage: View {
    @ObservedObject var state: AppState

    @AppStorage("config.subpage") private var subpageRaw: String = ConfigSubpage.runtime.rawValue
    @State private var subpage: ConfigSubpage = .runtime
    @State private var didLoadSubpage = false

    // Toast (success / failure message after config.set)
    @State private var toast: ConfigToast? = nil

    enum ConfigSubpage: String, CaseIterable, Identifiable {
        case runtime = "runtime"
        case pool = "pool"
        case monitoring = "monitoring"
        case spawn = "spawn"
        case defaults = "defaults"

        var id: String { rawValue }
        var label: String {
            switch self {
            case .runtime: return "Runtime"
            case .pool: return "Pool"
            case .monitoring: return "Monitoring"
            case .spawn: return "Spawn"
            case .defaults: return "Defaults"
            }
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            header

            Picker("", selection: $subpage) {
                ForEach(ConfigSubpage.allCases) { sp in
                    Text(sp.label).tag(sp)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .onChange(of: subpage) { _, newValue in
                subpageRaw = newValue.rawValue
            }

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    switch subpage {
                    case .runtime:
                        RuntimeSubpage(state: state, apply: apply)
                    case .pool:
                        PoolSubpage(state: state, apply: apply)
                    case .monitoring:
                        MonitoringSubpage(state: state, apply: apply)
                    case .spawn:
                        SpawnSubpage(state: state, apply: apply)
                    case .defaults:
                        DefaultsSubpage(state: state, apply: apply)
                    }
                }
                .padding(16)
            }

            if let t = toast {
                ToastBar(toast: t)
            }
        }
        .frame(minWidth: 720, minHeight: 460)
        .onAppear {
            if !didLoadSubpage {
                subpage = ConfigSubpage(rawValue: subpageRaw) ?? .runtime
                didLoadSubpage = true
            }
        }
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("Configuration")
                    .font(.largeTitle).bold()
                Text("Edits call config.set via IPC · restart sharecli for some keys to take effect")
                    .font(.caption2).foregroundStyle(.secondary)
            }
            Spacer()
            if state.isConnected {
                Label("connected", systemImage: "circle.fill")
                    .font(.caption).foregroundStyle(.green)
            } else {
                Label("offline", systemImage: "circle.fill")
                    .font(.caption).foregroundStyle(.red)
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 12)
        .padding(.bottom, 4)
    }

    /// Apply a config patch via the existing IPC `config.set` plumbing.
    /// `key` is a dotted path (e.g. `runtime.max_memory_mb`).
    private func apply(_ key: String, value: AnyCodable) {
        Task { @MainActor in
            let priorError = state.lastError
            await state.setConfig(key: key, value: value)
            // Inspect after a microtask hop so the @Published write
            // from setConfig's catch block has time to flush.
            try? await Task.sleep(nanoseconds: 50_000_000)
            if let err = state.lastError, err != priorError {
                toast = ConfigToast(level: .error, message: "\(key): \(err)")
            } else {
                toast = ConfigToast(level: .success, message: "Applied \(key)")
            }
            try? await Task.sleep(nanoseconds: 2_000_000_000)
            withAnimation(.easeInOut(duration: 0.2)) { toast = nil }
        }
    }
}

// MARK: - Toast

private struct ConfigToast: Equatable {
    enum Level: Equatable { case success, error }
    let level: Level
    let message: String
}

private struct ToastBar: View {
    let toast: ConfigToast
    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: toast.level == .success
                  ? "checkmark.circle.fill"
                  : "exclamationmark.triangle.fill")
                .foregroundStyle(toast.level == .success ? .green : .red)
            Text(toast.message)
                .font(.caption)
                .foregroundStyle(toast.level == .success ? .green : .red)
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
        .background(.quaternary)
    }
}

// MARK: - Numeric editor (slider + numeric input + validation warning)

/// A reusable labelled row with a slider + numeric input. Edits are
/// applied via the supplied closure on commit (text-field end-editing or
/// slider release).
struct NumericEditorRow: View {
    let label: String
    let key: String
    let value: Binding<String>
    let apply: (String, AnyCodable) -> Void
    let range: ClosedRange<Double>
    let step: Double
    let isInteger: Bool

    /// Optional validator returning a non-empty warning string when the
    /// current value violates a soft rule (e.g. exceeds total memory).
    let softWarning: (Double) -> String?

    /// Optional hard validator (mirrors src/config_validator.rs).
    let hardError: (Double) -> String?

    @State private var warning: String? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 12) {
                Text(label)
                    .font(.system(.body, design: .monospaced))
                    .frame(width: 240, alignment: .leading)

                Slider(
                    value: Binding(
                        get: { Double(value.wrappedValue) ?? 0 },
                        set: { newVal in
                            let formatted = isInteger
                                ? String(Int(newVal.rounded()))
                                : String(newVal)
                            if value.wrappedValue != formatted {
                                value.wrappedValue = formatted
                                validate()
                            }
                        }
                    ),
                    in: range,
                    step: step
                )
                .onChange(of: value.wrappedValue) { _, _ in
                    validate()
                }
                .frame(minWidth: 160)

                TextField("", text: value)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 100)
                    .multilineTextAlignment(.trailing)
                    .onSubmit { commit() }
                    .onChange(of: value.wrappedValue) { _, _ in validate() }

                Button("Apply") { commit() }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(hardError(parsedValue()) != nil)
            }

            if let w = warning {
                Text(w)
                    .font(.caption2)
                    .foregroundStyle(.orange)
                    .padding(.leading, 252)
            }
            if let err = hardError(parsedValue()) {
                Text("⚠ \(err)  (Rust validator will reject on save)")
                    .font(.caption2)
                    .foregroundStyle(.red)
                    .padding(.leading, 252)
            }
        }
        .onAppear { validate() }
    }

    private func parsedValue() -> Double {
        Double(value.wrappedValue) ?? 0
    }

    private func validate() {
        let v = parsedValue()
        warning = softWarning(v)
    }

    private func commit() {
        let v = parsedValue()
        if isInteger {
            apply(key, .int(Int(v.rounded())))
        } else {
            apply(key, .double(v))
        }
    }
}

// MARK: - Subpage: Runtime

private struct RuntimeSubpage: View {
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
                    } else {
                        Text(state.isConnected ? "Loading…" : "Not connected to sharecli-ipc")
                            .foregroundStyle(.secondary)
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

private struct PoolSubpage: View {
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

private struct MonitoringSubpage: View {
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

// MARK: - Subpage: Spawn

private struct SpawnSubpage: View {
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

private struct DefaultsSubpage: View {
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

private func sectionHeader(title: String, subtitle: String) -> some View {
    VStack(alignment: .leading, spacing: 2) {
        Text(title).font(.title2).bold()
        Text(subtitle).font(.caption).foregroundStyle(.secondary)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
}