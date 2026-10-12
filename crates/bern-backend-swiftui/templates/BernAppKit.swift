// Bern SwiftUI runtime: the macOS part. Generated build input; do not edit.
//
// Every Bern window is a SwiftUI scene in its own NSWindow. What AppKit and
// the user do to a window is reported to the session runtime as a fact;
// window commands from external entries run AppKit's own actions and
// answer once AppKit is done.

import AppKit
import AVKit
import SwiftUI

// MARK: - Platform state

/// What a window instance keeps about its NSWindow.
@MainActor
final class BernWindowPlatform {
    weak var nsWindow: NSWindow?
    var closeGuard: BernCloseGuard?
    var observers: [NSObjectProtocol] = []
    var visibility: NSKeyValueObservation?
    /// The frame `window.maximize` left, for `window.restore`.
    var restoreFrame: NSRect?
}

/// SwiftUI's window actions, which the host opens windows with.
@MainActor
final class BernHostPlatform {
    var openWindow: OpenWindowAction?
    var openSettings: OpenSettingsAction?
}

extension BernWindowContext {
    var nsWindow: NSWindow? { platform.nsWindow }

    func found(_ window: NSWindow?) {
        guard let window, window !== platform.nsWindow else { return }
        platform.nsWindow = window
        closeCheckChanged()
        observe(window)
        reportFacts()
    }

    /// Whatever the user or the system does to the window is reported as a
    /// fact, never turned into a command. A live resize is reported once
    /// it ends.
    private func observe(_ window: NSWindow) {
        for observer in platform.observers { NotificationCenter.default.removeObserver(observer) }
        let names: [Notification.Name] = [
            NSWindow.didMiniaturizeNotification, NSWindow.didDeminiaturizeNotification,
            NSWindow.didEnterFullScreenNotification, NSWindow.didExitFullScreenNotification,
            NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification,
            NSWindow.didMoveNotification, NSWindow.didResizeNotification,
            NSWindow.didEndLiveResizeNotification, NSWindow.didChangeOcclusionStateNotification,
        ]
        platform.observers = names.map { name in
            NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) {
                [weak self] _ in
                MainActor.assumeIsolated { self?.reportFacts() }
            }
        }
        // Ordering a window on or off screen posts none of the above.
        platform.visibility = window.observe(\.isVisible) { [weak self] _, _ in
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.reportFacts() } }
        }
    }

    func reportFacts() {
        guard let window = platform.nsWindow, generation > 0, !window.inLiveResize else { return }
        BernHost.shared.reportFacts(self, window)
    }

    /// Windows with a close check route the system close button and
    /// File > Close through the close protocol before anything closes.
    func closeCheckChanged() {
        guard closeCheck != nil, let window = platform.nsWindow, platform.closeGuard?.window !== window else {
            return
        }
        let guardian = BernCloseGuard(window: window, context: self)
        platform.closeGuard = guardian
        window.delegate = guardian
    }
}

// MARK: - Windows

extension BernHost {
    func platformWillStart() {
        // `initial: open` windows open when the session starts. Restoring a
        // previous session's closed windows would contradict that, so window
        // open/closed state is not restored; frames are still remembered.
        UserDefaults.standard.register(defaults: [
            "ApplePersistenceIgnoreState": true,
            "NSQuitAlwaysKeepsWindows": false,
        ])
    }

    func platformDidStart() {
        // Hiding the app hides every window without minimizing it.
        for name in [NSApplication.didHideNotification, NSApplication.didUnhideNotification] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated {
                    for context in BernHost.shared.windows.values { context.reportFacts() }
                }
            }
        }
    }

    /// Bring a window forward; AppKit can always do that.
    func activate(_ context: BernWindowContext) throws {
        open(context.kindID)
    }

    /// Every window can be asked to close.
    func checkClosable(_ context: BernWindowContext) throws {}

    func registerActions(openWindow: OpenWindowAction, openSettings: OpenSettingsAction) {
        platform.openWindow = openWindow
        platform.openSettings = openSettings
    }

    /// Open a window kind, or bring its instance forward.
    func open(_ kindID: String) {
        guard let kind = kinds.first(where: { $0.id == kindID }) else { return }
        if kind.isSettings {
            platform.openSettings?()
        } else {
            platform.openWindow?(id: kindID)
        }
        NSApplication.shared.activate()
    }

    /// Reopen the primary window, as clicking the Dock icon does.
    func reopenPrimary() {
        guard !primary.isEmpty, windows[primary] == nil else { return }
        open(primary)
    }

    /// The system window showing a window kind, for development aids.
    func systemWindow(_ kindID: String) -> NSWindow? {
        windows[kindID]?.nsWindow
    }

    // MARK: Window presentation

    /// Report what AppKit shows for a window: visibility, size state, the
    /// frame with a top-left origin on the main display, focus, and the
    /// window actions available now, as bits in `WindowAction` order.
    func reportFacts(_ context: BernWindowContext, _ window: NSWindow) {
        guard let handle else { return }
        let state = BernWindowState(window)
        let frame = BernWindowState.topLeft(window.frame)
        _ = bern_window_facts(
            handle, context.kind, context.generation, context.commandInFlight ? 0 : 1,
            state.visibility, state.size, frame.minX, frame.minY, frame.width, frame.height,
            window.isKeyWindow ? 1 : 0, state.actions)
    }

    private func nsWindow(_ context: BernWindowContext) throws -> NSWindow {
        guard let window = context.nsWindow else {
            throw BernCommandFailure(code: "target_unavailable", message: "`\(context.id)` has no system window yet")
        }
        return window
    }

    /// Run a window action and answer once AppKit reports that it is done:
    /// at once for synchronous changes, or when `name` is posted for
    /// animated ones. The facts it causes are reported before the answer.
    private func perform(
        _ context: BernWindowContext, request: UInt64, command: UInt32,
        until name: Notification.Name? = nil, _ action: () -> Void
    ) {
        context.commandInFlight = true
        let finish: @MainActor (BernCommandFailure?) -> Void = { [weak self] failure in
            context.reportFacts()
            context.commandInFlight = false
            if let failure {
                self?.fail(request, failure)
            } else {
                self?.complete(request, command: command) { _ in }
            }
        }
        guard let name, let window = context.nsWindow else {
            action()
            finish(nil)
            return
        }
        let wait = BernWait()
        wait.observer = NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { _ in
            MainActor.assumeIsolated { if wait.finish() { finish(nil) } }
        }
        action()
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
            MainActor.assumeIsolated {
                if wait.finish() {
                    finish(BernCommandFailure(
                        code: "ui_timeout",
                        message: "AppKit did not finish the window change in time; the outcome is unknown"))
                }
            }
        }
    }

    func minimize(_ context: BernWindowContext, request: UInt64, command: UInt32) throws {
        let window = try nsWindow(context)
        guard BernWindowState(window).can(.minimize) else { throw unavailable(context, "minimize") }
        perform(context, request: request, command: command, until: NSWindow.didMiniaturizeNotification) {
            window.miniaturize(nil)
        }
    }

    /// Undo one step, the most recent first: minimized, full screen, zoomed.
    func restore(_ context: BernWindowContext, request: UInt64, command: UInt32) throws {
        let window = try nsWindow(context)
        if window.isMiniaturized {
            perform(context, request: request, command: command, until: NSWindow.didDeminiaturizeNotification) {
                window.deminiaturize(nil)
            }
        } else if window.styleMask.contains(.fullScreen) {
            perform(context, request: request, command: command, until: NSWindow.didExitFullScreenNotification) {
                window.toggleFullScreen(nil)
            }
        } else if BernWindowState(window).size == 1 {
            // An animated zoom returns once the animation is done. AppKit
            // has nothing to return to for a window that opened zoomed;
            // it then gets the frame maximize left or its declared size.
            let fallback = context.platform.restoreFrame ?? defaultFrame(context, window)
            context.platform.restoreFrame = nil
            perform(context, request: request, command: command) {
                window.zoom(nil)
                if window.isZoomed { window.setFrame(fallback, display: true, animate: true) }
            }
        } else {
            throw unavailable(context, "restore")
        }
    }

    func maximize(_ context: BernWindowContext, request: UInt64, command: UInt32) throws {
        let window = try nsWindow(context)
        guard BernWindowState(window).can(.maximize) else { throw unavailable(context, "maximize") }
        context.platform.restoreFrame = window.frame
        perform(context, request: request, command: command) { window.zoom(nil) }
    }

    /// The declared size centred on the window's display, within its limits;
    /// two thirds of the display without a declaration.
    private func defaultFrame(_ context: BernWindowContext, _ window: NSWindow) -> NSRect {
        let visible = (window.screen ?? NSScreen.main)?.visibleFrame ?? window.frame
        let declared = kinds.first { $0.id == context.kindID }?.defaultSize.map {
            window.frameRect(forContentRect: NSRect(origin: .zero, size: $0)).size
        }
        let (minimum, maximum) = BernWindowState.limits(window)
        let wanted = declared ?? NSSize(width: visible.width * 2 / 3, height: visible.height * 2 / 3)
        let size = NSSize(
            width: min(max(wanted.width, minimum.width), maximum.width, visible.width),
            height: min(max(wanted.height, minimum.height), maximum.height, visible.height))
        return NSRect(
            x: visible.midX - size.width / 2, y: visible.midY - size.height / 2, width: size.width,
            height: size.height)
    }

    /// Move the top-left corner to `x, y`, keeping the title bar on a display.
    func move(_ context: BernWindowContext, x: Double, y: Double, request: UInt64, command: UInt32) throws {
        let window = try nsWindow(context)
        guard BernWindowState(window).can(.move) else { throw unavailable(context, "move") }
        let frame = BernWindowState.fromTopLeft(x: x, y: y, size: window.frame.size)
        let titleBar = NSRect(x: frame.minX, y: frame.maxY - 28, width: frame.width, height: 28)
        guard NSScreen.screens.contains(where: {
            let visible = $0.visibleFrame.intersection(titleBar)
            return visible.width >= 40 && visible.height >= 1
        }) else {
            throw BernCommandFailure(
                code: "out_of_range", message: "at \(x), \(y) the title bar of `\(context.id)` would be off every display")
        }
        perform(context, request: request, command: command) { window.setFrame(frame, display: true) }
    }

    /// Resize keeping the top-left corner, within the window's own limits.
    func resize(
        _ context: BernWindowContext, width: Double, height: Double, request: UInt64, command: UInt32
    ) throws {
        let window = try nsWindow(context)
        guard BernWindowState(window).can(.resize) else { throw unavailable(context, "resize") }
        let (minimum, maximum) = BernWindowState.limits(window)
        guard width >= minimum.width, height >= minimum.height, width <= maximum.width, height <= maximum.height
        else {
            let limit = maximum.width < .greatestFiniteMagnitude / 2 ? " up to \(Int(maximum.width)) × \(Int(maximum.height))" : ""
            throw BernCommandFailure(
                code: "out_of_range",
                message: "`\(context.id)` needs at least \(Int(minimum.width.rounded(.up))) × \(Int(minimum.height.rounded(.up)))\(limit)")
        }
        var frame = window.frame
        frame.origin.y = frame.maxY - height
        frame.size = NSSize(width: width, height: height)
        perform(context, request: request, command: command) { window.setFrame(frame, display: true) }
    }

    func finishClose(_ context: BernWindowContext) {
        context.allowsClose = true
        if let window = context.nsWindow {
            window.close()
        } else {
            context.dismiss?()
        }
    }
}

// MARK: - Window presentation types

/// A window's AppKit state in the terms of the session catalog.
@MainActor
struct BernWindowState {
    /// `WindowAction` order of the session catalog.
    enum Action: UInt32 {
        case activate, close, minimize, restore, maximize, move, resize
    }

    let visibility: UInt8
    let size: UInt8
    let actions: UInt32

    init(_ window: NSWindow) {
        let mask = window.styleMask
        let minimized = window.isMiniaturized
        let fullScreen = mask.contains(.fullScreen)
        // A window that cannot be resized reports `isZoomed` when its fixed
        // frame equals its standard frame; only resizable windows zoom.
        let zoomed = !fullScreen && mask.contains(.resizable) && window.isZoomed
        visibility = minimized ? 1 : (window.isVisible && !NSApplication.shared.isHidden ? 0 : 2)
        size = fullScreen ? 2 : (zoomed ? 1 : 0)
        let normal = !minimized && !fullScreen
        let available: [(Action, Bool)] = [
            (.activate, true),
            (.close, mask.contains(.closable)),
            (.minimize, normal && mask.contains(.miniaturizable)),
            (.restore, minimized || fullScreen || zoomed),
            (.maximize, normal && !zoomed && mask.contains(.resizable)),
            (.move, normal),
            (.resize, normal && mask.contains(.resizable)),
        ]
        actions = available.reduce(0) { bits, entry in entry.1 ? bits | 1 << entry.0.rawValue : bits }
    }

    func can(_ action: Action) -> Bool { actions & 1 << action.rawValue != 0 }

    /// AppKit frames grow upwards from the main display's bottom-left
    /// corner; the catalog measures from its top-left corner, downwards.
    static func topLeft(_ frame: NSRect) -> NSRect {
        let top = NSScreen.screens.first?.frame.maxY ?? 0
        return NSRect(x: frame.minX, y: top - frame.maxY, width: frame.width, height: frame.height)
    }

    static func fromTopLeft(x: Double, y: Double, size: NSSize) -> NSRect {
        let top = NSScreen.screens.first?.frame.maxY ?? 0
        return NSRect(x: x, y: top - y - size.height, width: size.width, height: size.height)
    }

    /// Smallest and largest outer size: the window's frame limits combined
    /// with the content limits SwiftUI sets from the root view.
    static func limits(_ window: NSWindow) -> (NSSize, NSSize) {
        let contentMin = window.frameRect(forContentRect: NSRect(origin: .zero, size: window.contentMinSize)).size
        let contentMax = window.frameRect(forContentRect: NSRect(origin: .zero, size: window.contentMaxSize)).size
        let minimum = NSSize(
            width: max(window.minSize.width, contentMin.width, 1), height: max(window.minSize.height, contentMin.height, 1))
        let maximum = NSSize(
            width: min(window.maxSize.width, contentMax.width), height: min(window.maxSize.height, contentMax.height))
        return (minimum, maximum)
    }
}

/// Finishes a waiting window command once: by its notification or by the
/// timeout, whichever comes first.
@MainActor
final class BernWait {
    var observer: NSObjectProtocol?
    private var done = false

    func finish() -> Bool {
        guard !done else { return false }
        done = true
        if let observer { NotificationCenter.default.removeObserver(observer) }
        return true
    }
}

/// Stands in as the window's delegate so that a close the system asks for
/// first runs the close protocol. Every other delegate message goes to the
/// delegate SwiftUI installed.
@MainActor
final class BernCloseGuard: NSObject, NSWindowDelegate {
    // Read by the Objective-C runtime's forwarding, which is not isolated;
    // it is only set once, on the main thread.
    nonisolated(unsafe) private let original: (any NSWindowDelegate)?
    private(set) weak var window: NSWindow?
    private weak var context: BernWindowContext?

    init(window: NSWindow, context: BernWindowContext) {
        self.original = window.delegate
        self.window = window
        self.context = context
    }

    nonisolated override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    nonisolated override func forwardingTarget(for selector: Selector!) -> Any? {
        if original?.responds(to: selector) == true { return original }
        return super.forwardingTarget(for: selector)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        guard let context, !context.allowsClose else {
            return original?.windowShouldClose?(sender) ?? true
        }
        do {
            _ = try BernHost.shared.requestClose(context)
        } catch let problem as BernCommandFailure {
            BernHost.shared.failure = BernFailure(window: context.id, code: problem.code, message: problem.message)
        } catch {}
        return false
    }
}

/// Reports the NSWindow that hosts a window root.
struct BernWindowReader: NSViewRepresentable {
    let found: @MainActor (NSWindow?) -> Void

    final class ReaderView: NSView {
        var found: (@MainActor (NSWindow?) -> Void)?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            found?(window)
        }
    }

    func makeNSView(context: Context) -> ReaderView {
        let view = ReaderView()
        view.found = found
        return view
    }

    func updateNSView(_ view: ReaderView, context: Context) {
        view.found = found
        if let window = view.window { found(window) }
    }
}

// MARK: - Development snapshots

/// Development aid, off unless `BERN_SNAPSHOT_DIR` is set: renders the
/// app's own windows to PNG files after launch and after each session
/// command, so tools can review layouts without screen access. System
/// materials may render without their live blur in these images.
@MainActor
enum BernSnapshot {
    static let directory = ProcessInfo.processInfo.environment["BERN_SNAPSHOT_DIR"].map {
        URL(fileURLWithPath: $0, isDirectory: true)
    }
    private static var sequence = 0

    /// Development override of the appearance, `light` or `dark`.
    static func applyAppearance() {
        switch ProcessInfo.processInfo.environment["BERN_APPEARANCE"] {
        case "light": NSApplication.shared.appearance = NSAppearance(named: .aqua)
        case "dark": NSApplication.shared.appearance = NSAppearance(named: .darkAqua)
        default: break
        }
    }

    private static var windowSignals: [DispatchSourceSignal] = []

    /// Development aid, off unless `BERN_DEV_WINDOW_SIGNALS` names a window
    /// kind: SIGUSR1 then calls `performClose(_:)` and SIGUSR2
    /// `performMiniaturize(_:)` on that window, exactly what its title-bar
    /// buttons do, so tools without screen access can exercise the system
    /// close path and facts that come from the user.
    static func installWindowSignals() {
        guard windowSignals.isEmpty,
            let kind = ProcessInfo.processInfo.environment["BERN_DEV_WINDOW_SIGNALS"]
        else { return }
        let actions: [(Int32, @MainActor (NSWindow) -> Void)] = [
            (SIGUSR1, { $0.performClose(nil) }),
            (SIGUSR2, { $0.performMiniaturize(nil) }),
        ]
        windowSignals = actions.map { number, action in
            signal(number, SIG_IGN)
            let source = DispatchSource.makeSignalSource(signal: number, queue: .main)
            source.setEventHandler {
                MainActor.assumeIsolated {
                    if let window = BernHost.shared.systemWindow(kind) { action(window) }
                }
            }
            source.resume()
            return source
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
        for window in NSApplication.shared.windows where window.isVisible && window.frame.width > 120 {
            guard let view = window.contentView?.superview ?? window.contentView else { continue }
            guard let image = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { continue }
            view.cacheDisplay(in: view.bounds, to: image)
            BernMedia.drawFrames(in: view, onto: image)
            let name = (window.identifier?.rawValue ?? "window-\(window.windowNumber)")
                .replacingOccurrences(of: "/", with: "-")
            let file = directory.appendingPathComponent(String(format: "%03d-%@.png", sequence, name))
            try? image.representation(using: .png, properties: [:])?.write(to: file)
        }
    }
}

final class BernAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        if !hasVisibleWindows {
            MainActor.assumeIsolated { BernHost.shared.reopenPrimary() }
        }
        return true
    }

    func applicationWillTerminate(_ notification: Notification) {
        MainActor.assumeIsolated { BernHost.shared.stop() }
    }
}

/// The window root's AppKit part: finds the NSWindow and keeps SwiftUI's
/// window actions for the host.
struct BernWindowPlatformModifier: ViewModifier {
    let context: BernWindowContext
    @Environment(\.openWindow) private var openWindow
    @Environment(\.openSettings) private var openSettings

    func body(content: Content) -> some View {
        content
            .overlay(alignment: .topLeading) {
                BernWindowReader { [context] window in context.found(window) }
                    .frame(width: 0, height: 0)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
            .onAppear {
                BernSnapshot.applyAppearance()
                BernSnapshot.installWindowSignals()
                BernHost.shared.registerActions(openWindow: openWindow, openSettings: openSettings)
            }
    }
}

// MARK: - Video presenter

/// macOS has no audio session to set up.
@MainActor
func bernPrepareMediaSession() {}

/// An AVPlayerView that only presents: no controls, no keyboard or mouse
/// handling and no Now Playing integration, so it never changes playback
/// on its own.
final class BernPlayerView: AVPlayerView {
    override var acceptsFirstResponder: Bool { false }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// Attaches a player to a view and reports the attachment to the hub.
struct BernVideoSurface: NSViewRepresentable {
    let playerID: String
    let window: String
    let gravity: AVLayerVideoGravity

    final class Coordinator {
        var token: String?
        var playerID: String?
        var window: String?
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> BernPlayerView {
        let view = BernPlayerView()
        view.controlsStyle = .none
        view.updatesNowPlayingInfoCenter = false
        view.showsFullScreenToggleButton = false
        view.videoGravity = gravity
        attach(view, context.coordinator)
        return view
    }

    func updateNSView(_ view: BernPlayerView, context: Context) {
        view.videoGravity = gravity
        if context.coordinator.playerID != playerID || context.coordinator.window != window {
            Self.detach(context.coordinator)
            attach(view, context.coordinator)
        }
    }

    static func dismantleNSView(_ view: BernPlayerView, coordinator: Coordinator) {
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

extension BernMedia {
    /// Development snapshots: caching a view's display leaves out video
    /// layers, so draw the current frame of each local file into the image.
    static func drawFrames(in root: NSView, onto image: NSBitmapImageRep) {
        var views: [BernPlayerView] = []
        func collect(_ view: NSView) {
            if let player = view as? BernPlayerView { views.append(player) }
            view.subviews.forEach(collect)
        }
        collect(root)
        guard !views.isEmpty, let context = NSGraphicsContext(bitmapImageRep: image) else { return }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = context
        defer { NSGraphicsContext.restoreGraphicsState() }
        for view in views where view.window != nil && !view.isHiddenOrHasHiddenAncestor {
            guard let player = view.player, let item = player.currentItem,
                let asset = item.asset as? AVURLAsset, asset.url.isFileURL
            else { continue }
            let generator = AVAssetImageGenerator(asset: asset)
            generator.appliesPreferredTrackTransform = true
            generator.requestedTimeToleranceBefore = .zero
            generator.requestedTimeToleranceAfter = .zero
            guard let frame = try? generator.copyCGImage(at: player.currentTime(), actualTime: nil) else { continue }
            var rect = view.convert(view.bounds, to: root)
            if root.isFlipped {
                rect.origin.y = root.bounds.height - rect.maxY
            }
            let size = CGSize(width: frame.width, height: frame.height)
            let fit = min(rect.width / size.width, rect.height / size.height)
            let drawn = CGRect(
                x: rect.midX - size.width * fit / 2, y: rect.midY - size.height * fit / 2,
                width: size.width * fit, height: size.height * fit)
            // The image's context is already in points.
            context.cgContext.draw(frame, in: drawn)
        }
    }
}

/// The macOS 26 slider's shape, drawn for earlier systems: a 6 pt rounded
/// track and a 20 x 16 pt white knob with a soft shadow.
struct BernScrubberShape: View {
    @SwiftUI.Binding var value: Double
    let range: ClosedRange<Double>
    let step: Double
    let onEditingChanged: (Bool) -> Void
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.controlActiveState) private var activeState
    @State private var dragging = false

    private static let knob = CGSize(width: 20, height: 16)
    private static let track: CGFloat = 6

    private var span: Double { max(range.upperBound - range.lowerBound, .ulpOfOne) }

    private func set(_ next: Double) {
        value = min(max(next, range.lowerBound), range.upperBound)
    }

    private func nudge(_ steps: Double) -> KeyPress.Result {
        guard isEnabled else { return .ignored }
        onEditingChanged(true)
        set(value + steps * step)
        onEditingChanged(false)
        return .handled
    }

    var body: some View {
        GeometryReader { geometry in
            let travel = max(geometry.size.width - Self.knob.width, 0)
            let fraction = min(max((value - range.lowerBound) / span, 0), 1)
            let x = travel * fraction
            let split = x + Self.knob.width / 2
            ZStack(alignment: .leading) {
                // The two parts meet under the knob, so neither is drawn
                // over the other.
                Capsule()
                    .fill(Color(nsColor: .quaternaryLabelColor))
                    .frame(width: max(geometry.size.width - split, 0), height: Self.track)
                    .offset(x: split)
                // Like the system slider, the played part dims in an
                // inactive window.
                Capsule()
                    .fill(
                        isEnabled && activeState != .inactive
                            ? Color.primary : Color(nsColor: .tertiaryLabelColor))
                    .frame(width: split, height: Self.track)
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(Color.white)
                    .overlay(
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .strokeBorder(Color.black.opacity(0.08), lineWidth: 0.5)
                    )
                    .shadow(color: .black.opacity(isEnabled ? 0.2 : 0), radius: 1.5, y: 0.5)
                    .frame(width: Self.knob.width, height: Self.knob.height)
                    .scaleEffect(dragging ? 1.1 : 1)
                    .offset(x: x)
            }
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { drag in
                        if !dragging {
                            dragging = true
                            onEditingChanged(true)
                        }
                        let at = (drag.location.x - Self.knob.width / 2) / max(travel, 1)
                        set(range.lowerBound + min(max(at, 0), 1) * span)
                    }
                    .onEnded { _ in
                        dragging = false
                        onEditingChanged(false)
                    }
            )
            .allowsHitTesting(isEnabled)
            .animation(.easeOut(duration: 0.15), value: dragging)
        }
        .frame(height: 22)
        .focusable(isEnabled)
        .onKeyPress(.leftArrow) { nudge(-1) }
        .onKeyPress(.downArrow) { nudge(-1) }
        .onKeyPress(.rightArrow) { nudge(1) }
        .onKeyPress(.upArrow) { nudge(1) }
    }
}

// MARK: - Chrome, immersion and idleness

/// A window's chrome, immersion and idleness: `unified` lays the content
/// under a transparent title bar; `immersive` hides the title bar, its
/// controls and the toolbar, and the pointer once it rests; `idleAfter`
/// turns `idle` on after that many seconds without input in the window.
struct BernWindowBehavior: ViewModifier {
    let context: BernWindowContext
    let unified: Bool
    let idleAfter: Double?
    let immersive: Bool
    @State private var monitor = BernEventMonitor()

    func body(content: Content) -> some View {
        content
            .onAppear {
                apply(attempts: 20)
                if let idleAfter {
                    monitor.start(context: context, after: idleAfter)
                }
            }
            .onDisappear { monitor.stop() }
            .onChange(of: immersive) { apply(attempts: 20) }
            .onChange(of: context.idle) { _, idle in
                if idle && immersive {
                    NSCursor.setHiddenUntilMouseMoves(true)
                }
            }
    }

    /// Set the window up once SwiftUI has created it.
    private func apply(attempts: Int) {
        guard let window = context.platform.nsWindow else {
            if attempts > 0 {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { apply(attempts: attempts - 1) }
            }
            return
        }
        let underTitle = unified || immersive
        if underTitle {
            window.styleMask.insert(.fullSizeContentView)
        } else {
            window.styleMask.remove(.fullSizeContentView)
        }
        window.titlebarAppearsTransparent = underTitle
        window.titleVisibility = underTitle ? .hidden : .visible
        window.toolbar?.isVisible = !immersive
        for kind in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            window.standardWindowButton(kind)?.isHidden = immersive
        }
    }
}

/// Watches input in one window and turns its `idle` on after a quiet spell.
@MainActor
final class BernEventMonitor {
    private var monitor: Any?
    private var timer: Timer?

    func start(context: BernWindowContext, after seconds: Double) {
        stop()
        context.platform.nsWindow?.acceptsMouseMovedEvents = true
        let events: NSEvent.EventTypeMask = [
            .mouseMoved, .leftMouseDown, .rightMouseDown, .otherMouseDown,
            .leftMouseDragged, .scrollWheel, .keyDown,
        ]
        monitor = NSEvent.addLocalMonitorForEvents(matching: events) { [weak self, weak context] event in
            if let context, event.window === context.platform.nsWindow {
                MainActor.assumeIsolated { self?.reset(context, after: seconds) }
            }
            return event
        }
        reset(context, after: seconds)
    }

    func stop() {
        if let monitor {
            NSEvent.removeMonitor(monitor)
        }
        monitor = nil
        timer?.invalidate()
        timer = nil
    }

    private func reset(_ context: BernWindowContext, after seconds: Double) {
        context.platform.nsWindow?.acceptsMouseMovedEvents = true
        if context.idle {
            context.idle = false
        }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: seconds, repeats: false) { [weak context] _ in
            MainActor.assumeIsolated { context?.idle = true }
        }
    }
}
