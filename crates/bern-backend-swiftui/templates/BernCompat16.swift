// The stand-ins of the iOS 16 front end. Generated build input; do not edit.
//
// What SwiftUI gained in iOS 17, as far as iOS 16 can have it. The source of
// the other front end is rewritten to call these; see `ios16.rs`.

import SwiftUI
import UIKit

// MARK: - Following state

/// The one signal of what any view may read without declaring it, such as
/// the language and the theme. Changes in one turn of the main loop make one
/// update.
final class BernShared: ObservableObject, @unchecked Sendable {
    static let shared = BernShared()

    private let lock = NSLock()
    private var scheduled = false

    func touch() {
        let schedule = lock.withLock {
            defer { scheduled = true }
            return !scheduled
        }
        guard schedule else { return }
        DispatchQueue.main.async { [self] in
            lock.withLock { scheduled = false }
            objectWillChange.send()
        }
    }
}

/// A property of a class every view follows.
@propertyWrapper
struct BernSharedValue<Value> {
    private var value: Value

    init(wrappedValue: Value) {
        value = wrappedValue
    }

    var wrappedValue: Value {
        get { value }
        set {
            value = newValue
            BernShared.shared.touch()
        }
    }
}

/// An object of the environment, followed by the view that reads it.
@MainActor
@propertyWrapper
struct BernObject<Object: ObservableObject & Sendable>: DynamicProperty {
    @EnvironmentObject private var object: Object

    init(_ type: Object.Type) {}

    var wrappedValue: Object { object }
}

private struct BernWindowContext16Key: EnvironmentKey {
    static let defaultValue: BernWindowContext? = nil
}

private struct BernPointerLocation16Key: EnvironmentKey {
    static let defaultValue: CGPoint? = nil
}

extension EnvironmentValues {
    var bernWindowContext16: BernWindowContext? {
        get { self[BernWindowContext16Key.self] }
        set { self[BernWindowContext16Key.self] = newValue }
    }

    var bernPointerLocation16: CGPoint? {
        get { self[BernPointerLocation16Key.self] }
        set { self[BernPointerLocation16Key.self] = newValue }
    }
}

extension View {
    func environment<Object: ObservableObject>(_ object: Object) -> some View {
        environmentObject(object)
    }

    func environment(_ context: BernWindowContext) -> some View {
        environmentObject(context).environment(\.bernWindowContext16, context)
    }
}

// MARK: - Changes

private struct BernChange16<Value: Equatable>: ViewModifier {
    let value: Value
    let initial: Bool
    let action: (Value, Value) -> Void
    @State private var appeared = false

    func body(content: Content) -> some View {
        content
            .onChange(of: value) { [value] now in action(value, now) }
            .onAppear {
                guard !appeared else { return }
                appeared = true
                if initial { action(value, value) }
            }
    }
}

extension View {
    func bernOnChange<Value: Equatable>(
        of value: Value, initial: Bool = false, _ action: @escaping (Value, Value) -> Void
    ) -> some View {
        modifier(BernChange16(value: value, initial: initial, action: action))
    }

    func bernOnChange<Value: Equatable>(
        of value: Value, initial: Bool = false, _ action: @escaping () -> Void
    ) -> some View {
        modifier(BernChange16(value: value, initial: initial) { _, _ in action() })
    }
}

/// `completion` once `animation` is over, which iOS 16 does not report: its
/// one use is a pulse of 0.08 seconds.
func bernWithAnimation(
    _ animation: Animation?, _ body: () -> Void, completion: @escaping @MainActor () -> Void
) {
    withAnimation(animation, body)
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
        MainActor.assumeIsolated { completion() }
    }
}

// MARK: - Empty states

/// What SwiftUI shows where there is nothing, as iOS 17 draws it.
struct ContentUnavailableView<Label: View, Description: View, Actions: View>: View {
    private let label: Label
    private let description: Description
    private let actions: Actions

    init(
        @ViewBuilder label: () -> Label,
        @ViewBuilder description: () -> Description = { EmptyView() },
        @ViewBuilder actions: () -> Actions = { EmptyView() }
    ) {
        self.label = label()
        self.description = description()
        self.actions = actions()
    }

    var body: some View {
        VStack(spacing: 12) {
            label
                .labelStyle(BernUnavailableLabel())
            description
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            actions
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct BernUnavailableLabel: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        VStack(spacing: 12) {
            configuration.icon
                .font(.system(size: 48))
                .foregroundStyle(.secondary)
            configuration.title
                .font(.title2.weight(.semibold))
                .multilineTextAlignment(.center)
        }
    }
}

extension ContentUnavailableView where Label == SwiftUI.Label<Text, Image>, Description == EmptyView, Actions == EmptyView {
    init(_ title: String, systemImage: String) {
        self.init { SwiftUI.Label(title, systemImage: systemImage) }
    }
}

// MARK: - Buttons

/// A bordered button in a circle, which iOS 16 has only in a capsule as wide
/// as the padding of a button with a title.
struct BernCircleButton16: ButtonStyle {
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(7)
            .background(Circle().fill(Color.primary.opacity(0.18)))
            .opacity(configuration.isPressed ? 0.6 : enabled ? 1 : 0.4)
            // As wide as a bordered button, whose padding is wider than tall.
            .padding(.horizontal, 5)
    }
}

// MARK: - Presentations

extension View {
    @ViewBuilder func bernCompactPopover() -> some View {
        if #available(iOS 16.4, *) {
            presentationCompactAdaptation(.popover)
        } else {
            self
        }
    }

    @ViewBuilder func bernPresentationBackground(@ViewBuilder content: () -> some View) -> some View {
        if #available(iOS 16.4, *) {
            presentationBackground(alignment: .center, content: content)
        } else {
            self
        }
    }

    /// An inspector, as a sheet.
    func bernInspector(isPresented: Binding<Bool>, @ViewBuilder content: @escaping () -> some View) -> some View {
        sheet(isPresented: isPresented, content: content)
    }
}

/// Closes a window of its own, which this front end has none of: every
/// window is in the app's one scene.
struct DismissWindowAction {
    func callAsFunction<Value>(id: String, value: Value) {}
}

// MARK: - Keys

struct KeyPress {
    enum Result { case handled, ignored }

    struct Phases: OptionSet {
        let rawValue: Int
        static let down = Phases(rawValue: 1)
    }

    let key: KeyEquivalent
    let modifiers: EventModifiers
}

func == (left: KeyEquivalent, right: KeyEquivalent) -> Bool {
    left.character == right.character
}

extension View {
    /// Keys go to the system: iOS 16 has no way to read them from SwiftUI.
    func bernOnKeyPress(_ key: KeyEquivalent, action: @escaping () -> KeyPress.Result) -> some View {
        self
    }

    func bernOnKeyPress(phases: KeyPress.Phases, action: @escaping (KeyPress) -> KeyPress.Result) -> some View {
        self
    }
}

// MARK: - Layout

extension View {
    func bernSafeAreaPadding(_ insets: EdgeInsets) -> some View {
        safeAreaInset(edge: .top, spacing: 0) { Color.clear.frame(height: insets.top) }
            .safeAreaInset(edge: .bottom, spacing: 0) { Color.clear.frame(height: insets.bottom) }
            .safeAreaInset(edge: .leading, spacing: 0) { Color.clear.frame(width: insets.leading) }
            .safeAreaInset(edge: .trailing, spacing: 0) { Color.clear.frame(width: insets.trailing) }
    }

    func bernListSectionSpacing(_ spacing: CGFloat) -> some View {
        self
    }
}

/// What `visualEffect` of iOS 17 is given to change.
struct BernVisualEffect16 {
    var moved = CGSize.zero

    func offset(_ offset: CGSize) -> BernVisualEffect16 {
        BernVisualEffect16(moved: CGSize(width: moved.width + offset.width, height: moved.height + offset.height))
    }
}

struct BernVisualProxy16 {
    let size: CGSize
}

private struct BernVisualEffectModifier16: ViewModifier {
    let effect: (BernVisualEffect16, BernVisualProxy16) -> BernVisualEffect16
    @State private var size: CGSize?

    func body(content: Content) -> some View {
        let result = effect(BernVisualEffect16(), BernVisualProxy16(size: size ?? .zero))
        content
            .onGeometryChange(for: CGSize.self) { $0.size } action: { size = $0 }
            .offset(result.moved)
            // Until measured, the offset is not known.
            .opacity(size == nil ? 0 : 1)
    }
}

extension View {
    func bernVisualEffect(
        _ effect: @escaping (BernVisualEffect16, BernVisualProxy16) -> BernVisualEffect16
    ) -> some View {
        modifier(BernVisualEffectModifier16(effect: effect))
    }
}

// MARK: - Scrolling

/// The animation of the scroll about to start, as a change of `position`
/// cannot carry one to `ScrollViewReader`.
@MainActor var bernScrollAnimation16: Animation?

private struct BernScrollPosition16<ID: Hashable>: ViewModifier {
    @Binding var id: ID?
    let anchor: UnitPoint

    func body(content: Content) -> some View {
        ScrollViewReader { proxy in
            content
                .onChange(of: id) { wanted in
                    guard let wanted else { return }
                    let animation = bernScrollAnimation16
                    bernScrollAnimation16 = nil
                    // After the update that changed it, so that the item
                    // is laid out.
                    DispatchQueue.main.async {
                        withAnimation(animation) { proxy.scrollTo(wanted, anchor: anchor) }
                    }
                }
                .onAppear {
                    guard let id else { return }
                    DispatchQueue.main.async { proxy.scrollTo(id, anchor: anchor) }
                }
        }
    }
}

extension View {
    func bernScrollPosition16<ID: Hashable>(id: Binding<ID?>, anchor: UnitPoint) -> some View {
        modifier(BernScrollPosition16(id: id, anchor: anchor))
    }
}

/// Where a scroll is about to stop.
struct ScrollTarget {
    var rect: CGRect
}

protocol ScrollTargetBehavior {
    typealias TargetContext = Void
    func updateTarget(_ target: inout ScrollTarget, context: TargetContext)
}

enum BernMargins16 { case scrollContent }

extension View {
    /// Scrolling stops where it stops: SwiftUI has no way to change that here.
    func bernScrollTargetBehavior(_ behavior: some ScrollTargetBehavior) -> some View {
        self
    }

    func bernContentMargins(_ edges: Edge.Set, _ length: CGFloat, for placement: BernMargins16) -> some View {
        self
    }
}

/// What `scrollTransition` of iOS 17 is given to change.
struct BernScrollEffect16 {
    func scaleEffect(_ scale: CGFloat, anchor: UnitPoint) -> BernScrollEffect16 { self }
    func opacity(_ opacity: Double) -> BernScrollEffect16 { self }
}

struct BernScrollPhase16 {
    let value: Double
}

extension View {
    /// An element stays as it is, as on the other targets without this.
    func bernScrollTransition(
        _ transition: @escaping (BernScrollEffect16, BernScrollPhase16) -> BernScrollEffect16
    ) -> some View {
        self
    }
}
