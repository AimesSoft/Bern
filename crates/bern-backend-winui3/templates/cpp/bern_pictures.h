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

#include <atomic>
#include <chrono>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <random>
#include <set>
#include <string>
#include <vector>

#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Networking.Connectivity.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.Threading.h>
#include <winrt/Windows.Web.Http.h>
#include <winrt/Windows.Web.Http.Filters.h>
#include <winrt/Windows.Web.Http.Headers.h>

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
    void Wake() { SetEvent(wake_.get()); }
    /// Nothing waits any longer.
    void End() {
        ended_ = true;
        Wake();
    }
    /// The view left: no tries until it comes back.
    void Left() { shown_ = false; }
    /// The view is back: it starts over.
    void Appeared() {
        if (!shown_.exchange(true)) {
            ++appearances_;
            Wake();
        }
    }

private:
    winrt::handle wake_;
    std::atomic<bool> ended_{false};
    std::atomic<bool> shown_{true};
    std::atomic<uint64_t> appearances_{0};
};

namespace detail {

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

struct Shared {
    std::mutex lock;
    http::HttpClient client{nullptr};
    /// Addresses the server refused on this network.
    std::set<std::string> refused;
    /// Who waits, for a change for the better.
    std::vector<std::weak_ptr<Waiter>> waiters;
    bool listening = false;
};

inline Shared& Pictures() {
    static Shared shared;
    return shared;
}

inline int64_t Now() {
    return std::chrono::steady_clock::now().time_since_epoch().count();
}

/// Cancels a request when `kSilence` passes after the last it `heard`.
class Silence {
public:
    Silence(foundation::IAsyncInfo request, std::shared_ptr<std::atomic<int64_t>> heard) {
        using namespace winrt::Windows::System::Threading;
        timer_ = ThreadPoolTimer::CreatePeriodicTimer(
            [request, heard](ThreadPoolTimer const&) {
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
inline foundation::IAsyncOperation<Bytes> Try(http::HttpClient client, std::string url, std::shared_ptr<Reading> reading) {
    *reading = Reading::Unreachable;
    try {
        auto heard = std::make_shared<std::atomic<int64_t>>(Now());
        foundation::Uri address{winrt::to_hstring(url)};
        auto asked = client.GetAsync(address, http::HttpCompletionOption::ResponseHeadersRead);
        Silence waiting(asked, heard);
        auto response = co_await asked;
        auto status = static_cast<uint32_t>(response.StatusCode());
        if (status < 200 || status > 299) {
            *reading = status == 408 || status == 429 || status >= 500 ? Reading::Unreachable : Reading::Refused;
            co_return nullptr;
        }
        auto body = response.Content().ReadAsBufferAsync();
        Silence reading_body(body, heard);
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

}  // namespace detail

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
    if (!shared.client) shared.client = NewClient();
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
    return shared.client;
}

/// Requests to one server share a connection, and the next try would join
/// the one that stalled.
inline void Renew(http::HttpClient const& used) {
    auto& shared = Pictures();
    std::lock_guard held(shared.lock);
    if (shared.client == used) shared.client = NewClient(true);
}

}  // namespace detail

/// The bytes at the http(s) address `url`, once they come and `accept`
/// takes them for a picture; nothing once `waiter` ended.
inline foundation::IAsyncOperation<Bytes> Until(std::string url, std::shared_ptr<Waiter> waiter, std::function<bool(Bytes const&)> accept) {
    detail::Enlist(waiter);
    size_t tries = 0;
    uint64_t seen = waiter->Appearances();
    auto reading = std::make_shared<Reading>();
    while (!waiter->Ended()) {
        if (!waiter->Shown() || detail::Refused(url)) {
            co_await detail::Pause(waiter, std::nullopt);
            continue;
        }
        if (uint64_t now = waiter->Appearances(); now != seen) {
            seen = now;
            tries = 0;
        }
        auto client = detail::Client();
        Bytes bytes = co_await detail::Try(client, url, reading);
        if (*reading == Reading::Picture) {
            if (accept(bytes)) co_return bytes;
            *reading = Reading::Refused;
        }
        if (*reading == Reading::Refused) {
            detail::Refuse(url);
            continue;
        }
        if (*reading == Reading::Silent) detail::Renew(client);
        std::optional<double> wait;
        if (tries < std::size(kWaits)) wait = kWaits[tries] * detail::Jitter();
        co_await detail::Pause(waiter, wait);
        tries = wait ? tries + 1 : 0;
    }
    co_return nullptr;
}

}  // namespace bern::pictures
