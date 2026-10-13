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

// MARK: - Motion

/// A spring from `style!`: undamped period and damping ratio.
struct BernMotion: Equatable, Sendable {
    let period: Double
    let damping: Double

    var animation: Animation {
        .spring(response: period * bernMotionScale, dampingFraction: damping)
    }
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
        case "surface.canvas": return Color(uiColor: .systemBackground)
        case "surface.panel": return Color(uiColor: .secondarySystemBackground)
        case "surface.raised": return Color(uiColor: .tertiarySystemBackground)
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
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var hover = false
    @State private var pressed = false

    func body(content: Content) -> some View {
        let (look, motion) = style.look(selected: false, hover: hover, pressed: pressed, enabled: enabled)
        content
            .bernTransform(look, motion, key: bernStateKey(false, hover, pressed, enabled), reduce: reduce)
            .onHover { hover = $0 }
            .simultaneousGesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in pressed = true }
                    .onEnded { _ in pressed = false })
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
    @Environment(\.accessibilityReduceMotion) private var reduce
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
            .onHover { hover = $0 }
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
    @Environment(\.accessibilityReduceMotion) private var reduce
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
                .onHover { hover = $0 }
        } else {
            configuration.label
                .contentShape(Rectangle())
                .opacity(pressed ? 0.75 : enabled ? 1 : 0.5)
        }
    }
}

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

// MARK: - Theme

/// The accent the application chose at run time, read by the `accent`
/// role; views that draw with it follow its changes.
@MainActor
@Observable
final class BernThemeState {
    static let shared = BernThemeState()
    var accent: BernColor?
}

/// The accent and appearance of `WindowSet`, applied to a window: controls
/// take the accent as their tint, drawn parts through the `accent` role,
/// and the appearance is the application's (`system`, `light`, `dark`).
struct BernThemeChoice: ViewModifier {
    let accent: BernColor?
    let appearance: String?

    func body(content: Content) -> some View {
        content
            .tint(accent?.color)
            .onChange(of: accent, initial: true) { BernThemeState.shared.accent = accent }
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
    var shortcut: KeyboardShortcut? = nil
    @ViewBuilder let results: () -> Results
    @FocusState private var focused: Bool

    func body(content: Content) -> some View {
        let field = content
            .searchable(text: text, placement: placement, prompt: prompt.map { Text(verbatim: $0) })
            .searchSuggestions { results() }
            .onSubmit(of: .search) { submit?() }
        SwiftUI.Group {
            if #available(macOS 15.0, iOS 18.0, *) {
                field.searchFocused($focused)
            } else {
                field
            }
        }
        .background {
            if let shortcut {
                Button("") { focused = true }
                    .keyboardShortcut(shortcut)
                    .opacity(0)
                    .frame(width: 0, height: 0)
                    .accessibilityHidden(true)
            }
        }
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
    let target: String
    @ViewBuilder let content: Content
    @Environment(\.colorScheme) private var scheme
    @State private var showing = false

    init(
        label: String, systemImage: String?, more: String, isOn: Binding<Bool>, target: String,
        @ViewBuilder content: () -> Content
    ) {
        self.label = label
        self.systemImage = systemImage
        self.more = more
        self._isOn = isOn
        self.target = target
        self.content = content()
    }

    var body: some View {
        ControlGroup {
            Toggle(isOn: $isOn) {
                if let systemImage {
                    Label(label, systemImage: systemImage)
                } else {
                    Text(verbatim: label)
                }
            }
            .toggleStyle(.button)
            Button {
                showing = true
            } label: {
                Image(systemName: "chevron.down")
            }
            .accessibilityLabel(Text(verbatim: more))
            .help(more)
        }
        .fixedSize()
        // A control group turns its buttons into segments of one control,
        // which anchors the popover.
        .popover(isPresented: $showing, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 12) { content }
                .padding(16)
                .frame(minWidth: 240, alignment: .leading)
                // A popover also on iPhone, which would make it a sheet,
                // drawn in the appearance of where it opens, as the
                // content is.
                .presentationCompactAdaptation(.popover)
                .presentationBackground {
                    Rectangle().fill(.regularMaterial).environment(\.colorScheme, scheme)
                }
                .environment(\.colorScheme, scheme)
        }
        .bernControl(
            target, role: "split_button", label: label,
            actions: BernControlActions(
                activate: { showing = true }, setBool: { isOn = $0 }, value: { isOn ? "true" : "false" }))
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
    @Environment(\.accessibilityReduceMotion) private var reduce
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
    @Environment(\.accessibilityReduceMotion) private var reduce
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

/// A sidebar destination the style draws: hover, press and selection
/// follow the style instead of the system's highlight.
struct BernSidebarRow<Value: Hashable>: View {
    let label: String
    let systemImage: String?
    let value: Value
    @Binding var selection: Value
    let style: BernStyle
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var hover = false
    @State private var pressed = false

    var body: some View {
        let isSelected = selection == value
        let (look, motion) = style.look(selected: isSelected, hover: hover, pressed: pressed, enabled: true)
        let fill = isSelected ? look.fill : nil
        let shape = RoundedRectangle(cornerRadius: style.corner ?? 6, style: .continuous)
        let foreground = look.foreground?.color ?? fill?.onTop ?? look.background?.onTop ?? Color.primary
        HStack(spacing: 7) {
            if let systemImage {
                Image(systemName: systemImage)
                    .frame(width: 18)
            }
            Text(verbatim: label)
            Spacer(minLength: 0)
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
        .onHover { hover = $0 }
        .onTapGesture { selection = value }
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in pressed = true }
                .onEnded { _ in pressed = false })
        .listRowBackground(Color.clear)
        .listRowSeparator(.hidden)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? [.isButton, .isSelected] : .isButton)
        .accessibilityAction { selection = value }
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
        removal: AnyTransition?, removalMotion: BernMotion?, appear: BernAppear? = nil
    ) -> some View {
        modifier(BernTransitionModifier(
            insertion: insertion, insertionMotion: insertionMotion,
            removal: removal, removalMotion: removalMotion, appear: appear))
    }

    /// One of the elements sharing `id` in a window: when one replaces
    /// another, it grows from where the other was.
    func bernHero(_ id: String) -> some View {
        modifier(BernHero(id: id))
    }

    /// The namespace shared elements of one window pair up in.
    func bernHeroRoot() -> some View {
        modifier(BernHeroRoot())
    }
}

private struct BernTransitionModifier: ViewModifier {
    let insertion: AnyTransition?
    let insertionMotion: BernMotion?
    let removal: AnyTransition?
    let removalMotion: BernMotion?
    /// The appearing, for when a staggering container sets the turns.
    let appear: BernAppear?
    @Environment(\.accessibilityReduceMotion) private var reduce
    @Environment(\.bernStagger) private var stagger

    func body(content: Content) -> some View {
        if let stagger, let appear {
            content
                .modifier(BernStaggeredAppear(appear: appear, motion: insertionMotion, clock: stagger))
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

private struct BernHeroKey: EnvironmentKey {
    static let defaultValue: Namespace.ID? = nil
}

extension EnvironmentValues {
    var bernHero: Namespace.ID? {
        get { self[BernHeroKey.self] }
        set { self[BernHeroKey.self] = newValue }
    }
}

private struct BernHeroRoot: ViewModifier {
    @Namespace private var namespace

    func body(content: Content) -> some View {
        content.environment(\.bernHero, namespace)
    }
}

private struct BernHero: ViewModifier {
    let id: String
    @Environment(\.bernHero) private var namespace
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        if let namespace, !(reduce || bernForcedReduceMotion) {
            content.matchedGeometryEffect(id: id, in: namespace)
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
        fill: BernColor?, fade: String? = nil, stroke: BernColor?, strokeWidth: Double, opacity: Double
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
            layer.stroke(path, with: .color(stroke.color), lineWidth: strokeWidth)
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
        if #available(macOS 15, iOS 18, *) {
            content.textRenderer(BernMarkRenderer(color: color.color))
        } else {
            // Before text renderers there are no line positions: underline.
            content.underline(true, color: color.color)
        }
    }
}

@available(macOS 15, iOS 18, *)
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
    func bernScrollEffect(scale: Double, opacity: Double) -> some View {
        modifier(BernScrollEffect(scale: scale, opacity: opacity))
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
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        if reduce || bernForcedReduceMotion {
            content
        } else {
            let (scale, opacity) = (scale, opacity)
            content.scrollTransition(.interactive.threshold(.centered)) { effect, phase in
                let near = 1 - min(abs(phase.value), 1)
                return effect
                    .scaleEffect(1 + (scale - 1) * near)
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
        frames[key] = frame
        settle()
        if key == coming { wait() }
    }

    func remove(_ key: AnyHashable) {
        frames[key] = nil
        settle()
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
        content
            .modifier(
                BernNudgeEffect(
                    count: items?.arrived == key ? items?.arrivals ?? 0 : 0,
                    motion: items?.arrival))
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(bernScrollSpace)) } action: { frame in
                items?.report(key, frame)
            }
            .onDisappear { items?.remove(key) }
    }
}

private let bernScrollSpace = "bern.scroll"

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
    @State private var items = BernScrollItems()
    @Environment(\.accessibilityReduceMotion) private var reduce

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
    @Environment(\.accessibilityReduceMotion) private var reduce

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

    var scale: Double {
        if case .pop(let from) = self { return from }
        return 1
    }

    var offset: CGSize {
        if case .slide(let x, let y) = self { return CGSize(width: x, height: y) }
        return .zero
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

/// An element with `appear` in a staggering container: it starts hidden and
/// moves in on its turn, once the window has shown its first frame; before
/// that it shows at once. Elements inside it do not stagger on their own.
struct BernStaggeredAppear: ViewModifier {
    let appear: BernAppear
    let motion: BernMotion?
    let clock: BernStaggerClock
    @Environment(BernWindowContext.self) private var window
    @Environment(\.accessibilityReduceMotion) private var reduce
    @Environment(\.layoutDirection) private var direction
    @State private var shown = false
    @State private var place = BernPlace()

    func body(content: Content) -> some View {
        content
            .environment(\.bernStagger, nil)
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { place.frame = $0 }
            .opacity(shown ? 1 : 0)
            .scaleEffect(shown ? 1 : appear.scale)
            .offset(shown ? .zero : appear.offset)
            .onAppear {
                guard window.settled, !reduce, !bernForcedReduceMotion, !shown else {
                    var transaction = Transaction()
                    transaction.disablesAnimations = true
                    withTransaction(transaction) { shown = true }
                    return
                }
                let motion = motion
                clock.take(place: place, rightToLeft: direction == .rightToLeft) { delay in
                    withAnimation((motion?.animation ?? .default).delay(delay)) { shown = true }
                }
            }
    }
}

/// Where an element stands, kept without redrawing it.
@MainActor
final class BernPlace {
    var frame = CGRect.zero
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
        let length = vertical ? bounds.height : bounds.width
        let thickness = vertical ? bounds.width : bounds.height
        let clamp = { (value: Double) in CGFloat(min(1, max(0, value))) }
        for subview in subviews {
            if let span = subview[BernRailSpan.self] {
                let start = clamp(span.lowerBound) * length
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
                let point = clamp(subview[BernRailAt.self] ?? 0) * length
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

/// A `Stack` holding a rail: its layers paint in order, as in a `ZStack`,
/// but a rail takes the size the other layers give the stack instead of
/// adding to it. With only rails, the stack is as large as the largest.
struct BernLayers: Layout {
    let alignment: UnitPoint

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let sized = subviews.filter { !$0[BernFillsLayers.self] }
        return (sized.isEmpty ? Array(subviews) : sized).reduce(.zero) { size, subview in
            let own = subview.sizeThatFits(proposal)
            return CGSize(width: max(size.width, own.width), height: max(size.height, own.height))
        }
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

/// A button that shows its content in a popover, such as a volume slider.
/// The content exists while the popover shows. `presented`, when given,
/// opens and closes it and follows it; external entries open it with
/// `activate`. With an icon only the icon shows.
struct BernPopoverButton<Content: View>: View {
    let label: String
    let systemImage: String?
    let presented: Binding<Bool>?
    let target: String
    @ViewBuilder let content: () -> Content
    @Environment(\.colorScheme) private var scheme
    @State private var local = false

    private var isPresented: Binding<Bool> {
        presented ?? $local
    }

    var body: some View {
        let showing = isPresented
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
        .popover(isPresented: showing, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 12) { content() }
                .padding(16)
                .frame(minWidth: 240, alignment: .leading)
                // A popover also on iPhone, drawn in the appearance of
                // where it opens.
                .presentationCompactAdaptation(.popover)
                .presentationBackground {
                    Rectangle().fill(.regularMaterial).environment(\.colorScheme, scheme)
                }
                .environment(\.colorScheme, scheme)
        }
        .bernControl(
            target, role: "popover_button", label: label,
            actions: BernControlActions(
                activate: { showing.wrappedValue = true },
                value: { showing.wrappedValue ? "open" : "closed" }))
    }
}
