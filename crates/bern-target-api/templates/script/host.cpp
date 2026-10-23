
namespace bern::Scripts {
static winrt::com_ptr<ICoreWebView2Controller> controller;
static winrt::com_ptr<ICoreWebView2> view;
static HWND parent = nullptr;
static uint64_t generation = 0;
static bool ready = false;
static std::vector<std::string> pending;
static void Open(uint64_t epoch);
void Stop() {
    ++generation; ready = false; pending.clear();
    if (controller) controller->Close();
    controller = nullptr; view = nullptr;
    if (parent) DestroyWindow(parent);
    parent = nullptr; bern_script_stop();
}
static void Pump(uint64_t epoch) {
    if (epoch != generation) return;
    auto batch = Host::Take(bern_script_poll(static_cast<double>(GetTickCount64())));
    if (!batch.empty()) pending.emplace_back(batch.begin(), batch.end());
    if (!parent && !pending.empty()) Open(epoch);
    if (ready && view) {
        auto batches = std::move(pending); pending.clear();
        for (auto const& queued : batches) {
            auto script = "bernScriptDispatch(" + queued + ")";
            if (FAILED(view->ExecuteScript(winrt::to_hstring(script).c_str(), nullptr))) { Stop(); return; }
        }
    }
    Host::After(std::chrono::milliseconds(16), [epoch] { Pump(epoch); });
}
static void Open(uint64_t epoch) {
    parent = CreateWindowExW(0, L"STATIC", L"Bern JavaScript", WS_POPUP, 0, 0, 1, 1, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
    if (!parent) { Stop(); return; }
    wchar_t temp[MAX_PATH]{}; GetTempPathW(MAX_PATH, temp);
    auto cache = std::wstring(temp) + L"BernScriptWebView2";
    auto result = CreateCoreWebView2EnvironmentWithOptions(nullptr, cache.c_str(), nullptr,
        Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>([epoch](HRESULT hr, ICoreWebView2Environment* env) -> HRESULT {
            if (epoch != generation) return S_OK;
            if (FAILED(hr) || !env) { Stop(); return S_OK; }
            auto result = env->CreateCoreWebView2Controller(parent, Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>([epoch](HRESULT hr, ICoreWebView2Controller* created) -> HRESULT {
                if (epoch != generation) { if (created) created->Close(); return S_OK; }
                if (FAILED(hr) || !created) { Stop(); return S_OK; }
                controller.copy_from(created); controller->get_CoreWebView2(view.put());
                if (!view) { Stop(); return S_OK; }
                controller->put_IsVisible(FALSE);
                EventRegistrationToken token{};
                view->add_WebMessageReceived(Microsoft::WRL::Callback<ICoreWebView2WebMessageReceivedEventHandler>([](ICoreWebView2*, ICoreWebView2WebMessageReceivedEventArgs* args) -> HRESULT {
                    LPWSTR message = nullptr;
                    if (SUCCEEDED(args->TryGetWebMessageAsString(&message)) && message) {
                        auto json = winrt::to_string(message); CoTaskMemFree(message);
                        bern_script_complete(reinterpret_cast<uint8_t const*>(json.data()), json.size());
                    }
                    return S_OK;
                }).Get(), &token);
                view->add_NavigationCompleted(Microsoft::WRL::Callback<ICoreWebView2NavigationCompletedEventHandler>([](ICoreWebView2*, ICoreWebView2NavigationCompletedEventArgs* args) -> HRESULT {
                    BOOL success = FALSE; args->get_IsSuccess(&success);
                    if (success) ready = true; else Stop();
                    return S_OK;
                }).Get(), &token);
                view->add_ProcessFailed(Microsoft::WRL::Callback<ICoreWebView2ProcessFailedEventHandler>([](ICoreWebView2*, ICoreWebView2ProcessFailedEventArgs*) -> HRESULT { Stop(); return S_OK; }).Get(), &token);
                if (FAILED(view->NavigateToString(winrt::to_hstring(/*bern:script-html*/).c_str()))) Stop();
                return S_OK;
            }).Get());
            if (FAILED(result)) Stop();
            return S_OK;
        }).Get());
    if (FAILED(result)) { Stop(); return; }
}
void Start() {
    Stop(); bern_script_start(); Pump(generation);
}
}
