// Bern runtime for Windows, XAML part 6 of 7: the controls media
// applications need. Scroll views that come to rest on an item, popover
// and split buttons on flyouts, notices, the inspector pane, drop areas,
// pinch and swipe gestures, the open panel and keyboard shortcuts.
// Generated build input; do not edit.

namespace B {

/// A transparent shield surrounds the editing region. XAML pointer events
/// bubble after the target sees them, so intercepting on the root alone is too
/// late for sliders; these four surfaces receive the outside press first.
inline void FocusRegion(Parent const& p, Ctx const& ctx, Build const& children) {
    controls::ContentControl holder;
    holder.IsTabStop(false);
    holder.UseSystemFocusVisuals(false);
    holder.HorizontalContentAlignment(xaml::HorizontalAlignment::Stretch);
    holder.VerticalContentAlignment(xaml::VerticalAlignment::Stretch);
    controls::Grid content;
    children(Parent::Of(content), ctx);
    holder.Content(content);
    p.Add(holder);
    if (!ctx.win || !ctx.win->root) return;
    auto root = ctx.win->root;
    controls::Canvas shield;
    shield.Visibility(xaml::Visibility::Collapsed);
    controls::Canvas::SetZIndex(shield, 10000);
    controls::Grid::SetRowSpan(shield, (std::max)(1, static_cast<int>(root.RowDefinitions().Size())));
    controls::Grid::SetColumnSpan(shield, (std::max)(1, static_cast<int>(root.ColumnDefinitions().Size())));
    std::array<controls::Border, 4> edges;
    for (auto const& edge : edges) {
        edge.Background(media::SolidColorBrush(winrt::Windows::UI::Color{0, 0, 0, 0}));
        shield.Children().Append(edge);
    }
    root.Children().Append(shield);
    auto held = std::make_shared<std::optional<uint32_t>>();
    auto weakHolder = winrt::make_weak(holder);
    auto weakShield = winrt::make_weak(shield);
    auto weakRoot = winrt::make_weak(root);
    auto update = [weakHolder, weakShield, weakRoot, edges, held] {
        auto holder = weakHolder.get(); auto shield = weakShield.get(); auto root = weakRoot.get();
        if (!holder || !shield || !root) return;
        bool editing = false;
        if (holder.IsLoaded() && holder.Visibility() == xaml::Visibility::Visible) {
            auto focused = xaml::Input::FocusManager::GetFocusedElement(root.XamlRoot());
            if (focused && (focused.try_as<controls::TextBox>() || focused.try_as<controls::PasswordBox>())) {
                for (auto node = focused.try_as<xaml::DependencyObject>(); node; node = media::VisualTreeHelper::GetParent(node)) {
                    if (node == holder) { editing = true; break; }
                }
            }
        }
        bool shown = editing || held->has_value();
        auto visibility = shown ? xaml::Visibility::Visible : xaml::Visibility::Collapsed;
        if (shield.Visibility() != visibility) shield.Visibility(visibility);
        if (!shown) return;
        auto box = holder.TransformToVisual(root).TransformBounds({0, 0, static_cast<float>(holder.ActualWidth()), static_cast<float>(holder.ActualHeight())});
        double width = root.ActualWidth(), height = root.ActualHeight();
        double left = (std::clamp)(static_cast<double>(box.X), 0.0, width);
        double right = (std::clamp)(static_cast<double>(box.X + box.Width), left, width);
        double top = (std::clamp)(static_cast<double>(box.Y), 0.0, height);
        double bottom = (std::clamp)(static_cast<double>(box.Y + box.Height), top, height);
        std::array<std::array<double, 4>, 4> rects{{
            {0, 0, width, top}, {0, bottom, width, height - bottom},
            {0, top, left, bottom - top}, {right, top, width - right, bottom - top}
        }};
        for (size_t i = 0; i < edges.size(); ++i) {
            auto const& edge = edges[i]; auto const& rect = rects[i];
            if (controls::Canvas::GetLeft(edge) != rect[0]) controls::Canvas::SetLeft(edge, rect[0]);
            if (controls::Canvas::GetTop(edge) != rect[1]) controls::Canvas::SetTop(edge, rect[1]);
            if (edge.Width() != rect[2]) edge.Width(rect[2]);
            if (edge.Height() != rect[3]) edge.Height(rect[3]);
        }
    };
    shield.PointerPressed([weakHolder, weakShield, held](auto&&, xaml::Input::PointerRoutedEventArgs const& args) {
        *held = args.Pointer().PointerId();
        args.Handled(true);
        if (auto shield = weakShield.get()) shield.CapturePointer(args.Pointer());
        if (auto holder = weakHolder.get()) holder.Focus(xaml::FocusState::Programmatic);
    });
    auto release = [weakShield, held, update](auto&&, xaml::Input::PointerRoutedEventArgs const& args) {
        if (*held != args.Pointer().PointerId()) return;
        args.Handled(true);
        held->reset();
        if (auto shield = weakShield.get()) shield.ReleasePointerCapture(args.Pointer());
        update();
    };
    shield.PointerReleased(release);
    shield.PointerCanceled(release);
    shield.PointerCaptureLost(release);
    auto focus = root.GotFocus([update](auto&&, auto&&) { update(); });
    auto layout = root.LayoutUpdated([update](auto&&, auto&&) { update(); });
    Reactive::OnCleanup([weakRoot, shield, focus, layout] {
        if (auto root = weakRoot.get()) {
            root.GotFocus(focus);
            root.LayoutUpdated(layout);
            uint32_t index;
            if (root.Children().IndexOf(shield, index)) root.Children().RemoveAt(index);
        }
    });
}

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
    /// Show the scroll bar only for a mouse.
    bool mouseIndicators = false;
    /// Where it opens: the point at this fraction of the content at the
    /// same fraction of the view.
    std::optional<double> openAt;
    /// Where it opens centered: the point at this fraction of the content
    /// in the middle of the view, or as near it as the content reaches.
    Len openCenter;
    /// `anchor`: the item of `position` stands this fraction of the way
    /// down the view (and the item), rather than at its start.
    std::optional<double> anchor;
    /// `browsing`: set while the person scrolls the view, a fling
    /// included.
    std::optional<Field> browsing;
};

/// The scroll offset that puts `item` at the anchor of `scroll`: the point
/// `fraction` of the way along the item at the same fraction of the view
/// (0 its start, 0.5 its center).
inline std::optional<double> OffsetOf(ScrollInfo const& scroll, xaml::UIElement const& item, double fraction) {
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
        double offset = start - (port - extent) * fraction;
        double most = scroll.horizontal ? viewer.ScrollableWidth() : viewer.ScrollableHeight();
        return (std::clamp)(offset, 0.0, (std::max)(0.0, most));
    } catch (winrt::hresult_error const&) {
        return std::nullopt;
    }
}

/// The element of the row of `key` in a grid of `scroll` that builds only
/// the rows near the view, built there; none when no such grid has it.
inline xaml::UIElement RevealItem(ScrollInfo& scroll, std::string const& key) {
    for (auto const& reveal : scroll.reveal)
        if (auto element = reveal(key)) return element;
    return nullptr;
}

/// The items of `scroll` still on screen, by key.
inline std::vector<std::pair<std::string, xaml::UIElement>> ItemsOf(ScrollInfo& scroll) {
    std::vector<std::pair<std::string, xaml::UIElement>> items;
    std::erase_if(scroll.items, [](auto const& item) { return !item.second.get(); });
    for (auto const& [key, weak] : scroll.items)
        if (auto element = weak.get(); element && element.as<xaml::FrameworkElement>().IsLoaded()) items.emplace_back(key, element);
    return items;
}

// ---------------------------------------------------------------------------
// Grids that build only the rows near the view

/// `Parent`, by a name the panel below can use: inside it `Parent` is the
/// element's own `Parent()`.
using RowParent = Parent;

/// A grid of one `ForEach` in a vertical scroll view that builds only the
/// rows near what is in sight: one view's height before and after the
/// effective viewport the scroll view gives it, or what the scroll view
/// shows while that is empty. A row out of sight takes the height it had
/// when last built, or that of the first item built; outside a scroll view
/// every row is built. Rows are built and dropped outside layout, after the
/// sight or the items change. Laid out as `GridPanel`.
struct LazyGridPanel : controls::PanelT<LazyGridPanel> {
    double minWidth = 120;
    double spacing = 8;
    std::optional<double> rowSpacing;
    std::optional<double> cellRatio;
    double cellExtra = 0;
    xaml::Thickness padding{0, 0, 0, 0};
    B::Motion const* reflow = nullptr;
    Places places;
    /// A list from a column (`EachProps::align`).
    bool list = false;

    struct Item {
        std::string key;
        Cell<Value> value;
        std::unique_ptr<Scope> owner;
        controls::Grid host;
        /// The row's region in its host: removing it plays the
        /// `disappear` of what is in it.
        RowParent outer;
        RowParent region;
    };
    /// Rows whose item is gone, playing their `disappear` where they were.
    std::vector<std::pair<controls::Grid, Rect>> leaving;
    /// Where each built row was last arranged, by key.
    std::unordered_map<std::string, Rect> arranged;
    std::vector<Value> values;
    std::vector<std::string> keys;
    std::unordered_map<std::string, std::shared_ptr<Item>> built;
    /// Heights of built items by key, at the width they were measured at;
    /// the guess for the rest.
    std::unordered_map<std::string, float> heights;
    float guess = 0;
    float laidWidth = -1;
    /// In a vertical scroll view, and what of the grid is in sight there.
    bool scrolls = false;
    std::optional<Rect> sight;
    /// No effective viewport to go by. XAML Islands on Windows 10 only ever
    /// gives an empty one, and stops reporting it while the view scrolls.
    bool blind = true;
    /// The last layout: columns, their width, where each row starts.
    size_t columns = 1;
    float cell = 0;
    std::vector<float> starts{0.0f};
    std::function<std::shared_ptr<Item>(size_t)> make;
    std::function<void(std::shared_ptr<Item> const&)> drop;
    bool posted = false;

    float Horizontal() const { return static_cast<float>(padding.Left + padding.Right); }
    float Vertical() const { return static_cast<float>(padding.Top + padding.Bottom); }
    float RowGap() const { return static_cast<float>(rowSpacing.value_or(spacing)); }
    std::optional<float> CellHeight(float width) const {
        if (!cellRatio || *cellRatio <= 0) return std::nullopt;
        return static_cast<float>(width / *cellRatio + cellExtra);
    }
    size_t Rows() const { return starts.size() - 1; }

    /// Where each row starts, from the heights known.
    void Lay() {
        size_t count = keys.size();
        size_t rows = (count + columns - 1) / columns;
        starts.assign(rows + 1, 0.0f);
        auto fixed = CellHeight(cell);
        float at = 0;
        for (size_t row = 0; row < rows; ++row) {
            starts[row] = at;
            float tallest = fixed.value_or(0.0f);
            if (!fixed) {
                for (size_t i = row * columns; i < (std::min)(count, (row + 1) * columns); ++i) {
                    auto found = heights.find(keys[i]);
                    tallest = (std::max)(tallest, found != heights.end() ? found->second : guess);
                }
            }
            at += tallest + RowGap();
        }
        starts[rows] = at;
    }

    /// The first row whose bottom is below `y`.
    size_t RowAt(float y) const {
        size_t low = 0, high = Rows();
        while (low < high) {
            size_t mid = (low + high) / 2;
            if (starts[mid + 1] - RowGap() <= y) low = mid + 1;
            else high = mid;
        }
        return low;
    }

    /// The rows to build, from the first to before the last.
    std::pair<size_t, size_t> Wanted() const {
        size_t rows = Rows();
        if (!scrolls) return {0, rows};
        size_t first = (std::min)(rows, (24 + columns - 1) / columns);
        if (!sight || laidWidth < 0 || (guess <= 0 && !CellHeight(cell))) return {0, first};
        float top = sight->Y - static_cast<float>(padding.Top);
        size_t from = (std::min)(rows, RowAt((std::max)(0.0f, top - sight->Height)));
        size_t to = (std::min)(rows, RowAt((std::max)(0.0f, top + sight->Height * 2)) + 1);
        return {from, (std::max)(from, to)};
    }

    /// Whether the rows built are the rows wanted.
    bool Current() const {
        auto [from, to] = Wanted();
        size_t end = (std::min)(keys.size(), to * columns);
        size_t start = (std::min)(end, from * columns);
        if (built.size() != end - start) return false;
        for (size_t i = start; i < end; ++i)
            if (!built.contains(keys[i])) return false;
        return true;
    }

    /// Build the rows wanted and drop the rest.
    void Update() {
        posted = false;
        if (!make) return;
        auto [from, to] = Wanted();
        std::unordered_map<std::string, std::shared_ptr<Item>> next;
        for (size_t i = from * columns; i < (std::min)(keys.size(), to * columns); ++i) {
            if (auto found = built.find(keys[i]); found != built.end()) {
                next.emplace(keys[i], found->second);
                built.erase(found);
            } else if (auto item = make(i)) {
                next.emplace(keys[i], item);
            }
        }
        // A row whose item is gone plays its `disappear` where it was; a row
        // only scrolled out of range goes at once.
        std::unordered_set<std::string> present(keys.begin(), keys.end());
        for (auto& [key, item] : built) {
            auto place = arranged.find(key);
            if (!present.contains(key) && place != arranged.end()) {
                item->owner->Dispose();
                item->outer.Remove(item->region);
                leaving.emplace_back(item->host, place->second);
            } else {
                drop(item);
            }
            if (place != arranged.end()) arranged.erase(place);
        }
        built = std::move(next);
        // Rows that have left are taken away.
        std::erase_if(leaving, [this](auto const& gone) {
            if (gone.first.Children().Size() > 0) return false;
            uint32_t at = 0;
            if (Children().IndexOf(gone.first, at)) Children().RemoveAt(at);
            return true;
        });
        InvalidateMeasure();
    }

    /// `Update` once layout is done: rows are not added while it runs.
    void Later() {
        if (posted) return;
        posted = true;
        Host::Post([weak = get_weak()] {
            if (auto self = weak.get()) self->Update();
        });
    }

    Size MeasureOverride(Size available) {
        float inner = std::isinf(available.Width) ? 0.0f : (std::max)(0.0f, available.Width - Horizontal());
        // A list from a column given any width wants that of its widest row
        // built, as the column would.
        if (list && std::isinf(available.Width)) {
            for (auto& [key, item] : built) {
                item->host.Measure(Size{INFINITY, INFINITY});
                inner = (std::max)(inner, item->host.DesiredSize().Width);
            }
        }
        // Another width wraps text otherwise: heights are measured again.
        if (inner != laidWidth) {
            laidWidth = inner;
            heights.clear();
            guess = 0;
        }
        auto gap = static_cast<float>(spacing);
        columns = (std::max)(static_cast<size_t>((inner + gap) / static_cast<float>((std::max)(1.0, minWidth) + spacing)), size_t{1});
        cell = (std::max)(0.0f, (inner - gap * static_cast<float>(columns - 1)) / static_cast<float>(columns));
        for (auto& [key, item] : built) {
            item->host.Measure(Size{cell, INFINITY});
            float height = item->host.DesiredSize().Height;
            heights[key] = height;
            if (guess <= 0 && height > 0) guess = height;
        }
        for (auto& [host, place] : leaving) host.Measure(Size{place.Width, place.Height});
        Lay();
        // Rows that have left are taken away after this layout.
        bool left = std::any_of(leaving.begin(), leaving.end(), [](auto const& gone) { return gone.first.Children().Size() == 0; });
        if (!Current() || left) Later();
        float total = Rows() ? starts.back() - RowGap() : 0.0f;
        return Size{inner + Horizontal(), total + Vertical()};
    }

    Size ArrangeOverride(Size final) {
        auto gap = static_cast<float>(spacing);
        auto fixed = CellHeight(cell);
        std::vector<std::pair<xaml::UIElement, winrt::Windows::Foundation::Point>> now;
        for (size_t i = 0; i < keys.size() && i / columns < Rows(); ++i) {
            auto found = built.find(keys[i]);
            if (found == built.end()) continue;
            size_t row = i / columns;
            float rowHeight = starts[row + 1] - RowGap() - starts[row];
            // In a row of cells, taller content runs into the gap below.
            float height = fixed ? (std::max)(*fixed, found->second->host.DesiredSize().Height) : rowHeight;
            Rect rect{static_cast<float>(padding.Left) + static_cast<float>(i % columns) * (cell + gap), static_cast<float>(padding.Top) + starts[row], cell, height};
            found->second->host.Arrange(rect);
            arranged[keys[i]] = rect;
            if (reflow) now.emplace_back(found->second->host, winrt::Windows::Foundation::Point{rect.X, rect.Y});
        }
        for (auto& [host, place] : leaving) host.Arrange(place);
        if (reflow) places.Arranged(reflow, std::move(now));
        return final;
    }
};

/// `Grid` whose items are one `ForEach`: in a vertical scroll view it builds
/// only the rows near what is in sight (`LazyGridPanel`); elsewhere it
/// builds them all. A built row keeps its elements, controls and state
/// while its key stays and it is near the view; its value is a cell the
/// row's bindings read.
inline void LazyGrid(Parent const& p, Ctx const& ctx, GridProps props, EachProps each, Build row) {
    auto impl = winrt::make_self<LazyGridPanel>();
    impl->reflow = props.reflow ? props.reflow : each.reflow;
    controls::Panel panel = impl.as<controls::Panel>();
    SetRole(panel, L"bern-fill-width");
    LazyGridPanel* grid = impl.get();
    KeepReflow(panel, [grid](Motion const* motion) { grid->reflow = motion; });
    grid->scrolls = ctx.scroll && !ctx.scroll->horizontal;
    auto keep = winrt::make_weak(panel);
    auto relayout = [keep] {
        if (auto panel = keep.get()) panel.InvalidateMeasure();
    };
    BindLen(ctx, props.minWidth, props.animate, [grid, relayout](double v) {
        grid->minWidth = (std::max)(1.0, v);
        relayout();
    });
    BindLen(ctx, props.gap, props.animate, [grid, relayout](double v) {
        grid->spacing = (std::max)(0.0, v);
        relayout();
    });
    BindPadding(ctx, props.padding, props.paddingX, props.paddingY, props.animate, [grid, relayout](double x, double y) {
        grid->padding = xaml::Thickness{x, y, x, y};
        relayout();
    });
    BindLen(ctx, props.rowGap, props.animate, [grid, relayout](double v) {
        grid->rowSpacing = (std::max)(0.0, v);
        relayout();
    });
    grid->cellRatio = props.cellRatio;
    BindLen(ctx, props.cellExtra, props.animate, [grid, relayout](double v) {
        grid->cellExtra = v;
        relayout();
    });
    // Rows added in one update take turns with `stagger`, and play their
    // `appear` transition; rows built as the view scrolls do not.
    auto stagger = each.stagger ? each.stagger : props.stagger;
    auto rowsCtx = stagger ? Staggered(ctx, *stagger) : ctx;
    auto animated = std::make_shared<bool>(false);
    std::string name = each.variable;
    auto scroll = ctx.scroll;
    std::optional<xaml::HorizontalAlignment> across;
    if (each.align) {
        std::string_view align = each.align;
        across = align == "center" ? xaml::HorizontalAlignment::Center
            : align == "trailing" ? xaml::HorizontalAlignment::Right
            : xaml::HorizontalAlignment::Left;
    }
    grid->make = [grid, row, rowsCtx, name, scroll, animated, across](size_t i) {
        auto id = grid->keys[i];
        controls::Grid host;
        auto outer = across ? Parent::Aligned(host, across, std::nullopt) : Parent::Of(host);
        auto region = outer.Sub();
        auto item = std::make_shared<LazyGridPanel::Item>(LazyGridPanel::Item{id, Cell<Value>(grid->values[i]), std::make_unique<Scope>(), host, outer, region});
        auto cell = item->value;
        auto rowCtx = rowsCtx.WithKey([id] { return id; }).WithLocal(name, [cell] { return cell.Get(); });
        Structure::Scope structural(*animated);
        Reactive::Untrack([&] {
            item->owner->Run([&] { row(item->region, rowCtx); });
            return 0;
        });
        grid->Children().Append(item->host);
        // The row's element, known by its key: scroll views find their
        // items this way.
        if (scroll && item->host.Children().Size() > 0)
            scroll->items.emplace_back(id, winrt::make_weak(item->host.Children().GetAt(0)));
        return item;
    };
    grid->drop = [grid](std::shared_ptr<LazyGridPanel::Item> const& item) {
        item->owner->Dispose();
        uint32_t at = 0;
        if (grid->Children().IndexOf(item->host, at)) grid->Children().RemoveAt(at);
    };
    // What the scroll view shows, in the grid's coordinates, for when the
    // effective viewport says nothing.
    auto look = [keep, grid, scroll] {
        auto panel = keep.get();
        if (!panel || !grid->scrolls || !scroll->viewer) return;
        auto viewer = scroll->viewer;
        auto port = static_cast<float>(viewer.ViewportHeight());
        if (port <= 0) return;
        try {
            auto content = viewer.Content().as<xaml::UIElement>();
            auto origin = panel.TransformToVisual(content).TransformPoint({0, 0});
            grid->sight = Rect{0, static_cast<float>(viewer.VerticalOffset()) - origin.Y, static_cast<float>(panel.ActualWidth()), port};
        } catch (winrt::hresult_error const&) {
        }
    };
    auto sighted = panel.EffectiveViewportChanged([grid, look](xaml::FrameworkElement const&, xaml::EffectiveViewportChangedEventArgs const& args) {
        auto viewport = args.EffectiveViewport();
        grid->blind = viewport.Height < 0;
        if (grid->blind) look();
        else grid->sight = viewport;
        if (grid->scrolls && !grid->Current()) grid->Later();
    });
    winrt::event_token viewed{}, sized{};
    // A scroll view's `position` reaches rows not built yet.
    if (grid->scrolls) {
        auto follow = [grid, look](auto&&, auto&&) {
            if (!grid->blind) return;
            look();
            if (!grid->Current()) grid->Later();
        };
        viewed = scroll->viewer.ViewChanged(follow);
        sized = scroll->viewer.SizeChanged(follow);
        scroll->reveal.push_back([keep, grid, scroll](std::string const& key) -> xaml::UIElement {
            auto panel = keep.get();
            if (!panel || !grid->make || !scroll->viewer) return nullptr;
            auto at = std::find(grid->keys.begin(), grid->keys.end(), key);
            if (at == grid->keys.end()) return nullptr;
            size_t row = static_cast<size_t>(at - grid->keys.begin()) / grid->columns;
            if (row >= grid->Rows()) return nullptr;
            // Stand near the row, and build the rows in sight there.
            auto viewer = scroll->viewer;
            float port = static_cast<float>(viewer.ViewportHeight());
            float y = static_cast<float>(grid->padding.Top) + grid->starts[row];
            try {
                auto content = viewer.Content().as<xaml::UIElement>();
                auto origin = panel.TransformToVisual(content).TransformPoint({0, 0});
                viewer.ChangeView(nullptr, static_cast<double>(origin.Y + y), nullptr, true);
            } catch (winrt::hresult_error const&) {
            }
            grid->sight = Rect{0, y, static_cast<float>(panel.ActualWidth()), port};
            grid->Update();
            auto found = grid->built.find(key);
            if (found == grid->built.end() || found->second->host.Children().Size() == 0) return nullptr;
            return found->second->host.Children().GetAt(0);
        });
    }
    bool fromColumn = each.align != nullptr;
    grid->list = fromColumn;
    Reactive::Run([grid, fromColumn, items = each.items, key = each.key, ctx = rowsCtx, animated, first = std::make_shared<bool>(true)] {
        auto list = R::AsList(items(ctx));
        // A list without rows takes no place, nor a gap, in its column.
        if (fromColumn) {
            auto wanted = list.empty() ? xaml::Visibility::Collapsed : xaml::Visibility::Visible;
            if (grid->Visibility() != wanted) {
                grid->Visibility(wanted);
                // Collapsed, it was never measured, and its containers were
                // laid out without it: measured again from the old, empty
                // constraint it would stay zero wide, its rows built and out
                // of sight, until the window changed size. They measure it
                // again, up to the scroll view.
                xaml::DependencyObject at = *grid;
                while (at) {
                    if (auto element = at.try_as<xaml::UIElement>()) element.InvalidateMeasure();
                    if (at.try_as<controls::ScrollViewer>()) break;
                    at = media::VisualTreeHelper::GetParent(at);
                }
            }
        }
        Reactive::Untrack([&] {
            grid->values.assign(list.begin(), list.end());
            grid->keys.clear();
            for (auto const& item : list) grid->keys.push_back(R::KeyText(R::At(item, {key})));
            // Rows still near the view follow their items' new values.
            for (size_t i = 0; i < list.size(); ++i)
                if (auto found = grid->built.find(grid->keys[i]); found != grid->built.end()) found->second->value.Set(list[i]);
            *animated = !*first;
            *first = false;
            grid->Lay();
            grid->Update();
            *animated = false;
            return 0;
        });
    });
    Reactive::OnCleanup([impl, sighted, scroll, viewed, sized] {
        impl->EffectiveViewportChanged(sighted);
        if (impl->scrolls) {
            scroll->viewer.ViewChanged(viewed);
            scroll->viewer.SizeChanged(sized);
        }
        for (auto& [key, item] : impl->built) item->owner->Dispose();
        impl->built.clear();
        impl->make = nullptr;
        impl->drop = nullptr;
    });
    Place(p, panel);
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
    if (props.mouseIndicators) {
        // `Auto` shows the bar as a mouse moves over the view; touch and
        // pen hide it before panning would show their thin indicator.
        using PointerHandler = xaml::Input::PointerEventHandler;
        auto follow = PointerHandler([horizontal](IInspectable const& sender, xaml::Input::PointerRoutedEventArgs const& args) {
            auto viewer = sender.as<controls::ScrollViewer>();
            auto shown = args.Pointer().PointerDeviceType() == PointerKind::Mouse ? controls::ScrollBarVisibility::Auto : controls::ScrollBarVisibility::Hidden;
            if ((horizontal ? viewer.HorizontalScrollBarVisibility() : viewer.VerticalScrollBarVisibility()) == shown) return;
            if (horizontal) viewer.HorizontalScrollBarVisibility(shown);
            else viewer.VerticalScrollBarVisibility(shown);
        });
        scroller.AddHandler(xaml::UIElement::PointerEnteredEvent(), winrt::box_value(follow), true);
        scroller.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(follow), true);
    }
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
    if (props.openCenter.Given()) {
        // Read once, untracked: the view opens there and then scrolls freely.
        double at = (std::clamp)(Reactive::Untrack([&] { return props.openCenter.Now(ctx); }), 0.0, 1.0);
        auto opened = std::make_shared<bool>(false);
        scroller.LayoutUpdated([weak = winrt::make_weak(scroller), at, opened, horizontal](auto&&, auto&&) {
            auto viewer = weak.get();
            if (!viewer || *opened) return;
            double room = horizontal ? viewer.ScrollableWidth() : viewer.ScrollableHeight();
            if (room <= 0) return;
            *opened = true;
            double port = horizontal ? viewer.ViewportWidth() : viewer.ViewportHeight();
            double middle = (std::clamp)(at * (room + port) - port / 2, 0.0, room);
            if (horizontal) viewer.ChangeView(middle, nullptr, nullptr, true);
            else viewer.ChangeView(nullptr, middle, nullptr, true);
        });
    }
    // Scrolled by hand: the view moves, and not because it was asked to.
    if (props.browsing) {
        auto field = *props.browsing;
        scroller.ViewChanged([scroll, ctx, field](auto&&, controls::ScrollViewerViewChangedEventArgs const& args) {
            bool byHand = args.IsIntermediate() && !scroll->byCode;
            if (!args.IsIntermediate()) scroll->byCode = false;
            if (R::Truthy(ctx.Peek(field)) != byHand) ctx.Write(field, Value::Bool(byHand));
        });
    }
    if (!props.snap && !props.position) return;
    // Items come to rest at the center only with `snap: center`; a position
    // alone is kept at the start, or at its `anchor`, as on the other
    // targets.
    bool center = props.snap && std::string_view(props.snap) == "center";
    double fraction = center ? 0.5 : props.anchor.value_or(0);
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
    auto nearest = [scroll, fraction]() -> std::optional<std::pair<std::string, xaml::UIElement>> {
        double now = scroll->horizontal ? scroll->viewer.HorizontalOffset() : scroll->viewer.VerticalOffset();
        std::optional<std::pair<std::string, xaml::UIElement>> best;
        double distance = INFINITY;
        for (auto const& item : ItemsOf(*scroll)) {
            auto offset = OffsetOf(*scroll, item.second, fraction);
            if (!offset) continue;
            double gap = std::abs(*offset - now);
            if (gap < distance) {
                distance = gap;
                best = item;
            }
        }
        return best;
    };
    auto scrollTo = [scroll, fraction](xaml::UIElement const& item, bool smooth) {
        auto offset = OffsetOf(*scroll, item, fraction);
        if (!offset) return;
        scroll->byCode = true;
        if (scroll->horizontal) scroll->viewer.ChangeView(*offset, nullptr, nullptr, !smooth || Reduced());
        else scroll->viewer.ChangeView(nullptr, *offset, nullptr, !smooth || Reduced());
    };
    // Scrolling that ends between items comes to rest on the nearest one,
    // unless the position stands at an anchor of its own: then the view
    // rests where the person leaves it.
    auto travelling = std::make_shared<winrt::weak_ref<xaml::UIElement>>();
    auto placed = std::make_shared<bool>(false);
    auto position = props.position;
    bool nudges = props.arrival && std::string_view(props.arrival) == "nudge";
    auto nudgeMotion = props.nudgeMotion;
    scroller.ViewChanged([scroll, nearest, scrollTo, travelling, placed, position, ctx, nudges, nudgeMotion, fraction, snaps = props.snap || !props.anchor](auto&&, controls::ScrollViewerViewChangedEventArgs const& args) {
        if (args.IsIntermediate() || !*placed) return;
        auto item = nearest();
        if (!item) return;
        double now = scroll->horizontal ? scroll->viewer.HorizontalOffset() : scroll->viewer.VerticalOffset();
        if (auto arrived = travelling->get()) {
            // A scroll the field asked for: until it reaches its item, the
            // view goes on to it (a row built only now has the view stand
            // near it first) rather than resting on another.
            auto at = OffsetOf(*scroll, arrived, fraction);
            if (at && std::abs(*at - now) > 1) return;
            *travelling = nullptr;
            if (nudges && arrived == item->second) NudgeOnce(arrived, nudgeMotion);
            return;
        }
        auto offset = OffsetOf(*scroll, item->second, fraction);
        if (snaps && offset && std::abs(*offset - now) > 1) {
            scrollTo(item->second, true);
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
                return 0;
            }
            // A row of a grid not built yet: built near the view, then
            // scrolled to once it is laid out.
            if (auto item = RevealItem(*scroll, key)) {
                *travelling = winrt::make_weak(item);
                Host::After(std::chrono::milliseconds(40), [scrollTo, weak = winrt::make_weak(item)] {
                    if (auto item = weak.get()) scrollTo(item, true);
                });
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
            bool found = false;
            for (auto const& [itemKey, item] : ItemsOf(*scroll))
                if (itemKey == key) {
                    scrollTo(item, false);
                    found = true;
                }
            if (!found) {
                if (auto item = RevealItem(*scroll, key)) {
                    Host::After(std::chrono::milliseconds(40), [scrollTo, weak = winrt::make_weak(item)] {
                        if (auto item = weak.get()) scrollTo(item, false);
                    });
                }
            }
            Host::After(std::chrono::milliseconds(140), [placed] { *placed = true; });
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

/// The shell's open dialog, for a folder or for `types`; the path chosen,
/// nothing on cancel. It runs its own message loop.
inline std::optional<std::wstring> OpenDialog(HWND hwnd, bool folder, std::vector<std::wstring> const& types) {
    auto dialog = winrt::create_instance<IFileOpenDialog>(CLSID_FileOpenDialog);
    FILEOPENDIALOGOPTIONS options{};
    winrt::check_hresult(dialog->GetOptions(&options));
    options |= FOS_FORCEFILESYSTEM;
    if (folder) options |= FOS_PICKFOLDERS;
    winrt::check_hresult(dialog->SetOptions(options));
    std::wstring spec;
    for (auto const& type : types) {
        if (!spec.empty()) spec += L';';
        spec += type == L"*" ? L"*.*" : L"*" + type;
    }
    if (!folder) {
        COMDLG_FILTERSPEC filter{spec.c_str(), spec.c_str()};
        winrt::check_hresult(dialog->SetFileTypes(1, &filter));
    }
    auto shown = dialog->Show(hwnd);
    if (shown == HRESULT_FROM_WIN32(ERROR_CANCELLED)) return std::nullopt;
    winrt::check_hresult(shown);
    winrt::com_ptr<IShellItem> item;
    winrt::check_hresult(dialog->GetResult(item.put()));
    PWSTR path = nullptr;
    winrt::check_hresult(item->GetDisplayName(SIGDN_FILESYSPATH, &path));
    std::wstring chosen(path);
    CoTaskMemFree(path);
    return chosen;
}

/// The system's open panel for `kinds`, on the context's window; `give`
/// runs with the chosen file's URL, nothing on cancel. Not the WinRT pickers:
/// in a XAML island on Windows 10 they show nothing and report nothing.
inline void PickFile(Ctx ctx, char const* kinds, std::function<void(std::string const&)> give) {
    bool folder = false;
    for (auto const& word : FileWords(kinds)) folder = folder || word == "folder";
    auto types = PanelTypes(kinds);
    bool windowed = ctx.win != nullptr;
    auto hwnd = windowed ? ctx.win->hwnd : nullptr;
    auto alive = windowed ? std::weak_ptr<bool>(ctx.win->life) : std::weak_ptr<bool>();
    // After the click that asked: the dialog's loop would run inside it.
    Host::Post([=] {
        try {
            auto chosen = OpenDialog(hwnd, folder, types);
            if (chosen && (!windowed || !alive.expired())) give(Host::FileUrl(*chosen));
        } catch (winrt::hresult_error const& error) {
            Report("open panel: " + Utf8(error.message()));
        }
    });
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

/// Whether a `ShortcutInput` waits for a key: it takes every key
/// meanwhile, and no accelerator runs.
inline bool shortcutRecording = false;

/// A text in Bern's form, modifiers in order and the key in lower case;
/// "" for an empty text, nothing for one that is no shortcut.
inline std::optional<std::string> NormalizedShortcut(std::string const& text) {
    std::vector<std::string> parts;
    std::string part;
    for (char c : text + "+") {
        if (c == '+') {
            auto begin = part.find_first_not_of(" \t");
            auto end = part.find_last_not_of(" \t");
            parts.push_back(begin == std::string::npos ? std::string() : part.substr(begin, end - begin + 1));
            part.clear();
        } else {
            part += static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
        }
    }
    if (parts.size() == 1 && parts[0].empty()) return std::string();
    auto key = parts.back();
    parts.pop_back();
    static std::vector<std::string> const order{"cmd", "ctrl", "alt", "shift"};
    static std::vector<std::string> const named{"space", "escape", "return", "tab", "delete", "up", "down", "left",
        "right", "home", "end", "pageup", "pagedown"};
    for (size_t i = 0; i < parts.size(); ++i) {
        if (std::find(order.begin(), order.end(), parts[i]) == order.end()) return std::nullopt;
        if (std::find(parts.begin() + i + 1, parts.end(), parts[i]) != parts.end()) return std::nullopt;
    }
    bool printable = key.size() == 1 && key[0] > 32 && key[0] < 127;
    if (!printable && std::find(named.begin(), named.end(), key) == named.end()) return std::nullopt;
    std::string out;
    for (auto const& modifier : order)
        if (std::find(parts.begin(), parts.end(), modifier) != parts.end()) out += modifier + "+";
    return out + key;
}

/// The name Bern gives a virtual key, as shortcuts write keys.
inline std::optional<std::string> KeyName(winrt::Windows::System::VirtualKey key) {
    using winrt::Windows::System::VirtualKey;
    static std::map<VirtualKey, std::string> const named{
        {VirtualKey::Space, "space"}, {VirtualKey::Escape, "escape"}, {VirtualKey::Enter, "return"}, {VirtualKey::Tab, "tab"},
        {VirtualKey::Back, "delete"}, {VirtualKey::Delete, "delete"}, {VirtualKey::Up, "up"}, {VirtualKey::Down, "down"},
        {VirtualKey::Left, "left"}, {VirtualKey::Right, "right"}, {VirtualKey::Home, "home"}, {VirtualKey::End, "end"},
        {VirtualKey::PageUp, "pageup"}, {VirtualKey::PageDown, "pagedown"},
    };
    if (auto found = named.find(key); found != named.end()) return found->second;
    int code = static_cast<int>(key);
    if (code >= 'A' && code <= 'Z') return std::string(1, static_cast<char>('a' + (code - 'A')));
    if (code >= '0' && code <= '9') return std::string(1, static_cast<char>(code));
    static std::map<int, char> const symbols{{VK_OEM_COMMA, ','}, {VK_OEM_PERIOD, '.'}, {VK_OEM_MINUS, '-'}, {VK_OEM_PLUS, '='},
        {VK_OEM_1, ';'}, {VK_OEM_2, '/'}, {VK_OEM_3, '`'}, {VK_OEM_4, '['}, {VK_OEM_5, '\\'}, {VK_OEM_6, ']'}, {VK_OEM_7, '\''}};
    if (auto found = symbols.find(code); found != symbols.end()) return std::string(1, found->second);
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
        if (shortcutRecording) return;
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

/// While mounted, the keys the text `keys` names call `invoked`: the
/// accelerator follows the text as it changes, and none stands while it is
/// empty or no shortcut.
inline void BoundAccelerator(Ctx const& ctx, Str keys, std::function<bool()> invoked) {
    Reactive::Run([ctx, keys, invoked] {
        auto normal = NormalizedShortcut(Utf8(keys(ctx)));
        if (normal && !normal->empty()) Accelerator(ctx, normal->c_str(), invoked);
    });
}

/// What the shortcut of the control `id` in this context does: its
/// `action` (`activate` or `focus`), as the external entries do; a split
/// button's flips its toggle. A disabled control's shortcut does nothing.
inline std::function<bool()> ControlShortcut(Ctx const& ctx, char const* id, char const* action) {
    auto target = ctx.Target(id);
    std::string kind = action ? action : "activate";
    return [target, kind] {
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
    };
}

/// While mounted, `keys` runs the shortcut of the control `id`.
inline void Shortcut(Ctx const& ctx, char const* keys, char const* id, char const* action) {
    Accelerator(ctx, keys, ControlShortcut(ctx, id, action));
}

/// While mounted, the keys the text `keys` names run the shortcut of the
/// control `id`.
inline void Shortcut(Ctx const& ctx, Str keys, char const* id, char const* action) {
    BoundAccelerator(ctx, std::move(keys), ControlShortcut(ctx, id, action));
}

struct KeyShortcutProps {
    char const* keys = nullptr;
    /// The text the keys follow instead of `keys`.
    Str bound;
    Get enabled;
    Act action;
};

/// `Shortcut`: its keys run the action while it is mounted and enabled,
/// without a control on screen.
inline void KeyShortcut(Parent const&, Ctx const& ctx, KeyShortcutProps props) {
    auto enabled = props.enabled;
    auto action = props.action;
    if (!action) return;
    std::function<bool()> invoked = [ctx, enabled, action] {
        if (enabled && !Reactive::Untrack([&] { return R::Truthy(enabled(ctx)); })) return false;
        action(ctx);
        return true;
    };
    if (props.bound) BoundAccelerator(ctx, props.bound, std::move(invoked));
    else Accelerator(ctx, props.keys, std::move(invoked));
}

struct ShortcutInputProps {
    char const* id = "";
    Str label;
    /// The shortcut the state holds; a choice runs `change` with it.
    Get current;
    std::function<void(Ctx const&, Value const&)> change;
    Str prompt;
    Str empty;
    Get enabled;
    bool labelHidden = false;
};

/// `ShortcutInput`: a button showing the shortcut as Windows writes it;
/// pressed, it takes the next key with its modifiers. Escape leaves the
/// shortcut as it was, Backspace or Delete alone clears it, and leaving the
/// button stops waiting. While it waits, no accelerator runs.
inline void ShortcutInput(Parent const& p, Ctx const& ctx, ShortcutInputProps props) {
    auto model = ModelOf(ctx, std::nullopt, nullptr, props.current, props.change);
    if (!model.read) return;
    auto text = Bound(ctx, props.label);
    auto prompt = Bound(ctx, props.prompt);
    auto empty = Bound(ctx, props.empty);
    auto isEnabled = Enabled(ctx, props.enabled);
    Cell<bool> recording(false);
    controls::Button button;
    Reactive::Run([button, model, recording, prompt, empty] {
        if (recording.Get()) {
            button.Content(winrt::box_value(prompt ? prompt() : bern::Text::Tr("bern.shortcut.prompt")));
            return;
        }
        auto keys = R::AsText(model.read());
        auto shown = keys.empty() ? winrt::hstring() : ShortcutText(keys.c_str());
        button.Content(winrt::box_value(shown.empty() ? (empty ? empty() : bern::Text::Tr("bern.shortcut.none")) : shown));
    });
    Reactive::Run([recording] { shortcutRecording = recording.Get(); });
    Reactive::OnCleanup([recording] { if (recording.Peek()) shortcutRecording = false; });
    if (isEnabled)
        Reactive::Run([button, isEnabled, recording] {
            bool on = isEnabled();
            button.IsEnabled(on);
            // A copy shares the cell; the captured one is const here.
            auto waiting = recording;
            if (!on) waiting.Set(false);
        });
    button.Click([recording](auto&&, auto&&) {
        auto waiting = recording;
        waiting.Set(!recording.Peek());
    });
    button.LostFocus([recording](auto&&, auto&&) {
        auto waiting = recording;
        waiting.Set(false);
    });
    button.PreviewKeyDown([recording, model](auto&&, xaml::Input::KeyRoutedEventArgs const& args) {
        if (!recording.Peek()) return;
        args.Handled(true);
        using winrt::Windows::System::VirtualKey;
        auto key = args.Key();
        auto waiting = recording;
        if (key == VirtualKey::Escape) {
            waiting.Set(false);
            return;
        }
        auto down = [](int code) { return (GetKeyState(code) & 0x8000) != 0; };
        std::string keys;
        if (down(VK_CONTROL)) keys += "cmd+";
        if (down(VK_MENU)) keys += "alt+";
        if (down(VK_SHIFT)) keys += "shift+";
        auto name = KeyName(key);
        // Modifiers alone, and keys Bern has no name for, wait for another.
        if (!name) return;
        waiting.Set(false);
        model.write(Value::Text(keys.empty() && *name == "delete" ? std::string() : keys + *name));
    });
    BindName(button, text);
    KeepWidth(button);
    if (p.isList) p.Add(props.labelHidden ? xaml::UIElement(Card(button)) : xaml::UIElement(LabeledCard(text, button)));
    else if (props.labelHidden || !text) p.Add(button);
    else p.Add(LabeledCard(text, button));
    Controls::Register(ctx, props.id, "shortcut_input", text, isEnabled,
        Actions{.edit = [model](std::string const& value) {
                    if (auto keys = NormalizedShortcut(value)) model.write(Value::Text(*keys));
                },
            .value = [model] { return std::optional<std::string>(R::AsText(model.peek())); }});
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

/// The platform's in-app acrylic in the dark or light appearance: the
/// material of transient surfaces, blurring the window's content behind.
inline media::Brush InAppAcrylic(bool dark) {
    wchar_t const* key = kWinUI ? L"AcrylicInAppFillColorDefaultBrush" : L"SystemControlAcrylicElementBrush";
    auto resources = xaml::Application::Current().Resources();
    for (auto theme : dark ? std::initializer_list<wchar_t const*>{L"Dark", L"Default"} : std::initializer_list<wchar_t const*>{L"Light"})
        if (auto value = ThemeValue(resources, theme, key))
            if (auto brush = value.try_as<media::Brush>()) return brush;
    return nullptr;
}

inline std::shared_ptr<Popover> MakePopover(Ctx const& ctx, Build content, bool windowTheme = false, StyleDef const* style = nullptr) {
    auto popover = std::make_shared<Popover>();
    auto inner = ctx;
    if (windowTheme) inner.appearance.clear();
    if (auto inherited = PresenterTheme(L"FlyoutPresenter", inner.appearance)) popover->flyout.FlyoutPresenterStyle(inherited);
    // Without a color of its own the bubble is the platform's in-app
    // acrylic, blurring the window's content under it (a video): it stays
    // inside the window, as a popup window of its own would show only the
    // acrylic's fallback color.
    bool material = !(style && style->base.background);
    if (material) popover->flyout.ShouldConstrainToRootBounds(true);
    std::weak_ptr<Popover> weak = popover;
    auto apply = [weak, ctx, inner, windowTheme, style, material] {
        auto popover = weak.lock();
        if (!popover || (!windowTheme && !style && !material)) return;
        bool dark = inner.appearance.empty() ? IsDark(ctx.win->root) : inner.appearance == "dark";
        auto theme = dark ? xaml::ElementTheme::Dark : xaml::ElementTheme::Light;
        auto look = xaml::Markup::XamlReader::Load(LR"(<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="FlyoutPresenter" />)").as<xaml::Style>();
        // The platform's material (acrylic) in the bubble's own appearance:
        // the default style would take it from the window's.
        if (auto themed = PresenterTheme(L"FlyoutPresenter", dark ? "dark" : "light")) look.BasedOn(themed);
        look.Setters().Append(xaml::Setter(xaml::FrameworkElement::RequestedThemeProperty(), winrt::box_value(theme)));
        if (style && style->base.background) look.Setters().Append(xaml::Setter(controls::Control::BackgroundProperty(), media::SolidColorBrush(ColorOf(ctx.win, dark, *style->base.background))));
        else if (auto acrylic = InAppAcrylic(dark)) look.Setters().Append(xaml::Setter(controls::Control::BackgroundProperty(), acrylic));
        if (style && style->base.foreground) look.Setters().Append(xaml::Setter(controls::Control::ForegroundProperty(), media::SolidColorBrush(ColorOf(ctx.win, dark, *style->base.foreground))));
        popover->flyout.FlyoutPresenterStyle(look);
        if (popover->content) {
            popover->content.RequestedTheme(theme);
            // Already-open flyouts must change alongside their window.
            auto node = media::VisualTreeHelper::GetParent(popover->content);
            while (node) {
                if (auto presenter = node.try_as<controls::FlyoutPresenter>()) { presenter.Style(look); break; }
                node = media::VisualTreeHelper::GetParent(node);
            }
        }
    };
    apply();
    if (ctx.win && ctx.win->root) {
        auto root = ctx.win->root;
        auto token = root.ActualThemeChanged([apply](auto&&, auto&&) { apply(); });
        Reactive::OnCleanup([root, token] { root.ActualThemeChanged(token); });
    }
    popover->flyout.Opening([weak, content, inner, apply](auto&&, auto&&) {
        if (auto popover = weak.lock()) { popover->Mount(content, inner); apply(); }
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
    bool windowTheme = false;
    StyleDef const* style = nullptr;
    Get current;
    std::function<void(Ctx const&, Value const&)> change;
    /// `prominence: plain`: words without a background, as a plain button.
    bool plain = false;
    /// `font_size`: the size of the icon.
    std::optional<double> iconSize;
};

/// A button that shows its children in a popover, such as a volume slider.
/// The children mount while the popover shows. `presented` follows the
/// popover and opens or closes it; external entries open it with
/// `activate`.
inline void PopoverButton(Parent const& p, Ctx const& ctx, PopoverButtonProps props, Build const& content) {
    controls::Button button;
    auto text = Bound(ctx, props.label);
    if (props.icon) {
        // As an `IconButton`: the icon with 10 on each side, no background
        // until pointed at.
        double size = props.iconSize.value_or(16);
        button.Content(Icons::Icon(props.icon, size));
        Styles::MakeSubtle(button);
        button.Width(size + 20);
        button.Height(size + 20);
        button.MinWidth(0);
        button.MinHeight(0);
        button.Padding(xaml::Thickness{0, 0, 0, 0});
    } else {
        button.Content(TextOf(text));
        if (props.plain) Styles::MakeSubtle(button);
    }
    if (text)
        Reactive::Run([button, text] {
            auto name = text();
            controls::ToolTipService::SetToolTip(button, winrt::box_value(name));
            xaml::Automation::AutomationProperties::SetName(button, name);
        });
    auto popover = MakePopover(ctx, content, props.windowTheme, props.style);
    button.Flyout(popover->flyout);
    auto model = ModelOf(ctx, props.presented, nullptr, props.current, props.change);
    if (model.read) {
        popover->flyout.Opened([model](auto&&, auto&&) {
            if (model.peek() != Value::Bool(true)) model.write(Value::Bool(true));
        });
        popover->flyout.Closed([model](auto&&, auto&&) {
            if (model.peek() != Value::Bool(false)) model.write(Value::Bool(false));
        });
        Reactive::Run([model, popover, weak = winrt::make_weak(button)] {
            bool want = model.read() == Value::Bool(true);
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
    /// Runs after the draft changed.
    Act then;
    /// `checked:`: engine state the toggle shows; a change runs `change`
    /// with the new value.
    Get current;
    std::function<void(Ctx const&, Value const&)> change;
    Str more;
    std::optional<ControlShape> shape;
};

/// A toggle linked with a part that shows `content` in a popover, such as
/// the settings of what it turns on: the toggle split button of Windows.
/// External entries turn it on or off with `set_bool` and open the popover
/// with `activate`.
inline void SplitButton(Parent const& p, Ctx const& ctx, SplitButtonProps props, Build const& content) {
    auto model = ModelOf(ctx, props.field, props.then, props.current, props.change);
    if (!model.read) return;
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
    Reactive::Run([button, model, updating] {
        bool on = model.read() == Value::Bool(true);
        if (button.IsChecked() != on) {
            *updating = true;
            button.IsChecked(on);
            *updating = false;
        }
    });
    button.IsCheckedChanged([updating, model](controls::ToggleSplitButton const& sender, auto&&) {
        if (!*updating) model.write(Value::Bool(sender.IsChecked()));
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
            .setBool = [model](bool on) { model.write(Value::Bool(on)); },
            .value = [model] { return std::optional<std::string>(model.peek() == Value::Bool(true) ? "true" : "false"); }});
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
    // The notice shows again only for a new key or text: the getters read
    // the engine state they are part of, which changes far more often.
    auto last = std::make_shared<std::optional<std::pair<Value, winrt::hstring>>>();
    Reactive::Run([ctx, content, key, shown, place, hide, generation, duration, motion, icon, last] {
        auto now = key ? key(ctx) : Value();
        auto text = content ? content(ctx) : winrt::hstring();
        if (*last && (*last)->first == now && (*last)->second == text) return;
        *last = std::pair{now, text};
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
