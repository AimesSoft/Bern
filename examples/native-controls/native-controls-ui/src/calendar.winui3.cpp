// Co-located WinUI 3 / XAML Islands native CalendarView control.

inline xaml::UIElement build(Props const& props, Actions const& actions, Theme const& theme) {
    controls::Border card;
    card.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    card.Padding(xaml::Thickness{14, 14, 14, 14});
    card.CornerRadius(xaml::CornerRadius{8, 8, 8, 8});
    card.BorderThickness(xaml::Thickness{1, 1, 1, 1});

    controls::StackPanel root;
    root.Spacing(12);

    controls::Grid header;
    header.ColumnSpacing(12);
    controls::ColumnDefinition hcol0;
    hcol0.Width(xaml::GridLength{1, xaml::GridUnitType::Star});
    controls::ColumnDefinition hcol1;
    hcol1.Width(xaml::GridLength{0, xaml::GridUnitType::Auto});
    controls::ColumnDefinition hcol2;
    hcol2.Width(xaml::GridLength{0, xaml::GridUnitType::Auto});
    header.ColumnDefinitions().Append(hcol0);
    header.ColumnDefinitions().Append(hcol1);
    header.ColumnDefinitions().Append(hcol2);

    controls::TextBlock title;
    title.FontSize(15);
    title.VerticalAlignment(xaml::VerticalAlignment::Center);
    controls::Grid::SetColumn(title, 0);

    controls::TextBlock dateText;
    dateText.FontSize(13);
    dateText.VerticalAlignment(xaml::VerticalAlignment::Center);
    controls::Grid::SetColumn(dateText, 1);

    controls::Button todayBtn;
    todayBtn.Click([actions](auto const&, auto const&) { actions.today(); });
    controls::Grid::SetColumn(todayBtn, 2);

    header.Children().Append(title);
    header.Children().Append(dateText);
    header.Children().Append(todayBtn);
    root.Children().Append(header);

    controls::CalendarView cal;
    cal.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    cal.SelectionMode(controls::CalendarViewSelectionMode::Single);

    auto syncing = std::make_shared<bool>(false);
    cal.SelectedDatesChanged([syncing, actions](controls::CalendarView const&, controls::CalendarViewSelectedDatesChangedEventArgs const& args) {
        if (*syncing || args.AddedDates().Size() == 0) return;
        auto ft = winrt::clock::to_file_time(args.AddedDates().GetAt(0));
        FILETIME wft{ft.value & 0xFFFFFFFFu, static_cast<DWORD>(ft.value >> 32)};
        SYSTEMTIME st{};
        if (FileTimeToSystemTime(&wft, &st)) {
            wchar_t buf[32]{};
            swprintf_s(buf, L"%04u-%02u-%02u", st.wYear, st.wMonth, st.wDay);
            actions.set_date(winrt::hstring(buf));
        }
    });
    root.Children().Append(cal);
    card.Child(root);

    Reactive::Run([=]() mutable {
        *syncing = true;
        title.Text(props.heading());
        auto iso = props.date();
        dateText.Text(iso);
        todayBtn.Content(winrt::box_value(props.today_label()));
        card.BorderBrush(media::SolidColorBrush(theme.border()));
        dateText.Foreground(media::SolidColorBrush(theme.text_secondary()));
        int y = 0, m = 0, d = 0;
        if (swscanf_s(iso.c_str(), L"%d-%d-%d", &y, &m, &d) == 3 && y >= 1900 && m >= 1 && m <= 12 && d >= 1 && d <= 31) {
            SYSTEMTIME st{};
            st.wYear = static_cast<WORD>(y);
            st.wMonth = static_cast<WORD>(m);
            st.wDay = static_cast<WORD>(d);
            st.wHour = 12;
            FILETIME wft{};
            if (SystemTimeToFileTime(&st, &wft)) {
                winrt::file_time ft{static_cast<uint64_t>(wft.dwLowDateTime) | (static_cast<uint64_t>(wft.dwHighDateTime) << 32)};
                auto dt = winrt::clock::from_file_time(ft);
                cal.SelectedDates().Clear();
                cal.SelectedDates().Append(dt);
                cal.SetDisplayDate(dt);
            }
        }
        *syncing = false;
    });

    return card;
}
