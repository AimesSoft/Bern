// Animated SVG documents. Edge WebView2 in both Windows front ends.
// This source is generated only for applications declaring Svg.

#include <shellapi.h>

namespace B {

namespace SvgRuntime {
inline std::string const prefix = /*bern:svg-prefix*/;
inline std::string const suffix = /*bern:svg-suffix*/;

/// WebView2's cache belongs in the user's temp directory, never beside an
/// application installed in Program Files.
inline std::wstring Cache() {
    wchar_t temp[MAX_PATH]{};
    GetTempPathW(MAX_PATH, temp);
    return std::wstring(temp) + L"BernSvgWebView2";
}

/// Before the first WebView2 of the process: transparent from the first
/// frame, so the document's own transparent page shows what is below it,
/// and the cache in the temp directory. Values the user set stay.
inline void Prepare() {
    static bool done = false;
    if (done) return;
    done = true;
    if (!GetEnvironmentVariableW(L"WEBVIEW2_DEFAULT_BACKGROUND_COLOR", nullptr, 0))
        SetEnvironmentVariableW(L"WEBVIEW2_DEFAULT_BACKGROUND_COLOR", L"00000000");
    if (!GetEnvironmentVariableW(L"WEBVIEW2_USER_DATA_FOLDER", nullptr, 0))
        SetEnvironmentVariableW(L"WEBVIEW2_USER_DATA_FOLDER", Cache().c_str());
}
}

inline std::string SvgJsonString(std::string const& value) {
    std::string out = "\"";
    constexpr char hex[] = "0123456789abcdef";
    for (unsigned char ch : value) {
        if (ch == '\\' || ch == '\"') { out += '\\'; out += char(ch); }
        else if (ch < 0x20) { out += "\\u00"; out += hex[ch >> 4]; out += hex[ch & 15]; }
        else out += char(ch);
    }
    return out + "\"";
}
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

struct SvgView : std::enable_shared_from_this<SvgView> {
    bool externalLinks = false;
    controls::Grid frame;
    std::string html, state;
    bool started = false, loaded = false, disposed = false, visible = false;
    /// The document's own navigation is under way: the only one allowed.
    bool opening = false;
    std::string Script() const {
        return "window.bernSvg?.update(Object.assign(" + state
            + ",{visible:" + (visible ? "true" : "false") + "}))";
    }
    void Visibility(bool value) {
        visible = value;
        Send();
    }
    // WebView2's Win32 presenter at the XAML frame's bounds, updated on
    // layout. XAML Islands has no WebView2 control, and WinUI 3's draws
    // the theme background under a transparent page, so both front ends
    // host the presenter: a child window above the XAML, cut to the
    // scrolling views around the frame.
    HWND window = nullptr;
    Win* owner = nullptr;
    winrt::com_ptr<ICoreWebView2Controller> controller;
    winrt::com_ptr<ICoreWebView2> web;
    winrt::event_token layoutToken{};
    void Send() {
        if (loaded && !disposed && web) web->ExecuteScript(winrt::to_hstring(Script()).c_str(), nullptr);
    }
    /// Where the presenter stands last: position, size, the visible part
    /// and whether it shows, in pixels of the window.
    std::array<int, 9> placed{};
    winrt::event_token renderToken{};
    int still = 0;

    /// Follow the frame as laid out and transformed (transitions move and
    /// scale it without a layout); true when the presenter moved.
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
                // The part inside every scrolling view around the frame.
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
        bool visible = shown && w > 0 && h > 0 && cr > cl && cb > ct;
        int l = visible ? int(std::floor(cl * scale)) - x : 0, t = visible ? int(std::floor(ct * scale)) - y : 0;
        int r = visible ? int(std::ceil(cr * scale)) - x : 0, b = visible ? int(std::ceil(cb * scale)) - y : 0;
        std::array<int, 9> now{x, y, w, h, l, t, r, b, visible ? 1 : 0};
        if (now == placed) return false;
        placed = now;
        if (visible != this->visible) Visibility(visible);
        SetWindowPos(window, HWND_TOP, x, y, w, h, SWP_NOACTIVATE | (visible ? SWP_SHOWWINDOW : SWP_HIDEWINDOW));
        if (visible) {
            bool whole = l <= 0 && t <= 0 && r >= w && b >= h;
            SetWindowRgn(window, whole ? nullptr : CreateRectRgn(l, t, r, b), TRUE);
        }
        if (controller) { RECT bounds{0, 0, w, h}; controller->put_Bounds(bounds); controller->put_IsVisible(visible); }
        return true;
    }
    /// Follow every frame for a while after a layout, until the frame has
    /// stood still for half a second.
    void Follow() {
        still = 0;
        if (renderToken.value) return;
        std::weak_ptr<SvgView> weak = shared_from_this();
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
        std::weak_ptr<SvgView> weak = shared_from_this();
        layoutToken = frame.LayoutUpdated([weak](auto&&, auto&&) {
            if (auto view = weak.lock(); view && !view->disposed) { view->Layout(); view->Follow(); }
        });
        SvgRuntime::Prepare();
        auto result = CreateCoreWebView2EnvironmentWithOptions(nullptr, SvgRuntime::Cache().c_str(), nullptr,
            Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
                [weak](HRESULT result, ICoreWebView2Environment* environment) -> HRESULT {
                    auto view = weak.lock();
                    if (!view || view->disposed) return S_OK;
                    if (FAILED(result) || !environment) { Report("SVG requires Microsoft Edge WebView2 Runtime"); return S_OK; }
                    return environment->CreateCoreWebView2Controller(view->window,
                        Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>(
                            [weak](HRESULT result, ICoreWebView2Controller* controller) -> HRESULT {
                                auto view = weak.lock();
                                if (!view || view->disposed) { if (controller) controller->Close(); return S_OK; }
                                if (FAILED(result) || !controller) { Report("SVG: could not create WebView2"); return S_OK; }
                                view->controller.copy_from(controller);
                                controller->get_CoreWebView2(view->web.put());
                                if (auto transparent = view->controller.try_as<ICoreWebView2Controller2>())
                                    transparent->put_DefaultBackgroundColor(COREWEBVIEW2_COLOR{0, 0, 0, 0});
                                winrt::com_ptr<ICoreWebView2Settings> settings;
                                view->web->get_Settings(settings.put());
                                settings->put_AreDefaultContextMenusEnabled(FALSE);
                                settings->put_IsWebMessageEnabled(FALSE);
                                settings->put_AreDevToolsEnabled(FALSE);
                                EventRegistrationToken token{};
                                view->web->add_NavigationStarting(Microsoft::WRL::Callback<ICoreWebView2NavigationStartingEventHandler>(
                                    [weak](ICoreWebView2*, ICoreWebView2NavigationStartingEventArgs* args) -> HRESULT {
                                        auto view = weak.lock();
                                        bool allowed = view && view->opening;
                                        if (view) view->opening = false;
                                        BOOL user = FALSE; args->get_IsUserInitiated(&user);
                                        if (view && view->externalLinks && user && !allowed) {
                                            LPWSTR uri = nullptr; args->get_Uri(&uri);
                                            if (uri && wcsncmp(uri, L"https://", 8) == 0 && !wcspbrk(uri, L" \t\r\n"))
                                                ShellExecuteW(nullptr, L"open", uri, nullptr, nullptr, SW_SHOWNORMAL);
                                            CoTaskMemFree(uri);
                                        }
                                        args->put_Cancel(!allowed); return S_OK;
                                    }).Get(), &token);
                                view->web->add_NewWindowRequested(Microsoft::WRL::Callback<ICoreWebView2NewWindowRequestedEventHandler>(
                                    [](ICoreWebView2*, ICoreWebView2NewWindowRequestedEventArgs* args) -> HRESULT { args->put_Handled(TRUE); return S_OK; }).Get(), &token);
                                view->web->add_NavigationCompleted(Microsoft::WRL::Callback<ICoreWebView2NavigationCompletedEventHandler>(
                                    [weak](ICoreWebView2*, ICoreWebView2NavigationCompletedEventArgs* args) -> HRESULT {
                                        BOOL success = FALSE; args->get_IsSuccess(&success);
                                        auto view = weak.lock();
                                        if (!view || view->loaded) return S_OK;
                                        if (success) { view->loaded = true; view->Send(); return S_OK; }
                                        COREWEBVIEW2_WEB_ERROR_STATUS status{}; args->get_WebErrorStatus(&status);
                                        Report("SVG: the document did not load (" + std::to_string(static_cast<int>(status)) + ")");
                                        return S_OK;
                                    }).Get(), &token);
                                // The controller takes the bounds the frame already has.
                                view->placed = {};
                                view->Layout();
                                view->opening = true;
                                return view->web->NavigateToString(winrt::to_hstring(view->html).c_str());
                            }).Get());
                }).Get());
        if (FAILED(result)) Report("SVG requires Microsoft Edge WebView2 Runtime");
    }
    void Close() {
        disposed = true;
        frame.LayoutUpdated(layoutToken);
        Unfollow();
        if (controller) controller->Close();
        web = nullptr; controller = nullptr;
        if (window) { DestroyWindow(window); window = nullptr; }
    }
};

inline void Svg(Parent const& p, Ctx const& ctx, SvgProps props) {
    auto view = std::make_shared<SvgView>();
    view->externalLinks = props.externalLinks;
    auto bytes = ResourceBytes(props.source);
    view->html = SvgRuntime::prefix + std::string(bytes.begin(), bytes.end()) + SvgRuntime::suffix;
    view->frame.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    view->frame.VerticalAlignment(xaml::VerticalAlignment::Top);
    BindLen(ctx, props.width, nullptr, [view](double n) { view->frame.Width((std::max)(0.0, n)); });
    BindLen(ctx, props.height, nullptr, [view](double n) { view->frame.Height((std::max)(0.0, n)); });
    if (props.label) BindName(view->frame, Bound(ctx, props.label));
    else xaml::Automation::AutomationProperties::SetAccessibilityView(view->frame, xaml::Automation::Peers::AccessibilityView::Raw);
    Reactive::Run([view, ctx, props] {
        bool playing = !props.playing || props.playing(ctx) == Value::Bool(true);
        double speed = props.speed ? R::AsDouble(props.speed(ctx)) : 1;
        double restart = props.restart ? R::AsDouble(props.restart(ctx)) : 0;
        if (!std::isfinite(speed) || speed <= 0) speed = 1;
        if (!std::isfinite(restart)) restart = 0;
        view->state = std::string("{\"playing\":") + (playing ? "true" : "false")
            + ",\"speed\":" + R::Number(speed) + ",\"restart\":" + R::Number(restart)
            + ",\"externalLinks\":" + (props.externalLinks ? "true" : "false")
            + ",\"data\":" + SvgJsonString(props.data ? props.data(ctx) : "")
            + ",\"interactive\":" + (props.interactive ? "true" : "false")
            + ",\"fit\":\"" + (props.fit ? props.fit : "contain") + "\"}";
        view->Send();
    });
    Place(p, view->frame);
    std::weak_ptr<SvgView> weak = view;
    view->frame.Loaded([weak, ctx](auto&&, auto&&) {
        if (auto v = weak.lock(); v && !v->disposed) {
            v->Visibility(true);
            if (!v->started) { v->started = true; v->Start(ctx); }
        }
    });
    view->frame.Unloaded([weak](auto&&, auto&&) { if (auto v = weak.lock()) v->Visibility(false); });
    Reactive::OnCleanup([view] { view->Close(); });
}

}
