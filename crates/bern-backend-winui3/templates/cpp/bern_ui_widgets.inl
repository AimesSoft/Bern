// Bern runtime for Windows, XAML part 2 of 4: standard controls and
// containers. Inside a form section each control is a card row, as Windows
// Settings lays them out; elsewhere it is the plain control.
// Generated build input; do not edit.

namespace B {

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
};

inline void Text(Parent const& p, Ctx const& ctx, TextProps props) {
    auto block = TextOf(Bound(ctx, props.content));
    if (auto style = Styles::TextStyle(props.style)) block.Style(style);
    if (props.emphasis && (std::string_view(props.emphasis) == "secondary" || std::string_view(props.emphasis) == "tertiary")) Secondary(block);
    Place(p, block);
}

struct LabelProps {
    Str title;
    char const* icon = nullptr;
    char const* style = nullptr;
};

inline void Label(Parent const& p, Ctx const& ctx, LabelProps props) {
    controls::StackPanel row;
    row.Orientation(controls::Orientation::Horizontal);
    row.Spacing(12);
    if (props.icon) row.Children().Append(Icons::Icon(props.icon));
    auto block = TextOf(Bound(ctx, props.title));
    block.VerticalAlignment(xaml::VerticalAlignment::Center);
    if (auto style = Styles::TextStyle(props.style)) block.Style(style);
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

struct ButtonProps {
    char const* id = "";
    Str label;
    char const* icon = nullptr;
    char const* role = nullptr;
    char const* prominence = nullptr;
    Get enabled;
    Act action;
};

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
    auto run = Run(ctx, props.action);
    if (run) button.Click([run](auto&&, auto&&) { run(); });
    Controls::Register(ctx, props.id, "button", text, isEnabled, Actions{.activate = run});
    if (p.isList && !ctx.toolbar) p.Add(Card(button));
    else p.Add(button);
}

struct IconButtonProps {
    char const* id = "";
    char const* icon = "circle";
    Str label;
    Get enabled;
    Act action;
};

inline void IconButton(Parent const& p, Ctx const& ctx, IconButtonProps props) {
    controls::Button button;
    button.Content(Icons::Icon(props.icon));
    Styles::MakeSubtle(button);
    auto text = Bound(ctx, props.label);
    if (text)
        Reactive::Run([button, text] {
            auto name = text();
            controls::ToolTipService::SetToolTip(button, winrt::box_value(name));
            xaml::Automation::AutomationProperties::SetName(button, name);
        });
    auto isEnabled = Enabled(ctx, props.enabled);
    if (isEnabled) Reactive::Run([button, isEnabled] { button.IsEnabled(isEnabled()); });
    auto run = Run(ctx, props.action);
    if (run) button.Click([run](auto&&, auto&&) { run(); });
    Controls::Register(ctx, props.id, "button", text, isEnabled, Actions{.activate = run});
    Place(p, button);
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
    if (secure) {
        controls::PasswordBox box;
        box.MinWidth(240);
        if (placeholder) Reactive::Run([box, placeholder] { box.PlaceholderText(placeholder()); });
        auto updating = Editable([box] { return box.Password(); }, [box](auto const& v) { box.Password(v); }, ctx, field);
        box.PasswordChanged([weak = winrt::make_weak(box), updating, ctx, field](auto&&, auto&&) {
            if (auto box = weak.get(); box && !*updating) SetDraft(ctx, field, Value::Text(Utf8(box.Password())));
        });
        if (run) box.KeyDown([run](auto&&, xaml::Input::KeyRoutedEventArgs const& args) { if (IsEnter(args)) run(); });
        control = box;
    } else {
        controls::TextBox box;
        box.MinWidth(240);
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
    if (p.isList) {
        if (multiline) {
            control.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
            p.Add(Card(Stacked(text, control)));
        } else {
            p.Add(LabeledCard(text, control));
        }
    } else {
        p.Add(Stacked(text, control));
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
            .value = [ctx, field] { return std::optional<std::string>(R::AsText(ctx.Peek(field))); }});
    return box;
}

inline void Search(Parent const& p, Ctx const& ctx, SearchProps props) {
    if (!props.field) return;
    Place(p, SearchBox(ctx, props));
}

struct SwitchProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    char const* style = nullptr;
};

inline void Switch(Parent const& p, Ctx const& ctx, SwitchProps props) {
    if (!props.field) return;
    auto field = *props.field;
    auto text = Bound(ctx, props.label);
    auto updating = std::make_shared<bool>(false);
    auto on = [ctx, field] { return ctx.Read(field) == Value::Bool(true); };
    if (props.style && std::string_view(props.style) == "checkbox") {
        controls::CheckBox check;
        if (text) check.Content(TextOf(text));
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
        check.Checked([updating, ctx, field](auto&&, auto&&) { if (!*updating) SetDraft(ctx, field, Value::Bool(true)); });
        check.Unchecked([updating, ctx, field](auto&&, auto&&) { if (!*updating) SetDraft(ctx, field, Value::Bool(false)); });
        Place(p, check);
    } else {
        controls::ToggleSwitch toggle;
        // The state text follows the app's language, not the system's.
        Reactive::Run([toggle] {
            toggle.OnContent(winrt::box_value(bern::Text::Tr("bern.switch.on")));
            toggle.OffContent(winrt::box_value(bern::Text::Tr("bern.switch.off")));
        });
        BindName(toggle, text);
        Reactive::Run([toggle, on, updating] {
            bool value = on();
            if (toggle.IsOn() != value) {
                *updating = true;
                toggle.IsOn(value);
                *updating = false;
            }
        });
        toggle.Toggled([weak = winrt::make_weak(toggle), updating, ctx, field](auto&&, auto&&) {
            if (auto toggle = weak.get(); toggle && !*updating) SetDraft(ctx, field, Value::Bool(toggle.IsOn()));
        });
        if (p.isList) {
            toggle.MinWidth(0);
            p.Add(LabeledCard(text, toggle));
        } else {
            if (text) toggle.Header(TextOf(text));
            p.Add(toggle);
        }
    }
    Controls::Register(ctx, props.id, "switch", text, nullptr,
        Actions{.setBool = [ctx, field](bool value) { SetDraft(ctx, field, Value::Bool(value)); },
            .value = [ctx, field] { return std::optional<std::string>(ctx.Peek(field) == Value::Bool(true) ? "true" : "false"); }});
}

struct SliderProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    std::optional<double> min, max, step;
    char const* minIcon = nullptr;
    char const* maxIcon = nullptr;
    Act commit;
};

/// A slider: dragging only updates this window's draft as a preview, and
/// releasing commits once; keyboard changes commit at once.
inline void Slider(Parent const& p, Ctx const& ctx, SliderProps props) {
    if (!props.field) return;
    auto field = *props.field;
    auto text = Bound(ctx, props.label);
    double low = props.min.value_or(0), high = props.max.value_or(1);
    double step = props.step.value_or((high - low) / 100);
    controls::Slider slider;
    slider.Minimum(low);
    slider.Maximum(high);
    slider.StepFrequency(step);
    slider.SmallChange(step);
    slider.LargeChange((high - low) / 10);
    slider.MinWidth(200);
    slider.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    auto run = Run(ctx, props.commit);
    auto snap = [low, high, explicitStep = props.step](double value) {
        if (explicitStep && *explicitStep > 0) value = low + std::round((value - low) / *explicitStep) * *explicitStep;
        return (std::clamp)(value, low, high);
    };
    auto updating = std::make_shared<bool>(false);
    auto dragging = std::make_shared<bool>(false);
    Reactive::Run([slider, ctx, field, updating] {
        double value = R::AsDouble(ctx.Read(field));
        if (std::abs(slider.Value() - value) > 1e-12) {
            *updating = true;
            slider.Value(value);
            *updating = false;
        }
    });
    slider.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(xaml::Input::PointerEventHandler([dragging](auto&&, auto&&) { *dragging = true; })), true);
    auto release = [dragging, run](auto&&, auto&&) {
        if (!*dragging) return;
        *dragging = false;
        if (run) run();
    };
    slider.AddHandler(xaml::UIElement::PointerReleasedEvent(), winrt::box_value(xaml::Input::PointerEventHandler(release)), true);
    slider.AddHandler(xaml::UIElement::PointerCaptureLostEvent(), winrt::box_value(xaml::Input::PointerEventHandler(release)), true);
    slider.ValueChanged([updating, dragging, ctx, field, snap, run](auto&&, controls::Primitives::RangeBaseValueChangedEventArgs const& args) {
        if (*updating) return;
        SetDraft(ctx, field, Value::Float(snap(args.NewValue())));
        if (!*dragging && run) run();
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
    auto content = Stacked(text, track);
    if (p.isList) p.Add(Card(content));
    else p.Add(content);
    Controls::Register(ctx, props.id, "slider", text, nullptr,
        Actions{.setNumber = [ctx, field, snap, run](double number) {
                    SetDraft(ctx, field, Value::Float(snap(number)));
                    if (run) run();
                },
            .value = [ctx, field] { return std::optional<std::string>(R::Number(R::AsDouble(ctx.Peek(field)))); }});
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

struct PickerProps {
    char const* id = "";
    Str label;
    std::optional<Field> field;
    std::vector<PickOption> options;
    char const* style = nullptr;
};

inline void Picker(Parent const& p, Ctx const& ctx, PickerProps props) {
    if (!props.field || props.options.empty()) return;
    auto field = *props.field;
    auto text = Bound(ctx, props.label);
    auto options = std::make_shared<std::vector<PickOption>>(std::move(props.options));
    auto indexOf = [options](Value const& value) -> std::optional<int> {
        if (value.kind != Value::Kind::Enum) return std::nullopt;
        for (size_t i = 0; i < options->size(); ++i)
            if ((*options)[i].value == value.variant) return static_cast<int>(i);
        return std::nullopt;
    };
    auto choose = [ctx, field, options](int index) { SetDraft(ctx, field, Value::Enum((*options)[index].value)); };
    std::string_view style = props.style ? props.style : "";
    xaml::FrameworkElement control{nullptr};
    if (style == "segmented" || style == "radio" || style == "inline") {
        std::vector<Str> labels;
        for (auto const& option : *options) labels.push_back(option.label);
        auto segments = MakeSegments(ctx, labels);
        Reactive::Run([segments, ctx, field, indexOf] {
            if (auto index = indexOf(ctx.Read(field))) segments.select(*index);
        });
        segments.onSelect(choose);
        control = segments.element;
    } else {
        controls::ComboBox combo;
        combo.MinWidth(160);
        auto updating = std::make_shared<bool>(false);
        Reactive::Run([combo, options, ctx, field, indexOf, updating] {
            std::vector<winrt::hstring> labels;
            for (auto const& option : *options) labels.push_back(option.label(ctx));
            *updating = true;
            auto selected = combo.SelectedIndex();
            combo.Items().Clear();
            for (auto const& label : labels) combo.Items().Append(winrt::box_value(label));
            combo.SelectedIndex(indexOf(ctx.Peek(field)).value_or(selected));
            *updating = false;
        });
        Reactive::Run([combo, ctx, field, indexOf, updating] {
            if (auto index = indexOf(ctx.Read(field)); index && combo.SelectedIndex() != *index) {
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
    BindName(control, text);
    if (ctx.toolbar) p.Add(control);
    else if (p.isList) p.Add(LabeledCard(text, control));
    else p.Add(Stacked(text, control));
    Controls::Register(ctx, props.id, "picker", text, nullptr,
        Actions{.select = [options, choose](std::string const& name) {
                    for (size_t i = 0; i < options->size(); ++i)
                        if ((*options)[i].name == name) { choose(static_cast<int>(i)); return true; }
                    return false;
                },
            .value = [ctx, field, options, indexOf]() -> std::optional<std::string> {
                if (auto index = indexOf(ctx.Peek(field))) return std::string((*options)[*index].name);
                return std::nullopt;
            }});
}

struct StackProps {
    char const* axis = "column";
    std::optional<double> gap;
    std::optional<double> padding;
    char const* align = nullptr;
};

inline void Stack(Parent const& p, Ctx const& ctx, StackProps props, Build const& children) {
    bool column = std::string_view(props.axis) == "column";
    double pad = props.padding.value_or(0);
    auto panel = MakeFlex(!column, props.gap.value_or(8), xaml::Thickness{pad, pad, pad, pad});
    std::string_view align = props.align ? props.align : "";
    if (column) {
        auto h = align == "center" ? xaml::HorizontalAlignment::Center
            : align == "trailing" ? xaml::HorizontalAlignment::Right
            : align == "leading" ? xaml::HorizontalAlignment::Left
            : xaml::HorizontalAlignment::Stretch;
        children(Parent::Aligned(panel, h, std::nullopt), ctx);
    } else {
        auto v = align == "top" ? xaml::VerticalAlignment::Top
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
    xaml::Thickness padding{0, 0, 0, 0};

    std::vector<xaml::UIElement> Shown() {
        std::vector<xaml::UIElement> shown;
        for (auto child : Children())
            if (child.Visibility() == xaml::Visibility::Visible) shown.push_back(child);
        return shown;
    }

    /// Columns and their width in `width`; without a bound, one row.
    std::pair<size_t, float> Columns(float width, size_t count) const {
        auto gap = static_cast<float>(spacing);
        if (std::isinf(width)) return {(std::max)(count, size_t{1}), static_cast<float>(minWidth)};
        auto columns = (std::max)(static_cast<size_t>((width + gap) / static_cast<float>(minWidth + spacing)), size_t{1});
        return {columns, (std::max)(0.0f, (width - gap * static_cast<float>(columns - 1)) / static_cast<float>(columns))};
    }

    float Horizontal() const { return static_cast<float>(padding.Left + padding.Right); }
    float Vertical() const { return static_cast<float>(padding.Top + padding.Bottom); }

    Size MeasureOverride(Size available) {
        float inner = std::isinf(available.Width) ? INFINITY : (std::max)(0.0f, available.Width - Horizontal());
        auto shown = Shown();
        auto [columns, cell] = Columns(inner, shown.size());
        float height = 0;
        for (size_t start = 0; start < shown.size(); start += columns) {
            float row = 0;
            for (size_t i = start; i < (std::min)(start + columns, shown.size()); ++i) {
                shown[i].Measure(Size{cell, INFINITY});
                row = (std::max)(row, shown[i].DesiredSize().Height);
            }
            height += row + (start > 0 ? static_cast<float>(spacing) : 0.0f);
        }
        float width = std::isinf(inner) ? (cell + static_cast<float>(spacing)) * static_cast<float>(shown.size()) - static_cast<float>(spacing) : inner;
        return Size{(std::max)(0.0f, width) + Horizontal(), height + Vertical()};
    }

    Size ArrangeOverride(Size final) {
        auto shown = Shown();
        auto [columns, cell] = Columns((std::max)(0.0f, final.Width - Horizontal()), shown.size());
        auto gap = static_cast<float>(spacing);
        float top = static_cast<float>(padding.Top);
        for (size_t start = 0; start < shown.size(); start += columns) {
            auto end = (std::min)(start + columns, shown.size());
            float row = 0;
            for (size_t i = start; i < end; ++i) row = (std::max)(row, shown[i].DesiredSize().Height);
            for (size_t i = start; i < end; ++i)
                shown[i].Arrange(Rect{static_cast<float>(padding.Left) + static_cast<float>(i - start) * (cell + gap), top, cell, row});
            top += row + gap;
        }
        for (auto child : Children())
            if (child.Visibility() != xaml::Visibility::Visible) child.Arrange(Rect{0, 0, 0, 0});
        return final;
    }
};

struct GridProps {
    double minWidth = 120;
    std::optional<double> gap;
    std::optional<double> padding;
};

inline void Grid(Parent const& p, Ctx const& ctx, GridProps props, Build const& children) {
    auto impl = winrt::make_self<GridPanel>();
    impl->minWidth = props.minWidth;
    impl->spacing = props.gap.value_or(8);
    double pad = props.padding.value_or(0);
    impl->padding = xaml::Thickness{pad, pad, pad, pad};
    controls::Panel panel = impl.as<controls::Panel>();
    children(Parent::Of(panel), ctx);
    Place(p, panel);
}

struct LayersProps {
    char const* align = nullptr;
};

/// `Stack`: layers in the one cell of a grid, later ones above earlier
/// ones, as large as the largest.
inline void Layers(Parent const& p, Ctx const& ctx, LayersProps props, Build const& children) {
    std::string_view align = props.align ? props.align : "center";
    auto h = align.ends_with("leading") ? xaml::HorizontalAlignment::Left
        : align.ends_with("trailing") ? xaml::HorizontalAlignment::Right
        : xaml::HorizontalAlignment::Center;
    auto v = align.starts_with("top") ? xaml::VerticalAlignment::Top
        : align.starts_with("bottom") ? xaml::VerticalAlignment::Bottom
        : xaml::VerticalAlignment::Center;
    controls::Grid layers;
    layers.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    layers.VerticalAlignment(xaml::VerticalAlignment::Top);
    children(Parent::Aligned(layers, h, v), ctx);
    // Where a column would stretch the stack, it keeps its size at the
    // leading edge.
    controls::Grid holder;
    holder.Children().Append(layers);
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

struct ImageProps {
    char const* source = "";
    Str label;
    std::optional<double> width;
    std::optional<double> height;
    char const* fit = nullptr;
};

/// A picture from a declared resource. With one side given, the other
/// follows the picture's aspect ratio. Without a label it is decorative.
inline void Image(Parent const& p, Ctx const& ctx, ImageProps props) {
    controls::Image image;
    std::string_view fit = props.fit ? props.fit : "contain";
    bool framed = props.width && props.height;
    image.Stretch(!framed || fit == "contain" ? media::Stretch::Uniform
        : fit == "cover" ? media::Stretch::UniformToFill
        : media::Stretch::Fill);
    // The declared frame. With both sides, the picture fits inside it and
    // is centered when it keeps its aspect ratio; with one, the picture
    // sets the other.
    controls::Grid frame;
    frame.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    frame.VerticalAlignment(xaml::VerticalAlignment::Top);
    if (framed) {
        frame.Width(*props.width);
        frame.Height(*props.height);
        image.HorizontalAlignment(xaml::HorizontalAlignment::Center);
        image.VerticalAlignment(xaml::VerticalAlignment::Center);
    } else {
        if (props.width) image.Width(*props.width);
        if (props.height) image.Height(*props.height);
        image.HorizontalAlignment(xaml::HorizontalAlignment::Left);
        image.VerticalAlignment(xaml::VerticalAlignment::Top);
    }
    frame.Children().Append(image);
    media::Imaging::BitmapImage bitmap;
    image.Source(bitmap);
    if (auto bytes = ResourceBytes(props.source); !bytes.empty()) LoadPicture(bitmap, std::move(bytes), props.source);
    else Report(std::string("image ") + props.source + ": the resource cannot be read");
    if (props.label) BindName(image, Bound(ctx, props.label));
    else xaml::Automation::AutomationProperties::SetAccessibilityView(image, xaml::Automation::Peers::AccessibilityView::Raw);
    // Where a column would stretch the picture, it keeps its size at the
    // leading edge.
    controls::Grid holder;
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

inline void Divider(Parent const& p) {
    controls::Border line;
    if (p.IsHorizontal()) line.Width(1);
    else line.Height(1);
    line.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    if (auto style = Styles::Themed(L"Border", {{L"Background", Keys::Divider}})) line.Style(style);
    Place(p, line);
}

inline void ScrollView(Parent const& p, Ctx const& ctx, char const* axis, Build const& children) {
    bool horizontal = axis && std::string_view(axis) == "horizontal";
    auto panel = MakeFlex(horizontal, 8);
    controls::ScrollViewer scroller;
    scroller.Content(panel);
    scroller.HorizontalScrollBarVisibility(horizontal ? controls::ScrollBarVisibility::Auto : controls::ScrollBarVisibility::Disabled);
    scroller.VerticalScrollBarVisibility(horizontal ? controls::ScrollBarVisibility::Disabled : controls::ScrollBarVisibility::Auto);
    scroller.HorizontalScrollMode(horizontal ? controls::ScrollMode::Enabled : controls::ScrollMode::Disabled);
    scroller.VerticalScrollMode(horizontal ? controls::ScrollMode::Disabled : controls::ScrollMode::Enabled);
    if (!horizontal) MarkFill(scroller);
    children(Parent::Of(panel), ctx);
    p.Add(scroller);
}

/// A grouped form: sections of card rows in a scrolling page with a
/// readable maximum width, as Windows Settings pages lay them out.
inline void Form(Parent const& p, Ctx const& ctx, Str const& title, Build const& children) {
    if (title && ctx.page) ctx.page->PushTitle(Bound(ctx, title));
    controls::StackPanel content;
    content.Spacing(24);
    content.MaxWidth(1000);
    content.Padding(xaml::Thickness{24, 8, 24, 24});
    controls::ScrollViewer scroller;
    scroller.Content(content);
    scroller.HorizontalScrollBarVisibility(controls::ScrollBarVisibility::Disabled);
    scroller.VerticalScrollBarVisibility(controls::ScrollBarVisibility::Auto);
    MarkFill(scroller);
    children(Parent::Of(content), ctx);
    p.Add(scroller);
}

/// A titled group of card rows with an optional footnote.
inline void Section(Parent const& p, Ctx const& ctx, Str const& title, Str const& footer, Build const& children) {
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
    p.Add(group);
}

/// Toolbar items for the enclosing page's header.
inline void Toolbar(Parent const&, Ctx const& ctx, Build const& children) {
    if (!ctx.page) return;
    auto holder = ctx.page->AddTools();
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
};

/// A menu button. Each item is registered on its own, so external entries
/// reach the items while the menu is closed.
inline void Menu(Parent const& p, Ctx const& ctx, char const* id, Str const& label, char const* icon, std::vector<std::optional<MenuItemDef>> items) {
    controls::DropDownButton button;
    auto text = Bound(ctx, label);
    controls::StackPanel content;
    content.Orientation(controls::Orientation::Horizontal);
    content.Spacing(8);
    if (icon) content.Children().Append(Icons::Icon(icon));
    if (text) content.Children().Append(TextOf(text));
    BindName(button, text);
    button.Content(content);
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
        auto run = Run(ctx, item->action);
        if (run) entry.Click([run](auto&&, auto&&) { run(); });
        flyout.Items().Append(entry);
        Controls::Register(ctx, item->id, "button", itemLabel, isEnabled, Actions{.activate = run});
    }
    button.Flyout(flyout);
    auto open = std::make_shared<bool>(false);
    flyout.Opened([open](auto&&, auto&&) { *open = true; });
    flyout.Closed([open](auto&&, auto&&) { *open = false; });
    Controls::Register(ctx, id, "menu", text, nullptr, Actions{.value = [open] { return std::optional<std::string>(*open ? "open" : "closed"); }});
    if (p.isList && !ctx.toolbar) p.Add(Card(button));
    else p.Add(button);
}

}
