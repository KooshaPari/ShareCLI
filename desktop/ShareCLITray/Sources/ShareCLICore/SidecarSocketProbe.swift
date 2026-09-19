/// SidecarSocketProbe.swift — liveness and responsiveness probes for the IPC
/// socket, plus the continuation gate that keeps a racing probe safe.
///
/// Two probes with very different costs, measured against the live sidecar on
/// 2026-09-19:
///
///   * `connect(2)` — 0.01-0.13 ms, and the sidecar's only work is accepting and
///     immediately seeing EOF. Used every tick to decide "is anything listening?"
///   * `health.status` — 0.4-2.1 s, because the sidecar rescans sysinfo. Used only
///     where responsiveness (not just reachability) is the question: confirming a
///     fresh spawn, and a slow watchdog against a listener that accepts but never
///     answers.

import Foundation
import Darwin

public enum SidecarSocketProbe {
    /// Blocking `connect(2)` with a bounded wait. A connected-side socket that
    /// never accepts cannot stall the caller: the connect is non-blocking and the
    /// wait is capped by `timeout`.
    public static func connectOnce(path: String, timeout: TimeInterval) -> Bool {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        defer { Darwin.close(fd) }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        let copied = path.withCString { cstr -> Bool in
            withUnsafeMutableBytes(of: &addr.sun_path) { raw in
                guard let base = raw.baseAddress else { return false }
                let limit = min(capacity, raw.count)
                memset(base, 0, limit)
                guard strlen(cstr) < limit else { return false }
                strncpy(base.assumingMemoryBound(to: CChar.self), cstr, limit - 1)
                return true
            }
        }
        guard copied else { return false }

        let flags = fcntl(fd, F_GETFL, 0)
        _ = fcntl(fd, F_SETFL, flags | O_NONBLOCK)
        let rc = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sap in
                Darwin.connect(fd, sap, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        if rc == 0 { return true }
        guard errno == EINPROGRESS else { return false }

        var pfd = pollfd(fd: fd, events: Int16(POLLOUT), revents: 0)
        guard poll(&pfd, 1, Int32(max(1, timeout * 1000))) > 0 else { return false }
        var err: Int32 = 0
        var len = socklen_t(MemoryLayout<Int32>.size)
        guard getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &len) == 0 else { return false }
        return err == 0
    }

    /// `connectOnce` off the main thread.
    public static func reachable(path: String, timeout: TimeInterval) async -> Bool {
        await withCheckedContinuation { cont in
            DispatchQueue.global(qos: .utility).async {
                cont.resume(returning: connectOnce(path: path, timeout: timeout))
            }
        }
    }

    /// Bounded `health.status`. Returns false on error *and* on timeout, so a
    /// wedged sidecar that accepts connections but never replies is reported
    /// unavailable rather than hanging the supervision loop forever.
    ///
    /// The losing branch is abandoned; the blocked socket read is released when
    /// the wedged sidecar is terminated (or when the peer closes).
    public static func healthProbe(client: IPCClient, timeout: TimeInterval) async -> Bool {
        let gate = ResumeGate<Bool>()
        return await withCheckedContinuation { (cont: CheckedContinuation<Bool, Never>) in
            gate.install(cont)
            Task {
                do {
                    _ = try await client.health()
                    gate.resume(true)
                } catch {
                    gate.resume(false)
                }
            }
            DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout) {
                gate.resume(false)
            }
        }
    }
}

/// Resumes a continuation exactly once, whichever of two racing paths wins. The
/// loser is dropped, so a slow sidecar cannot double-resume a continuation and
/// crash the process.
final class ResumeGate<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var cont: CheckedContinuation<T, Never>?
    private var done = false

    func install(_ continuation: CheckedContinuation<T, Never>) {
        lock.lock()
        cont = continuation
        lock.unlock()
    }

    func resume(_ value: T) {
        lock.lock()
        if done {
            lock.unlock()
            return
        }
        done = true
        let continuation = cont
        cont = nil
        lock.unlock()
        continuation?.resume(returning: value)
    }
}
