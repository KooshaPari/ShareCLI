/// main.swift — NSStatusItem tray entry point for ShareCLI Desktop.
///
/// Architecture:
///   NSStatusItem (menu bar icon)
///     └─ NSPopover  (click → popover with summary + quick actions)
///          └─ "Open Dashboard" button → NSWindow (full dashboard NSHostingView)

import AppKit
import SwiftUI
import ShareCLICore

@main
struct ShareCLITrayApp {
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var popover: NSPopover!
    private var dashboardWindow: NSWindow?
    private var eventMonitor: Any?

    @MainActor private let state = AppState.shared

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Hide from Dock — pure tray app
        NSApp.setActivationPolicy(.accessory)

        // Ensure IPC sidecar is running
        Task { @MainActor in
            await ensureIPC()
            state.startPolling()
        }

        setupStatusItem()
        setupPopover()
        // Right-click → NSMenu (left-click keeps the popover)
        TrayMenuController.installContextMenu(for: statusItem)
        MenuAction.shared.attachPopover(popover)
    }

    // MARK: - Status item

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        let btn = statusItem.button!
        btn.image = NSImage(systemSymbolName: "cpu", accessibilityDescription: "ShareCLI")
        btn.imagePosition = .imageLeading
        btn.action = #selector(togglePopover)
        btn.target = self

        // Keep title + icon updated from live monitoring.report gate visuals (AC-007.57, AC-007.62).
        NotificationCenter.default.addObserver(
            forName: .sharecliHealthChanged,
            object: nil,
            queue: .main
        ) { [weak self] note in
            guard let self else { return }
            let snap = note.object as? HealthSnapshot
            let visual: TrayGateVisual
            if let snap {
                visual = OperatorDisplay.resolveTrayGateVisual(gate: snap.gate, connected: true)
                self.statusItem.button?.title = OperatorDisplay.formatMenuBarTitleLine(
                    visual: visual,
                    health: snap
                )
            } else {
                visual = OperatorDisplay.resolveTrayGateVisual(
                    thermalPressure: "UNAVAILABLE",
                    gateDecision: "UNAVAILABLE",
                    connected: false
                )
                self.statusItem.button?.title = OperatorDisplay.formatMenuBarTitleOfflineLine(
                    visual: visual
                )
            }
            self.statusItem.button?.image = NSImage(
                systemSymbolName: visual.swiftSymbolName,
                accessibilityDescription: "ShareCLI \(visual.badgeLabel)"
            )
            switch visual.severity {
            case .normal: self.statusItem.button?.contentTintColor = .systemGreen
            case .warning, .offline: self.statusItem.button?.contentTintColor = .systemOrange
            case .critical: self.statusItem.button?.contentTintColor = .systemRed
            }
        }
    }

    // MARK: - Popover

    private func setupPopover() {
        popover = NSPopover()
        let maxHeight = min(480, Int((NSScreen.main?.visibleFrame.height ?? 480) * 0.8))
        popover.contentSize = NSSize(width: 360, height: maxHeight)
        popover.behavior = .applicationDefined
        popover.animates = true
        popover.contentViewController = NSHostingController(
            rootView: TrayPopoverView(state: state, onOpenDashboard: { [weak self] in
                self?.openDashboard()
            })
        )
    }

    @objc private func togglePopover() {
        guard let btn = statusItem.button else { return }
        if popover.isShown {
            closePopover()
        } else {
            popover.show(relativeTo: btn.bounds, of: btn, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
            startMonitoringClicksOutside()
        }
    }

    private func closePopover() {
        popover.performClose(nil)
        stopMonitoringClicksOutside()
    }

    /// With .applicationDefined behavior we must manually dismiss the popover
    /// when the user clicks anywhere outside it.
    private func startMonitoringClicksOutside() {
        stopMonitoringClicksOutside()
        eventMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            guard let self else { return }
            // If the click is outside the popover window, close it.
            if let popoverWindow = self.popover.contentViewController?.view.window,
               !popoverWindow.frame.contains(NSEvent.mouseLocation) {
                self.closePopover()
            }
        }
    }

    private func stopMonitoringClicksOutside() {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
            eventMonitor = nil
        }
    }

    // MARK: - Dashboard window

    private func openDashboard() {
        if let existing = dashboardWindow {
            existing.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 900, height: 620),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        win.title = "ShareCLI Dashboard"
        if let btn = statusItem.button {
            TrayWindowPositioner.place(window: win, below: btn)
        } else {
            win.center()
        }
        win.contentView = NSHostingView(rootView: DashboardView(state: state))
        win.isReleasedWhenClosed = false
        win.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        dashboardWindow = win
    }

    // MARK: - IPC lifecycle

    private func ensureIPC() async {
        // Try connecting; if that fails, attempt to start the sidecar.
        do {
            _ = try await IPCClient.defaultClient().health()
        } catch {
            // Sidecar not running — launch it.
            let exe = sidecarPath("sharecli-ipc")
            if let exe {
                let proc = Process()
                proc.executableURL = URL(fileURLWithPath: exe)
                try? proc.run()
                try? await Task.sleep(nanoseconds: 300_000_000)
            }
        }
    }

    private func sidecarPath(_ name: String) -> String? {
        let bundle = Bundle.main.bundlePath
        let bundleExe = "\(bundle)/Contents/Resources/bin/\(name)"
        if FileManager.default.fileExists(atPath: bundleExe) { return bundleExe }

        let output = Process()
        output.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        output.arguments = [name]
        let pipe = Pipe()
        output.standardOutput = pipe
        try? output.run()
        output.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
        return path?.isEmpty == false ? path : nil
    }
}

// Notification.Name.sharecliHealthChanged is declared in ShareCLICore/AppState.swift
// so both targets see the same symbol.
