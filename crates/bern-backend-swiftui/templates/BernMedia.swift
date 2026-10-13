// Bern media runtime for SwiftUI. Generated build input; do not edit.
//
// AVFoundation is the media executor: it performs the requests the Rust
// media hub routes to it and reports what it observes, tagged with the
// source version it played. Reports are facts; nothing here turns them into
// commands. Video views attach a player view without its own controls
// (AVPlayerView on macOS, an AVPlayerLayer on iOS; see the platform files),
// so every playback change starts as a Bern media command. The standard
// control bar uses system controls and system colors; video pixels keep
// their original colors.

import AVFoundation
import AVKit
import SwiftUI

// MARK: - Merged player state

/// Playback phase, in the order of the media contract's `MediaPhase`.
enum BernMediaPhase: UInt32 {
    case idle, loading, ready, playing, paused, buffering, ended, failed, unavailable

    var name: String {
        switch self {
        case .idle: "Idle"
        case .loading: "Loading"
        case .ready: "Ready"
        case .playing: "Playing"
        case .paused: "Paused"
        case .buffering: "Buffering"
        case .ended: "Ended"
        case .failed: "Failed"
        case .unavailable: "Unavailable"
        }
    }
}

/// One `MediaPlayer` record of the media contract, decoded field by field
/// in the contract's order.
struct BernPlayerState: Equatable {
    var playerID: String
    var owner: String
    var sourceBound: Bool
    var sourceURL: String?
    var sourceVersion: Int64
    var phase: BernMediaPhase
    var positionMs: Int64
    var durationMs: Int64?
    var seekable: Bool
    var volume: Double
    var muted: Bool
    var error: String?
    var views: Int64

    init(bern reader: inout BernReader) throws {
        playerID = try reader.text()
        owner = try reader.text()
        sourceBound = try Bool(bern: &reader)
        sourceURL = try reader.u8() == 0 ? nil : try reader.text()
        sourceVersion = try reader.i64()
        guard let phase = BernMediaPhase(rawValue: try reader.u32()) else {
            throw BernDecodeError(message: "unknown media phase")
        }
        self.phase = phase
        positionMs = try reader.i64()
        durationMs = try Optional<Int64>(bern: &reader)
        seekable = try Bool(bern: &reader)
        volume = try reader.f64()
        muted = try Bool(bern: &reader)
        error = try Optional<String>(bern: &reader)
        views = try reader.i64()
        // The latest request (command, status, version, error) is only
        // shown to external entries.
        if try reader.u8() == 1 {
            _ = try reader.text()
            _ = try reader.u32()
            _ = try reader.i64()
            _ = try Optional<String>(bern: &reader)
        }
    }
}

/// Media commands, in the order of the media catalog.
enum BernMediaCommand: UInt32 {
    case players, state, open, play, pause, seek, setVolume, setMuted
}

/// Player identities, merged states and the commands of the media hub.
@MainActor
@Observable
final class BernMedia {
    static let shared = BernMedia()

    private(set) var players: [String: BernPlayerState] = [:]
    @ObservationIgnored private var revision: UInt64 = 0
    @ObservationIgnored let backend = BernMediaBackend()

    func receive(_ bytes: [UInt8]) {
        var reader = BernReader(bytes)
        do {
            let next = try reader.u64()
            // Updates carry every player; only the newest revision counts.
            guard next >= revision else { return }
            var players: [String: BernPlayerState] = [:]
            for _ in 0..<(try reader.u32()) {
                let player = try BernPlayerState(bern: &reader)
                players[player.playerID] = player
            }
            revision = next
            if players != self.players { self.players = players }
        } catch {
            BernHost.shared.failure = BernFailure(window: nil, code: "invalid_media_state", message: "\(error)")
        }
    }

    /// Attach AVFoundation as the executor of every player.
    func attachExecutor() {
        var writer = BernWriter()
        writer.text("avfoundation")
        writer.text("1")
        writer.u8(1)  // keeps playing without a view
        writer.u8(1)  // several views may show one player
        _ = BernHost.shared.mediaCall(.attachExecutor, writer.bytes)
    }

    func command(
        _ command: BernMediaCommand, player: String, window: String?,
        _ encode: (inout BernWriter) -> Void = { _ in }
    ) {
        BernHost.shared.dispatchMedia(command.rawValue, window: window) { writer in
            writer.text(player)
            encode(&writer)
        }
    }

    func open(_ player: String, url: String, window: String?) {
        command(.open, player: player, window: window) { $0.text(url) }
    }

    func seek(_ player: String, positionMs: Int64, window: String?) {
        command(.seek, player: player, window: window) { $0.i64(positionMs) }
    }

    func setVolume(_ player: String, volume: Double, window: String?) {
        command(.setVolume, player: player, window: window) { $0.f64(volume) }
    }

    func setMuted(_ player: String, muted: Bool, window: String?) {
        command(.setMuted, player: player, window: window) { $0.u8(muted ? 1 : 0) }
    }

    /// Create the player of a `Video(source: …)` declaration.
    func create(_ player: String, owner: String) -> BernCommandFailure? {
        var writer = BernWriter()
        writer.text(player)
        writer.text(owner)
        return BernHost.shared.status(BernHost.shared.mediaCall(.create, writer.bytes))
    }

    /// Report the declaration's current source; the hub loads only changes.
    func bind(_ player: String, url: String?) -> BernCommandFailure? {
        var writer = BernWriter()
        writer.text(player)
        if let url {
            writer.u8(1)
            writer.text(url)
        } else {
            writer.u8(0)
        }
        return BernHost.shared.status(BernHost.shared.mediaCall(.bind, writer.bytes))
    }

    func release(_ player: String) {
        var writer = BernWriter()
        writer.text(player)
        BernHost.shared.mediaPost(.release, writer.bytes)
    }

    /// Views presenting a player, by attachment token.
    @ObservationIgnored private var surfaces: [String: (view: BernPlayerView, window: String)] = [:]

    func attach(_ view: BernPlayerView, token: String, player: String, window: String) {
        view.player = backend.player(player).player
        surfaces[token] = (view, window)
        var writer = BernWriter()
        writer.text(token)
        writer.u8(1)
        writer.text(player)
        writer.text(window)
        BernHost.shared.mediaPost(.view, writer.bytes)
    }

    func detach(token: String) {
        guard let surface = surfaces.removeValue(forKey: token) else { return }
        surface.view.player = nil
        var writer = BernWriter()
        writer.text(token)
        writer.u8(0)
        BernHost.shared.mediaPost(.view, writer.bytes)
    }

    /// A window instance closed. Its views stop presenting at once, even
    /// while the toolkit keeps the closed window's views for later.
    func windowClosed(_ window: String) {
        for (token, surface) in surfaces where surface.window == window {
            surface.view.player = nil
            surfaces[token] = nil
        }
    }
}

// MARK: - AVFoundation executor

/// Runs the requests of the media hub with one AVPlayer per player id.
@MainActor
final class BernMediaBackend {
    private var players: [String: BernAVPlayer] = [:]

    func player(_ id: String) -> BernAVPlayer {
        if let player = players[id] { return player }
        if players.isEmpty { bernPrepareMediaSession() }
        let player = BernAVPlayer(id: id)
        players[id] = player
        return player
    }

    /// Perform one request: request u64, player text, source version u64,
    /// operation u32 and its payload, as encoded by the media contract.
    func perform(_ bytes: [UInt8]) {
        var reader = BernReader(bytes)
        guard let request = try? reader.u64(), let id = try? reader.text(),
            let version = try? reader.u64(), let operation = try? reader.u32()
        else { return }
        do {
            switch operation {
            case 0:
                let url = try reader.text()
                guard let location = Self.url(url) else {
                    throw BernCommandFailure(code: "invalid_source", message: "`\(url)` is not a media URL")
                }
                player(id).load(location, version: version)
            case 1:
                player(id).unload(version: version)
            case 7:
                players.removeValue(forKey: id)?.release()
            default:
                let target = player(id)
                guard target.version == version else {
                    throw BernCommandFailure(code: "stale_source", message: "a newer source is loaded")
                }
                switch operation {
                case 2: target.play()
                case 3: target.pause()
                case 4:
                    let position = try reader.i64()
                    target.seek(to: position) { [weak self] in self?.complete(request, nil) }
                    return
                case 5: target.player.volume = Float(try reader.f64())
                case 6: target.player.isMuted = try reader.u8() != 0
                default:
                    throw BernCommandFailure(code: "unknown_command", message: "unknown media operation \(operation)")
                }
            }
            complete(request, nil)
        } catch let failure as BernCommandFailure {
            complete(request, failure)
        } catch {
            complete(request, BernCommandFailure(code: "invalid_request", message: "\(error)"))
        }
    }

    private func complete(_ request: UInt64, _ failure: BernCommandFailure?) {
        var writer = BernWriter()
        writer.u64(request)
        if let failure {
            writer.u8(1)
            writer.text(failure.code)
            writer.text(failure.message)
        } else {
            writer.u8(0)
        }
        BernHost.shared.mediaPost(.complete, writer.bytes)
    }

    private static func url(_ text: String) -> URL? {
        if text.hasPrefix("/") { return URL(fileURLWithPath: text) }
        guard let url = URL(string: text), url.scheme != nil else { return nil }
        return url
    }
}

/// One AVPlayer and the facts it reports for its current source version.
@MainActor
final class BernAVPlayer {
    let id: String
    let player = AVPlayer()
    private(set) var version: UInt64 = 0
    private var itemObservations: [NSKeyValueObservation] = []
    private var playerObservations: [NSKeyValueObservation] = []
    private var timeObserver: Any?
    private var endObserver: NSObjectProtocol?
    private var jumpObserver: NSObjectProtocol?
    private var failureMessage: String?
    private var ended = false
    private var started = false

    init(id: String) {
        self.id = id
        player.actionAtItemEnd = .pause
        // System media keys and Now Playing would change playback without
        // a Bern command; they are not connected.
        player.preventsDisplaySleepDuringVideoPlayback = true
        playerObservations = [
            player.observe(\.timeControlStatus) { [weak self] _, _ in
                Task { @MainActor in self?.report() }
            },
            player.observe(\.volume) { [weak self] _, _ in
                Task { @MainActor in self?.report() }
            },
            player.observe(\.isMuted) { [weak self] _, _ in
                Task { @MainActor in self?.report() }
            },
        ]
        timeObserver = player.addPeriodicTimeObserver(
            forInterval: CMTime(value: 1, timescale: 2), queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.report() }
        }
    }

    func load(_ url: URL, version: UInt64) {
        self.version = version
        reset()
        // AVPlayer keeps its rate across items; a new source waits for a
        // play command instead of starting on its own.
        player.pause()
        let item = AVPlayerItem(url: url)
        itemObservations = [
            item.observe(\.status) { [weak self] _, _ in
                Task { @MainActor in self?.report(version) }
            },
            item.observe(\.duration) { [weak self] _, _ in
                Task { @MainActor in self?.report(version) }
            },
        ]
        let center = NotificationCenter.default
        endObserver = center.addObserver(
            forName: AVPlayerItem.didPlayToEndTimeNotification, object: item, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.version == version else { return }
                self.ended = true
                self.report()
            }
        }
        // A player that played to its end and then moves without a seek is
        // no longer at the end; iOS moves it to the start when the app goes
        // to the background.
        jumpObserver = center.addObserver(
            forName: AVPlayerItem.timeJumpedNotification, object: item, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.version == version, self.ended else { return }
                let duration = item.duration
                let current = self.player.currentTime()
                if duration.isNumeric, current.isNumeric, current.seconds < duration.seconds - 0.25 {
                    self.ended = false
                    self.report()
                }
            }
        }
        player.replaceCurrentItem(with: item)
        report()
    }

    func unload(version: UInt64) {
        self.version = version
        reset()
        player.pause()
        player.replaceCurrentItem(with: nil)
        report()
    }

    func play() {
        if ended {
            ended = false
            player.seek(to: .zero)
        }
        started = true
        player.play()
    }

    func pause() {
        player.pause()
    }

    func seek(to positionMs: Int64, completion: @escaping @MainActor () -> Void) {
        ended = false
        let time = CMTime(value: positionMs, timescale: 1000)
        player.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
            Task { @MainActor in
                self?.report()
                completion()
            }
        }
    }

    func release() {
        reset()
        player.pause()
        player.replaceCurrentItem(with: nil)
        if let timeObserver { player.removeTimeObserver(timeObserver) }
        timeObserver = nil
        playerObservations = []
    }

    private func reset() {
        itemObservations = []
        if let endObserver { NotificationCenter.default.removeObserver(endObserver) }
        endObserver = nil
        if let jumpObserver { NotificationCenter.default.removeObserver(jumpObserver) }
        jumpObserver = nil
        failureMessage = nil
        ended = false
        started = false
    }

    /// Report the current facts, unless they describe an older source.
    private func report(_ source: UInt64? = nil) {
        if let source, source != version { return }
        var phase: BernMediaPhase
        var error: String?
        var durationMs: Int64?
        if let item = player.currentItem {
            let duration = item.duration
            if duration.isNumeric, !duration.isIndefinite {
                durationMs = Int64((duration.seconds * 1000).rounded())
            }
            switch item.status {
            case .failed:
                phase = .failed
                error = item.error?.localizedDescription ?? failureMessage
            case .unknown:
                phase = .loading
            default:
                if ended {
                    phase = .ended
                } else {
                    switch player.timeControlStatus {
                    case .playing:
                        started = true
                        phase = .playing
                    case .waitingToPlayAtSpecifiedRate: phase = .buffering
                    default: phase = started ? .paused : .ready
                    }
                }
            }
        } else {
            phase = .idle
        }
        let seconds = player.currentTime().seconds
        let position = seconds.isFinite ? Int64((seconds * 1000).rounded()) : 0
        let seekable = durationMs != nil && !(player.currentItem?.seekableTimeRanges.isEmpty ?? true)
        var writer = BernWriter()
        writer.text(id)
        writer.u64(version)
        writer.u32(phase.rawValue)
        writer.i64(max(position, 0))
        durationMs.encode(bern: &writer)
        writer.u8(seekable ? 1 : 0)
        writer.f64((Double(player.volume) * 1000).rounded() / 1000)
        writer.u8(player.isMuted ? 1 : 0)
        error.encode(bern: &writer)
        BernHost.shared.mediaPost(.event, writer.bytes)
    }
}

// MARK: - Video declaration

/// How video fills its frame: `contain` letterboxes, `cover` crops and
/// `fill` stretches.
enum BernVideoFit {
    case contain, cover, fill

    var gravity: AVLayerVideoGravity {
        switch self {
        case .contain: .resizeAspect
        case .cover: .resizeAspectFill
        case .fill: .resize
        }
    }
}

/// How a `Video` gets its player.
enum BernVideoBinding {
    /// A player created for this declaration, following its source binding.
    case source(String?)
    /// An application player from the `#[bern::players]` catalog.
    case player(String)
}

/// Owns the player of a `Video(source: …)` for as long as the view's
/// identity lives — also while a navigation stack covers it — and releases
/// it when that identity ends, such as when its window closes.
final class BernPlayerOwner: @unchecked Sendable {
    var player: String?

    deinit {
        guard let player else { return }
        DispatchQueue.main.async {
            MainActor.assumeIsolated { BernMedia.shared.release(player) }
        }
    }
}

/// Round a presenter's layer: the picture is a layer of its own, which a
/// SwiftUI clip does not reach. A change of radius moves with `motion`.
@MainActor
func bernRoundCorners(_ layer: CALayer?, _ radius: Double, motion: BernMotion?) {
    guard let layer else { return }
    layer.cornerCurve = .continuous
    layer.masksToBounds = true
    let from = Double(layer.presentation()?.cornerRadius ?? layer.cornerRadius)
    guard abs(from - radius) > 0.01 else { return }
    if let motion {
        let spring = CASpringAnimation(keyPath: "cornerRadius")
        spring.mass = 1
        spring.stiffness = pow(2 * Double.pi / motion.period, 2)
        spring.damping = 4 * Double.pi * motion.damping / motion.period
        spring.fromValue = from
        spring.toValue = radius
        spring.duration = spring.settlingDuration
        layer.add(spring, forKey: "bernCorner")
    }
    layer.cornerRadius = radius
}

struct BernVideo: View {
    let target: String
    let label: String?
    let binding: BernVideoBinding
    let controls: Bool
    let fit: BernVideoFit
    /// Rounds the picture; a change moves with `cornerMotion`.
    var corner: Double = 0
    var cornerMotion: BernMotion? = nil
    @Environment(BernWindowContext.self) private var window
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var owner = BernPlayerOwner()

    private var playerID: String {
        switch binding {
        case .player(let id): id
        case .source: "\(window.id)/\(target)"
        }
    }

    private var sourceKey: String {
        switch binding {
        case .player: ""
        case .source(let url): "\(window.id)|\(url ?? "")|\(url == nil)"
        }
    }

    var body: some View {
        let state = BernMedia.shared.players[playerID]
        VStack(spacing: 6) {
            ZStack {
                if state != nil {
                    BernVideoSurface(
                        playerID: playerID, window: window.id, gravity: fit.gravity, corner: corner,
                        cornerMotion: reduce || bernForcedReduceMotion ? nil : cornerMotion
                    )
                    .opacity(BernVideo.showsVideo(state) ? 1 : 0)
                }
                BernVideoStatus(state: state)
            }
            .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
            .aspectRatio(16.0 / 9.0, contentMode: .fit)
            .frame(maxWidth: .infinity)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(label ?? BernText.string("bern.media.video"))
            .accessibilityValue(state.map { BernVideo.describe($0) } ?? "")
            .bernControl(
                target, role: "video", label: label ?? BernText.string("bern.media.video"),
                actions: BernControlActions(value: { [playerID] in
                    BernMedia.shared.players[playerID].map { BernVideo.describe($0) }
                }))
            if controls {
                BernVideoControls(target: target, playerID: playerID, state: state)
            }
        }
        .task(id: sourceKey) { bindSource() }
    }

    /// Create the declaration's player once and report its source; the hub
    /// ignores an unchanged source, so redraws never reload media.
    private func bindSource() {
        guard case .source(let url) = binding, !window.id.isEmpty else { return }
        let media = BernMedia.shared
        let id = playerID
        if owner.player != id {
            if let previous = owner.player { media.release(previous) }
            owner.player = nil
            if let failure = media.create(id, owner: window.id) {
                BernHost.shared.failure = BernFailure(window: window.id, code: failure.code, message: failure.message)
                return
            }
            owner.player = id
        }
        if let failure = media.bind(id, url: url) {
            BernHost.shared.failure = BernFailure(window: window.id, code: failure.code, message: failure.message)
        }
    }

    static func showsVideo(_ state: BernPlayerState?) -> Bool {
        switch state?.phase {
        case .ready, .playing, .paused, .buffering, .ended: true
        default: false
        }
    }

    static func describe(_ state: BernPlayerState) -> String {
        let duration = state.durationMs.map { BernVideoControls.time($0) } ?? "--:--"
        return "\(state.phase.name) \(BernVideoControls.time(state.positionMs)) / \(duration)"
    }
}

/// What the video area shows while there is no picture.
struct BernVideoStatus: View {
    let state: BernPlayerState?

    var body: some View {
        switch state?.phase {
        case .loading:
            ProgressView()
        case .failed:
            ContentUnavailableView {
                Label(BernText.string("bern.media.failed"), systemImage: "exclamationmark.triangle")
            } description: {
                Text(verbatim: state?.error ?? "")
            }
        case .unavailable, nil:
            ContentUnavailableView(
                BernText.string("bern.media.unavailable"), systemImage: "play.slash")
        case .idle:
            ContentUnavailableView(BernText.string("bern.media.empty"), systemImage: "play.rectangle")
        default:
            EmptyView()
        }
    }
}

/// The standard control bar. It shows the merged state only and turns user
/// input into media commands: dragging previews locally and commits once,
/// and state updates never call back into a command. Actions read the
/// state when they run, so external entries act on the current state.
struct BernVideoControls: View {
    let target: String
    let playerID: String
    let state: BernPlayerState?
    @Environment(BernWindowContext.self) private var window
    @State private var scrubbing: Double?
    @State private var volumePreview: Double?

    private static func playable(_ state: BernPlayerState?) -> Bool {
        guard let state, state.sourceURL != nil else { return false }
        switch state.phase {
        case .idle, .failed, .unavailable: return false
        default: return true
        }
    }

    private static func playing(_ state: BernPlayerState?) -> Bool {
        state?.phase == .playing || state?.phase == .buffering
    }

    private static func seekable(_ state: BernPlayerState?) -> Bool {
        playable(state) && state?.seekable == true && state?.durationMs != nil
    }

    private static func controllable(_ state: BernPlayerState?) -> Bool {
        state != nil && state?.phase != .unavailable
    }

    static func time(_ milliseconds: Int64) -> String {
        let seconds = max(milliseconds, 0) / 1000
        let (hours, minutes, rest) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60)
        let pad = { (value: Int64) in value < 10 ? "0\(value)" : "\(value)" }
        return hours > 0 ? "\(hours):\(pad(minutes)):\(pad(rest))" : "\(minutes):\(pad(rest))"
    }

    private var current: BernPlayerState? { BernMedia.shared.players[playerID] }

    private func togglePlayback() {
        let command: BernMediaCommand = Self.playing(current) ? .pause : .play
        BernMedia.shared.command(command, player: playerID, window: window.id)
    }

    private func seek(_ milliseconds: Double) {
        BernMedia.shared.seek(playerID, positionMs: Int64(milliseconds.rounded()), window: window.id)
    }

    private func setMuted(_ muted: Bool) {
        BernMedia.shared.setMuted(playerID, muted: muted, window: window.id)
    }

    private func toggleMuted() {
        setMuted(!(current?.muted ?? false))
    }

    private func setVolume(_ volume: Double) {
        BernMedia.shared.setVolume(playerID, volume: min(max(volume, 0), 1), window: window.id)
    }

    var body: some View {
        let playing = Self.playing(state)
        let playable = Self.playable(state)
        let seekable = Self.seekable(state)
        let controllable = Self.controllable(state)
        let playLabel = BernText.string(playing ? "bern.media.pause" : "bern.media.play")
        let muted = state?.muted ?? false
        let muteLabel = BernText.string(muted ? "bern.media.unmute" : "bern.media.mute")
        let duration = Double(state?.durationMs ?? 0)
        let position = Double(state?.positionMs ?? 0)
        HStack(spacing: 10) {
            Button(action: togglePlayback) {
                Label(playLabel, systemImage: playing ? "pause.fill" : "play.fill")
                    .labelStyle(.iconOnly)
                    .frame(width: 16)
            }
            .buttonStyle(.borderless)
            .disabled(!playable)
            .help(playLabel)
            .bernControl(
                "\(target).play_pause", role: "button", label: playLabel, enabled: playable,
                actions: BernControlActions(activate: togglePlayback))
            Text(verbatim: Self.time(Int64(scrubbing ?? position)))
                .font(.caption)
                .monospacedDigit()
                .foregroundStyle(.secondary)
            BernScrubber(
                value: SwiftUI.Binding(get: { scrubbing ?? min(position, duration) }, set: { scrubbing = $0 }),
                in: 0...max(duration, 1),
                step: 5000,
                label: BernText.string("bern.media.position")
            ) { editing in
                if !editing, let value = scrubbing {
                    seek(value)
                    scrubbing = nil
                }
            }
            .disabled(!seekable)
            .bernControl(
                "\(target).position", role: "slider", label: BernText.string("bern.media.position"),
                enabled: seekable,
                actions: BernControlActions(
                    setNumber: { seek($0) },
                    value: { BernText.text(Double(current?.positionMs ?? 0)) }))
            Text(verbatim: state?.durationMs.map { Self.time($0) } ?? (playable ? BernText.string("bern.media.live") : "--:--"))
                .font(.caption)
                .monospacedDigit()
                .foregroundStyle(.secondary)
            Button(action: toggleMuted) {
                Label(muteLabel, systemImage: muted ? "speaker.slash.fill" : "speaker.wave.2.fill")
                    .labelStyle(.iconOnly)
                    .frame(width: 18)
            }
            .buttonStyle(.borderless)
            .disabled(!controllable)
            .help(muteLabel)
            .bernControl(
                "\(target).mute", role: "switch", label: BernText.string("bern.media.mute"),
                enabled: controllable,
                actions: BernControlActions(
                    activate: toggleMuted, setBool: { setMuted($0) },
                    value: { (current?.muted ?? false) ? "true" : "false" }))
            BernScrubber(
                value: SwiftUI.Binding(get: { volumePreview ?? state?.volume ?? 1 }, set: { volumePreview = $0 }),
                in: 0...1,
                step: 0.05,
                label: BernText.string("bern.media.volume")
            ) { editing in
                if !editing, let value = volumePreview {
                    setVolume(value)
                    volumePreview = nil
                }
            }
            .frame(width: 80)
            .disabled(!controllable)
            .bernControl(
                "\(target).volume", role: "slider", label: BernText.string("bern.media.volume"),
                enabled: controllable,
                actions: BernControlActions(
                    setNumber: { setVolume($0) }, value: { BernText.text(current?.volume ?? 1) }))
        }
    }
}

/// The media progress bar: the played part is neutral, never the accent,
/// so it does not compete with the picture. iOS and macOS 26 and later use
/// the system slider, whose knob is already a capsule; earlier macOS draws
/// the same shape. Either way editing begins, the value previews, and
/// editing ends once, as with `Slider`.
struct BernScrubber: View {
    @SwiftUI.Binding var value: Double
    let range: ClosedRange<Double>
    let step: Double
    let label: String
    let onEditingChanged: (Bool) -> Void

    init(
        value: SwiftUI.Binding<Double>, in range: ClosedRange<Double>, step: Double, label: String,
        onEditingChanged: @escaping (Bool) -> Void
    ) {
        _value = value
        self.range = range
        self.step = step
        self.label = label
        self.onEditingChanged = onEditingChanged
    }

    var body: some View {
        #if os(macOS)
        if #available(macOS 26, *) {
            slider
        } else {
            BernScrubberShape(value: $value, range: range, step: step, onEditingChanged: onEditingChanged)
                .accessibilityRepresentation {
                    Slider(value: $value, in: range) {
                        Text(verbatim: label)
                    } onEditingChanged: { editing in
                        onEditingChanged(editing)
                    }
                }
        }
        #else
        slider
        #endif
    }

    private var slider: some View {
        Slider(value: $value, in: range) {
            Text(verbatim: label)
        } onEditingChanged: { editing in
            onEditingChanged(editing)
        }
        .labelsHidden()
        .tint(.primary)
    }
}
