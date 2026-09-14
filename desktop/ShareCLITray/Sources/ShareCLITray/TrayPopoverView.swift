/// TrayPopoverView.swift — compact popover (click tray icon → this appears).
///
/// Shows: connection status, summary stats, quick-action buttons, process list preview.

import SwiftUI
import ShareCLICore

struct TrayPopoverView: View {
    @ObservedObject var state: AppState
    let onOpenDashboard: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            headerBar
            Divider()
            statsRow
            operatorSection
            poolStatusSection
            Divider()
            processPreview
            Divider()
            actionBar
        }
        .frame(width: 360)
        .frame(minHeight: 200, idealHeight: 480)
        .fixedSize(horizontal: true, vertical: false)
        .background(.ultraThinMaterial)
    }

    // MARK: - Header

    private var headerBar: some View {
        HStack {
            Image(systemName: gateVisual.swiftSymbolName)
                .foregroundStyle(gateVisual.swiftColor)
            Text("ShareCLI")
                .font(.headline)
            Spacer()
            thermalBadge
            Circle()
                .fill(state.isConnected ? Color.green : Color.red)
                .frame(width: 8, height: 8)
            Text(state.isConnected ? "connected" : "disconnected")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }

    private var gateVisual: TrayGateVisual {
        if let h = state.health, state.isConnected {
            return OperatorDisplay.resolveTrayGateVisual(gate: h.gate, connected: true)
        }
        return OperatorDisplay.resolveTrayGateVisual(
            thermalPressure: "UNAVAILABLE",
            gateDecision: "UNAVAILABLE",
            connected: false
        )
    }

    private var thermalBadge: some View {
        Text(gateVisual.badgeLabel)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .foregroundStyle(gateVisual.swiftColor)
            .overlay(
                RoundedRectangle(cornerRadius: 4)
                    .stroke(gateVisual.swiftColor, lineWidth: 1)
            )
    }

    // MARK: - Stats row

    private var statsRow: some View {
        HStack(spacing: 0) {
            statCell(
                icon: "app.badge",
                value: "\(state.health?.managed_processes ?? state.processes.count)",
                label: "Processes"
            )
            Divider().frame(height: 36)
            statCell(
                icon: "memorychip",
                value: state.health.map { "\($0.used_memory_mb)M / \($0.total_memory_mb)M" } ?? "—",
                label: "Memory"
            )
            Divider().frame(height: 36)
            statCell(
                icon: gateVisual.swiftSymbolName,
                value: gateVisual.badgeLabel,
                label: "Status",
                iconColor: gateVisual.swiftColor
            )
        }
        .padding(.vertical, 8)
    }

    // MARK: - Operator gate / host_watch (AC-007.56)

    @ViewBuilder
    private var operatorSection: some View {
        if let h = state.health, state.isConnected {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(OperatorDisplay.formatOperatorTrayLines(gate: h.gate, host: h.host_watch), id: \.self) { line in
                    Text(line)
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
        }
    }

    @ViewBuilder
    private var poolStatusSection: some View {
        if let pool = state.poolStatus, let status = state.statusSnapshot, state.isConnected {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(OperatorDisplay.formatPoolStatusOperatorLines(pool: pool, status: status), id: \.self) { line in
                    Text(line)
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
        }
    }

    private func statCell(
        icon: String,
        value: String,
        label: String,
        iconColor: Color = .secondary
    ) -> some View {
        VStack(spacing: 2) {
            Image(systemName: icon).foregroundStyle(iconColor)
            Text(value).font(.system(.body, design: .monospaced)).bold()
            Text(label).font(.caption2).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: - Process preview list (top 5)

    private var processPreview: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Managed Processes")
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 12)
                .padding(.top, 8)

            if state.processes.isEmpty {
                Text(state.isConnected ? "No managed processes" : "Waiting for IPC…")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 16)
            } else {
                ForEach(state.processes.prefix(5)) { proc in
                    processRow(proc)
                }
                if state.processes.count > 5 {
                    Text("+ \(state.processes.count - 5) more…")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 12)
                        .padding(.bottom, 4)
                }
            }
        }
        .frame(minHeight: 60)
    }

    private func processRow(_ proc: ProcessSummary) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 1) {
                Text(proc.name)
                    .font(.system(.caption, design: .monospaced))
                    .lineLimit(1)
                HStack(spacing: 4) {
                    if let project = proc.project {
                        Badge(text: project, color: .blue)
                    }
                    if let harness = proc.harness {
                        Badge(text: harness, color: .purple)
                    }
                }
            }
            Spacer()
            Text("\(proc.memory_mb)M")
                .font(.caption2)
                .foregroundStyle(.secondary)
            Button {
                Task { await state.kill(pid: proc.pid) }
            } label: {
                Image(systemName: "xmark.circle")
                    .foregroundStyle(.red.opacity(0.7))
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 4)
    }

    // MARK: - Action bar

    private var actionBar: some View {
        HStack {
            Button("Dashboard") {
                onOpenDashboard()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)

            Spacer()

            Button {
                Task { await state.refresh() }
            } label: {
                Image(systemName: "arrow.clockwise")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .help("Refresh now")

            Button {
                Task {
                    if state.isPaused { await state.resumePolling() }
                    else { await state.pausePolling() }
                }
            } label: {
                Image(systemName: state.isPaused ? "play.fill" : "pause.fill")
                    .foregroundStyle(state.isPaused ? .green : .orange)
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .help(state.isPaused ? "Resume polling" : "Pause polling")

            Button("Kill All") {
                Task { await state.killAll() }
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .foregroundStyle(.red)

            Button {
                NSApp.terminate(nil)
            } label: {
                Image(systemName: "power")
                    .foregroundStyle(.red)
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .help("Quit ShareCLITray")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }
}

struct Badge: View {
    let text: String
    let color: Color

    var body: some View {
        Text(text)
            .font(.system(size: 9, weight: .medium))
            .padding(.horizontal, 4)
            .padding(.vertical, 1)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}
