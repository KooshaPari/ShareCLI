/// LogLine.swift — extracted log-line model and parser from LogsPage.
///
/// Contains: LogLine struct (with Level enum, timestamp formatting,
/// and the dual-format parse function for tracing-subscriber and JSON).
///
/// Part of closure B1 (house-limit decomposition).

import SwiftUI

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
