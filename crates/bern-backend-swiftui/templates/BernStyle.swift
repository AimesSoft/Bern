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
        .spring(response: period, dampingFraction: damping)
    }
}

/// Whether motion is reduced: by the system, or by `BERN_MOTION=reduce`
/// for repeatable screenshots.
private let bernForcedReduceMotion = ProcessInfo.processInfo.environment["BERN_MOTION"] == "reduce"

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
            return Self.contrasting(Color.accentColor)
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

    private static func system(_ role: String) -> Color {
        switch role {
        case "accent", "focus.indicator": return .accentColor
        case "on_accent": return contrasting(.accentColor)
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
        look.motion = other.motion ?? motion
        return look
    }
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
        if reduce || bernForcedReduceMotion {
            self.animation(.easeInOut(duration: 0.15), value: key)
        } else {
            self
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
        removal: AnyTransition?, removalMotion: BernMotion?
    ) -> some View {
        modifier(BernTransitionModifier(
            insertion: insertion, insertionMotion: insertionMotion,
            removal: removal, removalMotion: removalMotion))
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
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        if reduce || bernForcedReduceMotion {
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
        fill: BernColor?, stroke: BernColor?, strokeWidth: Double, opacity: Double
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
            layer.fill(path, with: .color((fill ?? .role("fill.strong")).color))
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
