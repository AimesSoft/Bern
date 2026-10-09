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
        self.generation = generation
        self.id = id
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

@MainActor
@Observable
final class BernHost {
    static let shared = BernHost()

    @ObservationIgnored private var handle: OpaquePointer?
    @ObservationIgnored private var replies: [UInt64: (window: String?, success: ((inout BernReader) throws -> Void)?)] = [:]
    @ObservationIgnored private var nextToken: UInt64 = 1
    @ObservationIgnored private var revision: UInt64 = 0
    @ObservationIgnored private var apply: ((UInt32, inout BernReader) throws -> Void)?
    @ObservationIgnored private var kinds: [BernWindowKind] = []
    @ObservationIgnored private var windows: [String: BernWindowContext] = [:]
    @ObservationIgnored private var pendingOpens: [String: [UInt64]] = [:]
    @ObservationIgnored private var openWindow: OpenWindowAction?
    @ObservationIgnored private var openSettings: OpenSettingsAction?
    @ObservationIgnored private var openedInitial = false
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
            on_session_request: bernOnSessionRequest)
        guard let app = bern_app_start(fingerprint, callbacks, &error) else {
            startupError = String(decoding: Self.take(error), as: UTF8.self)
            return
        }
        _ = Self.take(error)
        handle = app
        engineID = String(decoding: Self.take(bern_app_engine_id(app)), as: UTF8.self)
        channel = String(decoding: Self.take(bern_app_channel(app)), as: UTF8.self)
        receiveState(Self.take(bern_app_snapshot(app)))
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
        guard let handle else { return }
        var writer = BernWriter()
        encode(&writer)
        let token = nextToken
        nextToken += 1
        replies[token] = (window, success)
        writer.bytes.withUnsafeBufferPointer { buffer in
            bern_app_dispatch(handle, command, buffer.baseAddress, buffer.count, token)
        }
    }

    func receiveReply(_ token: UInt64, _ bytes: [UInt8]) {
        guard let pending = replies.removeValue(forKey: token) else { return }
        var reader = BernReader(bytes)
        do {
            let status = try reader.u8()
            _ = try reader.u64()
            if status == 0 {
                try pending.success?(&reader)
            } else {
                failure = BernFailure(
                    window: pending.window, code: try reader.text(), message: try reader.text())
            }
        } catch {
            failure = BernFailure(window: pending.window, code: "invalid_reply", message: "\(error)")
        }
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

    func close(_ kindID: String) {
        windows[kindID]?.dismiss?()
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
            context.dismiss?()
            complete(request, command: command) { _ in }
        case 10:  // window.activate
            let context = try window(try reader.text())
            open(context.kindID)
            complete(request, command: command) { _ in }
        default:
            throw BernCommandFailure(code: "unknown_command", message: "unknown session command \(command)")
        }
    }

    private func unsupported(_ entry: BernControlEntry, _ action: String) -> BernCommandFailure {
        BernCommandFailure(code: "target_unavailable", message: "`\(entry.target)` (\(entry.role)) does not support \(action)")
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
            let name = (window.identifier?.rawValue ?? "window-\(window.windowNumber)")
                .replacingOccurrences(of: "/", with: "-")
            let file = directory.appendingPathComponent(String(format: "%03d-%@.png", sequence, name))
            try? image.representation(using: .png, properties: [:])?.write(to: file)
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
        .environment(context)
        .onAppear {
            BernSnapshot.applyAppearance()
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
