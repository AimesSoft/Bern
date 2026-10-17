// Bern SwiftUI runtime: the iOS part. Generated build input; do not edit.
//
// Where the app can open several windows (iPad), every Bern window but
// settings is a window of its own; settings, and on iPhone every window,
// is a virtual window in the primary window's scene, presented as iOS
// presents them: the primary window fills the scene and the other windows
// are sheets over it, the latest on top. Covered windows keep their drafts
// but cannot come forward until the windows over them close, so they
// report neither `activate` nor `close` as available. What the system
// does — a window or sheet closed by the user, a scene going to the
// background — is reported to the session runtime as a fact.

import AVKit
import SwiftUI
import UIKit
import UIKit.UIGestureRecognizerSubclass
import UniformTypeIdentifiers

// MARK: - Platform state

/// Where a window instance shows, its scene's phase, and whether its root
/// declared a close check, which decides whether its sheet may be swiped
/// away.
@MainActor
@Observable
final class BernWindowPlatform {
    var hasCloseCheck = false
    /// The root's frame in its window, in points.
    @ObservationIgnored var frame: CGRect = .zero
    /// The system window the root is in: its own, or the primary's for a
    /// sheet.
    @ObservationIgnored weak var window: UIWindow?
    @ObservationIgnored var phase: ScenePhase = .active
    /// Whether the window appears active: iPadOS keeps the scenes of
    /// several windows active, and only the front one appears active.
    @ObservationIgnored var appearsActive = true
    @ObservationIgnored var observers: [NSObjectProtocol] = []
}

/// The windows of the app: whether they can be windows of their own, and
/// the virtual ones in the primary window's scene.
@MainActor
@Observable
final class BernHostPlatform {
    /// Windows other than settings are windows of their own: the app runs
    /// where it can open several windows (iPad).
    @ObservationIgnored var nativeWindows = false
    /// Without windows of their own: whether the primary window fills the
    /// scene; closed, the scene offers to open it again.
    var primaryOpen = true
    /// Window kinds shown as sheets over the primary window, bottom first.
    var sheets: [String] = []
    @ObservationIgnored var openWindow: OpenWindowAction?
    @ObservationIgnored var dismissWindow: DismissWindowAction?
}

extension BernWindowContext {
    func reportFacts() {
        BernHost.shared.reportFacts(self)
    }

    func closeCheckChanged() {
        platform.hasCloseCheck = closeCheck != nil
    }

    /// The system window the root is in; its key status is the window's
    /// focus.
    func found(_ window: UIWindow?) {
        guard let window, window !== platform.window else { return }
        platform.window = window
        for observer in platform.observers { NotificationCenter.default.removeObserver(observer) }
        platform.observers = [UIWindow.didBecomeKeyNotification, UIWindow.didResignKeyNotification].map { name in
            NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.reportFacts() }
            }
        }
        reportFacts()
    }
}

// MARK: - Windows

extension BernHost {
    /// iPad opens windows of their own when the app allows several scenes;
    /// the engine learns it before it starts, for the window facts.
    func platformWillStart() {
        let manifest = Bundle.main.object(forInfoDictionaryKey: "UIApplicationSceneManifest") as? [String: Any]
        let multiple = manifest?["UIApplicationSupportsMultipleScenes"] as? Bool ?? false
        platform.nativeWindows = multiple && UIDevice.current.userInterfaceIdiom == .pad
        bern_app_native_windows(platform.nativeWindows ? 1 : 0)
    }

    func platformDidStart() {}

    func registerActions(openWindow: OpenWindowAction, dismissWindow: DismissWindowAction) {
        platform.openWindow = openWindow
        platform.dismissWindow = dismissWindow
    }

    /// Whether a window kind is a window of its own rather than virtual.
    func isNative(_ kindID: String) -> Bool {
        guard let kind = kinds.first(where: { $0.id == kindID }) else { return platform.nativeWindows }
        return platform.nativeWindows && !kind.isSettings && !kind.isDialog
    }

    /// Open a window kind. A window of its own opens, or comes forward when
    /// open; a virtual primary fills the scene again, and any other virtual
    /// window becomes the top sheet.
    func open(_ kindID: String) {
        guard kinds.contains(where: { $0.id == kindID }) else { return }
        if isNative(kindID) {
            // The value identifies the window, so an open one comes forward.
            platform.openWindow?(id: kindID, value: kindID)
        } else if kindID == primary {
            platform.primaryOpen = true
        } else if !platform.sheets.contains(kindID) {
            platform.sheets.append(kindID)
        }
    }

    /// Whether a sheet covers the window: sheets cover the primary window
    /// and the sheets below them.
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

    /// A window of its own comes forward; a covered window cannot, as iOS
    /// cannot bring it over the sheets above it.
    func activate(_ context: BernWindowContext) throws {
        guard !isCovered(context) else { throw covered(context) }
        if isNative(context.kindID) {
            platform.openWindow?(id: context.kindID, value: context.kindID)
        }
    }

    /// Windows of their own and the primary can close; a covered sheet holds
    /// the sheets over it.
    func checkClosable(_ context: BernWindowContext) throws {
        guard isNative(context.kindID) || context.kindID == primary || !isCovered(context) else {
            throw covered(context)
        }
    }

    func finishClose(_ context: BernWindowContext) {
        context.allowsClose = true
        if isNative(context.kindID) {
            platform.dismissWindow?(id: context.kindID, value: context.kindID)
        } else if context.kindID == primary {
            platform.primaryOpen = false
        } else if let index = platform.sheets.firstIndex(of: context.kindID) {
            platform.sheets.removeSubrange(index...)
        }
    }

    /// Report a window: visible while its scene is on screen, its frame on
    /// the screen, focused when it appears active, its system window is key
    /// in an active scene and nothing covers it. Uncovered windows activate;
    /// windows of their own, the primary and the top sheet close.
    func reportFacts(_ context: BernWindowContext) {
        guard let handle, context.generation > 0 else { return }
        let window = context.platform.window
        let phase = context.platform.phase
        let covered = isCovered(context)
        let closable = isNative(context.kindID) || context.kindID == primary || !covered
        let actions: UInt32 = (covered ? 0 : 1 << 0) | (closable ? 1 << 1 : 0)
        var frame = context.platform.frame
        if let window, let screen = window.windowScene?.screen {
            frame = window.convert(frame, to: screen.coordinateSpace)
        }
        let focused = phase == .active && context.platform.appearsActive
            && (window?.isKeyWindow ?? true) && !covered
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

/// The window root's iOS part: finds the system window, follows the
/// scene's phase, reports where the window shows, keeps SwiftUI's window
/// actions for the host, and keeps a sheet whose closing needs a decision
/// from being swiped away; its close button asks instead.
struct BernWindowPlatformModifier: ViewModifier {
    let context: BernWindowContext
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    @Environment(\.scenePhase) private var scenePhase

    func body(content: Content) -> some View {
        let confirms = context.platform.hasCloseCheck && (context.closeCheck?.needsConfirmation() ?? false)
        content
            .overlay(alignment: .topLeading) {
                BernWindowReader { [context] window in context.found(window) }
                    .frame(width: 0, height: 0)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { frame in
                context.platform.frame = frame
                context.reportFacts()
            }
            .onChange(of: scenePhase, initial: true) { _, phase in
                context.platform.phase = phase
                context.reportFacts()
            }
            .modifier(BernAppearsActiveReader(context: context))
            .interactiveDismissDisabled(confirms)
            // Files another app opens with this one ("Open In"), where
            // they are.
            .onOpenURL { url in
                if url.isFileURL { _ = url.startAccessingSecurityScopedResource() }
                BernFiles.open([url])
            }
            .onAppear {
                BernHost.shared.registerActions(openWindow: openWindow, dismissWindow: dismissWindow)
            }
    }
}

/// Follows whether the window appears active, from iOS 18; before it, an
/// active scene's key window counts as the front one.
private struct BernAppearsActiveReader: ViewModifier {
    let context: BernWindowContext

    func body(content: Content) -> some View {
        if #available(iOS 18.0, *) {
            content.modifier(Reader(context: context))
        } else {
            content
        }
    }

    @available(iOS 18.0, *)
    private struct Reader: ViewModifier {
        let context: BernWindowContext
        @Environment(\.appearsActive) private var appearsActive

        func body(content: Content) -> some View {
            content.onChange(of: appearsActive, initial: true) { _, active in
                context.platform.appearsActive = active
                context.reportFacts()
            }
        }
    }
}

/// Reports the system window a root is in.
private struct BernWindowReader: UIViewRepresentable {
    let found: @MainActor (UIWindow?) -> Void

    final class ReaderView: UIView {
        var found: (@MainActor (UIWindow?) -> Void)?

        override func didMoveToWindow() {
            super.didMoveToWindow()
            found?(window)
        }
    }

    func makeUIView(context: Context) -> ReaderView {
        let view = ReaderView()
        view.isUserInteractionEnabled = false
        view.found = found
        return view
    }

    func updateUIView(_ view: ReaderView, context: Context) {
        view.found = found
        if let window = view.window { found(window) }
    }
}

extension View {
    /// A window's declared minimum size, where people resize windows (iPad).
    func bernMinimumSize(width: CGFloat?, height: CGFloat?) -> some View {
        modifier(BernMinimumSize(width: width, height: height))
    }
}

struct BernMinimumSize: ViewModifier {
    let width: CGFloat?
    let height: CGFloat?

    func body(content: Content) -> some View {
        if BernHost.shared.platform.nativeWindows {
            content.frame(minWidth: width, minHeight: height)
        } else {
            content
        }
    }
}

// MARK: - Scene

/// The primary window's scene: the primary window fills it and the
/// virtual windows are sheets over it. `window` makes the root of a window
/// kind.
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
            // A primary window of its own closes with its scene.
            if platform.primaryOpen || platform.nativeWindows {
                window(host.primary)
            } else {
                BernClosedPrimary()
            }
        }
        .modifier(BernSheets(depth: 0, window: window))
        .modifier(BernSnapshotScheme())
        .task { host.openInitial(initial) }
        .onChange(of: platform.sheets) { host.refreshFacts() }
        .onChange(of: platform.primaryOpen) { host.refreshFacts() }
    }
}

/// What the scene shows while its primary window is closed.
struct BernClosedPrimary: View {
    var body: some View {
        let host = BernHost.shared
        let title = host.kinds.first { $0.id == host.primary }.map { BernText.string($0.title) } ?? ""
        ContentUnavailableView {
            Label(title, systemImage: "rectangle.portrait")
        } actions: {
            Button(BernText.format("bern.window.open", [title])) { host.open(host.primary) }
                .buttonStyle(.borderedProminent)
        }
    }
}

struct BernSheetItem: Identifiable {
    let id: String
}

/// The sheet at `depth`, presented over the one below it; dismissing a
/// sheet also dismisses the sheets it presented.
struct BernSheets<Window: View>: ViewModifier {
    let depth: Int
    let window: (String) -> Window

    func body(content: Content) -> some View {
        let platform = BernHost.shared.platform
        content.sheet(
            item: Binding(
                get: { platform.sheets.count > depth ? BernSheetItem(id: platform.sheets[depth]) : nil },
                set: { item in
                    // The system dismissed it, for example by a swipe: that
                    // is a fact, and the windows are already gone.
                    if item == nil, platform.sheets.count > depth {
                        platform.sheets.removeSubrange(depth...)
                    }
                })
        ) { item in
            window(item.id)
                .modifier(BernSheets(depth: depth + 1, window: window))
                .modifier(BernDialogHeight(kind: item.id))
        }
    }
}

/// A dialog window is as tall as its declared content and its title bar,
/// not a full-height sheet.
private struct BernDialogHeight: ViewModifier {
    let kind: String

    func body(content: Content) -> some View {
        if let kind = BernHost.shared.kinds.first(where: { $0.id == kind }), kind.isDialog,
            let size = kind.defaultSize
        {
            content.presentationDetents([.height(size.height + 64)])
        } else {
            content
        }
    }
}

/// A window shown as a sheet: titled, with a button that asks to close it
/// through the close protocol.
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
                    ToolbarItem(placement: .confirmationAction) {
                        Button(BernText.string("bern.window.done")) { BernHost.shared.close(kind) }
                    }
                }
        }
    }
}

/// A window of its own (iPad) whose content brings no navigation: titled,
/// in a toolbar the window controls sit in, as iPad windows are.
struct BernWindowChrome<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        NavigationStack {
            content
                .navigationTitle(BernText.string(title))
                .navigationBarTitleDisplayMode(.inline)
        }
    }
}

// MARK: - Development snapshots

/// Development aid, off unless `BERN_SNAPSHOT_DIR` is set: renders the
/// scene's windows to PNG files after launch and after each session
/// command. Video layers may render empty; the simulator's own screenshots
/// show them.
@MainActor
enum BernSnapshot {
    static let directory = ProcessInfo.processInfo.environment["BERN_SNAPSHOT_DIR"].map {
        URL(fileURLWithPath: $0, isDirectory: true)
    }
    private static var sequence = 0

    /// Development override of the appearance, `light` or `dark`.
    static var colorScheme: ColorScheme? {
        switch ProcessInfo.processInfo.environment["BERN_APPEARANCE"] {
        case "light": .light
        case "dark": .dark
        default: nil
        }
    }

    static func schedule() {
        guard directory != nil else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
            MainActor.assumeIsolated { capture() }
        }
    }

    static func capture() {
        guard let directory else { return }
        sequence += 1
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        for (index, window) in scenes.flatMap(\.windows).enumerated() where !window.isHidden {
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                _ = window.drawHierarchy(in: window.bounds, afterScreenUpdates: false)
            }
            let file = directory.appendingPathComponent(String(format: "%03d-window-%d.png", sequence, index))
            try? image.pngData()?.write(to: file)
        }
    }
}

// MARK: - Video presenter

/// Video plays with its sound also while the ring/silent switch is on
/// silent, as video apps do.
@MainActor
func bernPrepareMediaSession() {
    try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)
}

/// A view that only presents a player through its AVPlayerLayer: no
/// controls and no touch handling, so it never changes playback on its own.
final class BernPlayerView: UIView {
    override class var layerClass: AnyClass { AVPlayerLayer.self }

    private var playerLayer: AVPlayerLayer { layer as! AVPlayerLayer }

    var player: AVPlayer? {
        get { playerLayer.player }
        set { playerLayer.player = newValue }
    }

    var videoGravity: AVLayerVideoGravity {
        get { playerLayer.videoGravity }
        set { playerLayer.videoGravity = newValue }
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
    }

    required init?(coder: NSCoder) { nil }
}

/// Attaches a player to a view and reports the attachment to the hub.
struct BernVideoSurface: UIViewRepresentable {
    let playerID: String
    let window: String
    let gravity: AVLayerVideoGravity
    var corner: Double = 0
    var cornerMotion: BernMotion? = nil

    final class Coordinator {
        var token: String?
        var playerID: String?
        var window: String?
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> BernPlayerView {
        let view = BernPlayerView()
        view.videoGravity = gravity
        bernRoundCorners(view.layer, corner, motion: nil)
        attach(view, context.coordinator)
        return view
    }

    func updateUIView(_ view: BernPlayerView, context: Context) {
        view.videoGravity = gravity
        bernRoundCorners(view.layer, corner, motion: cornerMotion)
        if context.coordinator.playerID != playerID || context.coordinator.window != window {
            Self.detach(context.coordinator)
            attach(view, context.coordinator)
        }
    }

    static func dismantleUIView(_ view: BernPlayerView, coordinator: Coordinator) {
        detach(coordinator)
    }

    private func attach(_ view: BernPlayerView, _ coordinator: Coordinator) {
        let token = UUID().uuidString
        coordinator.token = token
        coordinator.playerID = playerID
        coordinator.window = window
        BernMedia.shared.attach(view, token: token, player: playerID, window: window)
    }

    private static func detach(_ coordinator: Coordinator) {
        if let token = coordinator.token { BernMedia.shared.detach(token: token) }
        coordinator.token = nil
        coordinator.playerID = nil
        coordinator.window = nil
    }
}

// MARK: - Chrome, immersion and idleness

/// A window's immersion and idleness on iOS: `immersive` hides the status
/// bar and the home indicator; `idleAfter` turns `idle` on after that many
/// seconds without a touch or pointer movement. iOS scenes have no title
/// bar, so `unified` changes nothing.
struct BernWindowBehavior: ViewModifier {
    let context: BernWindowContext
    let unified: Bool
    let idleAfter: Double?
    let immersive: Bool
    var idleEnabled: Bool = true
    @State private var timer = BernIdleTimer()

    func body(content: Content) -> some View {
        content
            .statusBarHidden(immersive)
            .persistentSystemOverlays(immersive ? .hidden : .automatic)
            // Touches are watched on the window, outside SwiftUI's gestures:
            // a drag gesture here would keep the navigation bar's buttons
            // from being tapped.
            .background(BernTouchWatcher { touched() })
            .onContinuousHover { _ in touched() }
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

// MARK: - Document picker

extension BernFiles {
    /// The picker on screen keeps its delegate here.
    private static var delegate: BernPickDelegate?

    /// Asks for one file of `kinds` with the system's document picker;
    /// `give` runs once one is chosen. The file is opened where it is and
    /// stays readable while the app runs.
    static func pick(_ kinds: String, then give: @escaping @MainActor (URL) -> Void) {
        let types = words(kinds).contains("any") ? [UTType.item] : types(kinds)
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: types, asCopy: false)
        picker.allowsMultipleSelection = false
        let delegate = BernPickDelegate(give: give)
        Self.delegate = delegate
        picker.delegate = delegate
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        var top = scenes.flatMap(\.windows).first(where: \.isKeyWindow)?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        top?.present(picker, animated: true)
    }
}

private final class BernPickDelegate: NSObject, UIDocumentPickerDelegate {
    let give: @MainActor (URL) -> Void

    init(give: @escaping @MainActor (URL) -> Void) { self.give = give }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else { return }
        _ = url.startAccessingSecurityScopedResource()
        MainActor.assumeIsolated { give(url) }
    }
}

/// Files opened with the app arrive in a scene that is already showing.
@MainActor
func bernShowPrimaryWindow() {}

/// The color scheme a snapshot run asks for. Outside one it leaves the
/// scheme alone: even `preferredColorScheme(nil)` here would override the
/// appearance the application chooses inside the window.
private struct BernSnapshotScheme: ViewModifier {
    func body(content: Content) -> some View {
        if let scheme = BernSnapshot.colorScheme {
            content.preferredColorScheme(scheme)
        } else {
            content
        }
    }
}

/// Calls `touched` whenever a touch begins anywhere in the window it is in,
/// without taking part in any gesture.
private struct BernTouchWatcher: UIViewRepresentable {
    let touched: @MainActor () -> Void

    func makeUIView(context: Context) -> BernTouchWatcherView {
        let view = BernTouchWatcherView()
        view.watcher.touched = { _ in touched() }
        return view
    }

    func updateUIView(_ view: BernTouchWatcherView, context: Context) {
        view.watcher.touched = { _ in touched() }
    }

    static func dismantleUIView(_ view: BernTouchWatcherView, coordinator: ()) {
        view.watcher.view?.removeGestureRecognizer(view.watcher)
    }
}

final class BernTouchWatcherView: UIView {
    let watcher = BernTouchRecognizer()

    override func didMoveToWindow() {
        super.didMoveToWindow()
        watcher.view?.removeGestureRecognizer(watcher)
        window?.addGestureRecognizer(watcher)
    }
}

/// Sees each touch begin and then steps aside: it never recognizes, cancels
/// or delays a touch, so every other gesture works as before.
final class BernTouchRecognizer: UIGestureRecognizer, UIGestureRecognizerDelegate {
    var touched: (@MainActor (Set<UITouch>) -> Void)?

    init() {
        super.init(target: nil, action: nil)
        cancelsTouchesInView = false
        delaysTouchesBegan = false
        delaysTouchesEnded = false
        delegate = self
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        touched?(touches)
        state = .failed
    }

    func gestureRecognizer(
        _ gestureRecognizer: UIGestureRecognizer,
        shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
    ) -> Bool { true }
}

// MARK: - Menus in a part with its own appearance

/// A part of a window with its own `appearance`. UIKit draws a menu with
/// the appearance of the control it opens from, and SwiftUI leaves its menu
/// buttons with the window's, so a touch that begins on a menu control in
/// such a part gives the control the part's appearance before its menu
/// opens: the innermost part around the touch and the control. A menu
/// opened before any touch, from the keyboard or VoiceOver, keeps the
/// window's.
@MainActor
final class BernAppearanceRegions {
    static let shared = BernAppearanceRegions()
    private var views: [BernAppearanceRegionView] = []
    /// The windows that already report their touches here.
    private let windows = NSHashTable<UIWindow>.weakObjects()
    /// The controls given a part's appearance, so one that leaves its part
    /// goes back to the window's.
    private let controls = NSHashTable<UIView>.weakObjects()

    func add(_ view: BernAppearanceRegionView) {
        views.removeAll { $0.window == nil && $0 !== view }
        if !views.contains(where: { $0 === view }) { views.append(view) }
        guard let window = view.window, !windows.contains(window) else { return }
        windows.add(window)
        let watcher = BernTouchRecognizer()
        watcher.touched = { touches in
            for touch in touches { BernAppearanceRegions.shared.touched(touch) }
        }
        window.addGestureRecognizer(watcher)
    }

    func remove(_ view: BernAppearanceRegionView) {
        views.removeAll { $0 === view }
    }

    /// Runs as the touch begins, before the control under it opens a menu.
    private func touched(_ touch: UITouch) {
        var control = touch.view
        while let view = control,
              !view.interactions.contains(where: { $0 is UIContextMenuInteraction }) {
            control = view.superview
        }
        guard let control, let window = control.window else { return }
        let frame = control.convert(control.bounds, to: nil)
        let point = touch.location(in: nil)
        var found: (area: CGFloat, dark: Bool)?
        for view in views where view.window === window && !view.isDescendant(of: control) {
            let part = view.convert(view.bounds, to: nil)
            let area = part.width * part.height
            if part.contains(point), part.contains(frame), area < (found?.area ?? .infinity) {
                found = (area, view.dark)
            }
        }
        if let found {
            control.overrideUserInterfaceStyle = found.dark ? .dark : .light
            controls.add(control)
        } else if controls.contains(control) {
            control.overrideUserInterfaceStyle = .unspecified
            controls.remove(control)
        } else {
            return
        }
        // The menu copies the control's appearance as it opens, which may
        // be before UIKit would bring the control's traits up to date.
        control.updateTraitsIfNeeded()
    }
}

/// Marks the part it is the background of; it takes no touches.
final class BernAppearanceRegionView: UIView {
    var dark = true

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil {
            BernAppearanceRegions.shared.remove(self)
        } else {
            BernAppearanceRegions.shared.add(self)
        }
    }
}

struct BernAppearanceRegion: UIViewRepresentable {
    let dark: Bool

    func makeUIView(context: Context) -> BernAppearanceRegionView {
        let view = BernAppearanceRegionView()
        view.dark = dark
        return view
    }

    func updateUIView(_ view: BernAppearanceRegionView, context: Context) {
        view.dark = dark
    }
}
