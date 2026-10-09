// P2-9 — Process tree Canvas DAG
// Additive file. Renders ProcessSummary rows as a Canvas-based DAG
// with parent → child edges, node chips (family / RSS / pid), and
// tap-to-select. Built on top of existing IPC types — no new RPCs.

import SwiftUI
import ShareCLICore

struct ProcessesTreeCanvasView: View {
    let state: AppState
    let onSelect: (ProcessSummary) -> Void

    var body: some View {
        Canvas { ctx, size in
            let layout = layoutNodes(in: size)
            drawEdges(ctx: ctx, layout: layout)
            drawNodes(ctx: ctx, layout: layout)
        }
        .background(Color(nsColor: .windowBackgroundColor).opacity(0.6))
    }

    // MARK: - Layout

    private struct PositionedNode {
        let process: ProcessSummary
        let center: CGPoint
        let size: CGSize
        let depth: Int
    }

    private func layoutNodes(in size: CGSize) -> [PositionedNode] {
        let processes = state.processes
        guard !processes.isEmpty else { return [] }

        // 1. Group by parent (ppid).
        var childrenOf: [UInt32: [ProcessSummary]] = [:]
        var roots: [ProcessSummary] = []
        let byPid: [UInt32: ProcessSummary] = Dictionary(
            uniqueKeysWithValues: processes.map { ($0.pid, $0) }
        )
        for p in processes {
            if let parent = byPid[p.ppidValue], parent.pid != p.pid {
                childrenOf[parent.pid, default: []].append(p)
            } else {
                roots.append(p)
            }
        }
        for k in childrenOf.keys { childrenOf[k]?.sort { $0.memory_mb > $1.memory_mb } }
        roots.sort { $0.memory_mb > $1.memory_mb }

        // 2. Lay out by depth, BFS-ish.
        let nodeSize = CGSize(width: 168, height: 64)
        let hGap: CGFloat = 28
        let vGap: CGFloat = 16
        let topMargin: CGFloat = 24
        let leftMargin: CGFloat = 16

        var positioned: [PositionedNode] = []
        var depthCounters: [Int: Int] = [:]

        func place(_ p: ProcessSummary, depth: Int) {
            let row = depthCounters[depth, default: 0]
            depthCounters[depth] = row + 1
            let maxDepth = max(depthCounters.values.max() ?? 1, 1)
            let totalRowsAtDepth = depthCounters[depth] ?? 1
            let verticalSpan = CGFloat(totalRowsAtDepth) * (nodeSize.height + vGap)
            let y = topMargin + (CGFloat(row) * (nodeSize.height + vGap)) + nodeSize.height / 2
            // X: column = depth, evenly spaced.
            let columnsTotal = max(maxDepth, roots.count, 1)
            let xSpan = max(size.width - leftMargin * 2 - nodeSize.width, CGFloat(columnsTotal) * (nodeSize.width + hGap))
            let x = leftMargin + nodeSize.width / 2 + (xSpan - nodeSize.width) * (CGFloat(depth) / CGFloat(max(columnsTotal - 1, 1)))
            _ = verticalSpan
            let center = CGPoint(x: x, y: y)
            positioned.append(PositionedNode(process: p, center: center, size: nodeSize, depth: depth))
            for child in (childrenOf[p.pid] ?? []) {
                place(child, depth: depth + 1)
            }
        }

        for r in roots { place(r, depth: 0) }
        return positioned
    }

    // MARK: - Drawing

    private func drawEdges(ctx: GraphicsContext, layout: [PositionedNode]) {
        let byPid: [UInt32: PositionedNode] = Dictionary(
            uniqueKeysWithValues: layout.map { ($0.process.pid, $0) }
        )
        for node in layout {
            guard let parent = byPid[node.process.ppidValue],
                  parent.process.pid != node.process.pid else { continue }
            var path = Path()
            let start = CGPoint(x: parent.center.x + parent.size.width / 2,
                                y: parent.center.y + parent.size.height / 2)
            let end = CGPoint(x: node.center.x - node.size.width / 2,
                              y: node.center.y)
            let midX = (start.x + end.x) / 2
            path.move(to: start)
            path.addCurve(to: end,
                          control1: CGPoint(x: midX, y: start.y),
                          control2: CGPoint(x: midX, y: end.y))
            ctx.stroke(path, with: .color(.secondary.opacity(0.5)), lineWidth: 1.2)
        }
    }

    private func drawNodes(ctx: GraphicsContext, layout: [PositionedNode]) {
        for node in layout {
            let rect = CGRect(
                x: node.center.x - node.size.width / 2,
                y: node.center.y - node.size.height / 2,
                width: node.size.width,
                height: node.size.height
            )
            let rrect = RoundedRectangle(cornerRadius: 8).path(in: rect)
            // Heat color: green → yellow → orange → red by RSS.
            let mb = Double(node.process.memory_mb)
            let hue = max(0, min(1, 1 - mb / 4096))
            let fill = Color(hue: 0.33 * hue, saturation: 0.6, brightness: 0.95)
            ctx.fill(rrect, with: .color(fill.opacity(0.18)))
            ctx.stroke(rrect, with: .color(fill.opacity(0.7)), lineWidth: 1.0)

            // Top label: family / pid
            let family = node.process.harness ?? node.process.project ?? "?"
            let pidStr = "\(node.process.pid)"
            let title = Text("\(family)  ·  \(pidStr)")
                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                .foregroundColor(.primary)
            ctx.draw(title, in: CGRect(x: rect.minX + 8, y: rect.minY + 6,
                                       width: rect.width - 16, height: 14))

            // Middle: name (truncated)
            let name = node.process.name
            let truncated = name.count > 24 ? String(name.prefix(22)) + "…" : name
            let nameTxt = Text(truncated)
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            ctx.draw(nameTxt, in: CGRect(x: rect.minX + 8, y: rect.minY + 22,
                                         width: rect.width - 16, height: 14))

            // Bottom: RSS
            let rss = Text(formatMB(mb))
                .font(.system(size: 10, design: .monospaced))
                .foregroundColor(.secondary)
            ctx.draw(rss, in: CGRect(x: rect.minX + 8, y: rect.minY + 40,
                                    width: rect.width - 16, height: 12))
        }
    }

    /// Accessibility label for a DAG node. Used by the tap overlay and tests
    /// so every node is reachable by VoiceOver.
    static func accessibilityLabel(for p: ProcessSummary) -> String {
        let family = p.harness ?? p.project ?? "process"
        return "\(p.name) · pid \(p.pid) · \(family)"
    }

    private func formatMB(_ mb: Double) -> String {
        if mb >= 1024 { return String(format: "%.1f GB", mb / 1024) }
        return String(format: "%.0f MB", mb)
    }
}

// MARK: - Tap-to-select overlay

struct ProcessesTreeCanvasTapOverlay: View {
    let state: AppState
    let onSelect: (ProcessSummary) -> Void

    var body: some View {
        GeometryReader { geo in
            let layout = computeLayout(in: geo.size, state: state)
            ZStack(alignment: .topLeading) {
                ForEach(layout, id: \.process.pid) { node in
                    Button(action: { onSelect(node.process) }) {
                        Color.clear
                    }
                    .buttonStyle(.plain)
                    .frame(width: node.size.width, height: node.size.height)
                    .position(x: node.center.x, y: node.center.y)
                    .help("\(node.process.name) — pid \(node.process.pid)")
                    .accessibilityElement()
                    .accessibilityLabel(
                        ProcessesTreeCanvasView.accessibilityLabel(for: node.process)
                    )
                }
            }
        }
    }

    private struct PositionedNode {
        let process: ProcessSummary
        let center: CGPoint
        let size: CGSize
    }

    private func computeLayout(in size: CGSize, state: AppState) -> [PositionedNode] {
        // Mirror ProcessesTreeCanvasView.layoutNodes for tap hit-testing.
        let processes = state.processes
        guard !processes.isEmpty else { return [] }
        var childrenOf: [UInt32: [ProcessSummary]] = [:]
        var roots: [ProcessSummary] = []
        let byPid: [UInt32: ProcessSummary] = Dictionary(
            uniqueKeysWithValues: processes.map { ($0.pid, $0) }
        )
        for p in processes {
            if let parent = byPid[p.ppidValue], parent.pid != p.pid {
                childrenOf[parent.pid, default: []].append(p)
            } else {
                roots.append(p)
            }
        }
        for k in childrenOf.keys { childrenOf[k]?.sort { $0.memory_mb > $1.memory_mb } }
        roots.sort { $0.memory_mb > $1.memory_mb }

        let nodeSize = CGSize(width: 168, height: 64)
        let hGap: CGFloat = 28
        let vGap: CGFloat = 16
        let topMargin: CGFloat = 24
        let leftMargin: CGFloat = 16

        var positioned: [PositionedNode] = []
        var depthCounters: [Int: Int] = [:]

        func place(_ p: ProcessSummary, depth: Int) {
            let row = depthCounters[depth, default: 0]
            depthCounters[depth] = row + 1
            let maxDepth = max(depthCounters.values.max() ?? 1, 1)
            let totalRowsAtDepth = depthCounters[depth] ?? 1
            _ = totalRowsAtDepth
            let y = topMargin + (CGFloat(row) * (nodeSize.height + vGap)) + nodeSize.height / 2
            let columnsTotal = max(maxDepth, roots.count, 1)
            let xSpan = max(size.width - leftMargin * 2 - nodeSize.width, CGFloat(columnsTotal) * (nodeSize.width + hGap))
            let x = leftMargin + nodeSize.width / 2 + (xSpan - nodeSize.width) * (CGFloat(depth) / CGFloat(max(columnsTotal - 1, 1)))
            let center = CGPoint(x: x, y: y)
            positioned.append(PositionedNode(process: p, center: center, size: nodeSize))
            for child in (childrenOf[p.pid] ?? []) {
                place(child, depth: depth + 1)
            }
        }

        for r in roots { place(r, depth: 0) }
        return positioned
    }
}

// MARK: - Composite (canvas + labeled overlay + accessibility container)

/// Combines the visual Canvas DAG with the labeled Button overlay and wraps
/// the pair in `accessibilityElement(children: .contain)` so VoiceOver can
/// enter the graph and navigate individual nodes.
struct ProcessesTreeCanvasCompositeView: View {
    let state: AppState
    let onSelect: (ProcessSummary) -> Void

    var body: some View {
        ZStack {
            ProcessesTreeCanvasView(state: state, onSelect: onSelect)
                .accessibilityHidden(true)
            ProcessesTreeCanvasTapOverlay(state: state, onSelect: onSelect)
        }
        .accessibilityElement(children: .contain)
    }
}
