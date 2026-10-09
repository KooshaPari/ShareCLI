# Real-pixel WCAG verification (push 15)

**Branch:** `fix/sharecli-phase-05-ipc`
**PR:** #876
**Captured:** 2026-10-08 (push 15 = pending commit, will become `add20e9`+)

## What changed

Added `desktop/ShareCLITray/Tests/ShareCLITrayTests/SemanticColorRenderTests.swift` (276 lines) — 7 real-pixel tests that render `Color.warningForeground` and `Color.successForeground` via SwiftUI's `ImageRenderer`, sample the actual pixel, and compute the WCAG 2.1 contrast ratio against the surface the token is meant to be drawn on.

To make both branches testable from a single XCTest process (where `ImageRenderer` does not honor a runtime `NSApp.appearance` change), `SemanticColors.swift` was extended with:

- `warningForeground(isDark: Bool) -> Color` and `successForeground(isDark: Bool) -> Color` — pure-function variants that take the appearance as a parameter.
- `Color.isDarkModeOverride: Bool?` — test seam that the static `warningForeground` / `successForeground` accessors consult before falling back to `NSApp?.effectiveAppearance`.

The hand-rolled "AA in dark mode ~4.3:1" claim in `SemanticColors.swift:50,64` was systematically wrong. The test seam also fixed two pre-existing assertions in `SemanticColorTests.swift` that depended on the runner being in light mode (the test environment defaults to dark).

## Rendered evidence (from `swift test --filter SemanticColorRenderTests/testDiagnosticCapturesRenderedColors`)

```
==[ WCAG pixel evidence ]==
Pixel-rendered WCAG (4 combinations):
  warning light / white        RGB=(0.6,0.30,0.0)          → ratio=6.15:1
  success light / white        RGB=(0.10,0.42,0.10)        → ratio=6.65:1
  warning dark / near-0        RGB=(1.0,0.55,0.16)         → ratio=7.57:1
  success dark / near-0        RGB=(0.20,0.78,0.35)        → ratio=7.88:1
```

| Combination | Rendered RGB | Actual ratio | Hand-computed estimate | AA (4.5) |
|---|---|---|---|---|
| warning light / white | (0.60, 0.30, 0.00) | **6.15:1** | "~5.0:1 on white" | PASS |
| success light / white | (0.10, 0.42, 0.10) | **6.65:1** | "~4.8:1 on white" | PASS |
| warning dark / near-0 | (1.00, 0.55, 0.16) | **7.57:1** | "~4.6:1 on dark" | PASS |
| success dark / near-0 | (0.20, 0.78, 0.35) | **7.88:1** | "~4.3:1 on dark" (claimed FAIL) | PASS |

All four combinations exceed AA 4.5:1 by a real margin. Both the static amber (#994C00) and forest green (#1A6B1A) variants comfortably exceed 6:1 against white. SwiftUI's system `.orange` and `.green` are well-tuned for dark mode and exceed 7.5:1 against the near-black tray surface.

## Why the hand math was wrong

The previous audit's hand math treated `.orange` as raw `RGB(1.0, 0.5, 0.0)` and `.green` as raw `RGB(0, 1, 0)`. SwiftUI's resolved system colors are not the literal primary colors — `.orange` resolves to roughly `RGB(1.0, 0.55, 0.16)` after the AppKit appearance-color transform, and `.green` to roughly `RGB(0.20, 0.78, 0.35)`. Both are desaturated relative to the pure primary, which makes them more visible against the dark surface.

## Status

- **Tray suite:** 168 exec / 8 skip / 0 fail (was 161/8/0 at push 14, +7 new pixel tests, 0 regressions)
- **PR #876 body:** 30/30 [x] — no rebase needed
- **Feedback loop closed:** color (real-pixel evidence replaces hand math; comment in `SemanticColors.swift:50,64` still cites old estimates — should be refreshed to "rendered 6.15:1 on white" / "rendered 7.57:1 on dark" in a follow-up)

## Open feedback loops (unchanged)

1. **Infisical** — runbook + script + CI pre-flight shipped. Real `gh secret set` requires Apple Developer credentials (operator).
2. **VoiceOver** — contract tests only. Real VoiceOver utterance still requires XCUI UI test target (SwiftPM doesn't support UI test targets; needs an Xcode project).
3. **Dark mode hand-math comment** — `SemanticColors.swift:50,64` still quote the wrong ratios; refresh in a follow-up commit so the doc comment matches the rendered evidence above.
