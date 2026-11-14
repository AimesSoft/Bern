// Native pure-Rust animated SVG presenter (`B::Svg` via `WriteableBitmap`)
// and optional `B::WebPage` (`WebView2`, included only when `WebPage` is used).

namespace B {

#if BERN_WITH_SVG
struct SvgProps {
    char const* source = nullptr;
    Str label;
    Len width;
    Len height;
    char const* fit = nullptr;
    Get playing;
    Get speed;
    Get restart;
    Str data;
    bool externalLinks = false;
    bool interactive = true;
};

struct NativeSvgState : std::enable_shared_from_this<NativeSvgState> {
    BernSvgPlayer* player = nullptr;
    controls::Grid frame;
    controls::Image image;
    media::Imaging::WriteableBitmap bitmap{nullptr};
    double widthPt = 0, heightPt = 0;
    bool playing = true, interactive = true, loaded = false, disposed = false, active = true;
    double speed = 1.0, restart = 0.0;
    uint8_t fitCode = 0;
    winrt::event_token renderToken{};
    winrt::event_token themeToken{};

    static double NowSeconds() {
        static LARGE_INTEGER freq = [] {
            LARGE_INTEGER f{};
            QueryPerformanceFrequency(&f);
            return f;
        }();
        LARGE_INTEGER counter{};
        QueryPerformanceCounter(&counter);
        return freq.QuadPart > 0 ? static_cast<double>(counter.QuadPart) / static_cast<double>(freq.QuadPart) : 0.0;
    }

    void RenderNow() {
        if (disposed || !player || !loaded) return;
        double scale = frame.XamlRoot() ? frame.XamlRoot().RasterizationScale() : 1.0;
        if (scale <= 0.0) scale = 1.0;
        uint32_t wPx = static_cast<uint32_t>((std::max)(1.0, std::round((std::max)(1.0, widthPt) * scale)));
        uint32_t hPx = static_cast<uint32_t>((std::max)(1.0, std::round((std::max)(1.0, heightPt) * scale)));
        bool dark = frame.ActualTheme() == xaml::ElementTheme::Dark;
        uint32_t textRgba = dark ? 0xFFFFFFFFu : 0x000000FFu;
        bern_svg_configure(
            player,
            playing ? 1 : 0,
            speed,
            restart,
            fitCode,
            interactive ? 1 : 0,
            dark ? 1 : 0,
            Reduced() ? 1 : 0,
            textRgba
        );
        uint8_t status = bern_svg_render(player, NowSeconds(), wPx, hPx, 1);
        if ((status & 1) != 0) {
            if (!bitmap || static_cast<uint32_t>(bitmap.PixelWidth()) != wPx || static_cast<uint32_t>(bitmap.PixelHeight()) != hPx) {
                bitmap = media::Imaging::WriteableBitmap(static_cast<int32_t>(wPx), static_cast<int32_t>(hPx));
                image.Source(bitmap);
            }
            uint8_t const* src = bern_svg_pixels_ptr(player);
            size_t len = bern_svg_pixels_len(player);
            auto buf = bitmap.PixelBuffer();
            if (src && buf.Length() == len && len > 0) {
                std::memcpy(buf.data(), src, len);
                bitmap.Invalidate();
            }
        }
        active = (status & 2) != 0;
        SyncLoop();
    }

    void SyncLoop() {
        bool want = !disposed && loaded && playing && active;
        if (want && !renderToken.value) {
            std::weak_ptr<NativeSvgState> weak = shared_from_this();
            renderToken = media::CompositionTarget::Rendering([weak](auto&&, auto&&) {
                if (auto s = weak.lock()) s->RenderNow();
            });
        } else if (!want && renderToken.value) {
            media::CompositionTarget::Rendering(renderToken);
            renderToken = {};
        }
    }

    void Pointer(uint8_t kind, winrt::Windows::Foundation::Point pt) {
        if (disposed || !player || !interactive) return;
        float nx = widthPt > 0 ? static_cast<float>(pt.X / widthPt) : 0.5f;
        float ny = heightPt > 0 ? static_cast<float>(pt.Y / heightPt) : 0.5f;
        uint8_t st = bern_svg_pointer(player, kind, nx, ny);
        if ((st & 2) != 0) active = true;
        if ((st & 1) != 0 || (st & 2) != 0) RenderNow();
    }

    void Close() {
        disposed = true;
        if (renderToken.value) {
            media::CompositionTarget::Rendering(renderToken);
            renderToken = {};
        }
        if (themeToken.value) {
            frame.ActualThemeChanged(themeToken);
            themeToken = {};
        }
        if (player) {
            bern_svg_free(player);
            player = nullptr;
        }
    }
};

inline void Svg(Parent const& p, Ctx const& ctx, SvgProps props) {
    auto state = std::make_shared<NativeSvgState>();
    std::string key = props.source ? props.source : "";
    if (!key.empty()) {
        auto const* keyPtr = reinterpret_cast<uint8_t const*>(key.data());
        if (bern_svg_cached(keyPtr, key.size()) != 0) {
            state->player = bern_svg_open(keyPtr, key.size(), nullptr, 0);
        } else {
            auto bytes = ResourceBytes(props.source);
            state->player = bern_svg_open(keyPtr, key.size(), bytes.data(), bytes.size());
        }
    }
    std::string_view fit = props.fit ? props.fit : "contain";
    state->fitCode = fit == "cover" ? 1 : (fit == "fill" ? 2 : 0);
    state->interactive = props.interactive;

    state->image.Stretch(media::Stretch::Fill);
    state->frame.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    state->frame.VerticalAlignment(xaml::VerticalAlignment::Top);
    state->frame.Background(media::SolidColorBrush(winrt::Windows::UI::Colors::Transparent()));
    state->frame.Children().Append(state->image);

    BindLen(ctx, props.width, nullptr, [state](double n) {
        state->widthPt = (std::max)(0.0, n);
        state->frame.Width(state->widthPt);
        state->RenderNow();
    });
    BindLen(ctx, props.height, nullptr, [state](double n) {
        state->heightPt = (std::max)(0.0, n);
        state->frame.Height(state->heightPt);
        state->RenderNow();
    });

    if (props.label) BindName(state->frame, Bound(ctx, props.label));
    else xaml::Automation::AutomationProperties::SetAccessibilityView(state->frame, xaml::Automation::Peers::AccessibilityView::Raw);

    Reactive::Run([state, ctx, props] {
        state->playing = !props.playing || props.playing(ctx) == Value::Bool(true);
        double speed = props.speed ? R::AsDouble(props.speed(ctx)) : 1.0;
        double restart = props.restart ? R::AsDouble(props.restart(ctx)) : 0.0;
        state->speed = (std::isfinite(speed) && speed > 0.0) ? speed : 1.0;
        state->restart = std::isfinite(restart) ? restart : 0.0;
        state->RenderNow();
    });

    std::weak_ptr<NativeSvgState> weak = state;
    if (props.interactive) {
        state->frame.IsDoubleTapEnabled(true);
        state->frame.IsTapEnabled(true);
        state->frame.DoubleTapped([weak](auto&&, xaml::Input::DoubleTappedRoutedEventArgs const& e) {
            if (auto s = weak.lock()) { e.Handled(true); s->Pointer(1, e.GetPosition(s->frame)); }
        });
        state->frame.Tapped([weak](auto&&, xaml::Input::TappedRoutedEventArgs const& e) {
            if (auto s = weak.lock()) { s->Pointer(0, e.GetPosition(s->frame)); }
        });
        state->frame.PointerMoved([weak](auto&&, xaml::Input::PointerRoutedEventArgs const& e) {
            if (auto s = weak.lock()) { s->Pointer(2, e.GetCurrentPoint(s->frame).Position()); }
        });
        state->frame.PointerExited([weak](auto&&, auto&&) {
            if (auto s = weak.lock()) { s->Pointer(3, {0, 0}); }
        });
        state->frame.PointerPressed([weak](auto&&, xaml::Input::PointerRoutedEventArgs const& e) {
            if (auto s = weak.lock()) { s->Pointer(4, e.GetCurrentPoint(s->frame).Position()); }
        });
        state->frame.PointerReleased([weak](auto&&, xaml::Input::PointerRoutedEventArgs const& e) {
            if (auto s = weak.lock()) { s->Pointer(5, e.GetCurrentPoint(s->frame).Position()); }
        });
    }

    state->themeToken = state->frame.ActualThemeChanged([weak](auto&&, auto&&) {
        if (auto s = weak.lock()) s->RenderNow();
    });
    state->frame.Loaded([weak](auto&&, auto&&) {
        if (auto s = weak.lock()) { s->loaded = true; s->RenderNow(); }
    });
    state->frame.Unloaded([weak](auto&&, auto&&) {
        if (auto s = weak.lock()) { s->loaded = false; s->SyncLoop(); }
    });

    Place(p, state->frame);
    Reactive::OnCleanup([state] { state->Close(); });
}
#endif

#if BERN_WITH_WEBPAGE
namespace WebPageRuntime {
inline std::wstring Cache() {
    wchar_t temp[MAX_PATH]{};
    GetTempPathW(MAX_PATH, temp);
    return std::wstring(temp) + L"BernWebPageWebView2";
}
}

inline std::string WebPageJsonString(std::string const& value) {
    std::string out = "\"";
    constexpr char hex[] = "0123456789abcdef";
    for (unsigned char ch : value) {
        if (ch == '\\' || ch == '\"') { out += '\\'; out += char(ch); }
        else if (ch < 0x20) { out += "\\u00"; out += hex[ch >> 4]; out += hex[ch & 15]; }
        else out += char(ch);
    }
    return out + "\"";
}

struct WebPageView : std::enable_shared_from_this<WebPageView> {
    std::wstring page;
    std::vector<std::pair<std::wstring, std::wstring>> cookies;
    std::wstring cookieDomain, userAgent;
    std::string closeOn;
    std::function<void()> close;
    controls::Grid frame;
    bool started = false, disposed = false, visible = false;
    HWND window = nullptr;
    Win* owner = nullptr;
    winrt::com_ptr<ICoreWebView2Controller> controller;
    winrt::com_ptr<ICoreWebView2> web;
    winrt::event_token layoutToken{};
    std::array<int, 9> placed{};
    HWND cut = nullptr;
    winrt::event_token renderToken{};
    int still = 0;

    bool Layout() {
        if (!window || !owner || !owner->root) return false;
        auto root = owner->root;
        double scale = frame.XamlRoot() ? frame.XamlRoot().RasterizationScale() : 1.0;
        auto boundsIn = [&](xaml::FrameworkElement const& element) {
            return element.TransformToVisual(root).TransformBounds(
                {0, 0, static_cast<float>(element.ActualWidth()), static_cast<float>(element.ActualHeight())});
        };
        bool shown = frame.IsLoaded() && frame.Visibility() == xaml::Visibility::Visible;
        winrt::Windows::Foundation::Rect box{};
        double cl = 0, ct = 0, cr = 0, cb = 0;
        try {
            if (shown) {
                box = boundsIn(frame);
                cl = box.X, ct = box.Y, cr = box.X + box.Width, cb = box.Y + box.Height;
                for (auto up = media::VisualTreeHelper::GetParent(frame); up; up = media::VisualTreeHelper::GetParent(up)) {
                    auto viewer = up.try_as<controls::ScrollViewer>();
                    if (!viewer) continue;
                    auto r = boundsIn(viewer);
                    cl = (std::max)(cl, double(r.X));
                    ct = (std::max)(ct, double(r.Y));
                    cr = (std::min)(cr, double(r.X + r.Width));
                    cb = (std::min)(cb, double(r.Y + r.Height));
                }
            }
        } catch (winrt::hresult_error const&) {
            shown = false;
        }
        int x = int(std::round(box.X * scale)), y = int(std::round(box.Y * scale));
        int w = int(std::round(box.Width * scale)), h = int(std::round(box.Height * scale));
        bool vis = shown && w > 0 && h > 0 && cr > cl && cb > ct;
        int l = vis ? int(std::floor(cl * scale)) - x : 0, t = vis ? int(std::floor(ct * scale)) - y : 0;
        int r = vis ? int(std::ceil(cr * scale)) - x : 0, b = vis ? int(std::ceil(cb * scale)) - y : 0;
        std::array<int, 9> now{x, y, w, h, l, t, r, b, vis ? 1 : 0};
        if (now == placed) return false;
        placed = now;
        visible = vis;
        SetWindowPos(window, HWND_TOP, x, y, w, h, SWP_NOACTIVATE | (vis ? SWP_SHOWWINDOW : SWP_HIDEWINDOW));
        if (vis) {
            bool whole = l <= 0 && t <= 0 && r >= w && b >= h;
            SetWindowRgn(window, whole ? nullptr : CreateRectRgn(l, t, r, b), TRUE);
        }
        if (controller) { RECT bounds{0, 0, w, h}; controller->put_Bounds(bounds); controller->put_IsVisible(vis); }
        Uncover(vis, l, t, r, b);
        return true;
    }

    HWND Island() const {
        RECT own{};
        GetWindowRect(window, &own);
        for (HWND child = GetWindow(owner->hwnd, GW_CHILD); child; child = GetWindow(child, GW_HWNDNEXT)) {
            RECT around{};
            if (child == window || !IsWindowVisible(child) || !GetWindowRect(child, &around)) continue;
            if (around.left <= own.left && around.top <= own.top && around.right >= own.right && around.bottom >= own.bottom) return child;
        }
        return nullptr;
    }

    void Uncover(bool vis, int l, int t, int r, int b) {
        if (!vis) {
            if (cut) SetWindowRgn(cut, nullptr, TRUE);
            cut = nullptr;
            return;
        }
        if (!cut) cut = Island();
        if (!cut) return;
        RECT island{}, own{};
        GetWindowRect(cut, &island);
        GetWindowRect(window, &own);
        int x = own.left - island.left, y = own.top - island.top;
        HRGN rest = CreateRectRgn(0, 0, 0x7FFF, 0x7FFF);
        HRGN place = CreateRectRgn(x + l, y + t, x + r, y + b);
        CombineRgn(rest, rest, place, RGN_DIFF);
        DeleteObject(place);
        SetWindowRgn(cut, rest, TRUE);
    }

    void Follow() {
        still = 0;
        if (renderToken.value) return;
        std::weak_ptr<WebPageView> weak = shared_from_this();
        renderToken = media::CompositionTarget::Rendering([weak](auto&&, auto&&) {
            auto view = weak.lock();
            if (!view) return;
            if (view->Layout()) view->still = 0;
            else if (++view->still > 30) view->Unfollow();
        });
    }

    void Unfollow() {
        if (!renderToken.value) return;
        media::CompositionTarget::Rendering(renderToken);
        renderToken = {};
    }

    void Start(Ctx const& ctx) {
        owner = ctx.win;
        if (!owner || !owner->hwnd) return;
        window = CreateWindowExW(WS_EX_NOACTIVATE, L"STATIC", L"", WS_CHILD | WS_CLIPSIBLINGS,
            0, 0, 1, 1, owner->hwnd, nullptr, GetModuleHandleW(nullptr), nullptr);
        std::weak_ptr<WebPageView> weak = shared_from_this();
        layoutToken = frame.LayoutUpdated([weak](auto&&, auto&&) {
            if (auto view = weak.lock(); view && !view->disposed) { view->Layout(); view->Follow(); }
        });
        auto result = CreateCoreWebView2EnvironmentWithOptions(nullptr, WebPageRuntime::Cache().c_str(), nullptr,
            Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
                [weak](HRESULT result, ICoreWebView2Environment* environment) -> HRESULT {
                    auto view = weak.lock();
                    if (!view || view->disposed) return S_OK;
                    if (FAILED(result) || !environment) { Report("WebPage requires Microsoft Edge WebView2 Runtime"); return S_OK; }
                    return environment->CreateCoreWebView2Controller(view->window,
                        Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>(
                            [weak](HRESULT result, ICoreWebView2Controller* controller) -> HRESULT {
                                auto view = weak.lock();
                                if (!view || view->disposed) { if (controller) controller->Close(); return S_OK; }
                                if (FAILED(result) || !controller) { Report("WebPage: could not create WebView2"); return S_OK; }
                                view->controller.copy_from(controller);
                                controller->get_CoreWebView2(view->web.put());
                                winrt::com_ptr<ICoreWebView2Settings> settings;
                                view->web->get_Settings(settings.put());
                                settings->put_AreDefaultContextMenusEnabled(FALSE);
                                settings->put_IsWebMessageEnabled(FALSE);
                                settings->put_AreDevToolsEnabled(FALSE);
                                view->OpenPage();
                                view->placed = {};
                                view->Layout();
                                return S_OK;
                            }).Get());
                }).Get());
        if (FAILED(result)) Report("WebPage requires Microsoft Edge WebView2 Runtime");
    }

    void OpenPage() {
        if (auto second = web.try_as<ICoreWebView2_2>()) {
            winrt::com_ptr<ICoreWebView2CookieManager> manager;
            second->get_CookieManager(manager.put());
            if (manager) {
                manager->DeleteAllCookies();
                for (auto const& [name, value] : cookies) {
                    winrt::com_ptr<ICoreWebView2Cookie> cookie;
                    if (SUCCEEDED(manager->CreateCookie(name.c_str(), value.c_str(), cookieDomain.c_str(), L"/", cookie.put())) && cookie) {
                        cookie->put_IsSecure(TRUE);
                        manager->AddOrUpdateCookie(cookie.get());
                    }
                }
            }
        }
        if (!userAgent.empty()) {
            winrt::com_ptr<ICoreWebView2Settings> settings;
            web->get_Settings(settings.put());
            if (auto agent = settings.try_as<ICoreWebView2Settings2>()) agent->put_UserAgent(userAgent.c_str());
        }
        EventRegistrationToken token{};
        if (!closeOn.empty()) {
            auto listener = "document.addEventListener('click',function(e){var t=e.target;if(t&&t.closest&&t.closest("
                + WebPageJsonString(closeOn)
                + ")){e.stopPropagation();e.preventDefault();location.href='bern-web-page://close'}},true);";
            web->AddScriptToExecuteOnDocumentCreated(winrt::to_hstring(listener).c_str(), nullptr);
        }
        web->add_NavigationStarting(Microsoft::WRL::Callback<ICoreWebView2NavigationStartingEventHandler>(
            [weak = weak_from_this()](ICoreWebView2*, ICoreWebView2NavigationStartingEventArgs* args) -> HRESULT {
                LPWSTR uri = nullptr;
                args->get_Uri(&uri);
                std::wstring_view address = uri ? uri : L"";
                bool isWeb = address.starts_with(L"http://") || address.starts_with(L"https://") || address.starts_with(L"about:");
                if (!isWeb) args->put_Cancel(TRUE);
                bool closing = address.starts_with(L"bern-web-page:");
                CoTaskMemFree(uri);
                if (auto view = weak.lock(); closing && view && view->close) view->close();
                return S_OK;
            }).Get(), &token);
        web->add_NewWindowRequested(Microsoft::WRL::Callback<ICoreWebView2NewWindowRequestedEventHandler>(
            [weak = weak_from_this()](ICoreWebView2*, ICoreWebView2NewWindowRequestedEventArgs* args) -> HRESULT {
                args->put_Handled(TRUE);
                LPWSTR uri = nullptr;
                args->get_Uri(&uri);
                if (auto view = weak.lock(); view && uri && view->web) view->web->Navigate(uri);
                CoTaskMemFree(uri);
                return S_OK;
            }).Get(), &token);
        web->Navigate(page.c_str());
    }

    void Close() {
        if (web && !page.empty()) {
            if (auto second = web.try_as<ICoreWebView2_2>()) {
                winrt::com_ptr<ICoreWebView2CookieManager> manager;
                second->get_CookieManager(manager.put());
                if (manager) manager->DeleteAllCookies();
            }
        }
        disposed = true;
        Uncover(false, 0, 0, 0, 0);
        frame.LayoutUpdated(layoutToken);
        Unfollow();
        if (controller) controller->Close();
        web = nullptr; controller = nullptr;
        if (window) { DestroyWindow(window); window = nullptr; }
    }
};

struct WebPageProps {
    Str url;
    Str cookiesFile;
    Str cookieDomain;
    Str userAgent;
    Str closeOn;
    Str label;
    Act action;
};

inline void WebPage(Parent const& p, Ctx const& ctx, WebPageProps props) {
    auto view = std::make_shared<WebPageView>();
    view->page = props.url ? std::wstring(props.url(ctx)) : std::wstring();
    view->cookieDomain = props.cookieDomain ? std::wstring(props.cookieDomain(ctx)) : std::wstring();
    view->userAgent = props.userAgent ? std::wstring(props.userAgent(ctx)) : std::wstring();
    view->closeOn = props.closeOn ? winrt::to_string(props.closeOn(ctx)) : std::string();
    view->close = Run(ctx, props.action);
    if (props.cookiesFile) {
        std::string bytes;
        HANDLE file = CreateFileW(std::wstring(props.cookiesFile(ctx)).c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, 0, nullptr);
        if (file != INVALID_HANDLE_VALUE) {
            bytes.resize(GetFileSize(file, nullptr));
            DWORD read = 0;
            ReadFile(file, bytes.data(), static_cast<DWORD>(bytes.size()), &read, nullptr);
            bytes.resize(read);
            CloseHandle(file);
        }
        std::wstring text(winrt::to_hstring(bytes));
        size_t at = 0;
        while (at < text.size()) {
            size_t end = (std::min)(text.find(L';', at), text.size());
            auto part = text.substr(at, end - at);
            at = end + 1;
            auto from = part.find_first_not_of(L" \t\r\n");
            auto equals = part.find(L'=');
            if (from == std::wstring::npos || equals == std::wstring::npos || equals <= from) continue;
            auto name = part.substr(from, equals - from);
            if (_wcsicmp(name.c_str(), L"path") == 0 || _wcsicmp(name.c_str(), L"domain") == 0
                || _wcsicmp(name.c_str(), L"expires") == 0 || _wcsicmp(name.c_str(), L"max-age") == 0) continue;
            auto value = part.substr(equals + 1);
            value.erase(value.find_last_not_of(L" \t\r\n") + 1);
            view->cookies.emplace_back(name, value);
        }
    }
    view->frame.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    view->frame.VerticalAlignment(xaml::VerticalAlignment::Stretch);
    MarkFill(view->frame);
    if (props.label) BindName(view->frame, Bound(ctx, props.label));
    Place(p, view->frame);
    std::weak_ptr<WebPageView> weak = view;
    view->frame.Loaded([weak, ctx](auto&&, auto&&) {
        if (auto v = weak.lock(); v && !v->disposed && !v->started) { v->started = true; v->Start(ctx); }
    });
    Reactive::OnCleanup([view] { view->Close(); });
}
#endif

}
