// Bern SwiftUI runtime: style overrides. Generated build input; do not edit.
//
// Controls keep their platform look unless the application's `style!`
// says otherwise. The generated `BernStyles` describes each rule as a
// `BernStyle`: a look at rest, a look per state (hover, pressed, selected,
// disabled) and the springs that move between them. Colors are semantic
// roles mapped to the system's colors, or explicit pairs for light and
// dark. When the system asks for reduced motion, transforms stay still and
// colors change with a short fade.

import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

// MARK: - Pointer, keyboard and focus

/// A keyboard shortcut. tvOS has none: the generated code gives its
/// controls none there, and the Siri Remote's buttons stand for the keys
/// they have (`bernRemoteShortcut`).
#if os(tvOS)
typealias BernKeyboardShortcut = Never
#else
typealias BernKeyboardShortcut = KeyboardShortcut
#endif

/// Shortcuts in Bern's form, `cmd+shift+k`: those that follow state, read
/// as the application runs, how Apple's systems write them, and what a
/// key pressed for a `ShortcutInput` is. A text that is empty or no
/// shortcut is none.
@MainActor
enum BernShortcuts {
    private static let modifierOrder = ["cmd", "ctrl", "alt", "shift"]
    private static let named = [
        "space", "escape", "return", "tab", "delete", "up", "down", "left", "right", "home", "end",
        "pageup", "pagedown",
    ]

    /// The text as Bern writes it: modifiers in order, the key in lower
    /// case; "" for an empty text, nil for one that is no shortcut.
    static func normalized(_ text: String) -> String? {
        let parts = text.split(separator: "+", omittingEmptySubsequences: false)
            .map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
        if parts == [""] { return "" }
        guard let key = parts.last else { return nil }
        let modifiers = parts.dropLast()
        guard modifiers.allSatisfy(modifierOrder.contains), Set(modifiers).count == modifiers.count else { return nil }
        let printable = key.count == 1 && key.unicodeScalars.allSatisfy { $0.isASCII && $0.value > 32 && $0.value < 127 }
        guard printable || named.contains(key) else { return nil }
        return (modifierOrder.filter(modifiers.contains) + [key]).joined(separator: "+")
    }

    /// The modifiers and the key of a shortcut.
    private static func parts(_ text: String) -> (key: String, modifiers: [String])? {
        guard let normal = normalized(text), !normal.isEmpty else { return nil }
        var parts = normal.split(separator: "+").map(String.init)
        let key = parts.removeLast()
        return (key, parts)
    }

    #if !os(tvOS)
    /// The keyboard shortcut a text stands for.
    static func keyboard(_ text: String) -> KeyboardShortcut? {
        guard let (key, modifiers) = parts(text) else { return nil }
        let equivalent: KeyEquivalent
        switch key {
        case "space": equivalent = .space
        case "escape": equivalent = .escape
        case "return": equivalent = .return
        case "tab": equivalent = .tab
        case "delete": equivalent = .delete
        case "up": equivalent = .upArrow
        case "down": equivalent = .downArrow
        case "left": equivalent = .leftArrow
        case "right": equivalent = .rightArrow
        case "home": equivalent = .home
        case "end": equivalent = .end
        case "pageup": equivalent = .pageUp
        case "pagedown": equivalent = .pageDown
        default: equivalent = KeyEquivalent(Character(key))
        }
        var flags: EventModifiers = []
        if modifiers.contains("cmd") { flags.insert(.command) }
        if modifiers.contains("ctrl") { flags.insert(.control) }
        if modifiers.contains("alt") { flags.insert(.option) }
        if modifiers.contains("shift") { flags.insert(.shift) }
        return KeyboardShortcut(equivalent, modifiers: flags)
    }

    /// How a keyboard shortcut reads, as `display` writes its text.
    static func display(_ shortcut: KeyboardShortcut) -> String {
        var parts: [String] = []
        if shortcut.modifiers.contains(.command) { parts.append("cmd") }
        if shortcut.modifiers.contains(.control) { parts.append("ctrl") }
        if shortcut.modifiers.contains(.option) { parts.append("alt") }
        if shortcut.modifiers.contains(.shift) { parts.append("shift") }
        let keys: [(KeyEquivalent, String)] = [
            (.space, "space"), (.escape, "escape"), (.return, "return"), (.tab, "tab"), (.delete, "delete"),
            (.upArrow, "up"), (.downArrow, "down"), (.leftArrow, "left"), (.rightArrow, "right"),
            (.home, "home"), (.end, "end"), (.pageUp, "pageup"), (.pageDown, "pagedown"),
        ]
        let key = keys.first { $0.0 == shortcut.key }?.1 ?? String(shortcut.key.character).lowercased()
        return display((parts + [key]).joined(separator: "+"))
    }
    #endif

    #if os(tvOS)
    /// The Siri Remote's button for a text: Back for `escape`, Play/Pause
    /// for `space`, the clickpad's edges for the arrows; none for others.
    static func remote(_ text: String) -> BernRemoteKey? {
        switch normalized(text) {
        case "escape": return .back
        case "space": return .playPause
        case "left": return .left
        case "right": return .right
        case "up": return .up
        case "down": return .down
        default: return nil
        }
    }
    #endif

    /// How Apple's systems write a shortcut: ⌃⌥⇧⌘ and the key, letters in
    /// capitals and named keys as their symbols; "" for none.
    static func display(_ text: String) -> String {
        guard let (key, modifiers) = parts(text) else { return "" }
        var out = ""
        if modifiers.contains("ctrl") { out += "⌃" }
        if modifiers.contains("alt") { out += "⌥" }
        if modifiers.contains("shift") { out += "⇧" }
        if modifiers.contains("cmd") { out += "⌘" }
        let symbols = [
            "escape": "⎋", "return": "↩", "tab": "⇥", "delete": "⌫", "up": "↑", "down": "↓",
            "left": "←", "right": "→", "home": "↖", "end": "↘", "pageup": "⇞", "pagedown": "⇟",
        ]
        if key == "space" { return out + BernText.string("bern.shortcut.space") }
        return out + (symbols[key] ?? key.uppercased())
    }

    /// The modifiers held, in Bern's order, for a key pressed.
    static func modifiers(command: Bool, control: Bool, option: Bool, shift: Bool) -> [String] {
        var parts: [String] = []
        if command { parts.append("cmd") }
        if control { parts.append("ctrl") }
        if option { parts.append("alt") }
        if shift { parts.append("shift") }
        return parts
    }
}

extension View {
    /// Follows the pointer over the view. A television has no pointer:
    /// there the focus is what stands over a control
    /// (`bernTelevisionFocus`).
    @ViewBuilder func bernHover(_ action: @escaping (Bool) -> Void) -> some View {
        #if os(tvOS)
        self
        #else
        onHover(perform: action)
        #endif
    }

    /// Follows a press anywhere on the view without taking part in its
    /// gestures. On tvOS presses reach only buttons, which report their
    /// own.
    @ViewBuilder func bernPressWatch(_ action: @escaping (Bool) -> Void) -> some View {
        #if os(tvOS)
        self
        #else
        simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in action(true) }
                .onEnded { _ in action(false) })
        #endif
    }

    /// Content shown over the view from it: a popover, or on tvOS, which
    /// has none, a sheet over the screen, as tvOS shows such content.
    @ViewBuilder func bernPopover<Content: View>(
        isPresented: Binding<Bool>, @ViewBuilder content: @escaping () -> Content
    ) -> some View {
        modifier(BernPopoverPresentation(requested: isPresented, panel: content))
    }

    /// A toggle shown as a button that stays down while on: the platform's
    /// button toggle, or on tvOS, whose toggles are buttons already, its
    /// own toggle.
    @ViewBuilder func bernButtonToggle() -> some View {
        #if os(tvOS)
        self
        #else
        toggleStyle(.button)
        #endif
    }

    /// A list row without the separator below it; tvOS lists have none.
    @ViewBuilder func bernListRowSeparatorHidden() -> some View {
        #if os(tvOS)
        self
        #else
        listRowSeparator(.hidden)
        #endif
    }

    /// A list or form whose own background gives way to what is behind
    /// it; on tvOS lists and forms have none of their own.
    @ViewBuilder func bernScrollContentBackgroundHidden() -> some View {
        #if os(tvOS)
        self
        #else
        scrollContentBackground(.hidden)
        #endif
    }

    /// A keyboard shortcut on a hidden button that runs `action`.
    @ViewBuilder func bernHiddenShortcut(_ shortcut: BernKeyboardShortcut?, action: @escaping () -> Void) -> some View {
        #if os(tvOS)
        self
        #else
        background {
            if let shortcut {
                Button("", action: action)
                    .keyboardShortcut(shortcut)
                    .opacity(0)
                    .frame(width: 0, height: 0)
                    .accessibilityHidden(true)
            }
        }
        #endif
    }
}

/// Keep the old binding and the platform presentation in agreement when
/// a command in one popover opens another (quick settings → full settings).
private struct BernPopoverPresentation<Panel: View>: ViewModifier {
    @Binding var requested: Bool
    @ViewBuilder let panel: () -> Panel
    @Environment(BernWindowContext.self) private var window
    @State private var id = UUID()
    @State private var presented = false

    func body(content: Content) -> some View {
        presentation(content)
            .onChange(of: requested, initial: true) {
                if requested {
                    if window.popoverOwner != id { window.dismissPopover?() }
                    window.popoverOwner = id
                    window.dismissPopover = { presented = false; requested = false }
                    presented = true
                } else {
                    presented = false
                    release()
                }
            }
            .onChange(of: presented) {
                if !presented {
                    if requested { requested = false }
                    release()
                }
            }
            .onDisappear {
                presented = false
                if requested { requested = false }
                release()
            }
    }

    @ViewBuilder private func presentation(_ content: Content) -> some View {
        #if os(tvOS)
        content.sheet(isPresented: $presented, content: panel)
        #elseif os(macOS)
        content.popover(isPresented: $presented, arrowEdge: .bottom) {
            panel().onExitCommand {
                presented = false
                requested = false
                release()
            }
        }
            .background(BernPopoverOutsidePress(active: presented) {
                presented = false
                requested = false
                release()
            }.allowsHitTesting(false))
        #else
        content.popover(isPresented: $presented, arrowEdge: .bottom, content: panel)
        #endif
    }

    private func release() {
        if window.popoverOwner == id {
            window.popoverOwner = nil
            window.dismissPopover = nil
        }
    }
}

// MARK: - Motion

/// A spring from `style!`: undamped period and damping ratio.
struct BernMotion: Equatable, Sendable {
    let period: Double
    let damping: Double

    var animation: Animation {
        .spring(response: period * bernMotionScale, dampingFraction: damping)
    }
}

/// An application may request reduced motion without overriding the
/// user's accessibility setting. Descendants honor either preference.
private struct BernReduceMotionKey: EnvironmentKey { static let defaultValue = false }
extension EnvironmentValues {
    var bernReduceMotion: Bool {
        get { self[BernReduceMotionKey.self] }
        set { self[BernReduceMotionKey.self] = newValue }
    }
}
@propertyWrapper
struct BernReducedMotion: DynamicProperty {
    @Environment(\.accessibilityReduceMotion) private var system
    @Environment(\.bernReduceMotion) private var application
    var wrappedValue: Bool { system || application }
}

/// Whether motion is reduced: by the system, or by `BERN_MOTION=reduce`
/// for repeatable screenshots.
let bernForcedReduceMotion = ProcessInfo.processInfo.environment["BERN_MOTION"] == "reduce"

/// How much slower springs and staggering run: ten times with
/// `BERN_MOTION=slow`, so motion can be checked frame by frame.
let bernMotionScale: Double = ProcessInfo.processInfo.environment["BERN_MOTION"] == "slow" ? 10 : 1

// MARK: - Colors

/// A semantic color role or an explicit pair for light and dark.
enum BernColor: Equatable, Sendable {
    case role(String)
    case fixed(light: UInt32, dark: UInt32)

    var color: Color {
        switch self {
        case .role(let role):
            if let custom = MainActor.assumeIsolated({ BernThemeState.shared.palette[role] }) { return custom.color }
            return Self.system(role)
        case .fixed(let light, let dark):
            return Self.dynamic(light: light, dark: dark)
        }
    }

    /// Text and icons drawn on this color: the paired role, or black or
    /// white, whichever reads better.
    var onTop: Color {
        switch self {
        case .role("accent"):
            return Self.contrasting(MainActor.assumeIsolated { Self.accentColor })
        case .role("selection.background"):
            return Self.system("selection.foreground")
        case .role(let role) where role.hasPrefix("status."):
            return .white
        case .role:
            return .primary
        case .fixed(let light, let dark):
            return Self.dynamic(
                light: Self.luminance(light) > 0.4 ? 0x111114 : 0xFFFFFF,
                dark: Self.luminance(dark) > 0.4 ? 0x111114 : 0xFFFFFF)
        }
    }

    private static func rgb(_ value: UInt32) -> (Double, Double, Double) {
        (Double((value >> 16) & 0xFF) / 255, Double((value >> 8) & 0xFF) / 255, Double(value & 0xFF) / 255)
    }

    /// Relative luminance of an sRGB color.
    private static func luminance(_ value: UInt32) -> Double {
        let (red, green, blue) = rgb(value)
        func linear(_ channel: Double) -> Double {
            channel <= 0.04045 ? channel / 12.92 : pow((channel + 0.055) / 1.055, 2.4)
        }
        return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    }

    private static func dynamic(light: UInt32, dark: UInt32) -> Color {
        let (lr, lg, lb) = rgb(light)
        let (dr, dg, db) = rgb(dark)
        #if os(macOS)
        return Color(nsColor: NSColor(name: nil) { appearance in
            appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
                ? NSColor(srgbRed: dr, green: dg, blue: db, alpha: 1)
                : NSColor(srgbRed: lr, green: lg, blue: lb, alpha: 1)
        })
        #else
        return Color(uiColor: UIColor { traits in
            traits.userInterfaceStyle == .dark
                ? UIColor(red: dr, green: dg, blue: db, alpha: 1)
                : UIColor(red: lr, green: lg, blue: lb, alpha: 1)
        })
        #endif
    }

    /// Black or white, whichever contrasts more with `color` as the system
    /// resolves it now.
    private static func contrasting(_ color: Color) -> Color {
        #if os(macOS)
        let resolved = NSColor(color).usingColorSpace(.sRGB)
        let red = Double(resolved?.redComponent ?? 0)
        let green = Double(resolved?.greenComponent ?? 0)
        let blue = Double(resolved?.blueComponent ?? 0)
        #else
        var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
        UIColor(color).getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        #endif
        let value = (UInt32(red * 255) << 16) | (UInt32(green * 255) << 8) | UInt32(blue * 255)
        return luminance(value) > 0.4 ? Color(red: 0.067, green: 0.067, blue: 0.078) : .white
    }

    /// The accent: the one the application chose, or the system's.
    @MainActor
    static var accentColor: Color {
        BernThemeState.shared.accent?.color ?? .accentColor
    }

    private static func system(_ role: String) -> Color {
        switch role {
        case "accent", "focus.indicator": return MainActor.assumeIsolated { accentColor }
        case "on_accent": return contrasting(MainActor.assumeIsolated { accentColor })
        case "text.primary", "icon.primary": return .primary
        case "text.secondary", "icon.secondary": return .secondary
        case "status.success": return .green
        case "status.warning": return .orange
        case "status.error": return .red
        case "fill.subtle", "disabled.surface": return Color.primary.opacity(0.06)
        case "fill.strong", "control.track": return Color.primary.opacity(0.12)
        case "control.thumb": return .white
        default: break
        }
        #if os(macOS)
        switch role {
        case "text.placeholder": return Color(nsColor: .placeholderTextColor)
        case "surface.canvas": return Color(nsColor: .windowBackgroundColor)
        case "surface.panel": return Color(nsColor: .controlBackgroundColor)
        case "surface.raised": return Color(nsColor: .textBackgroundColor)
        case "control.border": return Color(nsColor: .separatorColor)
        case "selection.background": return Color(nsColor: .selectedContentBackgroundColor)
        case "selection.foreground": return Color(nsColor: .alternateSelectedControlTextColor)
        case "disabled.foreground": return Color(nsColor: .disabledControlTextColor)
        default: return .primary
        }
        #else
        switch role {
        case "text.placeholder": return Color(uiColor: .placeholderText)
        #if os(tvOS)
        // A television's surfaces are the screen and the system's
        // materials, which have no named colors.
        case "surface.canvas": return Color.clear
        case "surface.panel": return Color.primary.opacity(0.06)
        case "surface.raised": return Color.primary.opacity(0.1)
        #else
        case "surface.canvas": return Color(uiColor: .systemBackground)
        case "surface.panel": return Color(uiColor: .secondarySystemBackground)
        case "surface.raised": return Color(uiColor: .tertiarySystemBackground)
        #endif
        case "control.border": return Color(uiColor: .separator)
        case "selection.background": return .accentColor
        case "selection.foreground": return .white
        case "disabled.foreground": return Color(uiColor: .tertiaryLabel)
        default: return .primary
        }
        #endif
    }
}

// MARK: - Styles

/// How a control looks in one state. Unset fields keep the look below it.
struct BernLook: Sendable {
    var background: BernColor?
    var foreground: BernColor?
    var border: BernColor?
    var borderWidth: Double = 1
    /// A selected item filled with this color.
    var fill: BernColor?
    /// A selected item marked by a bar on one edge.
    var indicator: BernColor?
    var indicatorEdge: Edge = .leading
    var indicatorWidth: Double = 3
    var scale: CGSize?
    var offset: CGSize?
    var opacity: Double?
    /// A shadow below the control, as if lifted.
    var shadow: BernShadow?
    /// The spring entering this state.
    var motion: BernMotion?
    /// `none`: this state shows nothing of its own.
    var nothing = false

    /// `other` laid over this look.
    func covered(by other: BernLook) -> BernLook {
        if other.nothing {
            return self
        }
        var look = self
        look.background = other.background ?? background
        look.foreground = other.foreground ?? foreground
        if other.border != nil {
            look.border = other.border
            look.borderWidth = other.borderWidth
        }
        if other.fill != nil {
            look.fill = other.fill
        }
        if other.indicator != nil {
            look.indicator = other.indicator
            look.indicatorEdge = other.indicatorEdge
            look.indicatorWidth = other.indicatorWidth
        }
        look.scale = other.scale ?? scale
        look.offset = other.offset ?? offset
        look.opacity = other.opacity ?? opacity
        look.shadow = other.shadow ?? shadow
        look.motion = other.motion ?? motion
        return look
    }
}

/// A shadow of `radius` blur, `y` down, black at `opacity`.
struct BernShadow: Sendable {
    let radius: Double
    let y: Double
    let opacity: Double
}

/// A control's style overrides from `style!`.
struct BernStyle: Sendable {
    var base = BernLook()
    var corner: Double?
    var padding: Double?
    var size: ControlSize?
    var hover: BernLook?
    var pressed: BernLook?
    var selected: BernLook?
    var disabled: BernLook?
    /// The spring back to rest.
    var settle = BernMotion(period: 0.45, damping: 1)
    /// `focus: none`: no focus effect of the platform's own.
    var showsFocus = true

    /// What the control is under a television's focus: a row of a list,
    /// which only the pointer lights (a background on hover); a control
    /// drawn on a background of its own, as a key; or a picture.
    var focusKind: BernFocusKind {
        if base.background != nil { return .control }
        if hover?.background != nil { return .row }
        return .picture
    }

    /// Whether the style draws the control itself instead of keeping the
    /// platform's look, which only moves.
    var drawsLook: Bool {
        let looks = [base, hover, pressed, disabled].compactMap { $0 }
        return corner != nil || padding != nil
            || looks.contains { $0.background != nil || $0.foreground != nil || $0.border != nil }
    }

    /// The look for a combination of states and the spring reaching it:
    /// disabled covers pressed, which covers hover; selection lies below
    /// them.
    func look(selected isSelected: Bool, hover isHovered: Bool, pressed isPressed: Bool, enabled: Bool) -> (BernLook, BernMotion) {
        var look = base
        var motion: BernMotion?
        for (active, state) in [(isSelected, selected), (isHovered && enabled, hover), (isPressed && enabled, pressed), (!enabled, disabled)] {
            if active, let state {
                look = look.covered(by: state)
                motion = state.motion ?? motion
            }
        }
        return (look, motion ?? settle)
    }

    /// Default padding of a drawn control at its size.
    var insets: EdgeInsets {
        if let padding {
            return EdgeInsets(top: padding / 2, leading: padding, bottom: padding / 2, trailing: padding)
        }
        switch size {
        case .small: return EdgeInsets(top: 3, leading: 8, bottom: 3, trailing: 8)
        case .large: return EdgeInsets(top: 9, leading: 18, bottom: 9, trailing: 18)
        case .extraLarge: return EdgeInsets(top: 14, leading: 24, bottom: 14, trailing: 24)
        default: return EdgeInsets(top: 5, leading: 12, bottom: 5, trailing: 12)
        }
    }
}

/// A state as a number, so a change of state starts its spring.
private func bernStateKey(_ selected: Bool, _ hover: Bool, _ pressed: Bool, _ enabled: Bool) -> Int {
    (selected ? 1 : 0) | (hover ? 2 : 0) | (pressed ? 4 : 0) | (enabled ? 0 : 8)
}

extension View {
    /// The look's scale, offset and opacity, moved by `motion` when `key`
    /// changes; still and faded instead when motion is reduced.
    @ViewBuilder
    func bernTransform(_ look: BernLook, _ motion: BernMotion, key: Int, reduce: Bool) -> some View {
        let shadow = look.shadow ?? BernShadow(radius: 0, y: 0, opacity: 0)
        if reduce || bernForcedReduceMotion {
            self
                .shadow(color: .black.opacity(shadow.opacity), radius: shadow.radius, y: shadow.y)
                .animation(.easeInOut(duration: 0.15), value: key)
        } else {
            self
                .shadow(color: .black.opacity(shadow.opacity), radius: shadow.radius, y: shadow.y)
                .scaleEffect(look.scale ?? CGSize(width: 1, height: 1))
                .offset(look.offset ?? .zero)
                .opacity(look.opacity ?? 1)
                .animation(motion.animation, value: key)
        }
    }

    /// Hover and press feedback of a style around a control that keeps its
    /// platform look.
    func bernFeedback(_ style: BernStyle) -> some View {
        modifier(BernFeedback(style: style))
    }
}

/// Feedback for a control that keeps its platform look: the style's
/// transforms follow the pointer and presses.
private struct BernFeedback: ViewModifier {
    let style: BernStyle
    @Environment(\.isEnabled) private var enabled
    @BernReducedMotion private var reduce
    @State private var hover = false
    @State private var pressed = false

    func body(content: Content) -> some View {
        let (look, motion) = style.look(selected: false, hover: hover, pressed: pressed, enabled: enabled)
        content
            .bernTransform(look, motion, key: bernStateKey(false, hover, pressed, enabled), reduce: reduce)
            .bernHover { hover = $0 }
            .bernPressWatch { pressed = $0 }
    }
}

/// A button the style draws itself.
struct BernButtonStyle: ButtonStyle {
    let style: BernStyle

    func makeBody(configuration: Configuration) -> some View {
        BernStyledButton(configuration: configuration, style: style)
    }
}

private struct BernStyledButton: View {
    let configuration: ButtonStyle.Configuration
    let style: BernStyle
    @Environment(\.isEnabled) private var enabled
    @BernReducedMotion private var reduce
    @State private var hover = false

    var body: some View {
        let pressed = configuration.isPressed
        let (look, motion) = style.look(selected: false, hover: hover, pressed: pressed, enabled: enabled)
        let shape = RoundedRectangle(cornerRadius: style.corner ?? 6, style: .continuous)
        let foreground = look.foreground?.color ?? look.background?.onTop ?? Color.primary
        configuration.label
            .padding(style.insets)
            .foregroundStyle(foreground)
            .background(shape.fill(look.background?.color ?? Color.clear))
            .overlay(shape.strokeBorder(look.border?.color ?? Color.clear, lineWidth: look.borderWidth))
            .contentShape(shape)
            .bernTransform(look, motion, key: bernStateKey(false, hover, pressed, enabled), reduce: reduce)
            .bernHover { hover = $0 }
            .bernTelevisionFocus(.control, corner: style.corner ?? 6, shown: style.showsFocus)
    }
}

/// A button that shows its content: nothing of the platform's bezel, the
/// style's look and motion around it, or a dimmed press without a style.
struct BernContentButtonStyle: ButtonStyle {
    let style: BernStyle?

    func makeBody(configuration: Configuration) -> some View {
        BernContentButton(configuration: configuration, style: style)
    }
}

private struct BernContentButton: View {
    let configuration: ButtonStyle.Configuration
    let style: BernStyle?
    @Environment(\.isEnabled) private var enabled
    @BernReducedMotion private var reduce
    @State private var hover = false

    var body: some View {
        let pressed = configuration.isPressed
        if let style {
            let (look, motion) = style.look(selected: false, hover: hover, pressed: pressed, enabled: enabled)
            let shape = RoundedRectangle(cornerRadius: style.corner ?? 0, style: .continuous)
            // The corner clips the content, so a picture takes it too; the
            // shadow then follows the clipped shape.
            configuration.label
                .padding(style.padding ?? 0)
                .bernForeground(look.foreground?.color)
                .background(look.background?.color ?? Color.clear)
                .clipShape(shape)
                .overlay(shape.strokeBorder(look.border?.color ?? Color.clear, lineWidth: look.borderWidth))
                .contentShape(shape)
                .bernTransform(look, motion, key: bernStateKey(false, hover, pressed, enabled), reduce: reduce)
                .bernHover { hover = $0 }
                .bernTelevisionFocus(style.focusKind, corner: style.corner ?? 0, shown: style.showsFocus)
        } else {
            configuration.label
                .contentShape(Rectangle())
                .opacity(pressed ? 0.75 : enabled ? 1 : 0.5)
                .bernTelevisionFocus(.picture, corner: 0)
        }
    }
}

/// What a control the style draws is, for the focus a television shows on
/// it (the design's Apple TV focus, after tvOS's own).
enum BernFocusKind {
    /// A button the style draws: under the focus it turns white with a
    /// dark label and grows by a tenth.
    case control
    /// A row of a list: it turns white, its content dark, and lifts.
    case row
    /// A picture, such as a poster or a frame: it lifts with a shadow.
    case picture
}

extension View {
    /// The focus a television shows on a control the style draws; nothing
    /// elsewhere, and nothing while the control is not focused.
    @ViewBuilder func bernTelevisionFocus(_ kind: BernFocusKind, corner: Double, shown: Bool = true) -> some View {
        #if os(tvOS)
        modifier(BernTelevisionFocus(kind: kind, corner: corner, shown: shown))
        #else
        self
        #endif
    }
}

#if os(tvOS)
/// The focus of a button's content: the button tells it.
private struct BernTelevisionFocus: ViewModifier {
    let kind: BernFocusKind
    let corner: Double
    let shown: Bool
    @Environment(\.isFocused) private var isFocused

    func body(content: Content) -> some View {
        content.modifier(BernTelevisionFocusLook(kind: kind, corner: corner, focused: isFocused && shown))
    }
}

/// A row that takes the focus itself, as a sidebar's: it is focusable and
/// shows its focus as a row.
struct BernFocusableRow: ViewModifier {
    let corner: Double
    @FocusState private var focused: Bool

    func body(content: Content) -> some View {
        content
            .modifier(BernTelevisionFocusLook(kind: .row, corner: corner, focused: focused))
            .focusable()
            .focused($focused)
    }
}

private struct BernTelevisionFocusLook: ViewModifier {
    let kind: BernFocusKind
    let corner: Double
    let focused: Bool
    @BernReducedMotion private var reduce

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: corner, style: .continuous)
        content
            // A row or a control turns white under the focus; what it
            // shows reads dark on it, as on a light page.
            .environment(\.colorScheme, focused && kind != .picture ? .light : .dark)
            .background { if focused && kind != .picture { shape.fill(Color.white) } }
            .scaleEffect(focused ? (kind == .picture ? 1.08 : kind == .row ? 1.03 : 1.1) : 1)
            .shadow(color: .black.opacity(focused ? (kind == .picture ? 0.65 : 0.5) : 0),
                    radius: focused ? (kind == .picture ? 22 : 17) : 0, y: focused ? (kind == .picture ? 13 : 8) : 0)
            .zIndex(focused ? 1 : 0)
            .animation(
                reduce || bernForcedReduceMotion
                    ? .easeInOut(duration: 0.15)
                    : .spring(response: 0.3, dampingFraction: focused ? 0.6 : 1),
                value: focused)
    }
}
#endif

extension View {
    /// `color` for text and icons when there is one; what lies above
    /// otherwise.
    @ViewBuilder
    func bernForeground(_ color: Color?) -> some View {
        if let color {
            self.foregroundStyle(color)
        } else {
            self
        }
    }
}

// MARK: - Accent and system controls

private struct BernAccentKey: EnvironmentKey {
    static let defaultValue: Color? = nil
}

extension EnvironmentValues {
    /// The accent the application chose, for the controls that show one.
    var bernAccent: Color? {
        get { self[BernAccentKey.self] }
        set { self[BernAccentKey.self] = newValue }
    }
}

/// Tints a control with the application's accent; without one the system's
/// accent stays.
struct BernAccentTint: ViewModifier {
    @Environment(\.bernAccent) private var accent

    func body(content: Content) -> some View {
        if let accent {
            content.tint(accent)
        } else {
            content
        }
    }
}

/// Which system button a button is: the platform's own styles, Liquid Glass
/// on macOS and iOS 26 and the bordered styles before.
enum BernButtonLook {
    case secondary, prominent, icon
    /// An icon with no background, in the secondary color.
    case flat
}

extension View {
    /// Takes the application's accent: the main button, switches, sliders,
    /// progress and sidebar icons.
    func bernAccentTint() -> some View {
        modifier(BernAccentTint())
    }

    /// A real system glass capsule around a native button or pull-down.
    /// Explicit height keeps neighboring actions the same size regardless
    /// of the different default AppKit button and menu bezels.
    func bernCapsuleControl(height: CGFloat, prominent: Bool = false) -> some View {
        modifier(BernCapsuleControl(height: height, prominent: prominent))
    }

    func bernButtonLook(_ look: BernButtonLook) -> some View {
        modifier(BernSystemButtonLook(look: look))
    }

}

private struct BernControlGroupHeightKey: EnvironmentKey {
    static let defaultValue: CGFloat? = nil
}
extension EnvironmentValues {
    var bernControlGroupHeight: CGFloat? {
        get { self[BernControlGroupHeightKey.self] }
        set { self[BernControlGroupHeightKey.self] = newValue }
    }
}

/// Controls on a bar the system draws, such as a `TabAccessory`: the bar
/// is the surface, so icon buttons are glyphs on it, without glass of
/// their own.
private struct BernOnBarKey: EnvironmentKey {
    static let defaultValue = false
}
extension EnvironmentValues {
    var bernOnBar: Bool {
        get { self[BernOnBarKey.self] }
        set { self[BernOnBarKey.self] = newValue }
    }
}

private struct BernGroupedButtonStyle: ButtonStyle {
    let height: CGFloat
    func makeBody(configuration: Configuration) -> some View {
        #if os(tvOS)
        BernFocusedGroupButton(configuration: configuration, height: height)
        #else
        configuration.label
            .frame(minWidth: height, minHeight: height)
            .contentShape(Rectangle())
            .opacity(configuration.isPressed ? 0.55 : 1)
        #endif
    }
}

#if os(tvOS)
/// A button of a group's shared glass on tvOS: under the focus its part
/// of the glass turns white with a dark label and lifts, as the focus of
/// tvOS's own buttons shows.
private struct BernFocusedGroupButton: View {
    let configuration: ButtonStyle.Configuration
    let height: CGFloat
    @Environment(\.isFocused) private var focused

    var body: some View {
        configuration.label
            .frame(minWidth: height, minHeight: height)
            .foregroundStyle(focused ? Color.black : Color.primary)
            .background { Capsule().fill(Color.white).opacity(focused ? 1 : 0) }
            .contentShape(Capsule())
            .scaleEffect(configuration.isPressed ? 0.94 : focused ? 1.1 : 1)
            .shadow(color: .black.opacity(focused ? 0.4 : 0), radius: focused ? 12 : 0, y: focused ? 6 : 0)
            .animation(.spring(response: 0.35, dampingFraction: focused ? 0.6 : 1), value: focused)
    }
}
#endif

private struct BernSceneScaledKey: EnvironmentKey {
    static let defaultValue = false
}

private struct BernSceneScaleKey: EnvironmentKey {
    static let defaultValue: CGFloat = 1
}

extension EnvironmentValues {
    /// Inside a `ScaleBox`: a scene laid out at fixed coordinates and
    /// scaled to its room.
    var bernSceneScaled: Bool {
        get { self[BernSceneScaledKey.self] }
        set { self[BernSceneScaledKey.self] = newValue }
    }

    /// How much a `ScaleBox` around enlarges its scene; 1 outside one.
    var bernSceneScale: CGFloat {
        get { self[BernSceneScaleKey.self] }
        set { self[BernSceneScaleKey.self] = newValue }
    }

    /// The scene a layer is placed in and the room around it; none inside
    /// a layer, which places its own layers in its own space.
    var bernSceneRoom: BernSceneRoom? {
        get { self[BernSceneRoomKey.self] }
        set { self[BernSceneRoomKey.self] = newValue }
    }
}

/// A `ScaleBox` scene's size and the room the box has around it on each
/// side, in scene points.
struct BernSceneRoom: Equatable {
    var width: CGFloat
    var height: CGFloat
    var x: CGFloat
    var y: CGFloat
}

private struct BernSceneRoomKey: EnvironmentKey {
    static let defaultValue: BernSceneRoom? = nil
}

/// A `Positioned` layer that bleeds: each side that reaches the edge of its
/// scene continues past it to the edge of the room around the scene, so
/// the layer meets the window's edges whatever the window's shape.
struct BernBleedingLayer: ViewModifier {
    let x: CGFloat
    let y: CGFloat
    let width: CGFloat
    let height: CGFloat
    let bleed: Bool
    @Environment(\.bernSceneRoom) private var room

    func body(content: Content) -> some View {
        // Half a point of slack: a layer laid out to the edge reaches it.
        let room = bleed ? room : nil
        let left = room.map { x <= 0.5 ? $0.x : 0 } ?? 0
        let right = room.map { x + width >= $0.width - 0.5 ? $0.x : 0 } ?? 0
        let top = room.map { y <= 0.5 ? $0.y : 0 } ?? 0
        let bottom = room.map { y + height >= $0.height - 0.5 ? $0.y : 0 } ?? 0
        content
            .environment(\.bernSceneRoom, nil)
            .frame(width: max(0, width + left + right), height: max(0, height + top + bottom))
            .offset(x: x - left, y: y - top)
    }
}

private struct BernSystemButtonLook: ViewModifier {
    let look: BernButtonLook
    @Environment(\.bernControlGroupHeight) private var groupHeight
    @Environment(\.bernOnBar) private var onBar
    @Environment(\.bernSceneScaled) private var scaled
    @Environment(\.bernAccent) private var accent
    @Environment(\.controlSize) private var controlSize

    @ViewBuilder func body(content: Content) -> some View {
        #if os(tvOS)
        // tvOS's own buttons take a television's sizes, which a scene that
        // is scaled up as a whole would enlarge again: there a button keeps
        // the design's sizes for a television (46 points, 52 for a large
        // one, on 16 and 17-point type) and shows the focus as tvOS's own.
        if scaled && groupHeight == nil && !onBar {
            let large = controlSize == .large || controlSize == .extraLarge
            switch look {
            case .icon:
                content.buttonStyle(BernFocusShapeStyle(
                    prominent: false, accent: accent ?? .accentColor, circle: true, height: 46, padding: 0))
                    .font(.system(size: 20))
            case .prominent, .secondary:
                content.buttonStyle(BernFocusShapeStyle(
                    prominent: look == .prominent, accent: accent ?? .accentColor, circle: false,
                    height: large ? 52 : 46, padding: large ? 28 : 22))
                    .font(.system(size: large ? 17 : 16, weight: look == .prominent ? .semibold : .medium))
            case .flat:
                // A plain button is its label, as a link reads in the text.
                content.buttonStyle(BernFocusShapeStyle(
                    prominent: false, accent: accent ?? .accentColor, circle: false, height: 0, padding: 0,
                    plain: true))
                    .font(.system(size: 16))
            }
        } else {
            standard(content)
        }
        #else
        standard(content)
        #endif
    }

    @ViewBuilder private func standard(_ content: Content) -> some View {
        if let groupHeight {
            content.buttonStyle(BernGroupedButtonStyle(height: groupHeight)).foregroundStyle(.primary)
        } else if onBar && (look == .icon || look == .flat) {
            content.buttonStyle(BernGroupedButtonStyle(height: 0)).foregroundStyle(.primary)
        } else if #available(macOS 26.0, iOS 26.0, tvOS 26.0, *) {
            switch look {
            case .secondary: content.buttonStyle(.glass).bernPrimaryLabel()
            case .prominent: content.buttonStyle(.glassProminent).bernAccentTint()
            case .icon: content.buttonStyle(.glass).buttonBorderShape(.circle).bernPrimaryLabel()
            case .flat: content.buttonStyle(.borderless).foregroundStyle(.secondary)
            }
        } else {
            switch look {
            case .secondary: content.buttonStyle(.bordered).bernPrimaryLabel()
            case .prominent: content.buttonStyle(.borderedProminent).bernAccentTint()
            case .icon: content.buttonStyle(.bordered).buttonBorderShape(.circle).bernPrimaryLabel()
            case .flat: content.buttonStyle(.borderless).foregroundStyle(.secondary)
            }
        }
    }
}

extension View {
    /// iOS colors a plain button's label with the surrounding tint, the
    /// application's accent under a tab bar; the design keeps the accent
    /// to the main button (8.1), so it is the primary color, as on macOS.
    @ViewBuilder func bernPrimaryLabel() -> some View {
        #if os(iOS)
        tint(.primary)
        #else
        self
        #endif
    }
}

/// One system glass surface for related buttons. The controls remain
/// native Buttons, Menus and popovers, with independent hit targets.
struct BernControlGroup<Content: View>: View {
    let height: CGFloat
    @ViewBuilder let content: Content
    var body: some View {
        HStack(spacing: 0) { content }
            .font(.system(size: 18, weight: .regular))
            .frame(height: height)
            .environment(\.bernControlGroupHeight, height)
            .modifier(BernGlassCapsule(prominent: false))
            .accessibilityElement(children: .contain)
    }
}

private struct BernGlassCapsule: ViewModifier {
    var prominent = false
    @Environment(\.bernAccent) private var accent
    @ViewBuilder func body(content: Content) -> some View {
        if #available(macOS 26.0, iOS 26.0, tvOS 26.0, *) {
            if prominent {
                content.foregroundStyle(BernColor.role("on_accent").color)
                    .background(accent ?? .accentColor, in: Capsule())
                    .glassEffect(.regular.tint(accent ?? .accentColor).interactive(), in: Capsule())
            } else {
                content.foregroundStyle(.primary)
                    .glassEffect(.regular.interactive(), in: Capsule())
            }
        } else {
            content.foregroundStyle(prominent ? BernColor.role("on_accent").color : Color.primary)
                .background(prominent ? AnyShapeStyle(accent ?? .accentColor) : AnyShapeStyle(.regularMaterial), in: Capsule())
        }
    }
}

extension View {
    /// Keeps the title out of the title bar; the window keeps it for its
    /// menus and the window list.
    @ViewBuilder
    func bernToolbarTitleHidden() -> some View {
        #if os(tvOS)
        self
        #else
        if #available(macOS 15.0, iOS 18.0, *) {
            toolbar(removing: .title)
        } else {
            self
        }
        #endif
    }
}

/// A sidebar destination: its icon takes the application's accent, as the
/// system's sidebars take the system's.
struct BernSidebarLabel: View {
    let title: String
    let systemImage: String
    @Environment(\.bernAccent) private var accent

    var body: some View {
        Label {
            Text(verbatim: title)
        } icon: {
            Image(systemName: systemImage).foregroundStyle(accent ?? Color.accentColor)
        }
    }
}

// MARK: - Theme

/// The accent the application chose at run time, read by the `accent`
/// role; views that draw with it follow its changes.
@MainActor
@Observable
final class BernThemeState {
    static let shared = BernThemeState()
    var accent: BernColor?
    var palette: [String: BernColor] = [:]
}

/// Malformed values never reach a platform color constructor.
private func bernPalette(_ text: String) -> [String: BernColor] {
    guard let data = text.data(using: .utf8),
          let input = try? JSONSerialization.jsonObject(with: data) as? [String: [String: String]] else { return [:] }
    func hex(_ value: String?) -> UInt32? {
        guard let value, value.count == 7, value.first == "#" else { return nil }
        return UInt32(value.dropFirst(), radix: 16)
    }
    return input.reduce(into: [:]) { result, item in
        if let light = hex(item.value["light"]), let dark = hex(item.value["dark"]) {
            result[item.key] = .fixed(light: light, dark: dark)
        }
    }
}

/// The accent and appearance of `WindowSet`, applied to a window. The
/// accent is not the window's tint: only the controls that show an accent
/// take it (`bernAccentTint`), drawn parts through the `accent` role; the
/// appearance is the application's (`system`, `light`, `dark`).
private struct BernWindowSchemeKey: EnvironmentKey {
    static let defaultValue: ColorScheme? = nil
}
extension EnvironmentValues {
    var bernWindowScheme: ColorScheme? {
        get { self[BernWindowSchemeKey.self] }
        set { self[BernWindowSchemeKey.self] = newValue }
    }
}

struct BernThemeChoice: ViewModifier {
    @Environment(\.colorScheme) private var scheme
    let accent: BernColor?
    let appearance: String?
    var palette: String = ""

    func body(content: Content) -> some View {
        let colors = bernPalette(palette)
        let chosen = colors["accent"] ?? accent
        content
            .environment(\.bernWindowScheme, appearance?.lowercased() == "dark" ? .dark : appearance?.lowercased() == "light" ? .light : scheme)
            .environment(\.bernAccent, chosen?.color)
            .onChange(of: chosen, initial: true) { BernThemeState.shared.accent = chosen }
            .onChange(of: palette, initial: true) { BernThemeState.shared.palette = colors }
        #if os(macOS)
            .onChange(of: appearance, initial: true) {
                switch appearance?.lowercased() {
                case "light": NSApplication.shared.appearance = NSAppearance(named: .aqua)
                case "dark": NSApplication.shared.appearance = NSAppearance(named: .darkAqua)
                case "system": NSApplication.shared.appearance = nil
                default: break
                }
            }
        #else
            .preferredColorScheme(
                appearance?.lowercased() == "light" ? .light : appearance?.lowercased() == "dark" ? .dark : nil)
        #endif
    }
}

// MARK: - Search

/// A search field with its results below it. External entries edit,
/// submit and focus it; `shortcut` focuses it from the keyboard. Focus
/// needs macOS 15 or iOS 18; before them the field cannot be focused by
/// command or shortcut.
struct BernSearchable<Results: View>: ViewModifier {
    let text: Binding<String>
    let placement: SearchFieldPlacement
    let prompt: String?
    let target: String
    var submit: (@MainActor () -> Void)? = nil
    var shortcut: BernKeyboardShortcut? = nil
    @ViewBuilder let results: () -> Results
    @FocusState private var focused: Bool

    func body(content: Content) -> some View {
        let field = content
            .searchable(text: text, placement: placement, prompt: prompt.map { Text(verbatim: $0) })
            .searchSuggestions { results() }
            .onSubmit(of: .search) { submit?() }
        SwiftUI.Group {
            #if os(tvOS)
            // tvOS searches on a screen of its own, which takes the focus
            // when it opens.
            field
            #else
            if #available(macOS 15.0, iOS 18.0, *) {
                field.searchFocused($focused)
            } else {
                field
            }
            #endif
        }
        .bernHiddenShortcut(shortcut) { focused = true }
        .bernControl(
            target, role: "search", label: prompt ?? "",
            actions: BernControlActions(
                edit: { text.wrappedValue = $0 }, submit: submit, value: { text.wrappedValue },
                focus: { focused = true }))
    }
}

// MARK: - Split buttons

/// A toggle linked with a button that shows `content` in a popover; one
/// control for external entries: `set_bool` turns it on or off,
/// `activate` opens the popover.
struct BernSplitButton<Content: View>: View {
    let label: String
    let systemImage: String?
    let more: String
    @Binding var isOn: Bool
    let capsule: Bool
    let height: CGFloat
    let target: String
    @ViewBuilder let content: Content
    @Environment(\.colorScheme) private var scheme
    @State private var showing = false

    init(
        label: String, systemImage: String?, more: String, isOn: Binding<Bool>, capsule: Bool = false, height: CGFloat = 34, target: String,
        @ViewBuilder content: () -> Content
    ) {
        self.label = label
        self.systemImage = systemImage
        self.more = more
        self._isOn = isOn
        self.capsule = capsule
        self.height = height
        self.target = target
        self.content = content()
    }

    @ViewBuilder private var toggleLabel: some View {
        if let systemImage {
            Label(label, systemImage: systemImage).labelStyle(.titleAndIcon)
        } else {
            Text(verbatim: label)
        }
    }

    /// The design's split button (6.3): the symbol in a 16-point box, 6
    /// before the words.
    @ViewBuilder private var capsuleLabel: some View {
        if let systemImage {
            HStack(spacing: 6) {
                Image(systemName: systemImage).frame(width: 16, height: 16).accessibilityHidden(true)
                Text(verbatim: label)
            }
        } else {
            Text(verbatim: label)
        }
    }

    @ViewBuilder private var buttons: some View {
        if capsule {
            HStack(spacing: 0) {
                Toggle(isOn: $isOn) { capsuleLabel }
                    .toggleStyle(BernSplitToggleStyle(height: height))
                Rectangle().fill(.primary.opacity(0.18)).frame(width: 0.5, height: height - 14)
                Button { showing = true } label: {
                    Image(systemName: "chevron.down").font(.system(size: 11, weight: .semibold))
                        .frame(width: 30, height: height).contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(verbatim: more))
                .help(more)
            }
            .font(.system(size: 13, weight: .medium))
            .modifier(BernGlassCapsule(prominent: isOn))
        } else {
            ControlGroup {
                Toggle(isOn: $isOn) { toggleLabel }.bernButtonToggle()
                Button { showing = true } label: { Image(systemName: "chevron.down") }
                    .accessibilityLabel(Text(verbatim: more)).help(more)
            }
        }
    }

    var body: some View {
        buttons
        .bernAccentTint()
        .fixedSize()
        // A control group turns its buttons into segments of one control,
        // which anchors the popover.
        .bernPopover(isPresented: $showing) {
            VStack(alignment: .leading, spacing: 12) { content }
                .padding(16)
                .frame(minWidth: 240, alignment: .leading)
                // A popover also on iPhone, which would make it a sheet,
                // drawn in the appearance of where it opens, as the
                // content is.
                .presentationCompactAdaptation(.popover)
                .bernPopoverBackground(nil, scheme: scheme)
                .environment(\.colorScheme, scheme)
                .environment(\.bernControlGroupHeight, nil)
        }
        .bernControl(
            target, role: "split_button", label: label,
            actions: BernControlActions(
                activate: { showing = true }, setBool: { isOn = $0 }, value: { isOn ? "true" : "false" }))
    }
}

private struct BernSplitToggleStyle: ToggleStyle {
    let height: CGFloat
    func makeBody(configuration: Configuration) -> some View {
        Button { configuration.isOn.toggle() } label: {
            // 14 from the capsule's end, 10 before the divider.
            configuration.label.padding(.leading, 14).padding(.trailing, 10).frame(height: height).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(configuration.isOn ? [.isSelected] : [])
    }
}

// MARK: - Notices

/// A short notice: it rises in when it appears or its key changes and sinks
/// out after `duration` seconds; a newer notice restarts the time.
struct BernToast: View {
    let text: String
    let systemImage: String?
    let key: AnyHashable
    let duration: Double
    let motion: BernMotion
    @BernReducedMotion private var reduce
    @State private var shown = false

    var body: some View {
        SwiftUI.Group {
            if let systemImage {
                SwiftUI.Label(text, systemImage: systemImage)
            } else {
                SwiftUI.Text(verbatim: text)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.regularMaterial, in: Capsule())
        .shadow(color: .black.opacity(0.12), radius: 10, y: 4)
        .opacity(shown ? 1 : 0)
        .offset(y: shown || reduce || bernForcedReduceMotion ? 0 : 24)
        .allowsHitTesting(false)
        .accessibilityAddTraits(.updatesFrequently)
        .task(id: key) {
            withAnimation(reduce || bernForcedReduceMotion ? .easeInOut(duration: 0.15) : motion.animation) { shown = true }
            try? await Task.sleep(for: .seconds(duration))
            guard !Task.isCancelled else { return }
            withAnimation(.easeInOut(duration: 0.25)) { shown = false }
        }
    }
}

// MARK: - Reveal on hover

private struct BernHoverScopeKey: EnvironmentKey {
    /// Without a pointer, revealed content always shows.
    #if os(macOS)
    static let defaultValue = false
    #else
    static let defaultValue = true
    #endif
}

extension EnvironmentValues {
    /// Whether the pointer is over the container of content marked
    /// `reveal: hover`.
    var bernHoverScope: Bool {
        get { self[BernHoverScopeKey.self] }
        set { self[BernHoverScopeKey.self] = newValue }
    }
}

extension View {
    /// A container whose content marked `reveal: hover` shows while the
    /// pointer is over it.
    func bernHoverScope() -> some View {
        modifier(BernHoverScope())
    }

    /// Shown while the pointer is over the enclosing container, or while
    /// it has the keyboard focus; still there for assistive technologies.
    func bernReveal(_ motion: BernMotion) -> some View {
        modifier(BernReveal(motion: motion))
    }
}

private struct BernHoverScope: ViewModifier {
    @State private var hover = false

    func body(content: Content) -> some View {
        #if os(macOS)
        content
            .environment(\.bernHoverScope, hover)
            .onHover { hover = $0 }
        #else
        content
        #endif
    }
}

private struct BernReveal: ViewModifier {
    let motion: BernMotion
    @Environment(\.bernHoverScope) private var hover
    @BernReducedMotion private var reduce
    @FocusState private var focused: Bool

    func body(content: Content) -> some View {
        let shown = hover || focused
        content
            .focused($focused)
            .opacity(shown ? 1 : 0)
            .allowsHitTesting(shown)
            .animation(reduce || bernForcedReduceMotion ? .easeInOut(duration: 0.15) : motion.animation, value: shown)
    }
}

/// Styled rows draw selection themselves, so the list no longer owns it.
/// Keep arrow-key navigation scoped to the focused sidebar, using only
/// currently visible destinations (including conditional module pages).
private struct BernSidebarNavigation<Value: Hashable>: ViewModifier {
    @Binding var selection: Value
    let values: [Value]
    @FocusState private var focused: Bool

    func body(content: Content) -> some View {
        content
            .focusable()
            .focused($focused)
            .focusEffectDisabled()
            .simultaneousGesture(TapGesture().onEnded { focused = true })
            .onKeyPress(.upArrow) { move(-1) }
            .onKeyPress(.downArrow) { move(1) }
    }

    private func move(_ offset: Int) -> KeyPress.Result {
        guard !values.isEmpty else { return .ignored }
        let current = values.firstIndex(of: selection) ?? (offset > 0 ? -1 : values.count)
        let next = min(max(current + offset, 0), values.count - 1)
        selection = values[next]
        return .handled
    }
}

extension View {
    func bernSidebarNavigation<Value: Hashable>(selection: Binding<Value>, values: [Value]) -> some View {
        modifier(BernSidebarNavigation(selection: selection, values: values))
    }
}

/// A sidebar destination the style draws: hover, press and selection
/// follow the style instead of the system's highlight.
struct BernSidebarRow<Value: Hashable>: View {
    let label: String
    let systemImage: String?
    let value: Value
    @Binding var selection: Value
    let style: BernStyle
    var badge: Int = 0
    @Environment(\.bernAccent) private var accent
    @BernReducedMotion private var reduce
    @State private var hover = false
    @State private var pressed = false

    var body: some View {
        let isSelected = selection == value
        let (look, motion) = style.look(selected: isSelected, hover: hover, pressed: pressed, enabled: true)
        let fill = isSelected ? look.fill : nil
        let shape = RoundedRectangle(cornerRadius: style.corner ?? 6, style: .continuous)
        let foreground = look.foreground?.color ?? fill?.onTop ?? look.background?.onTop ?? Color.primary
        // Neutral fills keep the platform's accent-colored navigation icons;
        // colored fills and explicit foregrounds keep their contrast pairing.
        let neutralFill = fill == nil || fill == .role("fill.subtle") || fill == .role("fill.strong")
        let iconForeground = look.foreground?.color ?? (neutralFill ? accent ?? Color.accentColor : foreground)
        HStack(spacing: 7) {
            if let systemImage {
                Image(systemName: systemImage)
                    .frame(width: 18)
                    .foregroundStyle(iconForeground)
            }
            Text(verbatim: label)
                .fontWeight(isSelected ? .bold : nil)
            Spacer(minLength: 0)
            if badge > 0 { Text(String(badge)).font(.system(size: 11, weight: .medium)).foregroundStyle(.secondary) }
        }
        .padding(.vertical, (style.padding ?? 8) * 0.6)
        .padding(.horizontal, style.padding ?? 8)
        .foregroundStyle(foreground)
        .background {
            ZStack(alignment: bernAlignment(look.indicatorEdge)) {
                shape.fill(fill?.color ?? look.background?.color ?? Color.clear)
                if isSelected, let indicator = look.indicator {
                    bernIndicator(indicator.color, edge: look.indicatorEdge, width: look.indicatorWidth)
                }
            }
        }
        .contentShape(Rectangle())
        .bernTransform(look, motion, key: bernStateKey(isSelected, hover, pressed, true), reduce: reduce)
        // List can disable implicit animations for its cells. Re-enable
        // them at this row's boundary so bernTransform's state animation
        // interpolates both the background and the scale on hover/press.
        .transaction { $0.disablesAnimations = false }
        .bernHover { updateInteraction(hover: $0, pressed: $0 && pressed) }
        #if os(tvOS)
        // A row takes the focus, turns white under it, and the remote's
        // select chooses it.
        .modifier(BernFocusableRow(corner: style.corner ?? 6))
        #endif
        .onTapGesture { selection = value }
        .bernPressWatch { updateInteraction(hover: hover, pressed: $0) }
        .listRowBackground(Color.clear)
        .bernListRowSeparatorHidden()
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? [.isButton, .isSelected] : .isButton)
        .accessibilityAction { selection = value }
    }

    private func updateInteraction(hover nextHover: Bool, pressed nextPressed: Bool) {
        guard hover != nextHover || pressed != nextPressed else { return }
        let (_, motion) = style.look(selected: selection == value, hover: nextHover, pressed: nextPressed, enabled: true)
        let animation = reduce || bernForcedReduceMotion ? Animation.easeInOut(duration: 0.15) : motion.animation
        withAnimation(animation) {
            hover = nextHover
            pressed = nextPressed
        }
    }
}

private func bernAlignment(_ edge: Edge) -> Alignment {
    switch edge {
    case .leading: return .leading
    case .trailing: return .trailing
    case .top: return .top
    case .bottom: return .bottom
    }
}

@ViewBuilder
private func bernIndicator(_ color: Color, edge: Edge, width: Double) -> some View {
    switch edge {
    case .leading, .trailing:
        Capsule().fill(color).frame(width: width).padding(.vertical, 6)
    case .top, .bottom:
        Capsule().fill(color).frame(height: width).padding(.horizontal, 6)
    }
}

// MARK: - Transitions and shared elements

extension View {
    /// How the element appears and disappears, each moved by its spring;
    /// a short fade when motion is reduced.
    func bernTransition(
        insertion: AnyTransition?, insertionMotion: BernMotion?,
        removal: AnyTransition?, removalMotion: BernMotion?, appear: BernAppear? = nil,
        delay: Double? = nil
    ) -> some View {
        modifier(BernTransitionModifier(
            insertion: insertion, insertionMotion: insertionMotion,
            removal: removal, removalMotion: removalMotion, appear: appear, delay: delay))
    }

    /// One of the elements sharing `id` in a window: when one replaces
    /// another, it grows from where the other was, on `motion`.
    func bernHero(_ id: String, motion: BernMotion = BernMotion(period: 0.48, damping: 0.86)) -> some View {
        modifier(BernHero(id: id, motion: motion))
    }

    /// Lines of `height` points for text set at `size`: the extra over the
    /// font's own line goes between the lines and, halved, above the first
    /// and below the last, as CSS's line-height.
    func bernLineHeight(_ height: CGFloat, size: CGFloat) -> some View {
        #if os(macOS)
        let font = NSFont.systemFont(ofSize: size)
        let natural = font.ascender - font.descender + font.leading
        #else
        let natural = UIFont.systemFont(ofSize: size).lineHeight
        #endif
        let extra = max(0, height - natural)
        return lineSpacing(extra).padding(.vertical, extra / 2)
    }

    /// A form's or scroll view's entrance: see `BernScrollEntrance`.
    func bernScrollEntrance(motion: BernMotion, delay: Double) -> some View {
        modifier(BernScrollEntrance(motion: motion, delay: delay))
    }

    /// Where the shared elements of one window record their frames.
    func bernHeroRoot() -> some View {
        modifier(BernHeroRoot())
    }
}

/// A scroll view draws the background of the bar above it, the title bar
/// on macOS and the navigation bar elsewhere, in its own frame. It enters
/// by fading in only what lies below the top safe area, so that background
/// and the bar stay as they are. The scroll view itself never moves.
private struct BernScrollEntrance: ViewModifier {
    let motion: BernMotion
    let delay: Double
    @BernReducedMotion private var reduce
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .mask {
                GeometryReader { geometry in
                    VStack(spacing: 0) {
                        Color.black.frame(height: geometry.safeAreaInsets.top)
                        Color.black.opacity(shown ? 1 : 0)
                    }
                    .ignoresSafeArea()
                }
            }
            .onAppear {
                let animation = reduce || bernForcedReduceMotion
                    ? Animation.easeInOut(duration: 0.15) : motion.animation
                withAnimation(animation.delay(delay)) { shown = true }
            }
    }
}

private struct BernTransitionModifier: ViewModifier {
    let insertion: AnyTransition?
    let insertionMotion: BernMotion?
    let removal: AnyTransition?
    let removalMotion: BernMotion?
    /// The appearing, for when a staggering container sets the turns.
    let appear: BernAppear?
    let delay: Double?
    @BernReducedMotion private var reduce
    @Environment(\.bernStagger) private var stagger

    func body(content: Content) -> some View {
        if let appear, stagger != nil || delay != nil || appear.grows {
            content
                .modifier(BernEntrance(appear: appear, motion: insertionMotion, clock: stagger, delay: delay))
                .transition(.asymmetric(
                    insertion: .identity,
                    removal: (removal ?? .identity).animation(removalMotion?.animation)))
        } else if reduce || bernForcedReduceMotion {
            content.transition(.opacity.animation(.easeInOut(duration: 0.15)))
        } else {
            content.transition(.asymmetric(
                insertion: (insertion ?? .identity).animation(insertionMotion?.animation),
                removal: (removal ?? .identity).animation(removalMotion?.animation)))
        }
    }
}

/// Where each shared element of a window was last laid out, in window
/// coordinates, so the one replacing it knows where to grow from.
@MainActor
final class BernHeroFrames {
    var last: [String: CGRect] = [:]
    /// How many elements of each id are on screen, and when the last one
    /// of an id went away.
    var mounted: [String: Int] = [:]
    var departed: [String: Date] = [:]

    /// The frame of the element an element of `id` appearing now replaces:
    /// one still on screen or gone in the last half second. An element
    /// that only comes back, such as a list row scrolled back into view,
    /// replaces nothing.
    func replaced(_ id: String) -> CGRect? {
        let recent = mounted[id, default: 0] > 0
            || departed[id].map { -$0.timeIntervalSinceNow < 0.5 } == true
        return recent ? last[id] : nil
    }
}

private struct BernHeroFramesKey: EnvironmentKey {
    static let defaultValue: BernHeroFrames? = nil
}

extension EnvironmentValues {
    var bernHeroFrames: BernHeroFrames? {
        get { self[BernHeroFramesKey.self] }
        set { self[BernHeroFramesKey.self] = newValue }
    }
}

private struct BernHeroRoot: ViewModifier {
    @State private var frames = BernHeroFrames()

    func body(content: Content) -> some View {
        content.environment(\.bernHeroFrames, frames)
    }
}

/// The element replacing another of the same id grows from where that one
/// was. `matchedGeometryEffect` would change their layout on the way, and
/// a split view laid out like that on macOS loses its toolbar safe area for
/// good, its columns ending up under the title bar. On macOS a picture of
/// the two flies above the window instead (`BernViewHero`). Elsewhere the
/// replacing element is first laid out untouched and unseen, then drawn
/// moved and scaled onto the other's frame, springing back to its own;
/// its layout never changes.
private struct BernHero: ViewModifier {
    let id: String
    let motion: BernMotion
    @Environment(\.bernHeroFrames) private var frames
    @BernReducedMotion private var reduce
    @State private var scale = CGSize(width: 1, height: 1)
    @State private var offset = CGSize.zero
    @State private var measured = false
    @State private var shown = false

    func body(content: Content) -> some View {
        #if os(macOS)
        content.modifier(BernViewHero(id: id, motion: motion))
        #else
        if let frames, !(reduce || bernForcedReduceMotion) {
            // An element with one to replace waits for its first layout.
            let waiting = !shown && (measured || frames.replaced(id) != nil)
            content
                .scaleEffect(x: scale.width, y: scale.height)
                .offset(offset)
                .opacity(waiting ? 0 : 1)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { grow(to: $0) }
                .onDisappear {
                    guard measured else { return }
                    frames.mounted[id, default: 1] -= 1
                    frames.departed[id] = Date()
                }
        } else {
            content
        }
        #endif
    }

    /// After the first layout, start on the frame of the element it
    /// replaces and spring to its own.
    private func grow(to frame: CGRect) {
        guard let frames else { return }
        let previous = measured ? nil : frames.replaced(id)
        frames.last[id] = frame
        guard !measured else { return }
        measured = true
        frames.mounted[id, default: 0] += 1
        guard let previous, frame.width > 0, frame.height > 0, previous != frame else {
            shown = true
            return
        }
        var still = Transaction()
        still.disablesAnimations = true
        DispatchQueue.main.async {
            withTransaction(still) {
                scale = CGSize(width: previous.width / frame.width, height: previous.height / frame.height)
                offset = CGSize(width: previous.midX - frame.midX, height: previous.midY - frame.midY)
                shown = true
            }
            DispatchQueue.main.async {
                withAnimation(motion.animation) {
                    scale = CGSize(width: 1, height: 1)
                    offset = .zero
                }
            }
        }
    }
}

/// Images travel above the native split-view columns on macOS. Other
/// Apple platforms retain SwiftUI's shared-element implementation.
struct BernPictureHero: ViewModifier {
    let id: String?
    let picture: BernImage.Picture?
    let fit: BernImageFit
    let corner: CGFloat
    let motion: BernMotion
    @BernReducedMotion private var reduce
    @State private var flying = false

    func body(content: Content) -> some View {
        if let id {
            #if os(macOS)
            content
                .opacity(flying ? 0 : 1)
                .background(BernPictureHeroProbe(id: id, picture: picture, fit: fit,
                    corner: corner, motion: motion, reduced: reduce || bernForcedReduceMotion,
                    visibility: { hidden in
                        var transaction = Transaction()
                        transaction.disablesAnimations = true
                        withTransaction(transaction) { flying = hidden }
                    }))
            #else
            content.bernHero(id)
            #endif
        } else {
            content
        }
    }
}

// MARK: - Drawing

/// A band of a pattern: `width` points of `color`, or a gap when `color`
/// is nil.
struct BernBand: Sendable {
    let color: BernColor?
    let width: Double
    let opacity: Double
}

/// A woven check: the bands repeat across and down, and their colors add
/// up where they cross; `twill` lays fine diagonal lines over it.
struct BernPatternDef: Sendable {
    let bands: [BernBand]
    let twill: Bool
}

/// What `Canvas` children draw, in canvas points.
enum BernDraw {
    static func shape(
        _ context: inout GraphicsContext, _ kind: String, _ rect: CGRect, corner: Double,
        fill: BernColor?, fade: String? = nil, stroke: BernColor?, strokeWidth: Double,
        dash: [Double] = [], opacity: Double
    ) {
        let path: Path
        switch kind {
        case "rounded":
            path = Path(roundedRect: rect, cornerRadius: corner, style: .continuous)
        case "capsule":
            path = Path(
                roundedRect: rect, cornerRadius: min(rect.width, rect.height) / 2,
                style: .continuous)
        case "ellipse":
            path = Path(ellipseIn: rect)
        default:
            path = Path(rect)
        }
        var layer = context
        layer.opacity = opacity
        if fill != nil || stroke == nil {
            let color = (fill ?? .role("fill.strong")).color
            if let fade {
                // The fill fades out toward the named edge.
                let (start, end): (CGPoint, CGPoint) = switch fade {
                case "up": (CGPoint(x: rect.midX, y: rect.maxY), CGPoint(x: rect.midX, y: rect.minY))
                case "left": (CGPoint(x: rect.maxX, y: rect.midY), CGPoint(x: rect.minX, y: rect.midY))
                case "right": (CGPoint(x: rect.minX, y: rect.midY), CGPoint(x: rect.maxX, y: rect.midY))
                default: (CGPoint(x: rect.midX, y: rect.minY), CGPoint(x: rect.midX, y: rect.maxY))
                }
                layer.fill(
                    path,
                    with: .linearGradient(
                        Gradient(colors: [color, color.opacity(0)]), startPoint: start, endPoint: end))
            } else {
                layer.fill(path, with: .color(color))
            }
        }
        if let stroke {
            layer.stroke(
                path, with: .color(stroke.color),
                style: StrokeStyle(lineWidth: strokeWidth, dash: dash.map { CGFloat($0) }))
        }
    }

    /// Values across the full width, scaled so `max` (or the largest
    /// value) reaches the top.
    static func series(
        _ context: inout GraphicsContext, _ size: CGSize, values: [Double], max top: Double?,
        area: Bool, fill: BernColor?, stroke: BernColor?, strokeWidth: Double, opacity: Double
    ) {
        guard !values.isEmpty else { return }
        let peak = top ?? values.max() ?? 1
        let scale = peak > 0 ? peak : 1
        let step = values.count > 1 ? size.width / Double(values.count - 1) : 0
        let points = values.enumerated().map { index, value in
            CGPoint(
                x: values.count > 1 ? Double(index) * step : size.width / 2,
                y: size.height - size.height * Swift.min(Swift.max(value / scale, 0), 1))
        }
        var line = Path()
        line.addLines(points)
        var layer = context
        layer.opacity = opacity
        let style = StrokeStyle(lineWidth: strokeWidth, lineCap: .round, lineJoin: .round)
        if area {
            var shape = line
            shape.addLine(to: CGPoint(x: points[points.count - 1].x, y: size.height))
            shape.addLine(to: CGPoint(x: points[0].x, y: size.height))
            shape.closeSubpath()
            layer.fill(shape, with: .color((fill ?? .role("accent")).color))
            if let stroke {
                layer.stroke(line, with: .color(stroke.color), style: style)
            }
        } else {
            layer.stroke(line, with: .color((stroke ?? .role("accent")).color), style: style)
        }
    }

    static func pattern(
        _ context: inout GraphicsContext, _ size: CGSize, _ pattern: BernPatternDef,
        opacity: Double
    ) {
        let sett = pattern.bands.reduce(0) { $0 + Swift.max($1.width, 0) }
        guard sett > 0 else { return }
        var layer = context
        layer.opacity = opacity
        for across in [true, false] {
            let extent = across ? size.width : size.height
            var offset = 0.0
            while offset < extent {
                for band in pattern.bands where band.width > 0 {
                    if let color = band.color {
                        let rect = across
                            ? CGRect(x: offset, y: 0, width: band.width, height: size.height)
                            : CGRect(x: 0, y: offset, width: size.width, height: band.width)
                        layer.fill(Path(rect), with: .color(color.color.opacity(band.opacity)))
                    }
                    offset += band.width
                    if offset >= extent { break }
                }
            }
        }
        if pattern.twill {
            var lines = Path()
            var start = -size.height
            while start < size.width {
                lines.move(to: CGPoint(x: start, y: size.height))
                lines.addLine(to: CGPoint(x: start + size.height, y: 0))
                start += 3
            }
            layer.stroke(lines, with: .color(Color.primary.opacity(0.05)), lineWidth: 0.5)
        }
    }
}

// MARK: - Text marks

extension View {
    /// `Text(mark:)`: a capsule of the color under each line, from 74% to
    /// 98% of the line's height and 0.4 of a character past its end,
    /// behind the glyphs and outside the layout.
    func bernMark(_ color: BernColor) -> some View {
        modifier(BernMark(color: color))
    }
}

private struct BernMark: ViewModifier {
    let color: BernColor

    func body(content: Content) -> some View {
        if #available(macOS 15, iOS 18, tvOS 18, *) {
            content.textRenderer(BernMarkRenderer(color: color.color))
        } else {
            // Before text renderers there are no line positions: underline.
            content.underline(true, color: color.color)
        }
    }
}

@available(macOS 15, iOS 18, tvOS 18, *)
private struct BernMarkRenderer: TextRenderer {
    let color: Color

    func draw(layout: Text.Layout, in context: inout GraphicsContext) {
        for line in layout {
            let bounds = line.typographicBounds
            let rect = bounds.rect
            let em = bounds.ascent + bounds.descent
            let band = CGRect(
                x: rect.minX, y: rect.minY + rect.height * 0.74,
                width: rect.width + em * 0.4, height: rect.height * 0.24)
            context.fill(
                Path(roundedRect: band, cornerRadius: band.height / 2, style: .continuous),
                with: .color(color))
            context.draw(line)
        }
    }
}

// MARK: - Linked to scrolling and gestures

extension View {
    /// Follows scrolling: `scale` at the center of the scroll view around
    /// it, 1 at its edges; `opacity` at its edges, 1 at the center. The
    /// platform computes every frame; with reduced motion nothing changes.
    /// `anchor` is the point of the view the scale grows from.
    func bernScrollEffect(scale: Double, opacity: Double, anchor: UnitPoint = .center) -> some View {
        modifier(BernScrollEffect(scale: scale, opacity: opacity, anchor: anchor))
    }

    /// Scrolling comes to rest with an item at the start or the center.
    func bernSnap(center: Bool, axis: Axis) -> some View {
        modifier(BernSnap(center: center, axis: axis))
    }

    /// An item of a scroll view that comes to rest at its center: its size
    /// leaves room for the first and last item to reach the center.
    func bernSnapItem() -> some View {
        modifier(BernSnapItem())
    }

    /// `position` holds the key of the item the scroll view stands at:
    /// scrolling updates it, and setting it scrolls there with `motion`;
    /// with `arrival`, the item gives that nudge once it is there.
    func bernScrollPosition<ID: Hashable & Sendable>(
        _ position: Binding<ID>, anchor: UnitPoint, motion: BernMotion, arrival: BernMotion? = nil
    ) -> some View {
        modifier(BernScrollPosition(position: position, anchor: anchor, motion: motion, arrival: arrival))
    }

    /// A short nudge each time `when` turns true: the view swells a little
    /// and springs back with `motion`.
    func bernNudge(_ when: Bool, motion: BernMotion) -> some View {
        modifier(BernNudge(when: when, motion: motion))
    }

    /// Replay feedback on each event revision, even when the selection stays
    /// the same. A new event cancels the previous release; mounting is quiet.
    func bernNudge<Key: Equatable>(key: Key, scale: Double, motion: BernMotion) -> some View {
        modifier(BernKeyedNudge(key: key, peak: scale, motion: motion))
    }

    /// Pinching scales `value` within `range`; `commit` runs when the
    /// fingers lift. Assistive technologies adjust it in tenths.
    func bernPinch(
        _ value: Binding<Double>, in range: ClosedRange<Double>, commit: @escaping @MainActor () -> Void
    ) -> some View {
        modifier(BernPinch(value: value, range: range, commit: commit))
    }

    /// Dragging along `axis` moves `value` within `range`: up or right
    /// raises it, and `distance` points cross the whole range.
    func bernSwipe(
        _ value: Binding<Double>, in range: ClosedRange<Double>, axis: Axis, distance: Double,
        commit: @escaping @MainActor () -> Void
    ) -> some View {
        modifier(BernSwipe(value: value, range: range, axis: axis, distance: distance, commit: commit))
    }
}

private struct BernScrollEffect: ViewModifier {
    let scale: Double
    let opacity: Double
    let anchor: UnitPoint
    @BernReducedMotion private var reduce

    func body(content: Content) -> some View {
        if reduce || bernForcedReduceMotion {
            content
        } else {
            let (scale, opacity, anchor) = (scale, opacity, anchor)
            content.scrollTransition(.interactive.threshold(.centered)) { effect, phase in
                let near = 1 - min(abs(phase.value), 1)
                return effect
                    .scaleEffect(1 + (scale - 1) * near, anchor: anchor)
                    .opacity(opacity + (1 - opacity) * near)
            }
        }
    }
}

/// The items of a scroll view that comes to rest on them: their frames in
/// the content, read while the platform picks where a scroll stops (which
/// may be off the main thread), and the largest size, which sets the
/// margins that let the first and last item reach the center.
final class BernSnapItems: @unchecked Sendable {
    /// The largest item, which views follow.
    let largest: BernSnapLargest
    private let lock = NSLock()
    private var frames: [UUID: CGRect] = [:]

    @MainActor
    init() {
        largest = BernSnapLargest()
    }

    func report(_ id: UUID, _ frame: CGRect) {
        lock.withLock { frames[id] = frame }
    }

    func remove(_ id: UUID) {
        lock.withLock { frames[id] = nil }
    }

    /// Where along `axis` a scroll stopping at `start` comes to rest: with
    /// the item nearest the anchor at it.
    func rest(_ start: CGFloat, length: CGFloat, axis: Axis, center: Bool) -> CGFloat? {
        lock.withLock {
            let horizontal = axis == .horizontal
            // The target and the frames share the content's origin; the
            // margins lie before it.
            let point = { (frame: CGRect) -> CGFloat in
                let from = horizontal ? frame.minX : frame.minY
                let size = horizontal ? frame.width : frame.height
                return from + (center ? size / 2 : 0)
            }
            let anchor = start + (center ? length / 2 : 0)
            guard let nearest = frames.values.min(by: { abs(point($0) - anchor) < abs(point($1) - anchor) })
            else { return nil }
            return point(nearest) - (center ? length / 2 : 0)
        }
    }

}

@MainActor
@Observable
final class BernSnapLargest {
    private(set) var size: CGSize = .zero

    func grow(_ next: CGSize) {
        let grown = CGSize(width: max(size.width, next.width), height: max(size.height, next.height))
        if grown != size { size = grown }
    }
}

private struct BernSnapItemsKey: EnvironmentKey {
    static let defaultValue: BernSnapItems? = nil
}

extension EnvironmentValues {
    var bernSnapItems: BernSnapItems? {
        get { self[BernSnapItemsKey.self] }
        set { self[BernSnapItemsKey.self] = newValue }
    }
}

private let bernSnapSpace = "bern.snap"

/// Stops a scroll with the item nearest the anchor at it, wherever the
/// platform would have stopped it, so a fling still carries on to a later
/// item.
private struct BernSnapBehavior: ScrollTargetBehavior {
    let items: BernSnapItems
    let center: Bool
    let axis: Axis

    func updateTarget(_ target: inout ScrollTarget, context: TargetContext) {
        let horizontal = axis == .horizontal
        let start = horizontal ? target.rect.minX : target.rect.minY
        let length = horizontal ? target.rect.width : target.rect.height
        guard let rest = items.rest(start, length: length, axis: axis, center: center) else { return }
        if horizontal { target.rect.origin.x = rest } else { target.rect.origin.y = rest }
    }
}

private struct BernSnap: ViewModifier {
    let center: Bool
    let axis: Axis
    @State private var items = BernSnapItems()
    @State private var viewport: CGFloat = 0

    func body(content: Content) -> some View {
        let horizontal = axis == .horizontal
        let item = horizontal ? items.largest.size.width : items.largest.size.height
        let margin = center ? max(0, (viewport - item) / 2) : 0
        content
            .environment(\.bernSnapItems, items)
            .scrollTargetBehavior(BernSnapBehavior(items: items, center: center, axis: axis))
            .contentMargins(horizontal ? .horizontal : .vertical, margin, for: .scrollContent)
            .onGeometryChange(for: CGFloat.self) { horizontal ? $0.size.width : $0.size.height } action: {
                viewport = $0
            }
    }
}

private struct BernSnapItem: ViewModifier {
    @Environment(\.bernSnapItems) private var items
    @State private var id = UUID()

    func body(content: Content) -> some View {
        content
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(bernSnapSpace)) } action: { frame in
                items?.report(id, frame)
                items?.largest.grow(frame.size)
            }
            .onDisappear { items?.remove(id) }
    }
}

extension View {
    /// The content of a scroll view that comes to rest on its items: the
    /// space their frames are measured in.
    func bernSnapContent() -> some View {
        coordinateSpace(.named(bernSnapSpace))
    }
}

/// Where the keyed items of a scroll view are, in its visible area, and
/// which one is nearest its anchor.
@MainActor
@Observable
final class BernScrollItems {
    @ObservationIgnored private var frames: [AnyHashable: CGRect] = [:]
    @ObservationIgnored private let accepts: (AnyHashable) -> Bool
    @ObservationIgnored private var settling = false

    init<ID: Hashable>(keyType: ID.Type) {
        accepts = { $0.base is ID }
    }

    func tracks(_ key: AnyHashable) -> Bool { accepts(key) }

    @ObservationIgnored var anchor = UnitPoint.center
    @ObservationIgnored var viewport = CGSize.zero
    private(set) var nearest: AnyHashable?
    /// The nudge an item gives when setting the position scrolled to it.
    @ObservationIgnored var arrival: BernMotion?
    /// The item the position last scrolled to, and how many arrivals there
    /// have been.
    private(set) var arrived: AnyHashable?
    private(set) var arrivals = 0
    /// How many scrolls to a set position have come to rest.
    private(set) var stops = 0
    /// The item being scrolled to; it arrives once it has stopped moving.
    @ObservationIgnored private var coming: AnyHashable?
    @ObservationIgnored private var nudges = false
    @ObservationIgnored private var watch = 0

    /// Setting the position scrolls to `key`: it arrives once it stops,
    /// with a nudge unless it is where the view starts.
    func expect(_ key: AnyHashable, nudge: Bool = true) {
        coming = key
        nudges = nudge
        wait()
    }

    /// Set while the position clears itself to scroll again to the same
    /// item, which is not the person scrolling.
    @ObservationIgnored var clearing = false
    /// The value the person's scrolling last wrote to the position; when it
    /// comes back as a change, there is nothing to scroll to.
    @ObservationIgnored var followed: AnyHashable?

    /// The person took over the scrolling; nothing arrives.
    func cancelArrival() {
        if !clearing { coming = nil }
    }

    private func wait() {
        watch += 1
        let current = watch
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(150))
            guard let self, current == self.watch, let key = self.coming else { return }
            self.coming = nil
            self.stops += 1
            if self.nudges {
                self.arrived = key
                self.arrivals += 1
            }
        }
    }

    func report(_ key: AnyHashable, _ frame: CGRect) {
        guard tracks(key), frames[key] != frame else { return }
        frames[key] = frame
        scheduleSettle()
        if key == coming { wait() }
    }

    func remove(_ key: AnyHashable) {
        guard frames.removeValue(forKey: key) != nil else { return }
        scheduleSettle()
    }

    /// Geometry arrives once per item. Compare the completed batch once,
    /// rather than scanning all items and publishing intermediate nearest
    /// positions after every individual report.
    private func scheduleSettle() {
        guard !settling else { return }
        settling = true
        Task { @MainActor [weak self] in
            guard let self else { return }
            self.settling = false
            self.settle()
        }
    }

    func settle() {
        let point = CGPoint(x: viewport.width * anchor.x, y: viewport.height * anchor.y)
        let distance = { (frame: CGRect) -> CGFloat in
            let x = frame.minX + frame.width * self.anchor.x - point.x
            let y = frame.minY + frame.height * self.anchor.y - point.y
            return x * x + y * y
        }
        let found = frames.min { distance($0.value) < distance($1.value) }?.key
        if found != nearest { nearest = found }
    }
}

private struct BernScrollItemsKey: EnvironmentKey {
    static let defaultValue: BernScrollItems? = nil
}

extension EnvironmentValues {
    var bernScrollItems: BernScrollItems? {
        get { self[BernScrollItemsKey.self] }
        set { self[BernScrollItemsKey.self] = newValue }
    }
}

extension View {
    /// A keyed item of a scroll view whose `position` is bound: it reports
    /// where it is.
    func bernScrollItem(_ key: some Hashable & Sendable) -> some View {
        modifier(BernScrollItem(key: AnyHashable(key)))
    }
}

private struct BernScrollItem: ViewModifier {
    let key: AnyHashable
    @Environment(\.bernScrollItems) private var items

    func body(content: Content) -> some View {
        let tracked = items?.tracks(key) == true
        return content
            .modifier(
                BernNudgeEffect(
                    count: items?.arrived == key ? items?.arrivals ?? 0 : 0,
                    motion: items?.arrival))
            .onGeometryChange(for: CGRect.self) {
                tracked ? $0.frame(in: .named(bernScrollSpace)) : .null
            } action: { frame in
                items?.report(key, frame)
            }
            .onDisappear { items?.remove(key) }
    }
}

private let bernScrollSpace = "bern.scroll"

/// Content that opens with the point at `at` of it in the middle of its
/// scroll view, or as near the middle as the content reaches. A probe in
/// its background finds the platform scroll view around it and sets the
/// offset once, when the content has its size: `ScrollViewReader` moved
/// to marks inside a background unreliably.
struct BernOpenCenter<Content: View>: View {
    let at: Double
    let horizontal: Bool
    @ViewBuilder let content: () -> Content

    var body: some View {
        content()
            .background(BernOpenCenterProbe(at: min(max(at, 0), 1), horizontal: horizontal))
    }
}

#if os(macOS)
private struct BernOpenCenterProbe: NSViewRepresentable {
    let at: Double
    let horizontal: Bool
    func makeNSView(context: Context) -> BernOpenCenterView {
        BernOpenCenterView(at: at, horizontal: horizontal)
    }
    func updateNSView(_ view: BernOpenCenterView, context: Context) {
        view.at = at
    }
}

final class BernOpenCenterView: NSView {
    var at: Double
    let horizontal: Bool
    private var opened = false
    private var tries = 0

    init(at: Double, horizontal: Bool) {
        self.at = at
        self.horizontal = horizontal
        super.init(frame: .zero)
    }
    required init?(coder: NSCoder) { nil }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil { open() }
    }

    /// Until the scroll view has measured its content, try again shortly.
    private func open() {
        guard !opened, window != nil else { return }
        guard let scroll = enclosingScrollView, let document = scroll.documentView else { return retry() }
        let clip = scroll.contentView.bounds
        let length = horizontal ? document.frame.width : document.frame.height
        let port = horizontal ? clip.width : clip.height
        guard length > 0, port > 0 else { return retry() }
        opened = true
        guard length > port else { return }
        let middle = min(max(at * length - port / 2, 0), length - port)
        scroll.contentView.scroll(to: horizontal ? NSPoint(x: middle, y: clip.origin.y) : NSPoint(x: clip.origin.x, y: middle))
        scroll.reflectScrolledClipView(scroll.contentView)
    }

    private func retry() {
        tries += 1
        guard tries < 40 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.open() }
    }
}
#else
private struct BernOpenCenterProbe: UIViewRepresentable {
    let at: Double
    let horizontal: Bool
    func makeUIView(context: Context) -> BernOpenCenterView {
        BernOpenCenterView(at: at, horizontal: horizontal)
    }
    func updateUIView(_ view: BernOpenCenterView, context: Context) {
        view.at = at
    }
}

final class BernOpenCenterView: UIView {
    var at: Double
    let horizontal: Bool
    private var opened = false
    private var tries = 0

    init(at: Double, horizontal: Bool) {
        self.at = at
        self.horizontal = horizontal
        super.init(frame: .zero)
        isUserInteractionEnabled = false
    }
    required init?(coder: NSCoder) { nil }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil { open() }
    }

    /// Until the scroll view has measured its content, try again shortly.
    private func open() {
        guard !opened, window != nil else { return }
        let scroll = sequence(first: superview, next: { $0?.superview })
            .lazy.compactMap { $0 as? UIScrollView }.first
        guard let scroll else { return retry() }
        let inset = scroll.adjustedContentInset
        let length = horizontal ? scroll.contentSize.width : scroll.contentSize.height
        let port = horizontal
            ? scroll.bounds.width - inset.left - inset.right
            : scroll.bounds.height - inset.top - inset.bottom
        guard length > 0, port > 0 else { return retry() }
        opened = true
        guard length > port else { return }
        let middle = min(max(at * length - port / 2, 0), length - port)
        var offset = scroll.contentOffset
        if horizontal { offset.x = middle - inset.left } else { offset.y = middle - inset.top }
        scroll.setContentOffset(offset, animated: false)
    }

    private func retry() {
        tries += 1
        guard tries < 40 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.open() }
    }
}
#endif

/// Scrolling sets `position` to the item nearest the anchor; setting
/// `position` scrolls to that item. The platform's own scroll position
/// moves the view, but does not tell which item the person scrolled to:
/// it only clears itself once the person scrolls, which is when the item
/// nearest the anchor starts to count.
private struct BernScrollPosition<ID: Hashable & Sendable>: ViewModifier {
    let position: Binding<ID>
    let anchor: UnitPoint
    let motion: BernMotion
    let arrival: BernMotion?
    @State private var target: ID?
    /// Set once the view stands at the first position, after its first
    /// layout has measured the margins.
    @State private var placed = false
    @State private var items = BernScrollItems(keyType: ID.self)
    @BernReducedMotion private var reduce

    func body(content: Content) -> some View {
        content
            .environment(\.bernScrollItems, items)
            .coordinateSpace(.named(bernScrollSpace))
            .scrollPosition(id: $target, anchor: anchor)
            .onGeometryChange(for: CGSize.self) { $0.size } action: { size in
                items.anchor = anchor
                items.arrival = arrival
                items.viewport = size
                items.settle()
            }
            .onAppear {
                items.expect(AnyHashable(position.wrappedValue), nudge: false)
                target = position.wrappedValue
                placed = true
            }
            // Once there, the platform's position lets go, so that the
            // person's next scroll comes to rest on an item as any other.
            .onChange(of: items.stops) { _, _ in
                items.clearing = true
                target = nil
                Task { @MainActor in items.clearing = false }
            }
            .onChange(of: position.wrappedValue) { _, wanted in
                if AnyHashable(wanted) == items.followed {
                    items.followed = nil
                    return
                }
                guard wanted != target, target != nil || AnyHashable(wanted) != items.nearest else { return }
                scroll(to: wanted)
            }
            .onChange(of: items.nearest) { _, _ in follow() }
            .onChange(of: target) { _, current in
                if current == nil { items.cancelArrival() }
                follow()
            }
    }

    /// While the person scrolls, `position` follows the item nearest the
    /// anchor.
    private func follow() {
        guard placed, target == nil, let nearest = items.nearest?.base as? ID,
            nearest != position.wrappedValue
        else { return }
        items.followed = AnyHashable(nearest)
        position.wrappedValue = nearest
    }

    private func scroll(to wanted: ID) {
        let animation = reduce || bernForcedReduceMotion ? nil : motion.animation
        let items = items
        let go = {
            items.clearing = false
            items.expect(AnyHashable(wanted))
            withAnimation(animation) { target = wanted }
        }
        if target == wanted {
            // The platform only scrolls when its position changes.
            items.clearing = true
            target = nil
            Task { @MainActor in go() }
        } else {
            go()
        }
    }
}

private struct BernKeyedNudge<Key: Equatable>: ViewModifier {
    let key: Key
    let peak: Double
    let motion: BernMotion
    @State private var mounted = false
    @State private var scale = 1.0
    @BernReducedMotion private var reduce

    func body(content: Content) -> some View {
        content
            .scaleEffect(reduce || bernForcedReduceMotion ? 1 : scale)
            .task(id: key) {
                guard mounted else { mounted = true; return }
                guard !(reduce || bernForcedReduceMotion) else { scale = 1; return }
                withAnimation(.easeOut(duration: 0.09)) { scale = peak }
                do { try await Task.sleep(for: .seconds(0.09)) } catch { return }
                guard !Task.isCancelled else { return }
                withAnimation(motion.animation) { scale = 1 }
            }
    }
}

private struct BernNudge: ViewModifier {
    let when: Bool
    let motion: BernMotion
    @State private var count = 0

    func body(content: Content) -> some View {
        content
            .modifier(BernNudgeEffect(count: count, motion: motion))
            .onChange(of: when) { _, now in
                if now { count += 1 }
            }
    }
}

/// Swells a little and springs back with `motion` each time `count` grows;
/// with reduced motion it stays still.
private struct BernNudgeEffect: ViewModifier {
    let count: Int
    let motion: BernMotion?
    @State private var scale = 1.0
    @BernReducedMotion private var reduce

    func body(content: Content) -> some View {
        content
            .scaleEffect(scale)
            .onChange(of: count) { old, new in
                guard new > old, let motion, !(reduce || bernForcedReduceMotion) else { return }
                withAnimation(.easeOut(duration: 0.08)) { scale = 1.06 } completion: {
                    withAnimation(motion.animation) { scale = 1 }
                }
            }
    }
}

private struct BernPinch: ViewModifier {
    let value: Binding<Double>
    let range: ClosedRange<Double>
    let commit: @MainActor () -> Void
    @State private var start: Double?

    func body(content: Content) -> some View {
        #if os(tvOS)
        // A television has no touch screen to pinch: only the adjustment
        // of assistive technologies.
        content
            .accessibilityAdjustableAction { direction in bernAdjust(value, range, direction, commit) }
        #else
        content
            .contentShape(Rectangle())
            .simultaneousGesture(
                MagnifyGesture()
                    .onChanged { gesture in
                        let from = start ?? value.wrappedValue
                        start = from
                        value.wrappedValue = min(max(from * gesture.magnification, range.lowerBound), range.upperBound)
                    }
                    .onEnded { _ in
                        start = nil
                        commit()
                    })
            .accessibilityAdjustableAction { direction in bernAdjust(value, range, direction, commit) }
        #endif
    }
}

private struct BernSwipe: ViewModifier {
    let value: Binding<Double>
    let range: ClosedRange<Double>
    let axis: Axis
    let distance: Double
    let commit: @MainActor () -> Void
    @State private var start: Double?

    func body(content: Content) -> some View {
        #if os(tvOS)
        // A television has no touch screen to swipe; the remote's swipes
        // move the focus. Only the adjustment of assistive technologies.
        content
            .accessibilityAdjustableAction { direction in bernAdjust(value, range, direction, commit) }
        #else
        content
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 10)
                    .onChanged { drag in
                        let from = start ?? value.wrappedValue
                        start = from
                        let moved = axis == .vertical ? -drag.translation.height : drag.translation.width
                        let span = range.upperBound - range.lowerBound
                        value.wrappedValue = min(max(from + moved / distance * span, range.lowerBound), range.upperBound)
                    }
                    .onEnded { _ in
                        start = nil
                        commit()
                    })
            .accessibilityAdjustableAction { direction in bernAdjust(value, range, direction, commit) }
        #endif
    }
}

/// One step of an adjustable value for assistive technologies: a tenth of
/// its range.
@MainActor
private func bernAdjust(
    _ value: Binding<Double>, _ range: ClosedRange<Double>, _ direction: AccessibilityAdjustmentDirection,
    _ commit: @MainActor () -> Void
) {
    let step = (range.upperBound - range.lowerBound) / 10
    switch direction {
    case .increment: value.wrappedValue = min(value.wrappedValue + step, range.upperBound)
    case .decrement: value.wrappedValue = max(value.wrappedValue - step, range.lowerBound)
    @unknown default: return
    }
    commit()
}

// MARK: - Stagger

/// How an element appears, for a staggered entrance that runs by hand: a
/// container's own appearing takes its children with it, so their
/// transitions would not play.
enum BernAppear: Sendable {
    case fade
    case pop(Double)
    case slide(x: Double, y: Double)
    case grow(Edge)

    var grows: Bool {
        if case .grow = self { return true }
        return false
    }

    var scale: Double {
        if case .pop(let from) = self { return from }
        return 1
    }

    var offset: CGSize {
        if case .slide(let x, let y) = self { return CGSize(width: x, height: y) }
        return .zero
    }
}

/// Only the axis perpendicular to the fixed edge grows; its other axis and
/// opacity stay unchanged, so a thin rule extends without swelling sideways.
private struct BernGrow: AnimatableModifier {
    let edge: Edge
    nonisolated var progress: CGFloat
    @Environment(\.layoutDirection) private var direction

    nonisolated var animatableData: CGFloat {
        get { progress }
        set { progress = newValue }
    }

    func body(content: Content) -> some View {
        let horizontal = edge == .leading || edge == .trailing
        let anchor: UnitPoint = switch edge {
        case .top: .top
        case .bottom: .bottom
        case .leading: direction == .rightToLeft ? .trailing : .leading
        case .trailing: direction == .rightToLeft ? .leading : .trailing
        }
        content.scaleEffect(x: horizontal ? progress : 1, y: horizontal ? 1 : progress, anchor: anchor)
    }
}

extension AnyTransition {
    static func bernGrow(edge: Edge) -> AnyTransition {
        .modifier(active: BernGrow(edge: edge, progress: 0), identity: BernGrow(edge: edge, progress: 1))
    }
}

private struct BernEntranceLook: ViewModifier {
    let appear: BernAppear
    let shown: Bool

    @ViewBuilder func body(content: Content) -> some View {
        if case .grow(let edge) = appear {
            content.modifier(BernGrow(edge: edge, progress: shown ? 1 : 0))
        } else {
            content
                .opacity(shown ? 1 : 0)
                .scaleEffect(shown ? 1 : appear.scale)
                .offset(shown ? .zero : appear.offset)
        }
    }
}

/// The turns of a staggering container's children: those that appear in
/// one update take turns in reading order, top to bottom and along each
/// line in the writing direction, each `step` seconds after the one
/// before; the count starts over with the next update.
@MainActor
final class BernStaggerClock {
    let step: Double
    private var waiting: [(place: BernPlace, start: (Double) -> Void)] = []
    private var rightToLeft = false

    init(step: Double) {
        self.step = step
    }

    /// Asks for a turn: `start` runs with its delay once every element
    /// appearing in this update has asked and been laid out.
    func take(place: BernPlace, rightToLeft: Bool, start: @escaping (Double) -> Void) {
        waiting.append((place, start))
        self.rightToLeft = rightToLeft
        if waiting.count == 1 {
            DispatchQueue.main.async { [self] in turn() }
        }
    }

    private func turn() {
        let rtl = rightToLeft
        let frames = waiting.map(\.place.frame)
        let order = waiting.indices.sorted { a, b in
            let one = frames[a], other = frames[b]
            // On one line when they overlap by half the shorter height.
            let overlap = min(one.maxY, other.maxY) - max(one.minY, other.minY)
            if overlap < min(one.height, other.height) / 2 {
                return one.minY < other.minY
            }
            if one.minX != other.minX {
                return rtl ? one.maxX > other.maxX : one.minX < other.minX
            }
            return a < b
        }
        let turns = waiting
        waiting = []
        for (place, index) in order.enumerated() {
            turns[index].start(Double(place) * step * bernMotionScale)
        }
    }
}

private struct BernStaggerKey: EnvironmentKey {
    static let defaultValue: BernStaggerClock? = nil
}

extension EnvironmentValues {
    var bernStagger: BernStaggerClock? {
        get { self[BernStaggerKey.self] }
        set { self[BernStaggerKey.self] = newValue }
    }
}

/// A container whose children take turns appearing.
struct BernStaggerScope<Content: View>: View {
    @State private var clock: BernStaggerClock
    let content: Content

    init(step: Double, @ViewBuilder content: () -> Content) {
        _clock = State(initialValue: BernStaggerClock(step: step))
        self.content = content()
    }

    var body: some View {
        content.environment(\.bernStagger, clock)
    }
}

/// A staggered or explicitly delayed entrance. Explicit delays and edge
/// growth also run on first mount, after layout. Waiting belongs to the view's
/// task, so leaving a page cancels it. Ordinary updates do not replay it.
struct BernEntrance: ViewModifier {
    let appear: BernAppear
    let motion: BernMotion?
    let clock: BernStaggerClock?
    let delay: Double?
    @Environment(BernWindowContext.self) private var window
    @BernReducedMotion private var reduce
    @Environment(\.layoutDirection) private var direction
    @State private var shown = false
    @State private var place = BernPlace()

    func body(content: Content) -> some View {
        let measuring = !shown
        return content
            .environment(\.bernStagger, nil)
            .onGeometryChange(for: CGRect.self) { measuring ? $0.frame(in: .global) : .zero } action: {
                if !shown { place.frame = $0 }
            }
            .modifier(BernEntranceLook(appear: appear, shown: shown || reduce || bernForcedReduceMotion))
            .task(id: window.settled) {
                guard !shown else { return }
                if reduce || bernForcedReduceMotion || (!window.settled && delay == nil && !appear.grows) {
                    var transaction = Transaction()
                    transaction.disablesAnimations = true
                    withTransaction(transaction) { shown = true }
                    return
                }
                guard window.settled else { return }
                let turn: Double
                if let clock {
                    turn = await withCheckedContinuation { continuation in
                        clock.take(place: place, rightToLeft: direction == .rightToLeft) { wait in
                            continuation.resume(returning: wait)
                        }
                    }
                } else {
                    turn = 0
                }
                do {
                    try await Task.sleep(for: .seconds(turn + (delay ?? 0) * bernMotionScale))
                    try Task.checkCancellation()
                } catch { return }
                withAnimation(motion?.animation ?? .default) { shown = true }
            }
    }
}

/// Where an element stands, kept without redrawing it.
@MainActor
final class BernPlace {
    var frame = CGRect.zero
}

// MARK: - Accordion

/// A vector lets an interrupted selection retain every column's current
/// width and velocity, instead of starting independent position animations.
// A preview measures its room before constructing any rows. Clipping a full
// stack would still start image requests for the omitted items.
enum BernPreviewGeometry {
    static func visible(total: Int, height: CGFloat, item: CGFloat, gap: CGFloat, more: CGFloat) -> Int {
        guard total > 0, height.isFinite, height > 0, item.isFinite, item > 0 else { return 0 }
        let gap = max(0, gap), more = max(0, more)
        if CGFloat(total) * item + CGFloat(total - 1) * gap <= height { return total }
        return min(total, max(0, Int(floor(max(0, height - more) / (item + gap)))))
    }
}

struct BernPreviewList<Item, ID: Hashable, Content: View>: View {
    let items: [Item]
    let id: KeyPath<Item, ID>
    let itemHeight: CGFloat
    let gap: CGFloat
    let moreHeight: CGFloat
    @ViewBuilder let content: (Item) -> Content

    var body: some View {
        GeometryReader { geometry in
            let count = BernPreviewGeometry.visible(total: items.count, height: geometry.size.height,
                                                   item: itemHeight, gap: gap, more: moreHeight)
            VStack(spacing: max(0, gap)) {
                ForEach(Array(items.prefix(count)), id: id) { item in
                    content(item).frame(height: itemHeight)
                }
                if count < items.count && geometry.size.height >= moreHeight {
                    Text("+\(items.count - count)")
                        .font(.caption).foregroundStyle(.secondary)
                        .padding(.horizontal, 6).frame(height: max(0, moreHeight))
                        .background(.quaternary, in: Capsule())
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        }
        .clipped()
    }
}

struct BernAccordionWeights: VectorArithmetic {
    var values: [Double]
    static let zero = Self(values: [])
    static func + (lhs: Self, rhs: Self) -> Self {
        Self(values: (0..<max(lhs.values.count, rhs.values.count)).map {
            (lhs.values.indices.contains($0) ? lhs.values[$0] : 0)
                + (rhs.values.indices.contains($0) ? rhs.values[$0] : 0)
        })
    }
    static func - (lhs: Self, rhs: Self) -> Self {
        var negative = rhs
        negative.scale(by: -1)
        return lhs + negative
    }
    mutating func scale(by rhs: Double) { values = values.map { $0 * rhs } }
    var magnitudeSquared: Double { values.reduce(0) { $0 + $1 * $1 } }
}

enum BernAccordionGeometry {
    static func widths(total: Double, collapsedWidth: Double, gap: Double, weights: [Double]) -> [Double] {
        guard !weights.isEmpty else { return [] }
        let available = max(0, total - max(0, gap) * Double(weights.count - 1))
        let collapsed = min(max(0, collapsedWidth), available / Double(weights.count))
        let extra = max(0, available - collapsed * Double(weights.count))
        let widths = weights.map { max(0, collapsed + extra * $0) }
        let sum = widths.reduce(0, +)
        // Large spring overshoots can hit zero. Renormalize so the row still
        // fits its bounds and never allocates negative or overlapping frames.
        return sum > 0 ? widths.map { $0 * available / sum } : widths
    }
}

private struct BernAccordionLayout: Layout {
    var weights: BernAccordionWeights
    let collapsedWidth: CGFloat
    let gap: CGFloat
    let height: CGFloat?
    var animatableData: BernAccordionWeights {
        get { weights }
        set { weights = newValue }
    }
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let proposedHeight = proposal.height.flatMap { $0.isFinite ? $0 : nil }
        return CGSize(width: proposal.width ?? (collapsedWidth * CGFloat(subviews.count) + gap * CGFloat(max(0, subviews.count - 1))), height: max(0, height ?? proposedHeight ?? 560))
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let current = subviews.indices.map { weights.values.indices.contains($0) ? weights.values[$0] : 0 }
        let widths = BernAccordionGeometry.widths(total: bounds.width, collapsedWidth: collapsedWidth, gap: gap, weights: current)
        var x = bounds.minX
        for index in subviews.indices {
            subviews[index].place(at: CGPoint(x: x, y: bounds.minY), anchor: .topLeading,
                                 proposal: ProposedViewSize(width: widths[index], height: bounds.height))
            x += widths[index] + gap
        }
    }
}

struct BernAccordionCell<Content: View>: View {
    let corner: CGFloat
    @ViewBuilder let content: Content
    var body: some View {
        GeometryReader { geometry in
            content
                .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
                .clipShape(RoundedRectangle(cornerRadius: max(0, corner), style: .continuous))
        }
    }
}

struct BernAccordion<Content: View>: View {
    let count: Int
    let selected: Int
    let collapsedWidth: CGFloat
    let gap: CGFloat
    let height: CGFloat?
    let motion: BernMotion?
    @ViewBuilder let content: Content
    @BernReducedMotion private var reduce
    var body: some View {
        let count = max(0, count)
        let selection = min(max(0, selected), max(0, count - 1))
        let weights = BernAccordionWeights(values: (0..<count).map { $0 == selection ? 1 : 0 })
        BernAccordionLayout(weights: weights, collapsedWidth: max(0, collapsedWidth), gap: max(0, gap), height: height.map { max(0, $0) }) {
            // Width belongs to the layout. Switching a column's content must
            // not create a second, independently animated sizing transition.
            content.animation(nil, value: selected)
        }
        .frame(maxWidth: .infinity, minHeight: height == nil ? 0 : nil, maxHeight: height == nil ? .infinity : nil)
        .animation(reduce || bernForcedReduceMotion ? nil : motion?.animation, value: weights)
    }
}

// MARK: - Grids

/// A minimum column width selects a column count. Changes within the same
/// count do not change the layout or restart its spring during a pinch.
struct BernAdaptiveGrid<Content: View>: View {
    let minimum: CGFloat
    let gap: CGFloat
    let rowGap: CGFloat
    let motion: BernMotion?
    /// With a ratio, every row is the column width ÷ the ratio + `cellExtra`
    /// tall, whatever its items.
    var cellRatio: Double? = nil
    var cellExtra: Double = 0
    @ViewBuilder let content: Content
    @State private var width: CGFloat = 0
    @BernReducedMotion private var reduce

    var body: some View {
        let columns = max(1, Int((width + gap) / max(1, minimum + gap)))
        let column = max(0, (width - gap * CGFloat(columns - 1)) / CGFloat(columns))
        LazyVGrid(
            columns: Array(repeating: GridItem(.flexible(minimum: 0), spacing: gap, alignment: .top), count: columns),
            alignment: .leading, spacing: rowGap
        ) {
            content
        }
        .environment(\.bernGridCell, cellRatio.map { column / CGFloat($0) + CGFloat(cellExtra) })
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(reduce || bernForcedReduceMotion ? nil : motion?.animation,
                   value: [CGFloat(columns), gap, rowGap])
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { measured in
            guard measured.isFinite, measured != width else { return }
            if width == 0 {
                var transaction = Transaction()
                transaction.disablesAnimations = true
                withTransaction(transaction) { width = measured }
            } else {
                width = measured
            }
        }
    }
}

private struct BernGridCellKey: EnvironmentKey {
    static let defaultValue: CGFloat? = nil
}

extension EnvironmentValues {
    /// The height of a cell of the grid around, when its rows are fixed.
    var bernGridCell: CGFloat? {
        get { self[BernGridCellKey.self] }
        set { self[BernGridCellKey.self] = newValue }
    }
}

private struct BernGridCell: ViewModifier {
    @Environment(\.bernGridCell) private var height
    func body(content: Content) -> some View {
        if let height {
            // The item keeps its own height, the cell its: taller content
            // runs into the gap instead of being squeezed.
            content.fixedSize(horizontal: false, vertical: true)
                .frame(height: height, alignment: .top)
        } else {
            content
        }
    }
}

extension View {
    /// An item of a grid whose rows are fixed: as tall as the cell, at its top.
    func bernGridCell() -> some View { modifier(BernGridCell()) }
}

// MARK: - Rails

private struct BernRailAt: LayoutValueKey {
    static let defaultValue: Double? = nil
}

private struct BernRailSpan: LayoutValueKey {
    static let defaultValue: ClosedRange<Double>? = nil
}

/// A strip along an axis that places each child at a fraction of its
/// length, or between two; across it, children are centered, and those
/// between two fractions without a thickness of their own fill it. The
/// rail is as thick as its thickest child.
struct BernRail: Layout {
    let vertical: Bool
    /// Where a placed child's own extent meets its point: 0 its start, 0.5
    /// its center, 1 its end.
    let anchor: Double
    /// How far the scale runs past each end of the rail.
    var overhang: Double = 0

    /// A span is offered the thickness the rail is given; an item its own
    /// size.
    private func size(of subview: LayoutSubview, across: CGFloat?) -> CGSize {
        guard subview[BernRailSpan.self] != nil, let across, across.isFinite else {
            return subview.sizeThatFits(.unspecified)
        }
        return subview.sizeThatFits(
            vertical ? ProposedViewSize(width: across, height: nil) : ProposedViewSize(width: nil, height: across))
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let across = vertical ? proposal.width : proposal.height
        var thickness: CGFloat = 0
        for subview in subviews {
            let size = size(of: subview, across: across)
            thickness = max(thickness, vertical ? size.width : size.height)
        }
        let length = (vertical ? proposal.height : proposal.width) ?? 0
        let finite = length.isFinite ? length : 0
        return vertical ? CGSize(width: thickness, height: finite) : CGSize(width: finite, height: thickness)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        // The scale: the rail's length and the overhang past both ends.
        let past = CGFloat(overhang)
        let length = (vertical ? bounds.height : bounds.width) + 2 * past
        let thickness = vertical ? bounds.width : bounds.height
        let clamp = { (value: Double) in CGFloat(min(1, max(0, value))) }
        for subview in subviews {
            if let span = subview[BernRailSpan.self] {
                let start = clamp(span.lowerBound) * length - past
                let extent = (clamp(span.upperBound) - clamp(span.lowerBound)) * length
                let offer = vertical ? CGSize(width: thickness, height: extent) : CGSize(width: extent, height: thickness)
                let own = subview.sizeThatFits(ProposedViewSize(offer))
                let across = (thickness - min(thickness, vertical ? own.width : own.height)) / 2
                let size = vertical
                    ? CGSize(width: min(thickness, own.width), height: extent)
                    : CGSize(width: extent, height: min(thickness, own.height))
                let origin = vertical
                    ? CGPoint(x: bounds.minX + across, y: bounds.minY + start)
                    : CGPoint(x: bounds.minX + start, y: bounds.minY + across)
                subview.place(at: origin, anchor: .topLeading, proposal: ProposedViewSize(size))
            } else {
                let size = subview.sizeThatFits(.unspecified)
                let point = clamp(subview[BernRailAt.self] ?? 0) * length - past
                let along = point - CGFloat(anchor) * (vertical ? size.height : size.width)
                let across = (thickness - (vertical ? size.width : size.height)) / 2
                let origin = vertical
                    ? CGPoint(x: bounds.minX + across, y: bounds.minY + along)
                    : CGPoint(x: bounds.minX + along, y: bounds.minY + across)
                subview.place(at: origin, anchor: .topLeading, proposal: ProposedViewSize(size))
            }
        }
    }
}

private struct BernFillsLayers: LayoutValueKey {
    static let defaultValue = false
}

/// A `Stack` holding a rail or a canvas without a size: its layers paint in
/// order, as in a `ZStack`, but such a layer takes the size the other
/// layers give the stack instead of adding to it, a backdrop. With only
/// such layers, the stack is as large as the largest.
struct BernLayers: Layout {
    let alignment: UnitPoint
    /// The stack's own size, where it gives one: its backdrops take it.
    var width: Double? = nil
    var height: Double? = nil
    /// With a backdrop, the stack fills the width it is offered, as a block
    /// does across a column, and keeps the height its other layers give.
    var fillsWidth = false

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let sized = subviews.filter { !$0[BernFillsLayers.self] }
        let fits = (sized.isEmpty ? Array(subviews) : sized).reduce(CGSize.zero) { (size: CGSize, subview: LayoutSubview) -> CGSize in
            let own = subview.sizeThatFits(proposal)
            return CGSize(width: max(size.width, own.width), height: max(size.height, own.height))
        }
        let offered = fillsWidth ? proposal.width.flatMap { $0.isFinite ? $0 : nil } : nil
        return CGSize(width: width.map { CGFloat($0) } ?? offered ?? fits.width, height: height.map { CGFloat($0) } ?? fits.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for subview in subviews {
            let size = subview.sizeThatFits(
                subview[BernFillsLayers.self] ? ProposedViewSize(bounds.size) : proposal)
            let origin = CGPoint(
                x: bounds.minX + (bounds.width - size.width) * alignment.x,
                y: bounds.minY + (bounds.height - size.height) * alignment.y)
            subview.place(at: origin, anchor: .topLeading, proposal: ProposedViewSize(size))
        }
    }
}

extension View {
    /// In a `BernLayers`, takes the size of the other layers.
    func bernFillsLayers() -> some View {
        layoutValue(key: BernFillsLayers.self, value: true)
    }

    /// Sits at `fraction` of the rail it stands in, moving with `motion`.
    func bernRailAt(_ fraction: Double, motion: BernMotion?) -> some View {
        layoutValue(key: BernRailAt.self, value: fraction)
            .animation(motion?.animation, value: fraction)
    }

    /// Spans the rail it stands in from one fraction to another.
    func bernRailSpan(from: Double, to: Double, motion: BernMotion?) -> some View {
        layoutValue(key: BernRailSpan.self, value: from...max(from, to))
            .animation(motion?.animation, value: [from, to])
    }
}

// MARK: - Popover buttons

extension View {
    /// Leave an unstyled macOS popover's surface to the system, including
    /// Liquid Glass and its arrow. A custom presentation background replaces
    /// that material, so apply it only when the app supplies a fill.
    @ViewBuilder func bernPopoverBackground(_ background: BernColor?, scheme: ColorScheme) -> some View {
        if let background {
            presentationBackground {
                Rectangle().fill(background.color).environment(\.colorScheme, scheme)
            }
        } else {
            #if os(macOS)
            preferredColorScheme(scheme)
            #else
            presentationBackground {
                Rectangle().fill(.regularMaterial).environment(\.colorScheme, scheme)
            }
            #endif
        }
    }
}

/// A button that shows its content in a popover, such as a volume slider.
/// The content exists while the popover shows. `presented`, when given,
/// opens and closes it and follows it; external entries open it with
/// `activate`. With an icon only the icon shows.
struct BernPopoverButton<Content: View>: View {
    let label: String
    let systemImage: String?
    let presented: Binding<Bool>?
    let target: String
    var windowTheme = false
    var style: BernStyle? = nil
    @ViewBuilder let content: () -> Content
    @Environment(\.colorScheme) private var scheme
    @Environment(\.bernWindowScheme) private var windowScheme
    @State private var local = false

    private var isPresented: Binding<Bool> {
        presented ?? $local
    }

    var body: some View {
        let showing = isPresented
        let panelScheme = windowTheme ? (windowScheme ?? scheme) : scheme
        Button {
            showing.wrappedValue.toggle()
        } label: {
            if let systemImage {
                Label(label, systemImage: systemImage).labelStyle(.iconOnly)
            } else {
                Text(verbatim: label)
            }
        }
        .help(label)
        .bernPopover(isPresented: showing) {
            VStack(alignment: .leading, spacing: 12) { content() }
                .padding(16)
                .frame(minWidth: 240, alignment: .leading)
                .presentationCompactAdaptation(.popover)
                .bernPopoverBackground(style?.base.background, scheme: panelScheme)
                .foregroundStyle(style?.base.foreground?.color ?? Color.primary)
                .environment(\.colorScheme, panelScheme)
                .environment(\.bernControlGroupHeight, nil)
        }
        .bernControl(
            target, role: "popover_button", label: label,
            actions: BernControlActions(
                activate: { showing.wrappedValue = true },
                value: { showing.wrappedValue ? "open" : "closed" }))
    }
}

// MARK: - Window search palette
/// One model per window, shared by the toolbar trigger and the overlay.
@MainActor @Observable final class BernPaletteState {
    var presented = false
    var expanded = false
    var target = ""
    var prompt = ""
    var text: Binding<String> = .constant("")
    var results: AnyView = AnyView(EmptyView())
    var source = CGRect.zero
    var detail = CGRect.zero
    var width: CGFloat = 640
    var height: CGFloat = 480
    var resultHeight: CGFloat = 420
    var selection: String?
    var navigationRevision = 0
    var entries: [BernPaletteEntry] = []
    var restoreFocus: (() -> Void)?
    var reduceMotion = false
    var closing = false
    /// The results of a search field in the page, not an overlay: choosing
    /// one runs it at once.
    var inline = false

    init(inline: Bool = false) { self.inline = inline }

    func open() {
        guard !presented else { return }
        text.wrappedValue = ""
        selection = nil
        entries = []
        resultHeight = 420
        closing = false
        presented = true
        expanded = false
        DispatchQueue.main.async {
            withAnimation(self.reduceMotion ? .easeOut(duration: 0.1) : .spring(response: 0.48, dampingFraction: 0.86)) {
                self.expanded = true
            }
        }
    }

    func close(then action: (() -> Void)? = nil) {
        if inline {
            action?()
            return
        }
        guard presented, !closing else { return }
        closing = true
        withAnimation(reduceMotion ? .easeOut(duration: 0.1) : .spring(response: 0.22, dampingFraction: 0.85)) {
            expanded = false
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + (reduceMotion ? 0.1 : 0.22)) {
            self.presented = false
            self.closing = false
            self.entries = []
            self.selection = nil
            DispatchQueue.main.async { self.restoreFocus?(); action?() }
        }
    }

    func move(_ delta: Int) {
        guard !entries.isEmpty, !closing else { return }
        let old = entries.firstIndex { $0.id == selection } ?? 0
        selection = entries[min(entries.count - 1, max(0, old + delta))].id
        navigationRevision += 1
    }

    func activate() {
        guard !closing, let entry = entries.first(where: { $0.id == selection }) else { return }
        entry.activate()
    }
}

struct BernPaletteEntry: Equatable {
    let id: String
    let activate: () -> Void
    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

struct BernPaletteEntries: PreferenceKey {
    static var defaultValue: [BernPaletteEntry] { [] }
    static func reduce(value: inout [BernPaletteEntry], nextValue: () -> [BernPaletteEntry]) {
        value += nextValue()
    }
}

private struct BernPaletteHeight: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

struct BernSearchPaletteHost<Content: View>: View {
    @ViewBuilder let content: () -> Content
    @State private var palette = BernPaletteState()
    @Environment(\.colorScheme) private var scheme
    @BernReducedMotion private var reduceMotion

    var body: some View {
        GeometryReader { bounds in
          // The window's content: what is smaller than the window starts at
          // its top leading corner, as on the other targets.
          content()
            .frame(width: bounds.size.width, height: bounds.size.height, alignment: .topLeading)
            .disabled(palette.presented)
            .accessibilityHidden(palette.presented)
            .overlay {
                GeometryReader { geometry in
                    if palette.presented {
                        ZStack(alignment: .topLeading) {
                            Color.black.opacity(palette.expanded ? (scheme == .dark ? 0.32 : 0.2) : 0)
                                .accessibilityHidden(true)
                                #if !os(macOS)
                                .contentShape(Rectangle())
                                .onTapGesture { palette.close() }
                                #endif
                            panel(in: geometry)
                        }
                        .frame(width: geometry.size.width, height: geometry.size.height)
                        #if os(macOS)
                        .background(BernPaletteOutsideClicks(panel: panelBounds(in: geometry)) { palette.close() })
                        #endif
                    }
                }
                .ignoresSafeArea()
            }
        }
        .environment(palette)
        .onChange(of: reduceMotion, initial: true) { palette.reduceMotion = reduceMotion }
    }

    private func panelBounds(in geometry: GeometryProxy) -> CGRect {
        let global = geometry.frame(in: .global)
        let available = geometry.size
        let panelWidth = min(palette.width, max(280, available.width - 32))
        // The toolbar's detail column determines the center, clamped to
        // the window so collapsed sidebars and narrow windows stay usable.
        let detailX = max(0, palette.detail.minX - global.minX)
        let centerX = min(available.width - panelWidth / 2 - 16,
                          max(panelWidth / 2 + 16, detailX + palette.detail.width / 2))
        let top: CGFloat = min(70, max(16, available.height * 0.1))
        let maxHeight = min(palette.height, max(120, available.height - top - 16))
        let panelHeight = min(maxHeight, 56 + palette.resultHeight)
        return CGRect(x: centerX - panelWidth / 2, y: top, width: panelWidth, height: panelHeight)
    }

    private func panel(in geometry: GeometryProxy) -> some View {
        let global = geometry.frame(in: .global)
        let bounds = panelBounds(in: geometry)
        let panelWidth = bounds.width
        let panelHeight = bounds.height
        let source = palette.source.offsetBy(dx: -global.minX, dy: -global.minY)
        let expanded = palette.expanded || reduceMotion
        return VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass").font(.system(size: 18)).foregroundStyle(.secondary)
                BernPaletteInput(text: palette.text, prompt: palette.prompt,
                                 up: { palette.move(-1) }, down: { palette.move(1) },
                                 submit: { palette.activate() }, cancel: { palette.close() })
                    .frame(height: 28)
                Button { palette.close() } label: {
                    Text("esc").font(.system(size: 11.5, weight: .medium)).foregroundStyle(.tertiary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Close search")
            }
            .padding(.horizontal, 18)
            .frame(height: 56)
            .overlay(alignment: .bottom) { Rectangle().fill(.primary.opacity(0.13)).frame(height: 0.5) }
            ScrollViewReader { scroll in
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) { palette.results }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(6)
                        .background(GeometryReader { geo in
                            Color.clear.preference(key: BernPaletteHeight.self, value: geo.size.height)
                        })
                        .id("bern-palette-results")
                }
                .scrollIndicators(.hidden)
                .onChange(of: palette.navigationRevision) {
                    if let selection = palette.selection {
                        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.15)) { scroll.scrollTo(selection) }
                    }
                }
                .onChange(of: palette.text.wrappedValue) {
                    scroll.scrollTo("bern-palette-results", anchor: .top)
                }
            }
        }
        .opacity(palette.expanded ? 1 : 0)
        .frame(width: expanded ? panelWidth : max(1, source.width), height: expanded ? panelHeight : max(1, source.height), alignment: .top)
        // The platform's overlay material, untinted.
        .background {
            RoundedRectangle(cornerRadius: expanded ? 22 : 17)
                .fill(.regularMaterial)
        }
        .clipShape(RoundedRectangle(cornerRadius: expanded ? 22 : 17))
        .contentShape(RoundedRectangle(cornerRadius: expanded ? 22 : 17))
        .overlay { RoundedRectangle(cornerRadius: expanded ? 22 : 17).strokeBorder(.primary.opacity(scheme == .dark ? 0.14 : 0.09), lineWidth: 0.5) }
        .shadow(color: .black.opacity(scheme == .dark ? 0.45 : 0.16), radius: 18, x: 0, y: 14)
        .position(x: expanded ? bounds.midX : source.midX, y: expanded ? bounds.midY : source.midY)
        .onPreferenceChange(BernPaletteHeight.self) { height in
            guard height > 0 else { return }
            withAnimation(reduceMotion ? nil : .spring(response: 0.5, dampingFraction: 0.62)) { palette.resultHeight = height }
        }
        .onPreferenceChange(BernPaletteEntries.self) { entries in
            palette.entries = entries
            if !entries.contains(where: { $0.id == palette.selection }) { palette.selection = entries.first?.id }
        }
        .onChange(of: palette.text.wrappedValue) { palette.selection = palette.entries.first?.id }
        #if os(macOS)
        .onExitCommand { palette.close() }
        #endif
    }
}

#if os(macOS)
/// Watch the native window so clicks over disabled split-view content still
/// dismiss the overlay. The observer never takes focus or handles panel input.
private struct BernPaletteOutsideClicks: NSViewRepresentable {
    let panel: CGRect
    let dismiss: () -> Void

    func makeNSView(context: Context) -> Observer { Observer() }
    func updateNSView(_ view: Observer, context: Context) {
        view.panel = panel
        view.dismiss = dismiss
    }
    static func dismantleNSView(_ view: Observer, coordinator: ()) { view.stop() }

    final class Observer: NSView {
        var panel = CGRect.zero
        var dismiss: (() -> Void)?
        private var monitor: Any?
        override var isFlipped: Bool { true }
        override func hitTest(_ point: NSPoint) -> NSView? { nil }
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            stop()
            guard window != nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
                let dismissed = MainActor.assumeIsolated {
                    guard let self, let window = self.window, event.window === window else { return false }
                    let point = self.convert(event.locationInWindow, from: nil)
                    guard self.bounds.contains(point), !self.panel.contains(point) else { return false }
                    self.dismiss?()
                    return true
                }
                return dismissed ? nil : event
            }
        }
        func stop() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
        }
    }
}

#endif

struct BernSearchPalette<Results: View>: View {
    let text: Binding<String>
    let prompt: String
    let panelPrompt: String
    let target: String
    let width: CGFloat
    let height: CGFloat
    var shortcut: BernKeyboardShortcut? = nil
    @ViewBuilder let results: () -> Results
    @Environment(BernPaletteState.self) private var palette
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    @Environment(\.bernReduceMotion) private var appReduceMotion
    @State private var source = CGRect.zero
    @FocusState private var focused: Bool

    private func open() {
        guard !palette.presented else { return }
        palette.reduceMotion = systemReduceMotion || appReduceMotion
        palette.text = text
        palette.prompt = panelPrompt
        palette.target = target
        palette.width = width
        palette.height = height
        palette.source = source
        palette.results = AnyView(results())
        palette.restoreFocus = { focused = true }
        palette.open()
    }

    var body: some View {
        Button(action: open) {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass").font(.system(size: 14))
                Text(verbatim: prompt).font(.system(size: 13)).lineLimit(1)
                Spacer(minLength: 4)
                #if !os(tvOS)
                if let shortcut { Text(verbatim: BernShortcuts.display(shortcut)).font(.system(size: 11.5, weight: .medium)) }
                #endif
            }
            .foregroundStyle(.secondary)
            .padding(.horizontal, 12)
            .frame(width: 250, height: 34)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .focused($focused)
        .background(GeometryReader { geometry in
            Color.clear.onChange(of: geometry.frame(in: .global), initial: true) { source = geometry.frame(in: .global) }
        })
        .bernHiddenShortcut(shortcut, action: open)
        .bernControl(target, role: "search", label: prompt,
                     actions: BernControlActions(activate: open, edit: { text.wrappedValue = $0 },
                                                 submit: { palette.activate() }, value: { text.wrappedValue }, focus: open))
    }
}

/// Registers the content column's geometry without participating in layout.
struct BernPaletteDetail: ViewModifier {
    @Environment(BernPaletteState.self) private var palette
    func body(content: Content) -> some View {
        content.background(GeometryReader { geo in
            Color.clear.onChange(of: geo.frame(in: .global), initial: true) { palette.detail = geo.frame(in: .global) }
        })
    }
}

struct BernSearchItem<Content: View>: View {
    let target: String
    let label: String
    let navigation: Bool
    let actions: BernControlActions
    @ViewBuilder let content: () -> Content
    @Environment(BernPaletteState.self) private var palette
    @Environment(\.bernItemKeys) private var keys
    @Environment(\.colorScheme) private var scheme
    @State private var hovered = false
    @FocusState private var focused: Bool
    private var id: String { target + keys }
    private func activate() { palette.close { actions.activate?() } }
    private var controlActions: BernControlActions {
        var wrapped = actions
        wrapped.activate = activate
        if let setFile = actions.setFile { wrapped.setFile = { url in palette.close { setFile(url) } } }
        return wrapped
    }
    var body: some View {
        Button(action: activate) { content().contentShape(Rectangle()) }
            .buttonStyle(BernPaletteButtonStyle())
            .focused($focused)
            .background {
                if navigation {
                    // In the page the rows run to the list's edges and the
                    // choice stands 6 points inside them, as in the panel,
                    // whose list is inset by those 6 points.
                    RoundedRectangle(cornerRadius: 12)
                        .fill(Color(red: 120/255, green: 120/255, blue: 128/255).opacity(palette.selection == id || hovered || focused ? (scheme == .dark ? 0.24 : 0.14) : 0))
                        .padding(.horizontal, palette.inline ? 6 : 0)
                }
            }
            .scaleEffect(!navigation && hovered ? 1.03 : 1)
            .animation(.spring(response: 0.22, dampingFraction: 0.85), value: hovered)
            #if os(tvOS)
            // A television's focus: a row turns white, a picture lifts.
            .modifier(BernTelevisionFocusLook(kind: navigation ? .row : .picture, corner: 12, focused: focused))
            #endif
            .bernHover { hovered = $0 }
            .accessibilityLabel(Text(verbatim: label))
            .accessibilityAddTraits(palette.selection == id ? .isSelected : [])
            .bernControl(target, role: "button", label: label, actions: controlActions)
            .preference(key: BernPaletteEntries.self, value: navigation ? [BernPaletteEntry(id: id, activate: activate)] : [])
            .id(id)
    }
}

private struct BernPaletteButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.opacity(configuration.isPressed ? 0.7 : 1)
    }
}

// MARK: - Search field in the page

/// A search field standing in the page, the platform's own
/// (`BernSearchInput`); its results are listed below it while it is in
/// use, focused or holding text. As in the palette the first result is
/// chosen, the arrow keys move the choice and return runs it.
struct BernSearchField<Results: View>: View {
    let text: Binding<String>
    let prompt: String
    let target: String
    /// The field's own height, width and text size; the platform's when
    /// not given.
    var height: CGFloat? = nil
    var fontSize: CGFloat? = nil
    var width: CGFloat? = nil
    /// The results show before anything is typed: while the field is in
    /// use, and on a television, which types with the remote beside it,
    /// always.
    var suggest = false
    var submit: (() -> Void)? = nil
    @ViewBuilder let results: () -> Results
    @State private var palette = BernPaletteState(inline: true)
    @State private var editing = false
    @State private var focusRequest = 0

    private func choose() {
        if palette.selection != nil { palette.activate() } else { submit?() }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            BernSearchInput(
                text: text, prompt: prompt, fontSize: fontSize, editing: $editing, focusRequest: focusRequest,
                up: { palette.move(-1) }, down: { palette.move(1) }, submit: choose,
                cancel: { text.wrappedValue = "" })
                .bernSearchSurface(height: height)
                .frame(width: width)
            if editing || !text.wrappedValue.isEmpty || (suggest && bernTelevision) {
                results()
                    .onPreferenceChange(BernPaletteEntries.self) { entries in
                        palette.entries = entries
                        if !entries.contains(where: { $0.id == palette.selection }) { palette.selection = entries.first?.id }
                    }
                    .onChange(of: text.wrappedValue) { palette.selection = palette.entries.first?.id }
            }
        }
        .environment(palette)
        .bernControl(target, role: "search", label: prompt,
                     actions: BernControlActions(edit: { text.wrappedValue = $0 }, submit: choose,
                                                 value: { text.wrappedValue }, focus: { focusRequest += 1 }))
    }
}

extension View {
    /// The search field's surface: on iOS 26 the system's glass in a
    /// capsule, as the other capsule controls; elsewhere the field's own.
    @ViewBuilder func bernSearchSurface(height: CGFloat?) -> some View {
        #if os(iOS)
        if #available(iOS 26.0, tvOS 26.0, *) {
            // The field insets its own symbol and text.
            self.frame(height: height ?? 36)
                .glassEffect(.regular.interactive(), in: Capsule())
        } else {
            self.frame(height: height)
        }
        #else
        self.frame(height: height)
        #endif
    }
}

struct BernSearchRow: View {
    let label: String
    let icon: String?
    let detail: String
    let hint: String
    /// A television reads the row from across the room: 16-point words,
    /// 13.5-point details.
    private var body_: CGFloat { bernTelevision ? 16 : 13 }
    private var small: CGFloat { bernTelevision ? 13.5 : 11.5 }
    var body: some View {
        HStack(spacing: 12) {
            if let icon {
                Image(systemName: icon).font(.system(size: bernTelevision ? 16 : 14)).foregroundStyle(.secondary)
                    .frame(width: bernTelevision ? 30 : 26, height: bernTelevision ? 30 : 26)
                    .background(.primary.opacity(0.08), in: RoundedRectangle(cornerRadius: 7))
            }
            Text(verbatim: label).font(.system(size: body_)).lineLimit(1)
            if !detail.isEmpty {
                Text(verbatim: detail).font(.system(size: small, weight: .medium)).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 0)
            if !hint.isEmpty { Text(verbatim: hint).font(.system(size: small, weight: .medium)).foregroundStyle(.tertiary) }
        }
        .padding(.horizontal, 12)
        .frame(height: 42)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

#if os(macOS)
/// AppKit's field editor preserves Chinese IME composition and dispatches
/// navigation commands only after composition has finished.
private final class BernPaletteTextField: NSTextField {
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard window != nil else { return }
        // The toolbar relinquishes focus after the overlay mounts. Focus
        // after that handoff, while the expansion is still in flight.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { [weak self] in
            guard let self, let window = self.window else { return }
            window.makeFirstResponder(self)
        }
    }
}

struct BernPaletteInput: NSViewRepresentable {
    let text: Binding<String>
    let prompt: String
    let up: () -> Void
    let down: () -> Void
    let submit: () -> Void
    let cancel: () -> Void
    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> NSTextField {
        let field = BernPaletteTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .systemFont(ofSize: 16)
        field.textColor = .labelColor
        field.placeholderString = prompt
        field.stringValue = text.wrappedValue
        field.delegate = context.coordinator
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.setAccessibilityLabel(prompt)
        return field
    }
    func updateNSView(_ field: NSTextField, context: Context) {
        context.coordinator.owner = self
        field.placeholderString = prompt
        if field.stringValue != text.wrappedValue && !((field.currentEditor() as? NSTextView)?.hasMarkedText() ?? false) {
            field.stringValue = text.wrappedValue
        }
    }
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var owner: BernPaletteInput
        init(_ owner: BernPaletteInput) { self.owner = owner }
        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            owner.text.wrappedValue = field.stringValue
        }
        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            guard !textView.hasMarkedText() else { return false }
            switch commandSelector {
            case #selector(NSResponder.moveUp(_:)): owner.up()
            case #selector(NSResponder.moveDown(_:)): owner.down()
            case #selector(NSResponder.insertNewline(_:)): owner.submit()
            case #selector(NSResponder.cancelOperation(_:)): owner.cancel()
            default: return false
            }
            return true
        }
    }
}
#else
/// The panel's field: the system text field, focused once the panel opens;
/// a hardware keyboard's arrows move the selection and Escape closes.
struct BernPaletteInput: View {
    let text: Binding<String>
    let prompt: String
    let up: () -> Void
    let down: () -> Void
    let submit: () -> Void
    let cancel: () -> Void
    @FocusState private var focused: Bool
    var body: some View {
        TextField(text: text, prompt: Text(verbatim: prompt)) { Text(verbatim: prompt) }
            .textFieldStyle(.plain)
            .font(.system(size: 16))
            .submitLabel(.search)
            .autocorrectionDisabled()
            .focused($focused)
            .onSubmit(submit)
            .onKeyPress(.upArrow) { up(); return .handled }
            .onKeyPress(.downArrow) { down(); return .handled }
            .onKeyPress(.escape) { cancel(); return .handled }
            .onAppear {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { focused = true }
            }
    }
}
#endif

/// A decorative surface keeps its children as native controls.
struct BernSurface: ViewModifier {
    let style: BernStyle
    func body(content: Content) -> some View {
        let shadow = style.base.shadow ?? BernShadow(radius: 0, y: 0, opacity: 0)
        surface(content)
            .shadow(color: .black.opacity(shadow.opacity), radius: shadow.radius, y: shadow.y)
    }

    @ViewBuilder
    private func surface(_ content: Content) -> some View {
        if let corner = style.corner {
            content
                .bernForeground(style.base.foreground?.color)
                .background(RoundedRectangle(cornerRadius: corner, style: .continuous).fill(style.base.background?.color ?? Color.clear))
                .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
        } else {
            // A square surface, such as a page's, also fills the safe areas
            // around it: under the status bar and the bars of the window.
            content
                .bernForeground(style.base.foreground?.color)
                .background(style.base.background?.color ?? Color.clear)
        }
    }
}


/// Keeps a chip's off state neutral and its selected label legible,
/// using the same native glass surface as neighboring capsule controls.
struct BernCapsuleToggleStyle: ToggleStyle {
    let height: CGFloat
    func makeBody(configuration: Configuration) -> some View {
        Button { configuration.isOn.toggle() } label: { configuration.label }
            .bernCapsuleControl(height: height, prominent: configuration.isOn)
            .accessibilityAddTraits(configuration.isOn ? [.isSelected] : [])
    }
}

extension View {
    /// A round system button of an exact diameter: the system's glass (or
    /// material before 26) in a circle, as the capsule controls.
    func bernCircleControl(diameter: CGFloat, prominent: Bool) -> some View {
        modifier(BernCircleControl(diameter: diameter, prominent: prominent))
    }
}

private struct BernCircleControl: ViewModifier {
    @Environment(\.bernAccent) private var accent
    @Environment(\.bernOnBar) private var onBar
    let diameter: CGFloat
    let prominent: Bool
    @ViewBuilder func body(content: Content) -> some View {
        #if os(tvOS)
        content.buttonStyle(BernFocusShapeStyle(
            prominent: prominent, accent: accent ?? .accentColor, circle: true, height: diameter, padding: 0))
        #else
        let control = content.buttonStyle(.plain)
            .foregroundStyle(prominent ? BernColor.role("on_accent").color : Color.primary)
            .frame(width: diameter, height: diameter)
            .contentShape(Circle())
        if onBar && !prominent {
            // The bar is the glass; the button is a target of its size.
            control
        } else if #available(macOS 26.0, iOS 26.0, tvOS 26.0, *) {
            if prominent {
                control
                    .background(accent ?? .accentColor, in: Circle())
                    .glassEffect(.regular.tint(accent ?? .accentColor).interactive(), in: Circle())
            } else {
                control.glassEffect(.regular.interactive(), in: Circle())
            }
        } else if prominent {
            control.background(accent ?? .accentColor, in: Circle())
        } else {
            control.background(.regularMaterial, in: Circle())
        }
        #endif
    }
}

#if os(tvOS)
/// A capsule or circle button of the design's size on tvOS. The system's
/// bordered buttons take sizes of their own, larger than a scene the
/// design lays out, so this one keeps the size and shows the focus as
/// tvOS's own buttons do: under it the button turns white with a dark
/// label, lifts by 1.08 with a shadow on a bouncy spring, and settles back
/// smoothly.
struct BernFocusShapeStyle: ButtonStyle {
    let prominent: Bool
    let accent: Color
    let circle: Bool
    let height: CGFloat
    let padding: CGFloat
    /// A plain button: its label alone, in the secondary color, until the
    /// focus puts a white capsule around it.
    var plain = false

    func makeBody(configuration: Configuration) -> some View {
        BernFocusShape(configuration: configuration, style: self)
    }
}

private struct BernFocusShape: View {
    let configuration: ButtonStyle.Configuration
    let style: BernFocusShapeStyle
    @Environment(\.isFocused) private var focused
    @Environment(\.isEnabled) private var enabled
    @BernReducedMotion private var reduce

    var body: some View {
        if style.plain {
            plain
        } else {
            shaped
        }
    }

    private var plain: some View {
        configuration.label
            .foregroundStyle(focused ? Color.black : Color.secondary)
            .background {
                if focused {
                    Capsule().fill(Color.white).padding(.horizontal, -14).padding(.vertical, -8)
                }
            }
            .contentShape(Rectangle())
            .scaleEffect(configuration.isPressed ? 0.96 : focused ? 1.1 : 1)
            .shadow(color: .black.opacity(focused ? 0.45 : 0), radius: focused ? 14 : 0, y: focused ? 8 : 0)
            .opacity(enabled ? 1 : 0.45)
            .animation(
                reduce || bernForcedReduceMotion
                    ? .easeInOut(duration: 0.15)
                    : .spring(response: 0.3, dampingFraction: focused ? 0.6 : 1),
                value: focused)
    }

    private var shaped: some View {
        let shape = style.circle ? AnyShape(Circle()) : AnyShape(Capsule())
        return configuration.label
            .padding(.horizontal, style.padding)
            .frame(width: style.circle ? style.height : nil, height: style.height)
            .foregroundStyle(focused ? Color.black : style.prominent ? BernColor.role("on_accent").color : Color.primary)
            .background {
                if focused {
                    shape.fill(Color.white)
                } else if style.prominent {
                    shape.fill(style.accent)
                } else if #available(tvOS 26.0, *) {
                    Color.clear.glassEffect(.regular, in: shape)
                } else {
                    shape.fill(.regularMaterial)
                }
            }
            .contentShape(shape)
            .scaleEffect(configuration.isPressed ? 0.96 : focused ? 1.1 : 1)
            .shadow(color: .black.opacity(focused ? 0.45 : 0), radius: focused ? 14 : 0, y: focused ? 8 : 0)
            .opacity(enabled ? 1 : 0.45)
            .animation(
                reduce || bernForcedReduceMotion
                    ? .easeInOut(duration: 0.15)
                    : .spring(response: 0.3, dampingFraction: focused ? 0.6 : 1),
                value: focused)
    }
}
#endif

private struct BernCapsuleControl: ViewModifier {
    @Environment(\.bernAccent) private var accent
    let height: CGFloat
    let prominent: Bool
    @ViewBuilder func body(content: Content) -> some View {
        #if os(tvOS)
        content
            .buttonStyle(BernFocusShapeStyle(
                prominent: prominent, accent: accent ?? .accentColor, circle: false, height: height,
                padding: height >= 40 ? 22 : height <= 30 ? 13 : 14))
            .font(height >= 40 ? .system(size: 15, weight: .semibold) : .system(size: 13, weight: .medium))
        #else
        // The design's button sizes (6.3): a main action of 42 points or
        // more is set in 15-point semibold, 22 from its ends; the others in
        // 13-point medium, 14 from their ends, or 13 on a 30-point chip.
        let control = content.buttonStyle(.plain)
            .font(height >= 40 ? .system(size: 15, weight: .semibold) : .system(size: 13, weight: .medium))
            .foregroundStyle(prominent ? BernColor.role("on_accent").color : Color.primary)
            .padding(.horizontal, height >= 40 ? 22 : height <= 30 ? 13 : 14).frame(height: height)
            .contentShape(Capsule())
        if #available(macOS 26.0, iOS 26.0, tvOS 26.0, *) {
            if prominent {
                // Keep the accent legible when the window's native glass
                // desaturates; the glass still supplies the interactive surface.
                control
                    .background(accent ?? .accentColor, in: Capsule())
                    .glassEffect(.regular.tint(accent ?? .accentColor).interactive(), in: Capsule())
            } else {
                control.glassEffect(.regular.interactive(), in: Capsule())
            }
        } else if prominent {
            control.background(accent ?? .accentColor, in: Capsule())
        } else {
            control.background(.regularMaterial, in: Capsule())
        }
        #endif
    }
}

/// Same-window modal: preserves the hero coordinate space and constrains
/// content at small window sizes. The caller disables the background tree.
struct BernModal<Content: View>: View {
    @BernReducedMotion private var reduce
    @Environment(BernWindowContext.self) private var window: BernWindowContext?
    /// A modal declared in a scaled scene shows at the scene's scale.
    @Environment(\.bernSceneScale) private var scale
    let width: CGFloat
    let height: CGFloat
    /// Covers the whole window where the window is compact.
    var cover = false
    @ViewBuilder var content: Content
    var presentation: (AnyView) -> AnyView = { $0 }
    var body: some View {
        Color.clear.frame(width: 0, height: 0)
            .preference(key: BernModalContent.self, value: AnyView(
                BernModalLayer(width: width, height: height, covers: cover && window?.compact == true,
                               scale: scale, content: content, presentation: presentation)
                    .environment(\.bernReduceMotion, reduce)
                    .environment(\.bernSceneScaled, scale != 1)))
    }
}

struct BernModalContent: PreferenceKey {
    static var defaultValue: AnyView? { nil }
    static func reduce(value: inout AnyView?, nextValue: () -> AnyView?) {
        if let next = nextValue() { value = next }
    }
}

/// The window root presents the modal, so a wide background layout cannot
/// enlarge its coordinate space.
private struct BernModalLayer<Content: View>: View {
    let width: CGFloat
    let height: CGFloat
    /// The whole window, edge to edge, as a phone's full-screen flow.
    var covers = false
    /// The scale of the scene the modal was declared in.
    var scale: CGFloat = 1
    let content: Content
    let presentation: (AnyView) -> AnyView
    var body: some View {
        if covers {
            // Its background reaches the edges; its content keeps to the
            // safe area as a window's does, unless a `Stack(under_bars:)`
            // in it goes under the status bar and the home indicator.
            presentation(AnyView(content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background { Rectangle().fill(.background).ignoresSafeArea() }
                .accessibilityAddTraits(.isModal)))
        } else {
            floating
        }
    }

    private var floating: some View {
        GeometryReader { proxy in
            #if os(macOS)
            let topInset: CGFloat = proxy.size.height < height + 32 ? 52 : 0
            #else
            let topInset: CGFloat = 0
            #endif
            let modalWidth = min(width, max(0, proxy.size.width / scale - 32))
            let modalHeight = min(height, max(0, (proxy.size.height - topInset) / scale - 32))
            ZStack {
                Color.black.opacity(0.24).contentShape(Rectangle()).onTapGesture { }
                presentation(AnyView(content
                    .frame(width: modalWidth, height: modalHeight)
                    .clipShape(RoundedRectangle(cornerRadius: 24))
                    .background {
                        RoundedRectangle(cornerRadius: 24).fill(.background)
                            .shadow(color: .black.opacity(0.28), radius: 40, y: 20)
                    }
                    .overlay(RoundedRectangle(cornerRadius: 24).strokeBorder(.primary.opacity(0.12), lineWidth: 0.5))
                    .accessibilityAddTraits(.isModal)
                    .scaleEffect(scale)
                    .frame(width: modalWidth * scale, height: modalHeight * scale)))
            }.frame(width: proxy.size.width, height: proxy.size.height - topInset).offset(y: topInset)
        }.ignoresSafeArea()
    }
}

/// Marks a `Spacer` in a `Row`: it takes room along the row, never the
/// row's height.
struct BernRowSpacer: LayoutValueKey {
    static let defaultValue = false
}

/// A `Row`, laid out as rows are on the other targets: while the items
/// fit, each is as wide as it wants and the room left over is shared
/// equally by those that grow, spacers and filling frames. Only a row too
/// narrow for them shrinks its items, each by its part of their width and
/// never below its least. (`HStack` offers its items equal parts in order
/// of flexibility, which cuts a short text off though the row has room.)
struct BernRow: Layout {
    var alignment: VerticalAlignment = .center
    var spacing: CGFloat? = nil

    private struct Item {
        var ideal: CGFloat
        var least: CGFloat
        var most: CGFloat
        var grows: Bool
        var spacer: Bool
    }

    private func gaps(_ subviews: Subviews) -> [CGFloat] {
        guard subviews.count > 1 else { return [] }
        return (0..<(subviews.count - 1)).map { index in
            spacing ?? subviews[index].spacing.distance(to: subviews[index + 1].spacing, along: .horizontal)
        }
    }

    /// The width each item is offered.
    private func widths(_ width: CGFloat?, _ height: CGFloat?, _ subviews: Subviews) -> [CGFloat] {
        let items = subviews.map { subview -> Item in
            // A spacer has no least width, as on the other targets; the
            // system's own is a gap of its idiom (60 points on tvOS), which
            // took the room of the texts beside it in a narrow row.
            if subview[BernRowSpacer.self] {
                return Item(ideal: 0, least: 0, most: .infinity, grows: true, spacer: true)
            }
            let least = subview.sizeThatFits(ProposedViewSize(width: 0, height: height)).width
            let ideal = subview.sizeThatFits(ProposedViewSize(width: nil, height: height)).width
            let most = subview.sizeThatFits(ProposedViewSize(width: .infinity, height: height)).width
            return Item(ideal: ideal, least: min(least, ideal), most: max(most, ideal), grows: most > ideal + 0.5, spacer: false)
        }
        guard let width else { return items.map(\.ideal) }
        var offered = items.map { $0.grows ? $0.least : $0.ideal }
        let room = width - gaps(subviews).reduce(0, +)
        let used = offered.reduce(0, +)
        if !width.isFinite {
            return items.map { $0.grows ? $0.most : $0.ideal }
        }
        if used <= room {
            // What is left over goes to the items that grow, as equally as
            // their least and most widths allow.
            var open = items.indices.filter { items[$0].grows }
            var left = room - items.indices.filter { !items[$0].grows }.reduce(0) { $0 + offered[$1] }
            while !open.isEmpty {
                let share = left / CGFloat(open.count)
                if let index = open.first(where: { items[$0].least > share || items[$0].most < share }) {
                    offered[index] = items[index].least > share ? items[index].least : items[index].most
                    left -= offered[index]
                    open.removeAll { $0 == index }
                    continue
                }
                for index in open { offered[index] = share }
                break
            }
        } else {
            // Too narrow: the others shrink by their part of what is
            // missing, weighed by their width, never below their least.
            var missing = used - room
            var open = items.indices.filter { !items[$0].grows && items[$0].ideal > items[$0].least }
            while missing > 0.01, !open.isEmpty {
                let total = open.reduce(0) { $0 + items[$1].ideal }
                guard total > 0 else { break }
                if let index = open.first(where: { offered[$0] - missing * items[$0].ideal / total < items[$0].least }) {
                    missing -= offered[index] - items[index].least
                    offered[index] = items[index].least
                    open.removeAll { $0 == index }
                    continue
                }
                for index in open { offered[index] -= missing * items[index].ideal / total }
                missing = 0
            }
        }
        return offered
    }

    private func arrange(_ width: CGFloat?, _ height: CGFloat?, _ subviews: Subviews)
        -> (size: CGSize, widths: [CGFloat], tops: [CGFloat], extents: [CGFloat])
    {
        let offered = widths(width, height, subviews)
        let gaps = gaps(subviews)
        var above: CGFloat = 0, below: CGFloat = 0
        var extents: [CGFloat] = [], guides: [CGFloat] = []
        for (subview, offer) in zip(subviews, offered) {
            if subview[BernRowSpacer.self] {
                extents.append(offer)
                guides.append(0)
                continue
            }
            let dimensions = subview.dimensions(in: ProposedViewSize(width: offer, height: height))
            let guide = dimensions[alignment]
            above = max(above, guide)
            below = max(below, dimensions.height - guide)
            extents.append(dimensions.width)
            guides.append(guide)
        }
        let tops = zip(subviews, guides).map { subview, guide in subview[BernRowSpacer.self] ? 0 : above - guide }
        let length = extents.reduce(0, +) + gaps.reduce(0, +)
        return (CGSize(width: length, height: above + below), offered, tops, extents)
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(proposal.width, proposal.height, subviews).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let layout = arrange(bounds.width, proposal.height, subviews)
        let gaps = gaps(subviews)
        var x = bounds.minX
        for index in subviews.indices {
            let spacer = subviews[index][BernRowSpacer.self]
            subviews[index].place(
                at: CGPoint(x: x, y: bounds.minY + layout.tops[index]),
                anchor: .topLeading,
                proposal: ProposedViewSize(width: layout.widths[index], height: spacer ? bounds.height : proposal.height))
            x += layout.extents[index] + (index < gaps.count ? gaps[index] : 0)
        }
    }
}

struct BernFlow: Layout {
    var gap: CGFloat = 8

    /// Where each child stands: lines broken at the width, a spacer taking
    /// no room there; then the spacers of a line share the room left on
    /// it, and each child is centred in its line's height.
    private func arrange(_ width: CGFloat?, _ subviews: Subviews) -> (CGSize, [CGRect]) {
        let limit = width ?? .infinity
        let spacers = subviews.map { $0[BernRowSpacer.self] }
        let sizes = subviews.indices.map { spacers[$0] ? .zero : subviews[$0].sizeThatFits(.unspecified) }
        var lines: [[Int]] = [[]]
        var x: CGFloat = 0
        for index in subviews.indices {
            if !lines[lines.count - 1].isEmpty && x + sizes[index].width > limit {
                lines.append([])
                x = 0
            }
            lines[lines.count - 1].append(index)
            x += sizes[index].width + gap
        }
        var frames = Array(repeating: CGRect.zero, count: subviews.count)
        var y: CGFloat = 0, widest: CGFloat = 0
        for (number, line) in lines.enumerated() where !line.isEmpty {
            let used = line.reduce(0) { $0 + sizes[$1].width } + gap * CGFloat(line.count - 1)
            let shares = line.filter { spacers[$0] }.count
            let share = shares > 0 && limit.isFinite ? max(0, limit - used) / CGFloat(shares) : 0
            let height = line.map { sizes[$0].height }.max() ?? 0
            var x: CGFloat = 0
            for index in line {
                let width = spacers[index] ? share : sizes[index].width
                let tall = spacers[index] ? height : sizes[index].height
                frames[index] = CGRect(x: x, y: y + (height - tall) / 2, width: width, height: tall)
                x += width + gap
            }
            widest = max(widest, x - gap)
            y += height + (number < lines.count - 1 ? gap : 0)
        }
        return (CGSize(width: width ?? widest, height: y), frames)
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(proposal.width, subviews).0
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let (_, frames) = arrange(bounds.width, subviews)
        for (view, frame) in zip(subviews, frames) {
            view.place(
                at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY), anchor: .topLeading,
                proposal: view[BernRowSpacer.self] ? ProposedViewSize(frame.size) : .unspecified)
        }
    }
}

/// `Picker(style: chips)`: a chip per option, the chosen one prominent, as
/// system glass capsules at the design's chip size (6.3). In a `Flow` the
/// chips are items of the flow; elsewhere they wrap in a flow of their own.
struct BernChips<Value: Hashable>: View {
    @Binding var selection: Value
    let options: [(String, Value)]
    /// Items of the enclosing flow.
    let flat: Bool
    let target: String
    let label: String
    let actions: BernControlActions

    var body: some View {
        if flat {
            // The flow lays each chip out; the first carries the picker's
            // identity for the control entry.
            ForEach(Array(options.enumerated()), id: \.offset) { index, option in
                if index == 0 {
                    chip(option).bernControl(target, role: "picker", label: label, actions: actions)
                } else {
                    chip(option)
                }
            }
        } else {
            BernFlow(gap: 8) {
                ForEach(Array(options.enumerated()), id: \.offset) { _, option in chip(option) }
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel(Text(verbatim: label))
            .bernControl(target, role: "picker", label: label, actions: actions)
        }
    }

    private func chip(_ option: (String, Value)) -> some View {
        let selected = option.1 == selection
        return Button { selection = option.1 } label: { Text(verbatim: option.0).lineLimit(1) }
            #if os(tvOS)
            .bernCapsuleControl(height: 40, prominent: selected)
            #else
            .bernCapsuleControl(height: 30, prominent: selected)
            #endif
            .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// No publisher is retained after the owning tree disappears or is disabled.
struct BernTimer: View {
    let interval: Double
    let enabled: Bool
    let tick: () -> Void
    var body: some View {
        Color.clear.frame(width: 0, height: 0).accessibilityHidden(true)
            .task(id: enabled) {
                guard enabled else { return }
                while !Task.isCancelled {
                    do { try await Task.sleep(for: .seconds(max(0.05, interval))) } catch { return }
                    guard !Task.isCancelled else { return }
                    tick()
                }
            }
    }
}

extension View {
    /// `default_focus`: the focus starts on the control when it appears,
    /// the remote's on a television and the keyboard's elsewhere.
    func bernDefaultFocus() -> some View {
        modifier(BernDefaultFocus())
    }
}

private struct BernDefaultFocus: ViewModifier {
    @FocusState private var focused: Bool
    #if os(tvOS)
    @Environment(\.bernFocusScope) private var scope
    #endif

    func body(content: Content) -> some View {
        content
            #if os(tvOS)
            // The focus engine starts here when it chooses the window's
            // focus, at launch and when what had the focus leaves.
            .modifier(BernPreferredFocus(scope: scope))
            #endif
            .focused($focused)
            // Shown later, the control takes the focus once it is in the
            // focus system.
            .onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { focused = true } }
    }
}

#if os(tvOS)
private struct BernPreferredFocus: ViewModifier {
    let scope: Namespace.ID?

    func body(content: Content) -> some View {
        if let scope {
            content.prefersDefaultFocus(true, in: scope)
        } else {
            content
        }
    }
}

private struct BernFocusScopeKey: EnvironmentKey {
    static let defaultValue: Namespace.ID? = nil
}

extension EnvironmentValues {
    /// The window's focus scope on tvOS, where `default_focus` prefers.
    var bernFocusScope: Namespace.ID? {
        get { self[BernFocusScopeKey.self] }
        set { self[BernFocusScopeKey.self] = newValue }
    }
}

/// The window root's focus scope on tvOS.
struct BernFocusScope: ViewModifier {
    @Namespace private var scope

    func body(content: Content) -> some View {
        content
            .focusScope(scope)
            .environment(\.bernFocusScope, scope)
    }
}
#endif

extension View {
    /// A form section's title: in a scene laid out at the design's size on
    /// tvOS, the design's small label (12.5 points, semibold, secondary)
    /// instead of tvOS's large one; the platform's elsewhere.
    @ViewBuilder func bernSectionTitle() -> some View {
        #if os(tvOS)
        modifier(BernSceneSectionText(size: 12.5, weight: .semibold))
        #else
        self
        #endif
    }

    /// A form section's footer, as its title at the design's size.
    @ViewBuilder func bernSectionFooter() -> some View {
        #if os(tvOS)
        modifier(BernSceneSectionText(size: 12.5, weight: .regular))
        #else
        self
        #endif
    }
}

#if os(tvOS)
private struct BernSceneSectionText: ViewModifier {
    let size: CGFloat
    let weight: Font.Weight
    @Environment(\.bernSceneScaled) private var scaled

    func body(content: Content) -> some View {
        if scaled {
            content.font(.system(size: size, weight: weight)).foregroundStyle(.secondary).textCase(nil)
        } else {
            content
        }
    }
}
#endif

extension View {
    /// A text field in a scaled scene on tvOS: tvOS's own field at the
    /// design's size for a television (46 points on 16-point type) instead
    /// of tvOS's, which the scene would enlarge again. tvOS draws its
    /// platter and focus, and selecting it opens the system's keyboard.
    @ViewBuilder func bernFieldLook() -> some View {
        #if os(tvOS)
        modifier(BernTelevisionField())
        #else
        self
        #endif
    }
}

#if os(tvOS)
private struct BernTelevisionField: ViewModifier {
    @Environment(\.bernSceneScaled) private var scaled

    func body(content: Content) -> some View {
        if scaled {
            // tvOS's own field, which draws its platter and its focus, at
            // the design's size.
            content
                .font(.system(size: 16))
                .frame(minHeight: 46)
        } else {
            content
        }
    }
}
#endif

/// `Shortcut`: a keyboard shortcut without a control of its own; on tvOS
/// the Siri Remote's button that stands for the key.
struct BernKeyShortcut: View {
    #if os(tvOS)
    let remote: BernRemoteKey?
    #else
    let shortcut: KeyboardShortcut?
    #endif
    let enabled: Bool
    let action: () -> Void

    /// A shortcut that follows state may be none for a while: the key
    /// does nothing then.
    #if os(tvOS)
    init(remote: BernRemoteKey?, enabled: Bool, action: @escaping () -> Void) {
        self.remote = remote
        self.enabled = enabled
        self.action = action
    }
    #else
    init(shortcut: KeyboardShortcut?, enabled: Bool, action: @escaping () -> Void) {
        self.shortcut = shortcut
        self.enabled = enabled
        self.action = action
    }
    #endif

    var body: some View {
        #if os(tvOS)
        Color.clear.frame(width: 0, height: 0).accessibilityHidden(true)
            .bernRemoteShortcut(remote, enabled: enabled, action: action)
        #else
        Button("", action: action)
            .keyboardShortcut(shortcut)
            .disabled(!enabled || shortcut == nil)
            .opacity(0)
            .frame(width: 0, height: 0)
            .accessibilityHidden(true)
        #endif
    }
}

// MARK: - Shortcut inputs

/// `ShortcutInput`: a button showing the shortcut as the platform writes
/// it; pressed, it takes the next key with its modifiers. Escape leaves the
/// shortcut as it was, Delete alone clears it, and a press elsewhere stops
/// waiting. A keyboard is needed: on a television it only shows the
/// shortcut, and on iOS it waits for a hardware keyboard's keys.
struct BernShortcutInput: View {
    let current: String
    let label: String
    let labelHidden: Bool
    let prompt: String
    let empty: String
    let commit: (String) -> Void
    @State private var recording = false
    #if os(macOS)
    @State private var monitor: Any?
    #elseif os(iOS)
    @FocusState private var focused: Bool
    #endif

    private var shown: String {
        if recording { return prompt }
        let text = BernShortcuts.display(current)
        return text.isEmpty ? empty : text
    }

    var body: some View {
        if labelHidden {
            field.accessibilityLabel(SwiftUI.Text(verbatim: label))
        } else {
            SwiftUI.LabeledContent {
                field
            } label: {
                SwiftUI.Text(verbatim: label)
            }
        }
    }

    @ViewBuilder private var field: some View {
        #if os(tvOS)
        SwiftUI.Text(verbatim: shown).foregroundStyle(.secondary)
        #else
        SwiftUI.Button {
            recording.toggle()
        } label: {
            SwiftUI.Text(verbatim: shown)
                .monospacedDigit()
                .frame(minWidth: 72)
        }
        .bernButtonLook(.secondary)
        #if os(macOS)
        .onChange(of: recording) { _, now in now ? listen() : stop() }
        .onDisappear { recording = false; stop() }
        #else
        .focusable(recording)
        .focused($focused)
        .onChange(of: recording) { _, now in focused = now }
        .onChange(of: focused) { _, now in if !now { recording = false } }
        .onKeyPress(phases: .down) { press in
            guard recording else { return .ignored }
            take(press)
            return .handled
        }
        #endif
        #endif
    }

    private func finish(_ keys: String?) {
        recording = false
        if let keys { commit(keys) }
    }

    #if os(macOS)
    /// AppKit's key codes of the keys Bern names.
    private static let codes: [UInt16: String] = [
        49: "space", 36: "return", 76: "return", 48: "tab", 51: "delete", 117: "delete",
        123: "left", 124: "right", 125: "down", 126: "up", 115: "home", 119: "end",
        116: "pageup", 121: "pagedown",
    ]

    /// Take key presses before windows and menus do, while waiting: the
    /// shortcut being chosen must not run what it is bound to now. A
    /// click anywhere stops waiting.
    private func listen() {
        stop()
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .leftMouseDown, .rightMouseDown]) { event in
            guard recording else { return event }
            if event.type != .keyDown {
                recording = false
                return event
            }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            let held = BernShortcuts.modifiers(
                command: flags.contains(.command), control: flags.contains(.control),
                option: flags.contains(.option), shift: flags.contains(.shift))
            if event.keyCode == 53 {
                finish(nil)
                return nil
            }
            let key = Self.codes[event.keyCode]
                ?? event.characters(byApplyingModifiers: [])?.lowercased()
            if key == "delete" && held.isEmpty {
                finish("")
                return nil
            }
            // Function keys and other keys Bern has no name for wait for another.
            if let key, let keys = BernShortcuts.normalized((held + [key]).joined(separator: "+")), !keys.isEmpty {
                finish(keys)
            }
            return nil
        }
    }

    private func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
    #elseif os(iOS)
    private func take(_ press: KeyPress) {
        let named: [(KeyEquivalent, String)] = [
            (.space, "space"), (.return, "return"), (.tab, "tab"), (.delete, "delete"),
            (.upArrow, "up"), (.downArrow, "down"), (.leftArrow, "left"), (.rightArrow, "right"),
            (.home, "home"), (.end, "end"), (.pageUp, "pageup"), (.pageDown, "pagedown"),
        ]
        if press.key == .escape {
            finish(nil)
            return
        }
        let held = BernShortcuts.modifiers(
            command: press.modifiers.contains(.command), control: press.modifiers.contains(.control),
            option: press.modifiers.contains(.option), shift: press.modifiers.contains(.shift))
        let key = named.first { $0.0 == press.key }?.1 ?? String(press.key.character).lowercased()
        if key == "delete" && held.isEmpty {
            finish("")
            return
        }
        if let keys = BernShortcuts.normalized((held + [key]).joined(separator: "+")), !keys.isEmpty {
            finish(keys)
        }
    }
    #endif
}

// MARK: - Forms on their page

private struct BernFormRowKey: EnvironmentKey {
    static let defaultValue: Color? = nil
}

extension EnvironmentValues {
    /// The rows' background of the form around, when its style gives one.
    var bernFormRow: Color? {
        get { self[BernFormRowKey.self] }
        set { self[BernFormRowKey.self] = newValue }
    }
}

extension View {
    /// Color the native split columns in place, keeping the navigation
    /// container and its toolbar safe area intact. Only the paint extends
    /// under the bars; the column's content keeps its original layout.
    @ViewBuilder func bernSidebarStyle(_ style: BernStyle) -> some View {
        if let background = style.base.background?.color {
            self.bernScrollContentBackgroundHidden()
                .background { background.ignoresSafeArea(.container) }
        } else {
            self
        }
    }

    @ViewBuilder func bernDetailStyle(_ style: BernStyle) -> some View {
        if let background = style.base.background?.color {
            self.background { background.ignoresSafeArea(.container) }
        } else {
            self
        }
    }

    /// A form standing on its page: with a background in its style, its
    /// rows take that and the form's own grouped background gives way to
    /// the page's.
    @ViewBuilder func bernFormStyle(_ style: BernStyle) -> some View {
        if let row = style.base.background?.color {
            self.bernScrollContentBackgroundHidden().environment(\.bernFormRow, row)
        } else {
            self
        }
    }

    /// The room between a form's sections; with a heading, none above it,
    /// so that it stands at the top of the page.
    @ViewBuilder func bernFormLayout(gap: Double?, rowHeight: Double?, heading: Bool) -> some View {
        #if os(iOS)
        self.bernSectionSpacing(gap).bernMinRowHeight(rowHeight).bernHeadingMargin(heading)
        #else
        self.bernMinRowHeight(rowHeight)
        #endif
    }

    #if os(iOS)
    @ViewBuilder private func bernSectionSpacing(_ gap: Double?) -> some View {
        if let gap { self.listSectionSpacing(CGFloat(gap)) } else { self }
    }

    @ViewBuilder private func bernHeadingMargin(_ heading: Bool) -> some View {
        if heading { self.contentMargins(.top, 0, for: .scrollContent) } else { self }
    }
    #endif

    @ViewBuilder private func bernMinRowHeight(_ height: Double?) -> some View {
        if let height { self.environment(\.defaultMinListRowHeight, CGFloat(height)) } else { self }
    }

    /// A section's rows inset by `x` along their sides and `y` above and
    /// below, the platform's where not given.
    @ViewBuilder func bernRowInsets(x: Double?, y: Double?) -> some View {
        #if os(iOS)
        let side = CGFloat(x ?? 16), edge = CGFloat(y ?? 11)
        self.listRowInsets(EdgeInsets(top: edge, leading: side, bottom: edge, trailing: side))
        #else
        if x != nil || y != nil {
            self.listRowInsets(EdgeInsets(top: CGFloat(y ?? 6), leading: CGFloat(x ?? 10), bottom: CGFloat(y ?? 6), trailing: CGFloat(x ?? 10)))
        } else {
            self
        }
        #endif
    }

    /// The rows of a plain section stand on the page: no background, no
    /// margins, no separators.
    func bernPlainRows() -> some View {
        self.listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets())
            .bernListRowSeparatorHidden()
    }
}

/// The rows' insets a form gives all its sections.
struct BernFormInsets: Equatable {
    var x: Double?
    var y: Double?
}

private struct BernFormInsetsKey: EnvironmentKey {
    static let defaultValue: BernFormInsets? = nil
}

extension EnvironmentValues {
    var bernFormInsets: BernFormInsets? {
        get { self[BernFormInsetsKey.self] }
        set { self[BernFormInsetsKey.self] = newValue }
    }
}

/// The rows of a section take the form's row background and insets, when
/// it gives them.
struct BernFormRows: ViewModifier {
    @Environment(\.bernFormRow) private var row
    @Environment(\.bernFormInsets) private var insets
    @ViewBuilder func body(content: Content) -> some View {
        let filled = Group {
            if let row { content.listRowBackground(row) } else { content }
        }
        if let insets {
            filled.bernRowInsets(x: insets.x, y: insets.y)
        } else {
            filled
        }
    }
}
