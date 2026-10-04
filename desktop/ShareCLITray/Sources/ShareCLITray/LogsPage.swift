/// LogsPage.swift — Stream / Filter / Export view over the sharecli log file.
///
/// The Rust sidecar writes structured tracing-subscriber output to
/// `~/.sharecli/logs/sharecli.log` (configurable via `SHARECLI_LOG_PATH`).
/// This view tails that file with `DispatchSource.makeFileSystemObjectSource`
/// (FSEvents-style) so it streams new lines live without polling. Each line is
/// decoded into a `LogLine` (level / timestamp / target / message) and shown
/// in a virtualised `List`.
///
/// Filter bar:
///   • text — substring matches against the message
///   • level — multi-select of TRACE / DEBUG / INFO / WARN / ERROR
///   • "Auto-tail" toggle — when on, the view follows new lines; when off, it
///     locks to the position the user scrolled to (they can scroll up to read
///     history without being yanked back).
///
/// Export bar:
///   • Snapshot — append a `.sharecli-logsnap-<ts>.log` line separator and copy
///     the current view filter to a file via `NSSavePanel`.
///   • Copy — copy visible lines to clipboard.

import SwiftUI
import ShareCLICore

// MARK: - Failed-action model (PLAN.md:228-231, task 1.22)

/// Which failed action a Logs page error banner belongs to. Each kind maps to
/// exactly one Retry control that re-runs the operation that failed, so the
/// page never shows a bare developer string with no way to recover.
enum LogsFailure: Equatable {
    /// `FileHandle(forReadingFrom:)` (or the read) failed.
    case openLogFile(String)
    /// Writing the filtered view to disk failed.
    case export(String)

    /// User-visible message: an actionable lead plus the raw error as detail.
    var message: String {
        switch self {
        case .openLogFile(let detail): return "Couldn't open the log file — \(detail)"
        case .export(let detail): return "Couldn't export the visible logs — \(detail)"
        }
    }

    /// The single action label for this failure kind ("one Retry button").
    var retryLabel: String { RetryCopy.retryLabel }

    /// Tooltip explaining what Retry will re-run.
    var actionHint: String {
        switch self {
        case .openLogFile: return "Retry opening the log file"
        case .export: return "Retry export"
        }
    }

    /// Exactly one action affordance per failure.
    var actionTitles: [String] { [retryLabel] }
}

struct LogsPage: View {
    @ObservedObject var state: AppState
    @AppStorage("logs.tailpaused") var tailPaused: Bool = false
    @AppStorage("logs.filterText") var filterText: String = ""
    @AppStorage("logs.filterLevels") var filterLevelsCSV: String = "DEBUG,INFO,WARN,ERROR"
    @FocusState private var filterFocused: Bool
    @State private var lines: [LogLine] = []
    @State private var logsFailure: LogsFailure? = nil
    @State private var lastRefresh: Date = .distantPast
    @State private var fileHandle: FileHandle? = nil
    @State private var fileSource: DispatchSourceFileSystemObject? = nil
    @State private var readBuffer = Data()
    @State private var lastSnapshot: Date = .distantPast
    @State private var copyToast: String? = nil

    var body: some View {
        if state.statusSnapshot?.live_log_path == nil {
            EmptyStateView(
                icon: "text.alignleft",
                title: "No log file configured",
                subtitle: "The sidecar didn't report a log_location. This usually means SHARECLI_LOG_PATH isn't set (or the file isn't writable). Set SHARECLI_LOG_PATH=/path/to/sharecli.log in the sidecar's environment, then restart the daemon.",
                variant: .hero,
                primaryTitle: "Refresh now",
                primaryIcon: "arrow.clockwise",
                primaryAction: { Task { await state.refresh() } },
                secondaryTitle: "Logging docs",
                secondaryIcon: "book",
                secondaryAction: {
                    if let url = URL(string: "https://docs.sharecli.dev/logs") { NSWorkspace.shared.open(url) }
                }
            )
        } else {
            HSplitView {
            // Filter / source / export bar (left side)
            VStack(alignment: .leading, spacing: 12) {
                Text("Logs")
                    .font(.headline)
                if let path = state.statusSnapshot?.live_log_path {
                    LabeledContent("Source") {
                        Text(path.path)
                            .font(.system(.caption, design: .monospaced))
                            .lineLimit(2)
                            .truncationMode(.middle)
                            .help(path.path)
                    }
                    LabeledContent("Size") {
                        Text(fileSizeString(for: path))
                            .font(.system(.body, design: .monospaced))
                    }
                } else {
                    Label("No log file reported — set SHARECLI_LOG_PATH for the sidecar", systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                        .font(.caption)
                }
                LabeledContent("Last refresh") {
                    Text(lastRefresh, style: .time)
                        .font(.system(.body, design: .monospaced))
                }
                Divider()
                LabeledContent("Filter text") {
                    TextField("substring…", text: $filterText)
                        .textFieldStyle(.roundedBorder)
                        .filterFieldFocus($filterFocused)
                }
                VStack(alignment: .leading, spacing: 6) {
                    Text("Levels")
                        .font(.caption)
                    LevelChips(state: self)
                }
                Toggle("Auto-tail (follow new lines)", isOn: Binding(
                    get: { !tailPaused },
                    set: { tailPaused = !$0 }
                ))
                .toggleStyle(.checkbox)
                Divider()
                HStack(spacing: 8) {
                    Button {
                        Task { await refreshFromFile() }
                    } label: {
                        Label("Refresh now", systemImage: "arrow.clockwise")
                    }
                    Button {
                        copyVisibleToClipboard()
                    } label: {
                        Label("Copy visible", systemImage: "doc.on.doc")
                    }
                    .disabled(filtered.isEmpty)
                    Button {
                        exportVisibleToFile()
                    } label: {
                        Label("Export…", systemImage: "square.and.arrow.up")
                    }
                    .disabled(filtered.isEmpty)
                }
                if let failure = logsFailure {
                    FailedActionBanner(
                        title: failure.message,
                        systemImage: "exclamationmark.triangle.fill",
                        actionTitle: failure.retryLabel,
                        actionSystemImage: "arrow.clockwise",
                        actionHint: failure.actionHint,
                        action: { retry(failure) }
                    )
                    .font(.caption)
                }
                if let toast = copyToast {
                    Text(toast)
                        .font(.caption)
                        .foregroundStyle(.green)
                }
                Spacer()
            }
            .padding(16)
            .frame(minWidth: 280, idealWidth: 320)

            // Log lines (right side)
            LogsListView(
                lines: filtered,
                followTail: !tailPaused,
                onUserScroll: { tailPaused = true }
            )
            .frame(minWidth: 480)
        }
        .task { await startStreaming() }
        .onChange(of: state.statusSnapshot?.live_log_path) { _, _ in
            Task { await startStreaming() }
        }
        } // end else
    }

    // MARK: - Filtering

    private var filtered: [LogLine] {
        let allowedLevels = Set(filterLevelsCSV.split(separator: ",").map(String.init))
        let needle = filterText.trimmingCharacters(in: .whitespaces)
        return lines.filter { line in
            let levelOK = allowedLevels.contains(line.level.rawValue)
            let textOK = needle.isEmpty || line.message.localizedCaseInsensitiveContains(needle)
                || line.target.localizedCaseInsensitiveContains(needle)
            return levelOK && textOK
        }
    }

    // MARK: - Streaming

    private func startStreaming() async {
        guard let path = state.statusSnapshot?.live_log_path else {
            lines = []
            logsFailure = nil
            return
        }
        stopStreaming()
        do {
            let handle = try FileHandle(forReadingFrom: path)
            self.fileHandle = handle
            // Seek to end-1MB so we don't load the entire log on every refresh,
            // but also so we include the most recent context.
            let totalSize = (try? FileManager.default.attributesOfItem(atPath: path.path)[.size] as? UInt64) ?? 0
            let startOffset: UInt64 = totalSize > (1 << 20) ? totalSize - (1 << 20) : 0
            try handle.seek(toOffset: startOffset)
            let initial = try handle.readToEnd() ?? Data()
            self.readBuffer = initial
            decodeBuffer()
            await MainActor.run { self.lastRefresh = Date() }
            let fd = handle.fileDescriptor
            let src = DispatchSource.makeFileSystemObjectSource(
                fileDescriptor: fd,
                eventMask: [.extend, .write, .delete, .rename],
                queue: .global(qos: .utility)
            )
            src.setEventHandler {
                Task { await self.refreshFromFile() }
            }
            src.setCancelHandler { }
            src.resume()
            self.fileSource = src
            logsFailure = nil
        } catch {
            self.fileHandle = nil
            self.fileSource = nil
            logsFailure = .openLogFile(error.localizedDescription)
        }
    }

    private func stopStreaming() {
        fileSource?.cancel()
        fileSource = nil
        try? fileHandle?.close()
        fileHandle = nil
        readBuffer = Data()
        lines = []
    }

    private func refreshFromFile() async {
        guard let handle = fileHandle else { return }
        let new = (try? handle.readToEnd()) ?? Data()
        if !new.isEmpty {
            readBuffer.append(new)
            decodeBuffer()
        }
        await MainActor.run {
            self.lastRefresh = Date()
        }
    }

    private func decodeBuffer() {
        // Split on \n; trailing partial stays in buffer for next tick.
        var all: [LogLine] = []
        var pending = readBuffer
        while let nl = pending.firstIndex(of: 0x0A) {
            let lineData = pending.subdata(in: pending.startIndex..<nl)
            if let str = String(data: lineData, encoding: .utf8) {
                all.append(LogLine.parse(str))
            }
            pending = pending.subdata(in: (nl + 1)..<pending.endIndex)
        }
        readBuffer = pending
        // Cap the in-memory buffer at 5000 lines to keep the SwiftUI render
        // happy; the user can scroll-up history via export if needed.
        if all.count > 5000 {
            all.removeFirst(all.count - 5000)
        }
        self.lines = all
    }

    // MARK: - Helpers

    private func fileSizeString(for url: URL) -> String {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: url.path),
              let size = attrs[.size] as? UInt64 else {
            return "—"
        }
        return ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file)
    }

    private func copyVisibleToClipboard() {
        let text = filtered.map(\.raw).joined(separator: "\n") + "\n"
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(text, forType: .string)
        copyToast = "Copied \(filtered.count) line(s) to clipboard"
        Task {
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            await MainActor.run { copyToast = nil }
        }
    }

    private func exportVisibleToFile() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.log]
        let stamp = ISO8601DateFormatter().string(from: Date()).replacingOccurrences(of: ":", with: "-")
        panel.nameFieldStringValue = "sharecli-logs-\(stamp).log"
        panel.canCreateDirectories = true
        let response = panel.runModal()
        guard response == .OK, let url = panel.url else { return }
        let text = filtered.map(\.raw).joined(separator: "\n") + "\n"
        do {
            try text.write(to: url, atomically: true, encoding: .utf8)
            copyToast = "Exported to \(url.lastPathComponent)"
            Task {
                try? await Task.sleep(nanoseconds: 1_500_000_000)
                await MainActor.run { copyToast = nil }
            }
        } catch {
            logsFailure = .export(error.localizedDescription)
        }
    }

    /// Re-run the exact action that failed. One control per failure kind
    /// (PLAN.md:228-231, task 1.22).
    private func retry(_ failure: LogsFailure) {
        switch failure {
        case .openLogFile:
            Task { await startStreaming() }
        case .export:
            exportVisibleToFile()
        }
    }
}

// MARK: - Level chips

private struct LevelChips: View {
    let state: LogsPage
    private let order: [LogLine.Level] = [.trace, .debug, .info, .warn, .error]
    var body: some View {
        HStack(spacing: 4) {
            ForEach(order, id: \.rawValue) { level in
                let enabled = state.filterLevelsCSV.split(separator: ",").map(String.init).contains(level.rawValue)
                Button {
                    toggle(level)
                } label: {
                    Text(level.rawValue.uppercased())
                        .font(.caption2.monospaced())
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(enabled ? level.color : Color.gray.opacity(0.2))
                        .foregroundStyle(enabled ? .white : .secondary)
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                }
                .buttonStyle(.plain)
            }
        }
    }
    private func toggle(_ level: LogLine.Level) {
        var current = state.filterLevelsCSV.split(separator: ",").map(String.init)
        if let idx = current.firstIndex(of: level.rawValue) {
            current.remove(at: idx)
        } else {
            current.append(level.rawValue)
        }
        state.filterLevelsCSV = current.joined(separator: ",")
    }
}

// MARK: - List of lines

private struct LogsListView: View {
    let lines: [LogLine]
    let followTail: Bool
    let onUserScroll: () -> Void
    /// Reduce motion (PLAN 1.23): gates the tail auto-scroll animation.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(lines, id: \.id) { line in
                        LogsLineRow(line: line)
                            .id(line.id)
                    }
                }
                .padding(.vertical, 4)
            }
            .background(Color.black.opacity(0.04))
            .gesture(
                DragGesture(minimumDistance: 5)
                    .onChanged { _ in onUserScroll() }
            )
            .onChange(of: lines.last?.id) { _, newID in
                if followTail, let id = newID {
                    Motion.run(.linear(duration: 0.1), reduceMotion: reduceMotion) {
                        proxy.scrollTo(id, anchor: .bottom)
                    }
                }
            }
        }
    }
}

private struct LogsLineRow: View {
    let line: LogLine
    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Text(line.timestampString)
                .font(.system(.caption2, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 70, alignment: .leading)
            Text(line.level.rawValue.uppercased())
                .font(.system(.caption2, design: .monospaced).bold())
                .foregroundStyle(.white)
                .padding(.horizontal, 4)
                .padding(.vertical, 1)
                .background(line.level.color)
                .clipShape(RoundedRectangle(cornerRadius: 3))
                .frame(width: 50)
            Text(line.target)
                .font(.system(.caption2, design: .monospaced))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(width: 110, alignment: .leading)
            Text(line.message)
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.primary)
                .textSelection(.enabled)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 2)
        .background(line.level.bgTint)
    }
}

// MARK: - LogLine model

struct LogLine: Identifiable, Hashable {
    enum Level: String, Hashable {
        case trace, debug, info, warn, error, unknown = ""
        var color: Color {
            switch self {
            case .trace: return .gray
            case .debug: return .blue.opacity(0.7)
            case .info: return .green.opacity(0.7)
            case .warn: return .orange
            case .error: return .red
            case .unknown: return .secondary
            }
        }
        var bgTint: Color {
            switch self {
            case .warn: return Color.orange.opacity(0.06)
            case .error: return Color.red.opacity(0.08)
            default: return Color.clear
            }
        }
    }
    let id: UUID = UUID()
    let raw: String
    let level: Level
    let timestamp: Date?
    let target: String
    let message: String

    var timestampString: String {
        guard let ts = timestamp else { return "—" }
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f.string(from: ts)
    }

    static func parse(_ line: String) -> LogLine {
        // tracing-subscriber default format:
        //   2024-05-01T12:00:00.123456Z  LEVEL target: message
        // JSON format:
        //   {"timestamp":"...","level":"INFO","target":"...","message":"..."}
        if line.hasPrefix("{") {
            if let data = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                let lvl = (obj["level"] as? String).flatMap(Level.init(rawValue:)) ?? .unknown
                let target = (obj["target"] as? String) ?? ""
                let message = (obj["fields"] as? [String: Any]).flatMap { ($0["message"] as? String) }
                    ?? (obj["message"] as? String)
                    ?? line
                let timestamp: Date? = {
                    if let s = obj["timestamp"] as? String {
                        let iso = ISO8601DateFormatter()
                        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
                        return iso.date(from: s) ?? ISO8601DateFormatter().date(from: s)
                    }
                    return nil
                }()
                return LogLine(raw: line, level: lvl, timestamp: timestamp, target: target, message: message)
            }
        }
        // Plain-text format: `<ts>  <level> <target>: <message>`
        let parts = line.split(separator: " ", maxSplits: 3, omittingEmptySubsequences: true)
        if parts.count >= 4 {
            let tsString = String(parts[0])
            let levelRaw = String(parts[1]).lowercased()
            let level = Level(rawValue: levelRaw) ?? .unknown
            let rest = String(parts[3])
            let colonIdx = rest.firstIndex(of: ":")
            let target: String
            let message: String
            if let ci = colonIdx {
                target = String(rest[..<ci])
                message = String(rest[rest.index(after: ci)...]).trimmingCharacters(in: .whitespaces)
            } else {
                target = ""
                message = rest
            }
            let iso = ISO8601DateFormatter()
            iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            let timestamp = iso.date(from: tsString) ?? ISO8601DateFormatter().date(from: tsString)
            return LogLine(raw: line, level: level, timestamp: timestamp, target: target, message: message)
        }
        return LogLine(raw: line, level: .unknown, timestamp: nil, target: "", message: line)
    }
}
