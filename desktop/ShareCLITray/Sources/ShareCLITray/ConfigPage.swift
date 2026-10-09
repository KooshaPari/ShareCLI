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

    /// Reduce motion (PLAN 1.23): gates the toast fade below.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

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
            Motion.run(.easeInOut(duration: 0.2), reduceMotion: reduceMotion) { toast = nil }
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
