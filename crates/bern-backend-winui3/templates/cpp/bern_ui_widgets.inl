// Bern runtime for Windows, XAML part 4 of 7: standard controls and
// containers. Inside a form section each control is a card row, as Windows
// Settings lays them out; elsewhere it is the plain control.
// Generated build input; do not edit.

namespace B {

/// `shape: capsule` and `height` of a system control. Fluent 2 has no
/// capsule buttons, so a capsule keeps the control's own corners and
/// takes the height.
struct ControlShape {
    bool capsule = false;
    Len height;
};

/// A rounded Composition clip on `element`, its radius from `radius` for
/// the element's size each time that changes; returns the geometry, whose
/// radius may also be set later.
inline composition::CompositionRoundedRectangleGeometry RoundedClip(xaml::FrameworkElement const& element, std::function<float(float, float)> radius) {
    auto visual = xaml::Hosting::ElementCompositionPreview::GetElementVisual(element);
    auto compositor = visual.Compositor();
    auto geometry = compositor.CreateRoundedRectangleGeometry();
    visual.Clip(compositor.CreateGeometricClip(geometry));
    // The size it has already, until it changes.
    float w0 = static_cast<float>(element.ActualWidth()), h0 = static_cast<float>(element.ActualHeight());
    geometry.Size(float2{w0, h0});
    if (radius) {
        float r = (std::max)(0.0f, (std::min)({radius(w0, h0), w0 / 2, h0 / 2}));
        geometry.CornerRadius(float2{r, r});
    }
    element.SizeChanged([geometry, radius](auto&&, xaml::SizeChangedEventArgs const& args) {
        float w = args.NewSize().Width, h = args.NewSize().Height;
        geometry.Size(float2{w, h});
        if (radius) {
            float r = (std::max)(0.0f, (std::min)({radius(w, h), w / 2, h / 2}));
            geometry.CornerRadius(float2{r, r});
        }
    });
    return geometry;
}

/// A control of `shape`: its own height, with the corners of its style.
inline void ShapeControl(Ctx const& ctx, controls::Control const& control, std::optional<ControlShape> const& shape) {
    if (!shape) return;
    if (shape->height.Given()) {
        BindLen(ctx, shape->height, nullptr, [control](double h) {
            control.Height((std::max)(0.0, h));
            control.MinHeight(0);
        });
        // The style's padding is known once the control is in the tree:
        // keep its sides, drop the top and bottom.
        auto weak = winrt::make_weak(control);
        auto token = std::make_shared<winrt::event_token>();
        *token = control.Loaded([weak, token](auto&&, auto&&) {
            auto control = weak.get();
            if (!control) return;
            control.Loaded(*token);
            auto padding = control.Padding();
            control.Padding(xaml::Thickness{padding.Left, 0, padding.Right, 0});
        });
    }
}

/// A weight of `text { … }`, as XAML's.
inline winrt::Windows::UI::Text::FontWeight WeightNamed(std::string_view name) {
    uint16_t weight = name == "ultralight" ? 100 : name == "thin" ? 200 : name == "light" ? 300 : name == "medium" ? 500
        : name == "semibold" ? 600 : name == "bold" ? 700 : name == "heavy" ? 800 : name == "black" ? 900 : 400;
    return winrt::Windows::UI::Text::FontWeight{weight};
}

/// A text style by Bern's name, with the weight and size `style!` gives it.
inline void ApplyTextStyle(controls::TextBlock const& block, char const* style) {
    // `control`: the font of the control it stands in, which the text
    // inherits, as a button's title does.
    if (style && std::string_view(style) == "control") return;
    if (auto named = Styles::TextStyle(style)) block.Style(named);
    std::string name = style ? style : "body";
    for (auto const& [entry, weight, size] : app->catalog.textStyles) {
        if (entry != name) continue;
        block.FontWeight(WeightNamed(weight));
        if (size) block.FontSize(*size);
    }
}

inline TextFn Bound(Ctx const& ctx, Str const& text) {
    if (!text) return nullptr;
    return [ctx, text] { return text(ctx); };
}

inline std::function<bool()> Enabled(Ctx const& ctx, Get const& enabled) {
    if (!enabled) return nullptr;
    return [ctx, enabled] { return R::Truthy(enabled(ctx)); };
}

inline std::function<void()> Run(Ctx const& ctx, Act const& action) {
    if (!action) return nullptr;
    return [ctx, action] { action(ctx); };
}

inline void BindText(controls::TextBlock const& block, TextFn const& text) {
    if (!text) return;
    Reactive::Run([block, text] { block.Text(text()); });
}

inline void BindName(xaml::UIElement const& element, TextFn const& text) {
    if (!text) return;
    Reactive::Run([element, text] { xaml::Automation::AutomationProperties::SetName(element, text()); });
}

inline controls::TextBlock TextOf(TextFn const& text) {
    controls::TextBlock block;
    block.TextWrapping(xaml::TextWrapping::Wrap);
    BindText(block, text);
    return block;
}

/// Secondary text, in both themes.
inline void Secondary(controls::TextBlock const& block) { block.Opacity(0.65); }

inline xaml::GridLength Star() { return xaml::GridLength{1, xaml::GridUnitType::Star}; }
inline xaml::GridLength AutoLength() { return xaml::GridLength{0, xaml::GridUnitType::Auto}; }

inline controls::ColumnDefinition Column(xaml::GridLength width) {
    controls::ColumnDefinition column;
    column.Width(width);
    return column;
}

/// A card row of a form section holding `content`.
inline controls::Border Card(xaml::UIElement const& content) {
    if (auto framework = content.try_as<xaml::FrameworkElement>(); framework && framework.VerticalAlignment() == xaml::VerticalAlignment::Stretch)
        framework.VerticalAlignment(xaml::VerticalAlignment::Center);
    controls::Border card;
    card.Child(content);
    if (auto style = Styles::Named(L"BernCard")) card.Style(style);
    return card;
}

/// A card row: a label on the left and a control on the right.
inline controls::Border LabeledCard(TextFn const& label, xaml::FrameworkElement const& control) {
    controls::Grid grid;
    grid.ColumnSpacing(12);
    grid.ColumnDefinitions().Append(Column(Star()));
    grid.ColumnDefinitions().Append(Column(AutoLength()));
    auto caption = TextOf(label);
    caption.VerticalAlignment(xaml::VerticalAlignment::Center);
    grid.Children().Append(caption);
    control.VerticalAlignment(xaml::VerticalAlignment::Center);
    controls::Grid::SetColumn(control, 1);
    grid.Children().Append(control);
    return Card(grid);
}

/// Add an element as it fits the parent: plainly, or as a card row.
inline void Place(Parent const& p, xaml::UIElement const& element) {
    if (p.isList) p.Add(Card(element));
    else p.Add(element);
}

inline controls::StackPanel Stacked(TextFn const& label, xaml::UIElement const& control) {
    controls::StackPanel panel;
    panel.Spacing(6);
    if (label) panel.Children().Append(TextOf(label));
    panel.Children().Append(control);
    return panel;
}

// ---------------------------------------------------------------------------

struct TextProps {
    Str content;
    char const* style = nullptr;
    char const* emphasis = nullptr;
    /// `mark:`: a role whose color bands each line.
    char const* mark = nullptr;
    /// At most this many lines, the last ending with an ellipsis.
    std::optional<double> lines;
    Len fontSize;
    /// The height of each line.
    Len lineHeight;
};

inline void Text(Parent const& p, Ctx const& ctx, TextProps props) {
    auto block = TextOf(Bound(ctx, props.content));
    ApplyTextStyle(block, props.style);
    if (props.lines) {
        block.MaxLines(static_cast<int32_t>((std::max)(1.0, *props.lines)));
        block.TextTrimming(xaml::TextTrimming::CharacterEllipsis);
    }
    BindLen(ctx, props.fontSize, nullptr, [block](double size) { block.FontSize((std::max)(1.0, size)); });
    if (props.lineHeight.Given()) {
        block.LineStackingStrategy(xaml::LineStackingStrategy::BlockLineHeight);
        BindLen(ctx, props.lineHeight, nullptr, [block](double height) { block.LineHeight((std::max)(0.0, height)); });
    }
    if (props.emphasis && (std::string_view(props.emphasis) == "secondary" || std::string_view(props.emphasis) == "tertiary")) Secondary(block);
    if (!props.mark) {
        Place(p, block);
        return;
    }
    // The marks lie behind the text in a layer of their own.
    controls::Grid holder;
    controls::Canvas layer;
    layer.IsHitTestVisible(false);
    holder.Children().Append(layer);
    holder.Children().Append(block);
    Paint paint{props.mark};
    auto redraw = [weak = winrt::make_weak(block), layer, paint] {
        if (auto text = weak.get()) DrawMarks(text, layer, paint);
    };
    block.SizeChanged([redraw](auto&&, auto&&) { redraw(); });
    auto content = Bound(ctx, props.content);
    // After a new text is laid out.
    Reactive::Run([content, weak = winrt::make_weak(block), redraw] {
        if (content) content();
        if (auto text = weak.get()) {
            auto token = std::make_shared<winrt::event_token>();
            *token = text.LayoutUpdated([weak, token, redraw](auto&&, auto&&) {
                if (auto text = weak.get()) text.LayoutUpdated(*token);
                redraw();
            });
        }
    });
    Place(p, holder);
}

struct LabelProps {
    Str title;
    char const* icon = nullptr;
    char const* style = nullptr;
    /// The size of the title and the icon.
    std::optional<double> fontSize;
    /// `display: icon`: the icon alone, the title its accessible name.
    bool iconOnly = false;
};

inline void Label(Parent const& p, Ctx const& ctx, LabelProps props) {
    auto title = Bound(ctx, props.title);
    if (props.iconOnly && props.icon) {
        auto icon = Icons::Icon(props.icon, props.fontSize.value_or(16));
        if (title)
            Reactive::Run([icon, title] {
                auto name = title();
                xaml::Automation::AutomationProperties::SetName(icon, name);
                controls::ToolTipService::SetToolTip(icon, winrt::box_value(name));
            });
        Place(p, icon);
        return;
    }
    // The icon scales with the text, as a symbol in a label does, and
    // stands a little more than half the text's size before it.
    auto block = TextOf(title);
    block.VerticalAlignment(xaml::VerticalAlignment::Center);
    ApplyTextStyle(block, props.style);
    if (props.fontSize) block.FontSize(*props.fontSize);
    double size = block.FontSize();
    controls::StackPanel row;
    row.Orientation(controls::Orientation::Horizontal);
    row.Spacing((std::clamp)(std::round(size * 0.6), 4.0, 12.0));
    if (props.icon) {
        auto icon = Icons::Icon(props.icon, std::round(size * 1.15));
        icon.VerticalAlignment(xaml::VerticalAlignment::Center);
        row.Children().Append(icon);
    }
    row.Children().Append(block);
    Place(p, row);
}

inline xaml::UIElement Content(TextFn const& text, char const* icon) {
    if (icon && text) {
        controls::StackPanel content;
        content.Orientation(controls::Orientation::Horizontal);
        content.Spacing(8);
        content.Children().Append(Icons::Icon(icon));
        content.Children().Append(TextOf(text));
        return content;
    }
    if (icon) return Icons::Icon(icon);
    return TextOf(text);
}

/// The kinds of a `files:` literal, such as `"video .srt"`.
inline std::vector<std::string> FileWords(char const* kinds) {
    std::vector<std::string> words;
    std::string text = kinds ? kinds : "any";
    std::string word;
    for (char c : text + " ") {
        if (c == ' ' || c == ',') {
            if (!word.empty()) words.push_back(word);
            word.clear();
        } else {
            word += static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
        }
    }
    return words;
}

/// The extension of a file name or URL, lowercase, without the dot.
inline std::string ExtensionOf(std::string const& name) {
    auto path = name.substr(0, name.find_first_of("?#"));
    auto dot = path.find_last_of('.');
    auto slash = path.find_last_of("/\\");
    if (dot == std::string::npos || (slash != std::string::npos && dot < slash)) return {};
    auto ext = path.substr(dot + 1);
    for (auto& c : ext) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    return ext;
}

/// Whether a file named `name` (or at a URL) is one of `kinds`.
inline bool Accepts(std::string const& name, char const* kinds) {
    auto ext = ExtensionOf(name);
    for (auto const& word : FileWords(kinds)) {
        if (word == "any") return true;
        if (word == "folder") continue;
        if (word[0] == '.') {
            if (ext == word.substr(1)) return true;
            continue;
        }
        for (auto const& [kind, extensions] : app->catalog.fileKinds)
            if (kind == word && std::find(extensions.begin(), extensions.end(), ext) != extensions.end()) return true;
    }
    return false;
}

/// A `MediaSource` for `url`.
inline Value SourceValue(std::string const& url) { return Value::Record({Value::Text(url)}); }

/// Give a picked or dropped file to `field`: a `MediaSource`, or an
/// optional one.
inline void GiveFile(Ctx const& ctx, Field field, std::string const& url) {
    auto const& type = app->catalog.sessions[field.session].fields[field.field].second;
    auto value = SourceValue(url);
    ctx.Write(field, type.kind == BType::Kind::Optional ? Value::Some(value) : value);
}

inline winrt::fire_and_forget PickFile(Ctx ctx, char const* kinds, std::function<void(std::string const&)> give);

struct ButtonProps {
    char const* id = "";
    Str label;
    char const* icon = nullptr;
    char const* role = nullptr;
    char const* prominence = nullptr;
    Get enabled;
    Act action;
    StyleDef const* style = nullptr;
    /// `pick:`: the session field a chosen file goes into before the
    /// action runs.
    std::optional<Field> pick;
    char const* files = nullptr;
    std::optional<ControlShape> shape;
    /// `width: fill`: across its container.
    bool fill = false;
    std::optional<double> iconSize;
};

/// What a button does: its action, after asking for a file with `pick`.
/// `give` is what external entries (`ui.set_file`) and the open panel use.
struct ButtonRun {
    std::function<void()> activate;
    std::function<void(std::string const&)> give;
};

inline ButtonRun RunOf(Ctx const& ctx, std::optional<Field> pick, char const* files, Act const& action) {
    ButtonRun run;
    auto act = Run(ctx, action);
    if (!pick) {
        run.activate = act;
        return run;
    }
    auto field = *pick;
    run.give = [ctx, field, act](std::string const& url) {
        GiveFile(ctx, field, url);
        if (act) act();
    };
    run.activate = [ctx, files, give = run.give] { PickFile(ctx, files, give); };
    return run;
}

/// Show `style!`'s look on a button: its lightweight resources, corner,
/// padding and size; its scale, offset, opacity and shadow on `moving`.
inline void StyleButton(controls::Button const& button, xaml::UIElement const& moving, StyleDef const* style, std::shared_ptr<ShadowImage> shadow = nullptr) {
    if (!style) return;
    if (style->draws) {
        auto resources = ButtonResources(*style, L"Button");
        if (!resources.empty()) Styles::Restyle(button, ThemedXaml(resources));
    }
    if (style->corner) button.CornerRadius(xaml::CornerRadius{*style->corner, *style->corner, *style->corner, *style->corner});
    if (style->padding) button.Padding(xaml::Thickness{*style->padding, *style->padding, *style->padding, *style->padding});
    if (style->size) {
        std::string_view size = style->size;
        double height = size == "small" ? 24 : size == "large" ? 42 : size == "extra_large" ? 52 : 32;
        double font = size == "small" ? 12 : size == "large" ? 15 : size == "extra_large" ? 17 : 14;
        double inline_ = size == "small" ? 8 : size == "large" ? 18 : size == "extra_large" ? 22 : 12;
        button.MinHeight(height);
        button.MinWidth(height);
        button.FontSize(font);
        if (!style->padding) button.Padding(xaml::Thickness{inline_, 0, inline_, 0});
    }
    auto state = std::make_shared<StyleState>();
    auto changed = [weak = winrt::make_weak(moving), style, state, shadow] {
        auto element = weak.get();
        if (!element) return;
        MoveTo(element, *style, *state);
        if (shadow) {
            bool quiet = false, system = false;
            auto look = Merged(*style, state->selected, state->hover, state->pressed, state->focus, state->disabled, quiet, system);
            static Motion const fade{0.25, 1};
            if (look.shadow) {
                auto [radius, y, opacity] = *look.shadow;
                if (radius != shadow->radius || y != shadow->y || opacity != shadow->opacity) {
                    shadow->radius = radius;
                    shadow->y = y;
                    shadow->opacity = opacity;
                    shadow->Draw();
                }
            }
            LayerTo(shadow->image, "state", 0, 0, 1, 1, look.shadow ? 1 : 0, &fade);
        }
    };
    TrackState(button, state, changed);
    changed();
}

/// A styled control with a shadow: the shadow behind it, both moving
/// together in a wrapper. Returns the wrapper.
inline controls::Grid Shadowed(controls::Button const& button, StyleDef const* style, std::shared_ptr<ShadowImage>& shadow) {
    auto wrapper = Wrap(button);
    bool shadows = style->base.shadow || (style->hover && style->hover->shadow) || (style->pressed && style->pressed->shadow);
    if (!shadows) return wrapper;
    shadow = std::make_shared<ShadowImage>();
    shadow->corner = style->corner.value_or(4);
    shadow->image.IsHitTestVisible(false);
    shadow->image.Stretch(media::Stretch::Fill);
    LayersOf(shadow->image)->Get("state")->opacity->Set(0);
    wrapper.Children().InsertAt(0, shadow->image);
    button.SizeChanged([weak = std::weak_ptr<ShadowImage>(shadow), button = winrt::make_weak(button)](auto&&, xaml::SizeChangedEventArgs const& args) {
        auto image = weak.lock();
        auto control = button.get();
        if (!image || !control) return;
        image->width = args.NewSize().Width;
        image->height = args.NewSize().Height;
        image->scale = ScaleOf(control);
        if (image->radius > 0) image->Draw();
    });
    return wrapper;
}

/// Take the keyboard focus to `control`, while it lives.
inline std::function<void()> FocusOf(controls::Control const& control) {
    return [weak = winrt::make_weak(control)] {
        if (auto control = weak.get()) control.Focus(xaml::FocusState::Programmatic);
    };
}

/// `default_focus`: the keyboard focus starts on the control `id` once it
/// is on screen, unless something in its window has it.
inline void DefaultFocus(Ctx const& ctx, char const* id) {
    if (!ctx.win || !ctx.win->root) return;
    auto target = ctx.Target(id);
    Host::After(std::chrono::milliseconds(0), [target, root = winrt::make_weak(ctx.win->root)] {
        auto element = root.get();
        if (!element || !element.XamlRoot()) return;
        auto focused = xaml::Input::FocusManager::GetFocusedElement(element.XamlRoot());
        if (focused && focused.try_as<controls::Control>()) return;
        for (auto const& [token, entry] : Controls::entries) {
            if (entry.target == target && entry.actions.focus) {
                entry.actions.focus();
                return;
            }
        }
    });
}

inline void Button(Parent const& p, Ctx const& ctx, ButtonProps props) {
    controls::Button button;
    auto text = Bound(ctx, props.label);
    button.Content(Content(text, props.icon));
    std::string_view prominence = props.prominence ? props.prominence : "";
    std::string_view role = props.role ? props.role : "";
    if (prominence == "prominent") {
        if (auto accent = Styles::Named(L"AccentButtonStyle")) button.Style(accent);
    } else if (prominence == "plain") {
        Styles::MakeSubtle(button);
    } else if (role == "destructive") {
        Styles::MakeCritical(button);
    }
    BindName(button, text);
    auto isEnabled = Enabled(ctx, props.enabled);
    if (isEnabled) Reactive::Run([button, isEnabled] { button.IsEnabled(isEnabled()); });
    auto run = RunOf(ctx, props.pick, props.files, props.action);
    if (run.activate) button.Click([activate = run.activate](auto&&, auto&&) { activate(); });
    Controls::Register(ctx, props.id, "button", text, isEnabled, Actions{.activate = run.activate, .focus = FocusOf(button), .setFile = run.give, .files = props.files});
    xaml::UIElement shown = button;
    ShapeControl(ctx, button, props.shape);
    if (props.fill) {
        button.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
        SetRole(button, L"bern-fill-width");
    }
    if (props.style) {
        std::shared_ptr<ShadowImage> shadow;
        auto wrapper = Shadowed(button, props.style, shadow);
        StyleButton(button, wrapper, props.style, shadow);
        shown = wrapper;
    }
    if (p.isList && !ctx.toolbar) p.Add(Card(shown));
    else p.Add(shown);
}

/// The widest border a style draws in any state; 0 when it draws none.
inline double BorderWidth(StyleDef const& style) {
    double width = 0;
    for (auto look : {std::optional<Look>(style.base), style.hover, style.pressed, style.selected, style.focus, style.disabled})
        if (look && look->border) width = (std::max)(width, look->borderWidth.value_or(1.0));
    return width;
}

/// A button showing its content instead of a label: no frame of its own,
/// the content as it is; a style's corner clips it.
inline void ContentButton(Parent const& p, Ctx const& ctx, ButtonProps props, Build const& content) {
    controls::Button button;
    auto holder = MakeFlex();
    content(Parent::Of(holder), ctx);
    button.Content(holder);
    button.HorizontalContentAlignment(xaml::HorizontalAlignment::Stretch);
    button.VerticalContentAlignment(xaml::VerticalAlignment::Stretch);
    // A capsule is the platform's button around the content, as it is
    // around a label; otherwise the content is all there is.
    bool framed = props.shape && props.shape->capsule;
    if (framed) {
        if (std::string_view(props.prominence ? props.prominence : "") == "prominent")
            if (auto accent = Styles::Named(L"AccentButtonStyle")) button.Style(accent);
    } else {
        // No frame: no border either, so the content and a style's padding
        // give its size, as on the other platforms.
        button.Padding(xaml::Thickness{0, 0, 0, 0});
        button.BorderThickness(xaml::Thickness{0, 0, 0, 0});
        Styles::MakeSubtle(button);
    }
    // A button takes its content's size; content that fills stretches it.
    Mirror(button, holder);
    if (FillsRow(holder)) button.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    // Content that does not fill the height stands in the middle of a
    // taller button, as a label does.
    if (FillsColumn(holder)) button.VerticalAlignment(xaml::VerticalAlignment::Stretch);
    else button.VerticalContentAlignment(xaml::VerticalAlignment::Center);
    auto text = Bound(ctx, props.label);
    BindName(button, text);
    auto isEnabled = Enabled(ctx, props.enabled);
    if (isEnabled) Reactive::Run([button, isEnabled] { button.IsEnabled(isEnabled()); });
    auto run = RunOf(ctx, props.pick, props.files, props.action);
    if (run.activate) button.Click([activate = run.activate](auto&&, auto&&) { activate(); });
    Controls::Register(ctx, props.id, "button", text, isEnabled, Actions{.activate = run.activate, .focus = FocusOf(button), .setFile = run.give, .files = props.files});
    xaml::UIElement shown = button;
    // A capsule clips its content to the button's corners.
    ShapeControl(ctx, button, props.shape);
    if (props.shape && props.shape->capsule)
        RoundedClip(holder.as<xaml::FrameworkElement>(), [weak = winrt::make_weak(button)](float, float) {
            auto button = weak.get();
            return button ? static_cast<float>(button.CornerRadius().TopLeft) : 0.0f;
        });
    if (props.style) {
        std::shared_ptr<ShadowImage> shadow;
        auto wrapper = Shadowed(button, props.style, shadow);
        StyleButton(button, wrapper, props.style, shadow);
        // A border the style draws stands inside its padding.
        if (!framed)
            if (double width = BorderWidth(*props.style); width > 0) {
                double inside = (std::max)(0.0, props.style->padding.value_or(0) - width);
                button.BorderThickness(xaml::Thickness{width, width, width, width});
                button.Padding(xaml::Thickness{inside, inside, inside, inside});
            }
        // The corner clips the content where the button's shape is, padding
        // included: a rounded Composition clip around the content.
        if (props.style->corner) {
            float r = static_cast<float>(*props.style->corner);
            button.CornerRadius(xaml::CornerRadius{r, r, r, r});
            auto visual = xaml::Hosting::ElementCompositionPreview::GetElementVisual(holder);
            auto compositor = visual.Compositor();
            auto geometry = compositor.CreateRoundedRectangleGeometry();
            geometry.CornerRadius(float2{r, r});
            visual.Clip(compositor.CreateGeometricClip(geometry));
            // The button's bounds, in the content's coordinates.
            auto fit = [geometry, weakButton = winrt::make_weak(button), weakHolder = winrt::make_weak(holder.as<xaml::FrameworkElement>())] {
                auto button = weakButton.get();
                auto content = weakHolder.get();
                if (!button || !content) return;
                winrt::Windows::Foundation::Point at{};
                try {
                    at = content.TransformToVisual(button).TransformPoint({0, 0});
                } catch (winrt::hresult_error const&) {
                    return;
                }
                geometry.Offset(float2{-at.X, -at.Y});
                geometry.Size(float2{static_cast<float>(button.ActualWidth()), static_cast<float>(button.ActualHeight())});
            };
            holder.as<xaml::FrameworkElement>().SizeChanged([fit](auto&&, auto&&) { fit(); });
            button.SizeChanged([fit](auto&&, auto&&) { fit(); });
        }
        shown = wrapper;
    }
    p.Add(shown);
}

struct IconButtonProps {
    char const* id = "";
    Str label;
    char const* icon = "circle";
    char const* role = nullptr;
    char const* prominence = nullptr;
    Get enabled;
    Act action;
    StyleDef const* style = nullptr;
    std::optional<Field> pick;
    char const* files = nullptr;
    /// `height`: a button of that size, round when prominent.
    std::optional<ControlShape> shape;
    bool fill = false;
    /// `font_size`: the size of the icon.
    std::optional<double> iconSize;
};

/// An icon button named by its label, as a Fluent app bar button: no
/// background until pointed at, the control's own corners; with `height`
/// that size. `prominent` takes the accent, and with `height` is a round
/// button of that diameter (the design's play button).
inline void IconButton(Parent const& p, Ctx const& ctx, IconButtonProps props) {
    controls::Button button;
    button.Content(Icons::Icon(props.icon, props.iconSize.value_or(16)));
    std::string_view prominence = props.prominence ? props.prominence : "";
    bool sized = props.shape && props.shape->height.Given();
    bool accent = prominence == "prominent";
    if (accent) {
        if (auto style = Styles::Named(L"AccentButtonStyle")) button.Style(style);
    } else if (!props.style || !props.style->draws) {
        Styles::MakeSubtle(button);
    }
    // A square around the icon: `height`, a style's control size, or the
    // icon with 10 on each side, as an app bar button.
    auto square = [button, accent](double d) {
        double size = (std::max)(0.0, d);
        button.Width(size);
        button.Height(size);
        button.MinWidth(0);
        button.MinHeight(0);
        button.Padding(xaml::Thickness{0, 0, 0, 0});
        if (accent) button.CornerRadius(xaml::CornerRadius{size / 2, size / 2, size / 2, size / 2});
    };
    if (sized) {
        BindLen(ctx, props.shape->height, nullptr, square);
    } else if (props.style && props.style->size) {
        std::string_view size = props.style->size;
        square(size == "small" ? 24 : size == "large" ? 42 : size == "extra_large" ? 52 : 32);
    } else {
        square(props.iconSize.value_or(16) + 20);
    }
    auto text = Bound(ctx, props.label);
    if (text)
        Reactive::Run([button, text] {
            auto name = text();
            controls::ToolTipService::SetToolTip(button, winrt::box_value(name));
            xaml::Automation::AutomationProperties::SetName(button, name);
        });
    auto isEnabled = Enabled(ctx, props.enabled);
    if (isEnabled) Reactive::Run([button, isEnabled] { button.IsEnabled(isEnabled()); });
    auto run = RunOf(ctx, props.pick, props.files, props.action);
    if (run.activate) button.Click([activate = run.activate](auto&&, auto&&) { activate(); });
    Controls::Register(ctx, props.id, "button", text, isEnabled, Actions{.activate = run.activate, .focus = FocusOf(button), .setFile = run.give, .files = props.files});
    xaml::UIElement shown = button;
    if (props.style) {
        std::shared_ptr<ShadowImage> shadow;
        auto wrapper = Shadowed(button, props.style, shadow);
        StyleButton(button, wrapper, props.style, shadow);
        // The style's size is the square's, not a text button's padding.
        if (!sized && props.style->size && !props.style->padding) button.Padding(xaml::Thickness{0, 0, 0, 0});
        shown = wrapper;
    }
    Place(p, shown);
}

/// Text editing: changes update the draft; draft changes from elsewhere
/// update the text without echoing back. Returns whether an update from the
/// draft is in progress.
inline std::shared_ptr<bool> Editable(std::function<winrt::hstring()> read, std::function<void(winrt::hstring const&)> write, Ctx const& ctx, Field field) {
    auto updating = std::make_shared<bool>(false);
    Reactive::Run([read, write, ctx, field, updating] {
        auto value = Wide(R::AsText(ctx.Read(field)));
        if (Reactive::Untrack([&] { return read(); }) != value) {
            *updating = true;
            write(value);
            *updating = false;
        }
    });
    return updating;
}

inline void SetDraft(Ctx const& ctx, Field field, Value const& value) {
    if (!(ctx.Peek(field) == value)) ctx.Write(field, value);
}

inline bool IsEnter(xaml::Input::KeyRoutedEventArgs const& args) {
    return args.Key() == winrt::Windows::System::VirtualKey::Enter;
}

struct TextInputProps {
    char const* id = "";
    Str label;
    /// `label_hidden`: the label names the field without showing.
    bool labelHidden = false;
    std::optional<Field> field;
    Str prompt;
    char const* kind = nullptr;
    Act submit;
};

inline void TextInput(Parent const& p, Ctx const& ctx, TextInputProps props) {
    if (!props.field) return;
    auto field = *props.field;
    auto text = Bound(ctx, props.label);
    auto placeholder = Bound(ctx, props.prompt);
    auto run = Run(ctx, props.submit);
    std::string_view kind = props.kind ? props.kind : "";
    bool secure = kind == "secure", multiline = kind == "multiline";
    xaml::FrameworkElement control{nullptr};
    // A field takes the free width of its row and gives way when the row
    // is narrow, as a text field does in a SwiftUI stack; in a column it
    // spans the column.
    if (secure) {
        controls::PasswordBox box;
        box.MinWidth(64);
        if (placeholder) Reactive::Run([box, placeholder] { box.PlaceholderText(placeholder()); });
        auto updating = Editable([box] { return box.Password(); }, [box](auto const& v) { box.Password(v); }, ctx, field);
        box.PasswordChanged([weak = winrt::make_weak(box), updating, ctx, field](auto&&, auto&&) {
            if (auto box = weak.get(); box && !*updating) SetDraft(ctx, field, Value::Text(Utf8(box.Password())));
        });
        if (run) box.KeyDown([run](auto&&, xaml::Input::KeyRoutedEventArgs const& args) { if (IsEnter(args)) run(); });
        control = box;
    } else {
        controls::TextBox box;
        box.MinWidth(64);
        box.AcceptsReturn(multiline);
        box.TextWrapping(multiline ? xaml::TextWrapping::Wrap : xaml::TextWrapping::NoWrap);
        if (multiline) box.MinHeight(80);
        if (placeholder) Reactive::Run([box, placeholder] { box.PlaceholderText(placeholder()); });
        auto updating = Editable([box] { return box.Text(); }, [box](auto const& v) { box.Text(v); }, ctx, field);
        box.TextChanged([weak = winrt::make_weak(box), updating, ctx, field](auto&&, auto&&) {
            if (auto box = weak.get(); box && !*updating) SetDraft(ctx, field, Value::Text(Utf8(box.Text())));
        });
        if (run && !multiline) box.KeyDown([run](auto&&, xaml::Input::KeyRoutedEventArgs const& args) { if (IsEnter(args)) run(); });
        control = box;
    }
    BindName(control, text);
    SetRole(control, L"bern-fill-width");
    if (p.isList) {
        if (multiline || props.labelHidden) {
            control.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
            p.Add(Card(props.labelHidden ? xaml::UIElement(control) : xaml::UIElement(Stacked(text, control))));
        } else {
            p.Add(LabeledCard(text, control));
        }
    } else if (props.labelHidden) {
        p.Add(control);
    } else {
        auto stacked = Stacked(text, control);
        Mirror(stacked, control);
        p.Add(stacked);
    }
    Actions actions{.edit = [ctx, field](std::string const& value) { SetDraft(ctx, field, Value::Text(value)); }, .submit = run};
    if (!secure) actions.value = [ctx, field] { return std::optional<std::string>(R::AsText(ctx.Peek(field))); };
    Controls::Register(ctx, props.id, "text_input", text, nullptr, std::move(actions));
}

struct SearchProps {
    char const* id = "";
    std::optional<Field> field;
    Str prompt;
    Act submit;
    /// A palette's prompt inside its panel.
    Str panelPrompt;
    /// A palette's panel size; a field in the page its own size.
    std::optional<double> width;
    std::optional<double> height;
    std::optional<double> fontSize;
    /// The results show on focus, before anything is typed.
    bool suggest = false;
    /// A palette's springs: growing out of its field, following its
    /// results' height, going back into the field.
    Motion const* grow = nullptr;
    Motion const* follow = nullptr;
    Motion const* shrink = nullptr;
};

inline controls::AutoSuggestBox SearchBox(Ctx const& ctx, SearchProps const& props) {
    auto field = *props.field;
    auto placeholder = Bound(ctx, props.prompt);
    controls::AutoSuggestBox box;
    box.QueryIcon(controls::SymbolIcon(controls::Symbol::Find));
    if (placeholder)
        Reactive::Run([box, placeholder] {
            auto text = placeholder();
            box.PlaceholderText(text);
            xaml::Automation::AutomationProperties::SetName(box, text);
        });
    auto updating = Editable([box] { return box.Text(); }, [box](auto const& v) { box.Text(v); }, ctx, field);
    box.TextChanged([weak = winrt::make_weak(box), updating, ctx, field](auto&&, auto&&) {
        if (auto box = weak.get(); box && !*updating) SetDraft(ctx, field, Value::Text(Utf8(box.Text())));
    });
    auto run = Run(ctx, props.submit);
    box.QuerySubmitted([run](auto&&, auto&&) { if (run) run(); });
    Controls::Register(ctx, props.id, "search", placeholder ? placeholder : TextFn([] { return winrt::hstring(); }), nullptr,
        Actions{.edit = [ctx, field](std::string const& value) { SetDraft(ctx, field, Value::Text(value)); },
            .submit = run,
            .value = [ctx, field] { return std::optional<std::string>(R::AsText(ctx.Peek(field))); },
            .focus = [weak = winrt::make_weak(box)] {
                if (auto box = weak.get()) box.Focus(xaml::FocusState::Programmatic);
            }});
    return box;
}

/// A flyout presenter style for content in an `appearance:` part.
/// A resource of one theme (`Dark`/`Default` or `Light`) in the
/// application's theme dictionaries, through merged dictionaries.
inline winrt::Windows::Foundation::IInspectable ThemeValue(xaml::ResourceDictionary const& dictionary, wchar_t const* theme, wchar_t const* key, int depth = 0) {
    if (!dictionary || depth > 8) return nullptr;
    try {
        auto themes = dictionary.ThemeDictionaries();
        if (auto boxed = winrt::box_value(theme); themes.HasKey(boxed))
            if (auto inner = themes.Lookup(boxed).try_as<xaml::ResourceDictionary>()) {
                if (auto boxedKey = winrt::box_value(key); inner.HasKey(boxedKey)) return inner.Lookup(boxedKey);
                if (auto value = ThemeValue(inner, theme, key, depth + 1)) return value;
            }
        for (auto merged : dictionary.MergedDictionaries())
            if (auto value = ThemeValue(merged, theme, key, depth + 1)) return value;
    } catch (winrt::hresult_error const&) {
    }
    return nullptr;
}

/// The look of a flyout opened from a surface of a fixed appearance, such
/// as a dark player in a light window. The theme alone turns the items'
/// text over but leaves the presenter's surface in the window's theme, so
/// the style also takes that theme's own surface and border brushes.
inline xaml::Style PresenterTheme(wchar_t const* target, std::string const& appearance) {
    if (appearance.empty()) return nullptr;
    static auto* cache = new std::map<std::wstring, xaml::Style>();
    std::wstring key = std::wstring(target) + L"|" + std::wstring(Wide(appearance));
    if (auto found = cache->find(key); found != cache->end()) return found->second;
    bool dark = appearance == "dark";
    xaml::Style style{nullptr};
    try {
        style = xaml::Markup::XamlReader::Load(LR"(<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType=")" + std::wstring(target) +
            LR"("><Setter Property="RequestedTheme" Value=")" + std::wstring(dark ? L"Dark" : L"Light") + L"\" /></Style>")
                    .as<xaml::Style>();
        std::wstring_view kind(target);
        wchar_t const* background = kind == L"MenuFlyoutPresenter" ? L"MenuFlyoutPresenterBackground" : kind == L"FlyoutPresenter" ? L"FlyoutPresenterBackground" : nullptr;
        wchar_t const* border = kind == L"MenuFlyoutPresenter" ? L"MenuFlyoutPresenterBorderBrush" : kind == L"FlyoutPresenter" ? L"FlyoutBorderThemeBrush" : nullptr;
        auto resources = xaml::Application::Current().Resources();
        auto lookup = [&](wchar_t const* name) -> winrt::Windows::Foundation::IInspectable {
            if (!name) return nullptr;
            for (auto theme : dark ? std::initializer_list<wchar_t const*>{L"Dark", L"Default"} : std::initializer_list<wchar_t const*>{L"Light"})
                if (auto value = ThemeValue(resources, theme, name)) return value;
            return nullptr;
        };
        if (auto value = lookup(background)) style.Setters().Append(xaml::Setter(controls::Control::BackgroundProperty(), value));
        if (auto value = lookup(border)) style.Setters().Append(xaml::Setter(controls::Control::BorderBrushProperty(), value));
    } catch (winrt::hresult_error const& error) {
        Report("styles: " + Utf8(error.message()));
    }
    cache->emplace(key, style);
    return style;
}

/// A search field; with results, they show below it while it is in use.
inline void Search(Parent const& p, Ctx const& ctx, SearchProps props, Build const& results) {
    if (!props.field) return;
    auto box = SearchBox(ctx, props);
    if (ctx.toolbar) box.Width(240);
    // A field standing in the page at its own size.
    if (props.width) box.Width(*props.width);
    if (props.height) box.Height(*props.height);
    if (props.fontSize) box.FontSize(*props.fontSize);
    if (results) {
        auto content = MakeFlex(false, 4, xaml::Thickness{4, 4, 4, 4});
        results(Parent::Of(content), ctx);
        controls::Flyout flyout;
        if (auto style = PresenterTheme(L"FlyoutPresenter", ctx.appearance)) flyout.FlyoutPresenterStyle(style);
        controls::ScrollViewer scroller;
        scroller.Content(content);
        scroller.MaxHeight(360);
        flyout.Content(scroller);
        auto field = *props.field;
        auto weakBox = winrt::make_weak(box);
        auto open = std::make_shared<bool>(false);
        // The field's text box inside the search box has the focus.
        auto focused = std::make_shared<bool>(false);
        bool suggest = props.suggest;
        auto show = [weakBox, flyout, open, focused, ctx, field, suggest] {
            auto box = weakBox.get();
            if (!box) return;
            bool typed = suggest || !R::AsText(ctx.Peek(field)).empty();
            if (*focused && typed && !*open) {
                controls::Primitives::FlyoutShowOptions options;
                options.ShowMode(controls::Primitives::FlyoutShowMode::Transient);
                options.Placement(controls::Primitives::FlyoutPlacementMode::BottomEdgeAlignedLeft);
                flyout.ShowAt(box, options);
            } else if (!typed && *open) {
                flyout.Hide();
            }
        };
        flyout.Opened([open, weakBox, scroller](auto&&, auto&&) {
            *open = true;
            if (auto box = weakBox.get()) scroller.MinWidth(box.ActualWidth());
        });
        flyout.Closed([open](auto&&, auto&&) { *open = false; });
        box.GotFocus([show, focused](auto&&, auto&&) {
            *focused = true;
            show();
        });
        box.LostFocus([focused](auto&&, auto&&) { *focused = false; });
        Reactive::Run([ctx, field, show] {
            ctx.Read(field);
            Reactive::Untrack([&] {
                show();
                return 0;
            });
        });
        // Choosing a result closes the list.
        content.AddHandler(xaml::UIElement::TappedEvent(), winrt::box_value(xaml::Input::TappedEventHandler([flyout](auto&&, auto&&) { flyout.Hide(); })), true);
        Reactive::OnCleanup([flyout] { flyout.Hide(); });
    }
    Place(p, box);
}

struct SwitchProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    /// Runs after the draft changed.
    Act then;
    /// `checked:`: engine state the switch shows; a change runs `change`
    /// with the new value.
    Get current;
    std::function<void(Ctx const&, Value const&)> change;
    char const* style = nullptr;
    char const* icon = nullptr;
    bool labelHidden = false;
    std::optional<ControlShape> shape;
};

/// What a control shows and how a change is made: a draft field, after
/// which an action runs, or a value read from state whose change runs a
/// command.
struct Model {
    std::function<Value()> read;
    std::function<Value()> peek;
    std::function<void(Value const&)> write;
};

inline Model ModelOf(Ctx const& ctx, std::optional<Field> field, Act then, Get current, std::function<void(Ctx const&, Value const&)> change) {
    Model model;
    if (current) {
        model.read = [ctx, current] { return current(ctx); };
        model.peek = [ctx, current] { return Reactive::Untrack([&] { return current(ctx); }); };
        model.write = [ctx, change](Value const& value) {
            if (change) change(ctx, value);
        };
    } else if (field) {
        auto f = *field;
        model.read = [ctx, f] { return ctx.Read(f); };
        model.peek = [ctx, f] { return ctx.Peek(f); };
        model.write = [ctx, f, then](Value const& value) {
            SetDraft(ctx, f, value);
            if (then) then(ctx);
        };
    }
    return model;
}

inline void Switch(Parent const& p, Ctx const& ctx, SwitchProps props) {
    auto model = ModelOf(ctx, props.field, props.then, props.current, props.change);
    if (!model.read) return;
    auto text = Bound(ctx, props.label);
    auto updating = std::make_shared<bool>(false);
    auto on = [model] { return model.read() == Value::Bool(true); };
    auto set = [model](bool value) { model.write(Value::Bool(value)); };
    std::function<void()> flip = [model] { model.write(Value::Bool(!(model.peek() == Value::Bool(true)))); };
    Actions actions{.activate = flip,
        .setBool = set,
        .value = [model] { return std::optional<std::string>(model.peek() == Value::Bool(true) ? "true" : "false"); }};
    if ((props.style && std::string_view(props.style) == "button") || ctx.toolbar) {
        // A button that stays pressed while on; in a toolbar its icon alone.
        controls::Primitives::ToggleButton toggle;
        bool iconOnly = props.icon && ctx.toolbar;
        if (iconOnly) toggle.Content(Icons::Icon(props.icon));
        else toggle.Content(Content(text, props.icon));
        if (text)
            Reactive::Run([toggle, text] {
                auto name = text();
                controls::ToolTipService::SetToolTip(toggle, winrt::box_value(name));
                xaml::Automation::AutomationProperties::SetName(toggle, name);
            });
        Reactive::Run([toggle, on, updating] {
            bool value = on();
            auto current = toggle.IsChecked();
            if (!current || current.Value() != value) {
                *updating = true;
                toggle.IsChecked(value);
                *updating = false;
            }
        });
        toggle.Checked([updating, set](auto&&, auto&&) { if (!*updating) set(true); });
        toggle.Unchecked([updating, set](auto&&, auto&&) { if (!*updating) set(false); });
        ShapeControl(ctx, toggle, props.shape);
        if (p.isList && !ctx.toolbar) p.Add(props.labelHidden ? xaml::UIElement(Card(toggle)) : xaml::UIElement(LabeledCard(text, toggle)));
        else p.Add(toggle);
        Controls::Register(ctx, props.id, "switch", text, nullptr, std::move(actions));
        return;
    }
    if (props.style && std::string_view(props.style) == "checkbox") {
        controls::CheckBox check;
        if (text && !props.labelHidden) check.Content(TextOf(text));
        if (props.labelHidden) check.MinWidth(0);
        BindName(check, text);
        Reactive::Run([check, on, updating] {
            bool value = on();
            auto current = check.IsChecked();
            if (!current || current.Value() != value) {
                *updating = true;
                check.IsChecked(value);
                *updating = false;
            }
        });
        check.Checked([updating, set](auto&&, auto&&) { if (!*updating) set(true); });
        check.Unchecked([updating, set](auto&&, auto&&) { if (!*updating) set(false); });
        Place(p, check);
    } else {
        controls::ToggleSwitch toggle;
        // The switch alone, as the design draws it: its label says what it
        // turns on, so no “On”/“Off” beside it.
        toggle.OnContent(nullptr);
        toggle.OffContent(nullptr);
        toggle.MinWidth(0);
        BindName(toggle, text);
        Reactive::Run([toggle, on, updating] {
            bool value = on();
            if (toggle.IsOn() != value) {
                *updating = true;
                toggle.IsOn(value);
                *updating = false;
            }
        });
        toggle.Toggled([weak = winrt::make_weak(toggle), updating, set](auto&&, auto&&) {
            if (auto toggle = weak.get(); toggle && !*updating) set(toggle.IsOn());
        });
        if (p.isList) {
            toggle.MinWidth(0);
            p.Add(props.labelHidden ? xaml::UIElement(Card(toggle)) : xaml::UIElement(LabeledCard(text, toggle)));
        } else {
            if (text && !props.labelHidden) toggle.Header(TextOf(text));
            p.Add(toggle);
        }
    }
    Controls::Register(ctx, props.id, "switch", text, nullptr, std::move(actions));
}

struct SliderProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    /// `current:`: engine state the slider shows; letting go runs
    /// `change` with the new value.
    Get current;
    std::function<void(Ctx const&, Value const&)> change;
    std::optional<double> min, max, step;
    char const* minIcon = nullptr;
    char const* maxIcon = nullptr;
    Act commit;
    bool labelHidden = false;
};

/// A slider: dragging only updates this window's draft as a preview, and
/// releasing commits once; keyboard changes commit at once. A slider
/// showing engine state moves only itself while dragged and runs its
/// command on release.
inline void Slider(Parent const& p, Ctx const& ctx, SliderProps props) {
    if (!props.field && !props.current) return;
    auto text = Bound(ctx, props.label);
    double low = props.min.value_or(0), high = props.max.value_or(1);
    double step = props.step.value_or((high - low) / 100);
    controls::Slider slider;
    slider.Minimum(low);
    slider.Maximum(high);
    slider.StepFrequency(step);
    slider.SmallChange(step);
    slider.LargeChange((high - low) / 10);
    slider.MinWidth(props.labelHidden ? 80 : 200);
    slider.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    auto run = Run(ctx, props.commit);
    auto snap = [low, high, explicitStep = props.step](double value) {
        if (explicitStep && *explicitStep > 0) value = low + std::round((value - low) / *explicitStep) * *explicitStep;
        return (std::clamp)(value, low, high);
    };
    auto updating = std::make_shared<bool>(false);
    auto dragging = std::make_shared<bool>(false);
    auto field = props.field;
    auto current = props.current;
    auto change = props.change;
    auto read = [ctx, field, current] { return field ? ctx.Read(*field) : current(ctx); };
    auto peek = [ctx, field, current] { return field ? ctx.Peek(*field) : Reactive::Untrack([&] { return current(ctx); }); };
    // Where the value goes: the draft, then the action; or the command.
    auto give = [ctx, field, change, run](double value, bool done) {
        if (field) {
            SetDraft(ctx, *field, Value::Float(value));
            if (done && run) run();
        } else if (done && change) {
            change(ctx, Value::Float(value));
        }
    };
    Reactive::Run([slider, read, updating, dragging] {
        double value = R::AsDouble(read());
        if (*dragging) return;
        if (std::abs(slider.Value() - value) > 1e-12) {
            *updating = true;
            slider.Value(value);
            *updating = false;
        }
    });
    slider.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(xaml::Input::PointerEventHandler([dragging](auto&&, auto&&) { *dragging = true; })), true);
    auto release = [dragging, give, weak = winrt::make_weak(slider), snap](auto&&, auto&&) {
        if (!*dragging) return;
        *dragging = false;
        if (auto slider = weak.get()) give(snap(slider.Value()), true);
    };
    slider.AddHandler(xaml::UIElement::PointerReleasedEvent(), winrt::box_value(xaml::Input::PointerEventHandler(release)), true);
    slider.AddHandler(xaml::UIElement::PointerCaptureLostEvent(), winrt::box_value(xaml::Input::PointerEventHandler(release)), true);
    slider.ValueChanged([updating, dragging, give, snap](auto&&, controls::Primitives::RangeBaseValueChangedEventArgs const& args) {
        if (*updating) return;
        give(snap(args.NewValue()), !*dragging);
    });
    BindName(slider, text);
    controls::Grid track;
    track.ColumnSpacing(8);
    track.ColumnDefinitions().Append(Column(AutoLength()));
    track.ColumnDefinitions().Append(Column(Star()));
    track.ColumnDefinitions().Append(Column(AutoLength()));
    if (props.minIcon) {
        auto icon = Icons::Icon(props.minIcon, 14);
        icon.VerticalAlignment(xaml::VerticalAlignment::Center);
        track.Children().Append(icon);
    }
    controls::Grid::SetColumn(slider, 1);
    track.Children().Append(slider);
    if (props.maxIcon) {
        auto icon = Icons::Icon(props.maxIcon, 14);
        icon.VerticalAlignment(xaml::VerticalAlignment::Center);
        controls::Grid::SetColumn(icon, 2);
        track.Children().Append(icon);
    }
    xaml::UIElement content = props.labelHidden ? xaml::UIElement(track) : xaml::UIElement(Stacked(text, track));
    // A slider takes the width it is given, as on the other targets.
    SetRole(content.as<xaml::FrameworkElement>(), L"bern-fill-width");
    if (p.isList) p.Add(Card(content));
    else p.Add(content);
    Controls::Register(ctx, props.id, "slider", text, nullptr,
        Actions{.setNumber = [give, snap](double number) { give(snap(number), true); },
            .value = [peek] { return std::optional<std::string>(R::Number(R::AsDouble(peek()))); }});
}

/// One option of a picker: variant index, variant name and label.
struct PickOption {
    uint32_t value = 0;
    char const* name = "";
    Str label;
};

/// A row of exclusive toggle buttons: the segmented control of the system
/// XAML, which has no selector bar.
struct Segments {
    xaml::FrameworkElement element{nullptr};
    std::function<void(int)> select;
    std::function<void(std::function<void(int)>)> onSelect;
};

inline Segments MakeSegments(Ctx const& ctx, std::vector<Str> const& labels, std::vector<char const*> const& icons = {}) {
    Segments segments;
#if BERN_WINUI
    {
        controls::SelectorBar bar;
        auto items = std::make_shared<std::vector<controls::SelectorBarItem>>();
        for (size_t i = 0; i < labels.size(); ++i) {
            controls::SelectorBarItem item;
            if (i < icons.size() && icons[i]) item.Icon(Icons::Icon(icons[i]));
            auto label = labels[i];
            Reactive::Run([item, label, ctx] { item.Text(label(ctx)); });
            bar.Items().Append(item);
            items->push_back(item);
        }
        auto updating = std::make_shared<bool>(false);
        // At its own width from the leading edge, as the design draws it.
        bar.HorizontalAlignment(xaml::HorizontalAlignment::Left);
        segments.element = bar;
        segments.select = [bar, items, updating](int index) {
            if (index < 0 || index >= static_cast<int>(items->size()) || bar.SelectedItem() == (*items)[index]) return;
            *updating = true;
            bar.SelectedItem((*items)[index]);
            *updating = false;
        };
        segments.onSelect = [bar, items, updating](std::function<void(int)> handler) {
            bar.SelectionChanged([weak = winrt::make_weak(bar), items, updating, handler](auto&&, auto&&) {
                auto bar = weak.get();
                if (!bar || *updating || !bar.SelectedItem()) return;
                for (size_t i = 0; i < items->size(); ++i)
                    if ((*items)[i] == bar.SelectedItem()) handler(static_cast<int>(i));
            });
        };
    }
#else
    {
        controls::StackPanel row;
        row.Orientation(controls::Orientation::Horizontal);
        row.Spacing(2);
        auto buttons = std::make_shared<std::vector<controls::Primitives::ToggleButton>>();
        auto handler = std::make_shared<std::function<void(int)>>();
        auto updating = std::make_shared<bool>(false);
        for (size_t i = 0; i < labels.size(); ++i) {
            controls::Primitives::ToggleButton button;
            auto label = labels[i];
            Reactive::Run([button, label, ctx] { button.Content(winrt::box_value(label(ctx))); });
            int index = static_cast<int>(i);
            button.Click([buttons, handler, updating, index](auto&&, auto&&) {
                if (*updating) return;
                for (size_t j = 0; j < buttons->size(); ++j) (*buttons)[j].IsChecked(static_cast<int>(j) == index);
                if (*handler) (*handler)(index);
            });
            row.Children().Append(button);
            buttons->push_back(button);
        }
        segments.element = row;
        segments.select = [buttons, updating](int index) {
            *updating = true;
            for (size_t j = 0; j < buttons->size(); ++j) (*buttons)[j].IsChecked(static_cast<int>(j) == index);
            *updating = false;
        };
        segments.onSelect = [handler](std::function<void(int)> next) { *handler = std::move(next); };
    }
#endif
    return segments;
}

/// A `FlowPanel` (defined with `Flow`) whose items are `gap` apart.
inline controls::Panel MakeFlowPanel(double gap);

struct PickerProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    /// Runs after the draft changed.
    Act then;
    /// `selected:`: engine state the picker shows; a choice runs `change`
    /// with the chosen value.
    Get current;
    std::function<void(Ctx const&, Value const&)> change;
    std::vector<PickOption> options;
    char const* style = nullptr;
    bool labelHidden = false;
    std::optional<ControlShape> shape;
};

inline void Picker(Parent const& p, Ctx const& ctx, PickerProps props) {
    auto model = ModelOf(ctx, props.field, props.then, props.current, props.change);
    if (!model.read || props.options.empty()) return;
    auto text = Bound(ctx, props.label);
    auto options = std::make_shared<std::vector<PickOption>>(std::move(props.options));
    auto indexOf = [options](Value const& value) -> std::optional<int> {
        if (value.kind != Value::Kind::Enum) return std::nullopt;
        for (size_t i = 0; i < options->size(); ++i)
            if ((*options)[i].value == value.variant) return static_cast<int>(i);
        return std::nullopt;
    };
    auto choose = [model, options](int index) {
        auto value = Value::Enum((*options)[index].value);
        if (!(model.peek() == value)) model.write(value);
    };
    std::string_view style = props.style ? props.style : "";
    xaml::FrameworkElement control{nullptr};
    if (style == "chips") {
        // A toggle button per option, the chosen one checked. In a flow the
        // buttons are items of the flow; elsewhere they wrap in a flow of
        // their own.
        using Buttons = std::vector<controls::Primitives::ToggleButton>;
        auto buttons = std::make_shared<Buttons>();
        auto check = [](Buttons const& buttons, std::optional<int> chosen) {
            for (size_t j = 0; j < buttons.size(); ++j) buttons[j].IsChecked(chosen && *chosen == static_cast<int>(j));
        };
        for (size_t i = 0; i < options->size(); ++i) {
            controls::Primitives::ToggleButton button;
            auto label = (*options)[i].label;
            Reactive::Run([button, label, ctx] { button.Content(winrt::box_value(label(ctx))); });
            int index = static_cast<int>(i);
            // A click toggles the button itself: the picker's value says
            // which is checked, also when the chosen chip is clicked again.
            button.Click([choose, check, model, indexOf, index, weak = std::weak_ptr<Buttons>(buttons)](auto&&, auto&&) {
                choose(index);
                if (auto buttons = weak.lock()) check(*buttons, indexOf(model.peek()));
            });
            buttons->push_back(button);
        }
        Reactive::Run([buttons, check, model, indexOf] { check(*buttons, indexOf(model.read())); });
        if (p.flow && props.labelHidden && !ctx.toolbar) {
            for (auto const& button : *buttons) p.Add(button);
        } else {
            auto panel = MakeFlowPanel(8);
            for (auto const& button : *buttons) panel.Children().Append(button);
            control = panel;
        }
    } else if (style == "segmented" || style == "radio" || style == "inline") {
        std::vector<Str> labels;
        for (auto const& option : *options) labels.push_back(option.label);
        auto segments = MakeSegments(ctx, labels);
        Reactive::Run([segments, model, indexOf] {
            if (auto index = indexOf(model.read())) segments.select(*index);
        });
        segments.onSelect(choose);
        control = segments.element;
    } else if (props.shape) {
        // A capsule: the chosen option on a drop-down button of its own
        // height, opening a menu of the options.
        controls::DropDownButton button;
        button.HorizontalAlignment(xaml::HorizontalAlignment::Left);
        controls::TextBlock chosen;
        button.Content(chosen);
        controls::MenuFlyout flyout;
        for (size_t i = 0; i < options->size(); ++i) {
            controls::ToggleMenuFlyoutItem item;
            auto label = (*options)[i].label;
            Reactive::Run([item, label, ctx] { item.Text(label(ctx)); });
            int index = static_cast<int>(i);
            item.Click([choose, index](auto&&, auto&&) { choose(index); });
            flyout.Items().Append(item);
        }
        if (auto style = PresenterTheme(L"MenuFlyoutPresenter", ctx.appearance)) flyout.MenuFlyoutPresenterStyle(style);
        button.Flyout(flyout);
        Reactive::Run([chosen, flyout, options, model, indexOf, ctx] {
            auto index = indexOf(model.read());
            chosen.Text(index ? (*options)[*index].label(ctx) : winrt::hstring());
            for (uint32_t i = 0; i < flyout.Items().Size(); ++i)
                if (auto item = flyout.Items().GetAt(i).try_as<controls::ToggleMenuFlyoutItem>()) item.IsChecked(index && *index == static_cast<int>(i));
        });
        ShapeControl(ctx, button, props.shape);
        control = button;
    } else {
        controls::ComboBox combo;
        combo.MinWidth(160);
        auto updating = std::make_shared<bool>(false);
        Reactive::Run([combo, options, ctx, model, indexOf, updating] {
            std::vector<winrt::hstring> labels;
            for (auto const& option : *options) labels.push_back(option.label(ctx));
            *updating = true;
            auto selected = combo.SelectedIndex();
            combo.Items().Clear();
            for (auto const& label : labels) combo.Items().Append(winrt::box_value(label));
            combo.SelectedIndex(indexOf(model.peek()).value_or(selected));
            *updating = false;
        });
        Reactive::Run([combo, model, indexOf, updating] {
            if (auto index = indexOf(model.read()); index && combo.SelectedIndex() != *index) {
                *updating = true;
                combo.SelectedIndex(*index);
                *updating = false;
            }
        });
        combo.SelectionChanged([weak = winrt::make_weak(combo), choose, updating](auto&&, auto&&) {
            if (auto combo = weak.get(); combo && !*updating && combo.SelectedIndex() >= 0) choose(combo.SelectedIndex());
        });
        control = combo;
    }
    // Chips that are items of a flow have no control of their own.
    if (control) {
        BindName(control, text);
        if (ctx.toolbar || props.labelHidden) p.isList && !ctx.toolbar ? p.Add(Card(control)) : p.Add(control);
        else if (p.isList) p.Add(LabeledCard(text, control));
        else p.Add(Stacked(text, control));
    }
    Controls::Register(ctx, props.id, "picker", text, nullptr,
        Actions{.select = [options, choose](std::string const& name) {
                    for (size_t i = 0; i < options->size(); ++i)
                        if ((*options)[i].name == name) { choose(static_cast<int>(i)); return true; }
                    return false;
                },
            .value = [model, options, indexOf]() -> std::optional<std::string> {
                if (auto index = indexOf(model.peek())) return std::string((*options)[*index].name);
                return std::nullopt;
            }});
}

struct StackProps {
    char const* axis = "column";
    Len gap;
    Len padding;
    Len width;
    Len height;
    char const* align = nullptr;
    Motion const* animate = nullptr;
    Motion const* reflow = nullptr;
    /// Padding across and along, in place of `padding` there.
    Len paddingX;
    Len paddingY;
};

/// Bind `padding`, `padding_x` and `padding_y` to `apply`, which takes
/// the horizontal and vertical padding.
inline void BindPadding(Ctx const& ctx, Len const& all, Len const& x, Len const& y, Motion const* motion, std::function<void(double, double)> apply) {
    auto now = std::make_shared<std::array<double, 2>>(std::array<double, 2>{0, 0});
    BindLen(ctx, all, motion, [now, apply, x = x.Given(), y = y.Given()](double v) {
        if (!x) (*now)[0] = (std::max)(0.0, v);
        if (!y) (*now)[1] = (std::max)(0.0, v);
        apply((*now)[0], (*now)[1]);
    });
    BindLen(ctx, x, motion, [now, apply](double v) {
        (*now)[0] = (std::max)(0.0, v);
        apply((*now)[0], (*now)[1]);
    });
    BindLen(ctx, y, motion, [now, apply](double v) {
        (*now)[1] = (std::max)(0.0, v);
        apply((*now)[0], (*now)[1]);
    });
}

inline void Stack(Parent const& p, Ctx const& ctx, StackProps props, Build const& children) {
    bool column = std::string_view(props.axis) == "column";
    auto impl = winrt::make_self<FlexPanel>();
    impl->horizontal = !column;
    impl->spacing = 8;
    impl->reflow = props.reflow;
    controls::Panel panel = impl.as<controls::Panel>();
    SetRole(panel, column ? L"bern-column" : L"bern-row");
    // Lengths follow their expressions, springing with `animate`.
    FlexPanel* flex = impl.get();
    KeepReflow(panel, [flex](Motion const* motion) { flex->reflow = motion; });
    auto keep = winrt::make_weak(panel);
    BindLen(ctx, props.gap, props.animate, [flex, keep](double v) {
        if (!keep.get()) return;
        flex->spacing = (std::max)(0.0, v);
        keep.get().InvalidateMeasure();
    });
    BindPadding(ctx, props.padding, props.paddingX, props.paddingY, props.animate, [flex, keep](double x, double y) {
        if (!keep.get()) return;
        flex->padding = xaml::Thickness{x, y, x, y};
        keep.get().InvalidateMeasure();
    });
    BindLen(ctx, props.width, props.animate, [panel](double v) { panel.Width((std::max)(0.0, v)); });
    BindLen(ctx, props.height, props.animate, [panel](double v) { panel.Height((std::max)(0.0, v)); });
    std::string_view align = props.align ? props.align : "";
    if (column) {
        auto h = align == "center" ? xaml::HorizontalAlignment::Center
            : align == "trailing" ? xaml::HorizontalAlignment::Right
            : align == "leading" ? xaml::HorizontalAlignment::Left
            : xaml::HorizontalAlignment::Stretch;
        children(Parent::Aligned(panel, h, std::nullopt), ctx);
    } else {
        // On the baseline: each child at its top, then moved down so the
        // first lines of text share one baseline.
        flex->baseline = align == "baseline";
        auto v = align == "top" || align == "baseline" ? xaml::VerticalAlignment::Top
            : align == "bottom" ? xaml::VerticalAlignment::Bottom
            : align == "fill" ? xaml::VerticalAlignment::Stretch
            : xaml::VerticalAlignment::Center;
        children(Parent::Aligned(panel, std::nullopt, v), ctx);
    }
    Place(p, panel);
}

/// The cells of a `Grid`: equal columns, as many as fit at `minWidth` or
/// wider, filled row by row; each row as tall as its tallest cell.
struct GridPanel : controls::PanelT<GridPanel> {
    double minWidth = 120;
    double spacing = 8;
    /// Between rows; `spacing` when not given.
    std::optional<double> rowSpacing;
    /// `cell_ratio` and `cell_extra`: every row the column width ÷ ratio
    /// \+ extra tall, its cells at the top.
    std::optional<double> cellRatio;
    double cellExtra = 0;
    xaml::Thickness padding{0, 0, 0, 0};
    B::Motion const* reflow = nullptr;
    Places places;

    std::vector<xaml::UIElement> Shown() {
        std::vector<xaml::UIElement> shown;
        for (auto child : Children())
            if (child.Visibility() == xaml::Visibility::Visible) shown.push_back(child);
        return shown;
    }

    /// Columns and their width in `width`; without a bound, one row.
    std::pair<size_t, float> Columns(float width, size_t count) const {
        auto gap = static_cast<float>(spacing);
        double least = (std::max)(1.0, minWidth);
        if (std::isinf(width)) return {(std::max)(count, size_t{1}), static_cast<float>(least)};
        auto columns = (std::max)(static_cast<size_t>((width + gap) / static_cast<float>(least + spacing)), size_t{1});
        return {columns, (std::max)(0.0f, (width - gap * static_cast<float>(columns - 1)) / static_cast<float>(columns))};
    }

    float Horizontal() const { return static_cast<float>(padding.Left + padding.Right); }
    float Vertical() const { return static_cast<float>(padding.Top + padding.Bottom); }

    float RowGap() const { return static_cast<float>(rowSpacing.value_or(spacing)); }
    std::optional<float> CellHeight(float cell) const {
        if (!cellRatio || *cellRatio <= 0) return std::nullopt;
        return static_cast<float>(cell / *cellRatio + cellExtra);
    }

    Size MeasureOverride(Size available) {
        float inner = std::isinf(available.Width) ? INFINITY : (std::max)(0.0f, available.Width - Horizontal());
        auto shown = Shown();
        auto [columns, cell] = Columns(inner, shown.size());
        float height = 0;
        auto fixed = CellHeight(cell);
        for (size_t start = 0; start < shown.size(); start += columns) {
            float row = 0;
            for (size_t i = start; i < (std::min)(start + columns, shown.size()); ++i) {
                shown[i].Measure(Size{cell, INFINITY});
                row = (std::max)(row, shown[i].DesiredSize().Height);
            }
            if (fixed) row = *fixed;
            height += row + (start > 0 ? RowGap() : 0.0f);
        }
        float width = std::isinf(inner) ? (cell + static_cast<float>(spacing)) * static_cast<float>(shown.size()) - static_cast<float>(spacing) : inner;
        return Size{(std::max)(0.0f, width) + Horizontal(), height + Vertical()};
    }

    Size ArrangeOverride(Size final) {
        auto shown = Shown();
        auto [columns, cell] = Columns((std::max)(0.0f, final.Width - Horizontal()), shown.size());
        auto gap = static_cast<float>(spacing);
        float top = static_cast<float>(padding.Top);
        auto fixed = CellHeight(cell);
        std::vector<std::pair<xaml::UIElement, winrt::Windows::Foundation::Point>> now;
        for (size_t start = 0; start < shown.size(); start += columns) {
            auto end = (std::min)(start + columns, shown.size());
            float row = 0;
            for (size_t i = start; i < end; ++i) row = (std::max)(row, shown[i].DesiredSize().Height);
            for (size_t i = start; i < end; ++i) {
                // In a row of cells, taller content runs into the gap below.
                float height = fixed ? (std::max)(*fixed, shown[i].DesiredSize().Height) : row;
                Rect rect{static_cast<float>(padding.Left) + static_cast<float>(i - start) * (cell + gap), top, cell, height};
                shown[i].Arrange(rect);
                if (reflow) now.emplace_back(shown[i], winrt::Windows::Foundation::Point{rect.X, rect.Y});
            }
            top += (fixed ? *fixed : row) + RowGap();
        }
        for (auto child : Children())
            if (child.Visibility() != xaml::Visibility::Visible) child.Arrange(Rect{0, 0, 0, 0});
        if (reflow) places.Arranged(reflow, std::move(now));
        return final;
    }
};

struct GridProps {
    Len minWidth;
    Len gap;
    Len padding;
    Motion const* animate = nullptr;
    Motion const* reflow = nullptr;
    Len rowGap;
    Len paddingX;
    Len paddingY;
    std::optional<double> cellRatio;
    Len cellExtra;
};

inline void Grid(Parent const& p, Ctx const& ctx, GridProps props, Build const& children) {
    auto impl = winrt::make_self<GridPanel>();
    impl->spacing = 8;
    impl->reflow = props.reflow;
    controls::Panel panel = impl.as<controls::Panel>();
    SetRole(panel, L"bern-fill-width");
    GridPanel* grid = impl.get();
    KeepReflow(panel, [grid](Motion const* motion) { grid->reflow = motion; });
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
    children(Parent::Of(panel), ctx);
    Place(p, panel);
}

struct LayersProps {
    char const* align = nullptr;
    Len width;
    Len height;
    Motion const* animate = nullptr;
    Motion const* reflow = nullptr;
    /// A style's surface: background, the color of text and icons, and
    /// corners that clip the layers.
    StyleDef const* style = nullptr;
    /// `enabled: false`: the layers take no input.
    Get enabled;
    /// `under_bars`: while shown, the layers fill the window up to its top
    /// edge, under the title bar and the header.
    bool underBars = false;
};

/// `Stack`: layers in the one cell of a grid, later ones above earlier
/// ones, as large as the largest; what fills stretches it.
inline void Layers(Parent const& p, Ctx const& ctx, LayersProps props, Build const& children) {
    std::string_view align = props.align ? props.align : "center";
    auto h = align.ends_with("leading") ? xaml::HorizontalAlignment::Left
        : align.ends_with("trailing") ? xaml::HorizontalAlignment::Right
        : xaml::HorizontalAlignment::Center;
    auto v = align.starts_with("top") ? xaml::VerticalAlignment::Top
        : align.starts_with("bottom") ? xaml::VerticalAlignment::Bottom
        : xaml::VerticalAlignment::Center;
    controls::Grid layers;
    SetRole(layers, L"bern-layers");
    // Where a column would stretch the stack, it keeps its size at the
    // leading edge, unless something in it fills.
    layers.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    layers.VerticalAlignment(xaml::VerticalAlignment::Top);
    BindLen(ctx, props.width, props.animate, [layers](double w) { layers.Width((std::max)(0.0, w)); });
    BindLen(ctx, props.height, props.animate, [layers](double w) { layers.Height((std::max)(0.0, w)); });
    if (props.underBars && ctx.win) {
        // It fills the window both ways, so its containers give it all
        // their room.
        SetRole(layers, L"bern-layers-wh");
        layers.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
        layers.VerticalAlignment(xaml::VerticalAlignment::Stretch);
        Win* win = ctx.win;
        win->underBars.Set(win->underBars.Peek() + 1);
        Reactive::OnCleanup([win] { win->underBars.Set(win->underBars.Peek() - 1); });
    }
    children(Parent::Aligned(layers, h, v), ctx);
    // Layers that are all backdrops fill where they stand, as one alone
    // would.
    if (layers.Children().Size() > 0 && !props.width.Given() && !props.height.Given()) {
        bool backdrops = true;
        for (auto child : layers.Children()) backdrops = backdrops && Backdrop(child);
        if (backdrops) {
            SetRole(layers, L"bern-layers-wh");
            layers.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
            layers.VerticalAlignment(xaml::VerticalAlignment::Stretch);
        }
    }
    // With a backdrop beside other layers and no width of its own, the
    // stack fills across a column, as a block does, and keeps the height
    // the other layers give: a row shares its room by roles, not by this.
    if (!props.width.Given()) {
        bool backdrop = false, others = false;
        for (auto child : layers.Children()) {
            bool canvas = RoleOf(Unwrapped(child)) == L"bern-canvas";
            (canvas ? backdrop : others) = true;
        }
        if (backdrop && others) layers.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    }
    if (!props.style && !props.enabled) {
        Place(p, layers);
        return;
    }
    // The text and icons take the style's color, and controls are disabled
    // with their holder.
    controls::ContentControl holder;
    holder.HorizontalContentAlignment(xaml::HorizontalAlignment::Stretch);
    holder.VerticalContentAlignment(xaml::VerticalAlignment::Stretch);
    holder.IsTabStop(false);
    holder.Content(layers);
    holder.HorizontalAlignment(layers.HorizontalAlignment());
    holder.VerticalAlignment(layers.VerticalAlignment());
    Mirror(holder, layers);
    if (auto style = props.style) {
        if (auto const& background = style->base.background)
            PaintWith<controls::Grid>(layers, L"Grid", L"Background", *background, [](controls::Grid const& e, media::Brush const& b) { e.Background(b); });
        if (auto const& foreground = style->base.foreground)
            PaintWith<controls::ContentControl>(holder, L"ContentControl", L"Foreground", *foreground,
                [](controls::ContentControl const& e, media::Brush const& b) { e.Foreground(b); });
        if (style->corner) {
            double r = *style->corner;
            layers.CornerRadius(xaml::CornerRadius{r, r, r, r});
            RoundedClip(layers, [r](float, float) { return static_cast<float>(r); });
        }
    }
    if (auto isEnabled = Enabled(ctx, props.enabled)) Reactive::Run([holder, isEnabled] { holder.IsEnabled(isEnabled()); });
    Place(p, holder);
}

/// The bytes of a declared resource, from `resources` next to the
/// executable; empty when it cannot be read.
inline std::vector<uint8_t> ResourceBytes(std::string_view name) {
    std::wstring path(MAX_PATH, L'\0');
    for (;;) {
        auto length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
        if (length < path.size()) {
            path.resize(length);
            break;
        }
        path.resize(path.size() * 2);
    }
    std::wstring file = path.substr(0, path.find_last_of(L'\\')) + L"\\resources\\" + std::wstring(Wide(name));
    std::replace(file.begin(), file.end(), L'/', L'\\');
    std::vector<uint8_t> bytes;
    HANDLE handle = CreateFileW(file.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (handle == INVALID_HANDLE_VALUE) return bytes;
    LARGE_INTEGER size{};
    if (GetFileSizeEx(handle, &size) && size.QuadPart > 0 && size.QuadPart < (1LL << 30)) {
        bytes.resize(static_cast<size_t>(size.QuadPart));
        DWORD read = 0;
        if (!ReadFile(handle, bytes.data(), static_cast<DWORD>(bytes.size()), &read, nullptr) || read != bytes.size()) bytes.clear();
    }
    CloseHandle(handle);
    return bytes;
}

/// Decode `bytes` into `bitmap`, back on the UI thread.
inline winrt::fire_and_forget LoadPicture(media::Imaging::BitmapImage bitmap, std::vector<uint8_t> bytes, std::string name) {
    winrt::apartment_context ui;
    try {
        winrt::Windows::Storage::Streams::InMemoryRandomAccessStream stream;
        winrt::Windows::Storage::Streams::DataWriter writer(stream);
        writer.WriteBytes(bytes);
        co_await writer.StoreAsync();
        writer.DetachStream();
        stream.Seek(0);
        co_await ui;
        co_await bitmap.SetSourceAsync(stream);
    } catch (winrt::hresult_error const& error) {
        Report("image " + name + ": " + Utf8(error.message()));
    }
}

/// A picture's frame and fit, with the sizes it has now.
struct PictureProps {
    char const* source = "";
    Str label;
    std::optional<double> width;
    std::optional<double> height;
    char const* fit = nullptr;
    /// The picture fills a frame its container sizes, as one with only an
    /// `aspect` does.
    bool fills = false;
};

/// The XAML picture: SDR pictures, and HDR ones where they cannot be drawn
/// as HDR. `framed` when both sides are given.
inline controls::Image SdrImage(PictureProps const& props, std::vector<uint8_t> bytes, TextFn const& name, std::optional<std::string> uri = std::nullopt) {
    controls::Image image;
    std::string_view fit = props.fit ? props.fit : "contain";
    bool framed = props.fills || (props.width && props.height);
    image.Stretch(!framed || fit == "contain" ? media::Stretch::Uniform
        : fit == "cover" ? media::Stretch::UniformToFill
        : media::Stretch::Fill);
    if (framed) {
        image.HorizontalAlignment(xaml::HorizontalAlignment::Center);
        image.VerticalAlignment(xaml::VerticalAlignment::Center);
    } else {
        if (props.width) image.Width(*props.width);
        if (props.height) image.Height(*props.height);
        image.HorizontalAlignment(xaml::HorizontalAlignment::Left);
        image.VerticalAlignment(xaml::VerticalAlignment::Top);
    }
    media::Imaging::BitmapImage bitmap;
    image.Source(bitmap);
    if (uri) bitmap.UriSource(winrt::Windows::Foundation::Uri(Wide(*uri)));
    else if (!bytes.empty()) LoadPicture(bitmap, std::move(bytes), props.source);
    if (name) BindName(image, name);
    else xaml::Automation::AutomationProperties::SetAccessibilityView(image, xaml::Automation::Peers::AccessibilityView::Raw);
    return image;
}

// ---------------------------------------------------------------------------
// HDR pictures. XAML's Image shows SDR only, so a PQ or HLG PNG or a JPEG
// with a gain map is decoded by Rust (`bern_picture_decode`) and drawn by
// Direct2D into an scRGB swap chain in a SwapChainPanel, which the system
// compositor shows in HDR. Light is fitted to the display: SDR white at the
// user's SDR brightness, highlights tone mapped to the display's peak, and
// on an SDR display a gain map gives the SDR picture its author made.

namespace Hdr {

using SwapChainPanelNative = std::conditional_t<kWinUI, IBernWinUISwapChainPanelNative, IBernIslandsSwapChainPanelNative>;

/// What a display shows: whether it is in HDR, its SDR white and its peak
/// in nits.
struct Display {
    bool hdr = false;
    float sdrWhite = 80;
    float peak = 80;
    bool operator==(Display const&) const = default;
    /// Peak over SDR white: how much brighter than white it can show.
    float Headroom() const { return hdr ? (std::max)(1.0f, peak / sdrWhite) : 1.0f; }
};

/// The display `hwnd` is on, as DXGI and the display configuration
/// report it now.
inline Display QueryDisplay(HWND hwnd) {
    Display display;
    HMONITOR monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
    winrt::com_ptr<IDXGIFactory1> factory;
    if (FAILED(CreateDXGIFactory1(IID_PPV_ARGS(factory.put())))) return display;
    for (UINT a = 0;; ++a) {
        winrt::com_ptr<IDXGIAdapter1> adapter;
        if (factory->EnumAdapters1(a, adapter.put()) != S_OK) break;
        for (UINT o = 0;; ++o) {
            winrt::com_ptr<IDXGIOutput> output;
            if (adapter->EnumOutputs(o, output.put()) != S_OK) break;
            auto output6 = output.try_as<IDXGIOutput6>();
            DXGI_OUTPUT_DESC1 desc{};
            if (!output6 || FAILED(output6->GetDesc1(&desc)) || desc.Monitor != monitor) continue;
            display.hdr = desc.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020;
            if (display.hdr) display.peak = desc.MaxLuminance > 0 ? desc.MaxLuminance : 1000.0f;
        }
    }
    if (!display.hdr) return display;
    // The SDR brightness the user chose for this display.
    MONITORINFOEXW info{};
    info.cbSize = sizeof(info);
    GetMonitorInfoW(monitor, &info);
    UINT32 paths = 0, modes = 0;
    if (GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &paths, &modes) != ERROR_SUCCESS) return display;
    std::vector<DISPLAYCONFIG_PATH_INFO> pathInfo(paths);
    std::vector<DISPLAYCONFIG_MODE_INFO> modeInfo(modes);
    if (QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &paths, pathInfo.data(), &modes, modeInfo.data(), nullptr) != ERROR_SUCCESS)
        return display;
    for (UINT32 i = 0; i < paths; ++i) {
        DISPLAYCONFIG_SOURCE_DEVICE_NAME source{};
        source.header = {DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, sizeof(source), pathInfo[i].sourceInfo.adapterId, pathInfo[i].sourceInfo.id};
        if (DisplayConfigGetDeviceInfo(&source.header) != ERROR_SUCCESS || wcscmp(source.viewGdiDeviceName, info.szDevice) != 0) continue;
        DISPLAYCONFIG_SDR_WHITE_LEVEL white{};
        white.header = {DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, sizeof(white), pathInfo[i].targetInfo.adapterId, pathInfo[i].targetInfo.id};
        if (DisplayConfigGetDeviceInfo(&white.header) == ERROR_SUCCESS && white.SDRWhiteLevel > 0)
            display.sdrWhite = white.SDRWhiteLevel * 80.0f / 1000.0f;
        break;
    }
    display.peak = (std::max)(display.peak, display.sdrWhite);
    return display;
}

/// The Direct3D and Direct2D devices every HDR picture draws with.
struct Gpu {
    winrt::com_ptr<ID3D11Device> d3d;
    winrt::com_ptr<IDXGIFactory2> factory;
    winrt::com_ptr<ID2D1DeviceContext> context;
};

inline std::shared_ptr<Gpu>& CurrentGpu() {
    static std::shared_ptr<Gpu> gpu;
    return gpu;
}

/// The devices, made on first use; null when Direct3D is unavailable.
inline std::shared_ptr<Gpu> GetGpu() {
    auto& gpu = CurrentGpu();
    if (gpu) return gpu;
    auto made = std::make_shared<Gpu>();
    UINT flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    HRESULT hr = D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, flags, nullptr, 0, D3D11_SDK_VERSION, made->d3d.put(), nullptr, nullptr);
    if (FAILED(hr)) {
        made->d3d = nullptr;
        hr = D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_WARP, nullptr, flags, nullptr, 0, D3D11_SDK_VERSION, made->d3d.put(), nullptr, nullptr);
    }
    if (FAILED(hr)) return nullptr;
    auto dxgiDevice = made->d3d.as<IDXGIDevice>();
    winrt::com_ptr<IDXGIAdapter> adapter;
    if (FAILED(dxgiDevice->GetAdapter(adapter.put())) || FAILED(adapter->GetParent(IID_PPV_ARGS(made->factory.put())))) return nullptr;
    winrt::com_ptr<ID2D1Factory1> factory;
    D2D1_FACTORY_OPTIONS options{};
    if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, __uuidof(ID2D1Factory1), &options, factory.put_void()))) return nullptr;
    winrt::com_ptr<ID2D1Device> device;
    if (FAILED(factory->CreateDevice(dxgiDevice.get(), device.put()))) return nullptr;
    if (FAILED(device->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE, made->context.put()))) return nullptr;
    gpu = made;
    return gpu;
}

/// One HDR picture on screen.
struct View : std::enable_shared_from_this<View> {
    controls::SwapChainPanel panel;
    controls::Grid frame{nullptr};
    Win* win = nullptr;
    std::shared_ptr<std::vector<uint8_t>> bytes;
    PictureProps props;
    TextFn name;
    bool gainMap = false;
    std::shared_ptr<Gpu> gpu;
    winrt::com_ptr<ID2D1Bitmap1> bitmap;
    uint32_t width = 0, height = 0;
    float peak = 1;
    /// The headroom the gain map was applied for; negative before decoding.
    float decodedFor = -1;
    bool decoding = false;
    Display display;
    winrt::com_ptr<IDXGISwapChain1> chain;
    UINT chainWidth = 0, chainHeight = 0;
    /// Whether the swap chain shows the current bitmap at its size.
    bool rendered = false;

    HWND Window() const { return win ? win->hwnd : nullptr; }

    /// Size the panel from the picture's aspect ratio: the fitted rectangle
    /// for `contain` in a frame, the frame for `cover` and `fill`, the
    /// missing side with one given.
    void Layout() {
        if (!width || !height) return;
        double aspect = double(width) / height;
        std::string_view fit = props.fit ? props.fit : "contain";
        if (props.width && props.height) {
            if (fit == "contain") {
                double w = *props.width, h = *props.height;
                if (w / h > aspect) w = h * aspect;
                else h = w / aspect;
                panel.Width(w);
                panel.Height(h);
            } else {
                panel.Width(*props.width);
                panel.Height(*props.height);
            }
        } else if (props.width) {
            panel.Width(*props.width);
            panel.Height(*props.width / aspect);
        } else if (props.height) {
            panel.Height(*props.height);
            panel.Width(*props.height * aspect);
        }
    }

    /// Show the XAML picture instead, when this one cannot be drawn.
    void FallBack() {
        frame.Children().Clear();
        frame.Children().Append(SdrImage(props, *bytes, name));
        chain = nullptr;
        bitmap = nullptr;
    }

    /// Take a decoded picture: premultiply it and make it a Direct2D bitmap.
    void Upload(BernPicture& picture, float headroom) {
        gpu = GetGpu();
        if (!gpu) return FallBack();
        for (size_t i = 0; i + 3 < picture.len; i += 4) {
            float alpha = picture.pixels[i + 3];
            picture.pixels[i] *= alpha;
            picture.pixels[i + 1] *= alpha;
            picture.pixels[i + 2] *= alpha;
        }
        auto properties = D2D1::BitmapProperties1(D2D1_BITMAP_OPTIONS_NONE,
            D2D1::PixelFormat(DXGI_FORMAT_R32G32B32A32_FLOAT, D2D1_ALPHA_MODE_PREMULTIPLIED));
        winrt::com_ptr<ID2D1Bitmap1> made;
        if (FAILED(gpu->context->CreateBitmap(D2D1::SizeU(picture.width, picture.height), picture.pixels, picture.width * 16, properties, made.put()))) {
            Report(std::string("HDR picture ") + props.source + ": too large for the GPU");
            return FallBack();
        }
        bitmap = made;
        width = picture.width;
        height = picture.height;
        peak = picture.peak;
        decodedFor = headroom;
        Layout();
        Render();
    }

    /// Draw the picture for the display it is on now.
    void Render() {
        if (!bitmap || !gpu) return;
        auto now = QueryDisplay(Window());
        bool changed = !(now == display);
        display = now;
        // A gain map applies as much gain as the display has headroom for.
        if (gainMap && !decoding && std::abs(display.Headroom() - decodedFor) > 0.05f) Decode(shared_from_this(), display.Headroom());
        float scaleX = panel.CompositionScaleX(), scaleY = panel.CompositionScaleY();
        double w = panel.ActualWidth(), h = panel.ActualHeight();
        if (w <= 0 || h <= 0) return;
        UINT pw = (std::max)(1L, std::lround(w * scaleX)), ph = (std::max)(1L, std::lround(h * scaleY));
        if (!chain) {
            DXGI_SWAP_CHAIN_DESC1 desc{};
            desc.Width = pw;
            desc.Height = ph;
            desc.Format = DXGI_FORMAT_R16G16B16A16_FLOAT;
            desc.SampleDesc.Count = 1;
            desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
            desc.BufferCount = 2;
            desc.Scaling = DXGI_SCALING_STRETCH;
            desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
            desc.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;
            if (FAILED(gpu->factory->CreateSwapChainForComposition(gpu->d3d.get(), &desc, nullptr, chain.put()))) return FallBack();
            if (auto chain3 = chain.try_as<IDXGISwapChain3>()) chain3->SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709);
            if (FAILED(panel.as<SwapChainPanelNative>()->SetSwapChain(chain.get()))) return FallBack();
            chainWidth = pw;
            chainHeight = ph;
        } else if (pw != chainWidth || ph != chainHeight) {
            if (FAILED(chain->ResizeBuffers(0, pw, ph, DXGI_FORMAT_UNKNOWN, 0))) return FallBack();
            chainWidth = pw;
            chainHeight = ph;
        } else if (!changed && w > 0 && rendered) {
            return;
        }
        if (auto chain2 = chain.try_as<IDXGISwapChain2>()) {
            DXGI_MATRIX_3X2_F inverse{1.0f / scaleX, 0, 0, 1.0f / scaleY, 0, 0};
            chain2->SetMatrixTransform(&inverse);
        }

        auto context = gpu->context;
        winrt::com_ptr<IDXGISurface> surface;
        if (FAILED(chain->GetBuffer(0, IID_PPV_ARGS(surface.put())))) return;
        auto targetProperties = D2D1::BitmapProperties1(D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            D2D1::PixelFormat(DXGI_FORMAT_R16G16B16A16_FLOAT, D2D1_ALPHA_MODE_PREMULTIPLIED));
        winrt::com_ptr<ID2D1Bitmap1> target;
        if (FAILED(context->CreateBitmapFromDxgiSurface(surface.get(), &targetProperties, target.put()))) return;

        // scRGB is 80 nits at 1.0; the picture's 1.0 is SDR white.
        float sdrWhite = display.hdr ? display.sdrWhite : 80.0f;
        winrt::com_ptr<ID2D1Effect> white, tone;
        if (FAILED(context->CreateEffect(CLSID_D2D1WhiteLevelAdjustment, white.put())) || FAILED(context->CreateEffect(CLSID_D2D1HdrToneMap, tone.put())))
            return FallBack();
        white->SetInput(0, bitmap.get());
        white->SetValue(D2D1_WHITELEVELADJUSTMENT_PROP_INPUT_WHITE_LEVEL, 80.0f);
        white->SetValue(D2D1_WHITELEVELADJUSTMENT_PROP_OUTPUT_WHITE_LEVEL, sdrWhite);
        tone->SetInputEffect(0, white.get());
        tone->SetValue(D2D1_HDRTONEMAP_PROP_INPUT_MAX_LUMINANCE, peak * sdrWhite);
        tone->SetValue(D2D1_HDRTONEMAP_PROP_OUTPUT_MAX_LUMINANCE, display.hdr ? display.peak : 80.0f);
        tone->SetValue(D2D1_HDRTONEMAP_PROP_DISPLAY_MODE, display.hdr ? D2D1_HDRTONEMAP_DISPLAY_MODE_HDR : D2D1_HDRTONEMAP_DISPLAY_MODE_SDR);
        winrt::com_ptr<ID2D1Image> output;
        tone->GetOutput(output.put());

        // `cover` crops the picture to the panel; otherwise the panel has
        // the picture's shape or is stretched by `fill`.
        std::string_view fit = props.fit ? props.fit : "contain";
        float sx = float(pw) / width, sy = float(ph) / height;
        D2D1_RECT_F source{0, 0, float(width), float(height)};
        if (props.width && props.height && fit == "cover") {
            float s = (std::max)(sx, sy);
            float cw = pw / s, ch = ph / s;
            source = {(width - cw) / 2, (height - ch) / 2, (width + cw) / 2, (height + ch) / 2};
            sx = sy = s;
        }
        context->SetTarget(target.get());
        context->SetDpi(96, 96);
        context->BeginDraw();
        context->Clear(D2D1::ColorF(0, 0, 0, 0));
        context->SetTransform(D2D1::Matrix3x2F::Scale(sx, sy));
        context->DrawImage(output.get(), D2D1::Point2F(0, 0), source, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC);
        context->SetTransform(D2D1::Matrix3x2F::Identity());
        HRESULT drawn = context->EndDraw();
        context->SetTarget(nullptr);
        HRESULT presented = SUCCEEDED(drawn) ? chain->Present(1, 0) : drawn;
        rendered = SUCCEEDED(presented);
        if (presented == D2DERR_RECREATE_TARGET || presented == DXGI_ERROR_DEVICE_REMOVED || presented == DXGI_ERROR_DEVICE_RESET) {
            // The GPU went away: start over with new devices.
            CurrentGpu() = nullptr;
            chain = nullptr;
            bitmap = nullptr;
            Decode(shared_from_this(), display.Headroom());
        }
    }

    /// Decode on a background thread, then upload on this one.
    static winrt::fire_and_forget Decode(std::shared_ptr<View> view, float headroom) {
        winrt::apartment_context ui;
        view->decoding = true;
        auto bytes = view->bytes;
        float applied = view->gainMap ? headroom : 0.0f;
        co_await winrt::resume_background();
        BernPicture picture{};
        bool decoded = bern_picture_decode(bytes->data(), bytes->size(), applied, &picture);
        co_await ui;
        view->decoding = false;
        if (!decoded) {
            Report(std::string("HDR picture ") + view->props.source + ": does not decode");
            view->FallBack();
            co_return;
        }
        view->rendered = false;
        view->Upload(picture, headroom);
        bern_picture_free(picture);
    }
};

/// Every HDR picture on screen, for the display check.
inline std::map<int64_t, std::shared_ptr<View>> views;
inline int64_t nextView = 1;
inline UINT_PTR timer = 0;

/// Once a second, while HDR pictures are on screen, redraw those whose
/// display changed: HDR switched on or off, SDR brightness changed, or the
/// window moved to another display.
inline void CALLBACK Tick(HWND, UINT, UINT_PTR, DWORD) {
    std::map<HWND, Display> seen;
    for (auto& [token, view] : views) {
        auto hwnd = view->Window();
        auto found = seen.find(hwnd);
        if (found == seen.end()) found = seen.emplace(hwnd, QueryDisplay(hwnd)).first;
        if (!(found->second == view->display)) view->Render();
    }
}

/// Put an HDR picture in `frame`, falling back to the XAML picture when it
/// cannot be drawn.
inline void Show(controls::Grid const& frame, Ctx const& ctx, std::shared_ptr<std::vector<uint8_t>> bytes, PictureProps const& props, TextFn name, bool gainMap) {
    auto view = std::make_shared<View>();
    view->frame = frame;
    view->win = ctx.win;
    view->bytes = std::move(bytes);
    view->props = props;
    view->name = name;
    view->gainMap = gainMap;
    std::string_view fit = props.fit ? props.fit : "contain";
    bool framed = props.width && props.height;
    bool centered = framed && fit == "contain";
    view->panel.HorizontalAlignment(centered ? xaml::HorizontalAlignment::Center : framed ? xaml::HorizontalAlignment::Stretch : xaml::HorizontalAlignment::Left);
    view->panel.VerticalAlignment(centered ? xaml::VerticalAlignment::Center : framed ? xaml::VerticalAlignment::Stretch : xaml::VerticalAlignment::Top);
    if (name) BindName(view->panel, name);
    else xaml::Automation::AutomationProperties::SetAccessibilityView(view->panel, xaml::Automation::Peers::AccessibilityView::Raw);
    frame.Children().Append(view->panel);
    std::weak_ptr<View> weak = view;
    view->panel.SizeChanged([weak](auto&&, auto&&) {
        if (auto view = weak.lock()) view->Render();
    });
    view->panel.CompositionScaleChanged([weak](auto&&, auto&&) {
        if (auto view = weak.lock()) view->Render();
    });
    auto token = nextView++;
    views[token] = view;
    if (!timer) timer = SetTimer(nullptr, 0, 1000, Tick);
    Reactive::OnCleanup([token] {
        views.erase(token);
        if (views.empty() && timer) {
            KillTimer(nullptr, timer);
            timer = 0;
        }
    });
    view->display = QueryDisplay(view->Window());
    View::Decode(view, view->display.Headroom());
}

}

/// The bytes of the file at `path`; empty when it cannot be read.
inline std::vector<uint8_t> FileBytes(std::wstring const& path) {
    std::vector<uint8_t> bytes;
    HANDLE handle = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (handle == INVALID_HANDLE_VALUE) return bytes;
    LARGE_INTEGER size{};
    if (GetFileSizeEx(handle, &size) && size.QuadPart > 0 && size.QuadPart < (1LL << 30)) {
        bytes.resize(static_cast<size_t>(size.QuadPart));
        DWORD read = 0;
        if (!ReadFile(handle, bytes.data(), static_cast<DWORD>(bytes.size()), &read, nullptr) || read != bytes.size()) bytes.clear();
    }
    CloseHandle(handle);
    return bytes;
}

struct ImageProps {
    /// A declared resource, by name.
    char const* source = nullptr;
    /// Or a `MediaSource` the engine gives: a file or an http(s) URL.
    Get url;
    Str label;
    Len width;
    Len height;
    char const* fit = nullptr;
    Motion const* animate = nullptr;
    /// Rounded corners, following state with `animate`.
    Len corner;
    /// `rendering: template`: the picture's alpha in the color of the text
    /// around it.
    bool templated = false;
    /// The picture's proportion in place of its own; alone, the picture
    /// takes its container's width.
    std::optional<double> aspect;
};

/// A frame as wide as its container and `aspect` as tall.
struct AspectPanel : controls::PanelT<AspectPanel> {
    double aspect = 1;
    Size MeasureOverride(Size available) {
        float width = std::isinf(available.Width) ? 0 : available.Width;
        float height = static_cast<float>(width / (std::max)(0.01, aspect));
        for (auto child : Children()) child.Measure(Size{width, height});
        return Size{width, height};
    }
    Size ArrangeOverride(Size final) {
        for (auto child : Children()) child.Arrange(Rect{0, 0, final.Width, final.Height});
        return final;
    }
};

/// The full path of a declared resource next to the executable.
inline std::wstring ResourcePath(std::string_view name) {
    std::wstring path(MAX_PATH, L'\0');
    for (;;) {
        auto length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
        if (length < path.size()) {
            path.resize(length);
            break;
        }
        path.resize(path.size() * 2);
    }
    std::wstring file = path.substr(0, path.find_last_of(L'\\')) + L"\\resources\\" + std::wstring(Wide(name));
    std::replace(file.begin(), file.end(), L'/', L'\\');
    return file;
}

/// Show encoded picture bytes in `frame`: HDR where the display shows it.
inline void ShowPicture(controls::Grid const& frame, Ctx const& ctx, std::shared_ptr<std::vector<uint8_t>> bytes, PictureProps const& picture, TextFn name) {
    uint32_t kind = bytes->empty() ? 0 : bern_picture_kind(bytes->data(), bytes->size());
    if (kind != 0 && Hdr::GetGpu()) Hdr::Show(frame, ctx, bytes, picture, name, kind == 3);
    else frame.Children().Append(SdrImage(picture, *bytes, name));
}

/// A scope that ends with the current one, for what shows later.
inline std::shared_ptr<Scope> LaterScope() {
    auto scope = std::make_shared<Scope>();
    Reactive::OnCleanup([scope] { scope->Dispose(); });
    return scope;
}

/// Pictures read lately, by file or resource, so that a page showing them
/// again shows them at once; at most 64 MB are kept, the least lately
/// used going first.
namespace Recent {
inline std::vector<std::pair<std::wstring, std::shared_ptr<std::vector<uint8_t>>>> kept;
inline size_t size = 0;
inline std::shared_ptr<std::vector<uint8_t>> Find(std::wstring const& key) {
    for (size_t at = 0; at < kept.size(); ++at)
        if (kept[at].first == key) {
            auto found = kept[at];
            kept.erase(kept.begin() + static_cast<ptrdiff_t>(at));
            kept.push_back(found);
            return found.second;
        }
    return nullptr;
}
inline void Keep(std::wstring const& key, std::shared_ptr<std::vector<uint8_t>> const& bytes) {
    if (bytes->empty() || bytes->size() > (16u << 20) || Find(key)) return;
    kept.emplace_back(key, bytes);
    size += bytes->size();
    while (size > (64u << 20) && !kept.empty()) {
        size -= kept.front().second->size();
        kept.erase(kept.begin());
    }
}
}

/// Read a picture's bytes away from the UI thread, then show it in the
/// frame within `owner`, unless that has ended meanwhile (the frame went
/// away or now shows another picture). Reading a file can wait on the disk
/// or on a virus scanner, which must not hold the window still while a
/// page builds.
inline winrt::fire_and_forget ShowLater(winrt::weak_ref<controls::Grid> weak, Ctx ctx, std::wstring key, std::function<std::vector<uint8_t>()> read,
    std::string label, PictureProps picture, TextFn name, std::shared_ptr<Scope> owner) {
    winrt::apartment_context ui;
    co_await winrt::resume_background();
    auto bytes = std::make_shared<std::vector<uint8_t>>(read());
    co_await ui;
    Recent::Keep(key, bytes);
    auto frame = weak.get();
    if (!frame || owner->disposed) co_return;
    if (bytes->empty()) Report("image " + label + ": the picture cannot be read");
    owner->Run([&] { ShowPicture(frame, ctx, bytes, picture, name); });
}

/// Show the picture `key` names: at once when it was read lately, else once
/// it has been read.
inline void ShowRead(controls::Grid const& frame, Ctx const& ctx, std::wstring key, std::function<std::vector<uint8_t>()> read, std::string label,
    PictureProps const& picture, TextFn const& name) {
    if (auto bytes = Recent::Find(key)) {
        ShowPicture(frame, ctx, bytes, picture, name);
        return;
    }
    ShowLater(winrt::make_weak(frame), ctx, std::move(key), std::move(read), std::move(label), picture, name, LaterScope());
}

/// A picture from a declared resource, or from a `MediaSource` the engine
/// gives. With one side given, the other follows the picture's aspect
/// ratio. Without a label it is decorative. HDR pictures are drawn in HDR
/// where the display shows it.
inline void Image(Parent const& p, Ctx const& ctx, ImageProps props) {
    PictureProps picture;
    picture.source = props.source ? props.source : "";
    picture.label = props.label;
    picture.fit = props.fit;
    if (props.width.Given()) picture.width = Reactive::Untrack([&] { return props.width.Now(ctx); });
    if (props.height.Given()) picture.height = Reactive::Untrack([&] { return props.height.Now(ctx); });
    // A proportion of its own sets the side not given.
    Len width = props.width, height = props.height;
    if (props.aspect && *props.aspect > 0) {
        double aspect = *props.aspect;
        if (picture.width && !picture.height) {
            picture.height = *picture.width / aspect;
            height = Len(*picture.height);
        } else if (picture.height && !picture.width) {
            picture.width = *picture.height * aspect;
            width = Len(*picture.width);
        } else if (!picture.width && !picture.height) {
            picture.fills = true;
        }
    }
    props.width = width;
    props.height = height;
    // A template: the alpha of a declared picture in the text's color.
    if (props.templated && props.source) {
        controls::BitmapIcon icon;
        icon.ShowAsMonochrome(true);
        icon.UriSource(winrt::Windows::Foundation::Uri(winrt::hstring(L"file:///" + ResourcePath(props.source))));
        BindLen(ctx, props.width, props.animate, [icon](double v) { icon.Width((std::max)(0.0, v)); });
        BindLen(ctx, props.height, props.animate, [icon](double v) { icon.Height((std::max)(0.0, v)); });
        icon.HorizontalAlignment(xaml::HorizontalAlignment::Left);
        icon.VerticalAlignment(xaml::VerticalAlignment::Top);
        if (auto name = props.label ? Bound(ctx, props.label) : nullptr) BindName(icon, name);
        else xaml::Automation::AutomationProperties::SetAccessibilityView(icon, xaml::Automation::Peers::AccessibilityView::Raw);
        Place(p, icon);
        return;
    }
    // The declared frame. With both sides, the picture fits inside it and
    // is centered when it keeps its aspect ratio; with one, the picture
    // sets the other.
    controls::Grid frame;
    frame.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    frame.VerticalAlignment(xaml::VerticalAlignment::Top);
    if (picture.width && picture.height) {
        BindLen(ctx, props.width, props.animate, [frame](double v) { frame.Width((std::max)(0.0, v)); });
        BindLen(ctx, props.height, props.animate, [frame](double v) { frame.Height((std::max)(0.0, v)); });
    }
    if (picture.fills) {
        frame.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
        frame.VerticalAlignment(xaml::VerticalAlignment::Stretch);
    }
    // Rounded corners clip the picture, following state.
    if (props.corner.Given()) {
        auto radius = std::make_shared<float>(0.0f);
        auto geometry = RoundedClip(frame, [radius](float, float) { return *radius; });
        BindLen(ctx, props.corner, props.animate, [geometry, radius](double r) {
            *radius = static_cast<float>((std::max)(0.0, r));
            geometry.CornerRadius(float2{*radius, *radius});
        });
    }
    TextFn name = props.label ? Bound(ctx, props.label) : nullptr;
    if (props.source) {
        std::string source = props.source;
        ShowRead(frame, ctx, L"resource:" + std::wstring(Wide(source)), [source] { return ResourceBytes(source); }, source, picture, name);
    } else if (props.url) {
        auto url = props.url;
        auto shown = std::make_shared<std::optional<std::string>>();
        Reactive::Run([frame, url, ctx, picture, name, shown] {
            auto next = R::Url(url(ctx));
            if (*shown == next && frame.Children().Size() > 0) return;
            *shown = next;
            Reactive::Untrack([&] {
                frame.Children().Clear();
                frame.Visibility(next ? xaml::Visibility::Visible : xaml::Visibility::Collapsed);
                if (!next) return 0;
                if (auto path = Host::FilePath(*next)) {
                    std::wstring file = *path;
                    ShowRead(frame, ctx, file, [file] { return FileBytes(file); }, *next, picture, name);
                } else {
                    frame.Children().Append(SdrImage(picture, {}, name, *next));
                }
                return 0;
            });
        });
    }
    // A picture with only a proportion takes its container's width.
    if (picture.fills) {
        auto impl = winrt::make_self<AspectPanel>();
        impl->aspect = *props.aspect;
        controls::Panel panel = impl.as<controls::Panel>();
        SetRole(panel, L"bern-fill-width");
        panel.Children().Append(frame);
        Place(p, panel);
        return;
    }
    // Where a column would stretch the picture, it keeps its size at the
    // leading edge.
    controls::Grid holder;
    holder.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    holder.Children().Append(frame);
    Place(p, holder);
}

struct ProgressProps {
    Get value;
    std::optional<double> total;
    Str label;
};

/// A progress bar of `value` out of `total` under its label, or without a
/// value a progress ring beside it.
inline void Progress(Parent const& p, Ctx const& ctx, ProgressProps props) {
    auto label = Bound(ctx, props.label);
    if (props.value) {
        double total = props.total.value_or(1);
        controls::ProgressBar bar;
        bar.Minimum(0);
        bar.Maximum(total);
        Reactive::Run([bar, ctx, value = props.value, total] {
            double current = R::AsDouble(value(ctx));
            bar.Value(current > 0 ? (std::min)(current, total) : 0);
        });
        BindName(bar, label);
        Place(p, Stacked(label, bar));
        return;
    }
    controls::ProgressRing ring;
    ring.IsActive(true);
    ring.Width(20);
    ring.Height(20);
    BindName(ring, label);
    controls::StackPanel row;
    row.Orientation(controls::Orientation::Horizontal);
    row.Spacing(8);
    row.Children().Append(ring);
    if (label) {
        auto text = TextOf(label);
        Secondary(text);
        text.VerticalAlignment(xaml::VerticalAlignment::Center);
        row.Children().Append(text);
    }
    Place(p, row);
}

inline void Spacer(Parent const& p) { p.Add(MakeSpacer()); }

inline void Divider(Parent const& p, Ctx const& ctx) {
    controls::Border line;
    if (ctx.toolbar) {
        // Between groups of toolbar items.
        line.Width(1);
        line.Height(20);
        line.Margin(xaml::Thickness{6, 0, 6, 0});
        line.VerticalAlignment(xaml::VerticalAlignment::Center);
        if (auto style = Styles::Themed(L"Border", {{L"Background", Keys::Divider}})) line.Style(style);
        p.Add(line);
        return;
    }
    if (p.IsHorizontal()) line.Width(1);
    else {
        // A line across what holds it, in a column or a stack alike.
        line.Height(1);
        SetRole(line, L"bern-fill-width");
    }
    line.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    if (auto style = Styles::Themed(L"Border", {{L"Background", Keys::Divider}})) line.Style(style);
    Place(p, line);
}

/// The layout of a form's sections: the room between them, and the least
/// height and insets of their rows.
struct FormProps {
    std::optional<double> gap;
    std::optional<double> rowHeight;
    std::optional<double> paddingX;
    std::optional<double> paddingY;
};

/// What rows of the sections in a form take from it.
inline std::shared_ptr<FormProps> currentForm;

/// The name a plain section's rows carry, for the form to find.
inline constexpr wchar_t kPlainSection[] = L"bern-plain-section";

/// A grouped form: sections of card rows in a scrolling page with a
/// readable maximum width, as Windows Settings pages lay them out. One
/// opening with a plain section starts at the top of the page, also when
/// a slot, a component or a condition gives that section, or takes it away.
inline void Form(Parent const& p, Ctx const& ctx, Str const& title, FormProps form, Build const& children) {
    if (title && ctx.page) ctx.page->PushTitle(Bound(ctx, title));
    controls::StackPanel content;
    content.Spacing(form.gap.value_or(24));
    content.MaxWidth(1000);
    auto saved = currentForm;
    currentForm = std::make_shared<FormProps>(form);
    controls::ScrollViewer scroller;
    scroller.Content(content);
    scroller.HorizontalScrollBarVisibility(controls::ScrollBarVisibility::Disabled);
    scroller.VerticalScrollBarVisibility(controls::ScrollBarVisibility::Auto);
    MarkFill(scroller);
    children(Parent::Of(content), ctx);
    currentForm = saved;
    auto top = [weak = winrt::make_weak(content)] {
        auto panel = weak.get();
        if (!panel) return;
        bool heading = false;
        if (panel.Children().Size() > 0)
            if (auto first = Unwrapped(panel.Children().GetAt(0)).try_as<xaml::FrameworkElement>())
                heading = first.Name() == kPlainSection;
        xaml::Thickness padding{24, heading ? 0.0 : 8.0, 24, 24};
        if (panel.Padding() != padding) panel.Padding(padding);
    };
    top();
    content.SizeChanged([top](auto&&, auto&&) { top(); });
    p.Add(scroller);
}

struct SectionProps {
    /// Rows on the page itself, without cards: a heading that scrolls with
    /// its form.
    bool plain = false;
    std::optional<double> paddingX;
    std::optional<double> paddingY;
};

/// A titled group of card rows with an optional footnote; the rows take
/// the section's (else the form's) insets and the form's least height. A
/// plain section's rows keep the page's layout, whatever the form gives
/// its groups: only its own insets apply.
inline void Section(Parent const& p, Ctx const& ctx, Str const& title, Str const& footer, SectionProps section, Build const& children) {
    if (section.plain) {
        auto x = section.paddingX.value_or(0), y = section.paddingY.value_or(0);
        auto rows = MakeFlex(false, 0, xaml::Thickness{x, y, x, y});
        rows.Name(kPlainSection);
        children(Parent::Aligned(rows, xaml::HorizontalAlignment::Stretch, std::nullopt), ctx);
        p.Add(rows);
        return;
    }
    auto form = currentForm;
    auto x = section.paddingX ? section.paddingX : form ? form->paddingX : std::nullopt;
    auto y = section.paddingY ? section.paddingY : form ? form->paddingY : std::nullopt;
    auto least = form ? form->rowHeight : std::nullopt;
    controls::StackPanel group;
    group.Spacing(4);
    if (title) {
        auto heading = TextOf(Bound(ctx, title));
        heading.Margin(xaml::Thickness{1, 0, 0, 4});
        if (auto style = Styles::TextStyle("strong")) heading.Style(style);
        group.Children().Append(heading);
    }
    controls::StackPanel rows;
    rows.Spacing(kWinUI ? 4 : 2);
    group.Children().Append(rows);
    if (footer) {
        auto note = TextOf(Bound(ctx, footer));
        note.Margin(xaml::Thickness{1, 4, 0, 0});
        if (auto style = Styles::TextStyle("caption")) note.Style(style);
        Secondary(note);
        group.Children().Append(note);
    }
    children(Parent::OfList(rows), ctx);
    // The rows' insets and least height, from the section or its form,
    // also for rows that come later.
    if (x || y || least) {
        auto apply = [weak = winrt::make_weak(rows), x, y, least] {
            auto panel = weak.get();
            if (!panel) return;
            for (auto child : panel.Children())
                if (auto card = child.try_as<controls::Border>()) {
                    xaml::Thickness padding{x.value_or(16), y.value_or(12), x.value_or(16), y.value_or(12)};
                    if (card.Padding() != padding) card.Padding(padding);
                    double minimum = least ? *least : y ? 0.0 : 56.0;
                    if (card.MinHeight() != minimum) card.MinHeight(minimum);
                }
        };
        apply();
        rows.SizeChanged([apply](auto&&, auto&&) { apply(); });
    }
    p.Add(group);
}

/// Toolbar items for the enclosing page's header.
inline void Toolbar(Parent const&, Ctx const& ctx, Build const& children) {
    if (!ctx.page) return;
    auto holder = ctx.page->AddTools();
    Reactive::Run([holder, ctx] { holder.Visibility(ctx.Shown() ? xaml::Visibility::Visible : xaml::Visibility::Collapsed); });
    children(Parent::Of(holder), ctx.Toolbar());
}

/// Toolbar items before a `Spacer`: at the leading end of the header,
/// after its way back. Buttons there go back or up, as the window's own
/// back does: no background until pointed at.
inline void LeadingToolbar(Parent const&, Ctx const& ctx, Build const& children) {
    if (!ctx.page) return;
    auto holder = ctx.page->AddTools(true);
    if (auto framework = holder.try_as<xaml::FrameworkElement>()) Styles::Restyle(framework, Styles::SubtleXaml());
    Reactive::Run([holder, ctx] { holder.Visibility(ctx.Shown() ? xaml::Visibility::Visible : xaml::Visibility::Collapsed); });
    children(Parent::Of(holder), ctx.Toolbar());
}

inline void LanguagePicker(Parent const& p, Ctx const& ctx, char const* id, Str const& label) {
    auto const& supported = bern::Text::Supported();
    controls::ComboBox combo;
    combo.MinWidth(160);
    for (auto const& tag : supported) combo.Items().Append(winrt::box_value(bern::Text::Autonym(tag)));
    auto text = Bound(ctx, label);
    auto updating = std::make_shared<bool>(false);
    Reactive::Run([combo, updating] {
        auto const& all = bern::Text::Supported();
        auto found = std::find(all.begin(), all.end(), bern::Text::Language());
        int index = found == all.end() ? -1 : static_cast<int>(found - all.begin());
        if (combo.SelectedIndex() != index) {
            *updating = true;
            combo.SelectedIndex(index);
            *updating = false;
        }
    });
    combo.SelectionChanged([weak = winrt::make_weak(combo), updating](auto&&, auto&&) {
        auto combo = weak.get();
        if (!combo || *updating || combo.SelectedIndex() < 0) return;
        auto tag = bern::Text::Supported()[combo.SelectedIndex()];
        // Text bindings update after the selection change finishes.
        Host::Post([tag] { bern::Text::SetLanguage(tag); });
    });
    BindName(combo, text);
    if (p.isList) p.Add(LabeledCard(text, combo));
    else p.Add(Stacked(text, combo));
    Controls::Register(ctx, id, "picker", text, nullptr,
        Actions{.select = [](std::string const& tag) { return bern::Text::SetLanguage(tag); },
            .value = [] { return std::optional<std::string>(bern::Text::PeekLanguage()); }});
}

/// An item of a menu; empty entries are dividers.
struct MenuItemDef {
    char const* id = "";
    Str label;
    char const* icon = nullptr;
    char const* role = nullptr;
    Get enabled;
    Act action;
    std::optional<Field> pick;
    char const* files = nullptr;
    /// Shown beside the item; the shortcut itself is registered on its own.
    char const* shortcut = nullptr;
};

/// A shortcut as Windows writes it: `cmd+shift+k` is Ctrl+Shift+K.
inline winrt::hstring ShortcutText(char const* keys) {
    std::string text, part;
    std::string all = keys ? keys : "";
    std::vector<std::string> parts;
    for (char c : all + "+") {
        if (c == '+') {
            if (!part.empty()) parts.push_back(part);
            part.clear();
        } else {
            part += c;
        }
    }
    for (size_t i = 0; i < parts.size(); ++i) {
        auto const& word = parts[i];
        std::string name = word == "cmd" || word == "ctrl" ? "Ctrl" : word == "alt" ? "Alt" : word == "shift" ? "Shift"
            : word.size() == 1 ? std::string(1, static_cast<char>(std::toupper(static_cast<unsigned char>(word[0]))))
            : std::string(1, static_cast<char>(std::toupper(static_cast<unsigned char>(word[0])))) + word.substr(1);
        if (word == "ctrl" && std::find(parts.begin(), parts.end(), "cmd") != parts.end()) continue;
        text += (text.empty() ? "" : "+") + name;
    }
    return Wide(text);
}

/// How a menu's button looks: its icon alone, and its height.
struct MenuLook {
    bool iconOnly = false;
    std::optional<ControlShape> shape;
};

/// A menu button. Each item is registered on its own, so external entries
/// reach the items while the menu is closed. With `iconOnly` it is an icon
/// button without a background until pointed at, as an app bar button.
inline void Menu(Parent const& p, Ctx const& ctx, char const* id, Str const& label, char const* icon, MenuLook look, std::vector<std::optional<MenuItemDef>> items) {
    controls::Button button{nullptr};
    auto text = Bound(ctx, label);
    if (look.iconOnly && icon) {
        // An app bar button: no background until pointed at.
        controls::Button plain;
        plain.Content(Icons::Icon(icon));
        Styles::MakeSubtle(plain);
        if (look.shape && look.shape->height.Given())
            BindLen(ctx, look.shape->height, nullptr, [plain](double d) {
                double size = (std::max)(0.0, d);
                plain.Width(size);
                plain.Height(size);
                plain.MinWidth(0);
                plain.MinHeight(0);
                plain.Padding(xaml::Thickness{0, 0, 0, 0});
            });
        if (text) Reactive::Run([plain, text] { controls::ToolTipService::SetToolTip(plain, winrt::box_value(text())); });
        button = plain;
    } else {
        controls::DropDownButton drop;
        controls::StackPanel content;
        content.Orientation(controls::Orientation::Horizontal);
        content.Spacing(8);
        if (icon) content.Children().Append(Icons::Icon(icon));
        if (text) content.Children().Append(TextOf(text));
        drop.Content(content);
        ShapeControl(ctx, drop, look.shape);
        button = drop;
    }
    BindName(button, text);
    controls::MenuFlyout flyout;
    for (auto const& item : items) {
        if (!item) {
            flyout.Items().Append(controls::MenuFlyoutSeparator());
            continue;
        }
        controls::MenuFlyoutItem entry;
        if (item->icon) entry.Icon(Icons::Icon(item->icon));
        auto itemLabel = item->label ? Bound(ctx, item->label) : TextFn([] { return winrt::hstring(); });
        Reactive::Run([entry, itemLabel] { entry.Text(itemLabel()); });
        auto isEnabled = Enabled(ctx, item->enabled);
        if (isEnabled) Reactive::Run([entry, isEnabled] { entry.IsEnabled(isEnabled()); });
        auto run = RunOf(ctx, item->pick, item->files, item->action);
        if (run.activate) entry.Click([activate = run.activate](auto&&, auto&&) { activate(); });
        if (item->shortcut) entry.KeyboardAcceleratorTextOverride(ShortcutText(item->shortcut));
        flyout.Items().Append(entry);
        Controls::Register(ctx, item->id, "button", itemLabel, isEnabled, Actions{.activate = run.activate, .setFile = run.give, .files = item->files});
    }
    if (auto style = PresenterTheme(L"MenuFlyoutPresenter", ctx.appearance)) flyout.MenuFlyoutPresenterStyle(style);
    button.Flyout(flyout);
    auto open = std::make_shared<bool>(false);
    flyout.Opened([open](auto&&, auto&&) { *open = true; });
    flyout.Closed([open](auto&&, auto&&) { *open = false; });
    Controls::Register(ctx, id, "menu", text, nullptr,
        Actions{.activate = [weak = winrt::make_weak(button)] {
                    if (auto button = weak.get()) button.Flyout().ShowAt(button);
                },
            .value = [open] { return std::optional<std::string>(*open ? "open" : "closed"); }});
    if (p.isList && !ctx.toolbar) p.Add(Card(button));
    else p.Add(button);
}

}
