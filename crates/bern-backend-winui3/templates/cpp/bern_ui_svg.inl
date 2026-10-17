// Animated SVG documents. Edge WebView2 in both Windows front ends.
// This source is generated only for applications declaring Svg.
namespace SvgRuntime {
inline std::string const prefix = /*bern:svg-prefix*/;
inline std::string const suffix = /*bern:svg-suffix*/;
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
    bool interactive = true;
};

struct SvgView : std::enable_shared_from_this<SvgView> {
    controls::Grid frame;
    std::string html, state;
    bool started = false, loaded = false, disposed = false, visible = false;
    std::string Script() const {
        return "window.bernSvg?.update(Object.assign(" + state
            + ",{visible:" + (visible ? "true" : "false") + "}))";
    }
    void Visibility(bool value) {
        visible = value;
        Send();
    }
#if BERN_WINUI
    controls::WebView2 web;
    void Send() {
        if (loaded && !disposed) web.CoreWebView2().ExecuteScriptAsync(winrt::to_hstring(Script()));
    }
    static winrt::fire_and_forget Open(std::shared_ptr<SvgView> self) {
        try {
            co_await self->web.EnsureCoreWebView2Async();
            if (self->disposed) co_return;
            auto core = self->web.CoreWebView2();
            core.Settings().AreDefaultContextMenusEnabled(false);
            core.Settings().IsWebMessageEnabled(false);
            core.Settings().AreDevToolsEnabled(false);
            std::weak_ptr<SvgView> weak = self;
            core.NavigationStarting([weak](auto&&, auto&& args) {
                auto view = weak.lock();
                if (!view || view->loaded || args.Uri() != L"about:blank") args.Cancel(true);
            });
            core.NewWindowRequested([](auto&&, auto&& args) { args.Handled(true); });
            core.NavigationCompleted([weak](auto&&, auto&& args) {
                if (auto view = weak.lock(); view && args.IsSuccess()) { view->loaded = true; view->Send(); }
            });
            self->web.DefaultBackgroundColor(winrt::Windows::UI::Color{0, 0, 0, 0});
            core.NavigateToString(winrt::to_hstring(self->html));
        } catch (winrt::hresult_error const& error) {
            Report("SVG: " + winrt::to_string(error.message()));
        }
    }
    void Start(Ctx const&) { frame.Children().Append(web); Open(shared_from_this()); }
    void Close() { disposed = true; web.Close(); }
#else
    // XAML Islands has no XAML WebView2 control. Host its Win32 presenter
    // at the XAML frame's bounds, updating position and DPI on layout.
    // As a child HWND this cannot follow arbitrary XAML ancestor clipping.
    HWND window = nullptr;
    Win* owner = nullptr;
    winrt::com_ptr<ICoreWebView2Controller> controller;
    winrt::com_ptr<ICoreWebView2> web;
    winrt::event_token layoutToken{};
    void Send() {
        if (loaded && !disposed && web) web->ExecuteScript(winrt::to_hstring(Script()).c_str(), nullptr);
    }
    void Layout() {
        if (!window || !owner || !owner->root) return;
        auto point = frame.TransformToVisual(owner->root).TransformPoint({0, 0});
        auto scale = frame.XamlRoot() ? frame.XamlRoot().RasterizationScale() : 1.0;
        int x = int(std::round(point.X * scale)), y = int(std::round(point.Y * scale));
        int w = int(std::round(frame.ActualWidth() * scale)), h = int(std::round(frame.ActualHeight() * scale));
        bool visible = frame.IsLoaded() && frame.Visibility() == xaml::Visibility::Visible && w > 0 && h > 0;
        if (visible != this->visible) Visibility(visible);
        SetWindowPos(window, HWND_TOP, x, y, w, h, SWP_NOACTIVATE | (visible ? SWP_SHOWWINDOW : SWP_HIDEWINDOW));
        if (controller) { RECT bounds{0, 0, w, h}; controller->put_Bounds(bounds); controller->put_IsVisible(visible); }
    }
    void Start(Ctx const& ctx) {
        owner = ctx.win;
        if (!owner || !owner->hwnd) return;
        window = CreateWindowExW(WS_EX_NOACTIVATE, L"STATIC", L"", WS_CHILD | WS_CLIPSIBLINGS,
            0, 0, 1, 1, owner->hwnd, nullptr, GetModuleHandleW(nullptr), nullptr);
        std::weak_ptr<SvgView> weak = shared_from_this();
        layoutToken = frame.LayoutUpdated([weak](auto&&, auto&&) { if (auto view = weak.lock()) view->Layout(); });
        // WebView2's cache belongs in the user's temp directory, never
        // beside an application installed in Program Files.
        wchar_t temp[MAX_PATH]{}; GetTempPathW(MAX_PATH, temp);
        std::wstring cache = std::wstring(temp) + L"BernSvgWebView2";
        auto result = CreateCoreWebView2EnvironmentWithOptions(nullptr, cache.c_str(), nullptr,
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
                                        auto view = weak.lock(); LPWSTR uri = nullptr; args->get_Uri(&uri);
                                        bool allowed = view && !view->loaded && uri && std::wstring_view(uri) == L"about:blank";
                                        CoTaskMemFree(uri); args->put_Cancel(!allowed); return S_OK;
                                    }).Get(), &token);
                                view->web->add_NewWindowRequested(Microsoft::WRL::Callback<ICoreWebView2NewWindowRequestedEventHandler>(
                                    [](ICoreWebView2*, ICoreWebView2NewWindowRequestedEventArgs* args) -> HRESULT { args->put_Handled(TRUE); return S_OK; }).Get(), &token);
                                view->web->add_NavigationCompleted(Microsoft::WRL::Callback<ICoreWebView2NavigationCompletedEventHandler>(
                                    [weak](ICoreWebView2*, ICoreWebView2NavigationCompletedEventArgs* args) -> HRESULT {
                                        BOOL success = FALSE; args->get_IsSuccess(&success);
                                        if (auto view = weak.lock(); view && success) { view->loaded = true; view->Send(); }
                                        return S_OK;
                                    }).Get(), &token);
                                view->Layout();
                                return view->web->NavigateToString(winrt::to_hstring(view->html).c_str());
                            }).Get());
                }).Get());
        if (FAILED(result)) Report("SVG requires Microsoft Edge WebView2 Runtime");
    }
    void Close() {
        disposed = true;
        frame.LayoutUpdated(layoutToken);
        if (controller) controller->Close();
        web = nullptr; controller = nullptr;
        if (window) { DestroyWindow(window); window = nullptr; }
    }
#endif
};

inline void Svg(Parent const& p, Ctx const& ctx, SvgProps props) {
    auto view = std::make_shared<SvgView>();
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
