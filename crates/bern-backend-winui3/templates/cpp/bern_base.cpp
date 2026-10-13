// Bern runtime for Windows: the shared part, compiled once.
// Generated build input; do not edit.

// Through the precompiled header, which starts with bern_base.h.
#include "bern_pch.h"

#include <audiosessiontypes.h>
#include <d3d11_4.h>
#include <mfapi.h>
#include <mferror.h>
#include <mfmediaengine.h>
#include <shellapi.h>
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

std::vector<double> Numbers(Value const& value) {
    std::vector<double> numbers;
    if (value.kind != Value::Kind::List) return numbers;
    for (auto const& item : value.Items()) numbers.push_back(AsDouble(item));
    return numbers;
}

std::optional<std::string> Url(Value const& value) {
    Value inner = value;
    if (inner.kind == Value::Kind::Optional) {
        if (!inner.IsSome()) return std::nullopt;
        Value some = inner.Items()[0];
        inner = std::move(some);
    }
    if (inner.kind == Value::Kind::Record && !inner.Items().empty() && inner.Items()[0].kind == Value::Kind::Text && !inner.Items()[0].Str().empty())
        return inner.Items()[0].Str();
    return std::nullopt;
}

std::string VariantName(Value const& value, char const* enumName) {
    if (value.kind != Value::Kind::Enum || !Host::catalog) return {};
    auto found = Host::catalog->types.enums.find(enumName);
    if (found == Host::catalog->types.enums.end() || value.variant >= found->second.size()) return {};
    return found->second[value.variant].name;
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
/// `WM_COPYDATA` of a later launch: the files it was opened with.
constexpr ULONG_PTR kOpenFiles = 0x4245524E;

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
    case WM_COPYDATA: {
        auto data = reinterpret_cast<COPYDATASTRUCT const*>(lparam);
        if (!data || data->dwData != kOpenFiles || !data->lpData) return FALSE;
        std::string text(static_cast<char const*>(data->lpData), data->cbData);
        std::vector<std::string> urls;
        for (size_t start = 0; start < text.size();) {
            auto end = text.find('\n', start);
            if (end == std::string::npos) end = text.size();
            if (end > start) urls.push_back(text.substr(start, end - start));
            start = end + 1;
        }
        Post([urls] {
            for (auto const& url : urls) openedFiles.push_back(url);
            if (onOpen) onOpen(urls);
        });
        return TRUE;
    }
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

std::string FileUrl(std::wstring const& path) {
    std::wstring url(4096, L'\0');
    DWORD length = static_cast<DWORD>(url.size());
    if (FAILED(UrlCreateFromPathW(path.c_str(), url.data(), &length, 0))) return {};
    url.resize(length);
    return Utf8(winrt::hstring(url));
}

std::optional<std::wstring> FilePath(std::string const& url) {
    if (url.rfind("file:", 0) != 0) return std::nullopt;
    std::wstring path(MAX_PATH * 4, L'\0');
    DWORD length = static_cast<DWORD>(path.size());
    if (FAILED(PathCreateFromUrlW(std::wstring(Wide(url)).c_str(), path.data(), &length, 0))) return std::nullopt;
    path.resize(length);
    return path;
}

/// The files on this process's command line, as `file:` URLs.
static std::vector<std::string> CommandLineFiles() {
    std::vector<std::string> urls;
    int count = 0;
    LPWSTR* arguments = CommandLineToArgvW(GetCommandLineW(), &count);
    if (!arguments) return urls;
    for (int i = 1; i < count; ++i) {
        std::wstring argument = arguments[i];
        if (argument.empty() || argument[0] == L'-' || argument[0] == L'/') continue;
        wchar_t full[MAX_PATH * 4];
        DWORD length = GetFullPathNameW(argument.c_str(), MAX_PATH * 4, full, nullptr);
        if (length == 0 || length >= MAX_PATH * 4) continue;
        auto attributes = GetFileAttributesW(full);
        if (attributes == INVALID_FILE_ATTRIBUTES || (attributes & FILE_ATTRIBUTE_DIRECTORY)) continue;
        if (auto url = FileUrl(full); !url.empty()) urls.push_back(url);
    }
    LocalFree(arguments);
    return urls;
}

bool Claim(std::string const& bundleId) {
    openedFiles = CommandLineFiles();
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
                if (!openedFiles.empty()) {
                    std::string text;
                    for (auto const& url : openedFiles) text += url + "\n";
                    COPYDATASTRUCT data{kOpenFiles, static_cast<DWORD>(text.size()), text.data()};
                    SendMessageTimeoutW(other, WM_COPYDATA, 0, reinterpret_cast<LPARAM>(&data), SMTO_ABORTIFHUNG, 2000, nullptr);
                }
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

#pragma comment(lib, "mfplat.lib")
#pragma comment(lib, "mfuuid.lib")
#pragma comment(lib, "oleaut32.lib")
#pragma comment(lib, "shell32.lib")
#pragma comment(lib, "shlwapi.lib")

std::string PlayerState::Describe() const {
    static char const* const names[] = {"Idle", "Loading", "Ready", "Playing", "Paused", "Buffering", "Ended", "Failed", "Unavailable"};
    std::string name = phase < 9 ? names[phase] : "Unavailable";
    return name + " " + Media::Time(positionMs) + " / " + (durationMs ? Media::Time(*durationMs) : std::string("--:--"));
}

namespace Media {

static bool Started() {
    static bool const started = SUCCEEDED(MFStartup(MF_VERSION));
    return started;
}

/// The Direct3D device the media engines decode and draw with, behind one
/// DXGI device manager; null without Direct3D 11 video support, and the
/// engines then play sound only.
static IMFDXGIDeviceManager* DeviceManager() {
    static winrt::com_ptr<ID3D11Device> device;
    static winrt::com_ptr<IMFDXGIDeviceManager> manager;
    static UINT token = 0;
    if (device && device->GetDeviceRemovedReason() == S_OK) return manager.get();
    // A device the GPU lost is replaced; the engines open the new one
    // through the manager.
    winrt::com_ptr<ID3D11Device> made;
    UINT flags = D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    if (FAILED(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, flags, nullptr, 0, D3D11_SDK_VERSION, made.put(), nullptr, nullptr)))
        return nullptr;
    // The engines use it from their own threads.
    if (auto multithread = made.try_as<ID3D11Multithread>()) multithread->SetMultithreadProtected(TRUE);
    if (!manager && FAILED(MFCreateDXGIDeviceManager(&token, manager.put()))) return nullptr;
    if (FAILED(manager->ResetDevice(made.get(), token))) return nullptr;
    device = made;
    return manager.get();
}

struct Stream;

/// Takes an engine's events, which arrive on its threads, to its stream on
/// the UI thread.
struct Notify : winrt::implements<Notify, IMFMediaEngineNotify> {
    std::weak_ptr<Stream> stream;
    uint64_t engine;
    Notify(std::weak_ptr<Stream> stream, uint64_t engine) : stream(std::move(stream)), engine(engine) {}
    HRESULT __stdcall EventNotify(DWORD event, DWORD_PTR param1, DWORD param2) noexcept override;
};

/// A view showing a player, newest last.
struct Screen {
    uint64_t token = 0;
    std::function<void(Frames const&)> changed;
    uint32_t width = 0, height = 0;
};

/// The views of each player, kept apart from its stream, which a release
/// ends while views may still show the player.
static std::unordered_map<std::string, std::vector<Screen>> screens;

/// One player of the executor. Each source gets a media engine of its own,
/// so that late events of the previous one are told apart. The engine draws
/// its frames into a swap chain of its own (windowless swap chain mode),
/// which views show; Windows tone maps HDR video there for SDR displays.
struct Stream : std::enable_shared_from_this<Stream> {
    std::string id;
    winrt::com_ptr<IMFMediaEngineEx> engine;
    /// The engine events belong to.
    uint64_t serial = 0;
    uint64_t version = 0;
    bool loaded = false, started = false, waiting = false, windowless = false, released = false;
    double volume = 1;
    bool muted = false;
    std::optional<std::string> error;
    std::chrono::steady_clock::time_point lastReport{};
    double lastPosition = 0;
    std::vector<std::function<void()>> seeks;
    Frames frames;
    /// The size the engine draws at.
    uint32_t drawnWidth = 0, drawnHeight = 0;

    explicit Stream(std::string name) : id(std::move(name)) {}

    /// A new engine with the player's volume, for a new source.
    void Start() {
        Stop();
        if (!Started()) winrt::throw_hresult(MF_E_PLATFORM_NOT_INITIALIZED);
        winrt::com_ptr<IMFMediaEngineClassFactory> factory;
        winrt::check_hresult(CoCreateInstance(CLSID_MFMediaEngineClassFactory, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(factory.put())));
        winrt::com_ptr<IMFAttributes> attributes;
        winrt::check_hresult(MFCreateAttributes(attributes.put(), 4));
        auto notify = winrt::make_self<Notify>(weak_from_this(), serial);
        winrt::check_hresult(attributes->SetUnknown(MF_MEDIA_ENGINE_CALLBACK, static_cast<IMFMediaEngineNotify*>(notify.get())));
        if (auto manager = DeviceManager()) winrt::check_hresult(attributes->SetUnknown(MF_MEDIA_ENGINE_DXGI_MANAGER, manager));
        else bern::Report("video " + id + ": no Direct3D 11 video device, sound only");
        winrt::check_hresult(attributes->SetUINT32(MF_MEDIA_ENGINE_AUDIO_CATEGORY, AudioCategory_Media));
        winrt::check_hresult(attributes->SetGUID(MF_MEDIA_ENGINE_BROWSER_COMPATIBILITY_MODE, MF_MEDIA_ENGINE_BROWSER_COMPATIBILITY_MODE_IE_EDGE));
        winrt::com_ptr<IMFMediaEngine> made;
        winrt::check_hresult(factory->CreateInstance(0, attributes.get(), made.put()));
        engine = made.as<IMFMediaEngineEx>();
        engine->SetVolume(volume);
        engine->SetMuted(muted);
    }

    /// Shut the engine down; its later events are ignored.
    void Stop() {
        ++serial;
        windowless = false;
        drawnWidth = drawnHeight = 0;
        Present(Frames{});
        if (engine) {
            engine->Shutdown();
            engine = nullptr;
        }
    }

    /// Change what the views show.
    void Present(Frames next) {
        if (next.chain == frames.chain && next.width == frames.width && next.height == frames.height) return;
        frames = next;
        Announce();
    }

    /// Tell each view what it shows: the newest the frames, the others the
    /// size only.
    void Announce() {
        auto found = screens.find(id);
        if (found == screens.end()) return;
        std::vector<std::pair<std::function<void(Frames const&)>, Frames>> calls;
        auto const& list = found->second;
        for (size_t i = 0; i < list.size(); ++i)
            calls.emplace_back(list[i].changed, Frames{i + 1 == list.size() ? frames.chain : nullptr, frames.width, frames.height});
        for (auto& [changed, shown] : calls)
            if (changed) changed(shown);
    }

    /// Draw at the size the newest view shows the video, or at the video's
    /// own size before it has one.
    void Fit() { Fit(frames.width, frames.height); }

    void Fit(uint32_t width, uint32_t height) {
        if (!engine || !windowless) return;
        if (auto found = screens.find(id); found != screens.end() && !found->second.empty() && found->second.back().width && found->second.back().height) {
            width = found->second.back().width;
            height = found->second.back().height;
        }
        if (!width || !height || (width == drawnWidth && height == drawnHeight)) return;
        RECT target{0, 0, static_cast<LONG>(width), static_cast<LONG>(height)};
        if (SUCCEEDED(engine->UpdateVideoStream(nullptr, &target, nullptr))) {
            drawnWidth = width;
            drawnHeight = height;
        }
    }

    /// With the metadata, or a new format: have the engine draw into a
    /// swap chain and give it to the views.
    void ShowVideo() {
        DWORD width = 0, height = 0;
        // The size with the pixel aspect ratio applied.
        if (!engine || !engine->HasVideo() || FAILED(engine->GetNativeVideoSize(&width, &height)) || !width || !height) {
            Present(Frames{});
            return;
        }
        HANDLE chain = nullptr;
        if (!windowless) windowless = SUCCEEDED(engine->EnableWindowlessSwapchainMode(TRUE));
        if (windowless) {
            drawnWidth = drawnHeight = 0;
            Fit(width, height);
            // The engine owns the handle.
            if (FAILED(engine->GetVideoSwapchainHandle(&chain))) chain = nullptr;
        }
        if (!chain) bern::Report("video " + id + ": the media engine gives no swap chain, sound only");
        Present(Frames{chain, static_cast<uint32_t>(width), static_cast<uint32_t>(height)});
    }

    void Handle(DWORD event, DWORD_PTR param1, DWORD param2) {
        if (released) return;
        switch (event) {
        case MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA:
        case MF_MEDIA_ENGINE_EVENT_FORMATCHANGE:
            ShowVideo();
            Report();
            break;
        case MF_MEDIA_ENGINE_EVENT_WAITING:
        case MF_MEDIA_ENGINE_EVENT_BUFFERINGSTARTED:
            waiting = true;
            Report();
            break;
        case MF_MEDIA_ENGINE_EVENT_PLAYING:
        case MF_MEDIA_ENGINE_EVENT_BUFFERINGENDED:
            waiting = false;
            Report();
            break;
        case MF_MEDIA_ENGINE_EVENT_SEEKED: {
            Report();
            auto done = std::move(seeks);
            seeks.clear();
            for (auto& complete : done) complete();
            break;
        }
        case MF_MEDIA_ENGINE_EVENT_ERROR:
            error = Describe(static_cast<MF_MEDIA_ENGINE_ERR>(param1), static_cast<HRESULT>(param2));
            Report();
            break;
        case MF_MEDIA_ENGINE_EVENT_TIMEUPDATE: {
            // Playback reports twice a second; a jump reports at once.
            double position = engine ? engine->GetCurrentTime() : 0;
            if (std::abs(position - lastPosition) > 0.75 || std::chrono::steady_clock::now() - lastReport >= std::chrono::milliseconds(500)) Report();
            break;
        }
        case MF_MEDIA_ENGINE_EVENT_LOADEDDATA:
        case MF_MEDIA_ENGINE_EVENT_CANPLAY:
        case MF_MEDIA_ENGINE_EVENT_CANPLAYTHROUGH:
        case MF_MEDIA_ENGINE_EVENT_DURATIONCHANGE:
        case MF_MEDIA_ENGINE_EVENT_PLAY:
        case MF_MEDIA_ENGINE_EVENT_PAUSE:
        case MF_MEDIA_ENGINE_EVENT_ENDED:
        case MF_MEDIA_ENGINE_EVENT_VOLUMECHANGE:
        case MF_MEDIA_ENGINE_EVENT_STALLED:
        case MF_MEDIA_ENGINE_EVENT_EMPTIED:
            Report();
            break;
        default:
            break;
        }
    }

    /// What went wrong, as a sentence with the system's error code.
    static std::string Describe(MF_MEDIA_ENGINE_ERR code, HRESULT result) {
        std::string text;
        switch (code) {
        case MF_MEDIA_ENGINE_ERR_SRC_NOT_SUPPORTED: text = "The file is damaged or its format is not supported"; break;
        case MF_MEDIA_ENGINE_ERR_DECODE: text = "The media could not be decoded"; break;
        case MF_MEDIA_ENGINE_ERR_NETWORK: text = "The media could not be loaded from the network"; break;
        case MF_MEDIA_ENGINE_ERR_ABORTED: text = "Loading the media was aborted"; break;
        default: text = "The media could not be played"; break;
        }
        if (FAILED(result)) {
            char hex[16];
            std::snprintf(hex, sizeof hex, "0x%08X", static_cast<unsigned>(result));
            text += std::string(" (") + hex + ")";
        }
        return text;
    }

    uint32_t CurrentPhase() const {
        if (!loaded) return Phase::Idle;
        if (error) return Phase::Failed;
        if (!engine) return Phase::Loading;
        auto ready = engine->GetReadyState();
        if (ready < MF_MEDIA_ENGINE_READY_HAVE_METADATA) return started ? Phase::Buffering : Phase::Loading;
        if (engine->IsEnded()) return Phase::Ended;
        if (!engine->IsPaused()) return waiting || ready < MF_MEDIA_ENGINE_READY_HAVE_FUTURE_DATA ? Phase::Buffering : Phase::Playing;
        return started ? Phase::Paused : Phase::Ready;
    }

    void Report() {
        // Events queued before the player was released arrive afterwards.
        if (!Host::handle || released) return;
        lastReport = std::chrono::steady_clock::now();
        double position = engine ? engine->GetCurrentTime() : 0;
        lastPosition = position;
        auto phase = CurrentPhase();
        // Unknown before the metadata, infinite for live streams.
        double duration = engine ? engine->GetDuration() : std::nan("");
        bool known = std::isfinite(duration) && duration > 0;
        bool seekable = false;
        if (known) {
            winrt::com_ptr<IMFMediaTimeRange> ranges;
            seekable = SUCCEEDED(engine->GetSeekable(ranges.put())) && ranges && ranges->GetLength() > 0;
        }
        Writer w;
        w.Text(id);
        w.U64(version);
        w.U32(phase);
        w.I64((std::max)(int64_t{0}, std::llround(position * 1000)));
        if (known) {
            w.U8(1);
            w.I64(std::llround(duration * 1000));
        } else {
            w.U8(0);
        }
        w.U8(seekable ? 1 : 0);
        w.F64(std::round(volume * 1000) / 1000);
        w.U8(muted ? 1 : 0);
        if (phase == Phase::Failed && error) {
            w.U8(1);
            w.Text(*error);
        } else {
            w.U8(0);
        }
        bern_media_event(Host::handle, w.bytes.data(), w.bytes.size());
    }

    void Load(std::string const& url, uint64_t next) {
        version = next;
        started = waiting = false;
        error.reset();
        loaded = true;
        try {
            Start();
            std::wstring location{Wide(url)};
            // Local files go to the engine as paths.
            if (url.rfind("file:", 0) == 0) {
                wchar_t path[MAX_PATH * 2];
                DWORD length = MAX_PATH * 2;
                winrt::check_hresult(PathCreateFromUrlW(location.c_str(), path, &length, 0));
                location = path;
            }
            BSTR source = SysAllocString(location.c_str());
            HRESULT result = engine->SetSource(source);
            SysFreeString(source);
            winrt::check_hresult(result);
        } catch (winrt::hresult_error const& failure) {
            auto message = Utf8(failure.message());
            error = message.substr(0, message.find('\n'));
        }
        Report();
    }

    void Unload(uint64_t next) {
        version = next;
        started = waiting = loaded = false;
        error.reset();
        Stop();
        Report();
    }

    void Play() {
        started = true;
        if (engine) {
            if (engine->IsEnded()) engine->SetCurrentTime(0);
            engine->Play();
        }
        Report();
    }

    void Pause() {
        if (engine) engine->Pause();
        Report();
    }

    void SetVolume(double next) {
        volume = next;
        if (engine) engine->SetVolume(next);
        Report();
    }

    void SetMuted(bool next) {
        muted = next;
        if (engine) engine->SetMuted(next);
        Report();
    }

    void Seek(int64_t positionMs, std::function<void()> done) {
        if (!engine) {
            done();
            return;
        }
        auto shared = std::make_shared<std::function<void()>>(std::move(done));
        seeks.push_back([shared] { if (*shared) { auto f = std::move(*shared); *shared = nullptr; f(); } });
        engine->SetCurrentTime(static_cast<double>((std::max)(int64_t{0}, positionMs)) / 1000);
        // A seek that reports no completion still completes.
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
        Stop();
    }
};

HRESULT __stdcall Notify::EventNotify(DWORD event, DWORD_PTR param1, DWORD param2) noexcept {
    // The engine waits for this one before it settles.
    if (event == MF_MEDIA_ENGINE_EVENT_NOTIFYSTABLESTATE) {
        SetEvent(reinterpret_cast<HANDLE>(param1));
        return S_OK;
    }
    try {
        Host::Post([stream = stream, engine = engine, event, param1, param2] {
            if (auto self = stream.lock(); self && self->serial == engine) self->Handle(event, param1, param2);
        });
    } catch (...) {
    }
    return S_OK;
}

static std::unordered_map<std::string, std::shared_ptr<Stream>> streams;
static Cell<std::shared_ptr<std::map<std::string, PlayerState> const>> players{std::make_shared<std::map<std::string, PlayerState> const>()};
static uint64_t revision = 0;
static uint64_t views = 0;

static std::shared_ptr<Stream> StreamOf(std::string const& id) {
    auto& slot = streams[id];
    if (!slot) slot = std::make_shared<Stream>(id);
    return slot;
}

static std::unordered_map<uint64_t, std::string> screenPlayers;
static uint64_t nextScreen = 1;

uint64_t Show(std::string const& player, std::function<void(Frames const&)> changed) {
    auto token = nextScreen++;
    screenPlayers[token] = player;
    screens[player].push_back(Screen{token, std::move(changed)});
    auto stream = StreamOf(player);
    stream->Fit();
    stream->Announce();
    return token;
}

void Hide(uint64_t screen) {
    auto found = screenPlayers.find(screen);
    if (found == screenPlayers.end()) return;
    auto player = found->second;
    screenPlayers.erase(found);
    auto& list = screens[player];
    bool newest = !list.empty() && list.back().token == screen;
    std::erase_if(list, [screen](Screen const& s) { return s.token == screen; });
    if (list.empty()) screens.erase(player);
    if (!newest) return;
    // The next newest view shows the frames.
    if (auto stream = streams.find(player); stream != streams.end()) {
        stream->second->Fit();
        stream->second->Announce();
    }
}

void Resize(uint64_t screen, uint32_t width, uint32_t height) {
    auto found = screenPlayers.find(screen);
    if (found == screenPlayers.end()) return;
    if (auto list = screens.find(found->second); list != screens.end())
        for (auto& s : list->second)
            if (s.token == screen) {
                s.width = width;
                s.height = height;
            }
    if (auto stream = streams.find(found->second); stream != streams.end()) stream->second->Fit();
}

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
    w.Text("windows-mediaengine");
    w.Text("1");
    w.U8(1);  // keeps playing without a view
    w.U8(0);  // one view shows a player at a time, the newest
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
        case 5: stream->SetVolume((std::clamp)(reader.F64(), 0.0, 1.0)); Complete(request, std::nullopt); break;
        case 6: stream->SetMuted(reader.U8() != 0); Complete(request, std::nullopt); break;
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
