// Bern SwiftUI runtime. Generated build input; do not edit.
//
// This file connects generated views to the in-process Rust runtime. It
// decodes typed values, mirrors engine state into observable objects,
// mirrors window-scoped session drafts, and runs session commands from
// external entries through the same control bindings the user operates.
// It contains no business logic; without style overrides every surface
// keeps the system appearance. What differs between macOS and iOS — windows,
// lifecycle, snapshots and the video view — is in BernAppKit.swift and
// BernUIKit.swift, of which a build compiles the one for its platform.

import ImageIO
import SwiftUI
import UniformTypeIdentifiers

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

/// A value a translated text shows.
enum BernArg {
    case text(String)
    /// A number, and the way Bern writes it.
    case number(Double, String)
    case date(year: Int64, month: Int64, day: Int64)
}

/// One part of a parsed text.
enum BernPiece {
    case text(String)
    case value(String)
    /// `#` in a plural branch: the number.
    case number
    case plural(String, [(String, [BernPiece])])
    case date(String, String)
    case duration(String, String)
}

/// Parses a translated text. The compiler checked the format, so the
/// parser trusts its shape.
struct BernTextParser {
    private let chars: [Character]
    private var at = 0

    init(_ text: String) { chars = Array(text) }

    private mutating func word(_ stops: Set<Character>) -> String {
        let start = at
        while at < chars.count, !stops.contains(chars[at]) { at += 1 }
        return String(chars[start..<at]).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Pieces up to the end, or up to the `}` that closes a plural branch.
    mutating func message(branch: Bool) -> [BernPiece] {
        var pieces: [BernPiece] = []
        func push(_ c: Character) {
            if case .text(let text)? = pieces.last {
                pieces[pieces.count - 1] = .text(text + String(c))
            } else {
                pieces.append(.text(String(c)))
            }
        }
        while at < chars.count {
            let c = chars[at]
            let next: Character? = at + 1 < chars.count ? chars[at + 1] : nil
            if c == "{" && next == "{" {
                push("{")
                at += 2
            } else if c == "}" && branch {
                return pieces
            } else if c == "}" && next == "}" {
                push("}")
                at += 2
            } else if c == "#" && branch {
                pieces.append(.number)
                at += 1
            } else if c == "{" {
                at += 1
                pieces.append(placeholder())
            } else {
                push(c)
                at += 1
            }
        }
        return pieces
    }

    private mutating func placeholder() -> BernPiece {
        let name = word([",", "{", "}"])
        guard at < chars.count, chars[at] == "," else {
            at += 1
            return .value(name)
        }
        at += 1
        let kind = word([",", "{", "}"])
        at += 1
        if kind != "plural" {
            let pattern = word(["{", "}"])
            at += 1
            return kind == "date" ? .date(name, pattern) : .duration(name, pattern)
        }
        var branches: [(String, [BernPiece])] = []
        while true {
            while at < chars.count, chars[at].isWhitespace { at += 1 }
            if at >= chars.count || chars[at] == "}" {
                at += 1
                return .plural(name, branches)
            }
            let selector = word(["{", "}"])
            at += 1
            branches.append((selector, message(branch: true)))
            at += 1
        }
    }
}

/// The session's interface language. Every localized text reads its
/// strings through it, so switching re-renders them in one update while
/// drafts, navigation and windows stay. Changed on the main actor only.
@Observable
final class BernLanguage: @unchecked Sendable {
    static let shared = BernLanguage()

    /// The languages the bundle is localized for.
    let supported: [String]
    private(set) var current: String
    private(set) var bundle: Bundle

    private init() {
        // The bundle lists a language once per source of localizations.
        var declared: [String] = []
        for language in Bundle.main.localizations where language != "Base" && !declared.contains(language) {
            declared.append(language)
        }
        supported = declared
        let initial = Bundle.main.preferredLocalizations.first ?? declared.first ?? "en"
        current = initial
        bundle = Self.bundle(for: initial)
    }

    private static func bundle(for language: String) -> Bundle {
        Bundle.main.path(forResource: language, ofType: "lproj").flatMap(Bundle.init(path:)) ?? .main
    }

    /// Switch to a supported language; false for any other tag.
    @discardableResult
    func select(_ language: String) -> Bool {
        guard supported.contains(language) else { return false }
        bundle = Self.bundle(for: language)
        current = language
        return true
    }

    /// A language's own name for itself, such as “简体中文” or “English”.
    static func autonym(_ language: String) -> String {
        Locale(identifier: language).localizedString(forIdentifier: language) ?? language
    }
}

/// Localized resources and Bern's basic, locale-independent formatting.
enum BernText {
    static func string(_ key: String) -> String {
        BernLanguage.shared.bundle.localizedString(forKey: key, value: nil, table: nil)
    }

    /// A framework text with positional `%@` arguments.
    static func format(_ key: String, _ arguments: [String]) -> String {
        String(format: string(key), arguments: arguments.map { $0 as CVarArg })
    }

    /// A translated text with its arguments, in the current language:
    /// text and numbers as they are, plural branches by the number's
    /// category, dates and durations by their patterns. The format is the
    /// one the compiler checks (`bern_protocol::format`).
    static func format(_ key: String, _ arguments: [String: BernArg]) -> String {
        var parser = BernTextParser(string(key))
        let pieces = parser.message(branch: false)
        return write(pieces, BernLanguage.shared.current, arguments, hash: nil)
    }

    static func arg(_ value: String) -> BernArg { .text(value) }

    static func arg(_ value: Int64) -> BernArg { .number(Double(value), String(value)) }

    static func arg(_ value: Double) -> BernArg { .number(value, text(value)) }

    static func arg(_ value: some BernLabeled) -> BernArg { .text(value.bernLabel) }

    private static func write(
        _ pieces: [BernPiece], _ language: String, _ arguments: [String: BernArg], hash: String?
    ) -> String {
        var out = ""
        for piece in pieces {
            switch piece {
            case .text(let text):
                out += text
            case .number:
                out += hash ?? ""
            case .value(let name):
                switch arguments[name] {
                case .text(let text)?: out += text
                case .number(_, let text)?: out += text
                case .date(let year, let month, let day)?:
                    out += "\(year)-\(pad2(month))-\(pad2(day))"
                case nil: break
                }
            case .plural(let name, let branches):
                let (value, shown) = number(arguments[name])
                let exact = "=" + (value.rounded() == value && abs(value) < 1e15 ? String(Int64(value)) : String(value))
                let category = pluralCategory(language, value)
                let chosen = branches.first { $0.0 == exact }
                    ?? branches.first { $0.0 == category }
                    ?? branches.first { $0.0 == "other" }
                if let chosen { out += write(chosen.1, language, arguments, hash: shown) }
            case .date(let name, let pattern):
                if case .date(let year, let month, let day)? = arguments[name] {
                    out += formatDate(pattern, year, month, day)
                }
            case .duration(let name, let pattern):
                out += formatDuration(pattern, number(arguments[name]).0)
            }
        }
        return out
    }

    private static func number(_ arg: BernArg?) -> (Double, String) {
        switch arg {
        case .number(let value, let text)?: return (value, text)
        case .text(let text)?: return (Double(text) ?? 0, text)
        default: return (0, "")
        }
    }

    private static func pluralCategory(_ language: String, _ number: Double) -> String {
        let base = language.split(whereSeparator: { $0 == "-" || $0 == "_" }).first.map(String.init) ?? language
        let oneForm = ["zh", "ja", "ko", "vi", "th", "id", "ms"].contains(base)
        return !oneForm && number == 1 ? "one" : "other"
    }

    private static func pad2(_ value: Int64) -> String {
        value >= 0 && value < 10 ? "0\(value)" : String(value)
    }

    /// Runs of one pattern letter, and the characters between them.
    private static func runs(_ pattern: String) -> [(Character, Int)] {
        var out: [(Character, Int)] = []
        for c in pattern {
            if let last = out.last, last.0 == c, c.isASCII, c.isLetter {
                out[out.count - 1].1 += 1
            } else {
                out.append((c, 1))
            }
        }
        return out
    }

    /// The day of the week, 0 for Monday.
    private static func weekday(_ year: Int64, _ month: Int64, _ day: Int64) -> Int64 {
        func div(_ a: Int64, _ b: Int64) -> Int64 {
            let quotient = a / b
            return a % b != 0 && (a < 0) != (b < 0) ? quotient - 1 : quotient
        }
        let offsets: [Int64] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4]
        let m = min(12, max(1, month))
        let y = m < 3 ? year - 1 : year
        let sundayFirst = ((y + div(y, 4) - div(y, 100) + div(y, 400) + offsets[Int(m - 1)] + day) % 7 + 7) % 7
        return (sundayFirst + 6) % 7
    }

    private static func formatDate(_ pattern: String, _ year: Int64, _ month: Int64, _ day: Int64) -> String {
        var out = ""
        for (c, count) in runs(pattern) {
            switch (c, count) {
            case ("y", 2): out += pad2((year % 100 + 100) % 100)
            case ("y", _): out += String(year)
            case ("M", 1): out += String(month)
            case ("M", 2): out += pad2(month)
            case ("M", 3): out += string("bern.date.month.\(month)")
            case ("M", _): out += string("bern.date.month_long.\(month)")
            case ("d", 1): out += String(day)
            case ("d", _): out += pad2(day)
            case ("E", 3): out += string("bern.date.weekday.\(weekday(year, month, day) + 1)")
            case ("E", _): out += string("bern.date.weekday_long.\(weekday(year, month, day) + 1)")
            default: out += String(repeating: c, count: count)
            }
        }
        return out
    }

    /// Seconds as `h`, `m` and `s` fields; the largest field written takes
    /// what the larger ones would hold.
    private static func formatDuration(_ pattern: String, _ seconds: Double) -> String {
        let total = seconds.isFinite ? Int64(max(0, seconds).rounded(.down)) : 0
        let hours = pattern.contains("h")
        let minutes = pattern.contains("m")
        var out = ""
        for (c, count) in runs(pattern) {
            let value: Int64? = switch c {
            case "h": total / 3600
            case "m": hours ? total % 3600 / 60 : total / 60
            case "s": hours || minutes ? total % 60 : total
            default: nil
            }
            if let value {
                out += count == 2 ? pad2(value) : String(value)
            } else {
                out += String(repeating: c, count: count)
            }
        }
        return out
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
        didSet { closeCheckChanged() }
    }
    /// The close confirmation shown as this window's dialog. While it is
    /// pending, the window's controls are blocked for every entry.
    var pendingClose: BernPendingClose?
    /// What the platform keeps about the window: the system window on
    /// macOS, the presentation on iOS.
    @ObservationIgnored let platform = BernWindowPlatform()
    /// Set while a window command runs, so the facts it causes are
    /// reported as caused by a command rather than by the user.
    @ObservationIgnored var commandInFlight = false
    @ObservationIgnored var allowsClose = false
    /// Whether the window had no input for its `idle_after` seconds.
    var idle = false
    /// The width of the window's content, for `window.compact` and its
    /// siblings.
    var width: CGFloat = 0
    /// The height of the window's content, for `window.short`.
    var height: CGFloat = 0
    /// Whether the window has shown its first frame: elements appearing
    /// after it move in, those of the first frame do not.
    var settled = false

    private var size: String { bernAppBreakpoints.size(of: width > 0 ? width : .infinity) }
    var compact: Bool { size == "compact" }
    var medium: Bool { size == "medium" }
    var expanded: Bool { size == "expanded" }
    var large: Bool { size == "large" }
    /// Shorter than the compact breakpoint, as a phone on its side.
    var short: Bool { height > 0 && height < bernAppBreakpoints.compact }
    private(set) var generation: UInt64 = 0
    private(set) var id: String = ""

    init(kind: UInt32, kindID: String, isSettings: Bool, sessions: [any BernSessionState]) {
        self.kind = kind
        self.kindID = kindID
        self.isSettings = isSettings
        self.sessions = sessions
        for session in sessions { session.window = self }
    }

    /// The window's title in the interface language.
    var title: String {
        BernHost.shared.kinds.first { $0.id == kindID }.map { BernText.string($0.title) } ?? ""
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
    var focus: (@MainActor () -> Void)? = nil
    /// Takes a file as if the user chose or dropped it.
    var setFile: (@MainActor (URL) -> Void)? = nil
    /// The kinds of file `setFile` takes, as `files:` names them.
    var fileKinds = "any"
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

private struct BernItemKeysKey: EnvironmentKey {
    static let defaultValue = ""
}

extension EnvironmentValues {
    /// Keys of the rows around a view, outermost first, as `[a][b]`.
    var bernItemKeys: String {
        get { self[BernItemKeysKey.self] }
        set { self[BernItemKeysKey.self] = newValue }
    }
}

extension View {
    /// A row of a repetition: controls inside it, also in components it
    /// places, add `[key]` to their targets.
    func bernItemKey(_ key: some CustomStringConvertible) -> some View {
        transformEnvironment(\.bernItemKeys) { $0 += "[\(key)]" }
    }
}

private struct BernControl: ViewModifier {
    let name: String
    let role: String
    let label: String
    let enabled: Bool
    let actions: BernControlActions
    @Environment(BernWindowContext.self) private var window
    @Environment(\.bernItemKeys) private var keys
    @State private var token = UUID()

    init(target: String, role: String, label: String, enabled: Bool, actions: BernControlActions) {
        self.name = target
        self.role = role
        self.label = label
        self.enabled = enabled
        self.actions = actions
    }

    private var target: String { name + keys }

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

/// Chooses one layout by the available width. Engine state and session
/// drafts live outside the view tree, so switching keeps them, and controls
/// keep their identity through the same `id` in both layouts.
/// The breakpoints of `[layouts]`: below `compact` the size is compact,
/// below `medium` medium, below `expanded` expanded, and large beyond.
struct BernBreakpoints: Sendable {
    let compact: CGFloat
    let medium: CGFloat?
    let expanded: CGFloat?

    static let sizes = ["compact", "medium", "expanded", "large"]

    func size(of width: CGFloat) -> String {
        if width < compact { return "compact" }
        if let medium, width < medium { return "medium" }
        if let expanded, width >= expanded { return "large" }
        return "expanded"
    }

    /// The layout a size shows: its own, else the next larger declared
    /// one, else the next smaller.
    static func layout(for size: String, declared: [String]) -> String {
        if declared.contains(size) { return size }
        let at = sizes.firstIndex(of: size) ?? 2
        let order = Array(sizes[(at + 1)...]) + Array(sizes[..<at].reversed())
        return order.first(where: declared.contains) ?? "expanded"
    }
}

/// One layout per size of the available width. The content builds the
/// layout named; external entries read the size.
struct BernAdaptive<Content: View>: View {
    /// The size external entries read; a reference, so the registered
    /// control always reports the current one.
    @MainActor
    final class Current {
        var size = "expanded"
    }

    let breakpoints: BernBreakpoints
    let declared: [String]
    let target: String?
    let content: (String) -> Content
    @State private var width: CGFloat?
    @State private var current = Current()

    init(
        breakpoints: BernBreakpoints, declared: [String], target: String?,
        @ViewBuilder content: @escaping (String) -> Content
    ) {
        self.breakpoints = breakpoints
        self.declared = declared
        self.target = target
        self.content = content
    }

    private var size: String { breakpoints.size(of: width ?? .infinity) }

    var body: some View {
        // The width measured is the room given, not the layout's own: a
        // frame without a minimum is at least as wide as its content, so a
        // wide layout chosen first would keep measuring itself wide.
        let chosen = content(BernBreakpoints.layout(for: size, declared: declared))
            .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
            .onChange(of: size, initial: true) { current.size = size }
        if let target {
            let current = self.current
            chosen.bernControl(
                target, role: "adaptive", label: "", actions: BernControlActions(value: { current.size }))
        } else {
            chosen
        }
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
    /// The localization key of the window's title.
    let title: String
    let isSettings: Bool
    /// Shown modally over the primary window.
    var isDialog = false
    /// The declared content size, used when a window has no other size to
    /// return to.
    let defaultSize: CGSize?
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

    @ObservationIgnored private(set) var handle: OpaquePointer?
    @ObservationIgnored private var replies: [UInt64: BernPendingReply] = [:]
    @ObservationIgnored private var nextToken: UInt64 = 1
    @ObservationIgnored private var revision: UInt64 = 0
    @ObservationIgnored private var apply: ((UInt32, inout BernReader) throws -> Void)?
    @ObservationIgnored private(set) var kinds: [BernWindowKind] = []
    @ObservationIgnored private(set) var primary = ""
    @ObservationIgnored private(set) var windows: [String: BernWindowContext] = [:]
    @ObservationIgnored private var pendingOpens: [String: [UInt64]] = [:]
    /// How the platform presents windows.
    @ObservationIgnored let platform = BernHostPlatform()
    @ObservationIgnored private var openedInitial = false
    @ObservationIgnored private var queries: [UInt64: @MainActor ([UInt8]) -> Void] = [:]
    @ObservationIgnored let controls = BernControls()

    var failure: BernFailure?
    private(set) var startupError: String?
    private(set) var engineID = ""
    private(set) var channel = ""

    /// Start the engine in this process and load the first state snapshot.
    func start(
        fingerprint: UInt64, kinds: [BernWindowKind], primary: String,
        apply: @escaping (UInt32, inout BernReader) throws -> Void
    ) {
        guard handle == nil else { return }
        platformWillStart()
        self.kinds = kinds
        self.primary = primary
        self.apply = apply
        var error = BernBuffer(ptr: nil, len: 0, capacity: 0)
        let callbacks = BernCallbacks(
            context: nil, on_state: bernOnState, on_reply: bernOnReply,
            on_session_request: bernOnSessionRequest, on_media_request: bernOnMediaRequest,
            on_media_state: bernOnMediaState, on_query: bernOnQuery)
        // Declared resources live in the bundle's Resources directory; the
        // engine resolves them by name through the host.
        let resources = Array((Bundle.main.resourceURL?.path ?? "").utf8)
        let started = resources.withUnsafeBufferPointer { pointer in
            bern_app_start(fingerprint, pointer.baseAddress, pointer.count, callbacks, &error)
        }
        guard let app = started else {
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
        platformDidStart()
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
            context.reportFacts()
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

    func unavailable(_ context: BernWindowContext, _ action: String) -> BernCommandFailure {
        BernCommandFailure(code: "target_unavailable", message: "`\(context.id)` cannot \(action) now")
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
        try checkClosable(context)
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

    func fail(_ request: UInt64, _ failure: BernCommandFailure) {
        var writer = BernWriter()
        writer.u8(1)
        writer.text(failure.code)
        writer.text(failure.message)
        send(request, writer.bytes)
    }

    func complete(_ request: UInt64, command: UInt32, _ result: (inout BernWriter) -> Void) {
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
            try activate(context)
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
        case 12:  // window.list is answered by the session runtime
            throw BernCommandFailure(code: "unknown_command", message: "window.list is answered by the runtime")
        case 13:  // window.minimize
            try minimize(try window(try reader.text()), request: request, command: command)
        case 14:  // window.restore
            try restore(try window(try reader.text()), request: request, command: command)
        case 15:  // window.maximize
            try maximize(try window(try reader.text()), request: request, command: command)
        case 16:  // window.move
            let context = try window(try reader.text())
            try move(context, x: try reader.f64(), y: try reader.f64(), request: request, command: command)
        case 17:  // window.resize
            let context = try window(try reader.text())
            try resize(context, width: try reader.f64(), height: try reader.f64(), request: request, command: command)
        case 18:  // ui.language
            complete(request, command: command) { writer in writer.text(BernLanguage.shared.current) }
        case 19:  // ui.set_language
            let language = try reader.text()
            guard BernLanguage.shared.select(language) else {
                throw BernCommandFailure(
                    code: "unsupported_language",
                    message: "`\(language)` is not supported; supported: \(BernLanguage.shared.supported.joined(separator: ", "))")
            }
            complete(request, command: command) { _ in }
        case 20:  // ui.focus
            let entry = try controls.find(try reader.text())
            guard let focus = entry.actions.focus else { throw unsupported(entry, "focus") }
            focus()
            complete(request, command: command) { _ in }
        case 21:  // ui.set_file
            let entry = try controls.find(try reader.text())
            let text = try reader.text()
            guard let give = entry.actions.setFile else { throw unsupported(entry, "set_file") }
            guard let url = URL(string: text), ["file", "http", "https"].contains(url.scheme?.lowercased() ?? "")
            else {
                throw BernCommandFailure(code: "invalid_url", message: "`\(text)` is not a file:// or http(s) URL")
            }
            guard BernFiles.accepts(url, kinds: entry.actions.fileKinds) else {
                throw BernCommandFailure(
                    code: "unsupported_file",
                    message: "`\(entry.target)` takes \(entry.actions.fileKinds), not `\(url.lastPathComponent)`")
            }
            give(url)
            complete(request, command: command) { _ in }
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
    @ObservationIgnored private var lastReady: Data?
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
        lastReady = nil
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
            case 1: phase = lastReady.map { .refreshing($0) } ?? .loading
            case 2:
                let data = try Data(bern: &reader)
                lastReady = data
                phase = .ready(data)
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
        case .ready(let data), .refreshing(let data):
            // Keep one structural identity while parameters refresh, so
            // keyed grids retain their cells, scroll position and reflow.
            presented(data)
                .overlay(alignment: .topTrailing) {
                    if case .refreshing = query.phase {
                        ProgressView().controlSize(.small).accessibilityLabel(Text(verbatim: loading))
                    }
                }
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
/// `BERN_RENDER=active` draws the controls of every window as in the key
/// window, so window captures taken while another application is in front
/// show what the user sees in the window they use.
let bernRenderActive = ProcessInfo.processInfo.environment["BERN_RENDER"] == "active"

struct BernRenderActive: ViewModifier {
    func body(content: Content) -> some View {
        #if os(macOS)
        if bernRenderActive {
            content.environment(\.controlActiveState, .key)
        } else {
            content
        }
        #else
        content
        #endif
    }
}

struct BernWindowRoot<Content: View>: View {
    let context: BernWindowContext
    let content: Content
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
        .overlayPreferenceValue(BernModalContent.self) { modal in
            // The modal arrives as a preference, after the change that
            // presents it. Its layer fades in and out here, so its coming
            // and going animate without animating the rest of the window;
            // its panel enters on appear.
            ZStack {
                if let modal { modal.transition(.opacity) }
            }
            .animation(BernMotion(period: 0.45, damping: 1.0).animation, value: modal != nil)
        }
        .bernHeroRoot()
        .modifier(BernRenderActive())
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
        .modifier(BernWindowPlatformModifier(context: context))
        .onGeometryChange(for: CGSize.self) { $0.size } action: {
            context.width = $0.width
            context.height = $0.height
        }
        .task {
            // After the first frame, appearing elements move in.
            await Task.yield()
            context.settled = true
        }
        .environment(context)
        // The interface language, not the system's, picks localized symbol
        // variants (for example `textformat` is “Aa” in English and “格式”
        // in Chinese) and formats numbers and dates.
        .environment(\.locale, Locale(identifier: BernLanguage.shared.current))
        .onAppear {
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

enum BernImageFit {
    case contain, cover, fill
}

/// A picture from a declared resource in Contents/Resources. With one side
/// given, the other follows the picture's aspect ratio. A label makes it an
/// image for assistive technologies; without one it is decorative.
///
/// HDR pictures show in high dynamic range where the display has headroom:
/// a JPEG or HEIC with a gain map decodes to HDR (macOS 15 and iOS 18 and
/// later), a PQ or HLG PNG keeps its range; elsewhere they are tone mapped.
struct BernImage: View {
    /// A declared resource, or
    var name: String? = nil
    /// a file or http(s) URL from a `MediaSource`.
    var url: String? = nil
    let label: String?
    let width: CGFloat?
    let height: CGFloat?
    let fit: BernImageFit
    var rendering: SwiftUI.Image.TemplateRenderingMode = .original
    /// An explicit width/height ratio fills the proposed grid column.
    var aspect: CGFloat? = nil
    var hero: String? = nil
    var heroMotion = BernMotion(period: 0.48, damping: 0.86)
    var heroCorner: CGFloat = 0
    @State private var fetched: Picture?

    struct Picture: @unchecked Sendable {
        let image: CGImage
        let orientation: SwiftUI.Image.Orientation
        /// Width over height as shown, after the orientation.
        let ratio: CGFloat
    }

    @MainActor private static var loaded: [String: Picture] = [:]

    @MainActor static func picture(_ name: String) -> Picture? {
        if let picture = loaded[name] { return picture }
        guard let url = Bundle.main.resourceURL?.appendingPathComponent(name),
              let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let picture = decode(source) else { return nil }
        loaded[name] = picture
        return picture
    }

    /// The first picture of `source`, in HDR where the system can show it,
    /// with its EXIF orientation.
    nonisolated static func decode(_ source: CGImageSource) -> Picture? {
        var decoded: CGImage?
        // The iOS simulator decodes gain maps to HDR as black, and cannot
        // show HDR anyway; it shows the SDR picture.
        #if !targetEnvironment(simulator)
        if #available(macOS 15, iOS 18, *) {
            decoded = CGImageSourceCreateImageAtIndex(
                source, 0, [kCGImageSourceDecodeRequest: kCGImageSourceDecodeToHDR] as CFDictionary)
        }
        #endif
        guard let image = decoded ?? CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
        // ImageIO leaves the EXIF orientation to the caller.
        let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any]
        let exif = (properties?[kCGImagePropertyOrientation] as? UInt32) ?? 1
        let orientation: SwiftUI.Image.Orientation = switch exif {
            case 2: .upMirrored
            case 3: .down
            case 4: .downMirrored
            case 5: .leftMirrored
            case 6: .right
            case 7: .rightMirrored
            case 8: .left
            default: .up
        }
        let turned = (5...8).contains(exif)
        let w = CGFloat(turned ? image.height : image.width), h = CGFloat(turned ? image.width : image.height)
        return Picture(image: image, orientation: orientation, ratio: w > 0 && h > 0 ? w / h : 1)
    }

    var body: some View {
        let picture = name.flatMap(Self.picture) ?? fetched ?? url.flatMap { BernPictures.shared.cached($0) }
        let ratio = picture?.ratio ?? 1
        let content = Group {
            if let picture {
                let image = SwiftUI.Image(decorative: picture.image, scale: 1, orientation: picture.orientation)
                    .renderingMode(rendering)
                switch fit {
                case .contain: image.resizable().aspectRatio(contentMode: .fit)
                case .cover: image.resizable().aspectRatio(contentMode: .fill)
                case .fill: image.resizable()
                }
            } else {
                Color.clear
            }
        }
        .allowedDynamicRange(.high)
        let shown = Group {
            if let aspect {
                Color.clear
                    .aspectRatio(max(0.01, aspect), contentMode: .fit)
                    // The overlay receives the aspect-fitted proposal directly.
                    // Measuring it and feeding that size back adds a geometry
                    // update for every image on every animated grid resize.
                    .overlay { content }
                    .clipped()
                    .frame(width: width, height: height)
            } else {
                content.frame(width: width ?? height.map { $0 * ratio }, height: height ?? width.map { $0 / ratio }).clipped()
            }
        }
        .task(id: url) {
            guard let url else { return }
            fetched = await BernPictures.shared.load(url)
        }
        .modifier(BernPictureHero(id: hero, picture: picture, fit: fit, corner: heroCorner, motion: heroMotion))
        if let label {
            shown.accessibilityElement().accessibilityLabel(Text(verbatim: label)).accessibilityAddTraits(.isImage)
        } else {
            shown.accessibilityHidden(true)
        }
    }
}

/// Pictures loaded from files and http(s) URLs, kept while the app runs,
/// with the average color of each for fills drawn from a picture.
@MainActor
@Observable
final class BernPictures {
    static let shared = BernPictures()
    @ObservationIgnored private var pictures: [String: BernImage.Picture] = [:]
    @ObservationIgnored private var loading: [String: Task<BernImage.Picture?, Never>] = [:]
    /// Average colors as 0xRRGGBB, by URL; observed, so fills follow.
    private(set) var averages: [String: UInt32] = [:]

    func cached(_ url: String) -> BernImage.Picture? { pictures[url] }

    func load(_ url: String) async -> BernImage.Picture? {
        if let picture = pictures[url] { return picture }
        let task = loading[url] ?? Task.detached(priority: .userInitiated) { () -> BernImage.Picture? in
            guard let location = URL(string: url) else { return nil }
            let data: Data?
            if location.isFileURL {
                data = try? Data(contentsOf: location)
            } else {
                data = try? await URLSession.shared.data(from: location).0
            }
            guard let data, let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
            return BernImage.decode(source)
        }
        loading[url] = task
        let picture = await task.value
        loading[url] = nil
        if let picture {
            pictures[url] = picture
            averages[url] = Self.average(picture.image)
        }
        return picture
    }

    /// The average color of `url`'s picture, loading it when needed.
    func average(_ url: String?) -> BernColor? {
        guard let url else { return nil }
        if let value = averages[url] { return .fixed(light: value, dark: value) }
        if pictures[url] == nil, loading[url] == nil {
            Task { _ = await load(url) }
        }
        return nil
    }

    /// The mean of the picture drawn down to one pixel.
    private static func average(_ image: CGImage) -> UInt32 {
        var pixel = [UInt8](repeating: 0, count: 4)
        guard let context = CGContext(
            data: &pixel, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return 0x808080 }
        context.interpolationQuality = .medium
        context.draw(image, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        return (UInt32(pixel[0]) << 16) | (UInt32(pixel[1]) << 8) | UInt32(pixel[2])
    }
}

// MARK: - Files

/// The kinds of file a control takes (`files:`): categories and `.ext`
/// extensions, separated by spaces.
@MainActor
enum BernFiles {
    /// Containers a category takes besides the system's own types, since
    /// the system may not know some, such as Matroska.
    private static let extensions: [String: [String]] = [/*bern:file-extensions*/]
    private static let categories: [String: [UTType]] = [
        "any": [.item], "video": [.movie, .video], "audio": [.audio], "picture": [.image],
        "folder": [.folder],
    ]

    static func words(_ kinds: String) -> [String] {
        kinds.split(separator: " ").map(String.init)
    }

    /// The content types of `kinds`, for the open panel and for drags.
    static func types(_ kinds: String) -> [UTType] {
        var types: [UTType] = []
        for word in words(kinds) {
            let named = word.hasPrefix(".") ? [String(word.dropFirst())] : extensions[word] ?? []
            for type in (word.hasPrefix(".") ? [] : categories[word] ?? [])
                + named.compactMap({ UTType(filenameExtension: $0) })
            where !types.contains(type) {
                types.append(type)
            }
        }
        return types
    }

    /// Drops that also take the files the system opens with the app, and
    /// files that wait for one.
    private static var openers: [UUID: (kinds: String, give: @MainActor (URL) -> Void)] = [:]
    private static var waiting: [URL] = []

    static func register(_ token: UUID, kinds: String, give: @escaping @MainActor (URL) -> Void) {
        openers[token] = (kinds, give)
        deliver()
    }

    static func unregister(_ token: UUID) {
        openers[token] = nil
    }

    /// Files the system opens with the app: each goes to a drop that takes
    /// its kind. With no such drop on screen the primary window comes back
    /// and the files wait for one; files no drop takes are left alone.
    static func open(_ urls: [URL]) {
        waiting += urls
        if openers.isEmpty { bernShowPrimaryWindow() }
        deliver()
    }

    private static func deliver() {
        guard !openers.isEmpty else { return }
        let urls = waiting
        waiting = []
        for url in urls {
            openers.values.first { accepts(url, kinds: $0.kinds) }?.give(url)
        }
    }

    /// Whether `url` names a file of `kinds`: by its extension, or by the
    /// type the system knows for it.
    static func accepts(_ url: URL, kinds: String) -> Bool {
        let words = words(kinds)
        if words.contains("any") { return true }
        let folder = url.isFileURL
            && ((try? url.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false)
        if folder { return words.contains("folder") }
        let ext = url.pathExtension.lowercased()
        if ext.isEmpty { return false }
        if words.contains("." + ext) { return true }
        let type = UTType(filenameExtension: ext)
        return words.contains { word in
            extensions[word]?.contains(ext) == true
                || (word != "folder" && type.map { type in
                    (categories[word] ?? []).contains { type.conforms(to: $0) }
                } ?? false)
        }
    }
}

/// Content that takes a file of `kinds` dropped onto it. `over` follows
/// whether such a file is held above it; files of other kinds are refused.
/// With `opens`, it also takes the files the system opens with the app.
struct BernDrop: ViewModifier {
    let kinds: String
    let over: Binding<Bool>?
    var opens = false
    let give: @MainActor (URL) -> Void
    @State private var inside = false
    @State private var accepted = false
    @State private var token = UUID()

    func body(content: Content) -> some View {
        content
            .onAppear { if opens { BernFiles.register(token, kinds: kinds, give: give) } }
            .onDisappear { BernFiles.unregister(token) }
            .onDrop(
                of: [.fileURL] + BernFiles.types(kinds),
                delegate: BernDropDelegate(
                    kinds: kinds, inside: $inside, accepted: $accepted, give: give))
            .onChange(of: accepted) { _, accepted in
                if over?.wrappedValue != accepted { over?.wrappedValue = accepted }
            }
    }
}

@MainActor
private struct BernDropDelegate: DropDelegate {
    let kinds: String
    let inside: Binding<Bool>
    let accepted: Binding<Bool>
    let give: @MainActor (URL) -> Void

    func dropEntered(info: DropInfo) {
        inside.wrappedValue = true
        if !BernFiles.words(kinds).contains("folder"),
            info.hasItemsConforming(to: BernFiles.types(kinds))
        {
            accepted.wrappedValue = true
            return
        }
        // The kind of a dragged file URL is only known from the URL.
        guard let provider = info.itemProviders(for: [.fileURL]).first else { return }
        let kinds = kinds
        let inside = inside
        let accepted = accepted
        Self.url(of: provider) { url in
            if inside.wrappedValue {
                accepted.wrappedValue = url.map { BernFiles.accepts($0, kinds: kinds) } ?? false
            }
        }
    }

    func dropExited(info: DropInfo) {
        inside.wrappedValue = false
        accepted.wrappedValue = false
    }

    func dropUpdated(info: DropInfo) -> DropProposal? {
        DropProposal(operation: accepted.wrappedValue ? .copy : .forbidden)
    }

    func validateDrop(info: DropInfo) -> Bool {
        info.hasItemsConforming(to: [.fileURL] + BernFiles.types(kinds))
    }

    func performDrop(info: DropInfo) -> Bool {
        let wanted = accepted.wrappedValue
        inside.wrappedValue = false
        accepted.wrappedValue = false
        guard wanted else { return false }
        let kinds = kinds
        let give = give
        if let provider = info.itemProviders(for: [.fileURL]).first {
            Self.url(of: provider) { url in
                if let url, BernFiles.accepts(url, kinds: kinds) { give(url) }
            }
            return true
        }
        // A file from another app's storage, such as Files on iOS, is
        // opened where it is.
        guard let provider = info.itemProviders(for: BernFiles.types(kinds)).first,
            let type = provider.registeredTypeIdentifiers.first
        else { return false }
        _ = provider.loadInPlaceFileRepresentation(forTypeIdentifier: type) { url, _, _ in
            guard let url else { return }
            Task { @MainActor in
                _ = url.startAccessingSecurityScopedResource()
                give(url)
            }
        }
        return true
    }

    private static func url(of provider: NSItemProvider, then: @escaping @MainActor (URL?) -> Void) {
        _ = provider.loadObject(ofClass: URL.self) { url, _ in
            Task { @MainActor in then(url) }
        }
    }
}

// Fixed logical scenes use a single transform so images, type and hit targets
// stay registered while fitting a smaller hosting window. No app frame clock.
struct BernScaleBox<Content: View>: View {
    let width: CGFloat
    let height: CGFloat
    @ViewBuilder var content: () -> Content
    var body: some View {
        GeometryReader { geometry in
            let scale = min(geometry.size.width / max(1, width), geometry.size.height / max(1, height))
            ZStack(alignment: .topLeading) { content() }
                .frame(width: width, height: height)
                .scaleEffect(scale, anchor: .topLeading)
                .frame(width: width * scale, height: height * scale, alignment: .topLeading)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .clipped()
    }
}

struct BernControlledSlider: View {
    let value: Double
    let minimum: Double
    let maximum: Double
    let step: Double?
    let label: String
    let labelHidden: Bool
    let commit: (Double) -> Void
    @State private var preview = 0.0
    @State private var editing = false
    var body: some View {
        Group {
            if let step {
                Slider(value: $preview, in: minimum...maximum, step: step, onEditingChanged: edit) { Text(label) }
            } else {
                Slider(value: $preview, in: minimum...maximum, onEditingChanged: edit) { Text(label) }
            }
        }
        .modifier(BernSliderLabelHidden(hidden: labelHidden))
        .bernAccentTint()
        .onAppear { preview = value }
        .onChange(of: value) { _, newValue in if !editing { preview = newValue } }
    }
    private func edit(_ active: Bool) {
        editing = active
        if !active { commit(preview) }
    }
}
struct BernSliderLabelHidden: ViewModifier {
    let hidden: Bool
    func body(content: Content) -> some View { if hidden { content.labelsHidden() } else { content } }
}

struct BernMarquee<Content: View>: View {
    let duration: Double
    let phase: Double
    let paused: Bool
    let opacity: Double
    @ViewBuilder var content: () -> Content
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    @Environment(\.bernReduceMotion) private var reduceMotion
    @State private var start = Date()
    @State private var elapsed = 0.0
    @State private var textWidth: CGFloat = 240
    var body: some View {
        GeometryReader { geometry in
            SwiftUI.TimelineView(.animation(paused: paused || reduceMotion || systemReduceMotion)) { clock in
                let time = elapsed + (paused ? 0 : clock.date.timeIntervalSince(start))
                let progress = (phase + time / max(1, duration)).truncatingRemainder(dividingBy: 1)
                content().fixedSize()
                    .background(GeometryReader { size in Color.clear.onAppear { textWidth = size.size.width }.onChange(of: size.size.width) { _, width in textWidth = width } })
                    .shadow(color: .black, radius: 1, y: 1)
                    .offset(x: (reduceMotion || systemReduceMotion) ? geometry.size.width * 0.28 : geometry.size.width - (geometry.size.width + textWidth) * progress)
            }
        }
        .opacity(min(1, max(0, opacity)))
        .clipped()
        .onChange(of: paused) { _, nowPaused in
            if nowPaused { elapsed += Date().timeIntervalSince(start) }
            else { start = Date() }
        }
    }
}
