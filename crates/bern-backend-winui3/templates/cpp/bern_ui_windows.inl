// Bern runtime for Windows, XAML part 7 of 7: native windows, their drafts,
// the close protocol, presentation facts, session commands of external
// entries, development snapshots and the application's lifetime. Window
// facts and commands go through Win32 in both front ends; WinUI 3 windows
// are xaml::Window with Mica and a XAML title bar, system XAML windows are
// Win32 windows hosting an island. Generated build input; do not edit.

using B::AutoLength;
using B::Column;
using B::Header;
using B::Star;

/// A failure raised inside the runtime, with a session catalog error code.
struct Fault {
    std::string code;
    std::string message;
};

/// Outcome of a close request or decision, as the session catalog's
/// `CloseResult` describes it.
struct CloseResult {
    static constexpr uint32_t Closed = 0, Rejected = 1, ConfirmationRequired = 2;
    uint32_t outcome = Closed;
    std::string window;
    std::optional<std::string> confirmation;
    uint64_t draftRevision = 0;
    std::vector<uint32_t> decisions;
    std::optional<std::string> message;
    std::optional<std::string> reason;

    Value ToValue() const {
        auto text = [](std::optional<std::string> const& value) { return value ? Value::Some(Value::Text(*value)) : Value::None(); };
        Values choices;
        for (auto decision : decisions) choices.push_back(Value::Enum(decision));
        return Value::Record({Value::Enum(outcome), Value::Text(window), text(confirmation), Value::Int(static_cast<int64_t>(draftRevision)),
            Value::List(std::move(choices)), text(message), text(reason)});
    }
};

inline Cell<Value>& Win::CellOf(Field field) {
    auto found = cells.find(field.session);
    if (found != cells.end() && field.field < static_cast<int>(found->second.size())) return found->second[field.field];
    static Cell<Value> none;
    return none;
}

/// Write a draft field: the session runtime first, then the cell.
inline bool Win::Write(Field field, Value const& written) {
    auto const& session = app->catalog.sessions[field.session];
    auto const& type = session.fields[field.field].second;
    // Literals of actions such as `draft.size.set(12)` may be integers.
    Value value = written;
    if (type.kind == BType::Kind::Float && value.kind == Value::Kind::Int) value = Value::Float(static_cast<double>(value.i));
    else if (type.kind == BType::Kind::Int && value.kind == Value::Kind::Float) value = Value::Int(static_cast<int64_t>(std::llround(value.f)));
    auto bytes = Codec::ToBytes(value, type, app->catalog.types);
    auto reply = Host::Take(bern_session_set(Host::handle, static_cast<uint32_t>(kind), generation, static_cast<uint32_t>(field.session),
        static_cast<uint32_t>(field.field), bytes.data(), bytes.size()));
    Reader reader(reply);
    if (auto failure = Host::Status(reply, reader)) {
        bern::Report(failure->code + ": " + failure->message);
        return false;
    }
    CellOf(field).Set(value);
    Snapshot::Schedule();
    return true;
}

/// Show a failed command to the user.
inline void Win::Fail(Failure const& failure) {
    if (!failures) return;
#if BERN_WINUI
    auto info = failures.as<controls::InfoBar>();
    info.Title(bern::Text::Tr("bern.alert.failed"));
    info.Message(Wide(failure.message));
    info.IsOpen(true);
#else
    failureText.Text(bern::Text::Tr("bern.alert.failed") + L": " + Wide(failure.message));
    failures.Visibility(xaml::Visibility::Visible);
#endif
    auto shown = ++failuresShown;
    auto bar = failures;
    auto counter = &failuresShown;
    auto alive = std::weak_ptr<bool>(life);
    Host::After(std::chrono::seconds(6), [shown, bar, counter, alive] {
        if (alive.expired() || shown != *counter) return;
#if BERN_WINUI
        bar.as<controls::InfoBar>().IsOpen(false);
#else
        bar.Visibility(xaml::Visibility::Collapsed);
#endif
    });
}

inline double Win::Scale() const {
    auto dpi = hwnd ? GetDpiForWindow(hwnd) : 0;
    return (dpi == 0 ? 96 : dpi) / 96.0;
}

inline Facts Win::CurrentFacts() {
    if (owner) {
        // A dialog window over its owner: where its dialog shows, and only
        // activate and close.
        Facts facts;
        facts.actions = (1u << 0) | (1u << 1);
        facts.focused = owner->focused;
        RECT rect{};
        GetWindowRect(owner->hwnd, &rect);
        auto scale = Scale();
        facts.x = rect.left / scale;
        facts.y = rect.top / scale;
        if (root) {
            facts.width = root.ActualWidth();
            facts.height = root.ActualHeight();
            try {
                if (owner->root) {
                    auto at = root.TransformToVisual(owner->root).TransformPoint({0, 0});
                    facts.x += at.X;
                    facts.y += at.Y;
                }
            } catch (winrt::hresult_error const&) {
            }
        }
        return facts;
    }
    LONG_PTR style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    uint8_t visibility = !IsWindowVisible(hwnd) ? 2 : IsIconic(hwnd) ? 1 : 0;
    uint8_t size = IsZoomed(hwnd) ? 1 : 0;
    auto scale = Scale();
    RECT rect{};
    GetWindowRect(hwnd, &rect);
    uint32_t actions = (1u << 0) | (1u << 1); // activate, close
    bool resizable = (style & WS_THICKFRAME) != 0;
    if (visibility == 0 && (style & WS_MINIMIZEBOX)) actions |= 1u << 2;
    if (visibility == 1 || size != 0) actions |= 1u << 3;
    if (resizable && size == 0 && visibility == 0 && (style & WS_MAXIMIZEBOX)) actions |= 1u << 4;
    if (size == 0 && visibility == 0) actions |= 1u << 5;
    if (resizable && size == 0 && visibility == 0) actions |= 1u << 6;
    std::array<double, 4> frame{rect.left / scale, rect.top / scale, (rect.right - rect.left) / scale, (rect.bottom - rect.top) / scale};
    // A minimized window is parked off screen; its frame is the one it comes
    // back to.
    if (visibility == 1 && lastFrame) frame = *lastFrame;
    else if (visibility == 0) lastFrame = frame;
    return Facts{visibility, size, frame[0], frame[1], frame[2], frame[3], focused, actions};
}

/// Report the current facts to the session runtime, then finish the
/// presentation commands they satisfy.
inline void Win::Report() {
    if (closed || !Host::handle || !hwnd) return;
    fullscreen.Set(window.AppWindow().Presenter().Kind() == winrt::Microsoft::UI::Windowing::AppWindowPresenterKind::FullScreen);
    auto facts = CurrentFacts();
    bern_window_facts(Host::handle, static_cast<uint32_t>(kind), generation, command ? 0 : 1, facts.visibility, facts.size, facts.x, facts.y,
        facts.width, facts.height, facts.focused ? 1 : 0, facts.actions);
    decltype(waits) done;
    std::erase_if(waits, [&](auto& wait) {
        if (!wait.first(facts)) return false;
        done.push_back(std::move(wait));
        return true;
    });
    for (auto& wait : done) wait.second(std::nullopt);
    if (waits.empty()) command = false;
}

inline void Win::ScheduleReport() {
    if (reportQueued) return;
    reportQueued = true;
    auto alive = std::weak_ptr<bool>(life);
    Host::Post([this, alive] {
        if (alive.expired()) return;
        reportQueued = false;
        Report();
    });
}

/// Perform a presentation change and finish once the facts show it, or with
/// `ui_timeout` after five seconds.
inline void Win::Perform(std::function<void()> change, std::function<bool(Facts const&)> satisfied, std::function<void(std::optional<Failure> const&)> complete) {
    command = true;
    waits.emplace_back(std::move(satisfied), std::move(complete));
    change();
    ScheduleReport();
    auto alive = std::weak_ptr<bool>(life);
    Host::After(std::chrono::seconds(5), [this, alive] {
        if (alive.expired()) return;
        Report();
        auto expired = std::move(waits);
        waits.clear();
        for (auto& wait : expired)
            wait.second(Failure{"ui_timeout", "the window system did not report the change in time; the outcome is unknown"});
        command = false;
    });
}

namespace WindowManager {

inline std::vector<std::shared_ptr<Win>> all;

inline std::shared_ptr<Win> Find(Win const* win) {
    for (auto const& candidate : all)
        if (candidate.get() == win) return candidate;
    return nullptr;
}

inline std::shared_ptr<Win> Named(std::string const& id) {
    for (auto const& win : all)
        if (win->id == id) return win;
    throw Fault{"target_not_found", "no open window `" + id + "`"};
}

inline std::shared_ptr<Win> OfHwnd(HWND hwnd) {
    for (auto const& win : all)
        if (win->hwnd == hwnd) return win;
    return nullptr;
}

void Show(Win& win);
void RequestCloseSafely(std::shared_ptr<Win> const& win);
void Ended(std::shared_ptr<Win> const& win);

/// The window messages both front ends handle the same way. True when the
/// message is handled and `result` holds its result.
inline bool Handle(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam, LRESULT& result) {
    auto win = OfHwnd(hwnd);
    if (!win) return false;
    switch (message) {
    case WM_CLOSE:
        if (win->allowClose) return false;
        RequestCloseSafely(win);
        result = 0;
        return true;
    case WM_GETMINMAXINFO: {
        auto const& def = win->Def();
        auto info = reinterpret_cast<MINMAXINFO*>(lparam);
        auto scale = win->Scale();
        if (def.minWidth) info->ptMinTrackSize.x = (std::max)(info->ptMinTrackSize.x, static_cast<LONG>(std::lround(*def.minWidth * scale)));
        if (def.minHeight) info->ptMinTrackSize.y = (std::max)(info->ptMinTrackSize.y, static_cast<LONG>(std::lround(*def.minHeight * scale)));
        return false;
    }
    case WM_ACTIVATE:
        win->focused = LOWORD(wparam) != WA_INACTIVE;
        win->ScheduleReport();
        if (win->focused) Snapshot::Schedule();
        return false;
    case WM_SIZE:
    case WM_MOVE:
    case WM_SHOWWINDOW:
    case WM_WINDOWPOSCHANGED:
        win->ScheduleReport();
        return false;
    case WM_DESTROY:
        Ended(win);
        return false;
    }
    return false;
}

#if BERN_WINUI
/// The procedures of WinUI windows before Bern subclassed them.
inline std::unordered_map<HWND, WNDPROC> originals;

inline LRESULT CALLBACK Subclass(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
    auto found = originals.find(hwnd);
    WNDPROC original = found != originals.end() ? found->second : reinterpret_cast<WNDPROC>(GetClassLongPtrW(hwnd, GCLP_WNDPROC));
    LRESULT result = 0;
    if (message == WM_GETMINMAXINFO) {
        // The window's own limits first, then the declared minimum size.
        result = CallWindowProcW(original, hwnd, message, wparam, lparam);
        Handle(hwnd, message, wparam, lparam, result);
        return result;
    }
    if (message == WM_NCDESTROY) originals.erase(hwnd);
    if (Handle(hwnd, message, wparam, lparam, result)) return result;
    return CallWindowProcW(original, hwnd, message, wparam, lparam);
}
#else
inline wchar_t const* const kWindowClass = L"BernWindow";

inline void UpdateFrameTheme(Win const& win) {
    if (!win.hwnd || !win.root) return;
    BOOL dark = win.root.ActualTheme() == xaml::ElementTheme::Dark;
    // DWMWA_USE_IMMERSIVE_DARK_MODE: 20 from Windows 10 20H1, 19 before.
    if (FAILED(DwmSetWindowAttribute(win.hwnd, 20, &dark, sizeof dark))) DwmSetWindowAttribute(win.hwnd, 19, &dark, sizeof dark);
}

inline LRESULT CALLBACK WindowProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
    LRESULT result = 0;
    auto win = OfHwnd(hwnd);
    if (win) {
        switch (message) {
        case WM_SIZE:
            if (win->island) SetWindowPos(win->island, nullptr, 0, 0, LOWORD(lparam), HIWORD(lparam), SWP_SHOWWINDOW | SWP_NOZORDER | SWP_NOACTIVATE);
            break;
        case WM_SETFOCUS:
            if (win->island) SetFocus(win->island);
            return 0;
        case WM_SETTINGCHANGE:
        case WM_THEMECHANGED:
            UpdateFrameTheme(*win);
            break;
        }
    }
    if (Handle(hwnd, message, wparam, lparam, result)) return result;
    return DefWindowProcW(hwnd, message, wparam, lparam);
}

inline void RegisterClassOnce() {
    static bool registered = false;
    if (registered) return;
    registered = true;
    WNDCLASSEXW type{sizeof(type)};
    type.lpfnWndProc = WindowProc;
    type.hInstance = GetModuleHandleW(nullptr);
    type.lpszClassName = kWindowClass;
    type.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    type.hIcon = LoadIconW(type.hInstance, MAKEINTRESOURCEW(1));
    type.hIconSm = type.hIcon;
    RegisterClassExW(&type);
}
#endif

struct Instance {
    uint64_t generation = 0;
    std::string id;
    std::unordered_map<int, std::vector<Cell<Value>>> cells;
};

inline Instance DecodeInstance(Bytes const& bytes) {
    Reader reader(bytes);
    if (auto failure = Host::Status(bytes, reader)) throw Fault{failure->code, failure->message};
    Instance instance;
    instance.generation = reader.U64();
    instance.id = reader.Text();
    auto slots = reader.U32();
    for (uint32_t i = 0; i < slots; ++i) {
        auto session = static_cast<int>(reader.U32());
        auto const& def = app->catalog.sessions[session];
        std::vector<Cell<Value>> fields;
        for (auto const& field : def.fields) fields.emplace_back(Codec::Decode(field.second, app->catalog.types, reader));
        instance.cells[session] = std::move(fields);
    }
    return instance;
}

/// Set the outer size of a window, in device-independent pixels.
inline void SetOuterSize(Win& win, double width, double height) {
    auto scale = win.Scale();
    int w = static_cast<int>(std::lround(width * scale)), h = static_cast<int>(std::lround(height * scale));
#if BERN_WINUI
    // The window's own frame logic knows its extended title bar.
    win.window.AppWindow().Resize(winrt::Windows::Graphics::SizeInt32{w, h});
#else
    SetWindowPos(win.hwnd, nullptr, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
#endif
}

/// Move a window, in device-independent pixels.
inline void SetPosition(Win& win, double x, double y) {
    auto scale = win.Scale();
    int px = static_cast<int>(std::lround(x * scale)), py = static_cast<int>(std::lround(y * scale));
#if BERN_WINUI
    win.window.AppWindow().Move(winrt::Windows::Graphics::PointInt32{px, py});
#else
    SetWindowPos(win.hwnd, nullptr, px, py, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
#endif
}

/// The application icon (resource 1) for the title bar, the taskbar and
/// Alt+Tab, at the sizes of the window's display.
inline void SetWindowIcons(HWND hwnd) {
    auto instance = GetModuleHandleW(nullptr);
    auto dpi = GetDpiForWindow(hwnd);
    auto load = [&](int metric) {
        int side = GetSystemMetricsForDpi(metric, dpi ? dpi : 96);
        return static_cast<HICON>(LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON, side, side, LR_SHARED));
    };
    if (auto large = load(SM_CXICON)) SendMessageW(hwnd, WM_SETICON, ICON_BIG, reinterpret_cast<LPARAM>(large));
    if (auto compact = load(SM_CXSMICON)) SendMessageW(hwnd, WM_SETICON, ICON_SMALL, reinterpret_cast<LPARAM>(compact));
}

#if BERN_WINUI
/// The title bar keeps the room of the system menu and the caption buttons
/// in the first and last columns of its template. WinUI sizes them with the
/// insets in pixels as if they were units, which at other scales than 100 %
/// moves what stands in the title bar away from the caption buttons; they
/// are set again in units after each layout that changes them. `fitted` is
/// what they were set to, and at which scale: every layout of the window
/// comes here, a resizing column every frame, and only when the title bar
/// has set its own widths or the scale has changed are the insets asked
/// for again.
inline void FitTitleBarInsets(controls::TitleBar const& titleBar, xaml::Window const& window, std::array<double, 3>& fitted) {
    if (!titleBar.XamlRoot() || media::VisualTreeHelper::GetChildrenCount(titleBar) == 0) return;
    auto grid = media::VisualTreeHelper::GetChild(titleBar, 0).try_as<controls::Grid>();
    if (!grid) return;
    auto columns = grid.ColumnDefinitions();
    uint32_t count = columns.Size();
    if (count < 2) return;
    double scale = titleBar.XamlRoot().RasterizationScale();
    if (scale <= 0) return;
    auto first = columns.GetAt(0), last = columns.GetAt(count - 1);
    if (first.Width().Value == fitted[0] && last.Width().Value == fitted[1] && scale == fitted[2]) return;
    auto bar = window.AppWindow().TitleBar();
    auto fit = [](controls::ColumnDefinition const& column, double width) {
        auto current = column.Width();
        if (current.GridUnitType == xaml::GridUnitType::Pixel && std::abs(current.Value - width) > 0.5)
            column.Width(xaml::GridLength{width, xaml::GridUnitType::Pixel});
    };
    fit(first, bar.LeftInset() / scale);
    fit(last, bar.RightInset() / scale);
    fitted = {first.Width().Value, last.Width().Value, scale};
}

/// The application icon in the XAML title bar: the image of icon resource 1
/// closest to the title bar's size. A PNG image is decoded as it is; a
/// bitmap image is wrapped in a one-image `.ico`, which XAML also decodes.
inline winrt::fire_and_forget ShowTitleIcon(controls::TitleBar titleBar, int side) {
    auto instance = GetModuleHandleW(nullptr);
    HRSRC group = FindResourceW(instance, MAKEINTRESOURCEW(1), RT_GROUP_ICON);
    if (!group) co_return;
    auto directory = static_cast<PBYTE>(LockResource(LoadResource(instance, group)));
    int id = directory ? LookupIconIdFromDirectoryEx(directory, TRUE, side, side, LR_DEFAULTCOLOR) : 0;
    HRSRC entry = id ? FindResourceW(instance, MAKEINTRESOURCEW(id), RT_ICON) : nullptr;
    if (!entry) co_return;
    auto data = static_cast<uint8_t const*>(LockResource(LoadResource(instance, entry)));
    auto size = SizeofResource(instance, entry);
    if (!data || size == 0) co_return;
    std::vector<uint8_t> bytes;
    static uint8_t const png[] = {0x89, 'P', 'N', 'G'};
    if (size >= 4 && std::equal(png, png + 4, data)) {
        bytes.assign(data, data + size);
    } else {
        // The group's entry for the image (14 bytes: width, height, colours,
        // reserved, planes, bit count, length, resource id) gives the
        // `.ico` entry its first 8 bytes.
        int count = directory[4] | (directory[5] << 8);
        PBYTE found = nullptr;
        for (int i = 0; i < count && !found; ++i) {
            PBYTE one = directory + 6 + 14 * i;
            if ((one[12] | (one[13] << 8)) == id) found = one;
        }
        if (!found) co_return;
        auto put = [&bytes](uint32_t value, int length) {
            for (int k = 0; k < length; ++k) bytes.push_back(static_cast<uint8_t>(value >> (8 * k)));
        };
        put(0, 2);    // reserved
        put(1, 2);    // icon
        put(1, 2);    // one image
        bytes.insert(bytes.end(), found, found + 8);
        put(size, 4);
        put(22, 4);   // the image follows the 6-byte header and the entry
        bytes.insert(bytes.end(), data, data + size);
    }
    try {
        winrt::Windows::Storage::Streams::InMemoryRandomAccessStream stream;
        winrt::Windows::Storage::Streams::DataWriter writer(stream);
        writer.WriteBytes(bytes);
        co_await writer.StoreAsync();
        writer.DetachStream();
        stream.Seek(0);
        media::Imaging::BitmapImage bitmap;
        bitmap.DecodePixelWidth(side);
        co_await bitmap.SetSourceAsync(stream);
        controls::ImageIconSource icon;
        icon.ImageSource(bitmap);
        titleBar.IconSource(icon);
    } catch (winrt::hresult_error const& error) {
        Report("title bar icon: " + Utf8(error.message()));
    }
}
#endif

/// The failure bar at the bottom of a window.
inline xaml::FrameworkElement MakeFailures(Win& win) {
#if BERN_WINUI
    controls::InfoBar bar;
    bar.Severity(controls::InfoBarSeverity::Error);
    bar.IsOpen(false);
    bar.Margin(xaml::Thickness{12, 0, 12, 12});
    bar.VerticalAlignment(xaml::VerticalAlignment::Bottom);
    return bar;
#else
    controls::Border bar;
    bar.Margin(xaml::Thickness{12, 0, 12, 12});
    bar.Padding(xaml::Thickness{12, 10, 12, 10});
    bar.VerticalAlignment(xaml::VerticalAlignment::Bottom);
    bar.Visibility(xaml::Visibility::Collapsed);
    if (auto style = Styles::Themed(L"Border", {{L"Background", L"SystemControlBackgroundChromeMediumLowBrush"}, {L"BorderBrush", Keys::Critical}}))
        bar.Style(style);
    bar.BorderThickness(xaml::Thickness{1, 1, 1, 1});
    controls::Grid line;
    line.ColumnSpacing(10);
    line.ColumnDefinitions().Append(Column(AutoLength()));
    line.ColumnDefinitions().Append(Column(Star()));
    line.ColumnDefinitions().Append(Column(AutoLength()));
    auto icon = Icons::Icon("exclamationmark.triangle");
    if (auto style = Styles::Themed(L"FontIcon", {{L"Foreground", Keys::Critical}})) icon.Style(style);
    icon.VerticalAlignment(xaml::VerticalAlignment::Center);
    line.Children().Append(icon);
    controls::TextBlock text;
    text.TextWrapping(xaml::TextWrapping::Wrap);
    text.VerticalAlignment(xaml::VerticalAlignment::Center);
    controls::Grid::SetColumn(text, 1);
    line.Children().Append(text);
    controls::Button dismiss;
    dismiss.Content(Icons::Icon("xmark", 12));
    Styles::MakeSubtle(dismiss);
    dismiss.Click([bar](auto&&, auto&&) { bar.Visibility(xaml::Visibility::Collapsed); });
    controls::Grid::SetColumn(dismiss, 2);
    line.Children().Append(dismiss);
    bar.Child(line);
    win.failureText = text;
    return bar;
#endif
}

/// The layout size of a width by the breakpoints of `[layouts]`.
inline std::string SizeOf(double width) {
    auto const& catalog = app->catalog;
    if (width < catalog.compactBelow.value_or(0)) return "compact";
    if (catalog.mediumBelow && width < *catalog.mediumBelow) return "medium";
    if (!catalog.expandedBelow || width < *catalog.expandedBelow) return "expanded";
    return "large";
}

/// Until a window is laid out, `window.compact` and its siblings follow
/// the size it opens at, so its first content is already built for that
/// size.
inline void SizeBeforeLayout(Win& win, double width, double height) {
    win.width = width;
    win.size.Set(SizeOf(width));
    win.isShort.Set(height < app->catalog.compactBelow.value_or(0));
}

/// Whether the cursor is hidden for an immersive, idle window.
inline bool cursorHidden = false;

inline void HideCursor(bool hide) {
    if (hide == cursorHidden) return;
    cursorHidden = hide;
    if (hide) {
        while (ShowCursor(FALSE) >= 0) {
        }
    } else {
        while (ShowCursor(TRUE) < 0) {
        }
    }
}

/// Whether `element` is on screen in `win`: loaded, and visible with every
/// element around it up to the window's content.
inline bool OnScreen(Win const& win, xaml::UIElement const& element) {
    auto framework = element.try_as<xaml::FrameworkElement>();
    if (framework && !framework.IsLoaded()) return false;
    for (xaml::DependencyObject node = element; node; node = media::VisualTreeHelper::GetParent(node)) {
        if (auto shown = node.try_as<xaml::UIElement>(); shown && shown.Visibility() != xaml::Visibility::Visible) return false;
        if (node == win.root) return true;
    }
    return false;
}

/// How long the window waits before it rests: `seconds`; with `idle_area`s
/// on screen and a mouse, the seconds of the area under the mouse, and none
/// while it is over none or outside the window.
inline std::optional<double> IdleWait(Win const& win, double seconds) {
    std::vector<std::pair<xaml::UIElement, double>> areas;
    for (auto const& [weak, wait] : win.idleAreas)
        if (auto area = weak.get(); area && OnScreen(win, area)) areas.emplace_back(area, wait);
    if (areas.empty() || win.mouse == Win::Mouse::Unknown) return seconds;
    if (win.mouse == Win::Mouse::Outside) return std::nullopt;
    // What takes the mouse there, topmost first: a control standing over
    // an area takes it off the area.
    try {
        auto hits = media::VisualTreeHelper::FindElementsInHostCoordinates(win.mousePoint, win.root).First();
        if (!hits.HasCurrent()) return std::nullopt;
        for (xaml::DependencyObject node = hits.Current(); node; node = media::VisualTreeHelper::GetParent(node))
            for (auto const& [area, wait] : areas)
                if (node == area) return wait;
    } catch (winrt::hresult_error const&) {
    }
    return std::nullopt;
}

/// `window.idle`: true after `seconds` without pointer, wheel or key input
/// in the window, and false at the next input. With `idle_area`s on screen
/// a mouse counts only while it rests over one, for that area's seconds:
/// elsewhere, or outside the window, the window stays awake.
inline void TrackIdle(std::shared_ptr<Win> const& win, double seconds, std::function<bool()> enabled = nullptr) {
    auto generation = std::make_shared<int64_t>(0);
    auto on = std::make_shared<bool>(true);
    std::weak_ptr<Win> weak = win;
    auto rest = [weak, generation, seconds, on] {
        auto win = weak.lock();
        if (!win) return;
        if (win->idle.Peek()) {
            win->idle.Set(false);
            Snapshot::Schedule();
        }
        auto mine = ++*generation;
        // Switched off (`idle_enabled`), the window stays awake.
        if (!*on) return;
        auto wait = IdleWait(*win, seconds);
        if (!wait) return;
        Host::After(std::chrono::milliseconds(static_cast<int64_t>(*wait * 1000)), [weak, generation, mine, seconds] {
            // Waking the window can show controls under a still mouse: it
            // rests only if the mouse is over an area still.
            if (auto win = weak.lock(); win && *generation == mine && IdleWait(*win, seconds)) {
                win->idle.Set(true);
                Snapshot::Schedule();
            }
        });
    };
    auto root = win->root;
    using PointerHandler = xaml::Input::PointerEventHandler;
    // A mouse's place is known before the input that wakes the window
    // counts; `idle_area`s go by it.
    auto place = [weak](xaml::Input::PointerRoutedEventArgs const& args) {
        auto win = weak.lock();
        if (!win) return;
        if (args.Pointer().PointerDeviceType() != PointerKind::Mouse) {
            win->mouse = Win::Mouse::Unknown;
            return;
        }
        win->mouse = Win::Mouse::At;
        win->mousePoint = args.GetCurrentPoint(nullptr).Position();
    };
    // Only a pointer that really moved counts: a window whose frame changes,
    // as an immersive one's does, gets moves where the pointer stayed.
    auto last = std::make_shared<POINT>();
    GetCursorPos(last.get());
    root.AddHandler(xaml::UIElement::PointerMovedEvent(), winrt::box_value(PointerHandler([rest, last, place](auto&&, auto&& args) {
        place(args);
        POINT now{};
        GetCursorPos(&now);
        if (now.x == last->x && now.y == last->y) return;
        *last = now;
        rest();
    })), true);
    root.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(PointerHandler([rest, place](auto&&, auto&& args) {
        place(args);
        rest();
    })), true);
    root.AddHandler(xaml::UIElement::PointerWheelChangedEvent(), winrt::box_value(PointerHandler([rest](auto&&, auto&&) { rest(); })), true);
    // A mouse leaving the window rests nowhere. The content also hears the
    // exits of its parts, so only a mouse outside it counts.
    root.AddHandler(xaml::UIElement::PointerExitedEvent(), winrt::box_value(PointerHandler([weak, rest](auto&& sender, auto&& args) {
        auto win = weak.lock();
        auto content = sender.try_as<xaml::FrameworkElement>();
        if (!win || !content || args.Pointer().PointerDeviceType() != PointerKind::Mouse) return;
        auto at = args.GetCurrentPoint(content).Position();
        if (at.X >= 0 && at.Y >= 0 && at.X < content.ActualWidth() && at.Y < content.ActualHeight()) return;
        win->mouse = Win::Mouse::Outside;
        rest();
    })), true);
    root.AddHandler(xaml::UIElement::KeyDownEvent(), winrt::box_value(xaml::Input::KeyEventHandler([rest](auto&&, auto&&) { rest(); })), true);
    win->idleRest = rest;
    if (enabled) {
        // Switched on again, the count starts over.
        Reactive::Run([enabled, on, rest] {
            bool now = enabled();
            Reactive::Untrack([&] {
                *on = now;
                rest();
                return 0;
            });
        });
    }
    rest();
}

/// Show or hide the window's title bar for `immersive`.
inline void ShowChrome(Win& win, bool immersive) {
#if BERN_WINUI
    if (win.titleBar) win.titleBar.Visibility(immersive ? xaml::Visibility::Collapsed : xaml::Visibility::Visible);
    try {
        if (auto presenter = win.window.AppWindow().Presenter().try_as<winrt::Microsoft::UI::Windowing::OverlappedPresenter>())
            presenter.SetBorderAndTitleBar(true, !immersive);
    } catch (winrt::hresult_error const&) {
    }
#else
    LONG_PTR style = GetWindowLongPtrW(win.hwnd, GWL_STYLE);
    LONG_PTR next = immersive ? (style & ~WS_CAPTION) | WS_THICKFRAME : style | WS_CAPTION;
    if (next != style) {
        SetWindowLongPtrW(win.hwnd, GWL_STYLE, next);
        SetWindowPos(win.hwnd, nullptr, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
#endif
}

/// The accent last shown, to change the brushes only when it changes.
inline int shownAccent = -1;

/// `WindowSet(accent:, appearance:)` for one window: its appearance, and the
/// application's accent.
inline void FollowTheme(std::shared_ptr<Win> const& win, Ctx const& ctx) {
    auto const& def = win->Def();
    // The interface language, for the system's choice of fonts (Chinese in
    // its UI face, not a fallback such as SimSun) and its typography.
    {
        std::weak_ptr<Win> weak = win;
        Reactive::Run([weak] {
            auto tag = Wide(bern::Text::Language());
            Reactive::Untrack([&] {
                if (auto win = weak.lock(); win && win->root) {
                    win->root.Language(tag);
                    if (win->sheet) win->sheet.Language(tag);
                }
                return 0;
            });
        });
    }
    if (def.appearance) {
        auto appearance = def.appearance;
        std::weak_ptr<Win> weak = win;
        Reactive::Run([weak, appearance, ctx] {
            std::string mode = appearance(ctx);
            Reactive::Untrack([&] {
                auto win = weak.lock();
                if (!win || !win->root) return 0;
                auto theme = B::ThemeNamed(mode);
                win->root.RequestedTheme(theme);
                if (win->sheet) win->sheet.RequestedTheme(theme);
#if BERN_WINUI
                // The caption buttons follow the window's appearance.
                if (win->window) try {
                    auto bar = win->window.AppWindow().TitleBar();
                    bar.PreferredTheme(mode == "dark" ? winrt::Microsoft::UI::Windowing::TitleBarTheme::Dark
                            : mode == "light"         ? winrt::Microsoft::UI::Windowing::TitleBarTheme::Light
                                                      : winrt::Microsoft::UI::Windowing::TitleBarTheme::UseDefaultAppMode);
                } catch (...) {
                }
#endif
                Snapshot::Schedule();
                return 0;
            });
        });
    }
    if (def.palette) {
        auto palette = def.palette;
        Reactive::Run([palette, ctx] {
            auto value = palette(ctx);
            Reactive::Untrack([&] { B::ApplyPalette(value); Snapshot::Schedule(); return 0; });
        });
    }
    if (def.accent) {
        auto accent = def.accent;
        Reactive::Run([accent, ctx] {
            int index = accent(ctx);
            Reactive::Untrack([&] {
                if (index < 0 || index >= static_cast<int>(app->accents.size()) || index == shownAccent) return 0;
                shownAccent = index;
                auto [light, dark] = app->accents[index];
                B::ApplyAccent(light, dark);
                Snapshot::Schedule();
                return 0;
            });
        });
    }
}

/// The page header's margins. While content is under the bars, the header
/// stays below the title bar and its trailing end keeps clear of the
/// caption buttons' column and 12 more, as a Fluent toolbar does over
/// content that reaches the window's top.
inline void PlaceHeader(Win& win) {
    if (!win.header) return;
    double top = kWinUI || win.owner ? 0.0 : 12.0, trailing = 16;
#if BERN_WINUI
    if (win.underBars.Peek() > 0 && !win.owner && win.titleBar && win.window && win.titleBar.XamlRoot()) {
        double scale = win.titleBar.XamlRoot().RasterizationScale();
        top = win.titleBar.ActualHeight();
        if (scale > 0) trailing = win.window.AppWindow().TitleBar().RightInset() / scale + 12;
    }
#endif
    auto margin = win.header.Margin();
    if (margin.Top != top || margin.Right != trailing) win.header.Margin(xaml::Thickness{16, top, trailing, 8});
}

/// The window's content, title, failure bar and window-level behavior, in
/// `root`; for every kind of window.
inline void Mount(std::shared_ptr<Win> const& win, controls::Grid const& host, std::shared_ptr<Page> const& page) {
    auto const& def = win->Def();
    auto content = MakeFlex();
    controls::Grid body;
    controls::RowDefinition headerRow;
    headerRow.Height(AutoLength());
    controls::RowDefinition contentRow;
    contentRow.Height(Star());
    body.RowDefinitions().Append(headerRow);
    body.RowDefinitions().Append(contentRow);
    auto failures = MakeFailures(*win);
    win->failures = failures;
    host.Children().Append(body);
    host.Children().Append(failures);
    // Above the content: copies of shared elements while they move.
    controls::Canvas overlay;
    overlay.IsHitTestVisible(false);
    if (win->root.RowDefinitions().Size() > 1) controls::Grid::SetRowSpan(overlay, static_cast<int32_t>(win->root.RowDefinitions().Size()));
    win->root.Children().Append(overlay);
    win->overlay = overlay;
    Win* raw = win.get();
    win->root.SizeChanged([raw](auto&&, xaml::SizeChangedEventArgs const& args) {
        PlaceHeader(*raw);
        if (args.NewSize().Width > 0) {
            raw->width = args.NewSize().Width;
            raw->size.Set(SizeOf(raw->width));
        }
        // `window.short`: shorter than the compact breakpoint.
        if (args.NewSize().Height > 0) raw->isShort.Set(args.NewSize().Height < app->catalog.compactBelow.value_or(0));
    });
    win->scope->Run([&] {
        auto header = Header(page, nullptr, false);
        header.Margin(xaml::Thickness{16, kWinUI || win->owner ? 0.0 : 12.0, 16, 8});
        body.Children().Append(header);
        controls::Grid holder;
        controls::Grid::SetRow(holder, 1);
        body.Children().Append(holder);
        holder.Children().Append(content);
        raw->host = host;
        raw->header = header;
        raw->holder = holder;
        // `under_bars`: while such a layer is shown the content spans the
        // header's row and, under the title bar, the window's; the bars
        // stay above it.
        Reactive::Run([raw] {
            bool under = raw->underBars.Get() > 0;
            Reactive::Untrack([&] {
                controls::Grid::SetRow(raw->holder, under ? 0 : 1);
                controls::Grid::SetRowSpan(raw->holder, under ? 2 : 1);
                controls::Canvas::SetZIndex(raw->header, under ? 1 : 0);
                if (raw->root && raw->root.RowDefinitions().Size() > 1 && !raw->owner) {
                    controls::Grid::SetRow(raw->host, under ? 0 : 1);
                    controls::Grid::SetRowSpan(raw->host, under ? 2 : 1);
#if BERN_WINUI
                    if (raw->titleBar) controls::Canvas::SetZIndex(raw->titleBar, under ? 1 : 0);
#endif
                }
                PlaceHeader(*raw);
                return 0;
            });
        });
        Ctx ctx;
        ctx.win = raw;
        // Controls written in the window itself are named after it.
        ctx.comp = def.id;
        ctx.page = page;
        def.render(Parent::Window(content), ctx);
        Reactive::Run([holder, raw] { holder.IsHitTestVisible(!raw->pending.Get().has_value()); });
        if (def.idleAfter) TrackIdle(win, *def.idleAfter, def.idleEnabled ? std::function<bool()>([enabled = def.idleEnabled, ctx] { return enabled(ctx); }) : nullptr);
        if (def.immersive) {
            auto immersive = def.immersive;
            Reactive::Run([raw, immersive, ctx] {
                bool on = immersive(ctx);
                bool idle = raw->idle.Get();
                Reactive::Untrack([&] {
                    if (raw->immersive.Peek() != on) {
                        raw->immersive.Set(on);
                        if (!raw->owner) ShowChrome(*raw, on);
                    }
                    HideCursor(on && idle);
                    return 0;
                });
            });
            Reactive::OnCleanup([] { HideCursor(false); });
        }
        FollowTheme(win, ctx);
    });
}

/// Show a dialog window's content dialog; it stays until the window closes.
inline winrt::fire_and_forget ShowSheet(std::shared_ptr<Win> win) {
    auto dialog = win->sheet;
    if (!dialog) co_return;
    try {
        co_await dialog.ShowAsync();
    } catch (winrt::hresult_error const& error) {
        Report("dialog window " + win->id + ": " + Utf8(error.message()));
    }
}

/// A dialog window: a content dialog over the window it belongs to, the
/// primary window when it is open.
inline std::string OpenDialog(std::shared_ptr<Win> const& win) {
    auto const& def = win->Def();
    std::shared_ptr<Win> owner;
    for (auto const& candidate : all)
        if (!candidate->owner && candidate.get() != win.get() && (!owner || candidate->Def().id == app->catalog.primary)) owner = candidate;
    if (!owner || !owner->root || !owner->root.XamlRoot()) {
        std::erase(all, win);
        throw Fault{"target_unavailable", "`" + def.id + "` shows over a window, and none is open"};
    }
    win->owner = owner.get();
    win->hwnd = owner->hwnd;
    controls::Grid root;
    win->root = root;
    controls::Grid host;
    root.Children().Append(host);
    auto page = std::make_shared<Page>();
    page->win = win.get();
    controls::ContentDialog dialog;
    if (auto style = Styles::Named(L"DefaultContentDialogStyle")) dialog.Style(style);
    // The declared size, within what a dialog allows.
    auto resources = dialog.Resources();
    if (def.width) {
        resources.Insert(winrt::box_value(L"ContentDialogMinWidth"), winrt::box_value(*def.width));
        resources.Insert(winrt::box_value(L"ContentDialogMaxWidth"), winrt::box_value((std::max)(*def.width, 320.0)));
    }
    if (def.height) resources.Insert(winrt::box_value(L"ContentDialogMinHeight"), winrt::box_value(*def.height));
    resources.Insert(winrt::box_value(L"ContentDialogPadding"), winrt::box_value(xaml::Thickness{0, 0, 0, 0}));
    dialog.Content(root);
    dialog.XamlRoot(owner->root.XamlRoot());
    dialog.RequestedTheme(owner->root.ActualTheme());
    win->sheet = dialog;
    auto titleKey = def.title;
    win->scope->Run([&] {
        Reactive::Run([dialog, titleKey] {
            controls::TextBlock title;
            title.Text(bern::Text::Tr(titleKey));
            if (auto style = Styles::TextStyle("title2")) title.Style(style);
            title.Margin(xaml::Thickness{24, 20, 24, 0});
            dialog.Title(title);
        });
    });
    // A content dialog is as wide as declared, else at most 548, within
    // the window it shows over.
    SizeBeforeLayout(*win, (std::min)(def.width ? (std::max)(*def.width, 320.0) : 548.0, owner->width),
        def.height.value_or(INFINITY));
    Mount(win, host, page);
    std::weak_ptr<Win> weak = win;
    // Its frame is known once laid out.
    root.SizeChanged([weak](auto&&, auto&&) {
        if (auto win = weak.lock()) win->ScheduleReport();
    });
    // Escape and the system's close go through the close protocol.
    dialog.Closing([weak](controls::ContentDialog const&, controls::ContentDialogClosingEventArgs const& args) {
        auto win = weak.lock();
        if (!win || win->allowClose) return;
        args.Cancel(true);
        RequestCloseSafely(win);
    });
    ShowSheet(win);
    win->ScheduleReport();
    Snapshot::Schedule();
    return win->id;
}

/// Open a window of `kind`, or show the one already open. Returns the
/// instance id.
inline std::string Open(std::string const& kindId) {
    auto const& windows = app->windows;
    int kind = -1;
    for (size_t i = 0; i < windows.size(); ++i)
        if (windows[i].id == kindId) kind = static_cast<int>(i);
    if (kind < 0) throw Fault{"target_not_found", "no window kind `" + kindId + "`"};
    for (auto const& existing : all)
        if (existing->kind == kind) {
            Show(*existing);
            return existing->id;
        }
    auto instance = DecodeInstance(Host::Take(bern_window_opened(Host::handle, static_cast<uint32_t>(kind))));
    auto const& def = windows[kind];
    auto win = std::make_shared<Win>();
    win->kind = kind;
    win->generation = instance.generation;
    win->id = instance.id;
    win->cells = std::move(instance.cells);
    all.push_back(win);
    if (def.dialog) {
        try {
            return OpenDialog(win);
        } catch (Fault const&) {
            if (Host::handle) bern_window_closed(Host::handle, static_cast<uint32_t>(win->kind), win->generation);
            throw;
        }
    }

    controls::Grid root;
    win->root = root;
    auto host = controls::Grid();
#if BERN_WINUI
    xaml::Window window;
    win->window = window;
    {
        HWND hwnd = nullptr;
        winrt::check_hresult(window.as<IWindowNative>()->get_WindowHandle(&hwnd));
        win->hwnd = hwnd;
    }
    window.SystemBackdrop(media::MicaBackdrop());
    window.ExtendsContentIntoTitleBar(true);
    controls::TitleBar titleBar;
    win->titleBar = titleBar;
    controls::RowDefinition titleRow;
    titleRow.Height(AutoLength());
    controls::RowDefinition hostRow;
    hostRow.Height(Star());
    root.RowDefinitions().Append(titleRow);
    root.RowDefinitions().Append(hostRow);
    root.Children().Append(titleBar);
    controls::Grid::SetRow(host, 1);
    root.Children().Append(host);
    window.Content(root);
    window.SetTitleBar(titleBar);
    titleBar.LayoutUpdated([weakBar = winrt::make_weak(titleBar), weakWindow = winrt::make_weak(window),
                               fitted = std::array<double, 3>{-1, -1, 0}](auto&&, auto&&) mutable {
        auto titleBar = weakBar.get();
        auto window = weakWindow.get();
        if (titleBar && window) FitTitleBarInsets(titleBar, window, fitted);
    });
    titleBar.SizeChanged([raw = win.get()](auto&&, auto&&) { PlaceHeader(*raw); });
    originals[win->hwnd] = reinterpret_cast<WNDPROC>(SetWindowLongPtrW(win->hwnd, GWLP_WNDPROC, reinterpret_cast<LONG_PTR>(Subclass)));
    if (def.settings) {
        auto presenter = window.AppWindow().Presenter().as<winrt::Microsoft::UI::Windowing::OverlappedPresenter>();
        presenter.IsMaximizable(false);
        presenter.IsMinimizable(false);
    }
#else
    RegisterClassOnce();
    DWORD style = WS_OVERLAPPEDWINDOW;
    if (def.settings) style &= ~(WS_MAXIMIZEBOX | WS_MINIMIZEBOX);
    win->hwnd = CreateWindowExW(0, kWindowClass, L"", style, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, nullptr, nullptr,
        GetModuleHandleW(nullptr), nullptr);
    if (!win->hwnd) {
        all.pop_back();
        throw Fault{"platform_error", "cannot create a window"};
    }
    xaml::Hosting::DesktopWindowXamlSource source;
    win->source = source;
    auto native = source.as<IDesktopWindowXamlSourceNative>();
    winrt::check_hresult(native->AttachToWindow(win->hwnd));
    winrt::check_hresult(native->get_WindowHandle(&win->island));
    if (auto style = Styles::Themed(L"Grid", {{L"Background", Keys::PageBackground}})) root.Style(style);
    root.Children().Append(host);
    source.Content(root);
    std::weak_ptr<Win> weakTheme = win;
    root.ActualThemeChanged([weakTheme](auto&&, auto&&) {
        if (auto win = weakTheme.lock()) UpdateFrameTheme(*win);
    });
#endif
    SetWindowIcons(win->hwnd);
#if BERN_WINUI
    ShowTitleIcon(titleBar, static_cast<int>(std::lround(16 * win->Scale())));
#endif
    auto appearance = Dev::Appearance();
    if (appearance == "dark") root.RequestedTheme(xaml::ElementTheme::Dark);
    else if (appearance == "light") root.RequestedTheme(xaml::ElementTheme::Light);
#if !BERN_WINUI
    UpdateFrameTheme(*win);
#endif

    auto page = std::make_shared<Page>();
    page->win = win.get();
    auto titleKey = def.title;
    win->scope->Run([&] {
#if BERN_WINUI
        // The title bar names the window beside its icon, as Fluent
        // windows do, unified or not.
        Reactive::Run([window, titleBar, titleKey] {
            auto title = bern::Text::Tr(titleKey);
            window.Title(title);
            titleBar.Title(title);
        });
#else
        HWND hwnd = win->hwnd;
        Reactive::Run([hwnd, titleKey] { SetWindowTextW(hwnd, bern::Text::Tr(titleKey).c_str()); });
#endif
    });
    double width = def.width.value_or(def.settings ? 640 : 960);
    double height = def.height.value_or(def.settings ? 560 : 680);
    SizeBeforeLayout(*win, width, height);
    Mount(win, host, page);
    SetOuterSize(*win, width, height);

    std::weak_ptr<Win> weak = win;
    auto closeNow = [weak](auto&&, xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args) {
        args.Handled(true);
        if (auto win = weak.lock()) RequestCloseSafely(win);
    };
    if (def.settings) {
        xaml::Input::KeyboardAccelerator escape;
        escape.Key(winrt::Windows::System::VirtualKey::Escape);
        escape.Invoked(closeNow);
        root.KeyboardAccelerators().Append(escape);
    }
    xaml::Input::KeyboardAccelerator closeKey;
    closeKey.Key(winrt::Windows::System::VirtualKey::W);
    closeKey.Modifiers(winrt::Windows::System::VirtualKeyModifiers::Control);
    closeKey.Invoked(closeNow);
    root.KeyboardAccelerators().Append(closeKey);
    root.KeyboardAcceleratorPlacementMode(xaml::Input::KeyboardAcceleratorPlacementMode::Hidden);

#if BERN_WINUI
    window.Activate();
#else
    ShowWindow(win->hwnd, SW_SHOWNORMAL);
    UpdateWindow(win->hwnd);
    SetForegroundWindow(win->hwnd);
#endif
    win->ScheduleReport();
    Snapshot::Schedule();
    return win->id;
}

inline void Show(Win& win) {
    if (win.owner) {
        Show(*win.owner);
        return;
    }
    if (IsIconic(win.hwnd)) ShowWindow(win.hwnd, SW_RESTORE);
#if BERN_WINUI
    win.window.Activate();
#else
    ShowWindow(win.hwnd, SW_SHOW);
#endif
    SetForegroundWindow(win.hwnd);
}

/// The application ends with its last window, as Windows applications do.
inline void ExitIfDone() {
    if (!all.empty()) return;
#if BERN_WINUI
    xaml::Application::Current().Exit();
#else
    PostQuitMessage(0);
#endif
}

/// The window instance ended: release its drafts, controls and
/// subscriptions; application players stay.
inline void Ended(std::shared_ptr<Win> const& win) {
    if (win->closed) return;
    win->closed = true;
    std::erase(all, win);
    // Dialog windows over this one end with it.
    std::vector<std::shared_ptr<Win>> sheets;
    for (auto const& other : all)
        if (other->owner == win.get()) sheets.push_back(other);
    for (auto const& sheet : sheets) {
        sheet->allowClose = true;
        if (sheet->sheet) sheet->sheet.Hide();
        Ended(sheet);
    }
    win->scope->Dispose();
    B::AllProbes().erase(win.get());
    auto waits = std::move(win->waits);
    win->waits.clear();
    for (auto& wait : waits) wait.second(Failure{"stale_window", "`" + win->id + "` closed while the command ran"});
    if (Host::handle) bern_window_closed(Host::handle, static_cast<uint32_t>(win->kind), win->generation);
#if !BERN_WINUI
    if (win->source) {
        win->source.Close();
        win->source = nullptr;
    }
#endif
    win->life.reset();
    Host::Post(ExitIfDone);
}

inline std::optional<CloseCheck> Evaluate(Win& win) {
    auto const& check = win.Def().close;
    if (!check) return std::nullopt;
    Ctx ctx;
    ctx.win = &win;
    return Reactive::Untrack([&] { return check(ctx); });
}

void DismissDialog(Win& win);
winrt::fire_and_forget ShowDialog(std::shared_ptr<Win> win, PendingClose pending);

inline void FinishClose(std::shared_ptr<Win> const& win) {
    win->allowClose = true;
    DismissDialog(*win);
    if (win->owner) {
        auto sheet = win->sheet;
        Ended(win);
        if (sheet) sheet.Hide();
        return;
    }
    Ended(win);
#if BERN_WINUI
    win->window.Close();
#else
    DestroyWindow(win->hwnd);
#endif
}

/// Run the window's close check and ask the session runtime whether to close
/// now or wait for a decision. A pending confirmation is returned again
/// rather than asked twice.
inline CloseResult RequestClose(std::shared_ptr<Win> const& win, bool close) {
    auto check = Evaluate(*win);
    bool needs = check && check->needsConfirmation;
    bool canSave = check && static_cast<bool>(check->save);
    auto reply = Host::Take(bern_window_request_close(Host::handle, static_cast<uint32_t>(win->kind), win->generation, needs ? 1 : 0, canSave ? 1 : 0));
    Reader reader(reply);
    if (auto failure = Host::Status(reply, reader)) throw Fault{failure->code, failure->message};
    CloseResult result;
    result.window = win->id;
    if (reader.U8() == 0) {
        if (close) FinishClose(win);
        return result;
    }
    PendingClose pending;
    pending.id = reader.Text();
    pending.message = check ? check->message : winrt::hstring();
    pending.canSave = canSave;
    pending.draftRevision = reader.U64();
    win->pending.Set(pending);
    ShowDialog(win, pending);
    Snapshot::Schedule();
    result.outcome = CloseResult::ConfirmationRequired;
    result.confirmation = pending.id;
    result.draftRevision = pending.draftRevision;
    result.decisions = canSave ? std::vector<uint32_t>{0, 1, 2} : std::vector<uint32_t>{1, 2};
    result.message = Utf8(pending.message);
    return result;
}

inline void RequestCloseSafely(std::shared_ptr<Win> const& win) {
    try {
        RequestClose(win, true);
    } catch (Fault const& fault) {
        win->Fail(Failure{fault.code, fault.message});
    }
}

inline void DismissDialog(Win& win) {
    auto dialog = win.dialog;
    win.dialog = nullptr;
    win.dialogConfirmation.clear();
    if (dialog) dialog.Hide();
}

using CloseOutcome = std::function<void(std::optional<CloseResult> const&, std::optional<Failure> const&)>;

/// Apply a decision from the dialog or from an external entry. A save that
/// fails keeps the window open.
inline void ConfirmClose(std::shared_ptr<Win> const& win, std::string const& confirmation, uint32_t decision, CloseOutcome completion) {
    Writer writer;
    writer.Text(confirmation);
    writer.U32(decision);
    auto reply = Host::Take(bern_window_confirm_close(Host::handle, static_cast<uint32_t>(win->kind), win->generation, writer.bytes.data(), writer.bytes.size()));
    auto current = win->pending.Peek();
    uint64_t revision = current ? current->draftRevision : 0;
    Reader reader(reply);
    if (auto failure = Host::Status(reply, reader)) {
        // A stale confirmation no longer describes the window.
        if (failure->code == "stale_confirmation" && current && current->id == confirmation) {
            win->pending.Set(std::nullopt);
            DismissDialog(*win);
        }
        completion(std::nullopt, failure);
        return;
    }
    auto step = reader.U8();
    win->pending.Set(std::nullopt);
    DismissDialog(*win);
    Snapshot::Schedule();
    CloseResult result;
    result.window = win->id;
    result.draftRevision = revision;
    switch (step) {
    case 0:
        FinishClose(win);
        completion(result, std::nullopt);
        break;
    case 1: {
        auto check = Evaluate(*win);
        if (!check || !check->save) {
            completion(std::nullopt, Failure{"invalid_params", "no save command"});
            return;
        }
        Ctx ctx;
        ctx.win = win.get();
        check->save(ctx, [win, result, completion](std::optional<Failure> const& failure) mutable {
            if (!failure) {
                FinishClose(win);
                completion(result, std::nullopt);
            } else {
                result.outcome = CloseResult::Rejected;
                result.reason = "save_failed: " + failure->code + ": " + failure->message;
                completion(result, std::nullopt);
            }
        });
        break;
    }
    default:
        result.outcome = CloseResult::Rejected;
        result.reason = "cancelled";
        completion(result, std::nullopt);
        break;
    }
}

inline winrt::fire_and_forget ShowDialog(std::shared_ptr<Win> win, PendingClose pending) {
    // The same confirmation is already on screen.
    if (win->dialog && win->dialogConfirmation == pending.id) co_return;
    DismissDialog(*win);
    controls::ContentDialog dialog;
    if (auto style = Styles::Named(L"DefaultContentDialogStyle")) dialog.Style(style);
    dialog.Title(winrt::box_value(pending.message));
    dialog.Content(winrt::box_value(bern::Text::Tr(pending.canSave ? "bern.close.save_detail" : "bern.close.discard_detail")));
    dialog.CloseButtonText(bern::Text::Tr("bern.close.cancel"));
    dialog.XamlRoot(win->root.XamlRoot());
    dialog.RequestedTheme(win->root.ActualTheme());
    if (pending.canSave) {
        dialog.PrimaryButtonText(bern::Text::Tr("bern.close.save"));
        dialog.SecondaryButtonText(bern::Text::Tr("bern.close.dont_save"));
        dialog.DefaultButton(controls::ContentDialogButton::Primary);
    } else {
        dialog.PrimaryButtonText(bern::Text::Tr("bern.close.discard"));
        dialog.DefaultButton(controls::ContentDialogButton::Close);
    }
    win->dialog = dialog;
    win->dialogConfirmation = pending.id;
    controls::ContentDialogResult result = controls::ContentDialogResult::None;
    try {
        result = co_await dialog.ShowAsync();
    } catch (...) {
        co_return;
    }
    // Answered elsewhere, for example by an external entry.
    if (win->dialog != dialog) co_return;
    win->dialog = nullptr;
    win->dialogConfirmation.clear();
    uint32_t decision = result == controls::ContentDialogResult::Primary ? (pending.canSave ? 0u : 1u)
        : result == controls::ContentDialogResult::Secondary              ? 1u
                                                                           : 2u;
    ConfirmClose(win, pending.id, decision, [](auto const&, auto const&) {});
}

using Complete = std::function<void(std::optional<Failure> const&)>;

inline void Minimize(Win& win, Complete done) {
    HWND hwnd = win.hwnd;
    win.Perform([hwnd] { ShowWindow(hwnd, SW_MINIMIZE); }, [](Facts const& facts) { return facts.visibility == 1; }, std::move(done));
}

inline void Restore(Win& win, Complete done) {
    auto facts = win.CurrentFacts();
    HWND hwnd = win.hwnd;
    win.Perform(
        [hwnd] {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        },
        [facts](Facts const& f) { return f.visibility == 0 && (facts.visibility == 1 ? f.size == facts.size : f.size == 0); }, std::move(done));
}

inline void Maximize(Win& win, Complete done) {
    HWND hwnd = win.hwnd;
    win.Perform([hwnd] { ShowWindow(hwnd, SW_MAXIMIZE); }, [](Facts const& facts) { return facts.size == 1; }, std::move(done));
}

inline void Move(Win& win, double x, double y, Complete done) {
    Win* target = &win;
    win.Perform(
        [target, x, y] { SetPosition(*target, x, y); },
        [x, y](Facts const& facts) { return std::abs(facts.x - x) <= 1.5 && std::abs(facts.y - y) <= 1.5; }, std::move(done));
}

inline void Resize(Win& win, double width, double height, Complete done) {
    double minimumWidth = win.Def().minWidth.value_or(1);
    double minimumHeight = win.Def().minHeight.value_or(1);
    if (width < minimumWidth || height < minimumHeight) {
        done(Failure{"out_of_range", "`" + win.id + "` needs at least " + R::Number(minimumWidth) + " × " + R::Number(minimumHeight)});
        return;
    }
    Win* target = &win;
    win.Perform(
        [target, width, height] { SetOuterSize(*target, width, height); },
        [width, height](Facts const& facts) { return std::abs(facts.width - width) <= 1.5 && std::abs(facts.height - height) <= 1.5; },
        std::move(done));
}

/// Open the windows declared `initial: open`.
inline void OpenInitial() {
    for (auto const& def : app->windows) {
        if (!def.initialOpen) continue;
        try {
            Open(def.id);
        } catch (Fault const& fault) {
            Report("cannot open `" + def.id + "`: " + fault.message);
        } catch (winrt::hresult_error const& error) {
            Report("cannot open `" + def.id + "`: " + Utf8(error.message()));
        }
    }
    ExitIfDone();
}

inline void ReopenPrimary() {
    try {
        Open(app->catalog.primary);
    } catch (Fault const& fault) {
        Report("cannot open `" + app->catalog.primary + "`: " + fault.message);
    }
}

}

namespace WindowOps {
inline void ToggleFullscreen(Ctx const& ctx, std::string const& kind) {
    for (auto const& win : WindowManager::all) {
        if (win->Def().id != kind) continue;
        auto app = win->window.AppWindow();
        using winrt::Microsoft::UI::Windowing::AppWindowPresenterKind;
        app.SetPresenter(app.Presenter().Kind() == AppWindowPresenterKind::FullScreen
            ? AppWindowPresenterKind::Overlapped : AppWindowPresenterKind::FullScreen);
        win->fullscreen.Set(app.Presenter().Kind() == AppWindowPresenterKind::FullScreen);
    }
}


/// `windows.<kind>.open()` and `activate()` in declarations.
inline void Open(Ctx const& ctx, std::string const& kind) {
    try {
        WindowManager::Open(kind);
    } catch (Fault const& fault) {
        if (ctx.win) ctx.win->Fail(Failure{fault.code, fault.message});
    }
}

/// `windows.<kind>.close()` in declarations: the close protocol, as from the
/// title bar.
inline void Close(Ctx const& ctx, std::string const& kind) {
    for (auto const& win : WindowManager::all) {
        if (win->Def().id != kind) continue;
        try {
            WindowManager::RequestClose(win, true);
        } catch (Fault const& fault) {
            if (ctx.win) ctx.win->Fail(Failure{fault.code, fault.message});
        }
        return;
    }
}

}

// ---------------------------------------------------------------------------
// Session commands of external entries, run through the same closures the
// user's input runs, in the order of the framework session catalog.

namespace Session {

inline void Send(uint64_t request, Bytes const& bytes) {
    bern_session_complete(Host::handle, request, bytes.data(), bytes.size());
    Snapshot::Schedule();
}

inline void Complete(uint64_t request, uint32_t index, Value const& result) {
    Writer writer;
    writer.U8(0);
    writer.U32(index);
    auto const& results = app->catalog.sessionResults;
    if (index < results.size()) Codec::Encode(result, results[index], app->catalog.types, writer);
    Send(request, writer.bytes);
}

inline void Fail(uint64_t request, std::string const& code, std::string const& message) {
    Writer writer;
    writer.U8(1);
    writer.Text(code);
    writer.Text(message);
    Send(request, writer.bytes);
}

/// The control mounted at `target`, copied: running its action may unmount it.
inline ControlEntry Find(std::string const& target) {
    ControlEntry const* found = nullptr;
    int matches = 0;
    for (auto const& [token, entry] : Controls::entries)
        if (entry.target == target) {
            found = &entry;
            ++matches;
        }
    if (matches == 0) throw Fault{"target_not_found", "no mounted control `" + target + "`; see ui.inspect"};
    if (matches > 1) throw Fault{"target_ambiguous", "`" + target + "` is mounted in several places"};
    if (found->window && found->window->IsBlocked())
        throw Fault{"target_blocked", "`" + target + "` is blocked by the close confirmation of `" + found->window->id + "`; decide it with window.confirm_close"};
    if (found->enabled && !Reactive::Untrack([&] { return found->enabled(); })) throw Fault{"target_unavailable", "`" + target + "` is disabled"};
    return *found;
}

template <class F> F Need(ControlEntry const& entry, F const& action, char const* name) {
    if (!action) throw Fault{"target_unavailable", "`" + entry.target + "` (" + entry.role + ") does not support " + name};
    return action;
}

inline Value Inspect() {
    auto optional = [](std::optional<std::string> const& text) { return text ? Value::Some(Value::Text(*text)) : Value::None(); };
    Values rows;
    for (auto const& [token, entry] : Controls::entries) {
        bool enabled = !entry.enabled || Reactive::Untrack([&] { return entry.enabled(); });
        auto label = entry.label ? Utf8(Reactive::Untrack([&] { return entry.label(); })) : std::string();
        auto value = entry.actions.value ? Reactive::Untrack([&] { return entry.actions.value(); }) : std::nullopt;
        std::optional<std::string> reason;
        if (entry.window && entry.window->IsBlocked()) reason = "blocked_by_modal";
        else if (!enabled) reason = "disabled";
        rows.push_back(Value::Record({Value::Text(entry.target), Value::Text(entry.window ? entry.window->id : std::string()), Value::Text(entry.role),
            Value::Text(label), Value::Bool(enabled), optional(value), optional(reason)}));
    }
    return Value::List(std::move(rows));
}

inline void Run(uint64_t request, uint32_t index, Reader& reader) {
    auto ok = [&](Value const& result) { Complete(request, index, result); };
    auto presentation = [request, index](std::shared_ptr<Win> const& win, auto perform) {
        // A dialog window shows over its owner: it only activates and closes.
        if (win->owner) throw Fault{"target_unavailable", "`" + win->id + "` is a dialog over another window; it can activate and close"};
        perform(*win, [request, index](std::optional<Failure> const& failure) {
            if (!failure) Complete(request, index, Value::Unit());
            else Fail(request, failure->code, failure->message);
        });
    };
    switch (index) {
    case 0: ok(Inspect()); break;
    case 1: {
        auto entry = Find(reader.Text());
        Need(entry, entry.actions.activate, "activate")();
        ok(Value::Unit());
        break;
    }
    case 2: {
        auto entry = Find(reader.Text());
        auto text = reader.Text();
        Need(entry, entry.actions.edit, "edit")(text);
        ok(Value::Unit());
        break;
    }
    case 3: {
        auto entry = Find(reader.Text());
        Need(entry, entry.actions.submit, "submit")();
        ok(Value::Unit());
        break;
    }
    case 4: {
        auto entry = Find(reader.Text());
        bool on = reader.U8() != 0;
        Need(entry, entry.actions.setBool, "set_bool")(on);
        ok(Value::Unit());
        break;
    }
    case 5: {
        auto entry = Find(reader.Text());
        auto number = reader.F64();
        Need(entry, entry.actions.setNumber, "set_number")(number);
        ok(Value::Unit());
        break;
    }
    case 6: {
        auto entry = Find(reader.Text());
        auto option = reader.Text();
        if (!Need(entry, entry.actions.select, "select")(option)) throw Fault{"invalid_params", "`" + entry.target + "` has no option `" + option + "`"};
        ok(Value::Unit());
        break;
    }
    case 7: {
        auto entry = Find(reader.Text());
        if (!Need(entry, entry.actions.back, "back")()) throw Fault{"target_unavailable", "nothing to go back to"};
        ok(Value::Unit());
        break;
    }
    case 8: ok(Value::Text(WindowManager::Open(reader.Text()))); break;
    case 9: ok(WindowManager::RequestClose(WindowManager::Named(reader.Text()), true).ToValue()); break;
    case 10:
        WindowManager::Show(*WindowManager::Named(reader.Text()));
        ok(Value::Unit());
        break;
    case 11: {
        auto win = WindowManager::Named(reader.Text());
        auto confirmation = reader.Text();
        auto decision = reader.U32();
        if (decision > 2) throw Fault{"invalid_params", "unknown close decision"};
        WindowManager::ConfirmClose(win, confirmation, decision, [request, index](std::optional<CloseResult> const& result, std::optional<Failure> const& failure) {
            if (failure) Fail(request, failure->code, failure->message);
            else Complete(request, index, result->ToValue());
        });
        break;
    }
    case 12: throw Fault{"unknown_command", "window.list is answered by the runtime"};
    case 13: presentation(WindowManager::Named(reader.Text()), WindowManager::Minimize); break;
    case 14: presentation(WindowManager::Named(reader.Text()), WindowManager::Restore); break;
    case 15: presentation(WindowManager::Named(reader.Text()), WindowManager::Maximize); break;
    case 16: {
        auto win = WindowManager::Named(reader.Text());
        auto x = reader.F64();
        auto y = reader.F64();
        presentation(win, [x, y](Win& w, WindowManager::Complete done) { WindowManager::Move(w, x, y, std::move(done)); });
        break;
    }
    case 17: {
        auto win = WindowManager::Named(reader.Text());
        auto width = reader.F64();
        auto height = reader.F64();
        presentation(win, [width, height](Win& w, WindowManager::Complete done) { WindowManager::Resize(w, width, height, std::move(done)); });
        break;
    }
    case 18: ok(Value::Text(bern::Text::PeekLanguage())); break;
    case 19: {
        auto language = reader.Text();
        if (!bern::Text::SetLanguage(language)) {
            std::string supported;
            for (auto const& tag : bern::Text::Supported()) supported += (supported.empty() ? "" : ", ") + tag;
            throw Fault{"unsupported_language", "`" + language + "` is not supported; supported: " + supported};
        }
        ok(Value::Unit());
        break;
    }
    case 20: {
        auto entry = Find(reader.Text());
        Need(entry, entry.actions.focus, "focus")();
        ok(Value::Unit());
        break;
    }
    case 21: {
        auto entry = Find(reader.Text());
        auto url = reader.Text();
        auto give = Need(entry, entry.actions.setFile, "set_file");
        auto scheme = url.find(':');
        bool valid = scheme != std::string::npos && scheme > 0 && url.find_first_of(" \t\r\n") == std::string::npos;
        std::string lower = url.substr(0, scheme == std::string::npos ? 0 : scheme);
        for (auto& c : lower) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
        if (!valid || (lower != "file" && lower != "http" && lower != "https")) throw Fault{"invalid_url", "`" + url + "` is not a file or http(s) URL"};
        if (!B::Accepts(url, entry.actions.files)) {
            throw Fault{"unsupported_file", "`" + entry.target + "` takes " + std::string(entry.actions.files ? entry.actions.files : "any") + ", not `" + url + "`"};
        }
        give(url);
        ok(Value::Unit());
        break;
    }
    default: throw Fault{"unknown_command", "unknown session command " + std::to_string(index)};
    }
}

/// Handle one session request from the session runtime.
inline void Handle(uint64_t request, Bytes const& bytes) {
    Reader reader(bytes);
    uint32_t index;
    try {
        index = reader.U32();
    } catch (DecodeError const& error) {
        Fail(request, "invalid_request", error.what());
        return;
    }
    try {
        Run(request, index, reader);
    } catch (Fault const& fault) {
        Fail(request, fault.code, fault.message);
    } catch (DecodeError const& error) {
        Fail(request, "invalid_request", error.what());
    } catch (winrt::hresult_error const& error) {
        Fail(request, "platform_error", Utf8(error.message()));
    }
}

}

// ---------------------------------------------------------------------------
// Development aids, off by default: `BERN_APPEARANCE=light|dark` overrides
// the system appearance, and `BERN_SNAPSHOT_DIR` makes the application render
// each window's content to `<dir>/<window>.png` after it opens and after
// every change, for checks without screen access.

namespace Snapshot {

inline bool queued = false;

inline winrt::fire_and_forget Save(xaml::UIElement element, std::wstring path, double dpi) {
    media::Imaging::RenderTargetBitmap bitmap;
    co_await bitmap.RenderAsync(element);
    if (bitmap.PixelWidth() == 0 || bitmap.PixelHeight() == 0) co_return;
    auto buffer = co_await bitmap.GetPixelsAsync();
    Bytes pixels(buffer.data(), buffer.data() + buffer.Length());
    bool dark = false;
    if (auto framework = element.try_as<xaml::FrameworkElement>()) dark = framework.ActualTheme() == xaml::ElementTheme::Dark;
    Dev::SavePng(std::move(pixels), bitmap.PixelWidth(), bitmap.PixelHeight(), dpi, std::move(path), dark);
}

/// The window as the screen shows it, frame and backdrop included.
inline void CaptureWindow(HWND hwnd, std::wstring path, double dpi) {
    RECT rect{};
    if (!GetWindowRect(hwnd, &rect)) return;
    int width = rect.right - rect.left, height = rect.bottom - rect.top;
    if (width <= 0 || height <= 0) return;
    HDC screen = GetDC(nullptr);
    HDC dc = CreateCompatibleDC(screen);
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    void* bits = nullptr;
    HBITMAP bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &bits, nullptr, 0);
    if (bitmap && bits) {
        auto previous = SelectObject(dc, bitmap);
        // PW_RENDERFULLCONTENT: composed content, XAML included.
        if (PrintWindow(hwnd, dc, 2)) {
            Bytes pixels(static_cast<uint8_t*>(bits), static_cast<uint8_t*>(bits) + static_cast<size_t>(width) * height * 4);
            for (size_t i = 3; i < pixels.size(); i += 4) pixels[i] = 255;
            Dev::SavePng(std::move(pixels), width, height, dpi, std::move(path), false);
        }
        SelectObject(dc, previous);
        DeleteObject(bitmap);
    }
    DeleteDC(dc);
    ReleaseDC(nullptr, screen);
}

/// `BERN_DUMP_TREE`: the element tree with each element's place in the
/// window and size, on standard error.
inline void DumpTree(xaml::DependencyObject const& node, int depth, xaml::UIElement const& root = nullptr) {
    if (depth > 80 || !node) return;
    std::string line(static_cast<size_t>(depth) * 2, ' ');
    line += Utf8(winrt::get_class_name(node));
    if (auto element = node.try_as<xaml::FrameworkElement>()) {
        winrt::Windows::Foundation::Point at{};
        try {
            if (root) at = element.TransformToVisual(root).TransformPoint({0, 0});
        } catch (winrt::hresult_error const&) {
        }
        char size[160];
        std::snprintf(size, sizeof size, " @%.0f,%.0f %.0fx%.0f%s", at.X, at.Y, element.ActualWidth(), element.ActualHeight(),
            element.Visibility() == xaml::Visibility::Visible ? "" : " hidden");
        line += size;
        if (auto tag = element.Tag()) line += " " + Utf8(winrt::unbox_value_or<winrt::hstring>(tag, L""));
        if (!std::isnan(element.Height())) line += " h=" + std::to_string(static_cast<int>(element.Height()));
        // Its own clip, and the slot layout gave it when that is smaller
        // than it asked for, which clips it too.
        if (auto clip = element.Clip()) {
            auto r = clip.Rect();
            char text[96];
            std::snprintf(text, sizeof text, " clip=%.1f,%.1f %.1fx%.1f", r.X, r.Y, r.Width, r.Height);
            line += text;
        }
        auto slot = controls::Primitives::LayoutInformation::GetLayoutSlot(element);
        auto wanted = element.DesiredSize();
        if (wanted.Width > slot.Width + 0.5f || wanted.Height > slot.Height + 0.5f) {
            char text[96];
            std::snprintf(text, sizeof text, " slot=%.1fx%.1f<%.1fx%.1f", slot.Width, slot.Height, wanted.Width, wanted.Height);
            line += text;
        }
        if (auto text = element.try_as<controls::TextBlock>()) {
            char font[64];
            std::snprintf(font, sizeof font, " %.1f/%d", text.FontSize(), static_cast<int>(text.FontWeight().Weight));
            line += " \"" + Utf8(text.Text()).substr(0, 24) + "\" " + Utf8(text.FontFamily().Source()) + font;
        }
    }
    bern::Report(line);
    auto count = media::VisualTreeHelper::GetChildrenCount(node);
    for (int32_t i = 0; i < count; ++i) DumpTree(media::VisualTreeHelper::GetChild(node, i), depth + 1, root ? root : node.try_as<xaml::UIElement>());
}

inline void CaptureAll() {
    auto directory = Dev::SnapshotDirectory();
    if (!directory) return;
    if (GetEnvironmentVariableW(L"BERN_DUMP_TREE", nullptr, 0) != 0)
        for (auto const& win : WindowManager::all)
            if (win->root) DumpTree(win->root, 0);
    CreateDirectoryW(directory->c_str(), nullptr);
    for (auto const& win : WindowManager::all) {
        if (!win->root) continue;
        std::wstring name(Wide(win->id));
        std::replace(name.begin(), name.end(), L'#', L'-');
        double dpi = win->Scale() * 96;
        try {
            CaptureWindow(win->hwnd, *directory + L"\\" + name + L"-window.png", dpi);
            Save(win->root, *directory + L"\\" + name + L".png", dpi);
            // Dialogs and menus live in popups above the content.
            int index = 0;
            if (auto root = win->root.XamlRoot())
                for (auto popup : media::VisualTreeHelper::GetOpenPopupsForXamlRoot(root))
                    if (auto child = popup.Child()) Save(child, *directory + L"\\" + name + L"-popup" + std::to_wstring(++index) + L".png", dpi);
        } catch (winrt::hresult_error const& error) {
            Report("snapshot of " + win->id + ": " + Utf8(error.message()));
        }
    }
}

/// `BERN_SNAPSHOT_BURST`: after a change, also the window as the screen
/// shows it every 40 ms for 800 ms, as `<window>-burst<n>.png`, to see
/// motion in development.
inline bool burst = false;
inline int burstRun = 0;

inline void Burst() {
    auto directory = Dev::SnapshotDirectory();
    if (!directory || GetEnvironmentVariableW(L"BERN_SNAPSHOT_BURST", nullptr, 0) == 0 || burst) return;
    burst = true;
    int run = ++burstRun;
    for (int frame = 0; frame < 20; ++frame) {
        Host::After(std::chrono::milliseconds(20 + frame * 40), [frame, run, directory] {
            if (frame == 19) burst = false;
            for (auto const& win : WindowManager::all) {
                if (!win->hwnd || win->owner) continue;
                std::wstring name(Wide(win->id));
                std::replace(name.begin(), name.end(), L'#', L'-');
                CaptureWindow(win->hwnd, *directory + L"\\" + name + L"-burst" + std::to_wstring(run) + L"-" + std::to_wstring(frame) + L".png", win->Scale() * 96);
            }
        });
    }
}

inline void Schedule() {
    if (!Dev::SnapshotDirectory()) return;
    Burst();
    if (queued) return;
    queued = true;
    Host::After(std::chrono::milliseconds(700), [] {
        queued = false;
        CaptureAll();
    });
}

}

// ---------------------------------------------------------------------------
// The application

/// The declared brand color as the accent color resources, with the lighter
/// and darker steps the controls use in each theme.
inline void ApplyBrand(xaml::ResourceDictionary const& resources, std::array<uint8_t, 3> brand) {
    auto mix = [](uint8_t from, uint8_t to, double amount) { return static_cast<uint8_t>(std::lround(from + (to - from) * amount)); };
    auto toward = [&](uint8_t target, double amount) {
        return winrt::Windows::UI::Color{255, mix(brand[0], target, amount), mix(brand[1], target, amount), mix(brand[2], target, amount)};
    };
    resources.Insert(winrt::box_value(L"SystemAccentColor"), winrt::box_value(winrt::Windows::UI::Color{255, brand[0], brand[1], brand[2]}));
    for (int step = 1; step <= 3; ++step) {
        resources.Insert(winrt::box_value(L"SystemAccentColorLight" + std::to_wstring(step)), winrt::box_value(toward(255, 0.2 * step)));
        resources.Insert(winrt::box_value(L"SystemAccentColorDark" + std::to_wstring(step)), winrt::box_value(toward(0, 0.2 * step)));
    }
}

/// The directory of the executable, for `resources`.
inline std::wstring ExecutableDirectory() {
    std::wstring path(MAX_PATH, L'\0');
    for (;;) {
        auto length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
        if (length < path.size()) {
            path.resize(length);
            break;
        }
        path.resize(path.size() * 2);
    }
    return path.substr(0, path.find_last_of(L'\\'));
}

/// Install resources, start the engine and open the initial windows. False
/// when the engine could not start; the user has been told why.
inline bool Launch(xaml::ResourceDictionary const& resources) {
    if (app->catalog.brandSeed) ApplyBrand(resources, *app->catalog.brandSeed);
    Styles::Install(resources);
    bern::Text::Install(app->catalog);
    Host::onSessionRequest = [](uint64_t request, Bytes bytes) { Session::Handle(request, bytes); };
    Host::onChange = [] { Snapshot::Schedule(); };
    Host::onActivate = [] { WindowManager::ReopenPrimary(); };
    auto directory = ExecutableDirectory() + L"\\resources";
    auto attributes = GetFileAttributesW(directory.c_str());
    bool exists = attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_DIRECTORY);
    if (auto problem = Host::Start(app->catalog, exists ? Utf8(winrt::hstring(directory)) : std::string())) {
        auto title = bern::Text::Tr("bern.startup.failed");
        MessageBoxW(nullptr, Wide(*problem).c_str(), title.c_str(), MB_OK | MB_ICONERROR);
        return false;
    }
    Media::AttachExecutor();
    WindowManager::OpenInitial();
    return true;
}

inline void Finish() {
    Media::DetachExecutor();
    Host::Stop();
}

#if BERN_WINUI

/// The runtime's resource index, when WinUI has to be told where it is.
inline std::wstring resourceIndex;

struct BernApplication : xaml::ApplicationT<BernApplication, xaml::Markup::IXamlMetadataProvider> {
    xaml::XamlTypeInfo::XamlControlsXamlMetaDataProvider provider;

    BernApplication() {
        // The copy of the runtime Windows ships with does not lead WinUI to
        // its own resources (the control styles among them); point at them.
        if (!resourceIndex.empty()) {
            ResourceManagerRequested([](auto&&, xaml::ResourceManagerRequestedEventArgs const& args) {
                args.CustomResourceManager(winrt::Microsoft::Windows::ApplicationModel::Resources::ResourceManager(resourceIndex));
            });
        }
        auto appearance = Dev::Appearance();
        if (appearance == "dark") RequestedTheme(xaml::ApplicationTheme::Dark);
        else if (appearance == "light") RequestedTheme(xaml::ApplicationTheme::Light);
        // WinUI ends the process on an unhandled error; say what it was.
        UnhandledException([](auto&&, xaml::UnhandledExceptionEventArgs const& args) {
            char code[16];
            std::snprintf(code, sizeof code, "0x%08lx", static_cast<unsigned long>(args.Exception().value));
            bern::Report("unhandled WinUI error " + std::string(code) + ": " + winrt::to_string(args.Message()));
        });
    }

    void OnLaunched(xaml::LaunchActivatedEventArgs const&) {
        try {
            Resources().MergedDictionaries().Append(controls::XamlControlsResources());
        } catch (winrt::hresult_error const& error) {
            bern::Report("cannot load the WinUI control styles: " + winrt::to_string(error.message()));
            throw;
        }
        if (!Launch(Resources())) Exit();
    }

    xaml::Markup::IXamlType GetXamlType(winrt::Windows::UI::Xaml::Interop::TypeName const& type) { return provider.GetXamlType(type); }
    xaml::Markup::IXamlType GetXamlType(winrt::hstring const& fullName) { return provider.GetXamlType(fullName); }
    winrt::com_array<xaml::Markup::XmlnsDefinition> GetXmlnsDefinitions() { return provider.GetXmlnsDefinitions(); }
};

/// Run the application on WinUI 3. Returns when the last window closed.
/// `resources`: the runtime's resource index, when WinUI has to be told.
inline int Run(AppDef const& definition, std::wstring const& resources) {
    app = &definition;
    resourceIndex = resources;
    xaml::Application::Start([](auto&&) { winrt::make<BernApplication>(); });
    Finish();
    return 0;
}

#else

/// Run the application on the system XAML. Returns when the last window
/// closed.
inline int Run(AppDef const& definition) {
    app = &definition;
    auto manager = xaml::Hosting::WindowsXamlManager::InitializeForCurrentThread();
    if (Launch(xaml::Application::Current().Resources())) {
        MSG message{};
        while (GetMessageW(&message, nullptr, 0, 0)) {
            BOOL handled = FALSE;
            // Keyboard navigation and accelerators of the island the message
            // is for.
            if (auto win = WindowManager::OfHwnd(GetAncestor(message.hwnd, GA_ROOT)); win && win->source)
                if (auto native = win->source.try_as<IDesktopWindowXamlSourceNative2>()) native->PreTranslateMessage(&message, &handled);
            if (!handled) {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    Finish();
    manager.Close();
    return 0;
}

#endif
