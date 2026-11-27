// How pictures are asked for over the network: what one try at an address
// gives, and when to try again. It knows nothing of the interface, so that a
// test runs it against a server that fails on purpose.
//
// An address the network could not reach, or that stays silent, is tried
// again, each wait longer than the one before, for as long as something
// waits for its picture. After the last wait it rests until the network
// changes or a window comes to the front; a view that appears again starts
// over at once. An address the server answered without a picture is not
// asked again on the same network.
#pragma once

#include <shlobj.h>
#include <knownfolders.h>
#include <objbase.h>

#include <algorithm>
#include <atomic>
#include <cctype>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <cwchar>
#include <deque>
#include <filesystem>
#include <functional>
#include <list>
#include <memory>
#include <mutex>
#include <optional>
#include <random>
#include <set>
#include <string>
#include <string_view>
#include <thread>
#include <unordered_map>
#include <vector>

#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Networking.Connectivity.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.Threading.h>
#include <winrt/Windows.Web.Http.h>
#include <winrt/Windows.Web.Http.Filters.h>
#include <winrt/Windows.Web.Http.Headers.h>

#pragma comment(lib, "shell32.lib")
#pragma comment(lib, "ole32.lib")

/// Seconds before each further try, give or take a quarter, so that pictures
/// that failed together do not all return together.
#ifndef BERN_PICTURE_WAITS
#define BERN_PICTURE_WAITS 1, 2, 4, 8, 16, 32
#endif
/// Seconds a try may hear nothing before it counts as failed. A stalled
/// connection reports no error.
#ifndef BERN_PICTURE_SILENCE
#define BERN_PICTURE_SILENCE 15
#endif

namespace bern::pictures {

namespace foundation = winrt::Windows::Foundation;
namespace http = winrt::Windows::Web::Http;
using Bytes = winrt::Windows::Storage::Streams::IBuffer;

inline constexpr double kWaits[] = {BERN_PICTURE_WAITS};
inline constexpr std::chrono::seconds kSilence{BERN_PICTURE_SILENCE};
inline constexpr size_t kMaxConcurrentRequests = 8;
inline constexpr size_t kSilentRotateThreshold = 3;
inline constexpr size_t kMaxMemoryCacheBytes = 64u << 20;
inline constexpr size_t kMaxCacheFileBytes = 16u << 20;
inline constexpr uint64_t kMaxDiskCacheBytes = 256ull << 20;
inline constexpr uint64_t kDiskEvictTargetBytes = 192ull << 20;
inline constexpr uint64_t kDiskEvictInterval = 16;

/// What one try at an address gave.
enum class Reading {
    Picture,
    /// The network failed, or the server could not answer now.
    Unreachable,
    /// Nothing came for `kSilence`.
    Silent,
    /// The server answered, and not with a picture.
    Refused,
};

namespace detail {

struct GateSignal {
    winrt::handle event{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
    std::atomic<bool> granted{false};
    bool cancelled = false;
};

struct CallerWait {
    winrt::handle event{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
};

struct Inflight;

}  // namespace detail

/// Who waits for a picture. Only one thread at a time runs what waits, but
/// a view and the network speak to it from others.
class Waiter {
public:
    Waiter() : wake_(CreateEventW(nullptr, FALSE, FALSE, nullptr)) {}
    void* Handle() const { return wake_.get(); }
    bool Ended() const { return ended_; }
    bool Shown() const { return shown_; }
    uint64_t Appearances() const { return appearances_; }
    /// Its next try goes now.
    void Wake() {
        SetEvent(wake_.get());
        NotifyInflight(true, false);
    }
    /// Nothing waits any longer.
    void End() {
        ended_ = true;
        SetEvent(wake_.get());
        NotifyAll();
    }
    /// The view left: no tries until it comes back.
    void Left() {
        shown_ = false;
        NotifyGate();
        NotifyInflight(false, true);
    }
    /// The view is back: it starts over.
    void Appeared() {
        if (!shown_.exchange(true)) {
            ++appearances_;
            SetEvent(wake_.get());
            NotifyInflight(false, true);
        }
    }

    void AttachGate(std::weak_ptr<detail::GateSignal> sig) {
        std::lock_guard held(hook_lock_);
        gate_signals_.push_back(std::move(sig));
    }

    void DetachGate(detail::GateSignal const* target) {
        std::lock_guard held(hook_lock_);
        std::erase_if(gate_signals_, [target](auto const& weak) {
            auto locked = weak.lock();
            return !locked || locked.get() == target;
        });
    }

    void AttachInflight(std::weak_ptr<detail::Inflight> task, std::weak_ptr<detail::CallerWait> caller) {
        bool already_ended = false;
        {
            std::lock_guard held(hook_lock_);
            inflight_hooks_.push_back({std::move(task), std::move(caller)});
            already_ended = ended_.load();
        }
        if (already_ended) {
            if (auto cw = caller.lock()) SetEvent(cw->event.get());
        }
    }

    void DetachCaller(detail::CallerWait const* target) {
        std::lock_guard held(hook_lock_);
        std::erase_if(inflight_hooks_, [target](auto const& hook) {
            auto cw = hook.caller.lock();
            return !cw || cw.get() == target;
        });
    }

private:
    struct InflightHook {
        std::weak_ptr<detail::Inflight> task;
        std::weak_ptr<detail::CallerWait> caller;
    };

    void NotifyGate() {
        std::vector<std::shared_ptr<detail::GateSignal>> signals;
        {
            std::lock_guard held(hook_lock_);
            std::erase_if(gate_signals_, [](auto const& weak) { return weak.expired(); });
            for (auto const& weak : gate_signals_) {
                if (auto sig = weak.lock()) signals.push_back(std::move(sig));
            }
        }
        for (auto const& sig : signals) SetEvent(sig->event.get());
    }

    void NotifyInflight(bool wake_only, bool sync_driver);

    void NotifyAll() {
        std::vector<std::shared_ptr<detail::GateSignal>> signals;
        std::vector<std::shared_ptr<detail::CallerWait>> callers;
        std::vector<std::shared_ptr<detail::Inflight>> tasks;
        {
            std::lock_guard held(hook_lock_);
            for (auto const& weak : gate_signals_) {
                if (auto sig = weak.lock()) signals.push_back(std::move(sig));
            }
            gate_signals_.clear();
            for (auto const& hook : inflight_hooks_) {
                if (auto cw = hook.caller.lock()) callers.push_back(std::move(cw));
                if (auto t = hook.task.lock()) tasks.push_back(std::move(t));
            }
            inflight_hooks_.clear();
        }
        for (auto const& sig : signals) SetEvent(sig->event.get());
        for (auto const& cw : callers) SetEvent(cw->event.get());
        SyncTasks(tasks, false);
    }

    static void SyncTasks(std::vector<std::shared_ptr<detail::Inflight>> const& tasks, bool wake_only);

    winrt::handle wake_;
    std::atomic<bool> ended_{false};
    std::atomic<bool> shown_{true};
    std::atomic<uint64_t> appearances_{0};
    std::mutex hook_lock_;
    std::vector<std::weak_ptr<detail::GateSignal>> gate_signals_;
    std::vector<InflightHook> inflight_hooks_;
};

namespace detail {

inline std::wstring ToWide(std::string_view text) {
    if (text.empty()) return {};
    int needed = MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), nullptr, 0);
    if (needed <= 0) return {};
    std::wstring out(static_cast<size_t>(needed), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), out.data(), needed);
    return out;
}

inline Bytes ToBuffer(std::vector<uint8_t> const& bytes) {
    winrt::Windows::Storage::Streams::DataWriter writer;
    if (!bytes.empty()) writer.WriteBytes(bytes);
    return writer.DetachBuffer();
}

inline std::shared_ptr<std::vector<uint8_t>> FromBuffer(Bytes const& buffer) {
    if (!buffer || buffer.Length() == 0) return std::make_shared<std::vector<uint8_t>>();
    uint8_t const* data = buffer.data();
    return std::make_shared<std::vector<uint8_t>>(data, data + buffer.Length());
}

inline std::wstring UrlKey(std::string_view url) {
    return L"url:" + ToWide(url);
}

inline std::string SanitizeFolderSegment(std::string_view id) {
    std::string out;
    out.reserve(id.size());
    for (unsigned char ch : id) {
        if (ch < 0x20 || ch == '<' || ch == '>' || ch == ':' || ch == '"' || ch == '/' || ch == '\\' || ch == '|' || ch == '?' || ch == '*') {
            out.push_back('_');
        } else {
            out.push_back(static_cast<char>(ch));
        }
    }
    while (!out.empty() && (out.back() == ' ' || out.back() == '.')) out.pop_back();
    return out.empty() ? std::string("Bern") : out;
}

inline std::string LowerAscii(std::string_view text) {
    std::string out;
    out.reserve(text.size());
    for (unsigned char ch : text) out.push_back(static_cast<char>(std::tolower(ch)));
    return out;
}

inline bool StartsWithIgnoreCase(std::string_view text, std::string_view prefix) {
    if (text.size() < prefix.size()) return false;
    for (size_t i = 0; i < prefix.size(); ++i) {
        if (std::tolower(static_cast<unsigned char>(text[i])) != std::tolower(static_cast<unsigned char>(prefix[i]))) return false;
    }
    return true;
}

inline bool IsLoopbackIPv4(std::string_view host) {
    if (!host.starts_with("127.")) return false;
    int parts = 0;
    size_t pos = 0;
    while (pos < host.size()) {
        size_t dot = host.find('.', pos);
        if (dot == std::string_view::npos) dot = host.size();
        if (dot == pos || dot - pos > 3) return false;
        int val = 0;
        for (size_t i = pos; i < dot; ++i) {
            if (host[i] < '0' || host[i] > '9') return false;
            val = val * 10 + (host[i] - '0');
        }
        if (val > 255) return false;
        ++parts;
        pos = dot + 1;
    }
    return parts == 4;
}

/// Only cache non-loopback `http://` and `https://` URLs on disk.
inline bool IsCacheableUrl(std::string_view url) {
    size_t scheme_len = 0;
    if (StartsWithIgnoreCase(url, "http://")) scheme_len = 7;
    else if (StartsWithIgnoreCase(url, "https://")) scheme_len = 8;
    else return false;

    std::string_view rest = url.substr(scheme_len);
    size_t auth_end = rest.find_first_of("/?#");
    std::string_view authority = auth_end == std::string_view::npos ? rest : rest.substr(0, auth_end);
    size_t at_pos = authority.rfind('@');
    if (at_pos != std::string_view::npos) authority = authority.substr(at_pos + 1);
    if (authority.empty()) return false;

    if (authority.front() == '[') {
        size_t close = authority.find(']');
        if (close == std::string_view::npos) return false;
        std::string host = LowerAscii(authority.substr(1, close - 1));
        if (host.empty() || host == "::1" || host == "0:0:0:0:0:0:0:1") return false;
        return true;
    }

    size_t colon = authority.find(':');
    std::string_view raw_host = colon == std::string_view::npos ? authority : authority.substr(0, colon);
    std::string host = LowerAscii(raw_host);
    if (host.empty() || host == "localhost" || host == "127.0.0.1" || IsLoopbackIPv4(host)) return false;
    return true;
}

/// 128-bit dual FNV-1a as 32 lowercase hex characters.
inline std::wstring CacheFileName(std::string_view url) {
    constexpr uint64_t kPrime = 1099511628211ULL;
    uint64_t h1 = 14695981039346656037ULL;
    for (unsigned char ch : url) {
        h1 ^= ch;
        h1 *= kPrime;
    }
    uint64_t h2 = 14695981039346656037ULL;
    static constexpr char const kSalt[] = "\x01bern.pictures.v1";
    for (char ch : std::string_view(kSalt, sizeof(kSalt) - 1)) {
        h2 ^= static_cast<unsigned char>(ch);
        h2 *= kPrime;
    }
    for (auto it = url.rbegin(); it != url.rend(); ++it) {
        h2 ^= static_cast<unsigned char>(*it);
        h2 *= kPrime;
    }
    wchar_t hex[33] = {};
    std::swprintf(hex, 33, L"%016llx%016llx", static_cast<unsigned long long>(h1), static_cast<unsigned long long>(h2));
    return std::wstring(hex, 32);
}

/// A client for pictures. All the clients of a process share their
/// connections, and over HTTP/2 a server has one: after a stall a new client
/// would go back to it, so only one that keeps to HTTP/1.1 (`apart`) gets a
/// new connection.
inline http::HttpClient NewClient(bool apart = false) {
    http::Filters::HttpBaseProtocolFilter filter;
    filter.AllowUI(false);
    if (apart) filter.MaxVersion(http::HttpVersion::Http11);
    http::HttpClient client(filter);
    client.DefaultRequestHeaders().UserAgent().TryParseAdd(L"Bern");
    return client;
}

struct Inflight {
    struct Caller {
        std::weak_ptr<Waiter> waiter;
        std::shared_ptr<CallerWait> signal;
    };

    std::string url;
    std::function<bool(Bytes const&)> accept;
    std::shared_ptr<Waiter> driver = std::make_shared<Waiter>();
    std::vector<Caller> callers;
    bool completed = false;
    bool dying = false;
    Bytes result{nullptr};

    void SyncDriver();
    void WakeDriver();
};

struct Shared {
    std::mutex lock;
    http::HttpClient client{nullptr};
    size_t silent_streak = 0;
    /// Addresses the server refused on this network.
    std::set<std::string> refused;
    /// Who waits, for a change for the better.
    std::vector<std::weak_ptr<Waiter>> waiters;
    bool listening = false;
    /// Concurrent HTTP request limit (max 8).
    size_t active_requests = 0;
    std::deque<std::shared_ptr<GateSignal>> gate_waiters;
    /// In-flight downloads shared by URL.
    std::unordered_map<std::string, std::shared_ptr<Inflight>> inflight;
    /// Disk cache configuration & counters.
#ifdef BERN_BUNDLE_ID
    std::string bundle_id = SanitizeFolderSegment(BERN_BUNDLE_ID);
#else
    std::string bundle_id = "Bern";
#endif
    bool cache_dir_resolved = false;
    std::filesystem::path cache_dir;
    std::atomic<uint64_t> tmp_seq{0};
    std::atomic<uint64_t> disk_writes{0};
    std::atomic<bool> evicting{false};
};

inline Shared& Pictures() {
    static Shared shared;
    return shared;
}

struct MemoryCache {
    using Entry = std::pair<std::wstring, std::shared_ptr<std::vector<uint8_t>>>;
    std::mutex lock;
    std::list<Entry> lru;
    std::unordered_map<std::wstring, std::list<Entry>::iterator> index;
    size_t size = 0;
};

inline MemoryCache& RecentBytes() {
    static MemoryCache cache;
    return cache;
}

inline std::shared_ptr<std::vector<uint8_t>> MemoryFind(std::wstring const& key) {
    if (key.empty()) return nullptr;
    auto& cache = RecentBytes();
    std::lock_guard held(cache.lock);
    auto found = cache.index.find(key);
    if (found == cache.index.end()) return nullptr;
    cache.lru.splice(cache.lru.end(), cache.lru, found->second);
    return found->second->second;
}

inline void MemoryKeep(std::wstring const& key, std::shared_ptr<std::vector<uint8_t>> const& bytes) {
    if (key.empty() || !bytes || bytes->empty() || bytes->size() > kMaxCacheFileBytes) return;
    auto& cache = RecentBytes();
    std::lock_guard held(cache.lock);
    if (auto found = cache.index.find(key); found != cache.index.end()) {
        cache.lru.splice(cache.lru.end(), cache.lru, found->second);
        return;
    }
    cache.lru.emplace_back(key, bytes);
    auto inserted = std::prev(cache.lru.end());
    cache.index.emplace(key, inserted);
    cache.size += bytes->size();
    while (cache.size > kMaxMemoryCacheBytes && !cache.lru.empty()) {
        auto oldest = cache.lru.begin();
        cache.size -= oldest->second->size();
        cache.index.erase(oldest->first);
        cache.lru.pop_front();
    }
}

inline std::filesystem::path ResolveCacheDir() {
    auto& shared = Pictures();
    std::string bundle;
    {
        std::lock_guard held(shared.lock);
        if (shared.cache_dir_resolved) return shared.cache_dir;
        bundle = shared.bundle_id;
    }
    std::filesystem::path base;
    PWSTR known = nullptr;
    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_LocalAppData, KF_FLAG_CREATE, nullptr, &known)) && known && *known) {
        base = std::filesystem::path(known);
    }
    if (known) CoTaskMemFree(known);
    if (base.empty()) {
        DWORD needed = GetTempPathW(0, nullptr);
        if (needed > 0) {
            std::wstring tmp(needed, L'\0');
            DWORD got = GetTempPathW(needed, tmp.data());
            if (got > 0 && got < needed) {
                tmp.resize(got);
                base = std::filesystem::path(tmp);
            }
        }
    }
    std::filesystem::path resolved;
    if (!base.empty()) {
        resolved = base / ToWide(bundle) / L"Cache" / L"pictures";
        std::error_code ec;
        std::filesystem::create_directories(resolved, ec);
        if (ec || !std::filesystem::exists(resolved, ec)) resolved.clear();
    }
    std::lock_guard held(shared.lock);
    if (!shared.cache_dir_resolved && shared.bundle_id == bundle) {
        shared.cache_dir = resolved;
        shared.cache_dir_resolved = true;
    }
    return shared.cache_dir_resolved ? shared.cache_dir : resolved;
}

inline std::shared_ptr<std::vector<uint8_t>> DiskFind(std::string_view url) {
    if (!IsCacheableUrl(url)) return nullptr;
    auto dir = ResolveCacheDir();
    if (dir.empty()) return nullptr;
    auto path = dir / CacheFileName(url);
    HANDLE file = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr, OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) return nullptr;
    LARGE_INTEGER file_size{};
    if (!GetFileSizeEx(file, &file_size) || file_size.QuadPart <= 0 || static_cast<uint64_t>(file_size.QuadPart) > kMaxCacheFileBytes) {
        CloseHandle(file);
        return nullptr;
    }
    auto bytes = std::make_shared<std::vector<uint8_t>>(static_cast<size_t>(file_size.QuadPart));
    size_t offset = 0;
    while (offset < bytes->size()) {
        DWORD chunk = static_cast<DWORD>((std::min<size_t>)(bytes->size() - offset, 1u << 20));
        DWORD read = 0;
        if (!ReadFile(file, bytes->data() + offset, chunk, &read, nullptr) || read == 0) {
            CloseHandle(file);
            return nullptr;
        }
        offset += read;
    }
    CloseHandle(file);
    return bytes;
}

inline void DiskRemove(std::string_view url) {
    if (!IsCacheableUrl(url)) return;
    auto dir = ResolveCacheDir();
    if (dir.empty()) return;
    auto path = dir / CacheFileName(url);
    DeleteFileW(path.c_str());
}

inline void EvictDiskCache(std::filesystem::path dir) {
    auto& shared = Pictures();
    struct Guard {
        std::atomic<bool>& flag;
        ~Guard() { flag.store(false); }
    } reset{shared.evicting};

    std::error_code ec;
    if (dir.empty() || !std::filesystem::exists(dir, ec)) return;
    struct Item {
        std::filesystem::path path;
        uint64_t size = 0;
        std::filesystem::file_time_type mtime{};
    };
    std::vector<Item> items;
    uint64_t total = 0;
    for (auto const& entry : std::filesystem::directory_iterator(dir, std::filesystem::directory_options::skip_permission_denied, ec)) {
        if (ec) break;
        std::error_code item_ec;
        if (!entry.is_regular_file(item_ec) || item_ec) continue;
        auto name = entry.path().filename().wstring();
        if (name.ends_with(L".tmp") || name.size() != 32) continue;
        uint64_t sz = entry.file_size(item_ec);
        if (item_ec) continue;
        auto mt = entry.last_write_time(item_ec);
        if (item_ec) continue;
        total += sz;
        items.push_back({entry.path(), sz, mt});
    }
    if (total <= kMaxDiskCacheBytes) return;
    std::sort(items.begin(), items.end(), [](Item const& a, Item const& b) { return a.mtime < b.mtime; });
    for (auto const& item : items) {
        if (total <= kDiskEvictTargetBytes) break;
        if (DeleteFileW(item.path.c_str())) {
            total = total > item.size ? total - item.size : 0;
        }
    }
}

inline void DiskKeep(std::string_view url, std::vector<uint8_t> const& bytes) {
    if (!IsCacheableUrl(url) || bytes.empty() || bytes.size() > kMaxCacheFileBytes) return;
    auto dir = ResolveCacheDir();
    if (dir.empty()) return;
    auto& shared = Pictures();
    std::wstring name = CacheFileName(url);
    auto final_path = dir / name;
    uint64_t seq = shared.tmp_seq.fetch_add(1) + 1;
    auto tmp_path = dir / (name + L"." + std::to_wstring(seq) + L".tmp");

    HANDLE file = CreateFileW(tmp_path.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) return;
    size_t offset = 0;
    bool ok = true;
    while (offset < bytes.size()) {
        DWORD chunk = static_cast<DWORD>((std::min<size_t>)(bytes.size() - offset, 1u << 20));
        DWORD written = 0;
        if (!WriteFile(file, bytes.data() + offset, chunk, &written, nullptr) || written == 0) {
            ok = false;
            break;
        }
        offset += written;
    }
    CloseHandle(file);
    if (!ok || !MoveFileExW(tmp_path.c_str(), final_path.c_str(), MOVEFILE_REPLACE_EXISTING)) {
        DeleteFileW(tmp_path.c_str());
        return;
    }
    uint64_t writes = shared.disk_writes.fetch_add(1) + 1;
    if (writes % kDiskEvictInterval == 0 && !shared.evicting.exchange(true)) {
        std::thread([dir] { EvictDiskCache(dir); }).detach();
    }
}

inline int64_t Now() {
    return std::chrono::steady_clock::now().time_since_epoch().count();
}

/// Cancels a request when `kSilence` passes after the last it `heard`, or
/// when its driver waiter ends.
class Silence {
public:
    Silence(foundation::IAsyncInfo request, std::shared_ptr<std::atomic<int64_t>> heard, std::shared_ptr<Waiter> waiter = nullptr) {
        using namespace winrt::Windows::System::Threading;
        timer_ = ThreadPoolTimer::CreatePeriodicTimer(
            [request, heard, waiter](ThreadPoolTimer const&) {
                if (waiter && waiter->Ended()) {
                    request.Cancel();
                    return;
                }
                auto quiet = std::chrono::steady_clock::duration(Now() - heard->load());
                if (quiet >= kSilence) request.Cancel();
            },
            std::chrono::milliseconds(100));
    }
    ~Silence() { timer_.Cancel(); }
    Silence(Silence const&) = delete;

private:
    winrt::Windows::System::Threading::ThreadPoolTimer timer_{nullptr};
};

/// Errors another try would only repeat. Any other is worth one.
inline bool Lasting(winrt::hresult code) {
    switch (static_cast<uint32_t>(code.value)) {
    case 0x80072F05:  // the certificate's date
    case 0x80072F06:  // the certificate's name
    case 0x80072F0D:  // the certificate's authority
    case 0x80072EE6:  // the address's scheme
    case 0x80072EE4:  // an address that makes no sense
    case 0x80070057:  // E_INVALIDARG: an address that is not one
        return true;
    default:
        return false;
    }
}

/// One try at `url`. It gives the bytes and says in `reading` what became
/// of it.
inline foundation::IAsyncOperation<Bytes> Try(http::HttpClient client, std::string url, std::shared_ptr<Reading> reading, std::shared_ptr<Waiter> waiter = nullptr) {
    *reading = Reading::Unreachable;
    try {
        auto heard = std::make_shared<std::atomic<int64_t>>(Now());
        foundation::Uri address{winrt::to_hstring(url)};
        auto asked = client.GetAsync(address, http::HttpCompletionOption::ResponseHeadersRead);
        Silence waiting(asked, heard, waiter);
        auto response = co_await asked;
        auto status = static_cast<uint32_t>(response.StatusCode());
        if (status < 200 || status > 299) {
            *reading = status == 408 || status == 429 || status >= 500 ? Reading::Unreachable : Reading::Refused;
            co_return nullptr;
        }
        auto body = response.Content().ReadAsBufferAsync();
        Silence reading_body(body, heard, waiter);
        body.Progress([heard](auto&&, uint64_t) { heard->store(Now()); });
        Bytes bytes = co_await body;
        *reading = Reading::Picture;
        co_return bytes;
    } catch (winrt::hresult_canceled const&) {
        *reading = Reading::Silent;
    } catch (winrt::hresult_error const& error) {
        if (error.code() == E_ABORT) *reading = Reading::Silent;
        else if (Lasting(error.code())) *reading = Reading::Refused;
    }
    co_return nullptr;
}

/// Waits `seconds`, or with none for as long as it takes, until `waiter`
/// is woken.
inline foundation::IAsyncAction Pause(std::shared_ptr<Waiter> waiter, std::optional<double> seconds) {
    if (seconds) {
        auto wait = std::chrono::duration_cast<foundation::TimeSpan>(std::chrono::duration<double>(*seconds));
        co_await winrt::resume_on_signal(waiter->Handle(), wait);
    } else {
        co_await winrt::resume_on_signal(waiter->Handle());
    }
}

inline double Jitter() {
    thread_local std::mt19937 random{std::random_device{}()};
    return std::uniform_real_distribution<double>(0.75, 1.25)(random);
}

inline void ReleaseGateSlotLocked(Shared& shared) {
    while (!shared.gate_waiters.empty()) {
        auto next = std::move(shared.gate_waiters.front());
        shared.gate_waiters.pop_front();
        if (next && !next->cancelled) {
            next->granted.store(true);
            SetEvent(next->event.get());
            return;
        }
    }
    if (shared.active_requests > 0) --shared.active_requests;
}

class SlotGuard {
public:
    SlotGuard() = default;
    explicit SlotGuard(bool acquired) : acquired_(acquired) {}
    SlotGuard(SlotGuard&& other) noexcept : acquired_(other.acquired_) { other.acquired_ = false; }
    SlotGuard& operator=(SlotGuard&& other) noexcept {
        if (this != &other) {
            Release();
            acquired_ = other.acquired_;
            other.acquired_ = false;
        }
        return *this;
    }
    SlotGuard(SlotGuard const&) = delete;
    SlotGuard& operator=(SlotGuard const&) = delete;
    ~SlotGuard() { Release(); }

    bool Acquired() const { return acquired_; }
    void Release() {
        if (!acquired_) return;
        acquired_ = false;
        auto& shared = Pictures();
        std::lock_guard held(shared.lock);
        ReleaseGateSlotLocked(shared);
    }

private:
    bool acquired_ = false;
};

inline foundation::IAsyncOperation<bool> AcquireSlot(std::shared_ptr<Waiter> waiter) {
    auto& shared = Pictures();
    std::shared_ptr<GateSignal> sig;
    {
        std::lock_guard held(shared.lock);
        if (waiter->Ended() || !waiter->Shown()) co_return false;
        if (shared.active_requests < kMaxConcurrentRequests) {
            ++shared.active_requests;
            co_return true;
        }
        sig = std::make_shared<GateSignal>();
        shared.gate_waiters.push_back(sig);
    }
    waiter->AttachGate(sig);
    if (waiter->Ended() || !waiter->Shown()) SetEvent(sig->event.get());
    co_await winrt::resume_on_signal(sig->event.get());
    waiter->DetachGate(sig.get());

    std::lock_guard held(shared.lock);
    if (sig->granted.load()) {
        if (waiter->Ended() || !waiter->Shown()) {
            ReleaseGateSlotLocked(shared);
            co_return false;
        }
        co_return true;
    }
    sig->cancelled = true;
    std::erase(shared.gate_waiters, sig);
    co_return false;
}

}  // namespace detail

/// Sets the bundle identifier used for the persistent picture cache folder
/// (`%LOCALAPPDATA%\<bundleId>\Cache\pictures`).
inline void SetBundleId(std::string const& bundleId) {
    if (bundleId.empty()) return;
    auto sanitized = detail::SanitizeFolderSegment(bundleId);
    auto& shared = detail::Pictures();
    std::lock_guard held(shared.lock);
    if (shared.bundle_id == sanitized) return;
    shared.bundle_id = std::move(sanitized);
    shared.cache_dir_resolved = false;
    shared.cache_dir.clear();
}

inline std::shared_ptr<std::vector<uint8_t>> MemoryFind(std::wstring const& key) {
    return detail::MemoryFind(key);
}

inline void MemoryKeep(std::wstring const& key, std::shared_ptr<std::vector<uint8_t>> const& bytes) {
    detail::MemoryKeep(key, bytes);
}

inline std::wstring UrlKey(std::string_view url) {
    return detail::UrlKey(url);
}

/// Every load waiting for its next try goes now.
inline void TryAgain() {
    auto& shared = detail::Pictures();
    std::vector<std::shared_ptr<Waiter>> waiting;
    {
        std::lock_guard held(shared.lock);
        std::erase_if(shared.waiters, [](auto const& one) { return one.expired(); });
        for (auto const& one : shared.waiters)
            if (auto waiter = one.lock()) waiting.push_back(waiter);
    }
    for (auto const& waiter : waiting) waiter->Wake();
}

/// The system's word on the network: once it is there after a change, a new
/// way out may answer what the last one could not.
inline void NetworkChanged(bool online) {
    if (!online) return;
    {
        auto& shared = detail::Pictures();
        std::lock_guard held(shared.lock);
        shared.refused.clear();
    }
    TryAgain();
}

namespace detail {

/// Registers `waiter`, and starts hearing of the network on the first.
inline void Enlist(std::shared_ptr<Waiter> const& waiter) {
    auto& shared = Pictures();
    std::lock_guard held(shared.lock);
    shared.waiters.push_back(waiter);
    if (!shared.client) shared.client = NewClient(false);
    if (shared.listening) return;
    shared.listening = true;
    using winrt::Windows::Networking::Connectivity::NetworkInformation;
    NetworkInformation::NetworkStatusChanged([](auto&&) {
        NetworkChanged(NetworkInformation::GetInternetConnectionProfile() != nullptr);
    });
}

inline bool Refused(std::string const& url) {
    auto& shared = Pictures();
    std::lock_guard held(shared.lock);
    return shared.refused.contains(url);
}

inline void Refuse(std::string const& url) {
    auto& shared = Pictures();
    std::lock_guard held(shared.lock);
    shared.refused.insert(url);
}

inline http::HttpClient Client() {
    auto& shared = Pictures();
    std::lock_guard held(shared.lock);
    if (!shared.client) shared.client = NewClient(false);
    return shared.client;
}

/// Keep the global client on HTTP/2 (`NewClient(false)`). Individual silent
/// failures retry with an isolated HTTP/1.1 client; only consecutive global
/// silents rotate the shared HTTP/2 client.
inline void NoteSilent(http::HttpClient const& used) {
    auto& shared = Pictures();
    std::lock_guard held(shared.lock);
    if (shared.client == used) {
        if (++shared.silent_streak >= kSilentRotateThreshold) {
            shared.client = NewClient(false);
            shared.silent_streak = 0;
        }
    }
}

inline void NoteResponsive() {
    auto& shared = Pictures();
    std::lock_guard held(shared.lock);
    shared.silent_streak = 0;
}

inline void Inflight::SyncDriver() {
    auto& shared = Pictures();
    bool any_live = false;
    bool any_shown = false;
    {
        std::lock_guard held(shared.lock);
        if (completed) return;
        std::erase_if(callers, [](Caller const& c) {
            auto w = c.waiter.lock();
            return !w || w->Ended();
        });
        any_live = !callers.empty();
        for (auto const& c : callers) {
            if (auto w = c.waiter.lock(); w && w->Shown()) {
                any_shown = true;
                break;
            }
        }
        if (!any_live) {
            dying = true;
            if (auto it = shared.inflight.find(url); it != shared.inflight.end() && it->second.get() == this) {
                shared.inflight.erase(it);
            }
        }
    }
    if (!any_live) driver->End();
    else if (any_shown) driver->Appeared();
    else driver->Left();
}

inline void Inflight::WakeDriver() {
    auto& shared = Pictures();
    bool should_wake = false;
    {
        std::lock_guard held(shared.lock);
        if (completed) return;
        for (auto const& c : callers) {
            if (auto w = c.waiter.lock(); w && !w->Ended() && w->Shown()) {
                should_wake = true;
                break;
            }
        }
    }
    if (should_wake) driver->Wake();
}

inline void FinishInflight(std::shared_ptr<Inflight> const& task, Bytes const& bytes) {
    auto& shared = Pictures();
    std::vector<Inflight::Caller> notify;
    {
        std::lock_guard held(shared.lock);
        if (auto it = shared.inflight.find(task->url); it != shared.inflight.end() && it->second == task) {
            shared.inflight.erase(it);
        }
        task->completed = true;
        task->result = bytes;
        notify.swap(task->callers);
    }
    for (auto const& c : notify) {
        if (c.signal) SetEvent(c.signal->event.get());
    }
}

inline winrt::fire_and_forget RunInflight(std::shared_ptr<Inflight> task) {
    co_await winrt::resume_background();

    if (auto cached = DiskFind(task->url); cached && !cached->empty()) {
        Bytes buf = ToBuffer(*cached);
        if (buf && task->accept(buf)) {
            MemoryKeep(UrlKey(task->url), cached);
            FinishInflight(task, buf);
            co_return;
        }
        DiskRemove(task->url);
    }

    Enlist(task->driver);
    size_t tries = 0;
    uint64_t seen = task->driver->Appearances();
    auto reading = std::make_shared<Reading>();
    bool use_isolated_http11 = false;

    while (!task->driver->Ended()) {
        if (!task->driver->Shown() || Refused(task->url)) {
            co_await Pause(task->driver, std::nullopt);
            continue;
        }
        if (uint64_t now = task->driver->Appearances(); now != seen) {
            seen = now;
            tries = 0;
            use_isolated_http11 = false;
        }

        SlotGuard slot(co_await AcquireSlot(task->driver));
        if (!slot.Acquired()) continue;

        http::HttpClient shared_client = Client();
        http::HttpClient request_client = use_isolated_http11 ? NewClient(true) : shared_client;
        Bytes bytes = co_await Try(request_client, task->url, reading, task->driver);
        slot.Release();

        if (*reading != Reading::Silent) {
            NoteResponsive();
            use_isolated_http11 = false;
        }
        if (*reading == Reading::Picture) {
            if (bytes && task->accept(bytes)) {
                auto vec = FromBuffer(bytes);
                if (vec && !vec->empty()) {
                    MemoryKeep(UrlKey(task->url), vec);
                    DiskKeep(task->url, *vec);
                }
                FinishInflight(task, bytes);
                co_return;
            }
            *reading = Reading::Refused;
        }
        if (*reading == Reading::Refused) {
            Refuse(task->url);
            continue;
        }
        if (*reading == Reading::Silent) {
            NoteSilent(request_client);
            use_isolated_http11 = true;
        }
        std::optional<double> wait;
        if (tries < std::size(kWaits)) wait = kWaits[tries] * Jitter();
        co_await Pause(task->driver, wait);
        tries = wait ? tries + 1 : 0;
    }

    FinishInflight(task, nullptr);
}

}  // namespace detail

inline void Waiter::NotifyInflight(bool wake_only, bool sync_driver) {
    std::vector<std::shared_ptr<detail::Inflight>> tasks;
    {
        std::lock_guard held(hook_lock_);
        std::erase_if(inflight_hooks_, [](auto const& hook) { return hook.caller.expired() || hook.task.expired(); });
        for (auto const& hook : inflight_hooks_) {
            if (auto t = hook.task.lock()) tasks.push_back(std::move(t));
        }
    }
    if (sync_driver) SyncTasks(tasks, false);
    if (wake_only) SyncTasks(tasks, true);
}

inline void Waiter::SyncTasks(std::vector<std::shared_ptr<detail::Inflight>> const& tasks, bool wake_only) {
    for (auto const& t : tasks) {
        if (wake_only) t->WakeDriver();
        else t->SyncDriver();
    }
}

/// The bytes at the http(s) address `url`, once they come and `accept`
/// takes them for a picture; nothing once `waiter` ended.
inline foundation::IAsyncOperation<Bytes> Until(std::string url, std::shared_ptr<Waiter> waiter, std::function<bool(Bytes const&)> accept) {
    if (!waiter || waiter->Ended()) co_return nullptr;
    if (auto cached = detail::MemoryFind(detail::UrlKey(url)); cached && !cached->empty()) {
        Bytes buf = detail::ToBuffer(*cached);
        if (buf && accept(buf)) co_return buf;
    }

    while (!waiter->Ended()) {
        std::shared_ptr<detail::Inflight> task;
        auto caller = std::make_shared<detail::CallerWait>();
        bool is_leader = false;
        {
            auto& shared = detail::Pictures();
            std::lock_guard held(shared.lock);
            if (waiter->Ended()) co_return nullptr;
            if (auto it = shared.inflight.find(url); it != shared.inflight.end() && !it->second->completed && !it->second->dying) {
                task = it->second;
            } else {
                task = std::make_shared<detail::Inflight>();
                task->url = url;
                task->accept = accept;
                shared.inflight[url] = task;
                is_leader = true;
            }
            task->callers.push_back({waiter, caller});
        }

        waiter->AttachInflight(task, caller);
        task->SyncDriver();
        if (is_leader) detail::RunInflight(task);

        co_await winrt::resume_on_signal(caller->event.get());
        waiter->DetachCaller(caller.get());

        if (waiter->Ended()) {
            task->SyncDriver();
            co_return nullptr;
        }

        Bytes result{nullptr};
        bool completed = false;
        {
            auto& shared = detail::Pictures();
            std::lock_guard held(shared.lock);
            completed = task->completed;
            result = task->result;
        }
        if (completed) {
            if (result && accept(result)) co_return result;
            co_return nullptr;
        }
        co_return nullptr;
    }
    co_return nullptr;
}

}  // namespace bern::pictures
