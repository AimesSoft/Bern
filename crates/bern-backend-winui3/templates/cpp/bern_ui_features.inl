// Bern runtime for Windows, XAML part 6 of 7: the controls media
// applications need. Scroll views that come to rest on an item, popover
// and split buttons on flyouts, notices, the inspector pane, drop areas,
// pinch and swipe gestures, the open panel and keyboard shortcuts.
// Generated build input; do not edit.

namespace B {

// ---------------------------------------------------------------------------
// Scroll views

struct ScrollProps {
    char const* axis = nullptr;
    /// Where scrolling comes to rest: an item at the `start` or `center`.
    char const* snap = nullptr;
    /// The key of the item at rest, followed and set.
    std::optional<Field> position;
    /// `nudge`: the item `position` scrolls to gives a nudge on arrival.
    char const* arrival = nullptr;
    Motion const* nudgeMotion = nullptr;
    Motion const* reflow = nullptr;
    /// Scroll without showing the scroll bar.
    bool hideIndicators = false;
    /// Where it opens: the point at this fraction of the content at the
    /// same fraction of the view.
    std::optional<double> openAt;
};

/// The scroll offset that puts `item` at the anchor of `scroll`.
inline std::optional<double> OffsetOf(ScrollInfo const& scroll, xaml::UIElement const& item, bool center) {
    auto viewer = scroll.viewer;
    auto content = viewer.Content().try_as<xaml::UIElement>();
    auto framework = item.try_as<xaml::FrameworkElement>();
    if (!content || !framework) return std::nullopt;
    try {
        // The middle stays put while the item scales about it, as a nudge
        // does.
        double extent = scroll.horizontal ? framework.ActualWidth() : framework.ActualHeight();
        auto middle = item.TransformToVisual(content).TransformPoint({static_cast<float>(framework.ActualWidth() / 2), static_cast<float>(framework.ActualHeight() / 2)});
        // The room left before the first item is outside the content.
        auto room = content.as<xaml::FrameworkElement>().Margin();
        double start = (scroll.horizontal ? middle.X + room.Left : middle.Y + room.Top) - extent / 2;
        double port = scroll.horizontal ? viewer.ViewportWidth() : viewer.ViewportHeight();
        double offset = center ? start + extent / 2 - port / 2 : start;
        double most = scroll.horizontal ? viewer.ScrollableWidth() : viewer.ScrollableHeight();
        return (std::clamp)(offset, 0.0, (std::max)(0.0, most));
    } catch (winrt::hresult_error const&) {
        return std::nullopt;
    }
}

/// The items of `scroll` still on screen, by key.
inline std::vector<std::pair<std::string, xaml::UIElement>> ItemsOf(ScrollInfo& scroll) {
    std::vector<std::pair<std::string, xaml::UIElement>> items;
    std::erase_if(scroll.items, [](auto const& item) { return !item.second.get(); });
    for (auto const& [key, weak] : scroll.items)
        if (auto element = weak.get(); element && element.as<xaml::FrameworkElement>().IsLoaded()) items.emplace_back(key, element);
    return items;
}

/// A scrolling container. With `snap`, scrolling comes to rest with an item
/// at the start or center; `position` follows the item at rest and scrolls
/// to the item it is set to, with a nudge on arrival for `arrival: nudge`.
/// Items are the rows of the lists inside, each known by its key.
inline void ScrollView(Parent const& p, Ctx const& ctx, ScrollProps props, Build const& children) {
    bool horizontal = props.axis && std::string_view(props.axis) == "horizontal";
    auto panel = MakeFlex(horizontal, 8);
    if (props.reflow) SetReflow(panel, props.reflow);
    controls::ScrollViewer scroller;
    scroller.Content(panel);
    // `Hidden` still scrolls, without a scroll bar.
    auto scrolling = props.hideIndicators ? controls::ScrollBarVisibility::Hidden : controls::ScrollBarVisibility::Auto;
    scroller.HorizontalScrollBarVisibility(horizontal ? scrolling : controls::ScrollBarVisibility::Disabled);
    scroller.VerticalScrollBarVisibility(horizontal ? controls::ScrollBarVisibility::Disabled : scrolling);
    scroller.HorizontalScrollMode(horizontal ? controls::ScrollMode::Enabled : controls::ScrollMode::Disabled);
    scroller.VerticalScrollMode(horizontal ? controls::ScrollMode::Disabled : controls::ScrollMode::Enabled);
    SetRole(scroller, horizontal ? L"bern-fill-width" : L"bern-fill");
    auto scroll = std::make_shared<ScrollInfo>();
    scroll->viewer = scroller;
    scroll->horizontal = horizontal;
    auto inner = ctx;
    inner.scroll = scroll;
    children(Parent::Of(panel), inner);
    p.Add(scroller);
    if (props.openAt) {
        // Once it first knows how far it scrolls, it stands at that
        // fraction of it.
        double at = (std::clamp)(*props.openAt, 0.0, 1.0);
        auto opened = std::make_shared<bool>(false);
        scroller.LayoutUpdated([weak = winrt::make_weak(scroller), at, opened, horizontal](auto&&, auto&&) {
            auto viewer = weak.get();
            if (!viewer || *opened) return;
            double room = horizontal ? viewer.ScrollableWidth() : viewer.ScrollableHeight();
            if (room <= 0) return;
            *opened = true;
            if (horizontal) viewer.ChangeView(at * room, nullptr, nullptr, true);
            else viewer.ChangeView(nullptr, at * room, nullptr, true);
        });
    }
    if (!props.snap && !props.position) return;
    // Items come to rest at the center only with `snap: center`; a position
    // alone is kept at the start, as on the other targets.
    bool center = props.snap && std::string_view(props.snap) == "center";
    auto padding = [scroll, panel, center, horizontal] {
        // Room before the first and after the last item, so they can come
        // to the center too.
        if (!center) return;
        double largest = 0;
        for (auto const& [key, item] : ItemsOf(*scroll)) {
            auto framework = item.as<xaml::FrameworkElement>();
            largest = (std::max)(largest, horizontal ? framework.ActualWidth() : framework.ActualHeight());
        }
        double port = horizontal ? scroll->viewer.ViewportWidth() : scroll->viewer.ViewportHeight();
        double room = (std::max)(0.0, (port - largest) / 2);
        auto flex = panel.as<xaml::FrameworkElement>();
        auto margin = horizontal ? xaml::Thickness{room, 0, room, 0} : xaml::Thickness{0, room, 0, room};
        auto now = flex.Margin();
        if (std::abs(now.Left - margin.Left) > 0.5 || std::abs(now.Top - margin.Top) > 0.5) flex.Margin(margin);
    };
    scroller.SizeChanged([padding](auto&&, auto&&) { padding(); });
    panel.as<xaml::FrameworkElement>().SizeChanged([padding](auto&&, auto&&) { padding(); });
    auto nearest = [scroll, center]() -> std::optional<std::pair<std::string, xaml::UIElement>> {
        double now = scroll->horizontal ? scroll->viewer.HorizontalOffset() : scroll->viewer.VerticalOffset();
        std::optional<std::pair<std::string, xaml::UIElement>> best;
        double distance = INFINITY;
        for (auto const& item : ItemsOf(*scroll)) {
            auto offset = OffsetOf(*scroll, item.second, center);
            if (!offset) continue;
            double gap = std::abs(*offset - now);
            if (gap < distance) {
                distance = gap;
                best = item;
            }
        }
        return best;
    };
    auto scrollTo = [scroll, center](xaml::UIElement const& item, bool smooth) {
        auto offset = OffsetOf(*scroll, item, center);
        if (!offset) return;
        if (scroll->horizontal) scroll->viewer.ChangeView(*offset, nullptr, nullptr, !smooth || Reduced());
        else scroll->viewer.ChangeView(nullptr, *offset, nullptr, !smooth || Reduced());
    };
    // Scrolling that ends between items comes to rest on the nearest one.
    auto travelling = std::make_shared<winrt::weak_ref<xaml::UIElement>>();
    auto placed = std::make_shared<bool>(false);
    auto position = props.position;
    bool nudges = props.arrival && std::string_view(props.arrival) == "nudge";
    auto nudgeMotion = props.nudgeMotion;
    scroller.ViewChanged([scroll, nearest, scrollTo, travelling, placed, position, ctx, nudges, nudgeMotion, center](auto&&, controls::ScrollViewerViewChangedEventArgs const& args) {
        if (args.IsIntermediate() || !*placed) return;
        auto item = nearest();
        if (!item) return;
        double now = scroll->horizontal ? scroll->viewer.HorizontalOffset() : scroll->viewer.VerticalOffset();
        auto offset = OffsetOf(*scroll, item->second, center);
        if (offset && std::abs(*offset - now) > 1) {
            scrollTo(item->second, true);
            return;
        }
        if (auto arrived = travelling->get()) {
            // A scroll the field asked for has arrived.
            *travelling = nullptr;
            if (nudges && arrived == item->second) NudgeOnce(arrived, nudgeMotion);
            return;
        }
        if (position && R::KeyText(ctx.Peek(*position)) != item->first) {
            auto const& type = app->catalog.sessions[position->session].fields[position->field].second;
            Value key = type.kind == BType::Kind::Int ? Value::Int(std::strtoll(item->first.c_str(), nullptr, 10)) : Value::Text(item->first);
            ctx.Write(*position, key);
        }
    });
    if (!position) {
        *placed = true;
        return;
    }
    // Setting the field scrolls to its item.
    Reactive::Run([scroll, position, ctx, placed, travelling, scrollTo, nearest] {
        auto key = R::KeyText(ctx.Read(*position));
        Reactive::Untrack([&] {
            if (!*placed) return 0;
            for (auto const& [itemKey, item] : ItemsOf(*scroll)) {
                if (itemKey != key) continue;
                auto at = nearest();
                if (at && at->first == key) return 0;
                *travelling = winrt::make_weak(item);
                scrollTo(item, true);
            }
            return 0;
        });
    });
    // The first display starts at the field's item, without a nudge.
    auto token = std::make_shared<winrt::event_token>();
    *token = scroller.LayoutUpdated([weak = winrt::make_weak(scroller), token, scroll, position, ctx, placed, scrollTo, padding](auto&&, auto&&) {
        auto viewer = weak.get();
        if (!viewer || viewer.ViewportWidth() <= 0 || ItemsOf(*scroll).empty()) return;
        viewer.LayoutUpdated(*token);
        padding();
        Host::Post([scroll, position, ctx, placed, scrollTo] {
            auto key = R::KeyText(ctx.Peek(*position));
            for (auto const& [itemKey, item] : ItemsOf(*scroll))
                if (itemKey == key) scrollTo(item, false);
            Host::After(std::chrono::milliseconds(100), [placed] { *placed = true; });
        });
    });
}

// ---------------------------------------------------------------------------
// The open panel

/// The file types of the open panel for `kinds`: extensions with the dot,
/// or `*` for any file.
inline std::vector<std::wstring> PanelTypes(char const* kinds) {
    std::vector<std::wstring> types;
    for (auto const& word : FileWords(kinds)) {
        if (word == "any" || word == "folder") return {L"*"};
        if (word[0] == '.') {
            types.push_back(std::wstring(Wide(word)));
            continue;
        }
        for (auto const& [kind, extensions] : app->catalog.fileKinds)
            if (kind == word)
                for (auto const& ext : extensions) types.push_back(L"." + std::wstring(Wide(ext)));
    }
    if (types.empty()) types.push_back(L"*");
    return types;
}

/// The system's open panel for `kinds`, on the context's window; `give`
/// runs with the chosen file's URL, nothing on cancel.
inline winrt::fire_and_forget PickFile(Ctx ctx, char const* kinds, std::function<void(std::string const&)> give) {
    auto hwnd = ctx.win ? ctx.win->hwnd : nullptr;
    auto alive = ctx.win ? std::weak_ptr<bool>(ctx.win->life) : std::weak_ptr<bool>();
    try {
        bool folder = false;
        for (auto const& word : FileWords(kinds)) folder = folder || word == "folder";
        std::string url;
        if (folder) {
            winrt::Windows::Storage::Pickers::FolderPicker picker;
            picker.as<IInitializeWithWindow>()->Initialize(hwnd);
            picker.FileTypeFilter().Append(L"*");
            auto chosen = co_await picker.PickSingleFolderAsync();
            if (chosen) url = Host::FileUrl(std::wstring(chosen.Path()));
        } else {
            winrt::Windows::Storage::Pickers::FileOpenPicker picker;
            picker.as<IInitializeWithWindow>()->Initialize(hwnd);
            for (auto const& type : PanelTypes(kinds)) picker.FileTypeFilter().Append(type);
            auto chosen = co_await picker.PickSingleFileAsync();
            if (chosen) url = Host::FileUrl(std::wstring(chosen.Path()));
        }
        if (!url.empty() && (!ctx.win || !alive.expired())) give(url);
    } catch (winrt::hresult_error const& error) {
        Report("open panel: " + Utf8(error.message()));
    }
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts

/// A key of a shortcut literal as a virtual key.
inline std::optional<winrt::Windows::System::VirtualKey> KeyOf(std::string const& key) {
    using winrt::Windows::System::VirtualKey;
    static std::map<std::string, VirtualKey> const named{
        {"space", VirtualKey::Space}, {"escape", VirtualKey::Escape}, {"return", VirtualKey::Enter}, {"tab", VirtualKey::Tab},
        {"delete", VirtualKey::Back}, {"up", VirtualKey::Up}, {"down", VirtualKey::Down}, {"left", VirtualKey::Left},
        {"right", VirtualKey::Right}, {"home", VirtualKey::Home}, {"end", VirtualKey::End}, {"pageup", VirtualKey::PageUp},
        {"pagedown", VirtualKey::PageDown},
    };
    if (auto found = named.find(key); found != named.end()) return found->second;
    if (key.size() != 1) return std::nullopt;
    char c = key[0];
    if (c >= 'a' && c <= 'z') return static_cast<VirtualKey>('A' + (c - 'a'));
    if (c >= '0' && c <= '9') return static_cast<VirtualKey>(c);
    static std::map<char, int> const symbols{{',', VK_OEM_COMMA}, {'.', VK_OEM_PERIOD}, {'-', VK_OEM_MINUS}, {'=', VK_OEM_PLUS},
        {';', VK_OEM_1}, {'/', VK_OEM_2}, {'`', VK_OEM_3}, {'[', VK_OEM_4}, {'\\', VK_OEM_5}, {']', VK_OEM_6}, {'\'', VK_OEM_7}};
    if (auto found = symbols.find(c); found != symbols.end()) return static_cast<VirtualKey>(found->second);
    return std::nullopt;
}

/// While mounted, `keys` (`cmd` is Control on Windows) calls `invoked`,
/// which says whether it handled them, as a keyboard accelerator of the
/// window. Without Control or Alt the keys are text while a text field has
/// the focus.
inline void Accelerator(Ctx const& ctx, char const* keys, std::function<bool()> invoked) {
    if (!ctx.win || !ctx.win->root) return;
    using winrt::Windows::System::VirtualKeyModifiers;
    std::string all = keys ? keys : "";
    std::vector<std::string> parts;
    std::string part;
    for (char c : all + "+") {
        if (c == '+') {
            if (!part.empty()) parts.push_back(part);
            part.clear();
        } else {
            part += c;
        }
    }
    if (parts.empty()) return;
    auto key = KeyOf(parts.back());
    if (!key) return;
    auto modifiers = VirtualKeyModifiers::None;
    for (size_t i = 0; i + 1 < parts.size(); ++i) {
        if (parts[i] == "cmd" || parts[i] == "ctrl") modifiers |= VirtualKeyModifiers::Control;
        else if (parts[i] == "alt") modifiers |= VirtualKeyModifiers::Menu;
        else if (parts[i] == "shift") modifiers |= VirtualKeyModifiers::Shift;
    }
    xaml::Input::KeyboardAccelerator accelerator;
    accelerator.Key(*key);
    accelerator.Modifiers(modifiers);
    bool typed = (modifiers & (VirtualKeyModifiers::Control | VirtualKeyModifiers::Menu)) == VirtualKeyModifiers::None;
    accelerator.Invoked([invoked, typed, root = winrt::make_weak(ctx.win->root)](auto&&, xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args) {
        if (typed)
            if (auto element = root.get(); element && element.XamlRoot()) {
                auto focused = xaml::Input::FocusManager::GetFocusedElement(element.XamlRoot());
                if (focused && (focused.try_as<controls::TextBox>() || focused.try_as<controls::PasswordBox>())) return;
            }
        if (invoked()) args.Handled(true);
    });
    auto root = ctx.win->root;
    root.KeyboardAccelerators().Append(accelerator);
    Reactive::OnCleanup([weak = winrt::make_weak(root), accelerator] {
        auto root = weak.get();
        if (!root) return;
        uint32_t index;
        if (root.KeyboardAccelerators().IndexOf(accelerator, index)) root.KeyboardAccelerators().RemoveAt(index);
    });
}

/// While mounted, `keys` runs the `action` (`activate` or `focus`) of the
/// control `id` in this context, as the external entries do; a split
/// button's flips its toggle. A disabled control's shortcut does nothing.
inline void Shortcut(Ctx const& ctx, char const* keys, char const* id, char const* action) {
    auto target = ctx.Target(id);
    std::string kind = action ? action : "activate";
    Accelerator(ctx, keys, [target, kind] {
        for (auto const& [token, entry] : Controls::entries) {
            if (entry.target != target) continue;
            if (entry.enabled && !Reactive::Untrack([&] { return entry.enabled(); })) return false;
            auto const& actions = entry.actions;
            if (kind == "focus" && actions.focus) actions.focus();
            // A split button's shortcut flips its toggle, as a switch's does.
            else if (entry.role == "split_button" && actions.setBool && actions.value) actions.setBool(actions.value() != "true");
            else if (actions.activate) actions.activate();
            return true;
        }
        return false;
    });
}

struct KeyShortcutProps {
    char const* keys = nullptr;
    Get enabled;
    Act action;
};

/// `Shortcut`: its keys run the action while it is mounted and enabled,
/// without a control on screen.
inline void KeyShortcut(Parent const&, Ctx const& ctx, KeyShortcutProps props) {
    auto enabled = props.enabled;
    auto action = props.action;
    if (!action) return;
    Accelerator(ctx, props.keys, [ctx, enabled, action] {
        if (enabled && !Reactive::Untrack([&] { return R::Truthy(enabled(ctx)); })) return false;
        action(ctx);
        return true;
    });
}

// ---------------------------------------------------------------------------
// Popover and split buttons

/// The flyout of content built while it shows, unmounted when it closes.
struct Popover {
    controls::Flyout flyout;
    std::unique_ptr<Scope> owner;
    controls::Panel content{nullptr};
    bool open = false;

    void Mount(Build const& build, Ctx const& ctx) {
        Unmount();
        content = MakeFlex(false, 12, xaml::Thickness{4, 4, 4, 4});
        content.MinWidth(240);
        owner = std::make_unique<Scope>();
        owner->Run([&] { build(Parent::Of(content), ctx); });
        flyout.Content(content);
    }

    void Unmount() {
        if (owner) owner->Dispose();
        owner.reset();
        if (content) content.Children().Clear();
        content = nullptr;
        flyout.Content(nullptr);
    }
};

inline std::shared_ptr<Popover> MakePopover(Ctx const& ctx, Build content) {
    auto popover = std::make_shared<Popover>();
    if (auto style = PresenterTheme(L"FlyoutPresenter", ctx.appearance)) popover->flyout.FlyoutPresenterStyle(style);
    std::weak_ptr<Popover> weak = popover;
    popover->flyout.Opening([weak, content, ctx](auto&&, auto&&) {
        if (auto popover = weak.lock()) popover->Mount(content, ctx);
    });
    popover->flyout.Opened([weak](auto&&, auto&&) {
        if (auto popover = weak.lock()) popover->open = true;
    });
    popover->flyout.Closed([weak](auto&&, auto&&) {
        if (auto popover = weak.lock()) {
            popover->open = false;
            // After the closing animation has used the content.
            Host::Post([weak] {
                if (auto popover = weak.lock(); popover && !popover->open) popover->Unmount();
            });
        }
    });
    Reactive::OnCleanup([popover] {
        popover->flyout.Hide();
        popover->Unmount();
    });
    return popover;
}

struct PopoverButtonProps {
    char const* id = "";
    Str label;
    char const* icon = nullptr;
    std::optional<Field> presented;
};

/// A button that shows its children in a popover, such as a volume slider.
/// The children mount while the popover shows. `presented` follows the
/// popover and opens or closes it; external entries open it with
/// `activate`.
inline void PopoverButton(Parent const& p, Ctx const& ctx, PopoverButtonProps props, Build const& content) {
    controls::Button button;
    auto text = Bound(ctx, props.label);
    if (props.icon) {
        button.Content(Icons::Icon(props.icon));
        Styles::MakeSubtle(button);
    } else {
        button.Content(TextOf(text));
    }
    if (text)
        Reactive::Run([button, text] {
            auto name = text();
            controls::ToolTipService::SetToolTip(button, winrt::box_value(name));
            xaml::Automation::AutomationProperties::SetName(button, name);
        });
    auto popover = MakePopover(ctx, content);
    button.Flyout(popover->flyout);
    auto presented = props.presented;
    if (presented) {
        auto field = *presented;
        popover->flyout.Opened([ctx, field](auto&&, auto&&) { SetDraft(ctx, field, Value::Bool(true)); });
        popover->flyout.Closed([ctx, field](auto&&, auto&&) { SetDraft(ctx, field, Value::Bool(false)); });
        Reactive::Run([ctx, field, popover, weak = winrt::make_weak(button)] {
            bool want = ctx.Read(field) == Value::Bool(true);
            Reactive::Untrack([&] {
                auto button = weak.get();
                if (!button) return 0;
                // Presented from the start: open once the button is shown.
                if (want && !popover->open) {
                    if (button.IsLoaded()) popover->flyout.ShowAt(button);
                    else button.Loaded([popover](IInspectable const& sender, auto&&) { popover->flyout.ShowAt(sender.as<xaml::FrameworkElement>()); });
                } else if (!want && popover->open) {
                    popover->flyout.Hide();
                }
                return 0;
            });
        });
    }
    Controls::Register(ctx, props.id, "popover_button", text, nullptr,
        Actions{.activate = [popover, weak = winrt::make_weak(button)] {
                    if (auto button = weak.get()) popover->flyout.ShowAt(button);
                },
            .value = [popover] { return std::optional<std::string>(popover->open ? "open" : "closed"); }});
    KeepWidth(button);
    if (p.isList && !ctx.toolbar) p.Add(Card(button));
    else p.Add(button);
}

struct SplitButtonProps {
    char const* id = "";
    Str label;
    char const* icon = nullptr;
    std::optional<Field> field;
    Str more;
    std::optional<ControlShape> shape;
};

/// A toggle linked with a part that shows `content` in a popover, such as
/// the settings of what it turns on: the toggle split button of Windows.
/// External entries turn it on or off with `set_bool` and open the popover
/// with `activate`.
inline void SplitButton(Parent const& p, Ctx const& ctx, SplitButtonProps props, Build const& content) {
    if (!props.field) return;
    auto field = *props.field;
    controls::ToggleSplitButton button;
    auto text = Bound(ctx, props.label);
    // Its icon and label, as the design shows it; a toolbar's icon alone.
    if (props.icon && ctx.toolbar) button.Content(Icons::Icon(props.icon));
    else button.Content(Content(text, props.icon));
    if (text)
        Reactive::Run([button, text] {
            auto name = text();
            controls::ToolTipService::SetToolTip(button, winrt::box_value(name));
            xaml::Automation::AutomationProperties::SetName(button, name);
        });
    auto updating = std::make_shared<bool>(false);
    Reactive::Run([button, ctx, field, updating] {
        bool on = ctx.Read(field) == Value::Bool(true);
        if (button.IsChecked() != on) {
            *updating = true;
            button.IsChecked(on);
            *updating = false;
        }
    });
    button.IsCheckedChanged([updating, ctx, field](controls::ToggleSplitButton const& sender, auto&&) {
        if (!*updating) SetDraft(ctx, field, Value::Bool(sender.IsChecked()));
    });
    ShapeControl(ctx, button, props.shape);
    auto popover = MakePopover(ctx, content);
    button.Flyout(popover->flyout);
    // The part that opens the popover is named for screen readers.
    auto more = Bound(ctx, props.more);
    button.Loaded([more, weak = winrt::make_weak(button)](auto&&, auto&&) {
        auto button = weak.get();
        if (!button) return;
        auto part = TemplatePart(button, L"SecondaryButton");
        if (!part) return;
        if (more) Reactive::Run([part, more] { xaml::Automation::AutomationProperties::SetName(part, more()); });
        else xaml::Automation::AutomationProperties::SetName(part, bern::Text::Tr("bern.split.more"));
    });
    Controls::Register(ctx, props.id, "split_button", text, nullptr,
        Actions{.activate = [popover, weak = winrt::make_weak(button)] {
                    if (auto button = weak.get()) popover->flyout.ShowAt(button);
                },
            .setBool = [ctx, field](bool on) { SetDraft(ctx, field, Value::Bool(on)); },
            .value = [ctx, field] { return std::optional<std::string>(ctx.Peek(field) == Value::Bool(true) ? "true" : "false"); }});
    KeepWidth(button);
    if (p.isList && !ctx.toolbar) p.Add(Card(button));
    else p.Add(button);
}

// ---------------------------------------------------------------------------
// Notices

struct ToastProps {
    Str content;
    char const* icon = nullptr;
    Get key;
    std::optional<double> duration;
    Motion const* motion = nullptr;
};

/// A short notice that shows when it appears or `key` changes and hides
/// itself after `duration` seconds, read out politely: the system's
/// InfoBar where it stands.
inline void Toast(Parent const& p, Ctx const& ctx, ToastProps props) {
    controls::Grid place;
    place.HorizontalAlignment(xaml::HorizontalAlignment::Center);
    p.Add(place);
    auto shown = std::make_shared<xaml::UIElement>(nullptr);
    auto hide = [shown, place] {
        auto node = *shown;
        *shown = nullptr;
        if (!node) return;
        static Motion const away{0.25, 1};
        LayerTo(node, "enter", 0, 16, 1, 1, 0, Reduced() ? nullptr : &away, [place, node] {
            uint32_t index;
            if (place.Children().IndexOf(node, index)) place.Children().RemoveAt(index);
        });
    };
    auto generation = std::make_shared<int64_t>(0);
    auto content = props.content;
    auto key = props.key;
    double duration = props.duration.value_or(3);
    auto motion = props.motion;
    std::string icon = props.icon ? props.icon : "";
    Reactive::Run([ctx, content, key, shown, place, hide, generation, duration, motion, icon] {
        if (key) key(ctx);
        auto text = content ? content(ctx) : winrt::hstring();
        Reactive::Untrack([&] {
            if (*shown) {
                uint32_t index;
                if (place.Children().IndexOf(*shown, index)) place.Children().RemoveAt(index);
            }
#if BERN_WINUI
            // The system's notice bar.
            controls::InfoBar notice;
            notice.Severity(controls::InfoBarSeverity::Informational);
            notice.IsClosable(false);
            notice.Message(text);
            if (!icon.empty()) {
                controls::FontIconSource glyph;
                glyph.Glyph(Icons::Glyph(icon));
                notice.IconSource(glyph);
            }
            notice.IsOpen(true);
#else
            // The system XAML has no InfoBar: a bar of its shape.
            controls::Border notice;
            notice.CornerRadius(xaml::CornerRadius{4, 4, 4, 4});
            notice.Padding(xaml::Thickness{12, 10, 12, 10});
            notice.BorderThickness(xaml::Thickness{1, 1, 1, 1});
            if (auto style = Styles::Themed(L"Border", {{L"Background", L"SystemControlBackgroundChromeMediumLowBrush"}, {L"BorderBrush", Keys::Divider}}))
                notice.Style(style);
            controls::StackPanel row;
            row.Orientation(controls::Orientation::Horizontal);
            row.Spacing(10);
            if (!icon.empty()) row.Children().Append(Icons::Icon(icon));
            controls::TextBlock label;
            label.Text(text);
            label.TextWrapping(xaml::TextWrapping::Wrap);
            label.VerticalAlignment(xaml::VerticalAlignment::Center);
            row.Children().Append(label);
            notice.Child(row);
#endif
            xaml::Automation::AutomationProperties::SetLiveSetting(notice, xaml::Automation::Peers::AutomationLiveSetting::Polite);
            xaml::Automation::AutomationProperties::SetName(notice, text);
            auto wrapper = Wrap(notice);
            place.Children().Append(wrapper);
            *shown = wrapper;
            LayerSet(wrapper, "enter", 0, 24, 1, 1, 0);
            LayerTo(wrapper, "enter", 0, 0, 1, 1, 1, motion);
            if (auto peer = xaml::Automation::Peers::FrameworkElementAutomationPeer::FromElement(notice))
                peer.RaiseAutomationEvent(xaml::Automation::Peers::AutomationEvents::LiveRegionChanged);
            auto mine = ++*generation;
            Host::After(std::chrono::milliseconds(static_cast<int64_t>(duration * 1000)), [generation, mine, hide] {
                if (*generation == mine) hide();
            });
            return 0;
        });
    });
    Reactive::OnCleanup([generation] { ++*generation; });
}

// ---------------------------------------------------------------------------
// The inspector

struct InspectorProps {
    std::optional<Field> presented;
    std::optional<double> width;
};

/// A side panel at the trailing edge of the container it stands in, shown
/// while `presented`: a split view's pane that pushes the container's other
/// content aside, sliding in and out.
inline void Inspector(Parent const& p, Ctx const& ctx, InspectorProps props, Build const& content, Build const& inspector) {
    controls::SplitView split;
    split.PanePlacement(controls::SplitViewPanePlacement::Right);
    split.DisplayMode(controls::SplitViewDisplayMode::Inline);
    split.OpenPaneLength(props.width.value_or(280));
    MarkFill(split);
    auto main = MakeFlex();
    MarkFill(main);
    content(Parent::Of(main), ctx);
    split.Content(main);
    auto pane = MakeFlex(false, 12, xaml::Thickness{16, 16, 16, 16});
    controls::ScrollViewer scroller;
    scroller.Content(pane);
    scroller.HorizontalScrollBarVisibility(controls::ScrollBarVisibility::Disabled);
    inspector(Parent::Aligned(pane, xaml::HorizontalAlignment::Stretch, std::nullopt), ctx);
    split.Pane(scroller);
    if (auto style = Styles::Themed(L"SplitView", {{L"PaneBackground", kWinUI ? L"LayerFillColorDefaultBrush" : L"SystemControlBackgroundChromeMediumLowBrush"}}))
        split.Style(style);
    if (props.presented) {
        auto field = *props.presented;
        auto updating = std::make_shared<bool>(false);
        Reactive::Run([split, ctx, field, updating] {
            bool open = ctx.Read(field) == Value::Bool(true);
            if (split.IsPaneOpen() != open) {
                *updating = true;
                split.IsPaneOpen(open);
                *updating = false;
            }
        });
        split.PaneClosed([ctx, field, updating](auto&&, auto&&) {
            if (!*updating) SetDraft(ctx, field, Value::Bool(false));
        });
    }
    p.Add(split);
}

// ---------------------------------------------------------------------------
// Drop areas and gestures

/// The one child of a gesture or drop area, in a panel that fills the way
/// the child does.
inline controls::Grid Area(Ctx const& ctx, Build const& content) {
    controls::Grid area;
    auto inner = MakeFlex();
    content(Parent::Of(inner), ctx);
    area.Children().Append(inner);
    Mirror(area, inner);
    // The whole area takes the pointer, not only what it shows.
    area.Background(media::SolidColorBrush(winrt::Windows::UI::Color{0, 0, 0, 0}));
    return area;
}

struct DropProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    char const* files = nullptr;
    std::optional<Field> over;
    bool opens = false;
    Act action;
};

/// Take the first dropped file of `files`, once the drag says which.
inline winrt::fire_and_forget TakeDrop(winrt::Windows::ApplicationModel::DataTransfer::DataPackageView view, xaml::DragOperationDeferral deferral,
    std::function<void(std::string const&)> give, char const* files) {
    winrt::apartment_context ui;
    try {
        auto items = co_await view.GetStorageItemsAsync();
        co_await ui;
        for (auto const& item : items) {
            auto url = Host::FileUrl(std::wstring(item.Path()));
            if (!url.empty() && Accepts(url, files)) {
                give(url);
                break;
            }
        }
    } catch (winrt::hresult_error const& error) {
        Report("drop: " + Utf8(error.message()));
    }
    deferral.Complete();
}

/// Its content accepts a file dragged onto it: while one of `files` is
/// over it `over` is true; dropping stores the file in `field` and runs the
/// action. With `opens`, files the system opens the application with
/// arrive here too.
inline void Drop(Parent const& p, Ctx const& ctx, DropProps props, Build const& content) {
    if (!props.field) return;
    auto field = *props.field;
    auto area = Area(ctx, content);
    area.AllowDrop(true);
    auto run = Run(ctx, props.action);
    auto files = props.files;
    std::function<void(std::string const&)> give = [ctx, field, run](std::string const& url) {
        GiveFile(ctx, field, url);
        if (run) run();
    };
    auto over = props.over;
    auto setOver = [ctx, over](bool value) {
        if (over) SetDraft(ctx, *over, Value::Bool(value));
    };
    using namespace winrt::Windows::ApplicationModel::DataTransfer;
    area.DragEnter([setOver](auto&&, xaml::DragEventArgs const& args) {
        if (!args.DataView().Contains(StandardDataFormats::StorageItems())) return;
        args.AcceptedOperation(DataPackageOperation::Copy);
        setOver(true);
    });
    area.DragOver([](auto&&, xaml::DragEventArgs const& args) {
        if (args.DataView().Contains(StandardDataFormats::StorageItems())) args.AcceptedOperation(DataPackageOperation::Copy);
    });
    area.DragLeave([setOver](auto&&, auto&&) { setOver(false); });
    area.Drop([setOver, give, files](auto&&, xaml::DragEventArgs const& args) {
        setOver(false);
        auto view = args.DataView();
        if (!view.Contains(StandardDataFormats::StorageItems())) return;
        TakeDrop(view, args.GetDeferral(), give, files);
    });
    auto text = Bound(ctx, props.label);
    BindName(area, text);
    if (props.opens) {
        // Files opened with the application: those it started with, then
        // those later launches hand over.
        auto take = [give, files](std::vector<std::string> const& urls) {
            for (auto const& url : urls)
                if (Accepts(url, files)) {
                    give(url);
                    return true;
                }
            return false;
        };
        if (!Host::openedFiles.empty()) {
            auto urls = Host::openedFiles;
            Host::openedFiles.clear();
            Host::Post([take, urls] { take(urls); });
        }
        auto previous = Host::onOpen;
        Host::onOpen = [take, previous](std::vector<std::string> const& urls) {
            if (take(urls)) Host::openedFiles.clear();
            else if (previous) previous(urls);
        };
        Reactive::OnCleanup([previous] { Host::onOpen = previous; });
    }
    Controls::Register(ctx, props.id, "drop", text, nullptr,
        Actions{.value = [ctx, field] { return R::Url(ctx.Peek(field)); }, .setFile = give, .files = files ? files : "any"});
    p.Add(area);
}

struct GestureProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    double min = 0, max = 1;
    char const* axis = nullptr;
    std::optional<double> distance;
    Act action;
};

/// Pinching the content — two fingers, or a touchpad pinch, which arrives
/// as a wheel with Control — scales `field` within `min` and `max`; the
/// action runs when the gesture ends.
inline void Pinch(Parent const& p, Ctx const& ctx, GestureProps props, Build const& content) {
    if (!props.field) return;
    auto field = *props.field;
    auto area = Area(ctx, content);
    double low = props.min, high = props.max;
    auto run = Run(ctx, props.action);
    auto set = [ctx, field, low, high](double value) { SetDraft(ctx, field, Value::Float((std::clamp)(value, low, high))); };
    auto settle = std::make_shared<int64_t>(0);
    area.AddHandler(xaml::UIElement::PointerWheelChangedEvent(), winrt::box_value(xaml::Input::PointerEventHandler([ctx, field, set, run, settle](IInspectable const& sender, xaml::Input::PointerRoutedEventArgs const& args) {
        // A touchpad pinch arrives as a wheel with Control, which the key
        // state shows even where the event's modifiers do not.
        bool control = (args.KeyModifiers() & winrt::Windows::System::VirtualKeyModifiers::Control) == winrt::Windows::System::VirtualKeyModifiers::Control ||
            GetKeyState(VK_CONTROL) < 0;
        if (!control) return;
        auto delta = args.GetCurrentPoint(sender.as<xaml::UIElement>()).Properties().MouseWheelDelta();
        set(R::AsDouble(ctx.Peek(field)) * std::exp(delta / 120.0 * 0.1));
        args.Handled(true);
        auto mine = ++*settle;
        Host::After(std::chrono::milliseconds(200), [settle, mine, run] {
            if (*settle == mine && run) run();
        });
    })), true);
    area.ManipulationMode(xaml::Input::ManipulationModes::Scale);
    auto start = std::make_shared<double>(0);
    area.ManipulationStarted([start, ctx, field](auto&&, auto&&) { *start = R::AsDouble(ctx.Peek(field)); });
    area.ManipulationDelta([start, set](auto&&, xaml::Input::ManipulationDeltaRoutedEventArgs const& args) {
        set(*start * args.Cumulative().Scale);
    });
    area.ManipulationCompleted([run](auto&&, auto&&) {
        if (run) run();
    });
    auto text = Bound(ctx, props.label);
    BindName(area, text);
    Controls::Register(ctx, props.id, "pinch", text, nullptr,
        Actions{.setNumber = [set, run](double value) {
                    set(value);
                    if (run) run();
                },
            .value = [ctx, field] { return std::optional<std::string>(R::Number(R::AsDouble(ctx.Peek(field)))); }});
    p.Add(area);
}

/// Dragging over the content along `axis` moves `field` through its range
/// over `distance` units (up and right increase it); the action runs when
/// the pointer lifts.
inline void Swipe(Parent const& p, Ctx const& ctx, GestureProps props, Build const& content) {
    if (!props.field) return;
    auto field = *props.field;
    auto area = Area(ctx, content);
    double low = props.min, high = props.max;
    bool vertical = !props.axis || std::string_view(props.axis) == "vertical";
    double reach = props.distance.value_or(300);
    auto run = Run(ctx, props.action);
    auto set = [ctx, field, low, high](double value) { SetDraft(ctx, field, Value::Float((std::clamp)(value, low, high))); };
    struct Drag {
        bool down = false, moving = false;
        double x = 0, y = 0, from = 0;
        uint32_t pointer = 0;
    };
    auto drag = std::make_shared<Drag>();
    using PointerHandler = xaml::Input::PointerEventHandler;
    area.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(PointerHandler([drag, ctx, field](IInspectable const& sender, xaml::Input::PointerRoutedEventArgs const& args) {
        auto point = args.GetCurrentPoint(sender.as<xaml::UIElement>());
        if (!point.Properties().IsLeftButtonPressed()) return;
        *drag = Drag{true, false, point.Position().X, point.Position().Y, R::AsDouble(ctx.Peek(field)), args.Pointer().PointerId()};
    })), true);
    area.AddHandler(xaml::UIElement::PointerMovedEvent(), winrt::box_value(PointerHandler([drag, vertical, reach, set, low, high](IInspectable const& sender, xaml::Input::PointerRoutedEventArgs const& args) {
        if (!drag->down || args.Pointer().PointerId() != drag->pointer) return;
        auto element = sender.as<xaml::UIElement>();
        auto at = args.GetCurrentPoint(element).Position();
        double moved = vertical ? drag->y - at.Y : at.X - drag->x;
        if (!drag->moving && std::abs(moved) < 10) return;
        if (!drag->moving) {
            drag->moving = true;
            element.CapturePointer(args.Pointer());
        }
        set(drag->from + moved / reach * (high - low));
        args.Handled(true);
    })), true);
    auto end = [drag, run](IInspectable const& sender, xaml::Input::PointerRoutedEventArgs const& args) {
        if (!drag->down) return;
        bool moved = drag->moving;
        drag->down = drag->moving = false;
        sender.as<xaml::UIElement>().ReleasePointerCapture(args.Pointer());
        if (moved && run) run();
    };
    area.AddHandler(xaml::UIElement::PointerReleasedEvent(), winrt::box_value(PointerHandler(end)), true);
    area.AddHandler(xaml::UIElement::PointerCanceledEvent(), winrt::box_value(PointerHandler(end)), true);
    auto text = Bound(ctx, props.label);
    BindName(area, text);
    Controls::Register(ctx, props.id, "swipe", text, nullptr,
        Actions{.setNumber = [set, run](double value) {
                    set(value);
                    if (run) run();
                },
            .value = [ctx, field] { return std::optional<std::string>(R::Number(R::AsDouble(ctx.Peek(field)))); }});
    p.Add(area);
}

}
