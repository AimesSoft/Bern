// Bern runtime for Windows, XAML part 1 of 4: contexts, parents, the
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
};

/// Everything the runtime needs to know about a generated application.
struct AppDef {
    Catalog catalog;
    std::vector<WindowDef> windows;
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
struct Ctx {
    Win* win = nullptr;
    std::string comp;
    std::shared_ptr<std::function<std::string()> const> key;
    std::shared_ptr<LocalVar const> locals;
    std::shared_ptr<Page> page;
    bool toolbar = false;
    std::optional<Field> nav;
    std::optional<Cell<bool>> shown;

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
        child.key = nullptr;
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
        child.key = std::make_shared<std::function<std::string()> const>(std::move(rowKey));
        return child;
    }
    Ctx WithPage(std::shared_ptr<Page> header) const { auto child = *this; child.page = std::move(header); return child; }
    Ctx Toolbar() const { auto child = *this; child.toolbar = true; return child; }
    Ctx WithNav(Field path) const { auto child = *this; child.nav = path; return child; }
    Ctx WithShown(Cell<bool> cell) const { auto child = *this; child.shown = cell; return child; }

    /// Whether this content is showing; content under a pushed destination
    /// is not.
    bool Shown() const { return shown ? shown->Get() : true; }

    /// The control target of `id`: `component.id`, with the row key inside
    /// lists.
    std::string Target(std::string_view id) const {
        auto target = comp + "." + std::string(id);
        if (key) target += "[" + (*key)() + "]";
        return target;
    }

    Value Read(Field field) const;
    Value Peek(Field field) const;
    bool Write(Field field, Value const& value) const;
};

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
    bern::Version version;

    Page() {
        tools.Orientation(controls::Orientation::Horizontal);
        tools.Spacing(4);
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

    /// A holder for toolbar items, removed when the current scope ends.
    controls::StackPanel AddTools() {
        controls::StackPanel holder;
        holder.Orientation(controls::Orientation::Horizontal);
        holder.Spacing(4);
        tools.Children().Append(holder);
        version.Bump();
        Reactive::OnCleanup([self = shared_from_this(), holder] {
            uint32_t index;
            if (self->tools.Children().IndexOf(holder, index)) self->tools.Children().RemoveAt(index);
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

/// Whether `element` takes free height in a column.
inline bool FillsColumn(xaml::UIElement const& element) {
    auto role = RoleOf(element);
    if (role == L"bern-fill" || role == L"bern-spacer") return true;
    if (role == L"bern-column")
        for (auto child : element.as<controls::Panel>().Children())
            if (child.Visibility() == xaml::Visibility::Visible && FillsColumn(child)) return true;
    return false;
}

struct FlexPanel : controls::PanelT<FlexPanel> {
    bool horizontal = false;
    double spacing = 8;
    xaml::Thickness padding{0, 0, 0, 0};

    bool Fills(xaml::UIElement const& child) const {
        if (horizontal) return RoleOf(child) == L"bern-spacer";
        return FillsColumn(child);
    }

    float Main(Size s) const { return horizontal ? s.Width : s.Height; }
    float Cross(Size s) const { return horizontal ? s.Height : s.Width; }
    Size Make(float main, float cross) const { return horizontal ? Size{main, cross} : Size{cross, main}; }

    Size MeasureOverride(Size available) {
        Size inner{(std::max)(0.0f, available.Width - static_cast<float>(padding.Left + padding.Right)),
            (std::max)(0.0f, available.Height - static_cast<float>(padding.Top + padding.Bottom))};
        float space = Main(inner), cross = Cross(inner), used = 0, thickness = 0, filled = 0;
        int visible = 0, filling = 0;
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) continue;
            ++visible;
            if (Fills(child)) { ++filling; continue; }
            child.Measure(Make(INFINITY, cross));
            used += Main(child.DesiredSize());
            thickness = (std::max)(thickness, Cross(child.DesiredSize()));
        }
        float gaps = static_cast<float>(spacing) * (std::max)(0, visible - 1);
        float free = std::isinf(space) ? INFINITY : (std::max)(0.0f, space - used - gaps);
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible || !Fills(child)) continue;
            child.Measure(Make(std::isinf(free) ? INFINITY : free / filling, cross));
            filled += Main(child.DesiredSize());
            thickness = (std::max)(thickness, Cross(child.DesiredSize()));
        }
        float main = filling > 0 && !std::isinf(space) ? space : used + gaps + filled;
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
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) {
                child.Arrange(Rect{0, 0, 0, 0});
                continue;
            }
            float length = Fills(child) ? share : Main(child.DesiredSize());
            auto left = static_cast<float>(padding.Left), top = static_cast<float>(padding.Top);
            child.Arrange(horizontal ? Rect{left + position, top, length, cross} : Rect{left, top + position, cross, length});
            position += length + static_cast<float>(spacing);
        }
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

/// Where builders put elements: a panel and a region inside it.
/// Conditional and repeated content gets a region of its own: clearing or
/// reordering a region touches only its elements, and new elements land at
/// the region's place among its siblings.
struct Region {
    std::vector<std::variant<xaml::UIElement, std::shared_ptr<Region>>> items;
    /// The enclosing region, which outlives this one: the panel keeps the
    /// root, and each region keeps the regions in it.
    Region* up = nullptr;

    void Elements(std::vector<xaml::UIElement>& output) const {
        for (auto const& item : items) {
            if (auto element = std::get_if<xaml::UIElement>(&item)) output.push_back(*element);
            else std::get<std::shared_ptr<Region>>(item)->Elements(output);
        }
    }
    uint32_t Count() const {
        uint32_t count = 0;
        for (auto const& item : items) count += std::holds_alternative<xaml::UIElement>(item) ? 1 : std::get<std::shared_ptr<Region>>(item)->Count();
        return count;
    }
    uint32_t Offset() const {
        if (!up) return 0;
        auto before = up->Offset();
        for (auto const& item : up->items) {
            if (auto region = std::get_if<std::shared_ptr<Region>>(&item)) {
                if (region->get() == this) break;
                before += (*region)->Count();
            } else {
                before += 1;
            }
        }
        return before;
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

struct Parent {
    controls::Panel panel{nullptr};
    std::shared_ptr<Region> region;
    /// Whether children are rows of a form section (cards).
    bool isList = false;
    std::optional<xaml::HorizontalAlignment> crossH;
    std::optional<xaml::VerticalAlignment> crossV;

    static Parent Of(controls::Panel const& panel) { return Parent{panel, RootRegion(panel)}; }
    static Parent OfList(controls::Panel const& panel) { return Parent{panel, RootRegion(panel), true}; }
    static Parent Aligned(controls::Panel const& panel, std::optional<xaml::HorizontalAlignment> h, std::optional<xaml::VerticalAlignment> v) {
        return Parent{panel, RootRegion(panel), false, h, v};
    }

    bool IsHorizontal() const {
        if (auto stack = panel.try_as<controls::StackPanel>()) return stack.Orientation() == controls::Orientation::Horizontal;
        return RoleOf(panel) == L"bern-row";
    }

    /// Add an element at the end of this region.
    void Add(xaml::UIElement const& element) const {
        if (auto framework = element.try_as<xaml::FrameworkElement>()) {
            if (crossH) framework.HorizontalAlignment(*crossH);
            if (crossV) framework.VerticalAlignment(*crossV);
        }
        auto index = region->Offset() + region->Count();
        region->items.emplace_back(element);
        auto children = panel.Children();
        children.InsertAt((std::min)(index, children.Size()), element);
    }

    /// A new empty region at the end of this one.
    Parent Sub() const {
        auto child = std::make_shared<Region>();
        child->up = region.get();
        region->items.emplace_back(child);
        return Parent{panel, child, isList, crossH, crossV};
    }

    /// Remove every element of this region.
    void Clear() const {
        std::vector<xaml::UIElement> elements;
        region->Elements(elements);
        auto children = panel.Children();
        for (auto const& element : elements) {
            uint32_t index;
            if (children.IndexOf(element, index)) children.RemoveAt(index);
        }
        region->items.clear();
    }

    /// Put child regions in a new order, keeping their elements.
    void Reorder(std::vector<Parent> const& order) const {
        std::vector<xaml::UIElement> elements;
        region->Elements(elements);
        auto children = panel.Children();
        for (auto const& element : elements) {
            uint32_t index;
            if (children.IndexOf(element, index)) children.RemoveAt(index);
        }
        region->items.clear();
        for (auto const& child : order) region->items.emplace_back(child.region);
        auto start = region->Offset();
        elements.clear();
        region->Elements(elements);
        for (uint32_t i = 0; i < elements.size(); ++i) children.InsertAt((std::min)(start + i, children.Size()), elements[i]);
    }

    /// Drop a child region and its elements.
    void Remove(Parent const& child) const {
        child.Clear();
        std::erase_if(region->items, [&](auto const& item) {
            auto found = std::get_if<std::shared_ptr<Region>>(&item);
            return found && found->get() == child.region.get();
        });
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

inline void Restyle(xaml::FrameworkElement const& element, std::wstring const& xaml) {
    try {
        element.Resources().MergedDictionaries().Append(xaml::Markup::XamlReader::Load(xaml).as<xaml::ResourceDictionary>());
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
/// Assets glyphs, the system icon fonts of Windows 11 and Windows 10.
namespace Icons {
inline wchar_t const* Glyph(std::string_view name) {
    static std::unordered_map<std::string_view, wchar_t const*> const glyphs = {
        {"plus", L""}, {"minus", L""}, {"xmark", L""}, {"checkmark", L""},
        {"chevron.left", L""}, {"chevron.right", L""}, {"chevron.down", L""},
        {"chevron.up.chevron.down", L""}, {"ellipsis", L""}, {"ellipsis.circle", L""},
        {"square.on.square", L""}, {"magnifyingglass", L""}, {"xmark.circle.fill", L""},
        {"doc", L""}, {"photo", L""}, {"film", L""}, {"music.note", L""},
        {"arrow.down.circle", L""}, {"gearshape", L""}, {"gearshape.2", L""},
        {"paintbrush", L""}, {"hand.tap", L""}, {"list.bullet.rectangle", L""},
        {"play.rectangle", L""}, {"play.fill", L""}, {"pause.fill", L""},
        {"speaker.fill", L""}, {"speaker.wave.3.fill", L""}, {"speaker.slash.fill", L""},
        {"sun.min", L""}, {"sun.max.fill", L""}, {"textformat", L""},
        {"textformat.size.smaller", L""}, {"textformat.size.larger", L""},
        {"character.cursor.ibeam", L""}, {"switch.2", L""}, {"star", L""},
        {"star.fill", L""}, {"square.and.arrow.up", L""}, {"trash", L""},
        {"exclamationmark.triangle", L""}, {"tray", L""}, {"heart", L""},
        {"arrow.left", L""},
    };
    auto found = glyphs.find(name);
    return found == glyphs.end() ? L"" : found->second;
}

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
#else
    xaml::Hosting::DesktopWindowXamlSource source{nullptr};
    HWND island = nullptr;
#endif

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
