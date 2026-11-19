// Co-located WinUI 3 / XAML Islands native level meter & number box control.
// Compiled inside `namespace bern::winui::native_*` (and `bern::islands::native_*`),
// so `xaml::*`, `controls::*`, `media::*`, `Reactive::Run` and `BERN_WINUI` are in scope.

inline xaml::UIElement build(Props const& props, Actions const& actions, Theme const& theme) {
    controls::Border card;
    card.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    card.Padding(xaml::Thickness{14, 14, 14, 14});
    card.CornerRadius(xaml::CornerRadius{8, 8, 8, 8});
    card.BorderThickness(xaml::Thickness{1, 1, 1, 1});

    controls::StackPanel root;
    root.Spacing(12);

    // Header row: heading (left) + readout (right), both supplied via Bern `tr!`.
    controls::Grid header;
    controls::ColumnDefinition col0;
    col0.Width(xaml::GridLength{1, xaml::GridUnitType::Star});
    controls::ColumnDefinition col1;
    col1.Width(xaml::GridLength{0, xaml::GridUnitType::Auto});
    header.ColumnDefinitions().Append(col0);
    header.ColumnDefinitions().Append(col1);

    controls::TextBlock title;
    title.FontSize(15);
    controls::Grid::SetColumn(title, 0);
    header.Children().Append(title);

    controls::TextBlock readout;
    readout.FontSize(13);
    controls::Grid::SetColumn(readout, 1);
    header.Children().Append(readout);
    root.Children().Append(header);

    // Controls row: ProgressBar fills remaining width + native WinUI 3 NumberBox.
    controls::Grid row;
    row.ColumnSpacing(12);
    controls::ColumnDefinition rcol0;
    rcol0.Width(xaml::GridLength{1, xaml::GridUnitType::Star});
    controls::ColumnDefinition rcol1;
    rcol1.Width(xaml::GridLength{0, xaml::GridUnitType::Auto});
    row.ColumnDefinitions().Append(rcol0);
    row.ColumnDefinitions().Append(rcol1);

    controls::ProgressBar bar;
    bar.Minimum(props.level_min);
    bar.Maximum(props.level_max);
    bar.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    bar.Height(14);
    bar.VerticalAlignment(xaml::VerticalAlignment::Center);
    bar.PointerMoved([bar, actions, maxVal = props.level_max](auto const&, xaml::Input::PointerRoutedEventArgs const& e) {
        double w = bar.ActualWidth();
        if (w > 1.0) {
            double frac = (std::clamp)(e.GetCurrentPoint(bar).Position().X / w, 0.0, 1.0);
            actions.set_hover(std::round(frac * maxVal * 10.0) / 10.0);
        }
    });
    bar.PointerExited([actions](auto const&, auto const&) {
        actions.set_hover(std::nullopt);
    });
    controls::Grid::SetColumn(bar, 0);
    row.Children().Append(bar);

    auto syncing = std::make_shared<bool>(false);
#if BERN_WINUI
    controls::NumberBox box;
    box.Minimum(props.level_min);
    box.Maximum(props.level_max);
    box.SmallChange(1.0);
    box.SpinButtonPlacementMode(controls::NumberBoxSpinButtonPlacementMode::Inline);
    box.Width(136);
    box.ValueChanged([syncing, actions](controls::NumberBox const&, controls::NumberBoxValueChangedEventArgs const& args) {
        if (*syncing || std::isnan(args.NewValue())) return;
        actions.set_level(args.NewValue());
    });
    controls::Grid::SetColumn(box, 1);
    row.Children().Append(box);
#else
    controls::StackPanel steppers;
    steppers.Orientation(controls::Orientation::Horizontal);
    steppers.Spacing(6);
    controls::Button minusBtn;
    minusBtn.Content(winrt::box_value(L"-"));
    minusBtn.Click([props, actions](auto const&, auto const&) { actions.set_level(props.level() - 1.0); });
    controls::Button plusBtn;
    plusBtn.Content(winrt::box_value(L"+"));
    plusBtn.Click([props, actions](auto const&, auto const&) { actions.set_level(props.level() + 1.0); });
    steppers.Children().Append(minusBtn);
    steppers.Children().Append(plusBtn);
    controls::Grid::SetColumn(steppers, 1);
    row.Children().Append(steppers);
#endif
    root.Children().Append(row);

    // Footer row: native ToggleSwitch (left) + Reset Button (right).
    controls::Grid footer;
    controls::ColumnDefinition fcol0;
    fcol0.Width(xaml::GridLength{1, xaml::GridUnitType::Star});
    controls::ColumnDefinition fcol1;
    fcol1.Width(xaml::GridLength{0, xaml::GridUnitType::Auto});
    footer.ColumnDefinitions().Append(fcol0);
    footer.ColumnDefinitions().Append(fcol1);

    controls::ToggleSwitch toggle;
    toggle.Toggled([syncing, actions](auto const& sender, auto const&) {
        if (*syncing) return;
        actions.set_boosted(sender.as<controls::ToggleSwitch>().IsOn());
    });
    controls::Grid::SetColumn(toggle, 0);
    footer.Children().Append(toggle);

    controls::Button resetBtn;
    resetBtn.VerticalAlignment(xaml::VerticalAlignment::Center);
    resetBtn.Click([actions](auto const&, auto const&) { actions.reset(); });
    controls::Grid::SetColumn(resetBtn, 1);
    footer.Children().Append(resetBtn);
    root.Children().Append(footer);

    card.Child(root);

    Reactive::Run([=]() mutable {
        *syncing = true;
        title.Text(props.heading());
        readout.Text(props.readout());
        double val = props.level();
        bar.Value(val);
#if BERN_WINUI
        if (box.Value() != val) box.Value(val);
#endif
        toggle.Header(winrt::box_value(props.boosted_label()));
        if (toggle.IsOn() != props.boosted()) toggle.IsOn(props.boosted());
        resetBtn.Content(winrt::box_value(props.reset_label()));
        card.BorderBrush(media::SolidColorBrush(theme.border()));
        readout.Foreground(media::SolidColorBrush(theme.text_secondary()));
        *syncing = false;
    });

    return card;
}
