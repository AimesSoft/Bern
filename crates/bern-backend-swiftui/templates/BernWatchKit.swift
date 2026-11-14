// Bern SwiftUI runtime: the watchOS part. Generated build input; do not edit.
//
// An Apple Watch shows one scene: the primary window fills the screen and
// every other window is presented as a sheet over it, the latest on top.
// Covered windows keep their drafts and cannot come forward until the sheets
// above them close. Menus and menu/segmented pickers, which watchOS leaves
// out of SwiftUI, open native watch sheets whose lists scroll with the
// Digital Crown.

import AVFoundation
import AVKit
import SwiftUI
import WatchKit

// MARK: - Platform state

@MainActor
@Observable
final class BernWindowPlatform {
    var hasCloseCheck = false
    @ObservationIgnored var frame: CGRect = .zero
    @ObservationIgnored var phase: ScenePhase = .active
    @ObservationIgnored var appearsActive = true
}

@MainActor
@Observable
final class BernHostPlatform {
    @ObservationIgnored var nativeWindows = false
    var primaryOpen = true
    var sheets: [String] = []
}

extension BernWindowContext {
    func reportFacts() {
        BernHost.shared.reportFacts(self)
    }

    func closeCheckChanged() {
        platform.hasCloseCheck = closeCheck != nil
    }

    /// The watch screen's bounds in points before the first layout pass.
    var unmeasuredSize: CGSize {
        let size = WKInterfaceDevice.current().screenBounds.size
        if size.width > 0, size.height > 0 { return size }
        return CGSize(width: 198, height: 242)
    }
}

// MARK: - Windows

extension BernHost {
    func platformWillStart() {
        platform.nativeWindows = false
        bern_app_native_windows(0)
    }

    func platformDidStart() {}

    func isNative(_ kindID: String) -> Bool { false }

    func open(_ kindID: String) {
        guard kinds.contains(where: { $0.id == kindID }) else { return }
        if kindID == primary {
            platform.primaryOpen = true
        } else if !platform.sheets.contains(kindID) {
            platform.sheets.append(kindID)
        }
    }

    func isCovered(_ context: BernWindowContext) -> Bool {
        if let index = platform.sheets.firstIndex(of: context.kindID) {
            return index < platform.sheets.count - 1
        }
        return context.kindID == primary && !platform.sheets.isEmpty
    }

    private func covered(_ context: BernWindowContext) -> BernCommandFailure {
        BernCommandFailure(
            code: "target_unavailable",
            message: "`\(context.id)` is covered by another window; close the windows over it first")
    }

    func activate(_ context: BernWindowContext) throws {
        guard !isCovered(context) else { throw covered(context) }
    }

    func checkClosable(_ context: BernWindowContext) throws {
        guard context.kindID == primary || !isCovered(context) else {
            throw covered(context)
        }
    }

    func finishClose(_ context: BernWindowContext) {
        context.allowsClose = true
        if context.kindID == primary {
            platform.primaryOpen = false
        } else if let index = platform.sheets.firstIndex(of: context.kindID) {
            platform.sheets.removeSubrange(index...)
        }
    }

    func reportFacts(_ context: BernWindowContext) {
        guard let handle, context.generation > 0 else { return }
        let phase = context.platform.phase
        let covered = isCovered(context)
        let closable = context.kindID == primary || !covered
        let actions: UInt32 = (covered ? 0 : 1 << 0) | (closable ? 1 << 1 : 0)
        var frame = context.platform.frame
        if frame.width <= 0 || frame.height <= 0 {
            frame = CGRect(origin: .zero, size: context.unmeasuredSize)
        }
        let focused = phase == .active && context.platform.appearsActive && !covered
        _ = bern_window_facts(
            handle, context.kind, context.generation, context.commandInFlight ? 0 : 1,
            phase == .background ? 2 : 0, 0, frame.minX, frame.minY, frame.width, frame.height,
            focused ? 1 : 0, actions)
    }

    func refreshFacts() {
        for context in windows.values { context.reportFacts() }
    }

    func minimize(_ context: BernWindowContext, request: UInt64, command: UInt32) throws {
        throw unavailable(context, "minimize")
    }

    func restore(_ context: BernWindowContext, request: UInt64, command: UInt32) throws {
        throw unavailable(context, "restore")
    }

    func maximize(_ context: BernWindowContext, request: UInt64, command: UInt32) throws {
        throw unavailable(context, "maximize")
    }

    func move(_ context: BernWindowContext, x: Double, y: Double, request: UInt64, command: UInt32) throws {
        throw unavailable(context, "move")
    }

    func resize(
        _ context: BernWindowContext, width: Double, height: Double, request: UInt64, command: UInt32
    ) throws {
        throw unavailable(context, "resize")
    }
}

struct BernWindowPlatformModifier: ViewModifier {
    let context: BernWindowContext
    @Environment(\.scenePhase) private var scenePhase

    func body(content: Content) -> some View {
        let confirms = context.platform.hasCloseCheck && (context.closeCheck?.needsConfirmation() ?? false)
        content
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { frame in
                context.platform.frame = frame
                context.reportFacts()
            }
            .onChange(of: scenePhase, initial: true) { _, phase in
                context.platform.phase = phase
                context.reportFacts()
            }
            .interactiveDismissDisabled(confirms)
            .onOpenURL { url in
                BernFiles.open([url])
            }
    }
}

// MARK: - Scene

struct BernVirtualHost<Window: View>: View {
    let initial: [String]
    let window: (String) -> Window

    init(initial: [String], @ViewBuilder window: @escaping (String) -> Window) {
        self.initial = initial
        self.window = window
    }

    var body: some View {
        let host = BernHost.shared
        let platform = host.platform
        Group {
            if platform.primaryOpen {
                window(host.primary)
            } else {
                BernClosedPrimary()
            }
        }
        .bernWindowInsetsRoot()
        .modifier(BernSheets(depth: 0, window: window))
        .modifier(BernSnapshotScheme())
        .task { host.openInitial(initial) }
        .onChange(of: platform.sheets) { host.refreshFacts() }
        .onChange(of: platform.primaryOpen) { host.refreshFacts() }
    }
}

struct BernClosedPrimary: View {
    var body: some View {
        let host = BernHost.shared
        let title = host.kinds.first { $0.id == host.primary }.map { BernText.string($0.title) } ?? ""
        ContentUnavailableView {
            Label(title, systemImage: "applewatch")
        } actions: {
            Button(BernText.format("bern.window.open", [title])) { host.open(host.primary) }
                .buttonStyle(.borderedProminent)
        }
    }
}

struct BernSheetItem: Identifiable {
    let id: String
}

struct BernSheets<Window: View>: ViewModifier {
    let depth: Int
    let window: (String) -> Window

    func body(content: Content) -> some View {
        let platform = BernHost.shared.platform
        let item = Binding(
            get: { platform.sheets.count > depth ? BernSheetItem(id: platform.sheets[depth]) : nil },
            set: { (item: BernSheetItem?) in
                if item == nil, platform.sheets.count > depth {
                    platform.sheets.removeSubrange(depth...)
                }
            })
        content.sheet(item: item) { item in
            window(item.id)
                .modifier(BernSheets(depth: depth + 1, window: window))
                .modifier(BernDialogHeight(kind: item.id))
        }
    }
}

private struct BernDialogHeight: ViewModifier {
    let kind: String

    func body(content: Content) -> some View {
        if let kind = BernHost.shared.kinds.first(where: { $0.id == kind }), kind.isDialog,
            let size = kind.defaultSize
        {
            content.presentationDetents([.height(size.height + 44)])
        } else {
            content
        }
    }
}

/// A window shown as a sheet on watchOS: titled, with a Done action in the
/// toolbar so windows whose close check blocks interactive dismissal can
/// still ask to close through the close protocol.
struct BernSheetChrome<Content: View>: View {
    let kind: String
    let title: String
    let content: Content

    init(kind: String, title: String, @ViewBuilder content: () -> Content) {
        self.kind = kind
        self.title = title
        self.content = content()
    }

    var body: some View {
        NavigationStack {
            content
                .navigationTitle(BernText.string(title))
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button {
                            BernHost.shared.close(kind)
                        } label: {
                            Label(BernText.string("bern.window.done"), systemImage: "xmark")
                        }
                    }
                }
        }
    }
}

// MARK: - Development snapshots

@MainActor
enum BernSnapshot {
    static let directory = ProcessInfo.processInfo.environment["BERN_SNAPSHOT_DIR"].map {
        URL(fileURLWithPath: $0, isDirectory: true)
    }

    static var colorScheme: ColorScheme? {
        switch ProcessInfo.processInfo.environment["BERN_APPEARANCE"] {
        case "light": .light
        case "dark": .dark
        default: nil
        }
    }

    static func schedule() {}
    static func capture() {}
}

private struct BernSnapshotScheme: ViewModifier {
    func body(content: Content) -> some View {
        if let scheme = BernSnapshot.colorScheme {
            content.preferredColorScheme(scheme)
        } else {
            content
        }
    }
}

// MARK: - Video presenter

@MainActor
func bernPrepareMediaSession() {
    try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
}

/// Holds the AVPlayer bound to a watchOS `VideoPlayer` view.
@MainActor
@Observable
final class BernPlayerView {
    var player: AVPlayer?
}

struct BernVideoSurface: View {
    let playerID: String
    let window: String
    var corner: Double = 0
    @State private var holder = BernPlayerView()
    @State private var token: String?

    var body: some View {
        VideoPlayer(player: holder.player)
            .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
            .onAppear { attach() }
            .onChange(of: "\(window)|\(playerID)") {
                detach()
                attach()
            }
            .onDisappear { detach() }
    }

    private func attach() {
        let next = UUID().uuidString
        token = next
        BernMedia.shared.attach(holder, token: next, player: playerID, window: window)
    }

    private func detach() {
        if let token { BernMedia.shared.detach(token: token) }
        token = nil
    }
}

// MARK: - Watch controls

/// watchOS has no pull-down `Menu`: the button opens a watch sheet whose
/// rows scroll with the Digital Crown and dismiss on selection.
struct BernWatchMenu<LabelContent: View, Items: View>: View {
    let labelText: String
    @ViewBuilder let label: () -> LabelContent
    @ViewBuilder let items: (@escaping () -> Void) -> Items
    @State private var presented = false

    var body: some View {
        Button {
            presented = true
        } label: {
            label()
        }
        .sheet(isPresented: $presented) {
            NavigationStack {
                List {
                    items { presented = false }
                }
                .navigationTitle(labelText)
                .navigationBarTitleDisplayMode(.inline)
            }
        }
    }
}

/// A long-press action menu on watchOS, where `View.contextMenu` was
/// deprecated with Force Touch in watchOS 7.
struct BernWatchContextMenu<Items: View>: ViewModifier {
    let title: String
    @ViewBuilder let items: (@escaping () -> Void) -> Items
    @State private var presented = false

    func body(content: Content) -> some View {
        content
            .simultaneousGesture(
                LongPressGesture(minimumDuration: 0.5).onEnded { _ in presented = true }
            )
            .sheet(isPresented: $presented) {
                NavigationStack {
                    List {
                        items { presented = false }
                    }
                    .navigationTitle(title)
                    .navigationBarTitleDisplayMode(.inline)
                }
            }
    }
}

/// A choice control on watchOS where `.menu` and `.segmented` picker styles
/// do not exist: shows the current option and opens a Crown-scrollable sheet
/// of options when tapped.
struct BernWatchPicker<Value: Hashable>: View {
    @Binding var selection: Value
    let label: String
    let options: [(String, Value)]
    var labelHidden = false
    var capsule = false
    var height: CGFloat = 32
    @State private var presented = false

    private var selectedLabel: String {
        options.first { $0.1 == selection }?.0 ?? ""
    }

    var body: some View {
        Group {
            if capsule {
                Button {
                    presented = true
                } label: {
                    HStack(spacing: 4) {
                        Text(verbatim: selectedLabel).lineLimit(1)
                        Image(systemName: "chevron.down")
                            .font(.system(size: 10, weight: .semibold))
                            .opacity(0.6)
                    }
                }
                .bernCapsuleControl(height: height)
                .accessibilityLabel(Text(verbatim: label))
            } else if labelHidden {
                Button {
                    presented = true
                } label: {
                    HStack {
                        Text(verbatim: selectedLabel).lineLimit(1)
                        Spacer(minLength: 4)
                        Image(systemName: "chevron.up.chevron.down")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
                .accessibilityLabel(Text(verbatim: label))
            } else {
                Button {
                    presented = true
                } label: {
                    LabeledContent {
                        Text(verbatim: selectedLabel)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    } label: {
                        Text(verbatim: label)
                    }
                }
            }
        }
        .sheet(isPresented: $presented) {
            NavigationStack {
                List {
                    ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                        Button {
                            selection = option.1
                            presented = false
                        } label: {
                            HStack {
                                Text(verbatim: option.0)
                                Spacer(minLength: 4)
                                if option.1 == selection {
                                    Image(systemName: "checkmark")
                                        .foregroundStyle(Color.accentColor)
                                }
                            }
                        }
                    }
                }
                .navigationTitle(label)
                .navigationBarTitleDisplayMode(.inline)
            }
        }
    }
}

/// watchOS's inline search field: opens the system dictation/scribble/keyboard
/// sheet when focused.
struct BernSearchInput: View {
    let text: Binding<String>
    let prompt: String
    var fontSize: CGFloat? = nil
    let editing: Binding<Bool>
    let focusRequest: Int
    let up: () -> Void
    let down: () -> Void
    let submit: () -> Void
    let cancel: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        TextField(prompt, text: text)
            .font(fontSize.map { .system(size: $0) })
            .submitLabel(.search)
            .focused($focused)
            .onSubmit(submit)
            .onChange(of: focused) { _, now in editing.wrappedValue = now }
            .onChange(of: focusRequest) { _, _ in focused = true }
    }
}

// MARK: - Platform hooks

struct BernFocusRegion: ViewModifier {
    func body(content: Content) -> some View { content }
}

@MainActor
func bernEndEditing() {}

extension View {
    func bernIdleArea(_ seconds: Double) -> some View { self }

    @ViewBuilder func bernTabAccessory(_ shown: Bool, _ accessory: AnyView) -> some View {
        overlay {
            if shown {
                ZStack(alignment: .bottom) {
                    Color.clear.allowsHitTesting(false)
                    let bar = accessory
                        .environment(\.bernOnBar, true)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .frame(minHeight: 32)
                        .frame(maxWidth: .infinity)
                    if #available(watchOS 26.0, *) {
                        bar.glassEffect(.regular, in: Capsule())
                            .padding(.horizontal, 8)
                            .padding(.bottom, 14)
                    } else {
                        bar.background(.regularMaterial, in: Capsule())
                            .padding(.horizontal, 8)
                            .padding(.bottom, 14)
                    }
                }
                .ignoresSafeArea(.container, edges: .bottom)
            }
        }
    }

    @ViewBuilder func bernTabAccessoryInset(_ shown: Bool, _ accessory: AnyView) -> some View {
        safeAreaInset(edge: .bottom, spacing: 0) {
            if shown {
                Color.clear.frame(height: 22).allowsHitTesting(false)
            }
        }
    }
}

struct BernWindowBehavior: ViewModifier {
    let context: BernWindowContext
    let unified: Bool
    let idleAfter: Double?
    let immersive: Bool
    var idleEnabled: Bool = true
    var landscape: Bool? = nil
    @State private var timer = BernIdleTimer()

    func body(content: Content) -> some View {
        content
            .ignoresSafeArea(.container, edges: immersive ? .all : [])
            .persistentSystemOverlays(immersive ? .hidden : .automatic)
            .simultaneousGesture(
                DragGesture(minimumDistance: 0).onChanged { _ in touched() }
            )
            .onAppear { touched() }
            .onChange(of: idleEnabled) { _, _ in touched() }
            .onDisappear { timer.stop(context) }
    }

    private func touched() {
        guard idleEnabled, let idleAfter else { timer.stop(context); return }
        timer.reset(context, after: idleAfter)
    }
}

@MainActor
final class BernIdleTimer {
    private var timer: Timer?

    func stop(_ context: BernWindowContext) {
        timer?.invalidate()
        timer = nil
        context.idle = false
    }

    func reset(_ context: BernWindowContext, after seconds: Double) {
        if context.idle {
            context.idle = false
        }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: seconds, repeats: false) { [weak context] _ in
            MainActor.assumeIsolated { context?.idle = true }
        }
    }
}

extension BernFiles {
    static func pick(_ kinds: String, then give: @escaping @MainActor (URL) -> Void) {}
}

@MainActor
func bernShowPrimaryWindow() {}

struct BernAppearanceRegion: View {
    let dark: Bool
    var body: some View { Color.clear }
}

@MainActor enum BernOrientation {
    static func turn(landscape: Bool?) {}
}

struct BernIconChoice: ViewModifier {
    let icon: String?
    func body(content: Content) -> some View { content }
}

enum BernPhotos {
    static func save(_ path: String) -> Int32 { 1 }
}

@MainActor
enum BernClipboard {
    nonisolated static let available = false
    static func copy(_ text: String) {}
    static func paste(then give: @escaping @MainActor (String) -> Void) {}
}
