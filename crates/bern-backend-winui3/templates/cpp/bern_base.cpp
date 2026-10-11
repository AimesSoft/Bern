// Bern runtime for Windows: the shared part, compiled once.
// Generated build input; do not edit.

// Through the precompiled header, which starts with bern_base.h.
#include "bern_pch.h"

#include <shlwapi.h>

#include <winrt/Windows.Globalization.h>
#include <winrt/Windows.Graphics.Imaging.h>
#include <winrt/Windows.Storage.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.UserProfile.h>

namespace bern {

void Report(std::string_view message) {
    std::fprintf(stderr, "bern: %.*s\n", static_cast<int>(message.size()), message.data());
    std::fflush(stderr);
}

static void ReportCurrent(char const* where) {
    try {
        throw;
    } catch (winrt::hresult_error const& error) {
        Report(std::string(where) + ": " + Utf8(error.message()));
    } catch (std::exception const& error) {
        Report(std::string(where) + ": " + error.what());
    } catch (...) {
        Report(std::string(where) + ": unknown error");
    }
}

// ---------------------------------------------------------------------------
// Values

static Values const kNoValues;
static std::string const kNoText;

Values const& Value::Items() const { return items ? *items : kNoValues; }
std::string const& Value::Str() const { return text ? *text : kNoText; }

bool Value::operator==(Value const& other) const {
    if (kind != other.kind) return false;
    switch (kind) {
    case Kind::Unit: return true;
    case Kind::Bool: return b == other.b;
    case Kind::Int: return i == other.i;
    case Kind::Float: return f == other.f;
    case Kind::Text: return text == other.text || Str() == other.Str();
    case Kind::Enum:
        if (variant != other.variant) return false;
        [[fallthrough]];
    default:
        return items == other.items || Items() == other.Items();
    }
}

namespace Codec {

void Encode(Value const& value, BType const& type, TypeTable const& types, Writer& w) {
    switch (type.kind) {
    case BType::Kind::Unit: return;
    case BType::Kind::Bool: w.U8(value.b ? 1 : 0); return;
    case BType::Kind::Int: w.I64(value.kind == Value::Kind::Float ? static_cast<int64_t>(value.f) : value.i); return;
    case BType::Kind::Float: w.F64(value.kind == Value::Kind::Int ? static_cast<double>(value.i) : value.f); return;
    case BType::Kind::Text: w.Text(value.Str()); return;
    case BType::Kind::Enum: {
        w.U32(value.variant);
        auto found = types.enums.find(type.name);
        if (found == types.enums.end() || value.variant >= found->second.size()) return;
        auto const& fields = found->second[value.variant].fields;
        auto const& items = value.Items();
        for (size_t i = 0; i < fields.size() && i < items.size(); ++i) Encode(items[i], fields[i], types, w);
        return;
    }
    case BType::Kind::Optional:
        if (!value.IsSome()) { w.U8(0); return; }
        w.U8(1);
        Encode(value.Items()[0], *type.inner, types, w);
        return;
    case BType::Kind::Record: {
        auto found = types.records.find(type.name);
        if (found == types.records.end()) return;
        auto const& items = value.Items();
        for (size_t i = 0; i < found->second.size() && i < items.size(); ++i) Encode(items[i], found->second[i], types, w);
        return;
    }
    case BType::Kind::List: {
        auto const& items = value.Items();
        w.U32(static_cast<uint32_t>(items.size()));
        for (auto const& item : items) Encode(item, *type.inner, types, w);
        return;
    }
    }
}

Value Decode(BType const& type, TypeTable const& types, Reader& r) {
    switch (type.kind) {
    case BType::Kind::Unit: return Value::Unit();
    case BType::Kind::Bool: {
        auto byte = r.U8();
        if (byte > 1) throw DecodeError("invalid bool byte");
        return Value::Bool(byte == 1);
    }
    case BType::Kind::Int: return Value::Int(r.I64());
    case BType::Kind::Float: return Value::Float(r.F64());
    case BType::Kind::Text: return Value::Text(r.Text());
    case BType::Kind::Enum: {
        auto found = types.enums.find(type.name);
        if (found == types.enums.end()) throw DecodeError("unknown enum `" + type.name + "`");
        auto index = r.U32();
        if (index >= found->second.size()) throw DecodeError("`" + type.name + "` has no such variant");
        Values fields;
        for (auto const& field : found->second[index].fields) fields.push_back(Decode(field, types, r));
        return Value::Enum(index, std::move(fields));
    }
    case BType::Kind::Optional: {
        auto flag = r.U8();
        if (flag == 0) return Value::None();
        if (flag != 1) throw DecodeError("invalid optional flag");
        return Value::Some(Decode(*type.inner, types, r));
    }
    case BType::Kind::Record: {
        auto found = types.records.find(type.name);
        if (found == types.records.end()) throw DecodeError("unknown record `" + type.name + "`");
        Values fields;
        for (auto const& field : found->second) fields.push_back(Decode(field, types, r));
        return Value::Record(std::move(fields));
    }
    case BType::Kind::List: {
        auto count = r.U32();
        Values items;
        items.reserve(count);
        for (uint32_t i = 0; i < count; ++i) items.push_back(Decode(*type.inner, types, r));
        return Value::List(std::move(items));
    }
    }
    throw DecodeError("unknown type");
}

Bytes ToBytes(Value const& value, BType const& type, TypeTable const& types) {
    Writer w;
    Encode(value, type, types, w);
    return std::move(w.bytes);
}

}

namespace R {

Value At(Value const& value, std::initializer_list<int> path) {
    Value current = value;
    for (int index : path) {
        if (current.kind != Value::Kind::Record || index >= static_cast<int>(current.Items().size())) return Value::Unit();
        Value next = current.Items()[index];
        current = std::move(next);
    }
    return current;
}

Value Neg(Value const& value) {
    if (value.kind == Value::Kind::Int) return Value::Int(-value.i);
    if (value.kind == Value::Kind::Float) return Value::Float(-value.f);
    return value;
}

static std::optional<int> Order(Value const& a, Value const& b) {
    auto compare = [](auto x, auto y) { return x < y ? -1 : x > y ? 1 : 0; };
    using K = Value::Kind;
    if (a.kind == K::Int && b.kind == K::Int) return compare(a.i, b.i);
    if (a.kind == K::Float && b.kind == K::Float) return compare(a.f, b.f);
    if (a.kind == K::Int && b.kind == K::Float) return compare(static_cast<double>(a.i), b.f);
    if (a.kind == K::Float && b.kind == K::Int) return compare(a.f, static_cast<double>(b.i));
    if (a.kind == K::Text && b.kind == K::Text) return a.Str().compare(b.Str()) < 0 ? -1 : a.Str() == b.Str() ? 0 : 1;
    return std::nullopt;
}

Value Lt(Value const& a, Value const& b) { auto o = Order(a, b); return Value::Bool(o && *o < 0); }
Value Le(Value const& a, Value const& b) { auto o = Order(a, b); return Value::Bool(o && *o <= 0); }
Value Gt(Value const& a, Value const& b) { auto o = Order(a, b); return Value::Bool(o && *o > 0); }
Value Ge(Value const& a, Value const& b) { auto o = Order(a, b); return Value::Bool(o && *o >= 0); }

std::optional<Value> Some(Value const& value) {
    if (value.IsSome()) return value.Items()[0];
    return std::nullopt;
}

std::string Number(double value) {
    if (value == std::floor(value) && std::abs(value) < 1e15) return std::to_string(static_cast<int64_t>(value));
    char buffer[64];
    auto result = std::to_chars(buffer, buffer + sizeof buffer, value);
    return std::string(buffer, result.ptr);
}

winrt::hstring Plain(Value const& value) {
    switch (value.kind) {
    case Value::Kind::Text: return Wide(value.Str());
    case Value::Kind::Int: return winrt::to_hstring(value.i);
    case Value::Kind::Float: return Wide(Number(value.f));
    case Value::Kind::Bool: return value.b ? L"true" : L"false";
    default: return L"";
    }
}

double AsDouble(Value const& value) {
    if (value.kind == Value::Kind::Float) return value.f;
    if (value.kind == Value::Kind::Int) return static_cast<double>(value.i);
    return 0;
}

std::string AsText(Value const& value) { return value.kind == Value::Kind::Text ? value.Str() : std::string(); }
Values const& AsList(Value const& value) { return value.kind == Value::Kind::List ? value.Items() : kNoValues; }

std::optional<uint32_t> Tag(Value const& value) {
    if (value.kind == Value::Kind::Enum) return value.variant;
    return std::nullopt;
}

Value Payload(Value const& value, int field) {
    if (value.kind == Value::Kind::Enum && field < static_cast<int>(value.Items().size())) return value.Items()[field];
    return Value::Unit();
}

std::string KeyText(Value const& value) {
    switch (value.kind) {
    case Value::Kind::Int: return std::to_string(value.i);
    case Value::Kind::Text: return value.Str();
    case Value::Kind::Float: return Number(value.f);
    case Value::Kind::Bool: return value.b ? "true" : "false";
    case Value::Kind::Enum: return std::to_string(value.variant);
    default: return {};
    }
}

}

// ---------------------------------------------------------------------------
// Reactive

void Scope::OnDispose(std::function<void()> cleanup) {
    if (disposed) cleanup();
    else cleanups.push_back(std::move(cleanup));
}

void Scope::Dispose() {
    if (disposed) return;
    disposed = true;
    auto pending = std::move(cleanups);
    cleanups.clear();
    for (auto it = pending.rbegin(); it != pending.rend(); ++it) {
        try { (*it)(); } catch (...) { ReportCurrent("cleanup"); }
    }
}

void Effect::Schedule() {
    if (disposed) return;
    if (Reactive::depth > 0) {
        if (!queued) {
            queued = true;
            Reactive::queue.push_back(shared_from_this());
        }
    } else {
        Run();
    }
}

static void ClearSources(Effect* effect) {
    auto sources = std::move(effect->sources);
    effect->sources.clear();
    for (auto const& source : sources)
        if (auto locked = source.lock()) locked->Forget(effect);
}

void Effect::Run() {
    queued = false;
    if (disposed) return;
    // The run may end this effect; it stays alive until the run returns.
    auto self = shared_from_this();
    ClearSources(this);
    // Scopes created by the previous run end before the next one.
    auto previous = std::move(inner);
    if (previous) previous->Dispose();
    inner = std::make_unique<Scope>();
    auto scope = inner.get();
    auto saved = Reactive::observer;
    Reactive::observer = this;
    try {
        scope->Run(body);
    } catch (...) {
        ReportCurrent("effect");
    }
    Reactive::observer = saved;
    // Keep the previous scope until here: a cleanup may still be running.
    previous.reset();
}

void Effect::Dispose() {
    if (disposed) return;
    disposed = true;
    ClearSources(this);
    if (inner) inner->Dispose();
}

namespace Reactive {

static std::vector<std::shared_ptr<Effect>> unowned;

void Run(std::function<void()> body) {
    auto effect = std::make_shared<Effect>();
    effect->body = std::move(body);
    if (current) current->OnDispose([effect] { effect->Dispose(); });
    else unowned.push_back(effect);
    effect->Run();
}

void OnCleanup(std::function<void()> cleanup) {
    if (current) current->OnDispose(std::move(cleanup));
}

void Drain() {
    while (!queue.empty()) {
        auto next = std::move(queue);
        queue.clear();
        for (auto const& effect : next) effect->Run();
    }
}

}

// ---------------------------------------------------------------------------
// Text

namespace Text {

using Table = std::unordered_map<std::string, winrt::hstring>;

static Strings const* all = nullptr;
static std::vector<std::string> supported;
static Cell<std::string> language{std::string("en")};
static Cell<Table const*> strings{nullptr};

static Table const* TableFor(std::string const& tag) {
    if (!all) return nullptr;
    auto found = all->find(tag);
    return found == all->end() ? nullptr : &found->second;
}

static std::string Lower(std::string text) {
    std::transform(text.begin(), text.end(), text.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return text;
}

/// The supported language that best matches the user's languages.
static std::string Choose(std::string const& fallback) {
    std::vector<std::string> wanted;
    try {
        for (auto const& tag : winrt::Windows::System::UserProfile::GlobalizationPreferences::Languages())
            wanted.push_back(Utf8(tag));
    } catch (...) {
    }
    wchar_t locale[LOCALE_NAME_MAX_LENGTH];
    if (GetUserDefaultLocaleName(locale, LOCALE_NAME_MAX_LENGTH) > 0) wanted.push_back(winrt::to_string(locale));
    for (auto const& tag : wanted)
        for (auto const& candidate : supported)
            if (Lower(candidate) == Lower(tag)) return candidate;
    for (auto const& tag : wanted) {
        auto lower = Lower(tag);
        auto starts = [&](char const* prefix) { return lower.rfind(prefix, 0) == 0; };
        // zh-CN, zh-SG → zh-Hans; zh-TW, zh-HK → zh-Hant.
        std::string script = starts("zh-tw") || starts("zh-hk") || starts("zh-mo") || starts("zh-hant") ? "zh-hant"
            : starts("zh") ? "zh-hans"
            : lower.substr(0, lower.find('-'));
        for (auto const& candidate : supported)
            if (Lower(candidate) == script) return candidate;
        for (auto const& candidate : supported)
            if (Lower(candidate).substr(0, Lower(candidate).find('-')) == script) return candidate;
    }
    return fallback;
}

void Install(Catalog const& catalog) {
    all = &catalog.strings;
    supported = catalog.languages;
    auto chosen = Choose(catalog.defaultLanguage);
    language.Set(chosen);
    strings.Set(TableFor(chosen));
}

std::string const& Language() { return language.Get(); }
std::string const& PeekLanguage() { return language.Peek(); }
std::vector<std::string> const& Supported() { return supported; }

bool SetLanguage(std::string const& tag) {
    if (std::find(supported.begin(), supported.end(), tag) == supported.end()) return false;
    Reactive::Batch([&] {
        language.Set(tag);
        strings.Set(TableFor(tag));
    });
    return true;
}

winrt::hstring Tr(std::string_view key) {
    if (auto table = strings.Get()) {
        auto found = table->find(std::string(key));
        if (found != table->end()) return found->second;
    }
    return Wide(key);
}

static winrt::hstring Fill(std::wstring_view source, std::initializer_list<std::pair<std::string_view, winrt::hstring>> args) {
    std::wstring output;
    output.reserve(source.size());
    for (size_t i = 0; i < source.size(); ++i) {
        auto c = source[i];
        if (c == L'{' && i + 1 < source.size() && source[i + 1] == L'{') { output.push_back(L'{'); ++i; continue; }
        if (c == L'}' && i + 1 < source.size() && source[i + 1] == L'}') { output.push_back(L'}'); ++i; continue; }
        if (c == L'{') {
            auto end = source.find(L'}', i + 1);
            if (end == std::wstring_view::npos) { output.append(source.substr(i)); break; }
            auto name = winrt::to_string(source.substr(i + 1, end - i - 1));
            for (auto const& [argName, value] : args)
                if (argName == name) { output.append(value); break; }
            i = end;
            continue;
        }
        output.push_back(c);
    }
    return winrt::hstring(output);
}

winrt::hstring Fmt(std::string_view key, std::initializer_list<std::pair<std::string_view, winrt::hstring>> args) {
    return Fill(Tr(key), args);
}

winrt::hstring Autonym(std::string const& tag) {
    if (tag == "zh-Hans") return L"简体中文";
    if (tag == "zh-Hant") return L"繁體中文";
    try {
        auto name = winrt::Windows::Globalization::Language(Wide(tag)).NativeName();
        if (!name.empty()) return name;
    } catch (...) {
    }
    return Wide(tag);
}

winrt::hstring Label(char const* enumName, uint32_t variant) {
    if (!Host::catalog) return L"";
    auto found = Host::catalog->types.enums.find(enumName);
    if (found == Host::catalog->types.enums.end() || variant >= found->second.size()) return L"";
    return Tr(std::string(enumName) + "." + found->second[variant].name);
}

winrt::hstring Display(Value const& value, char const* enumName) {
    if (value.kind == Value::Kind::Enum && enumName) return Label(enumName, value.variant);
    return R::Plain(value);
}

}

// ---------------------------------------------------------------------------
// Host

namespace Host {

constexpr UINT kRun = WM_APP + 1;
constexpr UINT kActivate = WM_APP + 2;

static HWND window = nullptr;
static std::mutex lock;
static std::vector<std::function<void()>> pending;
static std::unordered_map<UINT_PTR, std::function<void()>> timers;
static UINT_PTR nextTimer = 1;
static std::vector<Cell<Value>> engine;
static uint64_t revision = 0;
static uint64_t nextToken = 1;
struct Reply {
    Done done;
    std::function<void(Failure const&)> failed;
};
static std::unordered_map<uint64_t, Reply> replies;

static void Drain() {
    std::vector<std::function<void()>> work;
    {
        std::lock_guard guard(lock);
        work.swap(pending);
    }
    for (auto& item : work) {
        try { item(); } catch (...) { ReportCurrent("posted work"); }
    }
}

static LRESULT CALLBACK WindowProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
    switch (message) {
    case kRun:
        Drain();
        return 0;
    case kActivate:
        Post([] { if (onActivate) onActivate(); });
        return 0;
    case WM_TIMER: {
        KillTimer(hwnd, wparam);
        auto found = timers.find(wparam);
        if (found != timers.end()) {
            auto work = std::move(found->second);
            timers.erase(found);
            try { work(); } catch (...) { ReportCurrent("timer"); }
        }
        return 0;
    }
    }
    return DefWindowProcW(hwnd, message, wparam, lparam);
}

bool Claim(std::string const& bundleId) {
    auto className = L"BernHost." + std::wstring(Wide(bundleId));
    auto mutexName = L"Local\\Bern." + std::wstring(Wide(bundleId));
    CreateMutexW(nullptr, TRUE, mutexName.c_str());
    if (GetLastError() == ERROR_ALREADY_EXISTS) {
        // Another instance runs: let it come to the front and show itself.
        for (int attempt = 0; attempt < 50; ++attempt) {
            if (HWND other = FindWindowExW(HWND_MESSAGE, nullptr, className.c_str(), nullptr)) {
                DWORD process = 0;
                GetWindowThreadProcessId(other, &process);
                AllowSetForegroundWindow(process);
                PostMessageW(other, kActivate, 0, 0);
                break;
            }
            Sleep(100);
        }
        return false;
    }
    WNDCLASSEXW type{sizeof(type)};
    type.lpfnWndProc = WindowProc;
    type.hInstance = GetModuleHandleW(nullptr);
    type.lpszClassName = className.c_str();
    RegisterClassExW(&type);
    window = CreateWindowExW(0, className.c_str(), L"", 0, 0, 0, 0, 0, HWND_MESSAGE, nullptr, type.hInstance, nullptr);
    return true;
}

void Post(std::function<void()> work) {
    {
        std::lock_guard guard(lock);
        pending.push_back(std::move(work));
    }
    if (window) PostMessageW(window, kRun, 0, 0);
}

void After(std::chrono::milliseconds delay, std::function<void()> work) {
    if (!window) return;
    auto id = nextTimer++;
    timers[id] = std::move(work);
    SetTimer(window, id, static_cast<UINT>((std::max)(delay.count(), static_cast<long long>(USER_TIMER_MINIMUM))), nullptr);
}

Bytes Take(BernBuffer buffer) {
    Bytes bytes;
    if (buffer.ptr && buffer.len) bytes.assign(buffer.ptr, buffer.ptr + buffer.len);
    if (buffer.ptr) bern_buffer_free(buffer);
    return bytes;
}

std::optional<Failure> Status(Bytes const& bytes, Reader& reader) {
    if (bytes.empty()) return Failure{"invalid_reply", "empty reply"};
    if (reader.U8() == 0) return std::nullopt;
    Failure failure;
    failure.code = reader.Text();
    failure.message = reader.Text();
    return failure;
}

static Bytes Copy(uint8_t const* bytes, size_t len) {
    return bytes && len ? Bytes(bytes, bytes + len) : Bytes();
}

static void ReceiveState(Bytes const& bytes) {
    try {
        Reader reader(bytes);
        auto next = reader.U64();
        if (next < revision) return;
        revision = next;
        auto count = reader.U32();
        std::vector<std::pair<uint32_t, Value>> changes;
        for (uint32_t i = 0; i < count; ++i) {
            auto field = reader.U32();
            if (field >= catalog->state.size()) return;
            changes.emplace_back(field, Codec::Decode(catalog->state[field], catalog->types, reader));
        }
        Reactive::Batch([&] {
            for (auto& [field, value] : changes) engine[field].Set(std::move(value));
        });
    } catch (...) {
        ReportCurrent("engine state");
    }
    if (onChange) onChange();
}

static void ReceiveReply(uint64_t token, Bytes const& bytes) {
    auto found = replies.find(token);
    if (found == replies.end()) return;
    auto reply = std::move(found->second);
    replies.erase(found);
    std::optional<Failure> failure;
    try {
        Reader reader(bytes);
        auto status = reader.U8();
        reader.U64();
        if (status != 0) failure = Failure{reader.Text(), reader.Text()};
    } catch (...) {
        failure = Failure{"invalid_reply", "malformed reply"};
    }
    if (failure && reply.failed) reply.failed(*failure);
    if (reply.done) reply.done(failure);
    if (onChange) onChange();
}

static void OnState(void*, uint8_t const* bytes, size_t len) {
    Post([data = Copy(bytes, len)] { ReceiveState(data); });
}
static void OnReply(void*, uint64_t token, uint8_t const* bytes, size_t len) {
    Post([token, data = Copy(bytes, len)] { ReceiveReply(token, data); });
}
static void OnSessionRequest(void*, uint64_t request, uint8_t const* bytes, size_t len) {
    Post([request, data = Copy(bytes, len)]() mutable {
        if (onSessionRequest) onSessionRequest(request, std::move(data));
    });
}
static void OnMediaRequest(void*, uint8_t const* bytes, size_t len) {
    Post([data = Copy(bytes, len)] { Media::Request(data); });
}
static void OnMediaState(void*, uint8_t const* bytes, size_t len) {
    Post([data = Copy(bytes, len)] { Media::ReceiveState(data); });
}
static void OnQuery(void*, uint64_t subscription, uint8_t const* bytes, size_t len) {
    Post([subscription, data = Copy(bytes, len)] {
        auto found = queries.find(subscription);
        if (found != queries.end()) {
            auto receive = found->second;
            receive(data);
        }
    });
}

std::optional<std::string> Start(Catalog const& app, std::string const& resources) {
    catalog = &app;
    engine.clear();
    for (size_t i = 0; i < app.state.size(); ++i) engine.emplace_back(Value::Unit());
    BernCallbacks callbacks{nullptr, OnState, OnReply, OnSessionRequest, OnMediaRequest, OnMediaState, OnQuery};
    BernBuffer error{};
    auto app_handle = bern_app_start(app.fingerprint, reinterpret_cast<uint8_t const*>(resources.data()), resources.size(), callbacks, &error);
    if (!app_handle) {
        auto bytes = Take(error);
        return std::string(bytes.begin(), bytes.end());
    }
    handle = app_handle;
    auto id = Take(bern_app_engine_id(handle));
    engineId.assign(id.begin(), id.end());
    ReceiveState(Take(bern_app_snapshot(handle)));
    return std::nullopt;
}

void Stop() {
    auto app = handle;
    handle = nullptr;
    if (app) bern_app_stop(app);
}

Value const& EngineValue(int field) {
    static Value const unit;
    if (field < 0 || field >= static_cast<int>(engine.size())) return unit;
    return engine[field].Get();
}

uint64_t Token(Done done, std::function<void(Failure const&)> failed) {
    auto token = nextToken++;
    replies[token] = Reply{std::move(done), std::move(failed)};
    return token;
}

void Dispatch(int command, Values const& args, Done done, std::function<void(Failure const&)> failed) {
    if (!handle || command < 0 || command >= static_cast<int>(catalog->commands.size())) return;
    auto const& spec = catalog->commands[command];
    Writer writer;
    for (size_t i = 0; i < args.size() && i < spec.params.size(); ++i) Codec::Encode(args[i], spec.params[i], catalog->types, writer);
    auto token = Token(std::move(done), std::move(failed));
    bern_app_dispatch(handle, static_cast<uint32_t>(command), writer.bytes.data(), writer.bytes.size(), token);
}

}

// ---------------------------------------------------------------------------
// Media

std::string PlayerState::Describe() const {
    static char const* const names[] = {"Idle", "Loading", "Ready", "Playing", "Paused", "Buffering", "Ended", "Failed", "Unavailable"};
    std::string name = phase < 9 ? names[phase] : "Unavailable";
    return name + " " + Media::Time(positionMs) + " / " + (durationMs ? Media::Time(*durationMs) : std::string("--:--"));
}

namespace Media {

using namespace winrt::Windows::Media::Playback;
using winrt::Windows::Foundation::TimeSpan;

/// One player of the executor.
struct Stream : std::enable_shared_from_this<Stream> {
    std::string id;
    MediaPlayer player;
    uint64_t version = 0;
    bool loaded = false, started = false, opened = false, ended = false, released = false;
    std::optional<std::string> error;
    std::chrono::steady_clock::time_point lastReport{};
    TimeSpan lastPosition{};
    std::vector<std::function<void()>> seeks;

    explicit Stream(std::string name) : id(std::move(name)) {
        player.AutoPlay(false);
        // Commands come through the hub; the system media controls would
        // bypass it.
        player.CommandManager().IsEnabled(false);
    }

    void Watch() {
        std::weak_ptr<Stream> weak = shared_from_this();
        auto on = [weak](auto action) {
            return [weak, action](auto&&...) {
                Host::Post([weak, action] {
                    if (auto self = weak.lock()) action(*self);
                });
            };
        };
        player.MediaOpened(on([](Stream& s) { s.opened = true; s.Report(); }));
        player.MediaFailed([weak](auto&&, MediaPlayerFailedEventArgs const& args) {
            auto text = Describe(args);
            Host::Post([weak, text] {
                if (auto self = weak.lock()) { self->error = text; self->Report(); }
            });
        });
        player.MediaEnded(on([](Stream& s) { s.ended = true; s.Report(); }));
        player.VolumeChanged(on([](Stream& s) { s.Report(); }));
        player.IsMutedChanged(on([](Stream& s) { s.Report(); }));
        auto session = player.PlaybackSession();
        session.PlaybackStateChanged(on([](Stream& s) { s.Report(); }));
        session.NaturalDurationChanged(on([](Stream& s) { s.Report(); }));
        session.SeekCompleted(on([](Stream& s) {
            s.Report();
            auto done = std::move(s.seeks);
            s.seeks.clear();
            for (auto& complete : done) complete();
        }));
        session.PositionChanged(on([](Stream& s) {
            if (s.released) return;
            // Playback reports twice a second; a jump reports at once.
            auto position = s.player.PlaybackSession().Position();
            auto jumped = std::abs(std::chrono::duration_cast<std::chrono::milliseconds>(position - s.lastPosition).count()) > 750;
            if (jumped || std::chrono::steady_clock::now() - s.lastReport >= std::chrono::milliseconds(500)) s.Report();
        }));
    }

    /// What went wrong, as a sentence with the system's error code.
    static std::string Describe(MediaPlayerFailedEventArgs const& args) {
        if (!args.ErrorMessage().empty()) return Utf8(args.ErrorMessage());
        std::string text;
        switch (args.Error()) {
        case MediaPlayerError::SourceNotSupported: text = "The file is damaged or its format is not supported"; break;
        case MediaPlayerError::DecodingError: text = "The media could not be decoded"; break;
        case MediaPlayerError::NetworkError: text = "The media could not be loaded from the network"; break;
        case MediaPlayerError::Aborted: text = "Loading the media was aborted"; break;
        default: text = "The media could not be played"; break;
        }
        if (auto code = args.ExtendedErrorCode().value) {
            char hex[16];
            std::snprintf(hex, sizeof hex, "0x%08X", static_cast<unsigned>(code));
            text += std::string(" (") + hex + ")";
        }
        return text;
    }

    uint32_t CurrentPhase() const {
        if (!loaded) return Phase::Idle;
        if (error) return Phase::Failed;
        auto state = player.PlaybackSession().PlaybackState();
        if (!opened) return started ? Phase::Buffering : Phase::Loading;
        if (ended) return Phase::Ended;
        switch (state) {
        case MediaPlaybackState::Playing: return Phase::Playing;
        case MediaPlaybackState::Buffering:
        case MediaPlaybackState::Opening: return started ? Phase::Buffering : Phase::Loading;
        default: return started ? Phase::Paused : Phase::Ready;
        }
    }

    void Report() {
        // Events queued before the player was released arrive afterwards.
        if (!Host::handle || released) return;
        auto session = player.PlaybackSession();
        lastReport = std::chrono::steady_clock::now();
        lastPosition = session.Position();
        auto phase = CurrentPhase();
        auto duration = session.NaturalDuration();
        bool known = opened && duration.count() > 0;
        Writer w;
        w.Text(id);
        w.U64(version);
        w.U32(phase);
        w.I64((std::max)(int64_t{0}, std::chrono::duration_cast<std::chrono::milliseconds>(session.Position()).count()));
        if (known) {
            w.U8(1);
            w.I64(std::chrono::duration_cast<std::chrono::milliseconds>(duration).count());
        } else {
            w.U8(0);
        }
        w.U8(known && session.CanSeek() ? 1 : 0);
        w.F64(std::round(player.Volume() * 1000) / 1000);
        w.U8(player.IsMuted() ? 1 : 0);
        if (phase == Phase::Failed && error) {
            w.U8(1);
            w.Text(*error);
        } else {
            w.U8(0);
        }
        bern_media_event(Host::handle, w.bytes.data(), w.bytes.size());
    }

    void Load(std::string url, uint64_t next) {
        version = next;
        started = opened = ended = false;
        error.reset();
        loaded = true;
        player.Pause();
        player.Source(nullptr);
        Report();
        LoadSource(std::move(url), next);
    }

    winrt::fire_and_forget LoadSource(std::string url, uint64_t next) {
        std::weak_ptr<Stream> weak = shared_from_this();
        try {
            winrt::Windows::Media::Core::MediaSource source{nullptr};
            if (url.rfind("file:", 0) == 0) {
                wchar_t path[MAX_PATH * 2];
                DWORD length = MAX_PATH * 2;
                winrt::check_hresult(PathCreateFromUrlW(Wide(url).c_str(), path, &length, 0));
                auto file = co_await winrt::Windows::Storage::StorageFile::GetFileFromPathAsync(path);
                source = winrt::Windows::Media::Core::MediaSource::CreateFromStorageFile(file);
            } else {
                source = winrt::Windows::Media::Core::MediaSource::CreateFromUri(winrt::Windows::Foundation::Uri(Wide(url)));
            }
            Host::Post([weak, next, source] {
                auto self = weak.lock();
                if (!self || self->version != next || self->released) return;
                self->player.Source(source);
            });
        } catch (winrt::hresult_error const& failure) {
            auto message = Utf8(failure.message());
            Host::Post([weak, next, message] {
                auto self = weak.lock();
                if (!self || self->version != next) return;
                self->error = message.substr(0, message.find('\n'));
                self->Report();
            });
        }
    }

    void Unload(uint64_t next) {
        version = next;
        started = opened = ended = loaded = false;
        error.reset();
        player.Pause();
        player.Source(nullptr);
        Report();
    }

    void Play() {
        if (ended) {
            ended = false;
            player.PlaybackSession().Position(TimeSpan{});
        }
        started = true;
        player.Play();
        Report();
    }

    void Pause() {
        player.Pause();
        Report();
    }

    void Seek(int64_t positionMs, std::function<void()> done) {
        ended = false;
        auto shared = std::make_shared<std::function<void()>>(std::move(done));
        seeks.push_back([shared] { if (*shared) { auto f = std::move(*shared); *shared = nullptr; f(); } });
        player.PlaybackSession().Position(std::chrono::milliseconds((std::max)(int64_t{0}, positionMs)));
        // A seek the session finishes at once reports no completion.
        std::weak_ptr<Stream> weak = shared_from_this();
        Host::After(std::chrono::seconds(3), [weak, shared] {
            if (!*shared) return;
            if (auto self = weak.lock()) self->Report();
            auto f = std::move(*shared);
            *shared = nullptr;
            f();
        });
    }

    void Release() {
        if (released) return;
        released = true;
        seeks.clear();
        player.Pause();
        player.Source(nullptr);
        player.Close();
    }
};

static std::unordered_map<std::string, std::shared_ptr<Stream>> streams;
static Cell<std::shared_ptr<std::map<std::string, PlayerState> const>> players{std::make_shared<std::map<std::string, PlayerState> const>()};
static uint64_t revision = 0;
static uint64_t views = 0;

static std::shared_ptr<Stream> StreamOf(std::string const& id) {
    auto& slot = streams[id];
    if (!slot) {
        slot = std::make_shared<Stream>(id);
        slot->Watch();
    }
    return slot;
}

MediaPlayer Player(std::string const& id) { return StreamOf(id)->player; }

std::string Time(int64_t milliseconds) {
    auto seconds = (std::max)(int64_t{0}, milliseconds) / 1000;
    char text[32];
    if (seconds >= 3600)
        std::snprintf(text, sizeof text, "%lld:%02lld:%02lld", seconds / 3600, seconds % 3600 / 60, seconds % 60);
    else
        std::snprintf(text, sizeof text, "%lld:%02lld", seconds / 60, seconds % 60);
    return text;
}

static Bytes Call(BernBuffer (*function)(BernApp*, uint8_t const*, size_t), Bytes const& bytes) {
    return Host::Take(function(Host::handle, bytes.data(), bytes.size()));
}

static std::optional<Failure> StatusOf(Bytes const& reply) {
    Reader reader(reply);
    return Host::Status(reply, reader);
}

void AttachExecutor() {
    Writer w;
    w.Text("windows-mediaplayer");
    w.Text("1");
    w.U8(1);  // keeps playing without a view
    w.U8(1);  // several views may show one player
    if (auto failure = StatusOf(Call(bern_media_executor_attached, w.bytes))) Report("media executor: " + failure->message);
    ReceiveState(Host::Take(bern_media_snapshot(Host::handle)));
}

void DetachExecutor() {
    if (Host::handle) bern_media_executor_detached(Host::handle);
    for (auto& [id, stream] : streams) stream->Release();
    streams.clear();
}

static std::optional<std::pair<std::string, PlayerState>> FromValue(Value const& value) {
    if (value.kind != Value::Kind::Record) return std::nullopt;
    auto const& fields = value.Items();
    auto field = [&](size_t index) -> Value const& {
        static Value const unit;
        return index < fields.size() ? fields[index] : unit;
    };
    PlayerState state;
    if (auto source = R::Some(field(3)); source && source->kind == Value::Kind::Record && !source->Items().empty())
        state.source = source->Items()[0].Str();
    state.phase = field(5).kind == Value::Kind::Enum ? field(5).variant : Phase::Unavailable;
    state.positionMs = field(6).i;
    if (auto duration = R::Some(field(7)); duration && duration->kind == Value::Kind::Int) state.durationMs = duration->i;
    state.seekable = R::Truthy(field(8));
    state.volume = field(9).kind == Value::Kind::Float || field(9).kind == Value::Kind::Int ? R::AsDouble(field(9)) : 1;
    state.muted = R::Truthy(field(10));
    if (auto error = R::Some(field(11)); error && error->kind == Value::Kind::Text) state.error = error->Str();
    return std::make_pair(field(0).Str(), state);
}

void ReceiveState(Bytes const& bytes) {
    try {
        Reader reader(bytes);
        auto next = reader.U64();
        auto count = reader.U32();
        if (next < revision) return;
        revision = next;
        auto states = std::make_shared<std::map<std::string, PlayerState>>();
        auto type = BType::Record("MediaPlayer");
        for (uint32_t i = 0; i < count; ++i)
            if (auto entry = FromValue(Codec::Decode(type, Host::catalog->types, reader))) (*states)[entry->first] = entry->second;
        players.Set(states);
    } catch (...) {
        ReportCurrent("media state");
    }
    if (Host::onChange) Host::onChange();
}

static void Complete(uint64_t request, std::optional<Failure> const& failure) {
    Writer w;
    w.U64(request);
    if (!failure) {
        w.U8(0);
    } else {
        w.U8(1);
        w.Text(failure->code);
        w.Text(failure->message);
    }
    if (Host::handle) bern_media_complete(Host::handle, w.bytes.data(), w.bytes.size());
}

void Request(Bytes const& bytes) {
    uint64_t request = 0, sourceVersion = 0;
    uint32_t op = 0;
    std::string player;
    Reader reader(bytes);
    try {
        request = reader.U64();
        player = reader.Text();
        sourceVersion = reader.U64();
        op = reader.U32();
        switch (op) {
        case 0:
            StreamOf(player)->Load(reader.Text(), sourceVersion);
            Complete(request, std::nullopt);
            return;
        case 1:
            StreamOf(player)->Unload(sourceVersion);
            Complete(request, std::nullopt);
            return;
        case 7:
            if (auto found = streams.find(player); found != streams.end()) {
                found->second->Release();
                streams.erase(found);
            }
            Complete(request, std::nullopt);
            return;
        }
        auto stream = StreamOf(player);
        if (stream->version != sourceVersion) {
            Complete(request, Failure{"stale_source", "a newer source is loaded"});
            return;
        }
        switch (op) {
        case 2: stream->Play(); Complete(request, std::nullopt); break;
        case 3: stream->Pause(); Complete(request, std::nullopt); break;
        case 4: stream->Seek(reader.I64(), [request] { Complete(request, std::nullopt); }); break;
        case 5: stream->player.Volume((std::clamp)(reader.F64(), 0.0, 1.0)); Complete(request, std::nullopt); break;
        case 6: stream->player.IsMuted(reader.U8() != 0); Complete(request, std::nullopt); break;
        default: Complete(request, Failure{"unknown_command", "unknown media operation"}); break;
        }
    } catch (DecodeError const&) {
        return;
    } catch (...) {
        ReportCurrent("media request");
        Complete(request, Failure{"media_failed", "the media backend failed"});
    }
}

static std::vector<BType> const& Parameters(MediaOp op) {
    static std::vector<std::vector<BType>> const table = {
        {},
        {BType::Text()},
        {BType::Text(), BType::Record("MediaSource")},
        {BType::Text()},
        {BType::Text()},
        {BType::Text(), BType::Int()},
        {BType::Text(), BType::Float()},
        {BType::Text(), BType::Bool()},
    };
    return table[static_cast<size_t>(op)];
}

void Send(MediaOp op, std::string const& player, Values const& args, std::function<void(Failure const&)> failed) {
    if (!Host::handle) return;
    auto const& types = Parameters(op);
    Values values{Value::Text(player)};
    values.insert(values.end(), args.begin(), args.end());
    Writer w;
    for (size_t i = 0; i < values.size() && i < types.size(); ++i) Codec::Encode(values[i], types[i], Host::catalog->types, w);
    auto token = Host::Token(nullptr, std::move(failed));
    bern_media_dispatch(Host::handle, static_cast<uint32_t>(op), w.bytes.data(), w.bytes.size(), token);
}

std::optional<Failure> Create(std::string const& player, std::string const& owner) {
    Writer w;
    w.Text(player);
    w.Text(owner);
    return StatusOf(Call(bern_media_create, w.bytes));
}

std::optional<Failure> Bind(std::string const& player, std::optional<std::string> const& url) {
    Writer w;
    w.Text(player);
    if (!url) {
        w.U8(0);
    } else {
        w.U8(1);
        w.Text(*url);
    }
    return StatusOf(Call(bern_media_bind, w.bytes));
}

void Release(std::string const& player) {
    if (!Host::handle) return;
    Writer w;
    w.Text(player);
    bern_media_release(Host::handle, w.bytes.data(), w.bytes.size());
}

std::string AttachView(std::string const& player, std::string const& window) {
    auto token = "view-" + std::to_string(++views);
    if (Host::handle) {
        Writer w;
        w.Text(token);
        w.U8(1);
        w.Text(player);
        w.Text(window);
        bern_media_view(Host::handle, w.bytes.data(), w.bytes.size());
    }
    return token;
}

void DetachView(std::string const& token) {
    if (!Host::handle) return;
    Writer w;
    w.Text(token);
    w.U8(0);
    bern_media_view(Host::handle, w.bytes.data(), w.bytes.size());
}

std::optional<PlayerState> State(std::string const& player) {
    auto const& map = *players.Get();
    auto found = map.find(player);
    if (found == map.end()) return std::nullopt;
    return found->second;
}

std::optional<PlayerState> Peek(std::string const& player) {
    auto const& map = *players.Peek();
    auto found = map.find(player);
    if (found == map.end()) return std::nullopt;
    return found->second;
}

}

// ---------------------------------------------------------------------------
// Development aids

namespace Dev {

std::optional<std::wstring> SnapshotDirectory() {
    wchar_t buffer[MAX_PATH * 2];
    auto length = GetEnvironmentVariableW(L"BERN_SNAPSHOT_DIR", buffer, MAX_PATH * 2);
    if (length == 0 || length >= MAX_PATH * 2) return std::nullopt;
    return std::wstring(buffer, length);
}

std::string Appearance() {
    wchar_t buffer[16];
    auto length = GetEnvironmentVariableW(L"BERN_APPEARANCE", buffer, 16);
    if (length == 0 || length >= 16) return {};
    return winrt::to_string(std::wstring_view(buffer, length));
}

winrt::fire_and_forget SavePng(Bytes pixels, int32_t width, int32_t height, double dpi, std::wstring path, bool dark) {
    using namespace winrt::Windows::Graphics::Imaging;
    using namespace winrt::Windows::Storage::Streams;
    try {
        // The backdrop is not part of the rendering: put the content on the
        // theme's base color, as the window shows it.
        uint8_t backdrop = dark ? 0x20 : 0xF3;
        for (size_t i = 0; i + 3 < pixels.size(); i += 4) {
            int rest = 255 - pixels[i + 3];
            for (int c = 0; c < 3; ++c) pixels[i + c] = static_cast<uint8_t>((std::min)(255, pixels[i + c] + backdrop * rest / 255));
            pixels[i + 3] = 255;
        }
        InMemoryRandomAccessStream stream;
        auto encoder = co_await BitmapEncoder::CreateAsync(BitmapEncoder::PngEncoderId(), stream);
        encoder.SetPixelData(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Premultiplied, width, height, dpi, dpi, pixels);
        co_await encoder.FlushAsync();
        stream.Seek(0);
        Buffer buffer(static_cast<uint32_t>(stream.Size()));
        co_await stream.ReadAsync(buffer, buffer.Capacity(), InputStreamOptions::None);
        FILE* file = nullptr;
        if (_wfopen_s(&file, path.c_str(), L"wb") == 0 && file) {
            std::fwrite(buffer.data(), 1, buffer.Length(), file);
            std::fclose(file);
        }
    } catch (...) {
        ReportCurrent("snapshot");
    }
}

}

}
