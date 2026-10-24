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
    #if !os(tvOS)
    @ObservationIgnored var openWindow: OpenWindowAction?
    @ObservationIgnored var dismissWindow: DismissWindowAction?
    #endif
}

extension BernWindowContext {
    func reportFacts() {
        BernHost.shared.reportFacts(self)
    }

    func closeCheckChanged() {
        platform.hasCloseCheck = closeCheck != nil
    }

    /// The room the window has before its first layout measures it. Where
    /// every window is in the one scene, the scene's size: the primary
    /// window fills it, and nothing over it is larger. Windows of their
    /// own (iPad) may open at any size, so the largest screen's. Never
    /// less than the window gets, so a wide window never builds a narrow
    /// layout first.
    var unmeasuredSize: CGSize {
        let scenes = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .filter { $0.session.role == .windowApplication }
        if !BernHost.shared.platform.nativeWindows, scenes.count == 1 {
            let size = scenes[0].coordinateSpace.bounds.size
            if size.width > 0, size.height > 0 { return size }
        }
        let screens = scenes.map(\.screen.bounds.size)
        guard let width = screens.map(\.width).max(), let height = screens.map(\.height).max(), width > 0 else {
            return CGSize(width: CGFloat.infinity, height: CGFloat.infinity)
        }
        return CGSize(width: width, height: height)
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

    #if !os(tvOS)
    func registerActions(openWindow: OpenWindowAction, dismissWindow: DismissWindowAction) {
        platform.openWindow = openWindow
        platform.dismissWindow = dismissWindow
    }
    #endif

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
            #if !os(tvOS)
            platform.openWindow?(id: kindID, value: kindID)
            #endif
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
        #if !os(tvOS)
        if isNative(context.kindID) {
            platform.openWindow?(id: context.kindID, value: context.kindID)
        }
        #endif
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
            #if !os(tvOS)
            platform.dismissWindow?(id: context.kindID, value: context.kindID)
            #endif
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
    #if !os(tvOS)
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    #endif
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
            #if os(tvOS)
            .modifier(BernRemoteCommands(context: context))
            .modifier(BernFocusScope())
            #endif
            .interactiveDismissDisabled(confirms)
            // Files another app opens with this one ("Open In"), where
            // they are.
            .onOpenURL { url in
                if url.isFileURL { _ = url.startAccessingSecurityScopedResource() }
                BernFiles.open([url])
            }
            #if !os(tvOS)
            .onAppear {
                BernHost.shared.registerActions(openWindow: openWindow, dismissWindow: dismissWindow)
            }
            #endif
    }
}

/// Follows whether the window appears active, from iOS 18; before it, an
/// active scene's key window counts as the front one.
private struct BernAppearsActiveReader: ViewModifier {
    let context: BernWindowContext

    func body(content: Content) -> some View {
        if #available(iOS 18.0, tvOS 18.0, *) {
            content.modifier(Reader(context: context))
        } else {
            content
        }
    }

    @available(iOS 18.0, tvOS 18.0, *)
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
        let item = Binding(
            get: { platform.sheets.count > depth ? BernSheetItem(id: platform.sheets[depth]) : nil },
            set: { (item: BernSheetItem?) in
                // The system dismissed it, for example by a swipe or the
                // remote's Back: that is a fact, and the windows are
                // already gone.
                if item == nil, platform.sheets.count > depth {
                    platform.sheets.removeSubrange(depth...)
                }
            })
        #if os(tvOS)
        // A television shows another window over the whole screen, as
        // tvOS presents its own screens.
        content.fullScreenCover(item: item) { item in
            window(item.id)
                .modifier(BernSheets(depth: depth + 1, window: window))
                .background(.background)
        }
        #else
        content.sheet(item: item) { item in
            window(item.id)
                .modifier(BernSheets(depth: depth + 1, window: window))
                .modifier(BernDialogHeight(kind: item.id))
        }
        #endif
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
                #if !os(tvOS)
                .navigationBarTitleDisplayMode(.inline)
                #endif
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
                #if !os(tvOS)
                .navigationBarTitleDisplayMode(.inline)
                #endif
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

/// The surface a video view gives the application's media kernel: a view
/// whose layer is the `CAMetalLayer` the kernel draws into. It takes no
/// touches; the views around it do.
final class BernKernelView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }

    var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
    /// How much the scenes around the view enlarge it.
    var sceneScale = 1.0 {
        didSet { if sceneScale != oldValue { report() } }
    }
    /// Called when the size on screen changes.
    var resized: ((BernSurfaceSize) -> Void)?
    private var reported: BernSurfaceSize?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        metalLayer.isOpaque = true
        backgroundColor = .black
    }

    required init?(coder: NSCoder) { nil }

    /// The size on screen, in the pixels of the view's screen.
    var size: BernSurfaceSize {
        let backing = window?.screen.scale ?? traitCollection.displayScale
        return BernSurfaceSize(points: bounds.size, backing: Double(backing), sceneScale: sceneScale)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        report()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        report()
    }

    private func report() {
        let size = self.size
        guard size != reported else { return }
        reported = size
        resized?(size)
    }
}

/// Gives the application's media kernel a surface to show a player in,
/// with a display link of the screen.
struct BernKernelVideoSurface: UIViewRepresentable {
    let playerID: String
    let window: String
    let fit: BernVideoFit
    var corner: Double = 0
    var cornerMotion: BernMotion? = nil
    var sceneScale = 1.0

    final class Coordinator {
        var token: String?
        var playerID: String?
        var window: String?
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> BernKernelView {
        let view = BernKernelView()
        view.sceneScale = sceneScale
        bernRoundCorners(view.layer, corner, motion: nil)
        attach(view, context.coordinator)
        return view
    }

    func updateUIView(_ view: BernKernelView, context: Context) {
        view.sceneScale = sceneScale
        bernRoundCorners(view.layer, corner, motion: cornerMotion)
        if context.coordinator.playerID != playerID || context.coordinator.window != window {
            Self.detach(view, context.coordinator)
            attach(view, context.coordinator)
        }
    }

    static func dismantleUIView(_ view: BernKernelView, coordinator: Coordinator) {
        detach(view, coordinator)
    }

    private func attach(_ view: BernKernelView, _ coordinator: Coordinator) {
        let token = UUID().uuidString
        coordinator.token = token
        coordinator.playerID = playerID
        coordinator.window = window
        let frames = BernKernelFrames(token: token)
        let link = CADisplayLink(target: frames, selector: #selector(BernKernelFrames.step(_:)))
        view.resized = { size in BernMedia.shared.resizeKernel(token: token, size: size) }
        BernMedia.shared.attachKernel(
            view.metalLayer, size: view.size, fit: fit, token: token, player: playerID, window: window,
            link: link, frames: frames)
    }

    private static func detach(_ view: BernKernelView, _ coordinator: Coordinator) {
        view.resized = nil
        if let token = coordinator.token { BernMedia.shared.detachKernel(token: token) }
        coordinator.token = nil
        coordinator.playerID = nil
        coordinator.window = nil
    }
}

/// The first touch outside an editing region only dismisses its keyboard.
struct BernFocusRegion: ViewModifier {
    func body(content: Content) -> some View {
        content.background(BernFocusRegionPress())
    }
}

struct BernFocusRegionPress: UIViewRepresentable {
    func makeUIView(context: Context) -> Observer { Observer() }
    func updateUIView(_ view: Observer, context: Context) {}
    static func dismantleUIView(_ view: Observer, coordinator: ()) { view.stop() }

    final class Observer: UIView {
        private var press: OutsidePress?
        override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
        override func didMoveToWindow() {
            super.didMoveToWindow()
            stop()
            guard let window else { return }
            let press = OutsidePress()
            press.region = self
            press.cancelsTouchesInView = true
            window.addGestureRecognizer(press)
            self.press = press
        }
        func stop() {
            if let press { press.view?.removeGestureRecognizer(press) }
            press = nil
        }
    }

    final class OutsidePress: UIGestureRecognizer {
        weak var region: UIView?
        private func editor(in view: UIView) -> UIView? {
            if view.isFirstResponder && view is UITextInput { return view }
            for child in view.subviews {
                if let found = editor(in: child) { return found }
            }
            return nil
        }
        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            guard let region, let window = region.window, !region.isHidden,
                  let touch = touches.first, let field = editor(in: window),
                  region.bounds.contains(region.convert(CGPoint(x: field.bounds.midX, y: field.bounds.midY), from: field)),
                  !region.bounds.contains(touch.location(in: region)) else {
                state = .failed
                return
            }
            // Recognize before the target's release action; UIKit cancels the
            // underlying button/gesture while the keyboard gives up focus.
            state = .began
            field.resignFirstResponder()
        }
        override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
            if state == .began || state == .changed { state = .changed }
        }
        override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) { state = .ended }
        override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { state = .cancelled }
    }
}

/// `submit: clear`: the field gives up the keyboard.
@MainActor
func bernEndEditing() {
    UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
}

extension View {
    /// `idle_area` matters where a mouse rests; touch and the remote keep
    /// the window's idleness as it was.
    func bernIdleArea(_ seconds: Double) -> some View { self }
}

// MARK: - Tab accessory

extension View {
    /// `TabAccessory` on iOS 26.1 and later: the tab view's bottom
    /// accessory, the system's glass bar above the tab bar, as a music
    /// app shows what is playing.
    @ViewBuilder func bernTabAccessory(_ shown: Bool, _ accessory: AnyView) -> some View {
        #if os(tvOS)
        self
        #else
        if #available(iOS 26.1, tvOS 26.1, *) {
            tabViewBottomAccessory(isEnabled: shown) { accessory.environment(\.bernOnBar, true) }
        } else {
            self
        }
        #endif
    }

    /// Before iOS 26.1, each page keeps the bar at its bottom, above the
    /// tab bar, on a material capsule; tvOS, whose tabs are at the top,
    /// always does.
    @ViewBuilder func bernTabAccessoryInset(_ shown: Bool, _ accessory: AnyView) -> some View {
        if bernTabAccessoryIsSystem {
            self
        } else {
            safeAreaInset(edge: .bottom) {
                if shown {
                    accessory
                        .environment(\.bernOnBar, true)
                        .frame(height: 48)
                        .frame(maxWidth: .infinity)
                        .background(.regularMaterial, in: Capsule())
                        .padding(.horizontal, 16)
                        .padding(.bottom, 8)
                }
            }
        }
    }
}

/// Whether the system shows a tab view's bottom accessory: iOS 26.1 and
/// later.
private var bernTabAccessoryIsSystem: Bool {
    #if os(tvOS)
    false
    #else
    if #available(iOS 26.1, tvOS 26.1, *) { true } else { false }
    #endif
}

// MARK: - Chrome, immersion and idleness

/// A window's immersion and idleness on iOS: `immersive` hides the status
/// bar and the home indicator and lays the content over the whole screen,
/// under the safe areas, as a video player does; `idleAfter` turns `idle`
/// on after that many seconds without a touch or pointer movement. iOS
/// scenes have no title bar, so `unified` changes nothing. On tvOS a
/// television has no bars to hide: `immersive` lays the content over the
/// overscan margins, and a press of the remote or a touch of its clickpad
/// is what keeps the window from going idle.
struct BernWindowBehavior: ViewModifier {
    let context: BernWindowContext
    let unified: Bool
    let idleAfter: Double?
    let immersive: Bool
    var idleEnabled: Bool = true
    /// On a phone: landscape while true, portrait while false; `nil` lets
    /// the window follow the device.
    var landscape: Bool? = nil
    @State private var timer = BernIdleTimer()

    func body(content: Content) -> some View {
        content
            .ignoresSafeArea(.container, edges: immersive ? .all : [])
            .onChange(of: landscape, initial: true) { _, wanted in BernOrientation.turn(landscape: wanted) }
            #if !os(tvOS)
            .statusBarHidden(immersive)
            #endif
            .persistentSystemOverlays(immersive ? .hidden : .automatic)
            // Touches are watched on the window, outside SwiftUI's gestures:
            // a drag gesture here would keep the navigation bar's buttons
            // from being tapped.
            .background(BernTouchWatcher { touched() })
            #if os(tvOS)
            .onReceive(NotificationCenter.default.publisher(for: bernRemoteActivity)) { note in
                if note.object as? String == context.kindID { touched() }
            }
            #endif
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

#if os(tvOS)
extension BernFiles {
    /// tvOS has no document picker and no files to pick from; the target
    /// does not list `file_pick`, so generated code never asks.
    static func pick(_ kinds: String, then give: @escaping @MainActor (URL) -> Void) {}
}
#else
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
#endif

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

    #if os(tvOS)
    /// A press of the remote's buttons counts as a touch: a television
    /// has nothing else.
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent) {
        touched?([])
        super.pressesBegan(presses, with: event)
    }
    #endif

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

// MARK: - Search field

#if os(tvOS)
/// tvOS's search field: a text field, which opens the system's keyboard
/// screen when selected, as tvOS fields do.
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
            .bernFieldLook()
            .focused($focused)
            .onSubmit(submit)
            .onChange(of: focused) { _, now in editing.wrappedValue = now }
            .onChange(of: focusRequest) { _, _ in focused = true }
    }
}
#else
/// The platform's search field for a `Search` in the page: UIKit's own,
/// without a search bar's margins. Chinese input is passed on only once
/// it is composed.
struct BernSearchInput: UIViewRepresentable {
    let text: Binding<String>
    let prompt: String
    var fontSize: CGFloat? = nil
    let editing: Binding<Bool>
    let focusRequest: Int
    let up: () -> Void
    let down: () -> Void
    let submit: () -> Void
    let cancel: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UISearchTextField {
        let field = UISearchTextField()
        if let fontSize { field.font = .systemFont(ofSize: fontSize) }
        // On iOS 26 the field stands on the system's glass capsule.
        if #available(iOS 26.0, tvOS 26.0, *) {
            field.borderStyle = .none
            field.backgroundColor = .clear
        }
        field.placeholder = prompt
        field.text = text.wrappedValue
        field.returnKeyType = .search
        field.delegate = context.coordinator
        field.addTarget(context.coordinator, action: #selector(Coordinator.changed(_:)), for: .editingChanged)
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.accessibilityLabel = prompt
        return field
    }

    func updateUIView(_ field: UISearchTextField, context: Context) {
        context.coordinator.owner = self
        field.placeholder = prompt
        if field.text != text.wrappedValue && field.markedTextRange == nil { field.text = text.wrappedValue }
        if context.coordinator.focusRequest != focusRequest {
            context.coordinator.focusRequest = focusRequest
            DispatchQueue.main.async { field.becomeFirstResponder() }
        }
    }

    @MainActor final class Coordinator: NSObject, UITextFieldDelegate {
        var owner: BernSearchInput
        var focusRequest: Int
        init(_ owner: BernSearchInput) {
            self.owner = owner
            focusRequest = owner.focusRequest
        }
        @objc func changed(_ field: UITextField) {
            guard field.markedTextRange == nil else { return }
            owner.text.wrappedValue = field.text ?? ""
        }
        func textFieldDidBeginEditing(_ field: UITextField) { owner.editing.wrappedValue = true }
        func textFieldDidEndEditing(_ field: UITextField) { owner.editing.wrappedValue = false }
        func textFieldShouldReturn(_ field: UITextField) -> Bool {
            owner.submit()
            field.resignFirstResponder()
            return false
        }
        func textFieldShouldClear(_ field: UITextField) -> Bool {
            owner.cancel()
            return true
        }
    }
}

#endif

// MARK: - Orientation

#if os(tvOS)
/// A television has no orientations to turn to.
@MainActor enum BernOrientation {
    static func turn(landscape: Bool?) {}
}
#else
/// The orientations the phone may show the application in: all of them,
/// until a window asks for landscape or portrait.
@MainActor enum BernOrientation {
    static var allowed: UIInterfaceOrientationMask = .all

    /// Turn the phone's windows to landscape or portrait; iPads and a
    /// window that asks nothing keep the device's orientation.
    static func turn(landscape: Bool?) {
        guard let landscape, UIDevice.current.userInterfaceIdiom == .phone else { return }
        allowed = landscape ? .landscape : .portrait
        for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
            for window in scene.windows {
                window.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
            }
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: allowed)) { _ in }
        }
    }
}

/// Answers the system with the orientations the windows allow now.
final class BernOrientationDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, supportedInterfaceOrientationsFor window: UIWindow?) -> UIInterfaceOrientationMask {
        MainActor.assumeIsolated { BernOrientation.allowed }
    }
}
#endif

#if os(tvOS)
// MARK: - Television

/// The Siri Remote's buttons that stand for keys of `shortcut`s.
enum BernRemoteKey: Hashable {
    case back, playPause, left, right, up, down
}

/// The remote shortcuts on screen, the latest shown last. The root of each
/// window sends the remote's commands to its window's latest shortcut for
/// the button: Back, Play/Pause, and the clickpad's edges when the press
/// moves the focus nowhere. A window without a shortcut for Back leaves
/// the button to the system, which closes sheets and leaves the app.
@MainActor
@Observable
final class BernRemoteShortcuts {
    static let shared = BernRemoteShortcuts()

    final class Entry {
        let key: BernRemoteKey
        let window: String
        var action: () -> Void
        init(key: BernRemoteKey, window: String, action: @escaping () -> Void) {
            self.key = key
            self.window = window
            self.action = action
        }
    }

    private var entries: [Entry] = []

    func add(_ entry: Entry) {
        entries.append(entry)
    }

    func remove(_ entry: Entry) {
        entries.removeAll { $0 === entry }
    }

    /// The action for the button in the window, if a shortcut on screen
    /// has one.
    func action(_ key: BernRemoteKey, window: String) -> (() -> Void)? {
        guard let entry = entries.last(where: { $0.key == key && $0.window == window }) else { return nil }
        return { entry.action() }
    }
}

extension View {
    /// While on screen and enabled, the remote's button for `key` runs
    /// `action`.
    /// A key that follows state may be none for a while: the button
    /// does nothing then.
    func bernRemoteShortcut(_ key: BernRemoteKey?, enabled: Bool, action: @escaping () -> Void) -> some View {
        modifier(BernRemoteShortcut(key: key, enabled: enabled, action: action))
    }
}

private struct BernRemoteShortcut: ViewModifier {
    let key: BernRemoteKey?
    let enabled: Bool
    let action: () -> Void
    @Environment(BernWindowContext.self) private var window
    @State private var entry: BernRemoteShortcuts.Entry?
    @State private var shown = false

    func body(content: Content) -> some View {
        // The latest action, with the state it reads now.
        let _ = entry?.action = action
        content
            .onAppear {
                shown = true
                update()
            }
            .onDisappear {
                shown = false
                update()
            }
            .onChange(of: enabled) { update() }
            .onChange(of: key) { update() }
    }

    private func update() {
        let wanted = shown && enabled ? key : nil
        if let entry, entry.key != wanted {
            BernRemoteShortcuts.shared.remove(entry)
            self.entry = nil
        }
        if let wanted, entry == nil {
            let made = BernRemoteShortcuts.Entry(key: wanted, window: window.kindID, action: action)
            BernRemoteShortcuts.shared.add(made)
            entry = made
        }
    }
}

/// A press of the remote that no shortcut took: input that keeps the
/// window from going idle. The notification's object is the window's id.
let bernRemoteActivity = Notification.Name("BernRemoteActivity")

/// The root of a window on tvOS: the remote's commands reach the window's
/// shortcuts. The arrows act only where they move the focus nowhere, as
/// SwiftUI sends a move command to the focused view's ancestors then; an
/// arrow no shortcut takes is input that wakes an idle window.
struct BernRemoteCommands: ViewModifier {
    let context: BernWindowContext

    func body(content: Content) -> some View {
        let remote = BernRemoteShortcuts.shared
        let window = context.kindID
        content
            .onExitCommand(perform: remote.action(.back, window: window))
            .onPlayPauseCommand(perform: remote.action(.playPause, window: window))
            .onMoveCommand { direction in
                let key: BernRemoteKey
                switch direction {
                case .left: key = .left
                case .right: key = .right
                case .up: key = .up
                case .down: key = .down
                @unknown default: return
                }
                if let action = remote.action(key, window: window) {
                    action()
                } else {
                    NotificationCenter.default.post(name: bernRemoteActivity, object: window)
                }
            }
    }
}

/// tvOS has no switch, only a button that says On or Off. The switch of
/// the other platforms, drawn: the label, and at the row's end a capsule
/// filled with the accent while on, its white knob at the end it stands
/// for. The remote's select flips it; under the focus a ring stands
/// around the switch and it grows by a tenth. A scene laid out at the
/// design's size takes it at that size (64 × 28), the screen's own layout
/// half as large again.
struct BernFocusSwitchStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        BernFocusSwitch(configuration: configuration)
    }
}

private struct BernFocusSwitch: View {
    let configuration: ToggleStyle.Configuration
    @Environment(\.bernSceneScaled) private var scaled
    @Environment(\.bernAccent) private var accent
    @Environment(\.labelsVisibility) private var labels
    @FocusState private var focused: Bool

    var body: some View {
        let k: CGFloat = scaled ? 1 : 1.5
        let on = configuration.isOn
        Button {
            configuration.isOn.toggle()
        } label: {
            HStack(spacing: 12 * k) {
                if labels != .hidden {
                    configuration.label.foregroundStyle(.primary)
                    Spacer(minLength: 12 * k)
                }
                Capsule()
                    .fill(on ? (accent ?? Color.accentColor) : Color(white: 0.47, opacity: 0.32))
                    .overlay(alignment: on ? .trailing : .leading) {
                        Circle().fill(Color.white)
                            .padding(2 * k)
                            .shadow(color: .black.opacity(0.25), radius: 2 * k, y: 1 * k)
                    }
                    .frame(width: 64 * k, height: 28 * k)
                    .overlay {
                        if focused {
                            Capsule().strokeBorder(Color.primary, lineWidth: 3 * k).padding(-8 * k)
                        }
                    }
                    .scaleEffect(focused ? 1.1 : 1)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(BernBareButtonStyle())
        .focused($focused)
        .animation(.spring(response: 0.3, dampingFraction: 0.7), value: on)
        .animation(.spring(response: 0.3, dampingFraction: 0.6), value: focused)
        .accessibilityValue(Text(verbatim: on ? "1" : "0"))
    }
}

/// A segmented picker in a scene laid out at the design's size: tvOS's own
/// takes a television's size, which the scene would enlarge again, so
/// there the segments are drawn at the design's (40 points, 15-point
/// words) on a capsule track; the chosen one stands raised, the focused
/// one turns white and grows by a tenth. Elsewhere tvOS's own.
struct BernSegments<Value: Hashable>: View {
    @Binding var selection: Value
    let options: [(String, Value)]
    @Environment(\.bernSceneScaled) private var scaled

    var body: some View {
        if scaled {
            HStack(spacing: 2) {
                ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                    BernSegment(label: option.0, selected: option.1 == selection) { selection = option.1 }
                }
            }
            .padding(2)
            .background(Capsule().fill(Color(white: 0.47, opacity: 0.24)))
        } else {
            Picker(selection: $selection) {
                ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                    Text(verbatim: option.0).tag(option.1)
                }
            } label: {
                EmptyView()
            }
            .pickerStyle(.segmented)
        }
    }
}

private struct BernSegment: View {
    let label: String
    let selected: Bool
    let action: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        Button(action: action) {
            Text(verbatim: label)
                .font(.system(size: 15, weight: .medium))
                .lineLimit(1)
                .padding(.horizontal, 18)
                .frame(height: 36)
                .foregroundStyle(focused ? Color.black : Color.primary)
                .background(Capsule().fill(focused ? Color.white : selected ? Color(white: 0.39) : Color.clear))
                .contentShape(Capsule())
        }
        .buttonStyle(BernBareButtonStyle())
        .focused($focused)
        .scaleEffect(focused ? 1.1 : 1)
        .shadow(color: .black.opacity(focused ? 0.45 : 0), radius: focused ? 14 : 0, y: focused ? 8 : 0)
        .zIndex(focused ? 1 : 0)
        .animation(.spring(response: 0.3, dampingFraction: 0.6), value: focused)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// A button that shows its label and nothing of the platform's.
struct BernBareButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.opacity(configuration.isPressed ? 0.8 : 1)
    }
}

/// tvOS has no slider control. The television's own form: a track that
/// takes the focus like any control, lifts under it as focused controls
/// do, and moves by one step for each press of the remote's left or right
/// (a twentieth of its range without a step). A burst of presses is one
/// edit: it ends half a second after the last.
struct BernFocusSlider: View {
    @Binding var value: Double
    let minimum: Double
    let maximum: Double
    let step: Double?
    let label: String
    var minimumIcon: String? = nil
    var maximumIcon: String? = nil
    var labelHidden: Bool = false
    var onEditingChanged: (Bool) -> Void = { _ in }
    @FocusState private var focused: Bool
    @State private var editing = false
    @State private var settle: Task<Void, Never>?
    @Environment(\.isEnabled) private var enabled
    @Environment(\.bernSceneScaled) private var scaled
    @Environment(\.bernAccent) private var accent

    private var fraction: Double {
        maximum > minimum ? min(1, max(0, (value - minimum) / (maximum - minimum))) : 0
    }
    private var increment: Double { step ?? (maximum - minimum) / 20 }

    var body: some View {
        // The slider of the system's other platforms: a thin track filled
        // with the accent up to a white thumb. A scene laid out at the
        // design's size takes it at that size; the screen's own layout at
        // a television's (half as large again). Under the focus a ring
        // stands around it; it keeps its size, as a long one would cover
        // what stands beside it.
        let k: CGFloat = scaled ? 1 : 1.5
        HStack(spacing: 12 * k) {
            if !labelHidden {
                Text(label)
            }
            if let minimumIcon {
                Image(systemName: minimumIcon).foregroundStyle(.secondary)
            }
            GeometryReader { geometry in
                let thumb = CGSize(width: 40 * k, height: 24 * k)
                let x = max(0, geometry.size.width - thumb.width) * fraction
                ZStack(alignment: .leading) {
                    Capsule().fill(Color(white: 0.47, opacity: 0.36)).frame(height: 7 * k)
                    Capsule().fill(accent ?? Color.accentColor).frame(width: x + thumb.width / 2, height: 7 * k)
                    Capsule().fill(Color.white)
                        .frame(width: thumb.width, height: thumb.height)
                        .shadow(color: .black.opacity(0.3), radius: 4 * k, y: 2 * k)
                        .offset(x: x)
                }
                .frame(maxHeight: .infinity)
            }
            .frame(minWidth: 60 * k, maxWidth: .infinity)
            .frame(height: 31 * k)
            .contentShape(Rectangle())
            if let maximumIcon {
                Image(systemName: maximumIcon).foregroundStyle(.secondary)
            }
        }
        .overlay {
            if focused {
                Capsule().strokeBorder(Color.primary, lineWidth: 3 * k).padding(-8 * k)
            }
        }
        .animation(.spring(response: 0.3, dampingFraction: 0.6), value: focused)
        .focusable(enabled)
        .focused($focused)
        .onMoveCommand { direction in
            switch direction {
            case .left: nudge(-1)
            case .right: nudge(1)
            default: break
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
        .accessibilityValue(Text(verbatim: BernText.text(value)))
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: nudge(1)
            case .decrement: nudge(-1)
            @unknown default: break
            }
        }
    }

    private func nudge(_ sign: Double) {
        let next = min(maximum, max(minimum, value + sign * increment))
        if !editing {
            editing = true
            onEditingChanged(true)
        }
        value = next
        settle?.cancel()
        settle = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            editing = false
            onEditingChanged(false)
        }
    }
}
#endif
