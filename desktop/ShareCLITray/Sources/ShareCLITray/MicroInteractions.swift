/// MicroInteractions.swift — small reusable view modifiers for dashboard polish.
///
/// Provides three building blocks that the dashboard pages compose on top of
/// their existing layouts:
///
///  1. `.animateInOnAppear(delay:)` — scale+opacity entrance animation
///     for summary cards, list rows, and panel headers. Designed to feel
///     like the page "settles" rather than pop in.
///
///  2. `PressableButtonStyle` — a button style that scales the content to
///     0.96 with a 0.08s spring on press and back to 1.0 on release.
///     Drop-in replacement for `.borderless` / `.plain` button styles
///     used on action buttons across the dashboard.
///
///  3. `.hoverGlow(radius:)` — a subtle accent-colored border + drop shadow
///     that fades in when the cursor hovers over a card. Pairs naturally
///     with the existing `.background(.quaternary)` cards.
///
///  All three are wired up so that SwiftUI's `Label`s + `Image`s + `Text`s
///  render unchanged when the modifiers are not applied — they only add
///  visual signal on interaction.
import SwiftUI

// MARK: - Reduce-motion decision (PLAN.md:233-234, task 1.23)

/// The single decision point every motion site in the tray routes through.
///
/// Accessibility contract: when the operator has "Reduce motion" enabled,
/// motion (transitions, springs, scales, hover fades) is suppressed — but the
/// underlying *state change* still happens, instantly. Colours and non-motion
/// state are unaffected, matching the existing style where colour is signal,
/// not motion.
///
/// Kept free of SwiftUI state so the decision is unit-testable headlessly.
enum Motion {
    /// Whether motion should run for a given reduce-motion setting.
    static func isEnabled(reduceMotion: Bool) -> Bool { !reduceMotion }

    /// The animation to attach when motion is allowed; `nil` when reduced
    /// (SwiftUI treats `nil` as "no animation").
    static func animation(_ animation: Animation?, reduceMotion: Bool) -> Animation? {
        isEnabled(reduceMotion: reduceMotion) ? animation : nil
    }

    /// Applies a state change with motion when allowed, instantly when not.
    static func run(_ animation: Animation?, reduceMotion: Bool, _ change: () -> Void) {
        if let animation, isEnabled(reduceMotion: reduceMotion) {
            withAnimation(animation, change)
        } else {
            change()
        }
    }

    /// Environment convenience: the caller reads
    /// `@Environment(\.accessibilityReduceMotion)` and hands the value here.
    static func animation(_ animation: Animation?, environment: Bool) -> Animation? {
        self.animation(animation, reduceMotion: environment)
    }

    /// `.onChange`-style numeric tween gate: returns the target value.
    static func tweenValue(_ value: Double, reduceMotion: Bool) -> Double { value }
}

// MARK: - Entrance animation

private struct AnimateInOnAppearModifier: ViewModifier {
    let delay: Double
    @State private var visible: Bool = false

    /// Reduce motion: skip the entrance movement, land at the final state.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content
            .opacity(visible ? 1.0 : (Motion.isEnabled(reduceMotion: reduceMotion) ? 0.0 : 1.0))
            .scaleEffect(Motion.isEnabled(reduceMotion: reduceMotion) ? (visible ? 1.0 : 0.97) : 1.0)
            .onAppear {
                Motion.run(.spring(response: 0.42, dampingFraction: 0.78).delay(delay),
                           reduceMotion: reduceMotion) {
                    visible = true
                }
            }
    }
}

extension View {
    /// Fades + scales the view in when it first appears. Stagger with `delay`
    /// to cascade summary cards, list rows, and panel headers.
    func animateInOnAppear(delay: Double = 0) -> some View {
        modifier(AnimateInOnAppearModifier(delay: delay))
    }
}

// MARK: - Press feedback

/// A button style that scales content to 0.96 on press and back to 1.0
/// on release with a quick spring. Pairs with `.borderless` look — no
/// default chrome change, just tactile feedback.
struct PressableButtonStyle: ButtonStyle {
    /// Reduce motion: keep the colour feedback, drop the scale movement.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(Motion.isEnabled(reduceMotion: reduceMotion) && configuration.isPressed ? 0.96 : 1.0)
            .opacity(configuration.isPressed ? 0.85 : 1.0)
            .animation(Motion.animation(.spring(response: 0.18, dampingFraction: 0.7),
                                        reduceMotion: reduceMotion),
                       value: configuration.isPressed)
    }
}

extension ButtonStyle where Self == PressableButtonStyle {
    static var pressable: PressableButtonStyle { PressableButtonStyle() }
}

// MARK: - Hover glow

private struct HoverGlowModifier: ViewModifier {
    let radius: CGFloat
    @State private var hovering: Bool = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(Color.accentColor.opacity(hovering ? 0.5 : 0.0), lineWidth: 1.5)
                    .shadow(color: Color.accentColor.opacity(hovering ? 0.18 : 0.0), radius: radius)
                    .animation(Motion.animation(.easeInOut(duration: 0.18), reduceMotion: reduceMotion),
                               value: hovering)
                    .allowsHitTesting(false)
            )
            .onHover { hovering = $0 }
    }
}

extension View {
    /// Accent-colored glow on hover. Best on cards that already use
    /// `.background(.quaternary).clipShape(RoundedRectangle(cornerRadius: 8))`.
    func hoverGlow(radius: CGFloat = 6) -> some View {
        modifier(HoverGlowModifier(radius: radius))
    }
}

// MARK: - Smooth value-transition for numeric data

private struct AnimatedNumberModifier: ViewModifier {
    let value: Double
    let formatter: (Double) -> String
    @State private var displayed: Double = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        Text(formatter(displayed))
            .contentTransition(Motion.isEnabled(reduceMotion: reduceMotion) ? .numericText() : .identity)
            .onAppear { displayed = value }
            .onChange(of: value) { _, newValue in
                Motion.run(.easeOut(duration: 0.32), reduceMotion: reduceMotion) {
                    displayed = newValue
                }
            }
    }
}

extension View {
    /// Animates a numeric value through a custom formatter. Use for
    /// counters in summary cards (e.g. "12" → "13" with a small tween).
    func animatedNumber(_ value: Double, formatter: @escaping (Double) -> String) -> some View {
        modifier(AnimatedNumberModifier(value: value, formatter: formatter))
    }
}
