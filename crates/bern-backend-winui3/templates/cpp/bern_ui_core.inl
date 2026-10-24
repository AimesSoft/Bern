// Bern runtime for Windows, XAML part 1 of 7: contexts, parents, the
// control registry, the flexible panel, styles and icons. Included once
// per front end inside a namespace that defines `xaml` (Microsoft::UI::Xaml
// for WinUI 3 or Windows::UI::Xaml for XAML Islands) and `kWinUI`.
// Generated build input; do not edit.

using winrt::Windows::Foundation::IInspectable;
using winrt::Windows::Foundation::Rect;
using winrt::Windows::Foundation::Size;
namespace controls = xaml::Controls;
namespace media = xaml::Media;

struct Ctx;
struct Parent;
struct Page;
struct Win;

using Get = std::function<Value(Ctx const&)>;
using Str = std::function<winrt::hstring(Ctx const&)>;
using Act = std::function<void(Ctx const&)>;
using Build = std::function<void(Parent const&, Ctx const&)>;
using Branch = std::function<std::optional<int>(Ctx const&)>;
using TextFn = std::function<winrt::hstring()>;

/// A writable session field bound to a control.
struct Field {
    int session = 0;
    int field = 0;
    bool operator==(Field const&) const = default;
};

/// What a window's close check found.
struct CloseCheck {
    bool needsConfirmation = false;
    winrt::hstring message;
    /// Runs the declared save command; closing waits for its result.
    std::function<void(Ctx const&, Done)> save;
};

/// A declared window kind.
struct WindowDef {
    std::string id;
    /// Text key of the title.
    std::string title;
    bool settings = false;
    std::optional<double> width, height, minWidth, minHeight;
    bool initialOpen = false;
    /// Global indices of the session types each instance owns.
    std::vector<int> sessions;
    Build render;
    std::function<CloseCheck(Ctx const&)> close;
    /// `role: dialog`: shown modally over the primary window.
    bool dialog = false;
    /// `chrome: unified`: content under the title bar.
    bool unified = false;
    std::optional<double> idleAfter;
    std::function<bool(Ctx const&)> immersive;
    /// `idle_enabled`: idle detection switched on and off by state.
    std::function<bool(Ctx const&)> idleEnabled;
    /// `WindowSet(accent:)`: an index into the accents of `style!`.
    std::function<int(Ctx const&)> accent;
    std::function<std::string(Ctx const&)> palette;
    /// `WindowSet(appearance:)`: "light", "dark" or "" for the system's.
    std::function<char const*(Ctx const&)> appearance;
};

/// Everything the runtime needs to know about a generated application.
struct AppDef {
    Catalog catalog;
    std::vector<WindowDef> windows;
    /// The accents of `style!`, by index: light and dark sRGB.
    std::vector<std::pair<uint32_t, uint32_t>> accents;
};

inline AppDef const* app = nullptr;

// ---------------------------------------------------------------------------
// Contexts

struct LocalVar {
    std::string name;
    std::function<Value()> get;
    std::shared_ptr<LocalVar const> next;
};

/// What a component body reads: its window, the window's drafts, the
/// component name for control targets, the row key inside lists, local
/// variables, the page whose header shows titles, back and tools, and the
/// enclosing navigation.
namespace B {
struct SidebarRegistry;
struct TabsRegistry;
struct StaggerClock;
struct ScrollInfo;
struct RailPanel;
struct SceneRoom;
}

/// The keys of the repeated rows a control is in, outermost first.
struct RowKeys {
    std::function<std::string()> key;
    std::shared_ptr<RowKeys const> outer;
};

/// Where elements on a `Rail` go: the rail's panel, along which axis, and
/// which point of each sits at its fraction.
struct RailInfo {
    bool vertical = false;
    std::string anchor = "center";
    B::RailPanel* panel = nullptr;
    controls::Panel element{nullptr};
};

struct Ctx {
    Win* win = nullptr;
    std::string comp;
    std::shared_ptr<RowKeys const> keys;
    std::shared_ptr<LocalVar const> locals;
    std::shared_ptr<Page> page;
    bool toolbar = false;
    std::optional<Field> nav;
    std::optional<Cell<bool>> shown;
    /// `appearance:` of the part this content is in, for its popups.
    std::string appearance;
    std::shared_ptr<B::SidebarRegistry> sidebar;
    std::shared_ptr<B::TabsRegistry> tabs;
    std::shared_ptr<RailInfo const> rail;
    /// The stagger of the container being built, for its direct children.
    std::shared_ptr<B::StaggerClock> clock;
    /// The scroll view this content is in.
    std::shared_ptr<B::ScrollInfo> scroll;
    /// The `ScaleBox` scene this content is a layer of, whose room a layer
    /// that bleeds reaches into; none inside a layer.
    std::shared_ptr<B::SceneRoom> scene;

    /// Engine state field, recording a dependency.
    Value Engine(int field) const { return Host::EngineValue(field); }
    /// A field of the window's session drafts, recording a dependency.
    Value Session(int session, int field) const;
    /// A local variable: a component parameter, row variable or pattern
    /// binding.
    Value Local(std::string_view name) const {
        for (auto local = locals.get(); local; local = local->next.get())
            if (local->name == name) return local->get();
        return Value::Unit();
    }

    /// The context of a referenced component: its own name and parameters,
    /// evaluated in this context.
    Ctx Child(std::string component, std::initializer_list<std::pair<char const*, Get>> parameters = {}) const {
        auto parent = *this;
        std::shared_ptr<LocalVar const> next;
        for (auto const& [name, get] : parameters)
            next = std::make_shared<LocalVar const>(LocalVar{name, [parent, get] { return get(parent); }, next});
        auto child = *this;
        child.comp = std::move(component);
        child.locals = next;
        return child;
    }

    Ctx WithLocal(std::string name, std::function<Value()> get) const {
        auto child = *this;
        child.locals = std::make_shared<LocalVar const>(LocalVar{std::move(name), std::move(get), locals});
        return child;
    }
    Ctx WithKey(std::function<std::string()> rowKey) const {
        auto child = *this;
        child.keys = std::make_shared<RowKeys const>(RowKeys{std::move(rowKey), keys});
        return child;
    }
    Ctx WithPage(std::shared_ptr<Page> header) const { auto child = *this; child.page = std::move(header); return child; }
    Ctx Toolbar() const { auto child = *this; child.toolbar = true; return child; }
    Ctx WithNav(Field path) const { auto child = *this; child.nav = path; return child; }
    Ctx WithShown(Cell<bool> cell) const { auto child = *this; child.shown = cell; return child; }

    /// Whether this content is showing; content under a pushed destination
    /// is not.
    bool Shown() const { return shown ? shown->Get() : true; }

    /// The control target of `id`: `component.id`, with the key of every
    /// repeated row it is in, outermost first, through components too.
    std::string Target(std::string_view id) const {
        std::vector<std::string> rows;
        for (auto row = keys.get(); row; row = row->outer.get()) rows.push_back(row->key());
        auto target = comp + "." + std::string(id);
        for (auto row = rows.rbegin(); row != rows.rend(); ++row) target += "[" + *row + "]";
        return target;
    }

    /// `window.idle`, `window.compact` and the window's other sizes.
    Value WindowFlag(std::string_view name) const;

    Value Read(Field field) const;
    Value Peek(Field field) const;
    bool Write(Field field, Value const& value) const;
};

// ---------------------------------------------------------------------------
// What `style!` declares, as the generated tables describe it

namespace B {

/// A spring of `style!`: undamped period in seconds and damping ratio.
struct Motion {
    double period = 0.45;
    double damping = 1.0;
};

/// A color: a semantic role such as `accent`, or a pair of sRGB colors for
/// the light and dark appearance. Neither is a gap of a pattern.
struct Paint {
    char const* role = nullptr;
    uint32_t light = 0, dark = 0;
    bool fixed = false;
    Paint() = default;
    Paint(char const* name) : role(name) {}
    Paint(std::nullptr_t, uint32_t lightColor, uint32_t darkColor) : light(lightColor), dark(darkColor), fixed(true) {}
    bool Given() const { return role || fixed; }
};

/// How an element appears or disappears.
struct Transition {
    /// `Grow` scales along one axis from nothing, `edge` holding still.
    enum Kind { Fade, Pop, Slide, Grow };
    Kind kind = Fade;
    double from = 0.8;
    char const* edge = "bottom";
    double distance = 16;
    Motion const* motion = nullptr;
};

/// A band of a pattern: `width` units of `color` at `opacity`, or a gap.
struct Band {
    Paint color;
    double width = 0;
    double opacity = 1;
};

struct PatternDef {
    std::vector<Band> bands;
    bool twill = false;
};

/// The colors and transforms of one state of a styled control.
struct Look {
    std::optional<Paint> background, foreground, border, fill, indicator;
    std::optional<double> borderWidth, indicatorWidth;
    char const* indicatorEdge = nullptr;
    std::optional<std::array<double, 3>> shadow;
    std::optional<double> scaleX, scaleY, offsetX, offsetY, opacity;
    Motion const* motion = nullptr;
    bool system = false, quiet = false;
};

/// A control kind's (and variant's) style: the base look, each state's,
/// the corner, padding and size, and the motion back to rest.
struct StyleDef {
    Look base;
    std::optional<Look> hover, pressed, selected, focus, disabled;
    std::optional<double> corner, padding;
    char const* size = nullptr;
    /// Whether the style draws the control's colors instead of the
    /// platform's.
    bool draws = false;
    Motion const* settle = nullptr;
};

/// A length: a number, or an expression it follows.
struct Len {
    std::optional<double> fixed;
    Get bound;
    Len() = default;
    explicit Len(double value) : fixed(value) {}
    explicit Len(Get getter) : bound(std::move(getter)) {}
    bool Given() const { return fixed.has_value() || static_cast<bool>(bound); }
    double Now(Ctx const& ctx) const;
};

/// Children of a reflowing panel slide from where they were to where
/// layout puts them now (in the motion part).
inline void Reflow(xaml::UIElement const& child, float dx, float dy, Motion const* motion);

}

// ---------------------------------------------------------------------------
// Pages

/// A page with a header: titles, back actions, title widgets and tools
/// registered by what it shows. The innermost registration wins.
struct Page : std::enable_shared_from_this<Page> {
    std::vector<std::pair<int64_t, TextFn>> titles;
    std::vector<std::pair<int64_t, std::function<void()>>> backs;
    std::vector<std::pair<int64_t, xaml::UIElement>> titleWidgets;
    std::vector<int64_t> hidden;
    int64_t next = 1;
    controls::StackPanel tools;
    /// Toolbar items at the leading end, after the way back.
    controls::StackPanel leadingTools;
    bern::Version version;
    /// The window, whose immersive state hides the header.
    Win* win = nullptr;

    Page() {
        tools.Orientation(controls::Orientation::Horizontal);
        tools.Spacing(4);
        leadingTools.Orientation(controls::Orientation::Horizontal);
        leadingTools.Spacing(4);
    }

    template <class T> void Register(std::vector<std::pair<int64_t, T>> Page::*member, T item) {
        auto id = next++;
        (this->*member).emplace_back(id, std::move(item));
        version.Bump();
        Reactive::OnCleanup([self = shared_from_this(), member, id] {
            std::erase_if(self.get()->*member, [id](auto const& entry) { return entry.first == id; });
            self->version.Bump();
        });
    }

    void PushTitle(TextFn title) { Register(&Page::titles, std::move(title)); }
    void PushBack(std::function<void()> back) { Register(&Page::backs, std::move(back)); }
    void PushTitleWidget(xaml::UIElement widget) { Register(&Page::titleWidgets, std::move(widget)); }

    /// Hide this page's header while the current scope lasts, for content
    /// that brings headers of its own.
    void HideHeader() {
        auto id = next++;
        hidden.push_back(id);
        version.Bump();
        Reactive::OnCleanup([self = shared_from_this(), id] {
            std::erase(self->hidden, id);
            self->version.Bump();
        });
    }

    /// A holder for toolbar items, at the trailing end or after the way
    /// back (`leading`), removed when the current scope ends.
    controls::StackPanel AddTools(bool leading = false) {
        controls::StackPanel holder;
        holder.Orientation(controls::Orientation::Horizontal);
        holder.Spacing(4);
        auto into = leading ? leadingTools : tools;
        into.Children().Append(holder);
        version.Bump();
        Reactive::OnCleanup([self = shared_from_this(), holder, into] {
            uint32_t index;
            if (into.Children().IndexOf(holder, index)) into.Children().RemoveAt(index);
            self->version.Bump();
        });
        return holder;
    }

    std::optional<winrt::hstring> Title() const {
        if (titles.empty()) return std::nullopt;
        return titles.back().second();
    }
    std::function<void()> Back() const { return backs.empty() ? nullptr : backs.back().second; }
    xaml::UIElement TitleWidget() const { return titleWidgets.empty() ? nullptr : titleWidgets.back().second; }
    bool HeaderHidden() const { return !hidden.empty(); }
};

// ---------------------------------------------------------------------------
// The flexible panel. Content stacks along one axis, as StackPanel does,
// but filling children share the free space along it, as boxes do on the
// other targets: spacers in rows, and scrolling forms, split views and
// navigation in columns. A column with a filling child fills in turn.
// Roles travel in `Tag`: "bern-fill", "bern-spacer", "bern-column",
// "bern-row".

inline winrt::hstring RoleOf(xaml::UIElement const& element) {
    if (auto framework = element.try_as<xaml::FrameworkElement>())
        if (auto tag = framework.Tag()) return winrt::unbox_value_or<winrt::hstring>(tag, L"");
    return L"";
}

inline void SetRole(xaml::FrameworkElement const& element, wchar_t const* role) { element.Tag(winrt::box_value(role)); }
inline void MarkFill(xaml::FrameworkElement const& element) { SetRole(element, L"bern-fill"); }
/// Marks a control that keeps its own width, such as a button: a column
/// that stretches its children leaves it at its leading edge.
inline void KeepWidth(xaml::FrameworkElement const& element) {
    if (RoleOf(element).empty()) SetRole(element, L"bern-own-width");
}

/// Marks a column that takes the whole width it is given, whatever it
/// holds: an adaptive layout, which judges the layout size by it.
inline void MarkFullWidth(controls::Panel const& column) {
    column.Resources().Insert(winrt::box_value(L"bern.wide"), winrt::box_value(true));
}

inline bool FullWidth(xaml::UIElement const& column) {
    auto panel = column.try_as<controls::Panel>();
    return panel && panel.Resources().HasKey(winrt::box_value(L"bern.wide"));
}

/// The element a wrapper (`bern-wrap`, made for motion, placement or
/// styles) holds, or the element itself.
inline xaml::UIElement Unwrapped(xaml::UIElement element) {
    while (element && RoleOf(element) == L"bern-wrap") {
        auto children = element.as<controls::Panel>().Children();
        if (children.Size() == 0) break;
        element = children.GetAt(children.Size() - 1);
    }
    return element;
}

/// The element of a control's template named `name`, once it is loaded.
inline xaml::FrameworkElement TemplatePart(xaml::DependencyObject const& node, wchar_t const* name) {
    auto count = media::VisualTreeHelper::GetChildrenCount(node);
    for (int32_t i = 0; i < count; ++i) {
        auto child = media::VisualTreeHelper::GetChild(node, i);
        if (auto element = child.try_as<xaml::FrameworkElement>(); element && element.Name() == name) return element;
        if (auto found = TemplatePart(child, name)) return found;
    }
    return nullptr;
}

/// A wrapper around `inner`: one cell that takes the inner element's size
/// and its role in rows and columns.
inline controls::Grid Wrap(xaml::UIElement const& inner) {
    controls::Grid wrapper;
    SetRole(wrapper, L"bern-wrap");
    if (auto framework = inner.try_as<xaml::FrameworkElement>()) {
        wrapper.HorizontalAlignment(framework.HorizontalAlignment());
        wrapper.VerticalAlignment(framework.VerticalAlignment());
    }
    wrapper.Children().Append(inner);
    return wrapper;
}

/// The role of an element that holds `inner` alone, such as a button with
/// content: it fills the way the inner element does.
inline void Mirror(xaml::FrameworkElement const& holder, xaml::UIElement const& inner);

/// Whether `element` is a backdrop among a stack's layers: a rail or a
/// canvas without a size, which take the size of the other layers and do
/// not make the stack fill.
inline bool Backdrop(xaml::UIElement const& element) {
    auto role = std::wstring_view(RoleOf(Unwrapped(element)));
    return role.starts_with(L"bern-rail") || role == L"bern-canvas";
}

/// Whether `element` was given its height (or width): then it keeps it,
/// whatever it holds.
inline bool Sized(xaml::UIElement const& element, bool height) {
    auto framework = Unwrapped(element).try_as<xaml::FrameworkElement>();
    return framework && !std::isnan(height ? framework.Height() : framework.Width());
}

/// Whether `element` takes free height in a column: what fills, and rows
/// and columns holding something that does (a spacer fills a row only
/// along it), unless it was given its height.
inline bool FillsColumn(xaml::UIElement const& element) {
    if (Sized(element, true)) return false;
    if (RoleOf(element) == L"bern-wrap") return FillsColumn(Unwrapped(element));
    auto role = RoleOf(element);
    if (role == L"bern-fill" || role == L"bern-canvas" || role == L"bern-spacer" || role == L"bern-fill-height" || role == L"bern-layers-h" || role == L"bern-layers-wh" ||
        role == L"bern-rail-h" || role == L"bern-rail-wh")
        return true;
    if (role == L"bern-column" || role == L"bern-row")
        for (auto child : element.as<controls::Panel>().Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) continue;
            if (role == L"bern-row" && RoleOf(Unwrapped(child)) == L"bern-spacer") continue;
            if (FillsColumn(child)) return true;
        }
    return false;
}

/// Whether `element` takes free width in a row: spacers, what fills both
/// ways or across, and rows and columns holding something that does,
/// unless it was given its width.
inline bool FillsRow(xaml::UIElement const& element) {
    if (Sized(element, false)) return false;
    auto inner = Unwrapped(element);
    auto role = RoleOf(inner);
    if (role == L"bern-spacer" || role == L"bern-fill" || role == L"bern-canvas" || role == L"bern-fill-width" || role == L"bern-layers-w" || role == L"bern-layers-wh" ||
        role == L"bern-rail-w" || role == L"bern-rail-wh")
        return true;
    if (role == L"bern-column" && FullWidth(inner)) return true;
    if (role == L"bern-column" || role == L"bern-row")
        for (auto child : inner.as<controls::Panel>().Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) continue;
            if (role == L"bern-column" && RoleOf(Unwrapped(child)) == L"bern-spacer") continue;
            if (FillsRow(child)) return true;
        }
    return false;
}

/// The role of an element that holds `inner` alone, such as a button with
/// content: it fills the way the inner element does.
inline void Mirror(xaml::FrameworkElement const& holder, xaml::UIElement const& inner) {
    bool across = FillsRow(inner), down = FillsColumn(inner);
    if (across && down) SetRole(holder, L"bern-fill");
    else if (across) SetRole(holder, L"bern-fill-width");
    else if (down) SetRole(holder, L"bern-fill-height");
}

/// Where a reflowing panel put its children last, to slide them from there.
struct Places {
    std::vector<std::pair<xaml::UIElement, winrt::Windows::Foundation::Point>> last;

    void Arranged(B::Motion const* motion, std::vector<std::pair<xaml::UIElement, winrt::Windows::Foundation::Point>> now) {
        if (motion) {
            for (auto const& [child, at] : now) {
                for (auto const& [before, was] : last) {
                    if (before != child) continue;
                    float dx = was.X - at.X, dy = was.Y - at.Y;
                    if (std::abs(dx) >= 0.5f || std::abs(dy) >= 0.5f) B::Reflow(child, dx, dy, motion);
                    break;
                }
            }
        }
        last = std::move(now);
    }
};

/// How a panel of Bern's lets its children reflow, kept with the panel.
struct ReflowHolder : winrt::implements<ReflowHolder, IInspectable> {
    std::function<void(B::Motion const*)> set;
};

inline void KeepReflow(controls::Panel const& panel, std::function<void(B::Motion const*)> set) {
    auto holder = winrt::make_self<ReflowHolder>();
    holder->set = std::move(set);
    panel.Resources().Insert(winrt::box_value(L"bern.reflow"), holder.as<IInspectable>());
}

/// Let the children of `panel`, a panel of Bern's, reflow with `motion`.
inline void SetReflow(controls::Panel const& panel, B::Motion const* motion) {
    auto key = winrt::box_value(L"bern.reflow");
    auto resources = panel.Resources();
    if (resources.HasKey(key)) winrt::get_self<ReflowHolder>(resources.Lookup(key))->set(motion);
}

/// Where the first line of text in `element` sits, from its top: the
/// first text block below it, through containers; nothing without text.
inline std::optional<float> BaselineOf(xaml::UIElement const& element) {
    if (auto text = element.try_as<controls::TextBlock>()) return static_cast<float>(text.BaselineOffset());
    int32_t count = media::VisualTreeHelper::GetChildrenCount(element);
    for (int32_t i = 0; i < count; ++i) {
        auto child = media::VisualTreeHelper::GetChild(element, i).try_as<xaml::UIElement>();
        if (!child || child.Visibility() != xaml::Visibility::Visible) continue;
        if (auto below = BaselineOf(child)) {
            try {
                auto at = child.TransformToVisual(element).TransformPoint(winrt::Windows::Foundation::Point{0, 0});
                return at.Y + *below;
            } catch (winrt::hresult_error const&) {
                return *below;
            }
        }
    }
    return std::nullopt;
}

/// Whether `element` gives up width when its row is too narrow: text and
/// stacks holding it, but not controls or what was given its width.
inline bool Yields(xaml::UIElement const& element) {
    if (Sized(element, false)) return false;
    auto inner = Unwrapped(element);
    if (inner.try_as<controls::TextBlock>()) return true;
    auto role = std::wstring(RoleOf(inner));
    return role == L"bern-column" || role == L"bern-row" || role.starts_with(L"bern-layers");
}

struct FlexPanel : controls::PanelT<FlexPanel> {
    bool horizontal = false;
    double spacing = 8;
    xaml::Thickness padding{0, 0, 0, 0};
    B::Motion const* reflow = nullptr;
    Places places;
    /// A row on the baseline: children move down so their first lines of
    /// text share one baseline; `extra` is the height that takes.
    bool baseline = false;
    float extra = 0;

    bool Fills(xaml::UIElement const& child) const {
        if (horizontal) return FillsRow(child);
        return FillsColumn(child);
    }

    float Main(Size s) const { return horizontal ? s.Width : s.Height; }
    float Cross(Size s) const { return horizontal ? s.Height : s.Width; }
    Size Make(float main, float cross) const { return horizontal ? Size{main, cross} : Size{cross, main}; }

    /// Where a child given a side across larger than the room goes: it is
    /// measured and placed with that side, overflowing as its alignment
    /// says (evenly when centered), as on the other targets, rather than
    /// being clipped to the room; the panel does not grow for it. Returns
    /// the offset and the side.
    std::pair<float, float> Across(xaml::UIElement const& child, float room) const {
        auto framework = Unwrapped(child).try_as<xaml::FrameworkElement>();
        double given = framework ? (horizontal ? framework.Height() : framework.Width()) : NAN;
        if (std::isnan(given) || given <= room) return {0.0f, room};
        float side = static_cast<float>(given), over = side - room;
        auto placed = child.try_as<xaml::FrameworkElement>();
        if (!placed) return {-over / 2, side};
        if (horizontal) {
            auto align = placed.VerticalAlignment();
            return {align == xaml::VerticalAlignment::Top ? 0.0f : align == xaml::VerticalAlignment::Bottom ? -over : -over / 2, side};
        }
        auto align = placed.HorizontalAlignment();
        return {align == xaml::HorizontalAlignment::Left ? 0.0f : align == xaml::HorizontalAlignment::Right ? -over : -over / 2, side};
    }

    Size MeasureOverride(Size available) {
        Size inner{(std::max)(0.0f, available.Width - static_cast<float>(padding.Left + padding.Right)),
            (std::max)(0.0f, available.Height - static_cast<float>(padding.Top + padding.Bottom))};
        float space = Main(inner), cross = Cross(inner), used = 0, thickness = 0, filled = 0;
        int visible = 0, filling = 0;
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) continue;
            ++visible;
            if (Fills(child)) { ++filling; continue; }
            child.Measure(Make(INFINITY, Across(child, cross).second));
            used += Main(child.DesiredSize());
            thickness = (std::max)(thickness, (std::min)(Cross(child.DesiredSize()), cross));
        }
        float gaps = static_cast<float>(spacing) * (std::max)(0, visible - 1);
        // Too wide for its room: text gives way, as a stack's text wraps or
        // truncates on the other targets, so controls keep their size.
        if (horizontal && !std::isinf(space) && used + gaps > space) {
            float yielding = 0;
            for (auto child : Children())
                if (child.Visibility() == xaml::Visibility::Visible && !Fills(child) && Yields(child)) yielding += Main(child.DesiredSize());
            float over = used + gaps - space;
            if (yielding > 0) {
                used = 0;
                for (auto child : Children()) {
                    if (child.Visibility() != xaml::Visibility::Visible || Fills(child)) continue;
                    if (Yields(child)) {
                        float own = Main(child.DesiredSize());
                        child.Measure(Make((std::max)(0.0f, own - over * own / yielding), Across(child, cross).second));
                    }
                    used += Main(child.DesiredSize());
                    thickness = (std::max)(thickness, (std::min)(Cross(child.DesiredSize()), cross));
                }
            }
        }
        float free = std::isinf(space) ? INFINITY : (std::max)(0.0f, space - used - gaps);
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible || !Fills(child)) continue;
            child.Measure(Make(std::isinf(free) ? INFINITY : free / filling, Across(child, cross).second));
            filled += Main(child.DesiredSize());
            thickness = (std::max)(thickness, (std::min)(Cross(child.DesiredSize()), cross));
        }
        float main = filling > 0 && !std::isinf(space) ? space : used + gaps + filled;
        // What is longer than the room runs past its end, as a SwiftUI
        // stack in a frame of that size does: asking for more would have
        // XAML clip the panel to its room, across as well.
        if (!std::isinf(space)) main = (std::min)(main, space);
        if (baseline && horizontal) thickness += extra;
        auto size = Make(main, thickness);
        return Size{size.Width + static_cast<float>(padding.Left + padding.Right), size.Height + static_cast<float>(padding.Top + padding.Bottom)};
    }

    Size ArrangeOverride(Size final) {
        Size inner{(std::max)(0.0f, final.Width - static_cast<float>(padding.Left + padding.Right)),
            (std::max)(0.0f, final.Height - static_cast<float>(padding.Top + padding.Bottom))};
        float space = Main(inner), cross = Cross(inner), used = 0;
        int visible = 0, filling = 0;
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) continue;
            ++visible;
            if (Fills(child)) ++filling;
            else used += Main(child.DesiredSize());
        }
        float gaps = static_cast<float>(spacing) * (std::max)(0, visible - 1);
        float share = filling > 0 ? (std::max)(0.0f, space - used - gaps) / filling : 0;
        float position = 0;
        std::vector<std::pair<xaml::UIElement, winrt::Windows::Foundation::Point>> now;
        std::vector<std::pair<xaml::UIElement, Rect>> placed;
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) {
                child.Arrange(Rect{0, 0, 0, 0});
                continue;
            }
            float length = Fills(child) ? share : Main(child.DesiredSize());
            auto left = static_cast<float>(padding.Left), top = static_cast<float>(padding.Top);
            auto [offset, side] = Across(child, cross);
            Rect rect = horizontal ? Rect{left + position, top + offset, length, side} : Rect{left + offset, top + position, side, length};
            child.Arrange(rect);
            placed.emplace_back(child, rect);
            if (reflow) now.emplace_back(child, winrt::Windows::Foundation::Point{rect.X, rect.Y});
            position += length + static_cast<float>(spacing);
        }
        if (baseline && horizontal) {
            // Once arranged, each child's first baseline is known: move the
            // others down to the lowest.
            std::vector<std::optional<float>> lines;
            float lowest = 0;
            for (auto const& [child, rect] : placed) {
                lines.push_back(BaselineOf(child));
                if (lines.back()) lowest = (std::max)(lowest, *lines.back());
            }
            float needed = 0;
            for (size_t i = 0; i < placed.size(); ++i) {
                auto [child, rect] = placed[i];
                float shift = lines[i] ? lowest - *lines[i] : 0;
                if (shift > 0.5f) {
                    rect.Y += shift;
                    child.Arrange(rect);
                }
                needed = (std::max)(needed, shift + (std::min)(child.DesiredSize().Height, cross));
            }
            float more = (std::max)(0.0f, needed - (cross - extra));
            if (std::abs(more - extra) > 0.5f) {
                extra = more;
                InvalidateMeasure();
            }
        }
        if (reflow) places.Arranged(reflow, std::move(now));
        return final;
    }
};

/// A new flexible panel.
inline controls::Panel MakeFlex(bool horizontal = false, double spacing = 8, xaml::Thickness padding = {0, 0, 0, 0}) {
    auto impl = winrt::make_self<FlexPanel>();
    impl->horizontal = horizontal;
    impl->spacing = spacing;
    impl->padding = padding;
    controls::Panel panel = impl.as<controls::Panel>();
    SetRole(panel, horizontal ? L"bern-row" : L"bern-column");
    FlexPanel* flex = impl.get();
    KeepReflow(panel, [flex](B::Motion const* motion) { flex->reflow = motion; });
    return panel;
}

/// A flexible spacer inside rows and columns.
inline xaml::UIElement MakeSpacer() {
    controls::Border spacer;
    SetRole(spacer, L"bern-spacer");
    return spacer;
}

// ---------------------------------------------------------------------------
// Parents

/// A collection elements go into: a panel's children, or another list of
/// elements, such as the items of a navigation view.
struct Children {
    std::function<uint32_t()> size;
    std::function<void(uint32_t, xaml::UIElement const&)> insert;
    std::function<std::optional<uint32_t>(xaml::UIElement const&)> find;
    std::function<void(uint32_t)> remove;

    static std::shared_ptr<Children> Of(controls::Panel const& panel) {
        auto made = std::make_shared<Children>();
        auto weak = winrt::make_weak(panel);
        made->size = [weak] { auto p = weak.get(); return p ? p.Children().Size() : 0u; };
        made->insert = [weak](uint32_t at, xaml::UIElement const& element) {
            if (auto p = weak.get()) p.Children().InsertAt((std::min)(at, p.Children().Size()), element);
        };
        made->find = [weak](xaml::UIElement const& element) -> std::optional<uint32_t> {
            uint32_t index;
            if (auto p = weak.get(); p && p.Children().IndexOf(element, index)) return index;
            return std::nullopt;
        };
        made->remove = [weak](uint32_t at) {
            if (auto p = weak.get(); p && at < p.Children().Size()) p.Children().RemoveAt(at);
        };
        return made;
    }

    /// A vector of items, such as `NavigationView.MenuItems`.
    template <class T> static std::shared_ptr<Children> OfVector(winrt::Windows::Foundation::Collections::IVector<T> items) {
        auto made = std::make_shared<Children>();
        made->size = [items] { return items.Size(); };
        made->insert = [items](uint32_t at, xaml::UIElement const& element) { items.InsertAt((std::min)(at, items.Size()), element.as<T>()); };
        made->find = [items](xaml::UIElement const& element) -> std::optional<uint32_t> {
            uint32_t index;
            if (auto item = element.try_as<T>(); item && items.IndexOf(item, index)) return index;
            return std::nullopt;
        };
        made->remove = [items](uint32_t at) { if (at < items.Size()) items.RemoveAt(at); };
        return made;
    }
};

/// What leaving takes for an element with a `disappear` transition: it
/// stays in its place until `leave` calls `done`, then goes.
struct LeaveHook : winrt::implements<LeaveHook, IInspectable> {
    std::function<void(std::function<void()> done)> leave;
};

inline winrt::com_ptr<LeaveHook> LeaveHookOf(xaml::UIElement const& element) {
    auto framework = element.try_as<xaml::FrameworkElement>();
    if (!framework) return nullptr;
    auto key = winrt::box_value(L"bern.leave");
    auto resources = framework.Resources();
    if (!resources.HasKey(key)) return nullptr;
    return winrt::get_self<LeaveHook>(resources.Lookup(key))->get_strong();
}

/// Conditional and repeated content gets a region of its own: clearing or
/// reordering a region touches only its elements, and new elements land at
/// the region's place among its siblings. An element playing its
/// `disappear` transition stays in its region, counted, until it is gone.
struct Region {
    struct Entry {
        xaml::UIElement element{nullptr};
        std::shared_ptr<Region> region;
        bool leaving = false;
    };
    std::vector<Entry> items;
    /// The enclosing region, which outlives this one: the panel keeps the
    /// root, and each region keeps the regions in it.
    Region* up = nullptr;

    void Elements(std::vector<xaml::UIElement>& output, bool withLeaving = true) const {
        for (auto const& item : items) {
            if (item.region) item.region->Elements(output, withLeaving);
            else if (withLeaving || !item.leaving) output.push_back(item.element);
        }
    }
    uint32_t Count() const {
        uint32_t count = 0;
        for (auto const& item : items) count += item.region ? item.region->Count() : 1;
        return count;
    }
    uint32_t Offset() const {
        if (!up) return 0;
        auto before = up->Offset();
        for (auto const& item : up->items) {
            if (item.region) {
                if (item.region.get() == this) break;
                before += item.region->Count();
            } else {
                before += 1;
            }
        }
        return before;
    }
    /// Remove the entry of `element`, wherever it is below this region.
    bool Forget(xaml::UIElement const& element) {
        for (auto it = items.begin(); it != items.end(); ++it) {
            if (it->region) {
                if (it->region->Forget(element)) return true;
            } else if (it->element == element) {
                items.erase(it);
                return true;
            }
        }
        return false;
    }
};

/// Keeps a panel's root region alive as long as the panel: regions below
/// it find their place through it long after the builder that made it
/// returned.
struct RegionHolder : winrt::implements<RegionHolder, IInspectable> {
    std::shared_ptr<Region> region = std::make_shared<Region>();
};

/// The root region of `panel`, made on first use.
inline std::shared_ptr<Region> RootRegion(controls::Panel const& panel) {
    auto resources = panel.Resources();
    auto key = winrt::box_value(L"bern.region");
    if (resources.HasKey(key)) return winrt::get_self<RegionHolder>(resources.Lookup(key))->region;
    auto holder = winrt::make_self<RegionHolder>();
    resources.Insert(key, holder.as<IInspectable>());
    return holder->region;
}

/// Stretch `element` and the containers around it that hold something
/// filling, as placing them would have: rows and columns lay out what
/// fills at measure, but their own place in a stack or holder is set when
/// they are placed.
inline void Refill(xaml::FrameworkElement element) {
    for (int depth = 0; element && depth < 32; ++depth) {
        auto parent = element.Parent().try_as<xaml::FrameworkElement>();
        if (!parent) return;
        bool across = FillsRow(element), down = FillsColumn(element);
        auto role = std::wstring(RoleOf(parent));
        if (role.starts_with(L"bern-layers")) {
            if (across) element.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
            if (down) element.VerticalAlignment(xaml::VerticalAlignment::Stretch);
            bool rail = Backdrop(element);
            bool w = (across && !rail) || role.ends_with(L"-w") || role.ends_with(L"-wh");
            bool h = (down && !rail) || role.ends_with(L"-h") || role.ends_with(L"-wh");
            if (w || h) {
                SetRole(parent, w && h ? L"bern-layers-wh" : w ? L"bern-layers-w" : L"bern-layers-h");
                if (w) parent.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
                if (h) parent.VerticalAlignment(xaml::VerticalAlignment::Stretch);
            }
        } else if (role == L"bern-column") {
            if (across) element.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
        } else if (role == L"bern-row") {
            if (down) element.VerticalAlignment(xaml::VerticalAlignment::Stretch);
        } else if (parent.try_as<controls::ContentControl>() || parent.try_as<controls::Border>()) {
            // A holder fills the way what it holds does.
            if (across) element.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
            if (down) element.VerticalAlignment(xaml::VerticalAlignment::Stretch);
            if (!RoleOf(parent).empty() || across || down) Mirror(parent, element);
            if (across) parent.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
            if (down) parent.VerticalAlignment(xaml::VerticalAlignment::Stretch);
        } else {
            return;
        }
        element = parent;
    }
}

struct Parent {
    controls::Panel panel{nullptr};
    std::shared_ptr<Region> region;
    /// Whether children are rows of a form section (cards).
    bool isList = false;
    std::optional<xaml::HorizontalAlignment> crossH;
    std::optional<xaml::VerticalAlignment> crossV;
    /// Where the elements go: the panel's children unless another list.
    std::shared_ptr<Children> list;
    /// The root region of a list that is not a panel's, kept by its
    /// owner.
    std::shared_ptr<Region> root;
    /// Items of a `Flow`: a chip picker's chips are items of it too.
    bool flow = false;
    /// The window's content, directly or through a condition or a
    /// component: a stack there fills the window.
    bool window = false;

    static Parent Of(controls::Panel const& panel) { return Parent{panel, RootRegion(panel)}; }
    static Parent Window(controls::Panel const& panel) {
        Parent parent = Of(panel);
        parent.window = true;
        return parent;
    }
    static Parent OfList(controls::Panel const& panel) { return Parent{panel, RootRegion(panel), true}; }
    static Parent Aligned(controls::Panel const& panel, std::optional<xaml::HorizontalAlignment> h, std::optional<xaml::VerticalAlignment> v) {
        return Parent{panel, RootRegion(panel), false, h, v};
    }
    /// A parent whose elements go into `items`, such as menu items.
    static Parent Into(std::shared_ptr<Children> items) {
        Parent parent;
        parent.region = std::make_shared<Region>();
        parent.root = parent.region;
        parent.list = std::move(items);
        return parent;
    }

    std::shared_ptr<Children> Items() const { return list ? list : Children::Of(panel); }
    /// The region all this list's regions are in.
    std::shared_ptr<Region> Top() const { return root ? root : RootRegion(panel); }

    bool IsHorizontal() const {
        if (!panel) return false;
        if (auto stack = panel.try_as<controls::StackPanel>()) return stack.Orientation() == controls::Orientation::Horizontal;
        return RoleOf(panel) == L"bern-row";
    }

    /// Add an element at the end of this region.
    void Add(xaml::UIElement const& element) const {
        // What fills stretches across its container; the rest takes the
        // container's alignment.
        bool across = FillsRow(element), down = FillsColumn(element);
        // A stretching container leaves an element that keeps its own size
        // (pictures, stacks) where it is, and a control that keeps its own
        // width (a button) at its leading edge, as on the other targets:
        // the control's style gives it that alignment only once it is on
        // screen. So does a window for its content.
        if (auto framework = element.try_as<xaml::FrameworkElement>()) {
            bool stretching = crossH ? *crossH == xaml::HorizontalAlignment::Stretch : window;
            bool keepsH = crossH == xaml::HorizontalAlignment::Stretch && framework.HorizontalAlignment() != xaml::HorizontalAlignment::Stretch;
            bool keepsV = crossV == xaml::VerticalAlignment::Stretch && framework.VerticalAlignment() != xaml::VerticalAlignment::Stretch;
            if (stretching && !across && RoleOf(Unwrapped(element)) == L"bern-own-width") framework.HorizontalAlignment(xaml::HorizontalAlignment::Left);
            else if (crossH && (across || !keepsH)) framework.HorizontalAlignment(across ? xaml::HorizontalAlignment::Stretch : *crossH);
            if (crossV && (down || !keepsV)) framework.VerticalAlignment(down ? xaml::VerticalAlignment::Stretch : *crossV);
        }
        // A horizontal rail holding something that fills its height fills
        // the height it is given, and a vertical one holding something that
        // fills its width the width.
        if (panel && down && RoleOf(panel) == L"bern-rail-w") {
            SetRole(panel, L"bern-rail-wh");
            panel.VerticalAlignment(xaml::VerticalAlignment::Stretch);
        }
        if (panel && across && RoleOf(panel) == L"bern-rail-h") {
            SetRole(panel, L"bern-rail-wh");
            panel.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
        }
        // Layers holding something that fills fill the same way in turn; a
        // rail takes the size of its layers without making them fill.
        bool rail = Backdrop(element);
        bool downward = down && !rail;
        bool acrossward = across && !rail;
        if (panel && (acrossward || downward)) {
            auto role = std::wstring(RoleOf(panel));
            if (role.starts_with(L"bern-layers")) {
                bool w = acrossward || role.ends_with(L"-w") || role.ends_with(L"-wh");
                bool h = downward || role.ends_with(L"-h") || role.ends_with(L"-wh");
                SetRole(panel, w && h ? L"bern-layers-wh" : w ? L"bern-layers-w" : L"bern-layers-h");
                if (w) panel.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
                if (h) panel.VerticalAlignment(xaml::VerticalAlignment::Stretch);
            }
        }
        auto index = region->Offset() + region->Count();
        region->items.push_back(Region::Entry{element});
        Items()->insert(index, element);
        // Content that arrives after its containers were placed, such as a
        // branch of a condition, makes them fill in turn.
        if (panel && (across || down)) Refill(panel);
    }

    /// A new empty region at the end of this one.
    Parent Sub() const {
        auto child = std::make_shared<Region>();
        child->up = region.get();
        region->items.push_back(Region::Entry{nullptr, child});
        Parent sub = *this;
        sub.region = child;
        return sub;
    }

    /// Take `element` off its list now.
    void Detach(xaml::UIElement const& element) const {
        auto items = Items();
        if (auto index = items->find(element)) items->remove(*index);
    }

    /// Let the elements of `entries` go: at once, or after their
    /// `disappear` transition, staying counted in `kept` until then. The
    /// transitions start in `leaves`, once the region holds `kept`.
    void Let(std::vector<Region::Entry>& entries, std::vector<Region::Entry>& kept, std::vector<std::function<void()>>& leaves) const {
        for (auto& entry : entries) {
            if (entry.region) {
                Let(entry.region->items, kept, leaves);
                continue;
            }
            if (entry.leaving) {
                kept.push_back(entry);
                continue;
            }
            auto hook = LeaveHookOf(entry.element);
            if (!hook || !hook->leave) {
                Detach(entry.element);
                continue;
            }
            entry.leaving = true;
            kept.push_back(entry);
            auto element = entry.element;
            auto items = Items();
            std::weak_ptr<Region> weak = Top();
            // Leaving once: the hook goes with the element.
            auto leave = std::move(hook->leave);
            hook->leave = nullptr;
            leaves.push_back([leave, element, items, weak] {
                leave([element, items, weak] {
                    if (auto index = items->find(element)) items->remove(*index);
                    if (auto region = weak.lock()) region->Forget(element);
                });
            });
        }
    }

    /// Remove every element of this region; elements with a `disappear`
    /// transition stay until it has played.
    void Clear() const {
        std::vector<Region::Entry> kept;
        std::vector<std::function<void()>> leaves;
        Let(region->items, kept, leaves);
        region->items = std::move(kept);
        for (auto const& leave : leaves) leave();
    }

    /// Put child regions in a new order, keeping their elements; leaving
    /// elements stay after them until they are gone.
    void Reorder(std::vector<Parent> const& order) const {
        std::vector<xaml::UIElement> elements;
        region->Elements(elements);
        auto items = Items();
        for (auto const& element : elements)
            if (auto index = items->find(element)) items->remove(*index);
        std::vector<Region::Entry> leaving;
        for (auto const& item : region->items)
            if (!item.region && item.leaving) leaving.push_back(item);
        region->items.clear();
        for (auto const& child : order) region->items.push_back(Region::Entry{nullptr, child.region});
        for (auto const& item : leaving) region->items.push_back(item);
        auto start = region->Offset();
        elements.clear();
        region->Elements(elements);
        for (uint32_t i = 0; i < elements.size(); ++i) items->insert(start + i, elements[i]);
    }

    /// Drop a child region and its elements; its leaving elements stay in
    /// this region, where the child was, until they are gone.
    void Remove(Parent const& child) const {
        child.Clear();
        auto& entries = region->items;
        for (auto it = entries.begin(); it != entries.end(); ++it) {
            if (it->region.get() != child.region.get()) continue;
            auto leaving = std::move(child.region->items);
            for (auto& entry : leaving) entry.region = nullptr;
            it = entries.erase(it);
            entries.insert(it, leaving.begin(), leaving.end());
            break;
        }
    }
};

// ---------------------------------------------------------------------------
// Controls

/// What a control can do, each the same closure its element runs.
struct Actions {
    std::function<void()> activate;
    std::function<void(std::string const&)> edit;
    std::function<void()> submit;
    std::function<void(bool)> setBool;
    std::function<void(double)> setNumber;
    /// Select an option by its name; false when there is no such option.
    std::function<bool(std::string const&)> select;
    /// Go back; false when there is nothing to go back to.
    std::function<bool()> back;
    std::function<std::optional<std::string>()> value;
    /// Take the keyboard focus, as `ui.focus` does.
    std::function<void()> focus;
    /// Take a file, as if the user chose or dropped it (`ui.set_file`), of
    /// the kinds `files` names.
    std::function<void(std::string const&)> setFile;
    char const* files = nullptr;
};

struct ControlEntry {
    std::string target;
    Win* window = nullptr;
    std::string role;
    TextFn label;
    std::function<bool()> enabled;
    Actions actions;
};

/// Mounted controls by target. External entries operate them through the
/// same closures the user's input runs.
namespace Controls {
inline std::map<int64_t, ControlEntry> entries;
inline int64_t next = 1;

inline void Register(Ctx const& ctx, std::string_view id, std::string role, TextFn label, std::function<bool()> enabled, Actions actions) {
    auto token = next++;
    entries[token] = ControlEntry{ctx.Target(id), ctx.win, std::move(role), std::move(label), std::move(enabled), std::move(actions)};
    Reactive::OnCleanup([token] { entries.erase(token); });
}
}

// ---------------------------------------------------------------------------
// Styles and theme resources. WinUI 3 has the Windows 11 resources; the
// system XAML has the Windows 10 ones.

namespace Keys {
inline wchar_t const* SecondaryText = kWinUI ? L"TextFillColorSecondaryBrush" : L"SystemControlForegroundBaseMediumBrush";
inline wchar_t const* Divider = kWinUI ? L"DividerStrokeColorDefaultBrush" : L"SystemControlForegroundBaseLowBrush";
inline wchar_t const* Critical = kWinUI ? L"SystemFillColorCriticalBrush" : L"SystemControlErrorTextForegroundBrush";
inline wchar_t const* VideoBackground = kWinUI ? L"ControlFillColorTertiaryBrush" : L"SystemControlBackgroundBaseLowBrush";
inline wchar_t const* PageBackground = L"ApplicationPageBackgroundThemeBrush";
}

namespace Styles {

inline std::wstring Xaml() {
    std::wstring card = kWinUI
        ? LR"(<Setter Property="Background" Value="{ThemeResource CardBackgroundFillColorDefaultBrush}" />
    <Setter Property="BorderBrush" Value="{ThemeResource CardStrokeColorDefaultBrush}" />
    <Setter Property="BorderThickness" Value="1" />
    <Setter Property="CornerRadius" Value="{ThemeResource ControlCornerRadius}" />)"
        : LR"(<Setter Property="Background" Value="{ThemeResource SystemControlBackgroundListLowBrush}" />)";
    return LR"(<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Style x:Key="BernCard" TargetType="Border">
    )" + card + LR"(
    <Setter Property="Padding" Value="16,12" />
    <Setter Property="MinHeight" Value="56" />
  </Style>
</ResourceDictionary>)";
}

/// Lightweight styling: control resources `key → theme resource`, in each
/// theme.
inline std::wstring LightweightXaml(std::initializer_list<std::pair<wchar_t const*, wchar_t const*>> resources) {
    std::wstring entries;
    for (auto const& [key, value] : resources)
        entries += std::wstring(L"<StaticResource x:Key=\"") + key + L"\" ResourceKey=\"" + value + L"\" />";
    std::wstring themes;
    for (auto theme : {L"Default", L"Light"}) themes += std::wstring(L"<ResourceDictionary x:Key=\"") + theme + L"\">" + entries + L"</ResourceDictionary>";
    return LR"(<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"><ResourceDictionary.ThemeDictionaries>)" +
        themes + L"</ResourceDictionary.ThemeDictionaries></ResourceDictionary>";
}

/// A borderless button: transparent at rest and when disabled, the subtle
/// fills on hover and press, in every theme.
inline std::wstring SubtleXaml() {
    auto rest = kWinUI ? L"SubtleFillColorTransparentBrush" : L"SystemControlTransparentBrush";
    auto over = kWinUI ? L"SubtleFillColorSecondaryBrush" : L"SystemControlBackgroundListLowBrush";
    auto pressed = kWinUI ? L"SubtleFillColorTertiaryBrush" : L"SystemControlBackgroundListMediumBrush";
    return LightweightXaml({{L"ButtonBackground", rest}, {L"ButtonBackgroundPointerOver", over}, {L"ButtonBackgroundPressed", pressed},
        {L"ButtonBackgroundDisabled", rest}, {L"ButtonBorderBrush", rest}, {L"ButtonBorderBrushPointerOver", rest},
        {L"ButtonBorderBrushPressed", rest}, {L"ButtonBorderBrushDisabled", rest}});
}

inline void Install(xaml::ResourceDictionary const& resources) {
    try {
        resources.MergedDictionaries().Append(xaml::Markup::XamlReader::Load(Xaml()).as<xaml::ResourceDictionary>());
    } catch (winrt::hresult_error const& error) {
        Report("styles: " + Utf8(error.message()));
    }
}

inline IInspectable Resource(wchar_t const* key) {
    auto resources = xaml::Application::Current().Resources();
    auto boxed = winrt::box_value(key);
    if (resources.HasKey(boxed)) return resources.Lookup(boxed);
    return nullptr;
}

inline xaml::Style Named(wchar_t const* key) {
    if (auto found = Resource(key)) return found.try_as<xaml::Style>();
    return nullptr;
}

inline media::Brush Brush(wchar_t const* key) {
    if (auto found = Resource(key)) return found.try_as<media::Brush>();
    return nullptr;
}

/// How many lightweight dictionaries were parsed and how long that took,
/// for `BERN_FRAME_LOG`.
inline int64_t parsed = 0;
inline double parsing = 0;

/// The dictionary of `xaml`, parsed once and kept for the process: parsing
/// markup is the slow part of building a page, and every element with the
/// same look takes the same dictionary.
inline xaml::ResourceDictionary Dictionary(std::wstring const& xaml, bool fresh = false) {
    static auto* cache = new std::unordered_map<std::wstring, xaml::ResourceDictionary>();
    if (!fresh)
        if (auto found = cache->find(xaml); found != cache->end()) return found->second;
    auto began = std::chrono::steady_clock::now();
    auto dictionary = xaml::Markup::XamlReader::Load(xaml).as<xaml::ResourceDictionary>();
    ++parsed;
    parsing += std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - began).count();
    if (!fresh) cache->emplace(xaml, dictionary);
    return dictionary;
}

inline void Restyle(xaml::FrameworkElement const& element, std::wstring const& xaml) {
    try {
        auto merged = element.Resources().MergedDictionaries();
        try {
            merged.Append(Dictionary(xaml));
        } catch (winrt::hresult_error const&) {
            // A dictionary that will not be shared: one of its own.
            merged.Append(Dictionary(xaml, true));
        }
    } catch (winrt::hresult_error const& error) {
        Report("styles: " + Utf8(error.message()));
    }
}

inline void MakeSubtle(controls::Button const& button) { Restyle(button, SubtleXaml()); }

/// A button whose text and icon use the critical color of the theme.
inline void MakeCritical(controls::Button const& button) {
    Restyle(button, LightweightXaml({{L"ButtonForeground", Keys::Critical}, {L"ButtonForegroundPointerOver", Keys::Critical},
        {L"ButtonForegroundPressed", Keys::Critical}}));
}

/// A style setting properties of `target` elements to theme resources: it
/// follows the element's theme, also when it changes while the
/// application runs. Styles are made once and kept for the process.
inline xaml::Style Themed(wchar_t const* target, std::initializer_list<std::pair<wchar_t const*, wchar_t const*>> setters) {
    static auto* cache = new std::map<std::wstring, xaml::Style>();
    std::wstring id = target;
    std::wstring body;
    for (auto const& [property, key] : setters) {
        id += std::wstring(L"|") + property + L"=" + key;
        body += std::wstring(L"<Setter Property=\"") + property + L"\" Value=\"{ThemeResource " + key + L"}\" />";
    }
    if (auto found = cache->find(id); found != cache->end()) return found->second;
    xaml::Style style{nullptr};
    try {
        style = xaml::Markup::XamlReader::Load(
            LR"(<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType=")" + std::wstring(target) + L"\">" + body + L"</Style>")
                    .as<xaml::Style>();
    } catch (winrt::hresult_error const& error) {
        Report("styles: " + Utf8(error.message()));
    }
    cache->emplace(id, style);
    return style;
}

/// A text style by Bern's name.
inline xaml::Style TextStyle(char const* style) {
    std::string_view name = style ? style : "";
    wchar_t const* key;
    if (name == "large_title") key = kWinUI ? L"TitleLargeTextBlockStyle" : L"HeaderTextBlockStyle";
    else if (name == "title") key = L"TitleTextBlockStyle";
    else if (name == "title2") key = L"SubtitleTextBlockStyle";
    else if (name == "title3" || name == "headline" || name == "subheadline") key = kWinUI ? L"BodyStrongTextBlockStyle" : L"BaseTextBlockStyle";
    else if (name == "footnote" || name == "caption" || name == "caption2") key = L"CaptionTextBlockStyle";
    else if (name == "strong") key = kWinUI ? L"BodyStrongTextBlockStyle" : L"BaseTextBlockStyle";
    else key = L"BodyTextBlockStyle";
    return Named(key);
}

}

/// Bern icon names (SF Symbols naming) as Segoe Fluent Icons / Segoe MDL2
/// Assets glyphs, the system icon fonts of Windows 11 and Windows 10. The
/// table comes with the catalog.
namespace Icons {
inline wchar_t const* Glyph(std::string_view name) { return bern::IconGlyph(name); }

inline controls::FontIcon Icon(std::string_view name, double size = 16) {
    controls::FontIcon icon;
    icon.Glyph(Glyph(name));
    icon.FontSize(size);
    return icon;
}
}

// ---------------------------------------------------------------------------
// Window instances (operations in part 4)

/// A close confirmation waiting for a decision.
struct PendingClose {
    std::string id;
    winrt::hstring message;
    bool canSave = false;
    uint64_t draftRevision = 0;
    bool operator==(PendingClose const&) const = default;
};

/// What the platform shows for a window, in the session catalog's terms.
struct Facts {
    uint8_t visibility = 0, size = 0;
    double x = 0, y = 0, width = 0, height = 0;
    bool focused = false;
    uint32_t actions = 0;
};

struct Win {
    int kind = 0;
    uint64_t generation = 0;
    std::string id;
    std::unordered_map<int, std::vector<Cell<Value>>> cells;
    HWND hwnd = nullptr;
    controls::Grid root{nullptr};
    /// The failure bar: an InfoBar in WinUI 3, a plain bar in the system XAML.
    xaml::FrameworkElement failures{nullptr};
    controls::TextBlock failureText{nullptr};
    Cell<std::optional<PendingClose>> pending;
    bool allowClose = false;
    std::unique_ptr<Scope> scope = std::make_unique<Scope>();
    bool command = false;
    std::vector<std::pair<std::function<bool(Facts const&)>, std::function<void(std::optional<Failure> const&)>>> waits;
    controls::ContentDialog dialog{nullptr};
    std::string dialogConfirmation;
    bool reportQueued = false;
    bool closed = false;
    bool focused = false;
    int failuresShown = 0;
    std::optional<std::array<double, 4>> lastFrame;
    /// Expires when the window ends, for work queued on its behalf.
    std::shared_ptr<bool> life = std::make_shared<bool>(true);
#if BERN_WINUI
    xaml::Window window{nullptr};
    controls::TitleBar titleBar{nullptr};
#else
    xaml::Hosting::DesktopWindowXamlSource source{nullptr};
    HWND island = nullptr;
#endif

    /// `window.idle`: no input for the window's `idle_after` seconds.
    Cell<bool> fullscreen{false};
    Cell<bool> idle{false};
    /// The `idle_area`s of the window, each with its seconds.
    std::vector<std::pair<winrt::weak_ref<xaml::UIElement>, double>> idleAreas;
    /// Where the mouse is, for `idle_area`s: not known after a touch or a
    /// pen, which do not rest over areas; outside the window; or at
    /// `mousePoint`, in the coordinates of the window's content.
    enum class Mouse { Unknown, Outside, At } mouse = Mouse::Unknown;
    winrt::Windows::Foundation::Point mousePoint{};
    /// Starts the count toward `window.idle` again, with `idle_after`.
    std::function<void()> idleRest;
    /// The window's layout size by its width: compact, medium, expanded
    /// or large.
    Cell<std::string> size{std::string("expanded")};
    /// The width of the window's content: measured, or before the window
    /// is laid out the width it opens at. Layouts chosen by width go by it
    /// until they measure themselves.
    double width = INFINITY;
    /// `window.short`: shorter than the compact breakpoint.
    Cell<bool> isShort{false};
    /// Whether the window is immersive now: no title bar or toolbar.
    Cell<bool> immersive{false};
    /// Above the content: copies of shared elements while they move.
    controls::Canvas overlay{nullptr};
    /// The window's content host, its page header and the content holder
    /// under the header, which `under_bars` layers reach over.
    controls::Grid host{nullptr};
    xaml::FrameworkElement header{nullptr};
    controls::Grid holder{nullptr};
    /// How many `under_bars` layers are shown: while any is, the content
    /// reaches the window's top edge under the title bar and the header.
    Cell<int> underBars{0};
    /// A dialog window's content dialog, and the window it shows over.
    controls::ContentDialog sheet{nullptr};
    Win* owner = nullptr;
    /// Keyboard shortcuts of mounted controls, newest last.
    std::vector<std::pair<int64_t, xaml::Input::KeyboardAccelerator>> shortcuts;

    WindowDef const& Def() const { return app->windows[kind]; }
    Cell<Value>& CellOf(Field field);
    bool Write(Field field, Value const& value);
    void Fail(Failure const& failure);
    bool IsBlocked() const { return pending.Peek().has_value(); }
    double Scale() const;
    Facts CurrentFacts();
    void Report();
    void ScheduleReport();
    void Perform(std::function<void()> change, std::function<bool(Facts const&)> satisfied, std::function<void(std::optional<Failure> const&)> complete);
};

inline Value Ctx::Session(int session, int field) const { return win->CellOf(Field{session, field}).Get(); }
inline Value Ctx::WindowFlag(std::string_view name) const {
    if (!win) return Value::Bool(false);
    if (name == "fullscreen") return Value::Bool(win->fullscreen.Get());
    if (name == "idle") return Value::Bool(win->idle.Get());
    if (name == "short") return Value::Bool(win->isShort.Get());
    // A Windows desktop app never runs on a television.
    if (name == "tv") return Value::Bool(false);
    return Value::Bool(win->size.Get() == name);
}
inline double B::Len::Now(Ctx const& ctx) const {
    if (fixed) return *fixed;
    return bound ? R::AsDouble(bound(ctx)) : 0.0;
}
inline Value Ctx::Read(Field field) const { return win->CellOf(field).Get(); }
inline Value Ctx::Peek(Field field) const { return win->CellOf(field).Peek(); }
inline bool Ctx::Write(Field field, Value const& value) const { return win->Write(field, value); }

/// A failed command shows in the context's window.
inline std::function<void(Failure const&)> ShowIn(Ctx const& ctx) {
    auto win = ctx.win;
    // The reply may come after the window ended.
    std::weak_ptr<bool> alive = win ? std::weak_ptr<bool>(win->life) : std::weak_ptr<bool>();
    return [win, alive](Failure const& failure) {
        if (!alive.expired()) win->Fail(failure);
    };
}

/// `Host::Dispatch` with the failure shown in the context's window.
inline void Dispatch(Ctx const& ctx, int command, Values const& args, Done done = nullptr) {
    Host::Dispatch(command, args, std::move(done), ShowIn(ctx));
}

namespace WindowOps {
void ToggleFullscreen(Ctx const& ctx, std::string const& kind);
void Open(Ctx const& ctx, std::string const& kind);
void Close(Ctx const& ctx, std::string const& kind);
}

namespace MediaOps {
inline void Send(Ctx const& ctx, MediaOp op, std::string const& player, Values const& args = {}) {
    Media::Send(op, player, args, ShowIn(ctx));
}
/// `players.<id>.open(source)`: `source` is a `MediaSource` value.
inline void Open(Ctx const& ctx, std::string const& player, Value const& source) { Send(ctx, MediaOp::Open, player, {source}); }
}

namespace Snapshot {
void Schedule();
}
