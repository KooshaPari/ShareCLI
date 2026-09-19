/// SidecarProcessProbe.swift — `ps`-based inspection of running `sharecli-ipc`
/// processes, plus the terminate helper used to take over a wedged one.
///
/// The supervisor must never launch a second sidecar while one is running, so it
/// needs an answer to "is a sharecli-ipc process alive?" that is independent of
/// the socket. Parsing and the ownership rules live here, away from the loop, so
/// they are unit-testable without spawning anything.

import Foundation
import Darwin

/// One row of `ps -axo pid=,ppid=,state=,comm=`.
public struct SidecarProcessEntry: Equatable, Sendable {
    public let pid: Int32
    public let ppid: Int32
    public let state: String
    public let comm: String

    public init(pid: Int32, ppid: Int32, state: String, comm: String) {
        self.pid = pid
        self.ppid = ppid
        self.state = state
        self.comm = comm
    }

    /// Executable basename. macOS prints a full path for `comm` and Linux the
    /// basename, so compare on the last path component.
    public var name: String { (comm as NSString).lastPathComponent }

    /// A dead child the parent has not reaped yet. It must not count as a running
    /// sidecar, or the tray would refuse to relaunch after a crash.
    public var isZombie: Bool { state.hasPrefix("Z") }
}

public enum SidecarProcessProbe {
    /// Parse `ps -axo pid=,ppid=,state=,comm=` output. Rows without two numeric
    /// ids and a state are dropped; `comm` may contain spaces (paths), so it is
    /// everything after the third field.
    public static func parse(_ output: String) -> [SidecarProcessEntry] {
        var entries: [SidecarProcessEntry] = []
        for line in output.split(separator: "\n", omittingEmptySubsequences: true) {
            let fields = line.split(separator: " ", omittingEmptySubsequences: true)
            guard fields.count >= 4,
                  let pid = Int32(fields[0]),
                  let ppid = Int32(fields[1]) else { continue }
            entries.append(
                SidecarProcessEntry(
                    pid: pid,
                    ppid: ppid,
                    state: String(fields[2]),
                    comm: fields[3...].joined(separator: " ")
                )
            )
        }
        return entries
    }

    /// Non-zombie processes whose executable basename is `binaryName`.
    public static func liveSidecars(
        in entries: [SidecarProcessEntry],
        binaryName: String = "sharecli-ipc"
    ) -> [SidecarProcessEntry] {
        entries.filter { !$0.isZombie && $0.name == binaryName }
    }

    /// Processes this tray may terminate to recover the socket: its own direct
    /// children, or orphans reparented to launchd. A sidecar that is the child of
    /// another live process (for example a second tray) is left alone.
    public static func takeoverCandidates(
        in entries: [SidecarProcessEntry],
        ownPID: Int32,
        binaryName: String = "sharecli-ipc"
    ) -> [SidecarProcessEntry] {
        liveSidecars(in: entries, binaryName: binaryName)
            .filter { $0.ppid == ownPID || $0.ppid <= 1 }
    }

    /// Run `ps` and parse it. Blocking; callers run it off the main thread via
    /// `snapshotAsync()`.
    public static func snapshot(psPath: String = "/bin/ps") -> [SidecarProcessEntry] {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: psPath)
        proc.arguments = ["-axo", "pid=,ppid=,state=,comm="]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
        } catch {
            return []
        }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        proc.waitUntilExit()
        guard let text = String(data: data, encoding: .utf8) else { return [] }
        return parse(text)
    }

    /// `snapshot()` off the main thread.
    public static func snapshotAsync() async -> [SidecarProcessEntry] {
        await withCheckedContinuation { cont in
            DispatchQueue.global(qos: .utility).async {
                cont.resume(returning: snapshot())
            }
        }
    }

    /// SIGTERM the given PIDs, then SIGKILL anything still alive after
    /// `graceSeconds`. Runs off the main thread.
    public static func terminate(pids: [Int32], graceSeconds: TimeInterval) async {
        guard !pids.isEmpty else { return }
        await withCheckedContinuation { cont in
            DispatchQueue.global(qos: .utility).async {
                for pid in pids { kill(pid, SIGTERM) }
                let deadline = Date().addingTimeInterval(graceSeconds)
                while Date() < deadline, pids.contains(where: { kill($0, 0) == 0 }) {
                    usleep(100_000)
                }
                for pid in pids where kill(pid, 0) == 0 { kill(pid, SIGKILL) }
                usleep(200_000)
                cont.resume()
            }
        }
    }
}
