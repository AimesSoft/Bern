// Bern runtime for Windows, XAML part 5 of 7: conditional and repeated
// content, the containers that navigate, page headers, queries and video.
// Generated build input; do not edit.

namespace B {

/// Content that a builder replaces: the current scope and region.
struct Slot : std::enable_shared_from_this<Slot> {
    Parent region;
    std::unique_ptr<Scope> owner;

    explicit Slot(Parent target) : region(std::move(target)) {}

    static std::shared_ptr<Slot> In(Parent target) {
        auto slot = std::make_shared<Slot>(std::move(target));
        Reactive::OnCleanup([slot] { slot->End(); });
        return slot;
    }

    void End() {
        auto current = std::move(owner);
        if (current) current->Dispose();
    }

    /// Replace the content with what `build` makes, outside any effect.
    void Fill(std::function<void(Parent const&)> const& build) {
        Reactive::Untrack([&] {
            End();
            region.Clear();
            auto next = std::make_unique<Scope>();
            next->Run([&] { build(region); });
            owner = std::move(next);
            return 0;
        });
    }

    void Clear() {
        Reactive::Untrack([&] {
            End();
            region.Clear();
            return 0;
        });
    }
};

/// Conditional content: `branch` picks an arm, or none. Only a change of
/// arm rebuilds; the arm's own bindings update in place. What a change
/// after the first display mounts plays its `appear` transition.
inline void When(Parent const& p, Ctx const& ctx, Branch branch, std::vector<Build> arms) {
    auto slot = Slot::In(p.Sub());
    auto state = std::make_shared<std::pair<bool, std::optional<int>>>(false, std::nullopt);
    auto shared = std::make_shared<std::vector<Build>>(std::move(arms));
    Reactive::Run([slot, state, shared, branch, ctx] {
        auto next = branch(ctx);
        if (state->first && state->second == next) return;
        bool animated = state->first;
        state->first = true;
        state->second = next;
        Structure::Scope structural(animated);
        if (next && *next >= 0 && *next < static_cast<int>(shared->size())) {
            auto arm = (*shared)[*next];
            slot->Fill([&](Parent const& region) { arm(region, ctx); });
        } else {
            slot->Clear();
        }
    });
}

/// An arm builder that runs in a fixed context.
inline Build In(Build build, Ctx fixed) {
    return [build, fixed](Parent const& p, Ctx const&) { build(p, fixed); };
}

/// `if let Some(var) = value { then } else { otherwise }`.
inline void IfLet(Parent const& p, Ctx const& ctx, char const* variable, Get value, Build then, Build otherwise = nullptr) {
    auto inner = ctx.WithLocal(variable, [value, ctx] { return R::Some(value(ctx)).value_or(Value::Unit()); });
    std::vector<Build> arms{In(then, inner)};
    bool hasElse = static_cast<bool>(otherwise);
    if (hasElse) arms.push_back(otherwise);
    When(p, ctx, [value, hasElse](Ctx const& c) -> std::optional<int> {
        if (value(c).IsSome()) return 0;
        if (hasElse) return 1;
        return std::nullopt;
    }, std::move(arms));
}

/// One arm of a `match`: the variant (none for `_`), its content and the
/// names its payload fields bind.
struct MatchArm {
    std::optional<uint32_t> variant;
    Build build;
    std::vector<std::pair<char const*, int>> bindings;
};

inline MatchArm Arm(std::optional<uint32_t> variant, Build build, std::initializer_list<std::pair<char const*, int>> bindings = {}) {
    return MatchArm{variant, std::move(build), bindings};
}

inline void Match(Parent const& p, Ctx const& ctx, Get value, std::vector<MatchArm> arms) {
    std::vector<std::optional<uint32_t>> variants;
    std::vector<Build> builders;
    for (auto& arm : arms) {
        variants.push_back(arm.variant);
        auto inner = ctx;
        for (auto const& [name, field] : arm.bindings)
            inner = inner.WithLocal(name, [value, ctx, field] { return R::Payload(value(ctx), field); });
        builders.push_back(In(arm.build, inner));
    }
    When(p, ctx, [value, variants](Ctx const& c) -> std::optional<int> {
        auto tag = R::Tag(value(c));
        for (size_t i = 0; i < variants.size(); ++i)
            if (variants[i] && variants[i] == tag) return static_cast<int>(i);
        for (size_t i = 0; i < variants.size(); ++i)
            if (!variants[i]) return static_cast<int>(i);
        return std::nullopt;
    }, std::move(builders));
}

struct EachProps {
    Get items;
    int key = 0;
    char const* variable = "item";
    /// Rows added in one update appear one after another, this many
    /// seconds apart.
    std::optional<double> stagger;
    /// The container's children slide to their new places.
    Motion const* reflow = nullptr;
};

/// Keyed rows: a row keeps its elements, controls and state while its key
/// stays; its value is a cell the row's bindings read. Rows added after the
/// first display play their `appear` transition, in turn with `stagger`.
inline void Each(Parent const& p, Ctx const& ctx, EachProps props, Build row) {
    auto items = props.items;
    int key = props.key;
    char const* variable = props.variable;
    if (props.reflow && p.panel) SetReflow(p.panel, props.reflow);
    // Rows of one update take turns; a staggering list that appears in an
    // update plays its rows in turn.
    auto rowsCtx = props.stagger ? Staggered(ctx, *props.stagger) : ctx;
    auto first = std::make_shared<bool>(true);
    struct Row {
        std::string key;
        Cell<Value> value;
        std::unique_ptr<Scope> owner;
        Parent region;
    };
    auto region = p.Sub();
    auto rows = std::make_shared<std::vector<std::shared_ptr<Row>>>();
    Reactive::OnCleanup([rows] {
        for (auto& existing : *rows) existing->owner->Dispose();
        rows->clear();
    });
    std::string name = variable;
    Reactive::Run([region, rows, items, key, name, row, ctx = rowsCtx, first] {
        auto list = R::AsList(items(ctx));
        bool animated = !*first;
        *first = false;
        Reactive::Untrack([&] {
            Structure::Scope structural(animated);
            std::vector<std::string> orderBefore;
            std::unordered_map<std::string, std::shared_ptr<Row>> old;
            for (auto& existing : *rows) {
                orderBefore.push_back(existing->key);
                old[existing->key] = existing;
            }
            std::vector<std::shared_ptr<Row>> next;
            for (auto const& item : list) {
                auto id = R::KeyText(R::At(item, {key}));
                if (auto found = old.find(id); found != old.end()) {
                    found->second->value.Set(item);
                    next.push_back(found->second);
                    old.erase(found);
                    continue;
                }
                auto created = std::make_shared<Row>(Row{id, Cell<Value>(item), std::make_unique<Scope>(), region.Sub()});
                auto cell = created->value;
                auto rowCtx = ctx.WithKey([id] { return id; }).WithLocal(name, [cell] { return cell.Get(); });
                created->owner->Run([&] { row(created->region, rowCtx); });
                // The row's element, known by its key: scroll views find
                // their items this way.
                if (ctx.scroll) {
                    std::vector<xaml::UIElement> elements;
                    created->region.region->Elements(elements, false);
                    if (!elements.empty()) ctx.scroll->items.emplace_back(id, winrt::make_weak(elements.front()));
                }
                next.push_back(created);
            }
            for (auto& [id, stale] : old) {
                stale->owner->Dispose();
                region.Remove(stale->region);
            }
            // New rows were added at the end; reorder only when the kept rows
            // changed order.
            std::vector<std::string> kept;
            for (auto const& id : orderBefore)
                if (std::any_of(next.begin(), next.end(), [&](auto const& r) { return r->key == id; })) kept.push_back(id);
            bool inOrder = kept.size() <= next.size();
            for (size_t i = 0; inOrder && i < kept.size(); ++i) inOrder = next[i]->key == kept[i];
            if (!inOrder) {
                std::vector<Parent> order;
                for (auto const& r : next) order.push_back(r->region);
                region.Reorder(order);
            }
            *rows = std::move(next);
            return 0;
        });
    });
}

/// A list of rows; inside a form section the rows join the section.
inline void List(Parent const& p, Ctx const& ctx, Build const& children) {
    if (p.isList) {
        children(p, ctx);
        return;
    }
    controls::StackPanel rows;
    rows.Spacing(kWinUI ? 4 : 2);
    children(Parent::OfList(rows), ctx);
    p.Add(rows);
}

/// A page header: the innermost title or title widget, a back button while
/// something can go back, and the page's tools.
inline controls::Grid Header(std::shared_ptr<Page> page, TextFn fallback, bool showTitle) {
    controls::Grid grid;
    grid.Margin(xaml::Thickness{0, 0, 0, 8});
    grid.ColumnDefinitions().Append(Column(AutoLength()));
    grid.ColumnDefinitions().Append(Column(Star()));
    grid.ColumnDefinitions().Append(Column(AutoLength()));
    controls::Button back;
    back.Content(Icons::Icon("arrow.left", 16));
    back.VerticalAlignment(xaml::VerticalAlignment::Center);
    back.Margin(xaml::Thickness{0, 0, 8, 0});
    back.Padding(xaml::Thickness{10, 8, 10, 8});
    Styles::MakeSubtle(back);
    std::weak_ptr<Page> weak = page;
    back.Click([weak](auto&&, auto&&) {
        if (auto page = weak.lock())
            if (auto action = page->Back()) action();
    });
    controls::TextBlock title;
    if (auto style = Styles::Named(L"TitleTextBlockStyle")) title.Style(style);
    title.VerticalAlignment(xaml::VerticalAlignment::Center);
    title.TextTrimming(xaml::TextTrimming::CharacterEllipsis);
    controls::ContentControl holder;
    holder.HorizontalContentAlignment(xaml::HorizontalAlignment::Stretch);
    holder.VerticalAlignment(xaml::VerticalAlignment::Center);
    // The way back and the leading tools share the first column.
    controls::StackPanel leading;
    leading.Orientation(controls::Orientation::Horizontal);
    leading.VerticalAlignment(xaml::VerticalAlignment::Center);
    leading.Children().Append(back);
    page->leadingTools.VerticalAlignment(xaml::VerticalAlignment::Center);
    page->leadingTools.Margin(xaml::Thickness{0, 0, 8, 0});
    leading.Children().Append(page->leadingTools);
    controls::Grid::SetColumn(holder, 1);
    controls::Grid::SetColumn(page->tools, 2);
    page->tools.VerticalAlignment(xaml::VerticalAlignment::Center);
    page->tools.Margin(xaml::Thickness{8, 0, 0, 0});
    grid.Children().Append(leading);
    grid.Children().Append(holder);
    grid.Children().Append(page->tools);
    Reactive::Run([grid, back, title, holder, weak, fallback, showTitle] {
        auto page = weak.lock();
        if (!page) return;
        page->version.Track();
        auto backLabel = bern::Text::Tr("bern.navigation.back");
        controls::ToolTipService::SetToolTip(back, winrt::box_value(backLabel));
        xaml::Automation::AutomationProperties::SetName(back, backLabel);
        back.Visibility(page->Back() ? xaml::Visibility::Visible : xaml::Visibility::Collapsed);
        if (auto widget = page->TitleWidget()) {
            if (holder.Content() != widget) holder.Content(widget);
        } else {
            auto text = page->Title();
            if (!text && showTitle && fallback) text = fallback();
            title.Text(text.value_or(L""));
            if (holder.Content() != title) holder.Content(title);
        }
        bool empty = !page->Back() && !page->TitleWidget() && page->tools.Children().Size() == 0 && page->leadingTools.Children().Size() == 0 &&
            title.Text().empty();
        // An immersive window shows no title bar or toolbar.
        bool immersive = page->win && page->win->immersive.Get();
        grid.Visibility(page->HeaderHidden() || empty || immersive ? xaml::Visibility::Collapsed : xaml::Visibility::Visible);
    });
    return grid;
}

/// The sidebar's destinations, which may come and go with `if`, `match`
/// and `ForEach`: each item registers while it is mounted.
struct SidebarRegistry {
    struct Entry {
        int64_t token = 0;
        Value value;
        std::string name;
        TextFn label;
        controls::NavigationViewItem item{nullptr};
        std::shared_ptr<StyleState> state;
        std::function<void()> restyle;
    };
    std::vector<Entry> entries;
    controls::NavigationView view{nullptr};
    Ctx ctx;
    Field selection;
    Get fallback;
    bern::Version version;
    int64_t next = 1;
    bool updating = false;
    bool settling = false;

    /// Mounted items in the order they show.
    std::vector<Entry const*> Ordered() const {
        std::vector<std::pair<uint32_t, Entry const*>> found;
        for (auto const& entry : entries) {
            uint32_t index = 0;
            if (view.MenuItems().IndexOf(entry.item, index)) found.emplace_back(index, &entry);
        }
        std::sort(found.begin(), found.end(), [](auto const& a, auto const& b) { return a.first < b.first; });
        std::vector<Entry const*> ordered;
        for (auto const& [index, entry] : found) ordered.push_back(entry);
        return ordered;
    }

    Entry const* Selected(Value const& value) const {
        for (auto const& entry : entries)
            if (entry.value == value) return &entry;
        return nullptr;
    }

    /// The navigation view slides its pill from the old item to the new
    /// only where the two pills line up, measured with each item's move and
    /// scale; otherwise both pills shrink and grow at once. While it
    /// measures, as the selection changes, the items stand at rest; they
    /// are back before the frame is drawn.
    bool resting = false;
    void Rest(bool rest) {
        resting = rest;
        for (auto const& entry : entries)
            if (auto layers = ExistingLayers(entry.item)) layers->Rest(rest);
    }

    /// A selection without a mounted item moves on to `fallback` or the
    /// first item left, once the update that removed it has mounted
    /// whatever replaces it.
    void Settle() {
        settling = false;
        if (entries.empty()) return;
        auto current = ctx.Peek(selection);
        if (Selected(current)) return;
        std::optional<Value> back;
        if (fallback) {
            auto wanted = Reactive::Untrack([&] { return fallback(ctx); });
            if (Selected(wanted)) back = wanted;
        }
        if (!back) {
            auto ordered = Ordered();
            if (!ordered.empty()) back = ordered.front()->value;
        }
        if (back) ctx.Write(selection, *back);
    }
};

struct SidebarItemProps {
    Get value;
    std::variant<char const*, std::function<std::string(Ctx const&)>> name = "";
    Str label;
    char const* icon = nullptr;
    StyleDef const* style = nullptr;
    /// A live count beside the page, hidden at zero.
    Get badge;
};

/// The lightweight resources of a styled sidebar item, in every state.
inline std::vector<std::pair<std::wstring, Paint>> SidebarResources(StyleDef const& style) {
    std::vector<std::pair<std::wstring, Paint>> entries;
    struct State {
        wchar_t const* suffix;
        bool selected, hover, pressed;
    };
    for (auto const& state : {State{L"", false, false, false}, State{L"PointerOver", false, true, false}, State{L"Pressed", false, true, true},
             State{L"Selected", true, false, false}, State{L"SelectedPointerOver", true, true, false}, State{L"SelectedPressed", true, true, true}}) {
        bool quiet = false, system = false;
        auto look = Merged(style, state.selected, state.hover, state.pressed, false, false, quiet, system);
        if (system && state.suffix[0] != 0) continue;
        if (quiet) look = Merged(style, state.selected, false, false, false, false, quiet, system);
        auto background = look.fill ? look.fill : look.background;
        auto foreground = look.foreground ? look.foreground : PairOf(background);
        if (background) entries.emplace_back(std::wstring(L"NavigationViewItemBackground") + state.suffix, *background);
        if (foreground) entries.emplace_back(std::wstring(L"NavigationViewItemForeground") + state.suffix, *foreground);
    }
    // A filled selection needs no pill beside it.
    if (style.selected && style.selected->fill) entries.emplace_back(L"NavigationViewSelectionIndicatorForeground", Paint{"transparent"});
    return entries;
}

/// One sidebar destination; it is part of the selection while mounted.
inline void SidebarItem(Parent const& p, Ctx const& ctx, SidebarItemProps props) {
    auto registry = ctx.sidebar;
    if (!registry || !props.value) return;
    auto value = Reactive::Untrack([&] { return props.value(ctx); });
    std::string name = std::holds_alternative<char const*>(props.name) ? std::string(std::get<char const*>(props.name))
                                                                         : Reactive::Untrack([&] { return std::get<1>(props.name)(ctx); });
    TextFn label = props.label ? Bound(ctx, props.label) : TextFn([] { return winrt::hstring(); });
    controls::NavigationViewItem item;
    if (props.icon) item.Icon(Icons::Icon(props.icon));
    Reactive::Run([item, label] {
        auto text = label();
        item.Content(winrt::box_value(text));
        xaml::Automation::AutomationProperties::SetName(item, text);
    });
    if (props.badge) {
        auto badge = props.badge;
#if BERN_WINUI
        controls::InfoBadge info;
        item.InfoBadge(info);
        Reactive::Run([info, badge, ctx] {
            auto count = static_cast<int32_t>(std::llround(R::AsDouble(badge(ctx))));
            info.Value(count);
            info.Visibility(count > 0 ? xaml::Visibility::Visible : xaml::Visibility::Collapsed);
        });
#else
        // The system XAML's navigation items have no badge: the count
        // stands after the label.
        Reactive::Run([item, label, badge, ctx] {
            auto count = static_cast<int64_t>(std::llround(R::AsDouble(badge(ctx))));
            auto text = label();
            item.Content(winrt::box_value(count > 0 ? text + L"  " + winrt::to_hstring(count) : text));
        });
#endif
    }
    SidebarRegistry::Entry entry{registry->next++, value, name, label, item};
    if (auto style = props.style) {
        auto resources = SidebarResources(*style);
        if (!resources.empty()) Styles::Restyle(item, ThemedXaml(resources));
        if (style->corner) item.CornerRadius(xaml::CornerRadius{*style->corner, *style->corner, *style->corner, *style->corner});
        if (style->padding) item.Padding(xaml::Thickness{*style->padding, *style->padding, *style->padding, *style->padding});
        auto state = std::make_shared<StyleState>();
        auto restyle = [weak = winrt::make_weak(item), style, state] {
            if (auto element = weak.get()) MoveTo(element, *style, *state);
        };
        TrackState(item, state, restyle);
        entry.state = state;
        entry.restyle = restyle;
    }
    auto token = entry.token;
    registry->entries.push_back(entry);
    p.Add(item);
    registry->version.Bump();
    Reactive::OnCleanup([registry, token] {
        std::erase_if(registry->entries, [token](auto const& one) { return one.token == token; });
        registry->version.Bump();
    });
}

/// A titled group of sidebar items.
inline void SidebarSection(Parent const& p, Ctx const& ctx, Str title, Build const& children) {
    if (title) {
        controls::NavigationViewItemHeader heading;
        auto text = Bound(ctx, title);
        Reactive::Run([heading, text] { heading.Content(winrt::box_value(text())); });
        p.Add(heading);
    }
    children(p.Sub(), ctx);
}

struct SplitViewProps {
    char const* id = nullptr;
    Field selection;
    Get fallback;
    std::optional<SearchProps> search;
    /// `Sidebar(width:)`: the pane's width.
    std::optional<double> width;
    /// `Detail(titled:)`: whether the detail's header names the selected
    /// destination.
    Get titled;
};

/// The first element under `node` (itself included) that `pick` accepts,
/// depth first, without looking inside icons.
inline xaml::FrameworkElement FirstUnder(xaml::DependencyObject const& node, std::function<bool(xaml::DependencyObject const&)> const& pick, int depth = 0) {
    if (!node || depth > 40) return nullptr;
    if (pick(node)) return node.try_as<xaml::FrameworkElement>();
    if (node.try_as<controls::IconElement>()) return nullptr;
    int32_t count = media::VisualTreeHelper::GetChildrenCount(node);
    for (int32_t i = 0; i < count; ++i)
        if (auto found = FirstUnder(media::VisualTreeHelper::GetChild(node, i), pick, depth + 1)) return found;
    return nullptr;
}

/// Line the sidebar header's icon and title up with the destinations':
/// its first picture or icon centered on their icon column, its first text
/// at their title column, as the header of a sidebar does elsewhere.
inline void AlignWithItems(controls::NavigationView const& view, xaml::FrameworkElement const& header) {
    auto icon = [](xaml::DependencyObject const& node) { return node.try_as<controls::IconElement>() || node.try_as<controls::Image>(); };
    auto text = [](xaml::DependencyObject const& node) { return static_cast<bool>(node.try_as<controls::TextBlock>()); };
    auto align = [weakView = winrt::make_weak(view), weakHeader = winrt::make_weak(header), icon, text](auto&&, auto&&) {
        auto view = weakView.get();
        auto header = weakHeader.get();
        if (!view || !header || !view.XamlRoot() || !view.XamlRoot().Content()) return;
        controls::NavigationViewItem item{nullptr};
        for (auto entry : view.MenuItems())
            if ((item = entry.try_as<controls::NavigationViewItem>())) break;
        if (!item || !item.IsLoaded()) return;
        auto itemIcon = item.Icon().try_as<xaml::FrameworkElement>();
        auto itemText = FirstUnder(item, text);
        auto headerIcon = FirstUnder(header, icon);
        auto headerText = FirstUnder(header, text);
        if (!itemIcon || !itemText || !headerIcon || !headerText) return;
        auto root = view.XamlRoot().Content();
        try {
            auto x = [&](xaml::FrameworkElement const& e, double within) {
                return e.TransformToVisual(root).TransformPoint({static_cast<float>(within), 0}).X;
            };
            double move = x(itemIcon, itemIcon.ActualWidth() / 2) - x(headerIcon, headerIcon.ActualWidth() / 2);
            if (std::abs(move) > 0.5) {
                auto margin = header.Margin();
                margin.Left += move;
                header.Margin(margin);
                return;
            }
            double shift = x(itemText, 0) - x(headerText, 0);
            if (std::abs(shift) > 0.5) {
                auto margin = headerText.Margin();
                margin.Left = (std::max)(0.0, margin.Left + shift);
                headerText.Margin(margin);
            }
        } catch (winrt::hresult_error const&) {
        }
    };
    header.LayoutUpdated(align);
}

#if BERN_WINUI
/// The button that folds the pane is the title bar's, beside the window's
/// icon, as in Windows 11 applications. With several split views in a
/// window, the one shown last has it.
inline int64_t paneToggleOwner = 0;

inline void PaneToggle(controls::TitleBar const& bar, controls::NavigationView const& view) {
    auto key = winrt::box_value(L"bern.paneToggle");
    int64_t token = ++paneToggleOwner;
    bar.Resources().Insert(key, winrt::box_value(token));
    bar.IsPaneToggleButtonVisible(true);
    auto owns = [key, token](controls::TitleBar const& bar) {
        return bar.Resources().HasKey(key) && winrt::unbox_value<int64_t>(bar.Resources().Lookup(key)) == token;
    };
    auto revoker = bar.PaneToggleRequested(winrt::auto_revoke, [owns, weakView = winrt::make_weak(view)](controls::TitleBar const& sender, auto&&) {
        if (!owns(sender)) return;
        if (auto view = weakView.get()) view.IsPaneOpen(!view.IsPaneOpen());
    });
    auto requested = std::make_shared<decltype(revoker)>(std::move(revoker));
    Reactive::OnCleanup([requested, owns, key, weakBar = winrt::make_weak(bar)] {
        requested->revoke();
        auto bar = weakBar.get();
        if (!bar || !owns(bar)) return;
        bar.Resources().Remove(key);
        bar.IsPaneToggleButtonVisible(false);
    });
}
#endif

/// A pane of no declared width takes the width its edge is dragged to, as
/// a sidebar's divider does elsewhere: at least as wide as on macOS, at
/// most half as wide again as the view's own 320.
inline constexpr double kPaneNarrowest = 180, kPaneWidest = 480;

/// The view has no divider of its own: an invisible strip over the open
/// pane's edge takes the pointer and shows the system's resize cursor.
inline xaml::FrameworkElement PaneSizer(controls::NavigationView const& view) {
    controls::Grid strip;
    strip.Width(8);
    strip.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    strip.VerticalAlignment(xaml::VerticalAlignment::Stretch);
    strip.Background(media::SolidColorBrush(winrt::Windows::UI::Color{0, 0, 0, 0}));
    xaml::Automation::AutomationProperties::SetAccessibilityView(strip, xaml::Automation::Peers::AccessibilityView::Raw);
#if BERN_WINUI
    strip.as<xaml::IUIElementProtected>().ProtectedCursor(
        winrt::Microsoft::UI::Input::InputSystemCursor::Create(winrt::Microsoft::UI::Input::InputSystemCursorShape::SizeWestEast));
#else
    // System XAML has no cursor for an element: the island takes the
    // pointer's cursor as it moves.
    auto resize = [](auto&&, auto&&) { SetCursor(LoadCursorW(nullptr, IDC_SIZEWE)); };
    strip.PointerEntered(resize);
    strip.PointerMoved(resize);
#endif
    auto weakView = winrt::make_weak(view);
    auto weakStrip = winrt::make_weak(strip);
    // Only an open pane beside the content has an edge to drag.
    auto place = [weakView, weakStrip](bool closing = false) {
        auto view = weakView.get();
        auto strip = weakStrip.get();
        if (!view || !strip) return;
        bool shown = !closing && view.IsPaneOpen() && view.DisplayMode() == controls::NavigationViewDisplayMode::Expanded;
        strip.Visibility(shown ? xaml::Visibility::Visible : xaml::Visibility::Collapsed);
        strip.Margin(xaml::Thickness{view.OpenPaneLength() - 4, 0, 0, 0});
    };
    place();
    view.DisplayModeChanged([place](auto&&, auto&&) { place(); });
    view.PaneOpened([place](auto&&, auto&&) { place(); });
    view.PaneClosing([place](auto&&, auto&&) { place(true); });
    view.PaneClosed([place](auto&&, auto&&) { place(true); });
    view.RegisterPropertyChangedCallback(controls::NavigationView::OpenPaneLengthProperty(), [place](auto&&, auto&&) { place(); });

    struct Drag {
        bool down = false;
        double x = 0, from = 0;
    };
    auto drag = std::make_shared<Drag>();
    using PointerHandler = xaml::Input::PointerEventHandler;
    // Positions are taken in the view, which mirrors them right to left
    // as it mirrors the pane.
    strip.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(PointerHandler([drag, weakView](IInspectable const& sender, xaml::Input::PointerRoutedEventArgs const& args) {
        auto view = weakView.get();
        if (!view) return;
        auto point = args.GetCurrentPoint(view);
        if (!point.Properties().IsLeftButtonPressed()) return;
        *drag = Drag{true, point.Position().X, view.OpenPaneLength()};
        sender.as<xaml::UIElement>().CapturePointer(args.Pointer());
        args.Handled(true);
    })), true);
    strip.AddHandler(xaml::UIElement::PointerMovedEvent(), winrt::box_value(PointerHandler([drag, weakView](auto&&, xaml::Input::PointerRoutedEventArgs const& args) {
        auto view = weakView.get();
        if (!drag->down || !view) return;
        double width = drag->from + args.GetCurrentPoint(view).Position().X - drag->x;
        view.OpenPaneLength(std::round((std::clamp)(width, kPaneNarrowest, kPaneWidest)));
        args.Handled(true);
    })), true);
    auto end = [drag](IInspectable const& sender, xaml::Input::PointerRoutedEventArgs const& args) {
        if (!drag->down) return;
        drag->down = false;
        sender.as<xaml::UIElement>().ReleasePointerCapture(args.Pointer());
    };
    strip.AddHandler(xaml::UIElement::PointerReleasedEvent(), winrt::box_value(PointerHandler(end)), true);
    strip.AddHandler(xaml::UIElement::PointerCanceledEvent(), winrt::box_value(PointerHandler(end)), true);
    strip.AddHandler(xaml::UIElement::PointerCaptureLostEvent(), winrt::box_value(PointerHandler([drag](auto&&, auto&&) { drag->down = false; })), true);
    return strip;
}

/// The navigation view of Windows applications: a pane of destinations
/// with optional search and content at its foot, and a detail page whose
/// header shows the selected destination, the detail's tools and back. The
/// pane collapses as the window narrows, and its button folds it to the
/// view's strip of icons; a pane of no declared width takes the width its
/// edge is dragged to.
inline void SplitView(Parent const& p, Ctx const& ctx, SplitViewProps props, Build const& header, Build const& footer, Build const& sidebar,
    Build const& detail) {
    if (ctx.page) ctx.page->HideHeader();
    controls::NavigationView view;
    view.IsBackButtonVisible(controls::NavigationViewBackButtonVisible::Collapsed);
    view.IsSettingsVisible(false);
    view.PaneDisplayMode(controls::NavigationViewPaneDisplayMode::Auto);
    // The pane stays open at the usual window widths.
    view.ExpandedModeThresholdWidth(600);
    view.CompactModeThresholdWidth(400);
    view.AlwaysShowHeader(false);
#if BERN_WINUI
    view.IsTitleBarAutoPaddingEnabled(false);
    // The button that folds the pane is in the title bar, in every mode.
    view.IsPaneToggleButtonVisible(false);
    if (ctx.win && ctx.win->titleBar) PaneToggle(ctx.win->titleBar, view);
#else
    // Windows 10's: the button at the top of the pane, in every mode (the
    // system's title bar takes none).
    view.IsPaneToggleButtonVisible(true);
#endif
    MarkFill(view);
    auto registry = std::make_shared<SidebarRegistry>();
    registry->view = view;
    registry->ctx = ctx;
    registry->selection = props.selection;
    registry->fallback = props.fallback;
    auto selection = props.selection;
    if (props.search && props.search->field) view.AutoSuggestBox(SearchBox(ctx, *props.search));
    if (props.width) view.OpenPaneLength(*props.width);
    // Content pinned above the pane's list, lined up with the
    // destinations. The title bar above names the window.
    if (header) {
        auto top = MakeFlex(false, 8, xaml::Thickness{0, 8, 12, 8});
        header(Parent::Aligned(top, xaml::HorizontalAlignment::Stretch, std::nullopt), ctx);
        view.PaneHeader(top);
        AlignWithItems(view, top);
    }
    auto inner = ctx;
    inner.sidebar = registry;
    sidebar(Parent::Into(Children::OfVector<IInspectable>(view.MenuItems())), inner);
    if (footer) {
        auto foot = MakeFlex(false, 8, xaml::Thickness{12, 8, 12, 12});
        footer(Parent::Aligned(foot, xaml::HorizontalAlignment::Stretch, std::nullopt), ctx);
        view.PaneFooter(foot);
    }
    // The header and footer belong to the open pane: folded to its strip of
    // icons, the pane shows only the destinations, as a sidebar folded away
    // elsewhere takes its header and footer with it.
    auto parts = [weakView = winrt::make_weak(view)](bool open) {
        auto view = weakView.get();
        if (!view) return;
        auto shown = open ? xaml::Visibility::Visible : xaml::Visibility::Collapsed;
        for (auto part : {view.PaneHeader(), view.PaneFooter()})
            if (auto element = part.try_as<xaml::UIElement>()) element.Visibility(shown);
    };
    view.PaneOpening([parts](auto&&, auto&&) { parts(true); });
    view.PaneClosing([parts](auto&&, auto&&) { parts(false); });
    view.Loaded([parts](IInspectable const& sender, auto&&) { parts(sender.as<controls::NavigationView>().IsPaneOpen()); });

    Reactive::Run([registry, ctx, selection] {
        registry->version.Track();
        auto value = ctx.Read(selection);
        Reactive::Untrack([&] {
            auto view = registry->view;
            auto entry = registry->Selected(value);
            IInspectable item = entry ? IInspectable(entry->item) : nullptr;
            if (view.SelectedItem() != item) {
                registry->updating = true;
                registry->Rest(true);
                view.SelectedItem(item);
                registry->Rest(false);
                registry->updating = false;
            }
            for (auto const& one : registry->entries)
                if (one.state) {
                    bool selected = entry && one.token == entry->token;
                    if (one.state->selected != selected) {
                        one.state->selected = selected;
                        one.restyle();
                    }
                }
            if (!entry && !registry->settling) {
                registry->settling = true;
                std::weak_ptr<SidebarRegistry> weak = registry;
                Host::Post([weak] {
                    if (auto registry = weak.lock()) registry->Settle();
                });
            }
            return 0;
        });
    });
    // A click or a key chooses an item: the view raises ItemInvoked, then
    // measures its pills, then raises SelectionChanged.
    view.ItemInvoked([weak = std::weak_ptr<SidebarRegistry>(registry), ctx, selection](auto&&, controls::NavigationViewItemInvokedEventArgs const& args) {
        auto registry = weak.lock();
        if (!registry || registry->updating) return;
        auto current = registry->Selected(ctx.Peek(selection));
        if (current && current->item == args.InvokedItemContainer()) return;
        registry->Rest(true);
        // Should the selection stay, nothing raises SelectionChanged.
        std::weak_ptr<SidebarRegistry> later = registry;
        Host::Post([later] {
            if (auto registry = later.lock(); registry && registry->resting) registry->Rest(false);
        });
    });
    view.SelectionChanged([weak = std::weak_ptr<SidebarRegistry>(registry), ctx, selection](auto&&, controls::NavigationViewSelectionChangedEventArgs const& args) {
        auto registry = weak.lock();
        if (registry && registry->resting && !registry->updating) registry->Rest(false);
        if (!registry || registry->updating || !args.SelectedItem()) return;
        for (auto const& entry : registry->entries)
            if (entry.item == args.SelectedItem()) ctx.Write(selection, entry.value);
    });

    TextFn selectedLabel = [registry, ctx, selection] {
        registry->version.Track();
        auto value = ctx.Read(selection);
        if (auto entry = registry->Selected(value)) return entry->label();
        return winrt::hstring();
    };
    auto page = std::make_shared<Page>();
    page->win = ctx.win;
    // Untitled, the detail's pages show their own titles.
    bool titled = !props.titled || Reactive::Untrack([&] { return R::Truthy(props.titled(ctx)); });
    auto bar = Header(page, selectedLabel, titled);
    bar.Margin(xaml::Thickness{24, 8, 24, 8});
    controls::Grid body;
    controls::RowDefinition top;
    top.Height(AutoLength());
    controls::RowDefinition rest;
    rest.Height(Star());
    body.RowDefinitions().Append(top);
    body.RowDefinitions().Append(rest);
    body.Children().Append(bar);
    auto content = MakeFlex();
    controls::Grid::SetRow(content.as<xaml::FrameworkElement>(), 1);
    body.Children().Append(content);
    view.Content(body);
    detail(Parent::Of(content), ctx.WithPage(page));

    if (props.id)
        Controls::Register(ctx, props.id, "sidebar", selectedLabel, nullptr,
            Actions{.select = [registry, ctx, selection](std::string const& name) {
                        for (auto const& entry : registry->entries)
                            if (entry.name == name) {
                                ctx.Write(selection, entry.value);
                                return true;
                            }
                        return false;
                    },
                .value = [registry, ctx, selection]() -> std::optional<std::string> {
                    if (auto entry = registry->Selected(ctx.Peek(selection))) return entry->name;
                    return std::nullopt;
                }});
    if (props.width) {
        p.Add(view);
        return;
    }
    controls::Grid frame;
    MarkFill(frame);
    frame.Children().Append(view);
    frame.Children().Append(PaneSizer(view));
    p.Add(frame);
}

/// Push navigation over a typed route path. The root content stays mounted
/// under pushed destinations, so its subscriptions, players and scroll
/// position survive going back; the top route's destination is built on
/// top, and the page header offers back.
inline void Navigation(Parent const& p, Ctx const& ctx, char const* id, Field path, Build const& content, char const* variable, Build destination) {
    controls::Grid stack;
    MarkFill(stack);
    auto root = MakeFlex();
    auto top = MakeFlex();
    top.Visibility(xaml::Visibility::Collapsed);
    stack.Children().Append(root);
    stack.Children().Append(top);
    auto inner = ctx.WithNav(path);
    // The root's tools show only while the root is.
    Cell<bool> rootShown(true);
    content(Parent::Of(root), inner.WithShown(rootShown));

    std::function<bool()> back = [ctx, path] {
        auto routes = R::AsList(ctx.Peek(path));
        if (routes.empty()) return false;
        Values shorter(routes.begin(), routes.end() - 1);
        ctx.Write(path, Value::List(std::move(shorter)));
        return true;
    };
    for (auto [key, modifiers] : {std::pair{winrt::Windows::System::VirtualKey::Escape, winrt::Windows::System::VirtualKeyModifiers::None},
             std::pair{winrt::Windows::System::VirtualKey::Left, winrt::Windows::System::VirtualKeyModifiers::Menu}}) {
        xaml::Input::KeyboardAccelerator accelerator;
        accelerator.Key(key);
        accelerator.Modifiers(modifiers);
        accelerator.Invoked([back](auto&&, xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args) { args.Handled(back()); });
        top.KeyboardAccelerators().Append(accelerator);
    }
    top.KeyboardAcceleratorPlacementMode(xaml::Input::KeyboardAcceleratorPlacementMode::Hidden);

    auto slot = Slot::In(Parent::Of(top));
    auto page = ctx.page;
    std::string name = variable;
    Reactive::Run([root, top, slot, rootShown, inner, page, back, destination, name, ctx, path]() mutable {
        auto routes = R::AsList(ctx.Read(path));
        rootShown.Set(routes.empty());
        if (routes.empty()) {
            root.Visibility(xaml::Visibility::Visible);
            top.Visibility(xaml::Visibility::Collapsed);
            slot->Clear();
            return;
        }
        auto route = routes.back();
        auto routeCtx = inner.WithLocal(name, [route] { return route; });
        slot->Fill([&](Parent const& region) {
            if (page) page->PushBack([back] { back(); });
            destination(region, routeCtx);
        });
        root.Visibility(xaml::Visibility::Collapsed);
        top.Visibility(xaml::Visibility::Visible);
    });
    if (id)
        Controls::Register(ctx, id, "navigation", [] { return winrt::hstring(); }, nullptr,
            Actions{.back = back, .value = [ctx, path] { return std::optional<std::string>(std::to_string(R::AsList(ctx.Peek(path)).size())); }});
    p.Add(stack);
}

/// A row that pushes its route on the enclosing navigation.
inline void NavLink(Parent const& p, Ctx const& ctx, char const* id, Get route, Str label, Build const& children) {
    auto content = MakeFlex(true, 8);
    children(Parent::Aligned(content, std::nullopt, xaml::VerticalAlignment::Center), ctx);
    std::function<void()> activate = [ctx, route] {
        if (!ctx.nav) return;
        auto routes = R::AsList(ctx.Peek(*ctx.nav));
        Values longer(routes.begin(), routes.end());
        longer.push_back(route(ctx));
        ctx.Write(*ctx.nav, Value::List(std::move(longer)));
    };
    controls::Grid inside;
    inside.ColumnSpacing(12);
    inside.ColumnDefinitions().Append(Column(Star()));
    inside.ColumnDefinitions().Append(Column(AutoLength()));
    inside.Children().Append(content);
    auto chevron = Icons::Icon("chevron.right", 12);
    chevron.VerticalAlignment(xaml::VerticalAlignment::Center);
    controls::Grid::SetColumn(chevron, 1);
    inside.Children().Append(chevron);
    controls::Button button;
    button.Content(inside);
    button.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    button.HorizontalContentAlignment(xaml::HorizontalAlignment::Stretch);
    if (p.isList) {
        button.Padding(xaml::Thickness{16, 12, 16, 12});
        button.MinHeight(56);
    } else {
        Styles::MakeSubtle(button);
    }
    auto text = Bound(ctx, label);
    BindName(button, text);
    button.Click([activate](auto&&, auto&&) { activate(); });
    p.Add(button);
    if (ctx.nav) Controls::Register(ctx, id, "link", text, nullptr, Actions{.activate = activate});
}

/// A tab of `Tabs`.
struct TabSpec {
    Get value;
    std::variant<char const*, std::function<std::string(Ctx const&)>> name = "";
    Str label;
    char const* icon = nullptr;
    /// `display: icon`: the icon alone, the label its accessible name and
    /// tooltip.
    bool iconOnly = false;
    Build build;
};

/// The tabs of a `Tabs`, which may come and go: each registers while it is
/// mounted. Only the selected tab is built.
struct TabsRegistry {
    struct Entry {
        int64_t token = 0;
        Value value;
        std::string name;
        Str label;
        Build build;
        xaml::UIElement item{nullptr};
    };
    std::vector<Entry> entries;
    Ctx ctx;
    Field selection;
    Get fallback;
    bern::Version version;
    int64_t next = 1;
    bool updating = false;
    bool settling = false;
    std::function<std::optional<uint32_t>(xaml::UIElement const&)> position;
    std::function<void(xaml::UIElement const&)> show;

    Entry const* Selected(Value const& value) const {
        for (auto const& entry : entries)
            if (entry.value == value) return &entry;
        return nullptr;
    }

    void Settle() {
        settling = false;
        if (entries.empty() || Selected(ctx.Peek(selection))) return;
        std::optional<Value> back;
        if (fallback) {
            auto wanted = Reactive::Untrack([&] { return fallback(ctx); });
            if (Selected(wanted)) back = wanted;
        }
        if (!back) {
            Entry const* first = nullptr;
            uint32_t best = UINT32_MAX;
            for (auto const& entry : entries)
                if (auto at = position(entry.item); at && *at < best) {
                    best = *at;
                    first = &entry;
                }
            if (first) back = first->value;
        }
        if (back) ctx.Write(selection, *back);
    }
};

struct TabsProps {
    char const* id = nullptr;
    Field selection;
    Get fallback;
};

/// `TabAccessory`: a bar kept under the pages while `shown` holds.
struct TabAccessory {
    Get shown;
    Build build;
};

/// Tabs as a selector in the page header, as Windows Settings pages show
/// them: a selector bar in WinUI 3, a row of toggle buttons in the system
/// XAML, which has none.
inline void Tabs(Parent const& p, Ctx const& ctx, TabsProps props, Build const& tabs, TabAccessory const& accessory = {}) {
    auto registry = std::make_shared<TabsRegistry>();
    registry->ctx = ctx;
    registry->selection = props.selection;
    registry->fallback = props.fallback;
    auto selection = props.selection;
    xaml::FrameworkElement selector{nullptr};
    Parent items;
#if BERN_WINUI
    controls::SelectorBar bar;
    selector = bar;
    auto list = Children::OfVector<controls::SelectorBarItem>(bar.Items());
    items = Parent::Into(list);
    registry->position = list->find;
    registry->show = [bar, registry](xaml::UIElement const& item) {
        auto chosen = item ? item.as<controls::SelectorBarItem>() : nullptr;
        if (bar.SelectedItem() == chosen) return;
        registry->updating = true;
        bar.SelectedItem(chosen);
        registry->updating = false;
    };
    bar.SelectionChanged([weak = std::weak_ptr<TabsRegistry>(registry), ctx, selection](controls::SelectorBar const& sender, auto&&) {
        auto registry = weak.lock();
        if (!registry || registry->updating || !sender.SelectedItem()) return;
        for (auto const& entry : registry->entries)
            if (entry.item == sender.SelectedItem()) ctx.Write(selection, entry.value);
    });
#else
    controls::StackPanel row;
    row.Orientation(controls::Orientation::Horizontal);
    row.Spacing(2);
    selector = row;
    items = Parent::Of(row);
    auto list = Children::Of(row);
    registry->position = list->find;
    registry->show = [registry](xaml::UIElement const& item) {
        for (auto const& entry : registry->entries)
            entry.item.as<controls::Primitives::ToggleButton>().IsChecked(entry.item == item);
    };
#endif
    auto holder = MakeFlex();
    MarkFill(holder);
    auto slot = Slot::In(Parent::Of(holder));
    if (ctx.page) ctx.page->PushTitleWidget(selector);
    else p.Add(selector);
    auto inner = ctx;
    inner.tabs = registry;
    tabs(items, inner);
    auto shown = std::make_shared<int64_t>(0);
    Reactive::Run([registry, ctx, selection, slot, shown] {
        registry->version.Track();
        auto value = ctx.Read(selection);
        Reactive::Untrack([&] {
            auto entry = registry->Selected(value);
            registry->show(entry ? entry->item : nullptr);
            if (!entry) {
                if (!registry->settling) {
                    registry->settling = true;
                    std::weak_ptr<TabsRegistry> weak = registry;
                    Host::Post([weak] {
                        if (auto registry = weak.lock()) registry->Settle();
                    });
                }
                return 0;
            }
            if (*shown == entry->token) return 0;
            *shown = entry->token;
            auto build = entry->build;
            slot->Fill([&](Parent const& region) { build(region, ctx); });
            return 0;
        });
    });
    if (props.id)
        Controls::Register(ctx, props.id, "tabs",
            [registry, ctx, selection] {
                registry->version.Track();
                auto entry = registry->Selected(ctx.Read(selection));
                return entry && entry->label ? entry->label(ctx) : winrt::hstring();
            },
            nullptr,
            Actions{.select = [registry, ctx, selection](std::string const& name) {
                        for (auto const& entry : registry->entries)
                            if (entry.name == name) {
                                ctx.Write(selection, entry.value);
                                return true;
                            }
                        return false;
                    },
                .value = [registry, ctx, selection]() -> std::optional<std::string> {
                    if (auto entry = registry->Selected(ctx.Peek(selection))) return entry->name;
                    return std::nullopt;
                }});
    p.Add(holder);
    // The accessory below the pages, on a card as a raised bar.
    if (accessory.build) {
        controls::Grid row;
        auto card = Card(row);
        card.Margin(xaml::Thickness{12, 6, 12, 6});
        card.Padding(xaml::Thickness{8, 0, 8, 0});
        card.MinHeight(48);
        accessory.build(Parent::Of(row), ctx);
        if (auto shown = accessory.shown)
            Reactive::Run([card, shown, ctx] {
                card.Visibility(R::Truthy(shown(ctx)) ? xaml::Visibility::Visible : xaml::Visibility::Collapsed);
            });
        p.Add(card);
    }
}

/// One tab; part of its `Tabs` while mounted.
inline void Tab(Parent const& p, Ctx const& ctx, TabSpec spec) {
    auto registry = ctx.tabs;
    if (!registry || !spec.value) return;
    TabsRegistry::Entry entry;
    entry.token = registry->next++;
    entry.value = Reactive::Untrack([&] { return spec.value(ctx); });
    entry.name = std::holds_alternative<char const*>(spec.name) ? std::string(std::get<char const*>(spec.name))
                                                                  : Reactive::Untrack([&] { return std::get<1>(spec.name)(ctx); });
    entry.label = spec.label;
    entry.build = spec.build;
    auto label = spec.label;
    bool iconOnly = spec.iconOnly && spec.icon;
#if BERN_WINUI
    controls::SelectorBarItem item;
    if (spec.icon) item.Icon(Icons::Icon(spec.icon));
    if (label && !iconOnly) Reactive::Run([item, label, ctx] { item.Text(label(ctx)); });
#else
    controls::Primitives::ToggleButton item;
    if (iconOnly) item.Content(Icons::Icon(spec.icon));
    else if (label) Reactive::Run([item, label, ctx] { item.Content(winrt::box_value(label(ctx))); });
    auto value = entry.value;
    auto selection = registry->selection;
    item.Click([ctx, selection, value](auto&&, auto&&) { ctx.Write(selection, value); });
#endif
    if (iconOnly && label)
        Reactive::Run([item, label, ctx] {
            auto name = label(ctx);
            xaml::Automation::AutomationProperties::SetName(item, name);
            controls::ToolTipService::SetToolTip(item, winrt::box_value(name));
        });
    entry.item = item;
    auto token = entry.token;
    registry->entries.push_back(entry);
    p.Add(item);
    registry->version.Bump();
    Reactive::OnCleanup([registry, token] {
        std::erase_if(registry->entries, [token](auto const& one) { return one.token == token; });
        registry->version.Bump();
    });
}

/// Chooses one layout by the width available to it and the breakpoints of
/// `[layouts]`: a size without a layout of its own shows the next larger
/// one, else the next smaller. Engine state and drafts live outside the
/// element tree, so switching keeps them; controls with the same id in
/// every layout are the same control.
inline void Adaptive(Parent const& p, Ctx const& ctx, char const* id, std::array<Build, 4> layouts) {
    auto holder = MakeFlex(false, p.isList ? (kWinUI ? 4 : 2) : 8);
    holder.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    static char const* const kSizes[] = {"compact", "medium", "expanded", "large"};
    auto const& catalog = app->catalog;
    auto sizeOf = [&catalog](double width) {
        return width < catalog.compactBelow.value_or(0) ? 0
            : catalog.mediumBelow && width < *catalog.mediumBelow ? 1
            : !catalog.expandedBelow || width < *catalog.expandedBelow ? 2
            : 3;
    };
    // Until it has measured the room it is given, the window's width
    // stands in: the room is no wider, so a layout built as a narrow window
    // opens, or inserted into it later, is right from its first build.
    Cell<int> size(sizeOf(ctx.win ? ctx.win->width : INFINITY));
    holder.SizeChanged([size, sizeOf](auto&&, xaml::SizeChangedEventArgs const& args) mutable {
        // A hidden layout has no width to judge by; it keeps its size.
        double width = args.NewSize().Width;
        if (width > 0) size.Set(sizeOf(width));
    });
    auto shared = std::make_shared<std::array<Build, 4>>(std::move(layouts));
    auto layoutFor = [shared](int wanted) -> int {
        if ((*shared)[wanted]) return wanted;
        for (int larger = wanted + 1; larger < 4; ++larger)
            if ((*shared)[larger]) return larger;
        for (int smaller = wanted - 1; smaller >= 0; --smaller)
            if ((*shared)[smaller]) return smaller;
        return -1;
    };
    auto region = p.isList ? Parent::OfList(holder) : Parent::Of(holder);
    auto slot = Slot::In(region);
    auto shown = std::make_shared<int>(-2);
    Reactive::Run([size, shared, layoutFor, slot, ctx, shown] {
        int layout = layoutFor(size.Get());
        if (layout == *shown) return;
        *shown = layout;
        if (layout < 0) slot->Clear();
        else slot->Fill([&](Parent const& target) { (*shared)[layout](target, ctx); });
    });
    if (id)
        Controls::Register(ctx, id, "adaptive", [] { return winrt::hstring(); }, nullptr,
            Actions{.value = [size] { return std::optional<std::string>(kSizes[size.Peek()]); }});
    p.Add(holder);
}

// ---------------------------------------------------------------------------
// Queries: the result of an asynchronous query. The subscription belongs to
// the window instance and ends with this view; parameters are sent again
// only when they change, and only the current answer is shown.

/// A query's state: 0 idle, 1 loading, 2 ready, 3 refreshing, 4 failed.
struct QueryState {
    uint64_t version = 0;
    uint8_t phase = 0;
    std::optional<Value> data;
    std::optional<Failure> error;
    bool operator==(QueryState const& other) const {
        return version == other.version && phase == other.phase && data == other.data && (error.has_value() == other.error.has_value());
    }
};

struct QueryProps {
    char const* id = nullptr;
    int query = 0;
    std::vector<Get> params;
    Str loading;
    Str failed;
    Str empty;
    char const* icon = nullptr;
    char const* variable = "item";
};

inline std::optional<QueryState> ReadQueryState(Bytes const& bytes, BType const& result) {
    try {
        Reader reader(bytes);
        QueryState state;
        state.version = reader.U64();
        state.phase = reader.U8();
        auto const& types = app->catalog.types;
        if (state.phase == 2 || state.phase == 3) state.data = Codec::Decode(result, types, reader);
        if (state.phase == 4) {
            if (reader.U8() == 1) state.data = Codec::Decode(result, types, reader);
            state.error = Failure{reader.Text(), reader.Text()};
        }
        return state;
    } catch (DecodeError const&) {
        return std::nullopt;
    }
}

/// What the template shows of the data: nothing when the result is empty and
/// the declaration has an empty state.
inline std::optional<Value> ShownData(Value const& data, BType const& result, bool hasEmpty) {
    if (data.kind == Value::Kind::List && result.kind == BType::Kind::List && hasEmpty && data.Items().empty()) return std::nullopt;
    if (data.kind == Value::Kind::Optional) return R::Some(data);
    return data;
}

inline void Query(Parent const& p, Ctx const& ctx, QueryProps props, Build const& templ) {
    auto const& commands = app->catalog.commands;
    if (props.query < 0 || props.query >= static_cast<int>(commands.size())) return;
    auto const& spec = commands[props.query];
    auto result = spec.result;
    bool hasEmpty = static_cast<bool>(props.empty);
    Cell<QueryState> state;
    auto receive = [state, result](Bytes const& bytes) mutable {
        if (auto next = ReadQueryState(bytes, result); next && next->version >= state.Peek().version) state.Set(*next);
    };
    std::optional<uint64_t> subscription;
    {
        auto opened = Host::Take(bern_query_open(Host::handle, static_cast<uint32_t>(ctx.win->kind), ctx.win->generation, static_cast<uint32_t>(props.query)));
        Reader openReader(opened);
        if (auto failure = Host::Status(opened, openReader)) {
            QueryState failedState;
            failedState.phase = 4;
            failedState.error = failure;
            state.Set(failedState);
        } else {
            subscription = openReader.U64();
        }
    }
    if (subscription) {
        auto id = *subscription;
        Host::queries[id] = receive;
        Reactive::OnCleanup([id] {
            Host::queries.erase(id);
            if (Host::handle) bern_query_close(Host::handle, id);
        });
    }
    auto request = [subscription, receive](Bytes const& bytes) mutable {
        if (!subscription || !Host::handle) return;
        auto reply = Host::Take(bern_query_request(Host::handle, *subscription, bytes.data(), bytes.size()));
        Reader reader(reply);
        if (!Host::Status(reply, reader)) receive(Bytes(reply.begin() + 1, reply.end()));
    };
    auto params = std::make_shared<std::vector<Get>>(std::move(props.params));
    auto paramTypes = spec.params;
    auto encode = [params, paramTypes, ctx] {
        Writer w;
        for (size_t i = 0; i < params->size() && i < paramTypes.size(); ++i) Codec::Encode((*params)[i](ctx), paramTypes[i], app->catalog.types, w);
        return w.bytes;
    };
    auto sent = std::make_shared<std::optional<Bytes>>();
    Reactive::Run([encode, sent, request]() mutable {
        auto bytes = encode();
        if (*sent && **sent == bytes) return;
        *sent = bytes;
        Reactive::Untrack([&] { request(bytes); return 0; });
    });
    std::function<void()> retry = [encode, sent, request]() mutable {
        auto bytes = Reactive::Untrack([&] { return encode(); });
        *sent = bytes;
        request(bytes);
    };

    // Which status to show, and whether the template shows data.
    auto key = [state, result, hasEmpty]() -> std::string {
        auto const& current = state.Get();
        auto visible = current.data ? ShownData(*current.data, result, hasEmpty) : std::nullopt;
        if (current.phase == 1 || (current.phase == 0 && !current.data)) return "loading";
        if (current.phase == 4 && !current.data) return "failed";
        if (!visible && hasEmpty) return "empty";
        return "data";
    };
    auto textOf = [ctx](Str const& text, char const* fallback) -> TextFn {
        if (text) return [ctx, text] { return text(ctx); };
        std::string keyName = fallback;
        return [keyName] { return bern::Text::Tr(keyName); };
    };
    auto loadingText = textOf(props.loading, "bern.query.loading");
    auto failedText = textOf(props.failed, "bern.query.failed");
    auto emptyText = textOf(props.empty, "");
    bool inList = p.isList;
    std::string iconName = props.icon ? props.icon : "tray";
    auto add = [inList](Parent const& target, xaml::FrameworkElement const& element) {
        if (inList) target.Add(Card(element));
        else target.Add(element);
    };
    auto retryButton = [retry] {
        controls::Button button;
        Reactive::Run([button] { button.Content(winrt::box_value(bern::Text::Tr("bern.query.retry"))); });
        button.Click([retry](auto&&, auto&&) { retry(); });
        return button;
    };
    auto ring = [](double size) {
        controls::ProgressRing ring;
        ring.IsActive(true);
        ring.Width(size);
        ring.Height(size);
        return ring;
    };
    auto status = [](std::string const& glyph, TextFn const& title, xaml::FrameworkElement const& action) {
        controls::StackPanel column;
        column.Spacing(8);
        column.HorizontalAlignment(xaml::HorizontalAlignment::Center);
        column.Margin(xaml::Thickness{0, 24, 0, 24});
        auto symbol = Icons::Icon(glyph, 32);
        symbol.HorizontalAlignment(xaml::HorizontalAlignment::Center);
        column.Children().Append(symbol);
        auto heading = TextOf(title);
        heading.HorizontalAlignment(xaml::HorizontalAlignment::Center);
        heading.TextAlignment(xaml::TextAlignment::Center);
        if (auto style = Styles::TextStyle("strong")) heading.Style(style);
        column.Children().Append(heading);
        if (action) {
            action.HorizontalAlignment(xaml::HorizontalAlignment::Center);
            column.Children().Append(action);
        }
        return column;
    };
    std::vector<Build> arms{
        [=](Parent const& target, Ctx const&) {
            controls::StackPanel line;
            line.Orientation(controls::Orientation::Horizontal);
            line.Spacing(8);
            line.HorizontalAlignment(xaml::HorizontalAlignment::Center);
            line.Margin(xaml::Thickness{0, 18, 0, 18});
            line.Children().Append(ring(20));
            auto label = TextOf(loadingText);
            label.VerticalAlignment(xaml::VerticalAlignment::Center);
            Secondary(label);
            line.Children().Append(label);
            add(target, line);
        },
        [=](Parent const& target, Ctx const&) { add(target, status("exclamationmark.triangle", failedText, retryButton())); },
        [=](Parent const& target, Ctx const&) { add(target, status(iconName, emptyText, nullptr)); },
        [=](Parent const& target, Ctx const&) {
            controls::StackPanel line;
            line.Orientation(controls::Orientation::Horizontal);
            line.Spacing(8);
            line.Children().Append(ring(16));
            auto label = TextOf(loadingText);
            label.VerticalAlignment(xaml::VerticalAlignment::Center);
            Secondary(label);
            line.Children().Append(label);
            add(target, line);
        },
        [=](Parent const& target, Ctx const&) {
            controls::Grid line;
            line.ColumnSpacing(8);
            line.ColumnDefinitions().Append(Column(AutoLength()));
            line.ColumnDefinitions().Append(Column(Star()));
            line.ColumnDefinitions().Append(Column(AutoLength()));
            auto warning = Icons::Icon("exclamationmark.triangle");
            warning.VerticalAlignment(xaml::VerticalAlignment::Center);
            line.Children().Append(warning);
            auto label = TextOf(failedText);
            label.VerticalAlignment(xaml::VerticalAlignment::Center);
            controls::Grid::SetColumn(label, 1);
            line.Children().Append(label);
            auto button = retryButton();
            controls::Grid::SetColumn(button, 2);
            line.Children().Append(button);
            add(target, line);
        },
    };
    When(p, ctx, [key, state](Ctx const&) -> std::optional<int> {
        auto name = key();
        auto phase = state.Get().phase;
        if (name == "loading") return 0;
        if (name == "failed") return 1;
        if (name == "empty") return 2;
        if (name == "data" && phase == 3) return 3;
        if (name == "data" && phase == 4) return 4;
        return std::nullopt;
    }, std::move(arms));

    auto inner = ctx.WithLocal(props.variable, [state, result, hasEmpty] {
        auto const& current = state.Get();
        if (!current.data) return Value::Unit();
        return ShownData(*current.data, result, hasEmpty).value_or(Value::Unit());
    });
    When(p, ctx, [key](Ctx const&) -> std::optional<int> { if (key() == "data") return 0; return std::nullopt; },
        {[templ, inner](Parent const& target, Ctx const&) { templ(target, inner); }});

    if (props.id)
        Controls::Register(ctx, props.id, "query", [] { return winrt::hstring(); }, nullptr,
            Actions{.activate = retry, .value = [state]() -> std::optional<std::string> {
                        static char const* const names[] = {"idle", "loading", "ready", "refreshing", "failed"};
                        auto const& current = state.Peek();
                        std::string text = std::string(names[(std::min)(static_cast<int>(current.phase), 4)]) + " v" + std::to_string(current.version);
                        if (current.data) {
                            auto const& data = *current.data;
                            if (data.kind == Value::Kind::List) text += " (" + std::to_string(data.Items().size()) + ")";
                            else if (data.kind == Value::Kind::Optional) text += std::string(" (") + (data.IsSome() ? "1" : "0") + ")";
                        }
                        return text;
                    }});
}

// ---------------------------------------------------------------------------
// Video

inline std::optional<std::string> UrlOf(Value value) {
    if (value.kind == Value::Kind::Optional) {
        if (!value.IsSome()) return std::nullopt;
        Value inner = value.Items()[0];
        value = std::move(inner);
    }
    if (value.kind == Value::Kind::Record && !value.Items().empty() && value.Items()[0].kind == Value::Kind::Text) return value.Items()[0].Str();
    return std::nullopt;
}

/// A slider that previews while dragged and commits on release; keyboard
/// changes commit at once.
struct Scrubber {
    controls::Slider slider;
    std::shared_ptr<bool> updating = std::make_shared<bool>(false);
    std::shared_ptr<bool> dragging = std::make_shared<bool>(false);

    Scrubber(std::function<void(double)> commit, std::function<void(double)> preview) {
        auto drag = dragging;
        auto busy = updating;
        auto source = winrt::make_weak(slider);
        slider.IsThumbToolTipEnabled(false);
        slider.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(xaml::Input::PointerEventHandler([drag](auto&&, auto&&) { *drag = true; })), true);
        auto release = [drag, commit, source](auto&&, auto&&) {
            auto slider = source.get();
            if (!slider || !*drag) return;
            *drag = false;
            commit(slider.Value());
        };
        slider.AddHandler(xaml::UIElement::PointerReleasedEvent(), winrt::box_value(xaml::Input::PointerEventHandler(release)), true);
        slider.AddHandler(xaml::UIElement::PointerCaptureLostEvent(), winrt::box_value(xaml::Input::PointerEventHandler(release)), true);
        slider.ValueChanged([busy, drag, commit, preview](auto&&, controls::Primitives::RangeBaseValueChangedEventArgs const& args) {
            if (*busy) return;
            if (*drag) {
                if (preview) preview(args.NewValue());
            } else {
                commit(args.NewValue());
            }
        });
    }

    void Show(double value) const {
        if (*dragging) return;
        *updating = true;
        slider.Value(value);
        *updating = false;
    }
};

inline xaml::FrameworkElement ControlBar(Ctx const& ctx, std::string const& id, std::string const& player) {
    auto send = [ctx, player](MediaOp op, Values args) { MediaOps::Send(ctx, op, player, args); };
    std::function<void()> toggle = [send, player] {
        auto state = Media::Peek(player);
        send(state && state->Playing() ? MediaOp::Pause : MediaOp::Play, {});
    };
    std::function<void(double)> seek = [send](double ms) { send(MediaOp::Seek, {Value::Int(static_cast<int64_t>(std::llround(ms)))}); };
    std::function<void(bool)> setMuted = [send](bool muted) { send(MediaOp::SetMuted, {Value::Bool(muted)}); };
    std::function<void(double)> setVolume = [send](double volume) { send(MediaOp::SetVolume, {Value::Float((std::clamp)(volume, 0.0, 1.0))}); };
    std::function<void()> toggleMute = [setMuted, player] {
        auto state = Media::Peek(player);
        setMuted(!(state && state->muted));
    };

    auto playIcon = Icons::Icon("play.fill");
    controls::Button play;
    play.Content(playIcon);
    Styles::MakeSubtle(play);
    play.Click([toggle](auto&&, auto&&) { toggle(); });
    controls::TextBlock positionText;
    positionText.Text(L"0:00");
    positionText.VerticalAlignment(xaml::VerticalAlignment::Center);
    controls::TextBlock durationText;
    durationText.Text(L"--:--");
    durationText.VerticalAlignment(xaml::VerticalAlignment::Center);
    if (auto caption = Styles::TextStyle("caption")) {
        positionText.Style(caption);
        durationText.Style(caption);
    }
    Scrubber position(seek, [positionText](double ms) { positionText.Text(Wide(Media::Time(static_cast<int64_t>(ms)))); });
    position.slider.VerticalAlignment(xaml::VerticalAlignment::Center);
    auto muteIcon = Icons::Icon("speaker.wave.3.fill");
    controls::Button mute;
    mute.Content(muteIcon);
    Styles::MakeSubtle(mute);
    mute.Click([toggleMute](auto&&, auto&&) { toggleMute(); });
    Scrubber volume(setVolume, nullptr);
    volume.slider.Minimum(0);
    volume.slider.Maximum(1);
    volume.slider.StepFrequency(0.05);
    volume.slider.SmallChange(0.05);
    volume.slider.LargeChange(0.1);
    volume.slider.Width(96);
    volume.slider.VerticalAlignment(xaml::VerticalAlignment::Center);

    Reactive::Run([=] {
        auto state = Media::State(player);
        bool playing = state && state->Playing();
        auto playLabel = bern::Text::Tr(playing ? "bern.media.pause" : "bern.media.play");
        playIcon.Glyph(Icons::Glyph(playing ? "pause.fill" : "play.fill"));
        controls::ToolTipService::SetToolTip(play, winrt::box_value(playLabel));
        xaml::Automation::AutomationProperties::SetName(play, playLabel);
        play.IsEnabled(state && state->Playable());
        int64_t duration = state && state->durationMs ? *state->durationMs : 0;
        int64_t at = state ? state->positionMs : 0;
        if (!*position.dragging) {
            *position.updating = true;
            position.slider.Maximum(static_cast<double>((std::max)(int64_t{1}, duration)));
            position.slider.StepFrequency(1000);
            position.slider.SmallChange(5000);
            position.slider.LargeChange((std::max)(5000.0, duration / 10.0));
            *position.updating = false;
            position.Show(static_cast<double>((std::min)(at, duration)));
            positionText.Text(Wide(Media::Time(at)));
        }
        position.slider.IsEnabled(state && state->CanSeek());
        xaml::Automation::AutomationProperties::SetName(position.slider, bern::Text::Tr("bern.media.position"));
        if (state && state->durationMs) durationText.Text(Wide(Media::Time(duration)));
        else if (state && state->Playable()) durationText.Text(bern::Text::Tr("bern.media.live"));
        else durationText.Text(L"--:--");
        bool muted = state && state->muted;
        auto muteLabel = bern::Text::Tr(muted ? "bern.media.unmute" : "bern.media.mute");
        muteIcon.Glyph(Icons::Glyph(muted ? "speaker.slash.fill" : "speaker.wave.3.fill"));
        controls::ToolTipService::SetToolTip(mute, winrt::box_value(muteLabel));
        xaml::Automation::AutomationProperties::SetName(mute, muteLabel);
        bool controllable = state && state->Controllable();
        mute.IsEnabled(controllable);
        volume.slider.IsEnabled(controllable);
        volume.Show(state ? state->volume : 1.0);
        xaml::Automation::AutomationProperties::SetName(volume.slider, bern::Text::Tr("bern.media.volume"));
    });

    auto target = [&id](char const* name) { return id + "." + name; };
    Controls::Register(ctx, target("play_pause"), "button",
        [player] { auto s = Media::State(player); return bern::Text::Tr(s && s->Playing() ? "bern.media.pause" : "bern.media.play"); },
        [player] { auto s = Media::State(player); return s && s->Playable(); }, Actions{.activate = toggle});
    Controls::Register(ctx, target("position"), "slider", [] { return bern::Text::Tr("bern.media.position"); },
        [player] { auto s = Media::State(player); return s && s->CanSeek(); },
        Actions{.setNumber = seek, .value = [player] { auto s = Media::Peek(player); return std::optional<std::string>(R::Number(static_cast<double>(s ? s->positionMs : 0))); }});
    Controls::Register(ctx, target("mute"), "switch", [] { return bern::Text::Tr("bern.media.mute"); },
        [player] { auto s = Media::State(player); return s && s->Controllable(); },
        Actions{.activate = toggleMute, .setBool = setMuted, .value = [player] { auto s = Media::Peek(player); return std::optional<std::string>(s && s->muted ? "true" : "false"); }});
    Controls::Register(ctx, target("volume"), "slider", [] { return bern::Text::Tr("bern.media.volume"); },
        [player] { auto s = Media::State(player); return s && s->Controllable(); },
        Actions{.setNumber = setVolume, .value = [player] { auto s = Media::Peek(player); return std::optional<std::string>(R::Number(s ? s->volume : 1.0)); }});

    controls::Grid bar;
    bar.ColumnSpacing(6);
    std::vector<xaml::FrameworkElement> parts{play, positionText, position.slider, durationText, mute, volume.slider};
    for (size_t i = 0; i < parts.size(); ++i) {
        bar.ColumnDefinitions().Append(Column(i == 2 ? Star() : AutoLength()));
        controls::Grid::SetColumn(parts[i], static_cast<int32_t>(i));
        bar.Children().Append(parts[i]);
    }
    return bar;
}

using SwapChainPanelNative2 = std::conditional_t<kWinUI, IBernWinUISwapChainPanelNative2, IBernIslandsSwapChainPanelNative2>;

/// A player's frames on screen: the swap chain its media engine draws
/// into, in a SwapChainPanel. The engine draws as many pixels as the video
/// takes up on screen; the panel is that many DIPs large and scaled back by
/// the display scale, so that each pixel of the swap chain is one on screen.
struct VideoScreen {
    controls::SwapChainPanel panel;
    controls::Canvas canvas;
    media::ScaleTransform scale;
    controls::Grid frame{nullptr};
    std::string fit;
    Media::Frames frames;
    HANDLE shown = nullptr;
    uint64_t token = 0;
    /// Clipped by a rounded Composition clip instead of the frame's own.
    bool rounded = false;

    void Show(Media::Frames const& next) {
        frames = next;
        if (next.chain != shown) {
            shown = next.chain;
            HRESULT result = panel.as<SwapChainPanelNative2>()->SetSwapChainHandle(shown);
            if (FAILED(result) && shown) Report("video: the panel does not take the media engine's swap chain");
        }
        Layout();
    }

    void Layout() {
        double w = frame.ActualWidth(), h = frame.ActualHeight();
        if (w <= 0 || h <= 0) return;
        if (!rounded) {
            media::RectangleGeometry clip;
            clip.Rect({0, 0, static_cast<float>(w), static_cast<float>(h)});
            frame.Clip(clip);
        }
        if (!frames.width || !frames.height) return;
        // The swap chain keeps the video's shape: the fitted rectangle for
        // `contain`, the covering one for `cover`, which the frame crops,
        // and for `fill`, stretched to the frame.
        double aspect = double(frames.width) / frames.height;
        bool wider = w / h > aspect;
        bool covers = fit == "cover" || fit == "fill";
        double cw = wider == covers ? w : h * aspect;
        double ch = wider == covers ? w / aspect : h;
        double dw = fit == "fill" ? w : cw, dh = fit == "fill" ? h : ch;
        auto root = frame.XamlRoot();
        double s = root ? root.RasterizationScale() : 1.0;
        long pw = (std::max)(1L, std::lround(cw * s)), ph = (std::max)(1L, std::lround(ch * s));
        panel.Width(static_cast<double>(pw));
        panel.Height(static_cast<double>(ph));
        scale.ScaleX(dw / pw);
        scale.ScaleY(dh / ph);
        // On whole pixels, so that the frames stay sharp.
        controls::Canvas::SetLeft(panel, std::round((w - dw) / 2 * s) / s);
        controls::Canvas::SetTop(panel, std::round((h - dh) / 2 * s) / s);
        Media::Resize(token, static_cast<uint32_t>(pw), static_cast<uint32_t>(ph));
    }
};

/// A video's picture above its control bar: the picture takes the height
/// it is given, or 16:9 of its width where the height is unbounded, and at
/// least 120.
struct VideoPanel : controls::PanelT<VideoPanel> {
    static constexpr float kGap = 6;

    float Bar(Size available) {
        if (Children().Size() < 2) return 0;
        auto bar = Children().GetAt(1);
        bar.Measure(Size{available.Width, INFINITY});
        return bar.DesiredSize().Height + kGap;
    }

    Size MeasureOverride(Size available) {
        float width = std::isinf(available.Width) ? 640.0f : available.Width;
        float bar = Bar(Size{width, available.Height});
        float picture = std::isinf(available.Height) ? std::round(width * 9.0f / 16) : available.Height - bar;
        picture = (std::max)(120.0f, picture);
        if (Children().Size() > 0) Children().GetAt(0).Measure(Size{width, picture});
        return Size{std::isinf(available.Width) ? width : 0.0f, picture + bar};
    }

    Size ArrangeOverride(Size final) {
        float bar = Children().Size() > 1 ? Children().GetAt(1).DesiredSize().Height + kGap : 0;
        float picture = (std::max)(120.0f, final.Height - bar);
        if (Children().Size() > 0) Children().GetAt(0).Arrange(Rect{0, 0, final.Width, picture});
        if (Children().Size() > 1) Children().GetAt(1).Arrange(Rect{0, picture + kGap, final.Width, (std::max)(0.0f, bar - kGap)});
        return Size{final.Width, picture + bar};
    }
};

struct VideoProps {
    char const* id = "";
    Get source;
    char const* player = nullptr;
    Str label;
    bool controls = true;
    char const* fit = nullptr;
    /// Rounds the picture, following its expression with `animate`.
    Len corner;
    Motion const* animate = nullptr;
};

/// A video of a declaration's own player (`source`) or of an application
/// player, with the standard command-driven control bar.
inline void Video(Parent const& p, Ctx const& ctx, VideoProps props) {
    auto win = ctx.win;
    TextFn labelText = props.label ? Bound(ctx, props.label) : TextFn([] { return bern::Text::Tr("bern.media.video"); });
    std::string playerId = props.player ? std::string(props.player) : win->id + "/" + ctx.Target(props.id);
    if (props.source) {
        if (auto failure = Media::Create(playerId, win->id)) {
            win->Fail(*failure);
        } else {
            Reactive::OnCleanup([playerId] { Media::Release(playerId); });
            auto bound = std::make_shared<std::optional<std::optional<std::string>>>();
            auto source = props.source;
            Reactive::Run([bound, source, ctx, playerId, win] {
                auto url = UrlOf(source(ctx));
                if (*bound && **bound == url) return;
                *bound = url;
                if (auto failure = Reactive::Untrack([&] { return Media::Bind(playerId, url); })) win->Fail(*failure);
            });
        }
    }

    controls::Grid frame;
    frame.MinHeight(120);
    if (auto style = Styles::Themed(L"Grid", {{L"Background", Keys::VideoBackground}})) frame.Style(style);
    auto screen = std::make_shared<VideoScreen>();
    screen->frame = frame;
    screen->fit = props.fit ? props.fit : "contain";
    auto element = screen->panel;
    element.RenderTransform(screen->scale);
    element.IsHitTestVisible(false);
    screen->canvas.IsHitTestVisible(false);
    screen->canvas.Children().Append(element);
    frame.Children().Append(screen->canvas);
    controls::StackPanel status;
    status.Spacing(6);
    status.HorizontalAlignment(xaml::HorizontalAlignment::Center);
    status.VerticalAlignment(xaml::VerticalAlignment::Center);
    status.MaxWidth(420);
    frame.Children().Append(status);
    std::weak_ptr<VideoScreen> weak = screen;
    frame.SizeChanged([weak](auto&&, auto&&) {
        if (auto screen = weak.lock()) screen->Layout();
    });
    // On a display of another scale.
    element.CompositionScaleChanged([weak](auto&&, auto&&) {
        if (auto screen = weak.lock()) screen->Layout();
    });
    screen->token = Media::Show(playerId, [weak](Media::Frames const& frames) {
        if (auto screen = weak.lock()) screen->Show(frames);
    });
    auto token = Media::AttachView(playerId, win->id);
    Reactive::OnCleanup([token, screen] {
        Media::DetachView(token);
        Media::Hide(screen->token);
        if (screen->shown) {
            screen->shown = nullptr;
            try { screen->panel.as<SwapChainPanelNative2>()->SetSwapChainHandle(nullptr); } catch (...) {}
        }
    });
    Reactive::Run([element, status, playerId, labelText] {
        auto state = Media::State(playerId);
        element.Opacity(state && state->ShowsFrames() ? 1.0 : 0.0);
        status.Children().Clear();
        auto add = [&](char const* icon, winrt::hstring const& title, std::optional<std::string> const& detail) {
            auto symbol = Icons::Icon(icon, 32);
            symbol.HorizontalAlignment(xaml::HorizontalAlignment::Center);
            status.Children().Append(symbol);
            controls::TextBlock heading;
            heading.Text(title);
            heading.HorizontalAlignment(xaml::HorizontalAlignment::Center);
            if (auto style = Styles::TextStyle("strong")) heading.Style(style);
            status.Children().Append(heading);
            if (detail) {
                controls::TextBlock note;
                note.Text(Wide(*detail));
                note.TextWrapping(xaml::TextWrapping::Wrap);
                note.TextAlignment(xaml::TextAlignment::Center);
                note.MaxLines(3);
                note.TextTrimming(xaml::TextTrimming::CharacterEllipsis);
                Secondary(note);
                status.Children().Append(note);
            }
        };
        if (!state || state->Is(Phase::Unavailable)) {
            add("film", bern::Text::Tr("bern.media.unavailable"), std::nullopt);
        } else if (state->Is(Phase::Idle)) {
            add("film", bern::Text::Tr("bern.media.empty"), std::nullopt);
        } else if (state->Is(Phase::Loading)) {
            controls::ProgressRing ring;
            ring.IsActive(true);
            ring.Width(32);
            ring.Height(32);
            status.Children().Append(ring);
        } else if (state->Is(Phase::Failed)) {
            add("exclamationmark.triangle", bern::Text::Tr("bern.media.failed"), state->error);
        }
        xaml::Automation::AutomationProperties::SetName(element, labelText());
        xaml::Automation::AutomationProperties::SetHelpText(element, Wide(state ? state->Describe() : std::string()));
    });
    Controls::Register(ctx, props.id, "video", labelText, nullptr,
        Actions{.value = [playerId]() -> std::optional<std::string> {
            if (auto state = Media::Peek(playerId)) return state->Describe();
            return std::nullopt;
        }});
    if (props.corner.Given()) {
        // A rounded Composition clip, which also clips the swap chain.
        screen->rounded = true;
        auto visual = xaml::Hosting::ElementCompositionPreview::GetElementVisual(frame);
        auto compositor = visual.Compositor();
        auto geometry = compositor.CreateRoundedRectangleGeometry();
        visual.Clip(compositor.CreateGeometricClip(geometry));
        frame.SizeChanged([geometry](auto&&, xaml::SizeChangedEventArgs const& args) {
            geometry.Size(float2{args.NewSize().Width, args.NewSize().Height});
        });
        BindLen(ctx, props.corner, props.animate, [geometry](double r) {
            float radius = static_cast<float>((std::max)(0.0, r));
            geometry.CornerRadius(float2{radius, radius});
        });
    }
    // The picture takes the space it is given, or 16:9 of its width.
    auto impl = winrt::make_self<VideoPanel>();
    controls::Panel container = impl.as<controls::Panel>();
    SetRole(container, L"bern-fill");
    container.Children().Append(frame);
    if (props.controls) container.Children().Append(ControlBar(ctx, props.id, playerId));
    if (p.isList) p.Add(Card(container));
    else p.Add(container);
}

}
