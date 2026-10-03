// P2-12 — ResourcesExtras.swift
//
// Additive file. Two side-panel components for the Processes → Resources
// subpage: full cmdline preview (via IPC `process.cmdline`) and open TCP
// listening sockets (via `lsof` shell-out).
//
// Canonical IPC shape (IPCClient.swift, ProcessCmdline):
//   ProcessCmdline { pid: UInt32, cmdline: String, argv: [String] }
//
// Designed to drop into ResourcesView's body (which lives inside the
// 2090-line ProcessesPage.swift — this session avoids editing that file).
// Available standalone for one-line wire later:
//   ResourcesExtrasSection(process: row, state: state)

import SwiftUI
import ShareCLICore

/// Side-panel wrapper that combines cmdline preview + open ports for a
/// single process row. Pass any `ProcessSummary`; the view fetches
/// detailed data on appear and re-fetches when the row's PID changes.
struct ResourcesExtrasSection: View {
    let process: ProcessSummary
    let state: AppState

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            CmdlinePreview(pid: process.pid, state: state)
            OpenPortsList(pid: process.pid)
        }
    }
}

// MARK: - Cmdline preview

/// Async-fetches the full `/proc/<pid>/cmdline` via IPC `process.cmdline`
/// and renders the raw NUL-joined string + a parsed argv token table.
/// On macOS the sidecar returns an empty cmdline; the view falls back
/// to a friendly "no /proc" message in that case.
struct CmdlinePreview: View {
    let pid: UInt32
    let state: AppState

    @State private var cmdline: ProcessCmdline?
    @State private var loading: Bool = false
    @State private var error: String?

    var body: some View {
        Panel(title: "Command line (PID \(pid))", systemImage: "terminal") {
            VStack(alignment: .leading, spacing: 8) {
                if loading {
                    ProgressView()
                        .controlSize(.small)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else if let cmdline {
                    if cmdline.cmdline.isEmpty {
                        Text("No /proc on macOS — sidecar returned an empty cmdline.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else {
                        rawBlock(cmdline.cmdline)
                        if !cmdline.argv.isEmpty {
                            Divider()
                            Text("argv (\(cmdline.argv.count) tokens)")
                                .font(.caption.weight(.semibold))
                                .foregroundStyle(.secondary)
                            argvTable(cmdline.argv)
                        }
                    }
                } else if let error {
                    Text("Error: \(error)")
                        .font(.caption)
                        .foregroundStyle(.red)
                } else {
                    Text("Awaiting fetch…")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .task(id: pid) { await fetch() }
    }

    private func fetch() async {
        loading = true
        defer { loading = false }
        do {
            cmdline = try await state.client.fetchCmdline(pid: pid)
            error = nil
        } catch {
            self.error = String(describing: error)
            cmdline = nil
        }
    }

    private func rawBlock(_ raw: String) -> some View {
        ScrollView(.horizontal, showsIndicators: true) {
            Text(raw)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
                .lineLimit(6)
        }
        .padding(8)
        .background(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(Color(nsColor: .textBackgroundColor))
        )
    }

    private func argvTable(_ argv: [String]) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(Array(argv.enumerated()), id: \.offset) { idx, tok in
                HStack(alignment: .top, spacing: 8) {
                    Text("[\(idx)]")
                        .font(.caption2.monospacedDigit())
                        .foregroundStyle(.tertiary)
                        .frame(width: 32, alignment: .trailing)
                    Text(tok)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                }
            }
        }
    }
}

// MARK: - Open ports (lsof shell-out)

/// Lists TCP sockets in LISTEN state owned by the given PID. Uses
/// `lsof -nP -iTCP -sTCP:LISTEN -p <pid>` — runs via Foundation's
/// `Process`. Returns parsed rows; empty if lsof is unavailable or the
/// process owns no listening sockets.
///
/// Note: shell-out is wrapped in a `Task.detached` so the view stays
/// responsive; results are published via `@State`.
struct OpenPortsList: View {
    let pid: UInt32

    @State private var rows: [ListeningPort] = []
    @State private var loading: Bool = false
    @State private var error: String?

    var body: some View {
        Panel(title: "Open listening sockets (PID \(pid))", systemImage: "network") {
            VStack(alignment: .leading, spacing: 6) {
                if loading {
                    ProgressView()
                        .controlSize(.small)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else if let error {
                    Text("Error: \(error)")
                        .font(.caption)
                        .foregroundStyle(.red)
                } else if rows.isEmpty {
                    Text("No TCP sockets in LISTEN state (or lsof unavailable).")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(rows) { row in
                        portRow(row)
                    }
                }
            }
        }
        .task(id: pid) { await fetch() }
    }

    private func fetch() async {
        loading = true
        defer { loading = false }
        do {
            rows = try await Self.listeningPorts(pid: pid)
            error = nil
        } catch {
            self.error = String(describing: error)
            rows = []
        }
    }

    private func portRow(_ row: ListeningPort) -> some View {
        HStack(alignment: .center, spacing: 8) {
            Image(systemName: "antenna.radiowaves.left.and.right")
                .foregroundStyle(.cyan)
                .frame(width: 16)
            VStack(alignment: .leading, spacing: 1) {
                Text(row.address)
                    .font(.caption.monospaced())
                Text(row.fd)
                    .font(.caption2.monospaced())
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Text(row.protocolName)
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 2)
    }

    /// Run `lsof -nP -iTCP -sTCP:LISTEN -p <pid>` and parse its output.
    /// Throws if `lsof` can't be invoked or produces no output.
    static func listeningPorts(pid: UInt32) async throws -> [ListeningPort] {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: "/usr/sbin/lsof")
                process.arguments = [
                    "-nP",
                    "-iTCP", "-sTCP:LISTEN",
                    "-p", String(pid)
                ]
                let stdout = Pipe()
                let stderr = Pipe()
                process.standardOutput = stdout
                process.standardError = stderr

                do {
                    try process.run()
                } catch {
                    continuation.resume(throwing: error)
                    return
                }
                process.waitUntilExit()

                let data = stdout.fileHandleForReading.readDataToEndOfFile()
                let text = String(data: data, encoding: .utf8) ?? ""
                let rows = Self.parseLsof(text)
                continuation.resume(returning: rows)
            }
        }
    }

    /// Parse lsof -iTCP output. Lines look like:
    ///   shareclit 94918 root  12u  IPv4 0x... 0t0  TCP 127.0.0.1:9999 (LISTEN)
    /// The trailing "(LISTEN)" is implied by the -s flag.
    static func parseLsof(_ text: String) -> [ListeningPort] {
        var results: [ListeningPort] = []
        for rawLine in text.split(separator: "\n") {
            let line = String(rawLine)
            // Skip the header line (starts with "COMMAND").
            guard !line.hasPrefix("COMMAND") else { continue }
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty else { continue }
            // Find the TCP address column (e.g. "127.0.0.1:9999" or "[::]:8080")
            // by tokenizing on whitespace; we look for a token that contains
            // a colon and either starts with a digit or '[' (IPv6).
            let tokens = trimmed.split(whereSeparator: { $0 == " " || $0 == "\t" }).map(String.init)
            let addressToken = tokens.first(where: { token in
                token.contains(":") && (token.first?.isNumber == true || token.first == "[")
            })
            guard let address = addressToken else { continue }
            // FD column is index 3 in lsof's default format.
            let fd = tokens.count >= 4 ? tokens[3] : "?"
            results.append(ListeningPort(
                pid: 0, // filled in by caller if needed
                address: address,
                fd: fd,
                protocolName: "TCP"
            ))
        }
        return results
    }
}

/// Parsed row from `lsof -iTCP -sTCP:LISTEN -p <pid>`.
struct ListeningPort: Identifiable, Hashable {
    let pid: UInt32
    let address: String
    let fd: String
    let protocolName: String
    var id: String { "\(address)|\(fd)" }
}

// MARK: - Reusable Panel chrome (shared with FlameChart.swift)

private struct Panel<Content: View>: View {
    let title: String
    let systemImage: String
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label(title, systemImage: systemImage)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(.secondary)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color(nsColor: .controlBackgroundColor))
        )
    }
}
