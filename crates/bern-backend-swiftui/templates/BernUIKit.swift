// Bern SwiftUI runtime: the iOS part. Generated build input; do not edit.
//
// An iOS app has one scene, so Bern windows are virtual windows inside it,
// presented as iOS presents them: the primary window fills the scene and
// the other windows are sheets over it, the latest on top. Covered windows
// keep their drafts but cannot come forward until the windows over them
// close, so they report neither `activate` nor `close` as available. What
// the system does — a sheet swiped away, the app going to the background —
// is reported to the session runtime as a fact.

import AVKit
import SwiftUI
import UIKit

// MARK: - Platform state

/// Where a window instance shows, and whether its root declared a close
/// check, which decides whether its sheet may be swiped away.
@MainActor
@Observable
final class BernWindowPlatform {
    var hasCloseCheck = false
    /// The root's frame in the scene, in points.
    @ObservationIgnored var frame: CGRect = .zero
}

/// The virtual windows of the scene.
@MainActor
@Observable
final class BernHostPlatform {
    /// Whether the primary window fills the scene; closed, the scene offers
    /// to open it again.
    var primaryOpen = true
    /// Window kinds shown as sheets, bottom first.
    var sheets: [String] = []
    /// The scene is in the foreground and receives input.
    var active = true
    /// The scene is on screen, also while inactive.
    var visible = true
}

extension BernWindowContext {
    func reportFacts() {
        BernHost.shared.reportFacts(self)
    }

    func closeCheckChanged() {
        platform.hasCloseCheck = closeCheck != nil
    }
}

// MARK: - Windows

extension BernHost {
    func platformWillStart() {}

    func platformDidStart() {}

    /// Open a window kind: the primary fills the scene again, any other
    /// becomes the top sheet. An open window stays where it is.
    func open(_ kindID: String) {
        guard kinds.contains(where: { $0.id == kindID }) else { return }
        if kindID == primary {
            platform.primaryOpen = true
        } else if !platform.sheets.contains(kindID) {
            platform.sheets.append(kindID)
        }
    }

    /// The window that receives input: the top sheet, or the primary.
    private var top: String? {
        platform.sheets.last ?? (platform.primaryOpen ? primary : nil)
    }

    func isTop(_ context: BernWindowContext) -> Bool { top == context.kindID }

    /// Only the top window is in front; iOS cannot bring a covered one
    /// over the sheets above it.
    func activate(_ context: BernWindowContext) throws {
        guard isTop(context) else {
            throw BernCommandFailure(
                code: "target_unavailable",
                message: "`\(context.id)` is covered by another window; close the windows over it first")
        }
    }

    /// The top window and the primary can close; a covered sheet holds the
    /// sheets over it.
    func checkClosable(_ context: BernWindowContext) throws {
        guard isTop(context) || context.kindID == primary else {
            throw BernCommandFailure(
                code: "target_unavailable",
                message: "`\(context.id)` is covered by another window; close the windows over it first")
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

    /// Report a virtual window: visible while the scene is on screen, its
    /// frame in the scene, focused when it is the top window of an active
    /// scene. Only the top window activates; it and the primary close.
    func reportFacts(_ context: BernWindowContext) {
        guard let handle, context.generation > 0 else { return }
        let top = isTop(context)
        let closable = top || context.kindID == primary
        let actions: UInt32 = (top ? 1 << 0 : 0) | (closable ? 1 << 1 : 0)
        let frame = context.platform.frame
        _ = bern_window_facts(
            handle, context.kind, context.generation, context.commandInFlight ? 0 : 1,
            platform.visible ? 0 : 2, 0, frame.minX, frame.minY, frame.width, frame.height,
            platform.active && top ? 1 : 0, actions)
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

/// The window root's iOS part: reports where the window shows, and keeps a
/// sheet whose closing needs a decision from being swiped away; its close
/// button asks instead.
struct BernWindowPlatformModifier: ViewModifier {
    let context: BernWindowContext

    func body(content: Content) -> some View {
        let confirms = context.platform.hasCloseCheck && (context.closeCheck?.needsConfirmation() ?? false)
        content
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { frame in
                context.platform.frame = frame
                context.reportFacts()
            }
            .interactiveDismissDisabled(confirms)
    }
}

// MARK: - Scene

/// The app's scene: the primary window fills it and the other windows are
/// sheets over it. `window` makes the root of a window kind.
struct BernVirtualHost<Window: View>: View {
    let initial: [String]
    let window: (String) -> Window
    @Environment(\.scenePhase) private var scenePhase

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
        .modifier(BernSheets(depth: 0, window: window))
        .preferredColorScheme(BernSnapshot.colorScheme)
        .task { host.openInitial(initial) }
        .onChange(of: scenePhase, initial: true) { _, phase in
            platform.active = phase == .active
            platform.visible = phase != .background
            host.refreshFacts()
        }
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
            window(item.id).modifier(BernSheets(depth: depth + 1, window: window))
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

    final class Coordinator {
        var token: String?
        var playerID: String?
        var window: String?
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> BernPlayerView {
        let view = BernPlayerView()
        view.videoGravity = gravity
        attach(view, context.coordinator)
        return view
    }

    func updateUIView(_ view: BernPlayerView, context: Context) {
        view.videoGravity = gravity
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
