// Bern SwiftUI runtime. Generated build input; do not edit.
//
// This file connects generated views to the in-process Rust runtime. It
// decodes typed values, mirrors engine state into observable objects,
// mirrors window-scoped session drafts, and runs session commands from
// external entries through the same control bindings the user operates.
// It contains no business logic and no custom colors: every surface keeps
// the system appearance.

import AppKit
import SwiftUI

// MARK: - Binary values

struct BernDecodeError: Error {
    let message: String
}

struct BernWriter {
    var bytes: [UInt8] = []

    mutating func u8(_ value: UInt8) { bytes.append(value) }

    mutating func u32(_ value: UInt32) {
        withUnsafeBytes(of: value.littleEndian) { bytes.append(contentsOf: $0) }
    }

    mutating func u64(_ value: UInt64) {
        withUnsafeBytes(of: value.littleEndian) { bytes.append(contentsOf: $0) }
    }

    mutating func i64(_ value: Int64) { u64(UInt64(bitPattern: value)) }

    mutating func f64(_ value: Double) { u64(value.bitPattern) }

    mutating func text(_ value: String) {
        let utf8 = Array(value.utf8)
        u32(UInt32(utf8.count))
        bytes.append(contentsOf: utf8)
    }
}

struct BernReader {
    private let bytes: [UInt8]
    private var position = 0

    init(_ bytes: [UInt8]) { self.bytes = bytes }

    var isAtEnd: Bool { position == bytes.count }

    private mutating func take(_ count: Int) throws -> ArraySlice<UInt8> {
        guard count >= 0, position + count <= bytes.count else {
            throw BernDecodeError(message: "unexpected end of data")
        }
        defer { position += count }
        return bytes[position..<(position + count)]
    }

    mutating func u8() throws -> UInt8 { try take(1).first! }

    mutating func u32() throws -> UInt32 {
        var value: UInt32 = 0
        for (index, byte) in try take(4).enumerated() { value |= UInt32(byte) << (8 * UInt32(index)) }
        return value
    }

    mutating func u64() throws -> UInt64 {
        var value: UInt64 = 0
        for (index, byte) in try take(8).enumerated() { value |= UInt64(byte) << (8 * UInt64(index)) }
        return value
    }

    mutating func i64() throws -> Int64 { Int64(bitPattern: try u64()) }

    mutating func f64() throws -> Double { Double(bitPattern: try u64()) }

    mutating func text() throws -> String {
        let count = Int(try u32())
        guard let value = String(bytes: try take(count), encoding: .utf8) else {
            throw BernDecodeError(message: "text is not UTF-8")
        }
        return value
    }
}

protocol BernCodable {
    init(bern reader: inout BernReader) throws
    func encode(bern writer: inout BernWriter)
}

extension Bool: BernCodable {
    init(bern reader: inout BernReader) throws {
        switch try reader.u8() {
        case 0: self = false
        case 1: self = true
        default: throw BernDecodeError(message: "invalid bool")
        }
    }

    func encode(bern writer: inout BernWriter) { writer.u8(self ? 1 : 0) }
}

extension Int64: BernCodable {
    init(bern reader: inout BernReader) throws { self = try reader.i64() }
    func encode(bern writer: inout BernWriter) { writer.i64(self) }
}

extension Double: BernCodable {
    init(bern reader: inout BernReader) throws { self = try reader.f64() }
    func encode(bern writer: inout BernWriter) { writer.f64(self) }
}

extension String: BernCodable {
    init(bern reader: inout BernReader) throws { self = try reader.text() }
    func encode(bern writer: inout BernWriter) { writer.text(self) }
}

extension Optional: BernCodable where Wrapped: BernCodable {
    init(bern reader: inout BernReader) throws {
        switch try reader.u8() {
        case 0: self = .none
        case 1: self = .some(try Wrapped(bern: &reader))
        default: throw BernDecodeError(message: "invalid optional")
        }
    }

    func encode(bern writer: inout BernWriter) {
        switch self {
        case .none: writer.u8(0)
        case .some(let value):
            writer.u8(1)
            value.encode(bern: &writer)
        }
    }
}

extension Array: BernCodable where Element: BernCodable {
    init(bern reader: inout BernReader) throws {
        let count = Int(try reader.u32())
        var values: [Element] = []
        values.reserveCapacity(Swift.min(count, 4096))
        for _ in 0..<count { values.append(try Element(bern: &reader)) }
        self = values
    }

    func encode(bern writer: inout BernWriter) {
        writer.u32(UInt32(count))
        for value in self { value.encode(bern: &writer) }
    }
}

// MARK: - Text

/// Enums whose variants declare localized labels.
protocol BernLabeled {
    var bernLabel: String { get }
}

/// Localized resources and Bern's basic, locale-independent formatting.
enum BernText {
    static func string(_ key: String) -> String {
        Bundle.main.localizedString(forKey: key, value: nil, table: nil)
    }

    static func format(_ key: String, _ arguments: [String]) -> String {
        String(format: string(key), arguments: arguments.map { $0 as CVarArg })
    }

    static func text(_ value: String) -> String { value }

    static func text(_ value: Int64) -> String { String(value) }

    static func text(_ value: Double) -> String {
        if value.rounded() == value, abs(value) < 1e15 { return String(Int64(value)) }
        return String(value)
    }

    static func text(_ value: some BernLabeled) -> String { value.bernLabel }
}

// MARK: - Session state

/// Implemented by generated session state classes.
@MainActor
protocol BernSessionState: AnyObject {
    static var bernIndex: UInt32 { get }
    var window: BernWindowContext? { get set }
    func load(_ reader: inout BernReader) throws
}

/// One window instance: its kind, generation and session drafts.
@MainActor
@Observable
final class BernWindowContext {
    let kind: UInt32
    let kindID: String
    let isSettings: Bool
    @ObservationIgnored let sessions: [any BernSessionState]
    @ObservationIgnored var dismiss: DismissAction?
    /// The window's declared close check, set by the generated window root.
    @ObservationIgnored var closeCheck: BernCloseCheck? {
        didSet { installCloseGuard() }
    }
    /// The close confirmation shown as this window's dialog. While it is
    /// pending, the window's controls are blocked for every entry.
    var pendingClose: BernPendingClose?
    @ObservationIgnored private(set) weak var nsWindow: NSWindow?
    @ObservationIgnored private var closeGuard: BernCloseGuard?
    @ObservationIgnored var allowsClose = false
    private(set) var generation: UInt64 = 0
    private(set) var id: String = ""

    init(kind: UInt32, kindID: String, isSettings: Bool, sessions: [any BernSessionState]) {
        self.kind = kind
        self.kindID = kindID
        self.isSettings = isSettings
        self.sessions = sessions
        for session in sessions { session.window = self }
    }

    func session<S: BernSessionState>(_ type: S.Type) -> S {
        sessions.first { $0 is S } as! S
    }

    func attached(generation: UInt64, id: String) {
        // A new instance starts without a pending or finished close.
        if generation != self.generation {
            allowsClose = false
            pendingClose = nil
        }
        self.generation = generation
        self.id = id
    }

    func found(_ window: NSWindow?) {
        guard let window, window !== nsWindow else { return }
        nsWindow = window
        installCloseGuard()
    }

    /// Windows with a close check route the system close button and
    /// File > Close through the close protocol before anything closes.
    private func installCloseGuard() {
        guard closeCheck != nil, let window = nsWindow, closeGuard?.window !== window else { return }
        let guardian = BernCloseGuard(window: window, context: self)
        closeGuard = guardian
        window.delegate = guardian
    }

    /// Write one session field through Rust, which validates it.
    func set(session: UInt32, field: UInt32, _ encode: (inout BernWriter) -> Void) -> Bool {
        BernHost.shared.sessionSet(self, session: session, field: field, encode)
    }
}

// MARK: - Controls

/// What an external entry may do with a mounted control. The closures are
/// the same ones the control runs for user input.
struct BernControlActions {
    var activate: (@MainActor () -> Void)? = nil
    var edit: (@MainActor (String) -> Void)? = nil
    var submit: (@MainActor () -> Void)? = nil
    var setBool: (@MainActor (Bool) -> Void)? = nil
    var setNumber: (@MainActor (Double) -> Void)? = nil
    var select: (@MainActor (String) -> Bool)? = nil
    var back: (@MainActor () -> Bool)? = nil
    var value: (@MainActor () -> String?)? = nil
}

struct BernControlEntry {
    let target: String
    let window: String
    let role: String
    let label: String
    let enabled: Bool
    let actions: BernControlActions
}

struct BernCommandFailure: Error {
    let code: String
    let message: String
}

@MainActor
final class BernControls {
    private var entries: [UUID: BernControlEntry] = [:]

    func register(_ token: UUID, _ entry: BernControlEntry) { entries[token] = entry }

    func remove(_ token: UUID) { entries[token] = nil }

    var all: [BernControlEntry] {
        entries.values.sorted { ($0.window, $0.target) < ($1.window, $1.target) }
    }

    func find(_ target: String) throws -> BernControlEntry {
        let matches = entries.values.filter { $0.target == target }
        if matches.isEmpty {
            throw BernCommandFailure(code: "target_not_found", message: "no mounted control `\(target)`; see ui.inspect")
        }
        if matches.count > 1 {
            throw BernCommandFailure(code: "target_ambiguous", message: "`\(target)` is mounted in several places")
        }
        if BernHost.shared.isBlocked(matches[0].window) {
            throw BernCommandFailure(
                code: "target_blocked",
                message: "`\(target)` is blocked by the close confirmation of `\(matches[0].window)`; decide it with window.confirm_close")
        }
        guard matches[0].enabled else {
            throw BernCommandFailure(code: "target_unavailable", message: "`\(target)` is disabled")
        }
        return matches[0]
    }
}

private struct BernControl: ViewModifier {
    let target: String
    let role: String
    let label: String
    let enabled: Bool
    let actions: BernControlActions
    @Environment(BernWindowContext.self) private var window
    @State private var token = UUID()

    func body(content: Content) -> some View {
        content
            .accessibilityIdentifier(target)
            .task(id: "\(window.id)|\(target)|\(label)|\(enabled)") {
                BernHost.shared.controls.register(
                    token,
                    BernControlEntry(
                        target: target, window: window.id, role: role, label: label, enabled: enabled,
                        actions: actions))
            }
            .onDisappear { BernHost.shared.controls.remove(token) }
    }
}

extension View {
    func bernControl(
        _ target: String, role: String, label: String, enabled: Bool = true,
        actions: BernControlActions
    ) -> some View {
        modifier(BernControl(target: target, role: role, label: label, enabled: enabled, actions: actions))
    }
}

// MARK: - Host

/// What to do when the reply of a command arrives.
struct BernPendingReply {
    let window: String?
    var success: ((inout BernReader) throws -> Void)? = nil
    var completion: (@MainActor (BernCommandFailure?) -> Void)? = nil
}

struct BernFailure: Identifiable {
    let id = UUID()
    let window: String?
    let code: String
    let message: String
}

/// Window kinds of the application, generated from the window catalog.
struct BernWindowKind {
    let id: String
    let isSettings: Bool
}

private func bernCopy(_ bytes: UnsafePointer<UInt8>?, _ count: Int) -> [UInt8] {
    guard let bytes, count > 0 else { return [] }
    return Array(UnsafeBufferPointer(start: bytes, count: count))
}

private func bernOnState(_ context: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ count: Int) {
    let data = bernCopy(bytes, count)
    DispatchQueue.main.async { MainActor.assumeIsolated { BernHost.shared.receiveState(data) } }
}

private func bernOnReply(
    _ context: UnsafeMutableRawPointer?, _ token: UInt64, _ bytes: UnsafePointer<UInt8>?, _ count: Int
) {
    let data = bernCopy(bytes, count)
    DispatchQueue.main.async { MainActor.assumeIsolated { BernHost.shared.receiveReply(token, data) } }
}

private func bernOnSessionRequest(
    _ context: UnsafeMutableRawPointer?, _ request: UInt64, _ bytes: UnsafePointer<UInt8>?, _ count: Int
) {
    let data = bernCopy(bytes, count)
    DispatchQueue.main.async { MainActor.assumeIsolated { BernHost.shared.receiveSessionRequest(request, data) } }
}

private func bernOnMediaRequest(_ context: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ count: Int) {
    let data = bernCopy(bytes, count)
    DispatchQueue.main.async { MainActor.assumeIsolated { BernMedia.shared.backend.perform(data) } }
}

private func bernOnMediaState(_ context: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ count: Int) {
    let data = bernCopy(bytes, count)
    DispatchQueue.main.async { MainActor.assumeIsolated { BernMedia.shared.receive(data) } }
}

private func bernOnQuery(
    _ context: UnsafeMutableRawPointer?, _ subscription: UInt64, _ bytes: UnsafePointer<UInt8>?, _ count: Int
) {
    let data = bernCopy(bytes, count)
    DispatchQueue.main.async { MainActor.assumeIsolated { BernHost.shared.receiveQuery(subscription, data) } }
}

/// Media functions of the bridge that answer at once.
enum BernMediaCall {
    case attachExecutor, create, bind
}

/// Media functions of the bridge that answer later or not at all.
enum BernMediaPost {
    case release, view, complete, event
}

@MainActor
@Observable
final class BernHost {
    static let shared = BernHost()

    @ObservationIgnored private var handle: OpaquePointer?
    @ObservationIgnored private var replies: [UInt64: BernPendingReply] = [:]
    @ObservationIgnored private var nextToken: UInt64 = 1
    @ObservationIgnored private var revision: UInt64 = 0
    @ObservationIgnored private var apply: ((UInt32, inout BernReader) throws -> Void)?
    @ObservationIgnored private var kinds: [BernWindowKind] = []
    @ObservationIgnored private var windows: [String: BernWindowContext] = [:]
    @ObservationIgnored private var pendingOpens: [String: [UInt64]] = [:]
    @ObservationIgnored private var openWindow: OpenWindowAction?
    @ObservationIgnored private var openSettings: OpenSettingsAction?
    @ObservationIgnored private var openedInitial = false
    @ObservationIgnored private var queries: [UInt64: @MainActor ([UInt8]) -> Void] = [:]
    @ObservationIgnored let controls = BernControls()

    var failure: BernFailure?
    private(set) var startupError: String?
    private(set) var engineID = ""
    private(set) var channel = ""

    /// Start the engine in this process and load the first state snapshot.
    func start(
        fingerprint: UInt64, kinds: [BernWindowKind],
        apply: @escaping (UInt32, inout BernReader) throws -> Void
    ) {
        guard handle == nil else { return }
        self.kinds = kinds
        self.apply = apply
        var error = BernBuffer(ptr: nil, len: 0, capacity: 0)
        let callbacks = BernCallbacks(
            context: nil, on_state: bernOnState, on_reply: bernOnReply,
            on_session_request: bernOnSessionRequest, on_media_request: bernOnMediaRequest,
            on_media_state: bernOnMediaState, on_query: bernOnQuery)
        guard let app = bern_app_start(fingerprint, callbacks, &error) else {
            startupError = String(decoding: Self.take(error), as: UTF8.self)
            return
        }
        _ = Self.take(error)
        handle = app
        engineID = String(decoding: Self.take(bern_app_engine_id(app)), as: UTF8.self)
        channel = String(decoding: Self.take(bern_app_channel(app)), as: UTF8.self)
        receiveState(Self.take(bern_app_snapshot(app)))
        BernMedia.shared.attachExecutor()
        BernMedia.shared.receive(Self.take(bern_media_snapshot(app)))
    }

    func stop() {
        if let handle { bern_app_stop(handle) }
        handle = nil
    }

    private static func take(_ buffer: BernBuffer) -> [UInt8] {
        defer { bern_buffer_free(buffer) }
        guard let pointer = buffer.ptr, buffer.len > 0 else { return [] }
        return Array(UnsafeBufferPointer(start: pointer, count: Int(buffer.len)))
    }

    // MARK: Engine state and commands

    func receiveState(_ bytes: [UInt8]) {
        var reader = BernReader(bytes)
        do {
            let next = try reader.u64()
            guard next >= revision else { return }
            revision = next
            let count = try reader.u32()
            for _ in 0..<count {
                let field = try reader.u32()
                try apply?(field, &reader)
            }
        } catch {
            failure = BernFailure(window: nil, code: "invalid_state", message: "\(error)")
        }
    }

    /// Dispatch an engine command; `window` receives a failure alert.
    func dispatch(
        _ command: UInt32, window: String?, _ encode: (inout BernWriter) -> Void,
        success: ((inout BernReader) throws -> Void)? = nil
    ) {
        send(command, window: window, encode: encode, reply: BernPendingReply(window: window, success: success))
    }

    /// Dispatch an engine command and learn whether it succeeded.
    func dispatch(
        _ command: UInt32, window: String?, encode: (inout BernWriter) -> Void,
        completion: (@MainActor (BernCommandFailure?) -> Void)?
    ) {
        send(command, window: window, encode: encode, reply: BernPendingReply(window: window, completion: completion))
    }

    private func send(
        _ command: UInt32, window: String?, encode: (inout BernWriter) -> Void, reply: BernPendingReply
    ) {
        guard let handle else {
            reply.completion?(BernCommandFailure(code: "engine_stopped", message: "the engine is not running"))
            return
        }
        var writer = BernWriter()
        encode(&writer)
        let token = nextToken
        nextToken += 1
        replies[token] = reply
        writer.bytes.withUnsafeBufferPointer { buffer in
            bern_app_dispatch(handle, command, buffer.baseAddress, buffer.count, token)
        }
    }

    /// Dispatch a media command; `window` receives a failure alert.
    func dispatchMedia(_ command: UInt32, window: String?, _ encode: (inout BernWriter) -> Void) {
        guard let handle else { return }
        var writer = BernWriter()
        encode(&writer)
        let token = nextToken
        nextToken += 1
        replies[token] = BernPendingReply(window: window)
        writer.bytes.withUnsafeBufferPointer { buffer in
            bern_media_dispatch(handle, command, buffer.baseAddress, buffer.count, token)
        }
    }

    func mediaCall(_ call: BernMediaCall, _ bytes: [UInt8]) -> [UInt8] {
        guard let handle else { return [] }
        let buffer = bytes.withUnsafeBufferPointer { pointer in
            switch call {
            case .attachExecutor: bern_media_executor_attached(handle, pointer.baseAddress, pointer.count)
            case .create: bern_media_create(handle, pointer.baseAddress, pointer.count)
            case .bind: bern_media_bind(handle, pointer.baseAddress, pointer.count)
            }
        }
        return Self.take(buffer)
    }

    func mediaPost(_ post: BernMediaPost, _ bytes: [UInt8]) {
        guard let handle else { return }
        bytes.withUnsafeBufferPointer { pointer in
            switch post {
            case .release: bern_media_release(handle, pointer.baseAddress, pointer.count)
            case .view: bern_media_view(handle, pointer.baseAddress, pointer.count)
            case .complete: bern_media_complete(handle, pointer.baseAddress, pointer.count)
            case .event: bern_media_event(handle, pointer.baseAddress, pointer.count)
            }
        }
    }

    /// Read a `0` or `1, code, message` answer.
    func status(_ bytes: [UInt8]) -> BernCommandFailure? {
        var reader = BernReader(bytes)
        guard let status = try? reader.u8() else {
            return BernCommandFailure(code: "engine_stopped", message: "the engine is not running")
        }
        guard status != 0 else { return nil }
        return BernCommandFailure(code: (try? reader.text()) ?? "failed", message: (try? reader.text()) ?? "")
    }

    func receiveReply(_ token: UInt64, _ bytes: [UInt8]) {
        guard let pending = replies.removeValue(forKey: token) else { return }
        var reader = BernReader(bytes)
        do {
            let status = try reader.u8()
            _ = try reader.u64()
            if status == 0 {
                try pending.success?(&reader)
                pending.completion?(nil)
            } else {
                let code = try reader.text()
                let message = try reader.text()
                failure = BernFailure(window: pending.window, code: code, message: message)
                pending.completion?(BernCommandFailure(code: code, message: message))
            }
        } catch {
            failure = BernFailure(window: pending.window, code: "invalid_reply", message: "\(error)")
            pending.completion?(BernCommandFailure(code: "invalid_reply", message: "\(error)"))
        }
    }

    // MARK: Queries

    /// Open a subscription to catalog query `index` for a window instance;
    /// later states of it go to `receive`.
    func queryOpen(
        _ context: BernWindowContext, index: UInt32, receive: @escaping @MainActor ([UInt8]) -> Void
    ) throws -> UInt64 {
        guard let handle, context.generation > 0 else {
            throw BernCommandFailure(code: "target_not_found", message: "the window is not attached")
        }
        var reader = BernReader(Self.take(bern_query_open(handle, context.kind, context.generation, index)))
        guard try reader.u8() == 0 else {
            throw BernCommandFailure(code: try reader.text(), message: try reader.text())
        }
        let subscription = try reader.u64()
        queries[subscription] = receive
        return subscription
    }

    /// Ask for new parameters; returns the subscription's state right away.
    func queryRequest(_ subscription: UInt64, _ params: [UInt8]) throws -> [UInt8] {
        guard let handle else {
            throw BernCommandFailure(code: "engine_stopped", message: "the engine is not running")
        }
        let bytes = params.withUnsafeBufferPointer { buffer in
            Self.take(bern_query_request(handle, subscription, buffer.baseAddress, buffer.count))
        }
        var reader = BernReader(bytes)
        guard try reader.u8() == 0 else {
            throw BernCommandFailure(code: try reader.text(), message: try reader.text())
        }
        return Array(bytes.dropFirst())
    }

    func queryClose(_ subscription: UInt64) {
        queries[subscription] = nil
        if let handle { bern_query_close(handle, subscription) }
    }

    func receiveQuery(_ subscription: UInt64, _ bytes: [UInt8]) {
        queries[subscription]?(bytes)
    }

    // MARK: Windows

    func registerActions(openWindow: OpenWindowAction, openSettings: OpenSettingsAction) {
        self.openWindow = openWindow
        self.openSettings = openSettings
    }

    func attach(_ context: BernWindowContext) {
        guard let handle else { return }
        var reader = BernReader(Self.take(bern_window_opened(handle, context.kind)))
        do {
            guard try reader.u8() == 0 else { return }
            let generation = try reader.u64()
            let id = try reader.text()
            let count = try reader.u32()
            for _ in 0..<count {
                let index = try reader.u32()
                guard let session = context.sessions.first(where: { type(of: $0).bernIndex == index }) else {
                    throw BernDecodeError(message: "unknown session \(index)")
                }
                try session.load(&reader)
            }
            context.attached(generation: generation, id: id)
            windows[context.kindID] = context
            BernSnapshot.schedule()
            for request in pendingOpens.removeValue(forKey: context.kindID) ?? [] {
                complete(request, command: 8) { $0.text(id) }
            }
        } catch {
            failure = BernFailure(window: nil, code: "invalid_window", message: "\(error)")
        }
    }

    func detach(_ context: BernWindowContext) {
        guard let handle, windows[context.kindID] === context else { return }
        windows[context.kindID] = nil
        BernMedia.shared.windowClosed(context.id)
        bern_window_closed(handle, context.kind, context.generation)
    }

    func sessionSet(
        _ context: BernWindowContext, session: UInt32, field: UInt32, _ encode: (inout BernWriter) -> Void
    ) -> Bool {
        guard let handle, context.generation > 0 else { return false }
        var writer = BernWriter()
        encode(&writer)
        let result = writer.bytes.withUnsafeBufferPointer { buffer in
            Self.take(
                bern_session_set(
                    handle, context.kind, context.generation, session, field, buffer.baseAddress,
                    buffer.count))
        }
        return result.first == 0
    }

    /// Open a window kind, or bring its instance forward.
    func open(_ kindID: String) {
        guard let kind = kinds.first(where: { $0.id == kindID }) else { return }
        if kind.isSettings {
            openSettings?()
        } else {
            openWindow?(id: kindID)
        }
        NSApplication.shared.activate()
    }

    /// Request closing a window kind's instance through the close protocol.
    func close(_ kindID: String) {
        guard let context = windows[kindID] else { return }
        do {
            _ = try requestClose(context)
        } catch let problem as BernCommandFailure {
            failure = BernFailure(window: context.id, code: problem.code, message: problem.message)
        } catch {}
    }

    // MARK: Close protocol

    /// The system window showing a window kind, for development aids.
    func systemWindow(_ kindID: String) -> NSWindow? {
        windows[kindID]?.nsWindow
    }

    func isBlocked(_ windowID: String) -> Bool {
        windows.values.contains { $0.id == windowID && $0.pendingClose != nil }
    }

    /// Run the window's declared close check and ask the session runtime
    /// whether to close now or to wait for a confirmation decision. A
    /// pending confirmation is returned again rather than asked twice.
    func requestClose(_ context: BernWindowContext) throws -> BernCloseResult {
        guard let handle, context.generation > 0 else {
            throw BernCommandFailure(code: "target_not_found", message: "the window is not attached")
        }
        let check = context.closeCheck
        let needsConfirmation = check?.needsConfirmation() ?? false
        let canSave = check?.save != nil
        var reader = BernReader(
            Self.take(
                bern_window_request_close(
                    handle, context.kind, context.generation, needsConfirmation ? 1 : 0, canSave ? 1 : 0)))
        if try reader.u8() != 0 {
            throw BernCommandFailure(code: try reader.text(), message: try reader.text())
        }
        if try reader.u8() == 0 {
            finishClose(context)
            return BernCloseResult(outcome: .closed, window: context.id)
        }
        let pending = BernPendingClose(
            id: try reader.text(), message: check?.message() ?? "", canSave: canSave,
            draftRevision: try reader.u64())
        if context.pendingClose != pending { context.pendingClose = pending }
        BernSnapshot.schedule()
        return BernCloseResult(
            outcome: .confirmationRequired, window: context.id, confirmation: pending.id,
            draftRevision: pending.draftRevision, decisions: pending.decisions, message: pending.message)
    }

    /// Apply a decision from the dialog or from an external entry. A save
    /// that fails keeps the window open.
    func confirmClose(
        _ context: BernWindowContext, confirmation: String, decision: BernCloseDecision,
        completion: @escaping @MainActor (Result<BernCloseResult, BernCommandFailure>) -> Void
    ) {
        guard let handle else {
            completion(.failure(BernCommandFailure(code: "engine_stopped", message: "the engine is not running")))
            return
        }
        var writer = BernWriter()
        writer.text(confirmation)
        writer.u32(decision.rawValue)
        let bytes = writer.bytes.withUnsafeBufferPointer { buffer in
            Self.take(bern_window_confirm_close(handle, context.kind, context.generation, buffer.baseAddress, buffer.count))
        }
        var reader = BernReader(bytes)
        let revision = context.pendingClose?.draftRevision ?? 0
        do {
            if try reader.u8() != 0 {
                let problem = BernCommandFailure(code: try reader.text(), message: try reader.text())
                // A stale confirmation no longer describes the window.
                if problem.code == "stale_confirmation", context.pendingClose?.id == confirmation {
                    context.pendingClose = nil
                }
                completion(.failure(problem))
                return
            }
            let step = try reader.u8()
            context.pendingClose = nil
            BernSnapshot.schedule()
            switch step {
            case 0:
                finishClose(context)
                completion(.success(BernCloseResult(outcome: .closed, window: context.id, draftRevision: revision)))
            case 1:
                guard let save = context.closeCheck?.save else {
                    completion(.failure(BernCommandFailure(code: "invalid_params", message: "no save command")))
                    return
                }
                save { [weak self] problem in
                    if let problem {
                        completion(
                            .success(
                                BernCloseResult(
                                    outcome: .rejected, window: context.id, draftRevision: revision,
                                    reason: "save_failed: \(problem.code): \(problem.message)")))
                    } else {
                        self?.finishClose(context)
                        completion(.success(BernCloseResult(outcome: .closed, window: context.id, draftRevision: revision)))
                    }
                }
            default:
                completion(
                    .success(
                        BernCloseResult(outcome: .rejected, window: context.id, draftRevision: revision, reason: "cancelled")))
            }
        } catch {
            completion(.failure(BernCommandFailure(code: "invalid_reply", message: "\(error)")))
        }
    }

    /// Decide from the window's own dialog.
    func decide(_ context: BernWindowContext, _ pending: BernPendingClose, _ decision: BernCloseDecision) {
        confirmClose(context, confirmation: pending.id, decision: decision) { _ in }
    }

    private func finishClose(_ context: BernWindowContext) {
        context.allowsClose = true
        if let window = context.nsWindow {
            window.close()
        } else {
            context.dismiss?()
        }
    }

    /// Open windows declared `initial: open` once per session.
    func openInitial(_ kindIDs: [String]) {
        guard !openedInitial else { return }
        openedInitial = true
        for kindID in kindIDs { open(kindID) }
    }

    // MARK: Session commands from external entries

    func receiveSessionRequest(_ request: UInt64, _ bytes: [UInt8]) {
        var reader = BernReader(bytes)
        do {
            let command = try reader.u32()
            try run(command, request: request, &reader)
        } catch let failure as BernCommandFailure {
            fail(request, failure)
        } catch {
            fail(request, BernCommandFailure(code: "invalid_request", message: "\(error)"))
        }
    }

    private func fail(_ request: UInt64, _ failure: BernCommandFailure) {
        var writer = BernWriter()
        writer.u8(1)
        writer.text(failure.code)
        writer.text(failure.message)
        send(request, writer.bytes)
    }

    private func complete(_ request: UInt64, command: UInt32, _ result: (inout BernWriter) -> Void) {
        var writer = BernWriter()
        writer.u8(0)
        writer.u32(command)
        result(&writer)
        send(request, writer.bytes)
    }

    private func send(_ request: UInt64, _ bytes: [UInt8]) {
        guard let handle else { return }
        bytes.withUnsafeBufferPointer { buffer in
            bern_session_complete(handle, request, buffer.baseAddress, buffer.count)
        }
        BernSnapshot.schedule()
    }

    private func window(_ id: String) throws -> BernWindowContext {
        guard let context = windows.values.first(where: { $0.id == id }) else {
            throw BernCommandFailure(code: "target_not_found", message: "no open window `\(id)`")
        }
        return context
    }

    /// Command indices follow the framework session catalog order.
    private func run(_ command: UInt32, request: UInt64, _ reader: inout BernReader) throws {
        switch command {
        case 0:  // ui.inspect
            let entries = controls.all
            complete(request, command: command) { writer in
                writer.u32(UInt32(entries.count))
                for entry in entries {
                    writer.text(entry.target)
                    writer.text(entry.window)
                    writer.text(entry.role)
                    writer.text(entry.label)
                    writer.u8(entry.enabled ? 1 : 0)
                    let value: String? = entry.actions.value.flatMap { $0() }
                    value.encode(bern: &writer)
                    let reason: String? =
                        isBlocked(entry.window) ? "blocked_by_modal" : (entry.enabled ? nil : "disabled")
                    reason.encode(bern: &writer)
                }
            }
        case 1:  // ui.activate
            let entry = try controls.find(try reader.text())
            guard let activate = entry.actions.activate else { throw unsupported(entry, "activate") }
            activate()
            complete(request, command: command) { _ in }
        case 2:  // ui.edit
            let entry = try controls.find(try reader.text())
            let value = try reader.text()
            guard let edit = entry.actions.edit else { throw unsupported(entry, "edit") }
            edit(value)
            complete(request, command: command) { _ in }
        case 3:  // ui.submit
            let entry = try controls.find(try reader.text())
            guard let submit = entry.actions.submit else { throw unsupported(entry, "submit") }
            submit()
            complete(request, command: command) { _ in }
        case 4:  // ui.set_bool
            let entry = try controls.find(try reader.text())
            let value = try Bool(bern: &reader)
            guard let set = entry.actions.setBool else { throw unsupported(entry, "set_bool") }
            set(value)
            complete(request, command: command) { _ in }
        case 5:  // ui.set_number
            let entry = try controls.find(try reader.text())
            let value = try reader.f64()
            guard let set = entry.actions.setNumber else { throw unsupported(entry, "set_number") }
            set(value)
            complete(request, command: command) { _ in }
        case 6:  // ui.select
            let entry = try controls.find(try reader.text())
            let option = try reader.text()
            guard let select = entry.actions.select else { throw unsupported(entry, "select") }
            guard select(option) else {
                throw BernCommandFailure(code: "invalid_params", message: "`\(entry.target)` has no option `\(option)`")
            }
            complete(request, command: command) { _ in }
        case 7:  // ui.back
            let entry = try controls.find(try reader.text())
            guard let back = entry.actions.back else { throw unsupported(entry, "back") }
            guard back() else {
                throw BernCommandFailure(code: "target_unavailable", message: "nothing to go back to")
            }
            complete(request, command: command) { _ in }
        case 8:  // window.open
            let kindID = try reader.text()
            guard kinds.contains(where: { $0.id == kindID }) else {
                throw BernCommandFailure(code: "target_not_found", message: "no window kind `\(kindID)`")
            }
            if let existing = windows[kindID] {
                open(kindID)
                complete(request, command: command) { $0.text(existing.id) }
            } else {
                pendingOpens[kindID, default: []].append(request)
                open(kindID)
            }
        case 9:  // window.close
            let context = try window(try reader.text())
            let result = try requestClose(context)
            complete(request, command: command) { result.encode(&$0) }
        case 10:  // window.activate
            let context = try window(try reader.text())
            open(context.kindID)
            complete(request, command: command) { _ in }
        case 11:  // window.confirm_close
            let context = try window(try reader.text())
            let confirmation = try reader.text()
            guard let decision = BernCloseDecision(rawValue: try reader.u32()) else {
                throw BernCommandFailure(code: "invalid_params", message: "unknown close decision")
            }
            confirmClose(context, confirmation: confirmation, decision: decision) { [weak self] result in
                switch result {
                case .success(let result): self?.complete(request, command: command) { result.encode(&$0) }
                case .failure(let problem): self?.fail(request, problem)
                }
            }
        default:
            throw BernCommandFailure(code: "unknown_command", message: "unknown session command \(command)")
        }
    }

    private func unsupported(_ entry: BernControlEntry, _ action: String) -> BernCommandFailure {
        BernCommandFailure(code: "target_unavailable", message: "`\(entry.target)` (\(entry.role)) does not support \(action)")
    }
}

// MARK: - Close protocol types

/// A window's declared close check, generated from its `Window` declaration.
struct BernCloseCheck {
    /// Whether closing needs a decision, evaluated when closing is requested.
    let needsConfirmation: @MainActor () -> Bool
    /// The declared confirmation message.
    let message: @MainActor () -> String
    /// The declared save command; it reports whether it succeeded.
    let save: (@MainActor (@escaping @MainActor (BernCommandFailure?) -> Void) -> Void)?
}

/// A close confirmation waiting for a decision.
struct BernPendingClose: Identifiable, Equatable {
    let id: String
    let message: String
    let canSave: Bool
    let draftRevision: UInt64

    var decisions: [BernCloseDecision] { canSave ? [.save, .discard, .cancel] : [.discard, .cancel] }
}

/// Decisions in the order of the session catalog's `CloseDecision`.
enum BernCloseDecision: UInt32 {
    case save, discard, cancel
}

/// The session catalog's `CloseResult`, encoded field by field.
struct BernCloseResult {
    enum Outcome: UInt32 {
        case closed, rejected, confirmationRequired
    }

    let outcome: Outcome
    let window: String
    var confirmation: String? = nil
    var draftRevision: UInt64 = 0
    var decisions: [BernCloseDecision] = []
    var message: String? = nil
    var reason: String? = nil

    func encode(_ writer: inout BernWriter) {
        writer.u32(outcome.rawValue)
        writer.text(window)
        confirmation.encode(bern: &writer)
        writer.i64(Int64(draftRevision))
        writer.u32(UInt32(decisions.count))
        for decision in decisions { writer.u32(decision.rawValue) }
        message.encode(bern: &writer)
        reason.encode(bern: &writer)
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
private struct BernWindowReader: NSViewRepresentable {
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

    private static var closeSignal: DispatchSourceSignal?

    /// Development aid, off unless `BERN_DEV_CLOSE_SIGNAL` names a window
    /// kind: SIGUSR1 then calls `performClose(_:)` on that window, exactly
    /// what its title-bar close button does, so tools without screen access
    /// can exercise the system close path.
    static func installCloseSignal() {
        guard closeSignal == nil,
            let kind = ProcessInfo.processInfo.environment["BERN_DEV_CLOSE_SIGNAL"]
        else { return }
        signal(SIGUSR1, SIG_IGN)
        let source = DispatchSource.makeSignalSource(signal: SIGUSR1, queue: .main)
        source.setEventHandler {
            MainActor.assumeIsolated { BernHost.shared.systemWindow(kind)?.performClose(nil) }
        }
        source.resume()
        closeSignal = source
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

// MARK: - Queries

/// What a query subscription shows, as the session runtime reports it.
enum BernQueryPhase<Data> {
    case idle
    case loading
    case ready(Data)
    /// New parameters are loading; the previous data stays visible.
    case refreshing(Data)
    /// The last request failed; data from the last success stays visible.
    case failed(previous: Data?, code: String, message: String)
}

/// Releases a query subscription and its control registration when the
/// owning view's identity ends — also when its window closes.
final class BernQueryOwner: @unchecked Sendable {
    var subscription: UInt64?
    var control: UUID?

    deinit {
        let subscription = subscription
        let control = control
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                if let subscription { BernHost.shared.queryClose(subscription) }
                if let control { BernHost.shared.controls.remove(control) }
            }
        }
    }
}

/// One subscription to a catalog query. Every parameter change is a new
/// request with a new version; the session runtime cancels the previous
/// request and forwards only the answer for the current version, and
/// states older than the last one shown are ignored here as well.
@MainActor
@Observable
final class BernQuery<Data: BernCodable> {
    private(set) var phase: BernQueryPhase<Data> = .idle
    @ObservationIgnored let index: UInt32
    @ObservationIgnored private var version: UInt64 = 0
    @ObservationIgnored private let owner = BernQueryOwner()
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var requested: [UInt8]?
    @ObservationIgnored private var wanted: (window: BernWindowContext, params: [UInt8], target: String)?
    @ObservationIgnored private var scheduled = false
    @ObservationIgnored var count: ((Data) -> Int)?

    init(index: UInt32) {
        self.index = index
    }

    /// Called while the view renders: remember the parameters it shows and
    /// request them after the update, never during it.
    func want(_ window: BernWindowContext, params: [UInt8], target: String) {
        guard window.generation > 0 else { return }
        if window.generation == generation, params == requested, owner.subscription != nil {
            wanted = nil
            return
        }
        wanted = (window, params, target)
        guard !scheduled else { return }
        scheduled = true
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated { self?.flush() }
        }
    }

    private func flush() {
        scheduled = false
        guard let (window, params, target) = wanted else { return }
        wanted = nil
        let host = BernHost.shared
        do {
            if owner.subscription != nil, generation != window.generation {
                close()
            }
            if owner.subscription == nil {
                owner.subscription = try host.queryOpen(window, index: index) { [weak self] bytes in
                    self?.receive(bytes)
                }
                generation = window.generation
                register(window, target: target)
            }
            if params != requested { request(params) }
        } catch let problem as BernCommandFailure {
            phase = .failed(previous: nil, code: problem.code, message: problem.message)
        } catch {}
    }

    private func request(_ params: [UInt8]) {
        guard let subscription = owner.subscription else { return }
        requested = params
        do {
            receive(try BernHost.shared.queryRequest(subscription, params))
        } catch let problem as BernCommandFailure {
            phase = .failed(previous: nil, code: problem.code, message: problem.message)
        } catch {}
    }

    /// Ask again with the current parameters.
    func retry() {
        if let requested { request(requested) }
    }

    private func close() {
        if let subscription = owner.subscription { BernHost.shared.queryClose(subscription) }
        owner.subscription = nil
        requested = nil
        version = 0
    }

    /// The query as a control: `ui.inspect` reports its state and
    /// `ui.activate` asks again.
    private func register(_ window: BernWindowContext, target: String) {
        let token = owner.control ?? UUID()
        owner.control = token
        BernHost.shared.controls.register(
            token,
            BernControlEntry(
                target: target, window: window.id, role: "query", label: "", enabled: true,
                actions: BernControlActions(
                    activate: { [weak self] in self?.retry() },
                    value: { [weak self] in self?.summary })))
    }

    func receive(_ bytes: [UInt8]) {
        var reader = BernReader(bytes)
        do {
            let next = try reader.u64()
            guard next >= version else { return }
            let phase: BernQueryPhase<Data>
            switch try reader.u8() {
            case 0: phase = .idle
            case 1: phase = .loading
            case 2: phase = .ready(try Data(bern: &reader))
            case 3: phase = .refreshing(try Data(bern: &reader))
            case 4:
                let previous = try reader.u8() == 1 ? try Data(bern: &reader) : nil
                phase = .failed(previous: previous, code: try reader.text(), message: try reader.text())
            default: throw BernDecodeError(message: "unknown query phase")
            }
            version = next
            self.phase = phase
            BernSnapshot.schedule()
        } catch {
            phase = .failed(previous: nil, code: "invalid_state", message: "\(error)")
        }
    }

    /// Machine-readable state for `ui.inspect`.
    var summary: String {
        let size = { (data: Data) in self.count.map { " (\($0(data)))" } ?? "" }
        switch phase {
        case .idle: return "idle"
        case .loading: return "loading v\(version)"
        case .ready(let data): return "ready v\(version)\(size(data))"
        case .refreshing(let data): return "refreshing v\(version)\(size(data))"
        case .failed(let previous, let code, let message):
            return "failed v\(version) \(code): \(message)\(previous.map(size) ?? "")"
        }
    }
}

/// A `Query` declaration: owns its subscription and shows each state with
/// standard controls. `shown` maps the data to what the template shows,
/// or `nil` when it is empty.
struct BernQueryView<Data: BernCodable, Shown, Content: View>: View {
    let target: String
    let params: [UInt8]
    let loading: String
    let empty: String?
    let emptyIcon: String
    let failed: String
    let shown: (Data) -> Shown?
    let content: (Shown) -> Content
    @Environment(BernWindowContext.self) private var window
    @State private var query: BernQuery<Data>

    init(
        query index: UInt32, target: String, params: [UInt8], loading: String, empty: String?,
        emptyIcon: String, failed: String, count: ((Data) -> Int)?, shown: @escaping (Data) -> Shown?,
        @ViewBuilder content: @escaping (Shown) -> Content
    ) {
        self.target = target
        self.params = params
        self.loading = loading
        self.empty = empty
        self.emptyIcon = emptyIcon
        self.failed = failed
        self.shown = shown
        self.content = content
        let query = BernQuery<Data>(index: index)
        query.count = count
        _query = State(initialValue: query)
    }

    var body: some View {
        let _ = query.want(window, params: params, target: target)
        switch query.phase {
        case .idle, .loading:
            BernQueryProgress(text: loading)
        case .ready(let data):
            presented(data)
        case .refreshing(let data):
            BernQueryProgress(text: loading)
            presented(data)
        case .failed(let previous, _, let message):
            if let previous {
                BernQueryFailureRow(title: failed, message: message) { query.retry() }
                presented(previous)
            } else {
                ContentUnavailableView {
                    Label(failed, systemImage: "exclamationmark.triangle")
                } description: {
                    Text(verbatim: message)
                } actions: {
                    Button(BernText.string("bern.query.retry")) { query.retry() }
                }
                .frame(maxWidth: .infinity)
            }
        }
    }

    @ViewBuilder private func presented(_ data: Data) -> some View {
        if let value = shown(data) {
            content(value)
        } else if let empty {
            ContentUnavailableView(empty, systemImage: emptyIcon)
                .frame(maxWidth: .infinity)
        }
    }
}

/// Small progress row shown while a query loads or refreshes.
struct BernQueryProgress: View {
    let text: String

    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(verbatim: text).foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
    }
}

/// A failure shown next to the data from the last success.
struct BernQueryFailureRow: View {
    let title: String
    let message: String
    let retry: () -> Void

    var body: some View {
        HStack {
            Label {
                VStack(alignment: .leading, spacing: 2) {
                    Text(verbatim: title)
                    Text(verbatim: message).font(.footnote).foregroundStyle(.secondary)
                }
            } icon: {
                Image(systemName: "exclamationmark.triangle")
            }
            Spacer()
            Button(BernText.string("bern.query.retry"), action: retry)
        }
    }
}

// MARK: - Window roots

/// Root of every window: attaches the instance to the session runtime,
/// provides environment objects and presents command failures.
struct BernWindowRoot<Content: View>: View {
    let context: BernWindowContext
    let content: Content
    @Environment(\.openWindow) private var openWindow
    @Environment(\.openSettings) private var openSettings
    @Environment(\.dismiss) private var dismiss

    init(context: BernWindowContext, @ViewBuilder content: () -> Content) {
        self.context = context
        self.content = content()
    }

    var body: some View {
        let host = BernHost.shared
        Group {
            if let error = host.startupError {
                ContentUnavailableView {
                    Label(BernText.string("bern.startup.failed"), systemImage: "exclamationmark.triangle")
                } description: {
                    Text(verbatim: error)
                }
            } else {
                content
            }
        }
        .alert(
            context.pendingClose?.message ?? "",
            isPresented: Binding(get: { context.pendingClose != nil }, set: { _ in }),
            presenting: context.pendingClose
        ) { pending in
            if pending.canSave {
                Button(BernText.string("bern.close.save")) { host.decide(context, pending, .save) }
                Button(BernText.string("bern.close.dont_save"), role: .destructive) {
                    host.decide(context, pending, .discard)
                }
            } else {
                Button(BernText.string("bern.close.discard"), role: .destructive) {
                    host.decide(context, pending, .discard)
                }
            }
            Button(BernText.string("bern.close.cancel"), role: .cancel) { host.decide(context, pending, .cancel) }
        } message: { pending in
            Text(verbatim: BernText.string(pending.canSave ? "bern.close.save_detail" : "bern.close.discard_detail"))
        }
        .overlay(alignment: .topLeading) {
            BernWindowReader { [context] window in context.found(window) }
                .frame(width: 0, height: 0)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
        }
        .environment(context)
        .onAppear {
            BernSnapshot.applyAppearance()
            BernSnapshot.installCloseSignal()
            host.registerActions(openWindow: openWindow, openSettings: openSettings)
            context.dismiss = dismiss
            host.attach(context)
        }
        .onDisappear { host.detach(context) }
        .alert(
            BernText.string("bern.alert.failed"),
            isPresented: Binding(
                get: { host.failure.map { $0.window == nil || $0.window == context.id } ?? false },
                set: { if !$0 { host.failure = nil } }),
            presenting: host.failure
        ) { _ in
            Button(BernText.string("bern.alert.ok")) { host.failure = nil }
        } message: { failure in
            Text(verbatim: failure.message)
        }
    }
}

final class BernAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationWillTerminate(_ notification: Notification) {
        MainActor.assumeIsolated { BernHost.shared.stop() }
    }
}
