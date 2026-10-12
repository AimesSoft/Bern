// Bern runtime for Windows: the part shared by both front ends (WinUI 3 and
// the system XAML in XAML Islands). Values and their binary layout,
// reactive cells, localized text, the engine behind the in-process C
// interface, and the media executor. Nothing here depends on a XAML
// namespace. Generated build input; do not edit.

#pragma once

#include <windows.h>
// Before C++/WinRT, so that its objects may implement classic COM
// interfaces.
#include <unknwn.h>
// The macro of the same name hides the XAML and media engine methods.
#undef GetCurrentTime

#include <algorithm>
#include <array>
#include <charconv>
#include <cmath>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <functional>
#include <initializer_list>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <variant>
#include <vector>

#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>

// ---------------------------------------------------------------------------
// The in-process C interface of the engine (`bern_app`, linked statically).

extern "C" {
struct BernApp;

struct BernBuffer {
    uint8_t* ptr;
    size_t len;
    size_t capacity;
};

struct BernCallbacks {
    void* context;
    void (*on_state)(void*, uint8_t const*, size_t);
    void (*on_reply)(void*, uint64_t, uint8_t const*, size_t);
    void (*on_session_request)(void*, uint64_t, uint8_t const*, size_t);
    void (*on_media_request)(void*, uint8_t const*, size_t);
    void (*on_media_state)(void*, uint8_t const*, size_t);
    void (*on_query)(void*, uint64_t, uint8_t const*, size_t);
};

BernApp* bern_app_start(uint64_t fingerprint, uint8_t const* resources, size_t resources_len, BernCallbacks callbacks, BernBuffer* error);
void bern_app_stop(BernApp* app);
BernBuffer bern_app_snapshot(BernApp* app);
BernBuffer bern_app_engine_id(BernApp* app);
void bern_app_dispatch(BernApp* app, uint32_t command, uint8_t const* params, size_t len, uint64_t token);
BernBuffer bern_window_opened(BernApp* app, uint32_t kind);
void bern_window_closed(BernApp* app, uint32_t kind, uint64_t generation);
uint64_t bern_window_facts(BernApp* app, uint32_t kind, uint64_t generation, uint8_t source, uint8_t visibility, uint8_t size,
    double x, double y, double width, double height, uint8_t focused, uint32_t actions);
BernBuffer bern_window_request_close(BernApp* app, uint32_t kind, uint64_t generation, uint8_t needs_confirmation, uint8_t can_save);
BernBuffer bern_window_confirm_close(BernApp* app, uint32_t kind, uint64_t generation, uint8_t const* bytes, size_t len);
BernBuffer bern_session_set(BernApp* app, uint32_t kind, uint64_t generation, uint32_t session, uint32_t field, uint8_t const* value, size_t len);
void bern_session_complete(BernApp* app, uint64_t request, uint8_t const* bytes, size_t len);
BernBuffer bern_query_open(BernApp* app, uint32_t kind, uint64_t generation, uint32_t query);
BernBuffer bern_query_request(BernApp* app, uint64_t subscription, uint8_t const* params, size_t len);
void bern_query_close(BernApp* app, uint64_t subscription);
BernBuffer bern_media_executor_attached(BernApp* app, uint8_t const* bytes, size_t len);
void bern_media_executor_detached(BernApp* app);
BernBuffer bern_media_snapshot(BernApp* app);
void bern_media_dispatch(BernApp* app, uint32_t command, uint8_t const* params, size_t len, uint64_t token);
BernBuffer bern_media_create(BernApp* app, uint8_t const* bytes, size_t len);
BernBuffer bern_media_bind(BernApp* app, uint8_t const* bytes, size_t len);
void bern_media_release(BernApp* app, uint8_t const* bytes, size_t len);
void bern_media_view(BernApp* app, uint8_t const* bytes, size_t len);
void bern_media_complete(BernApp* app, uint8_t const* bytes, size_t len);
void bern_media_event(BernApp* app, uint8_t const* bytes, size_t len);
void bern_buffer_free(BernBuffer buffer);

/// An HDR picture decoded by Rust: RGBA rows in linear scRGB with straight
/// alpha, 1.0 at SDR white.
struct BernPicture {
    uint32_t width;
    uint32_t height;
    float peak;
    float* pixels;
    size_t len;
};

/// 0 not HDR, 1 PQ, 2 HLG, 3 a gain map.
uint32_t bern_picture_kind(uint8_t const* data, size_t len);
/// `headroom` (display peak over SDR white) decides how much of a gain map
/// applies; zero applies all of it.
bool bern_picture_decode(uint8_t const* data, size_t len, float headroom, BernPicture* out);
void bern_picture_free(BernPicture picture);
}

namespace bern {

using Bytes = std::vector<uint8_t>;

/// Text for XAML from UTF-8, and back.
inline winrt::hstring Wide(std::string_view text) { return winrt::to_hstring(text); }
inline std::string Utf8(winrt::hstring const& text) { return winrt::to_string(text); }

/// Write a diagnostic line to standard error.
void Report(std::string_view message);

// ---------------------------------------------------------------------------
// Values in Bern's binary layout.

struct Value;
using Values = std::vector<Value>;

/// A schema value. Texts are UTF-8; enum fields, record fields, list items
/// and the inner value of `Some` share one immutable list.
struct Value {
    enum class Kind : uint8_t { Unit, Bool, Int, Float, Text, Enum, Optional, Record, List };

    Kind kind = Kind::Unit;
    bool b = false;
    int64_t i = 0;
    double f = 0;
    uint32_t variant = 0;
    std::shared_ptr<std::string const> text;
    std::shared_ptr<Values const> items;

    static Value Unit() { return {}; }
    static Value Bool(bool value) { Value v; v.kind = Kind::Bool; v.b = value; return v; }
    static Value Int(int64_t value) { Value v; v.kind = Kind::Int; v.i = value; return v; }
    static Value Float(double value) { Value v; v.kind = Kind::Float; v.f = value; return v; }
    static Value Text(std::string value) { Value v; v.kind = Kind::Text; v.text = std::make_shared<std::string const>(std::move(value)); return v; }
    static Value Enum(uint32_t index, Values fields = {}) { Value v; v.kind = Kind::Enum; v.variant = index; v.items = std::make_shared<Values const>(std::move(fields)); return v; }
    static Value None() { Value v; v.kind = Kind::Optional; v.items = std::make_shared<Values const>(); return v; }
    static Value Some(Value inner) { Value v; v.kind = Kind::Optional; v.items = std::make_shared<Values const>(Values{std::move(inner)}); return v; }
    static Value Record(Values fields) { Value v; v.kind = Kind::Record; v.items = std::make_shared<Values const>(std::move(fields)); return v; }
    static Value List(Values items) { Value v; v.kind = Kind::List; v.items = std::make_shared<Values const>(std::move(items)); return v; }

    Values const& Items() const;
    std::string const& Str() const;
    bool IsSome() const { return kind == Kind::Optional && items && !items->empty(); }

    bool operator==(Value const& other) const;
};

/// A schema type.
struct BType {
    enum class Kind : uint8_t { Unit, Bool, Int, Float, Text, Enum, Record, Optional, List };

    Kind kind = Kind::Unit;
    std::string name;
    std::shared_ptr<BType const> inner;

    static BType Unit() { return {}; }
    static BType Bool() { BType t; t.kind = Kind::Bool; return t; }
    static BType Int() { BType t; t.kind = Kind::Int; return t; }
    static BType Float() { BType t; t.kind = Kind::Float; return t; }
    static BType Text() { BType t; t.kind = Kind::Text; return t; }
    static BType Enum(std::string name) { BType t; t.kind = Kind::Enum; t.name = std::move(name); return t; }
    static BType Record(std::string name) { BType t; t.kind = Kind::Record; t.name = std::move(name); return t; }
    static BType Optional(BType inner) { BType t; t.kind = Kind::Optional; t.inner = std::make_shared<BType const>(std::move(inner)); return t; }
    static BType List(BType inner) { BType t; t.kind = Kind::List; t.inner = std::make_shared<BType const>(std::move(inner)); return t; }
};

struct VariantDef {
    std::string name;
    std::vector<BType> fields;
};

/// Enum and record definitions by name.
struct TypeTable {
    std::unordered_map<std::string, std::vector<VariantDef>> enums;
    std::unordered_map<std::string, std::vector<BType>> records;

    TypeTable& Enum(std::string name, std::vector<VariantDef> variants) { enums[std::move(name)] = std::move(variants); return *this; }
    TypeTable& Record(std::string name, std::vector<BType> fields) { records[std::move(name)] = std::move(fields); return *this; }
};

struct DecodeError : std::runtime_error {
    using std::runtime_error::runtime_error;
};

struct Writer {
    Bytes bytes;

    void U8(uint8_t value) { bytes.push_back(value); }
    void U32(uint32_t value) { Raw(&value, sizeof value); }
    void U64(uint64_t value) { Raw(&value, sizeof value); }
    void I64(int64_t value) { Raw(&value, sizeof value); }
    void F64(double value) { Raw(&value, sizeof value); }
    void Text(std::string_view value) {
        U32(static_cast<uint32_t>(value.size()));
        bytes.insert(bytes.end(), value.begin(), value.end());
    }

private:
    void Raw(void const* data, size_t size) {
        auto first = static_cast<uint8_t const*>(data);
        bytes.insert(bytes.end(), first, first + size);
    }
};

struct Reader {
    std::span<uint8_t const> bytes;
    size_t position = 0;

    explicit Reader(std::span<uint8_t const> source) : bytes(source) {}

    bool AtEnd() const { return position == bytes.size(); }
    uint8_t U8() { return *Take(1); }
    uint32_t U32() { uint32_t v; std::memcpy(&v, Take(4), 4); return v; }
    uint64_t U64() { uint64_t v; std::memcpy(&v, Take(8), 8); return v; }
    int64_t I64() { int64_t v; std::memcpy(&v, Take(8), 8); return v; }
    double F64() { double v; std::memcpy(&v, Take(8), 8); return v; }
    std::string Text() {
        auto length = U32();
        auto data = Take(length);
        return std::string(reinterpret_cast<char const*>(data), length);
    }
    Bytes Rest() const { return Bytes(bytes.begin() + position, bytes.end()); }

private:
    uint8_t const* Take(size_t count) {
        if (position + count > bytes.size()) throw DecodeError("unexpected end of value");
        auto data = bytes.data() + position;
        position += count;
        return data;
    }
};

namespace Codec {
void Encode(Value const& value, BType const& type, TypeTable const& types, Writer& w);
Value Decode(BType const& type, TypeTable const& types, Reader& r);
Bytes ToBytes(Value const& value, BType const& type, TypeTable const& types);
}

/// Operations generated expressions use on schema values. The compiler
/// type-checked every expression, so each helper sees the kinds its
/// operator allows; anything else yields a neutral value.
namespace R {
Value At(Value const& value, std::initializer_list<int> path);
inline bool Truthy(Value const& value) { return value.kind == Value::Kind::Bool && value.b; }
inline Value Not(Value const& value) { return Value::Bool(!Truthy(value)); }
Value Neg(Value const& value);
inline Value Eq(Value const& a, Value const& b) { return Value::Bool(a == b); }
inline Value Ne(Value const& a, Value const& b) { return Value::Bool(!(a == b)); }
Value Lt(Value const& a, Value const& b);
Value Le(Value const& a, Value const& b);
Value Gt(Value const& a, Value const& b);
Value Ge(Value const& a, Value const& b);
template <class F> Value And(Value const& a, F&& b) { return Value::Bool(Truthy(a) && Truthy(b())); }
template <class F> Value Or(Value const& a, F&& b) { return Value::Bool(Truthy(a) || Truthy(b())); }
template <class... F> Value Variant(uint32_t index, F&&... fields) { return Value::Enum(index, Values{std::forward<F>(fields)...}); }
/// The inner value of `Some`, or nothing for `None`.
std::optional<Value> Some(Value const& value);
/// Bern's basic, locale-independent number text, the same on every target.
std::string Number(double value);
winrt::hstring Plain(Value const& value);
double AsDouble(Value const& value);
std::string AsText(Value const& value);
Values const& AsList(Value const& value);
std::optional<uint32_t> Tag(Value const& value);
Value Payload(Value const& value, int field);
std::string KeyText(Value const& value);
}

// ---------------------------------------------------------------------------
// Reactive cells, effects and scopes on the UI thread. An effect runs now
// and again whenever a cell it read changes; scopes own effects, cleanups
// and child scopes, and disposing a scope ends them all. Effects created
// while an effect runs belong to that run and end before the next one.

struct Effect;

struct Source {
    virtual ~Source() = default;
    virtual void Forget(Effect* effect) = 0;
};

struct Scope {
    std::vector<std::function<void()>> cleanups;
    bool disposed = false;

    void OnDispose(std::function<void()> cleanup);
    void Dispose();
    template <class F> void Run(F&& body);
    ~Scope() { Dispose(); }
};

struct Effect : std::enable_shared_from_this<Effect> {
    std::function<void()> body;
    std::vector<std::weak_ptr<Source>> sources;
    std::unique_ptr<Scope> inner;
    bool disposed = false;
    bool queued = false;

    void Schedule();
    void Run();
    void Dispose();
};

namespace Reactive {
inline Effect* observer = nullptr;
inline Scope* current = nullptr;
inline int depth = 0;
inline std::vector<std::shared_ptr<Effect>> queue;

/// Run `body` now and again whenever a cell it read changes, until the
/// current scope ends.
void Run(std::function<void()> body);
/// Register a cleanup with the current scope.
void OnCleanup(std::function<void()> cleanup);
void Drain();

/// Apply several changes and re-run each affected effect once afterwards.
template <class F> void Batch(F&& body) {
    ++depth;
    try { body(); } catch (...) { --depth; throw; }
    if (--depth == 0) Drain();
}

/// Read cells without recording dependencies.
template <class F> auto Untrack(F&& body) {
    auto saved = observer;
    observer = nullptr;
    struct Restore { Effect* saved; ~Restore() { observer = saved; } } restore{saved};
    return body();
}
}

template <class F> void Scope::Run(F&& body) {
    auto saved = Reactive::current;
    Reactive::current = this;
    struct Restore { Scope* saved; ~Restore() { Reactive::current = saved; } } restore{saved};
    body();
}

/// A value that effects can depend on. Copies share one state.
template <class T> class Cell {
    struct State : Source, std::enable_shared_from_this<State> {
        T value;
        std::vector<std::weak_ptr<Effect>> readers;
        explicit State(T initial) : value(std::move(initial)) {}
        void Forget(Effect* effect) override {
            std::erase_if(readers, [effect](auto const& reader) {
                auto locked = reader.lock();
                return !locked || locked.get() == effect;
            });
        }
    };
    std::shared_ptr<State> state;

public:
    Cell() : state(std::make_shared<State>(T{})) {}
    explicit Cell(T initial) : state(std::make_shared<State>(std::move(initial))) {}

    /// The value, recording a dependency of the running effect.
    T const& Get() const {
        if (auto effect = Reactive::observer) {
            bool known = false;
            for (auto const& reader : state->readers)
                if (auto locked = reader.lock(); locked.get() == effect) { known = true; break; }
            if (!known) {
                state->readers.push_back(effect->weak_from_this());
                effect->sources.push_back(std::weak_ptr<Source>(std::static_pointer_cast<Source>(state)));
            }
        }
        return state->value;
    }

    /// Read without recording a dependency.
    T const& Peek() const { return state->value; }

    /// Change the value; readers re-run when it differs.
    void Set(T next) {
        if (state->value == next) return;
        state->value = std::move(next);
        Changed();
    }

    /// Re-run readers after changing the value in place.
    void Changed() const {
        std::vector<std::shared_ptr<Effect>> current;
        for (auto const& reader : state->readers)
            if (auto effect = reader.lock()) current.push_back(effect);
        state->readers.clear();
        for (auto const& effect : current) effect->Schedule();
    }

    T& Mutate() { return state->value; }
};

/// A counter cell for collections whose items cannot be compared.
class Version {
    Cell<int64_t> cell{0};
public:
    void Track() const { cell.Get(); }
    void Bump() { cell.Set(cell.Peek() + 1); }
};

// ---------------------------------------------------------------------------
// The application description shared by both front ends.

/// A failed command, shown to the user and reported to callers.
struct Failure {
    std::string code;
    std::string message;
};

using Done = std::function<void(std::optional<Failure> const&)>;

/// A catalog command: parameter types and result type.
struct CommandDef {
    std::string name;
    std::vector<BType> params;
    BType result;
    bool isQuery = false;
};

/// Typed drafts of a UI session, one instance per hosting window.
struct SessionDef {
    std::string name;
    std::vector<std::pair<std::string, BType>> fields;
};

/// `language → key → text`.
using Strings = std::unordered_map<std::string, std::unordered_map<std::string, winrt::hstring>>;

/// Everything about the application that does not depend on a XAML
/// namespace.
struct Catalog {
    std::string id;
    std::string bundleId;
    /// Text key of the application name.
    std::string name;
    std::string version;
    uint64_t fingerprint = 0;
    std::vector<std::string> languages;
    std::string defaultLanguage;
    Strings strings;
    /// Protocol, UI, media and session catalog types.
    TypeTable types;
    /// Engine state field types, by field index.
    std::vector<BType> state;
    /// Protocol commands and queries, by catalog index.
    std::vector<CommandDef> commands;
    std::vector<SessionDef> sessions;
    /// Result types of the framework session commands, by command index.
    std::vector<BType> sessionResults;
    std::string primary;
    std::vector<std::string> players;
    std::optional<double> compactBelow;
    std::optional<std::array<uint8_t, 3>> brandSeed;
};

/// The generated application catalog (`app_catalog.cpp`).
Catalog BernCatalog();

// ---------------------------------------------------------------------------
// Localized text and the interface language. The language and its strings
// are cells: switching language re-runs every binding that shows text, in
// one update, while drafts, navigation and windows stay.

namespace Text {
void Install(Catalog const& catalog);
/// The current language tag, recording a dependency.
std::string const& Language();
std::string const& PeekLanguage();
std::vector<std::string> const& Supported();
/// Switch the interface language. False for unsupported tags.
bool SetLanguage(std::string const& tag);
/// The translation of `key` in the current language.
winrt::hstring Tr(std::string_view key);
/// A translation with `{name}` placeholders filled from `args`.
winrt::hstring Fmt(std::string_view key, std::initializer_list<std::pair<std::string_view, winrt::hstring>> args);
/// A language's own name for itself, such as “简体中文” or “English”.
winrt::hstring Autonym(std::string const& tag);
/// Text of a value for runtime data: numbers and texts as Bern formats
/// them, enums by their label.
winrt::hstring Display(Value const& value, char const* enumName);
/// The localized label of an enum variant.
winrt::hstring Label(char const* enumName, uint32_t variant);
}

// ---------------------------------------------------------------------------
// The engine behind the in-process interface. Callbacks arrive on engine
// and channel threads; their bytes are copied and handled on the UI thread
// in arrival order.

namespace Host {
inline Catalog const* catalog = nullptr;
inline BernApp* handle = nullptr;

/// What the front end handles: session commands from external entries.
inline std::function<void(uint64_t, Bytes)> onSessionRequest;
/// Called after engine state, a reply or media state arrived.
inline std::function<void()> onChange;

/// A reply token: the outcome goes to `done`, a failure also to `failed`.
uint64_t Token(Done done, std::function<void(Failure const&)> failed);

/// Run `work` on the UI thread after the current call returns.
void Post(std::function<void()> work);
/// Run `work` on the UI thread once `delay` has passed.
void After(std::chrono::milliseconds delay, std::function<void()> work);
/// Create the UI thread's message window; `bundleId` names it so a second
/// instance can find it. False when another instance owns it: that one is
/// told to show itself and runs `onActivate`.
bool Claim(std::string const& bundleId);
inline std::function<void()> onActivate;

/// Start the engine for this front end. Returns an error message or nothing.
std::optional<std::string> Start(Catalog const& app, std::string const& resources);
void Stop();

/// Engine state field, recording a dependency.
Value const& EngineValue(int field);

/// Copy and release a buffer returned by the runtime.
Bytes Take(BernBuffer buffer);
/// `0, …` or `1, code, message`.
std::optional<Failure> Status(Bytes const& bytes, Reader& reader);

/// Dispatch engine command `command` with its arguments. The outcome goes
/// to `done`; a failure also goes to `failed`, which shows it.
void Dispatch(int command, Values const& args, Done done, std::function<void(Failure const&)> failed);

/// Query subscriptions by id, fed by the engine.
inline std::unordered_map<uint64_t, std::function<void(Bytes const&)>> queries;

inline std::string engineId;
}

// ---------------------------------------------------------------------------
// The media executor: a Media Foundation media engine per player and
// source, reporting what it does as observations, never as commands.

/// The standard media commands declarations use on players, by their index
/// in the media catalog.
enum class MediaOp : uint32_t { Open = 2, Play = 3, Pause = 4, Seek = 5, SetVolume = 6, SetMuted = 7 };

namespace Phase {
constexpr uint32_t Idle = 0, Loading = 1, Ready = 2, Playing = 3, Paused = 4, Buffering = 5, Ended = 6, Failed = 7, Unavailable = 8;
}

/// The merged state of one player, from the hub.
struct PlayerState {
    std::optional<std::string> source;
    uint32_t phase = Phase::Unavailable;
    int64_t positionMs = 0;
    std::optional<int64_t> durationMs;
    bool seekable = false;
    double volume = 1;
    bool muted = false;
    std::optional<std::string> error;

    bool operator==(PlayerState const&) const = default;
    bool Is(uint32_t p) const { return phase == p; }
    bool Playing() const { return Is(Phase::Playing) || Is(Phase::Buffering); }
    bool Playable() const { return source && !Is(Phase::Idle) && !Is(Phase::Failed) && !Is(Phase::Unavailable); }
    bool CanSeek() const { return Playable() && seekable && durationMs; }
    bool Controllable() const { return !Is(Phase::Unavailable); }
    bool ShowsFrames() const { return Is(Phase::Ready) || Is(Phase::Playing) || Is(Phase::Paused) || Is(Phase::Buffering) || Is(Phase::Ended); }
    std::string Describe() const;
};

namespace Media {
void AttachExecutor();
void DetachExecutor();
void ReceiveState(Bytes const& bytes);
void Request(Bytes const& bytes);
/// What a view of a player shows: the swap chain the player's media engine
/// draws into, and the video's size in square pixels. The chain is null
/// while there is no video and for every view but the newest of a player.
struct Frames {
    HANDLE chain = nullptr;
    uint32_t width = 0, height = 0;
};
/// Show `player`'s frames in a new view, which shows them until a newer
/// view does; `changed` runs on the UI thread with what it shows now.
uint64_t Show(std::string const& player, std::function<void(Frames const&)> changed);
void Hide(uint64_t screen);
/// The size in pixels the video shows at in `screen`, which the engine
/// draws at while that view is the newest.
void Resize(uint64_t screen, uint32_t width, uint32_t height);
/// Dispatch a standard media command on `player` with its arguments after
/// the player id.
void Send(MediaOp op, std::string const& player, Values const& args, std::function<void(Failure const&)> failed);
std::optional<Failure> Create(std::string const& player, std::string const& owner);
std::optional<Failure> Bind(std::string const& player, std::optional<std::string> const& url);
void Release(std::string const& player);
std::string AttachView(std::string const& player, std::string const& window);
void DetachView(std::string const& token);
/// The hub's state of `player`, recording a dependency.
std::optional<PlayerState> State(std::string const& player);
std::optional<PlayerState> Peek(std::string const& player);
std::string Time(int64_t milliseconds);
}

// ---------------------------------------------------------------------------
// Development snapshots and the appearance override.

namespace Dev {
/// `BERN_SNAPSHOT_DIR`, when set.
std::optional<std::wstring> SnapshotDirectory();
/// `BERN_APPEARANCE`: "light", "dark" or empty.
std::string Appearance();
/// Encode BGRA pixels as a PNG at `path`, on the theme's base color.
winrt::fire_and_forget SavePng(Bytes pixels, int32_t width, int32_t height, double dpi, std::wstring path, bool dark);
}

}
