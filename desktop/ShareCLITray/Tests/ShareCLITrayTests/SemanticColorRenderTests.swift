//! FR: Real-pixel WCAG verification for Color.warningForeground and
//! FR: Color.successForeground. Uses SwiftUI's `ImageRenderer` to render
//! FR: each color token at the requested appearance, samples the actual
//! FR: pixel, and computes the contrast ratio against the background the
//! FR: token is meant to be drawn on. This is real-pixel evidence, not
//! FR: a hand calculation.

import XCTest
import SwiftUI
import AppKit
@testable import ShareCLITray

@MainActor
final class SemanticColorRenderTests: XCTestCase {

    // MARK: - Pixel sampling

    /// Render a Solid-color SwiftUI view via ImageRenderer, return the
    /// RGB of the center pixel. Falls back to (0,0,0) on failure
    /// (which would then fail the contrast assertions).
    private func renderAndSample(
        _ color: Color,
        size: CGSize = CGSize(width: 16, height: 16),
        background: Color = .white
    ) -> (red: Double, green: Double, blue: Double)? {
        let host = NSHostingController(
            rootView:
                ZStack {
                    background
                    color
                }
                .frame(width: size.width, height: size.height)
        )
        host.view.frame = CGRect(origin: .zero, size: size)
        let renderer = ImageRenderer(content: host.rootView)
        renderer.scale = 1.0
        renderer.proposedSize = ProposedViewSize(size)
        guard let cg = renderer.cgImage else { return nil }
        // Sample a center-ish pixel (offset 2 from top-left to avoid
        // any border drawing artifacts).
        let bytesPerPixel = 4
        let bytesPerRow = bytesPerPixel * Int(size.width)
        let x = 2
        let y = 2
        let offset = y * bytesPerRow + x * bytesPerPixel
        // CFData is a CoreFoundation type — pull bytes into a Swift
        // array first.
        let length = CFDataGetLength(cg.dataProvider!.data!)
        let rawPtr = CFDataGetBytePtr(cg.dataProvider!.data!)!
        let data = Array(UnsafeBufferPointer(start: rawPtr, count: length))
        var pixel: [UInt8] = []
        for i in 0..<bytesPerPixel {
            pixel.append(data[offset + i])
        }
        return (
            Double(pixel[0]) / 255.0,
            Double(pixel[1]) / 255.0,
            Double(pixel[2]) / 255.0
        )
    }

    // MARK: - WCAG 2.1 relative luminance

    /// Convert sRGB 0-1 to linear, per WCAG 2.x.
    private func linear(_ c: Double) -> Double {
        c <= 0.03928 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4)
    }

    /// WCAG 2.1 relative luminance.
    private func luminance(r: Double, g: Double, b: Double) -> Double {
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }

    /// WCAG 2.1 contrast ratio between two sRGB colors.
    private func contrast(r1: Double, g1: Double, b1: Double,
                          r2: Double, g2: Double, b2: Double) -> Double {
        let l1 = luminance(r: r1, g: g1, b: b1)
        let l2 = luminance(r: r2, g: g2, b: b2)
        let (light, dark) = l1 > l2 ? (l1, l2) : (l2, l1)
        return (light + 0.05) / (dark + 0.05)
    }

    // MARK: - Setup / teardown

    /// Drive the static `warningForeground` / `successForeground`
    /// accessors through a specific branch via the test seam. We
    /// cannot rely on `NSApp.appearance` because `ImageRenderer` does
    /// not honor a runtime appearance change inside an XCTest
    /// runner.
    private func withAppearance(_ isDark: Bool, _ body: () throws -> Void) rethrows {
        let previous = Color.isDarkModeOverride
        Color.isDarkModeOverride = isDark
        defer { Color.isDarkModeOverride = previous }
        try body()
    }

    // MARK: - Tests

    /// Real-pixel test: with the test seam pinned to light, the
    /// static token resolves to dark amber (#994C00) and must
    /// achieve ≥ 4.5:1 contrast against a white background —
    /// matching the claim in `SemanticColors.swift:49` ("~5.0:1 on
    /// white").
    func testWarningForegroundLightAppearanceOnWhiteMeetsAA() throws {
        try withAppearance(false) {
            guard let rgb = renderAndSample(
                Color.warningForeground,
                background: .white
            ) else {
                XCTFail("ImageRenderer returned nil CGImage; cannot sample warningForeground in light appearance")
                return
            }
            let ratio = contrast(r1: rgb.red, g1: rgb.green, b1: rgb.blue,
                                  r2: 1.0, g2: 1.0, b2: 1.0)
            XCTAssertGreaterThanOrEqual(
                ratio, 4.5,
                "Color.warningForeground (light) on white must meet AA (≥ 4.5:1); rendered RGB=(\(rgb.red),\(rgb.green),\(rgb.blue)), ratio=\(ratio):1"
            )
            // Sanity: should be close to dark amber (0.60, 0.30, 0.0).
            XCTAssertEqual(rgb.red, 0.60, accuracy: 0.01, "light warningForeground red channel should be 0.60")
            XCTAssertEqual(rgb.green, 0.30, accuracy: 0.01, "light warningForeground green channel should be 0.30")
            XCTAssertEqual(rgb.blue, 0.0, accuracy: 0.01, "light warningForeground blue channel should be 0.0")
        }
    }

    /// Real-pixel test: with the test seam pinned to dark, the token
    /// resolves to SwiftUI `.orange` and must achieve ≥ 4.5:1
    /// contrast against a near-black background — the surface the
    /// tray/dashboard uses in dark mode.
    func testWarningForegroundDarkAppearanceOnBlackMeetsAA() throws {
        try withAppearance(true) {
            guard let rgb = renderAndSample(
                Color.warningForeground,
                background: Color(red: 0.10, green: 0.10, blue: 0.10)
            ) else {
                XCTFail("ImageRenderer returned nil CGImage; cannot sample warningForeground in dark appearance")
                return
            }
            let bg = (r: 0.10, g: 0.10, b: 0.10)
            let ratio = contrast(r1: rgb.red, g1: rgb.green, b1: rgb.blue,
                                  r2: bg.r, g2: bg.g, b2: bg.b)
            XCTAssertGreaterThanOrEqual(
                ratio, 4.5,
                "Color.warningForeground (dark) on near-black must meet AA (≥ 4.5:1); rendered RGB=(\(rgb.red),\(rgb.green),\(rgb.blue)), ratio=\(ratio):1"
            )
        }
    }

    /// Real-pixel test: with the test seam pinned to light,
    /// `Color.successForeground` resolves to static dark green
    /// (#1A6B1A) and must achieve ≥ 4.5:1 on white.
    func testSuccessForegroundLightAppearanceOnWhiteMeetsAA() throws {
        try withAppearance(false) {
            guard let rgb = renderAndSample(
                Color.successForeground,
                background: .white
            ) else {
                XCTFail("ImageRenderer returned nil CGImage; cannot sample successForeground in light appearance")
                return
            }
            let ratio = contrast(r1: rgb.red, g1: rgb.green, b1: rgb.blue,
                                  r2: 1.0, g2: 1.0, b2: 1.0)
            XCTAssertGreaterThanOrEqual(
                ratio, 4.5,
                "Color.successForeground (light) on white must meet AA (≥ 4.5:1); rendered RGB=(\(rgb.red),\(rgb.green),\(rgb.blue)), ratio=\(ratio):1"
            )
            // Sanity: should be close to dark green (0.10, 0.42, 0.10).
            XCTAssertEqual(rgb.red, 0.10, accuracy: 0.01, "light successForeground red channel should be 0.10")
            XCTAssertEqual(rgb.green, 0.42, accuracy: 0.01, "light successForeground green channel should be 0.42")
            XCTAssertEqual(rgb.blue, 0.10, accuracy: 0.01, "light successForeground blue channel should be 0.10")
        }
    }

    /// Real-pixel test: with the test seam pinned to dark, the
    /// token resolves to SwiftUI `.green` and must achieve ≥ 4.5:1
    /// on near-black.
    func testSuccessForegroundDarkAppearanceOnBlackMeetsAA() throws {
        try withAppearance(true) {
            guard let rgb = renderAndSample(
                Color.successForeground,
                background: Color(red: 0.10, green: 0.10, blue: 0.10)
            ) else {
                XCTFail("ImageRenderer returned nil CGImage; cannot sample successForeground in dark appearance")
                return
            }
            let bg = (r: 0.10, g: 0.10, b: 0.10)
            let ratio = contrast(r1: rgb.red, g1: rgb.green, b1: rgb.blue,
                                  r2: bg.r, g2: bg.g, b2: bg.b)
            XCTAssertGreaterThanOrEqual(
                ratio, 4.5,
                "Color.successForeground (dark) on near-black must meet AA (≥ 4.5:1); rendered RGB=(\(rgb.red),\(rgb.green),\(rgb.blue)), ratio=\(ratio):1"
            )
        }
    }

    /// Sanity test: warning (light) must render to amber, not
    /// accidentally black/white.
    func testWarningForegroundLightRendersToNonDefaultColor() throws {
        try withAppearance(false) {
            guard let rgb = renderAndSample(
                Color.warningForeground,
                background: .white
            ) else {
                XCTFail("ImageRenderer returned nil CGImage")
                return
            }
            let isWhite = rgb.red > 0.95 && rgb.green > 0.95 && rgb.blue > 0.95
            XCTAssertFalse(
                isWhite,
                "Color.warningForeground (light) rendered to near-white RGB=(\(rgb.red),\(rgb.green),\(rgb.blue)); the token is not visible"
            )
            let isBlack = rgb.red < 0.05 && rgb.green < 0.05 && rgb.blue < 0.05
            XCTAssertFalse(
                isBlack,
                "Color.warningForeground (light) rendered to near-black RGB=(\(rgb.red),\(rgb.green),\(rgb.blue)); expected amber"
            )
        }
    }

    /// Sanity test: success (light) must render to a green-ish hue.
    func testSuccessForegroundLightRendersToGreenishHue() throws {
        try withAppearance(false) {
            guard let rgb = renderAndSample(
                Color.successForeground,
                background: .white
            ) else {
                XCTFail("ImageRenderer returned nil CGImage")
                return
            }
            XCTAssertGreaterThan(
                rgb.green, rgb.red,
                "Color.successForeground (light) green channel (\(rgb.green)) must exceed red (\(rgb.red)); got RGB=(\(rgb.red),\(rgb.green),\(rgb.blue))"
            )
            XCTAssertGreaterThan(
                rgb.green, rgb.blue,
                "Color.successForeground (light) green channel (\(rgb.green)) must exceed blue (\(rgb.blue)); got RGB=(\(rgb.red),\(rgb.green),\(rgb.blue))"
            )
        }
    }

    /// Diagnostic: log all 4 (color, appearance) combinations so a
    /// future regression has the actual rendered pixel data attached
    /// to the test failure. Fails if the renderer returns nil.
    func testDiagnosticCapturesRenderedColors() throws {
        var samples: [(String, String, (Double, Double, Double)?)] = []
        try withAppearance(false) {
            let w = renderAndSample(Color.warningForeground, background: .white)
            samples.append(("warning light / white", "white", w))
            let s = renderAndSample(Color.successForeground, background: .white)
            samples.append(("success light / white", "white", s))
        }
        try withAppearance(true) {
            let w = renderAndSample(Color.warningForeground, background: Color(white: 0.10))
            samples.append(("warning dark / near-0", "near-0", w))
            let s = renderAndSample(Color.successForeground, background: Color(white: 0.10))
            samples.append(("success dark / near-0", "near-0", s))
        }
        let msg = "Pixel-rendered WCAG (4 combinations):\n" + samples.map { entry -> String in
            let (label, bgLabel, rgbOpt) = entry
            let rgb = rgbOpt.map { "(\($0.0),\($0.1),\($0.2))" } ?? "nil"
            let r: Double = {
                guard let rgb = rgbOpt else { return 0 }
                let bg: (Double, Double, Double) = bgLabel == "white" ? (1, 1, 1) : (0.1, 0.1, 0.1)
                let l1 = self.luminance(r: rgb.0, g: rgb.1, b: rgb.2)
                let l2 = self.luminance(r: bg.0, g: bg.1, b: bg.2)
                let (light, dark) = l1 > l2 ? (l1, l2) : (l2, l1)
                return (light + 0.05) / (dark + 0.05)
            }()
            return "  \(label.padding(toLength: 28, withPad: " ", startingAt: 0)) RGB=\(rgb) → ratio=\(String(format: "%.2f", r)):1"
        }.joined(separator: "\n")
        XCTAssertTrue(
            samples.allSatisfy { $0.2 != nil },
            "renderAndSample returned nil for at least one combination; rendered evidence unavailable\n\(msg)"
        )
        // Always log the rendered evidence so it is captured in CI
        // artifacts even on the success path.
        print("==[ WCAG pixel evidence ]==\n\(msg)")
    }
}
