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

/// The XAML picture: SDR pictures, and HDR ones where they cannot be drawn
/// as HDR. `framed` when both sides are given.
inline controls::Image SdrImage(ImageProps const& props, std::vector<uint8_t> bytes, TextFn const& name) {
    controls::Image image;
    std::string_view fit = props.fit ? props.fit : "contain";
    bool framed = props.width && props.height;
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
    if (!bytes.empty()) LoadPicture(bitmap, std::move(bytes), props.source);
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
    ImageProps props;
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
inline void Show(controls::Grid const& frame, Ctx const& ctx, std::shared_ptr<std::vector<uint8_t>> bytes, ImageProps const& props, TextFn name, bool gainMap) {
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

/// A picture from a declared resource. With one side given, the other
/// follows the picture's aspect ratio. Without a label it is decorative.
/// HDR pictures are drawn in HDR where the display shows it.
inline void Image(Parent const& p, Ctx const& ctx, ImageProps props) {
    // The declared frame. With both sides, the picture fits inside it and
    // is centered when it keeps its aspect ratio; with one, the picture
    // sets the other.
    controls::Grid frame;
    frame.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    frame.VerticalAlignment(xaml::VerticalAlignment::Top);
    if (props.width && props.height) {
        frame.Width(*props.width);
        frame.Height(*props.height);
    }
    auto bytes = std::make_shared<std::vector<uint8_t>>(ResourceBytes(props.source));
    if (bytes->empty()) Report(std::string("image ") + props.source + ": the resource cannot be read");
    TextFn name = props.label ? Bound(ctx, props.label) : nullptr;
    uint32_t kind = bytes->empty() ? 0 : bern_picture_kind(bytes->data(), bytes->size());
    if (kind != 0 && Hdr::GetGpu()) Hdr::Show(frame, ctx, bytes, props, name, kind == 3);
    else frame.Children().Append(SdrImage(props, *bytes, name));
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
