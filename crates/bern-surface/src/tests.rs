use crate::controls::{self, ControlKind};
use crate::ir::{Axis, ColorValue, Length, SurfaceNode, SurfaceTree};
use crate::palette::SurfacePalette;
use crate::{DebugRenderer, IR_VERSION, web};
use bern_theme::{ContrastPolicy, Mode, Srgb};

fn sample_node(kind: ControlKind) -> SurfaceNode {
    match kind {
        ControlKind::Button => controls::button(controls::ButtonSpec {
            label: "Save".into(),
            enabled: true,
            ..Default::default()
        }),
        ControlKind::IconButton => controls::icon_button(controls::IconButtonSpec::default()),
        ControlKind::Switch => controls::switch(controls::SwitchSpec {
            label: "Auto".into(),
            checked: true,
            enabled: true,
            ..Default::default()
        }),
        ControlKind::Checkbox => controls::checkbox(controls::CheckboxSpec {
            label: "Accept".into(),
            checked: true,
            enabled: true,
            ..Default::default()
        }),
        ControlKind::ToggleButton => controls::toggle_button(controls::ToggleButtonSpec {
            label: "Shuffle".into(),
            checked: true,
            enabled: true,
            ..Default::default()
        }),
        ControlKind::Slider => controls::slider(controls::SliderSpec {
            label: Some("Volume".into()),
            fraction: 0.42,
            min_icon: Some("speaker.fill"),
            max_icon: Some("speaker.wave.3.fill"),
            ..Default::default()
        }),
        ControlKind::MediaScrubber => controls::media_scrubber(controls::MediaScrubberSpec {
            fraction: 0.5,
            enabled: true,
            tint: None,
        }),
        ControlKind::TextInput => controls::text_input(controls::TextInputSpec {
            label: "Name".into(),
            enabled: true,
            ..Default::default()
        }),
        ControlKind::NumberInput => controls::number_input(controls::NumberInputSpec {
            label: "Count".into(),
            formatted: "42".into(),
            can_decrement: true,
            can_increment: true,
            enabled: true,
            ..Default::default()
        }),
        ControlKind::DatePicker => controls::date_picker(controls::DatePickerSpec {
            label: "Date".into(),
            formatted: "2026-11-25".into(),
            enabled: true,
            ..Default::default()
        }),
        ControlKind::TimePicker => controls::time_picker(controls::TimePickerSpec {
            label: "Time".into(),
            formatted: "09:30".into(),
            enabled: true,
            ..Default::default()
        }),
        ControlKind::ColorPicker => controls::color_picker(controls::ColorPickerSpec {
            label: "Tint".into(),
            ..Default::default()
        }),
        ControlKind::ShortcutInput => controls::shortcut_input(controls::ShortcutInputSpec {
            label: "Play".into(),
            shown: "Space".into(),
            enabled: true,
            ..Default::default()
        }),
        ControlKind::PickerMenu => controls::picker_menu(controls::PickerMenuSpec {
            label: "Quality".into(),
            selected_label: "High".into(),
            enabled: true,
            ..Default::default()
        }),
        ControlKind::PickerSegmented => controls::picker_segmented(controls::PickerSegmentedSpec {
            label: "Mode".into(),
            options: vec![
                controls::SegmentOptionSpec {
                    label: "A".into(),
                    selected: true,
                },
                controls::SegmentOptionSpec {
                    label: "B".into(),
                    selected: false,
                },
            ],
            enabled: true,
            ..Default::default()
        }),
        ControlKind::PickerChips => controls::picker_chips(controls::PickerChipsSpec {
            label: "Tags".into(),
            options: vec![
                controls::ChipOptionSpec {
                    label: "One".into(),
                    selected: true,
                },
                controls::ChipOptionSpec {
                    label: "Two".into(),
                    selected: false,
                },
            ],
            enabled: true,
            ..Default::default()
        }),
        ControlKind::MenuPanel => {
            controls::menu_panel(controls::MenuPanelSpec { min_width: 200.0 })
        }
        ControlKind::MenuItem => controls::menu_item(controls::MenuItemSpec {
            label: "Open".into(),
            icon: Some("folder"),
            checked: Some(true),
            reserve_check_column: true,
            enabled: true,
            ..Default::default()
        }),
        ControlKind::ControlGroup => controls::control_group(controls::ControlGroupSpec::default()),
        ControlKind::PopoverPanel => controls::popover_panel(controls::PopoverPanelSpec::default()),
        ControlKind::SplitButton => controls::split_button(controls::SplitButtonSpec {
            height: 36.0,
            open: false,
        }),
        ControlKind::Search => controls::search(controls::SearchSpec {
            width: Some(220.0),
            has_results: true,
        }),
        ControlKind::SearchPalette => controls::search_palette(controls::SearchPaletteSpec {
            prompt: "Search".into(),
            shortcut_hint: Some("⌘K".into()),
            enabled: true,
            ..Default::default()
        }),
        ControlKind::SearchItem => controls::search_item(controls::SearchItemSpec {
            label: "Settings".into(),
            icon: Some("gear"),
            detail: Some("Preferences".into()),
            hint: Some("⌘,".into()),
            ..Default::default()
        }),
        ControlKind::Toolbar => controls::toolbar(controls::ToolbarSpec { height: 52.0 }),
        ControlKind::SplitView => controls::split_view(controls::SplitViewSpec::default()),
        ControlKind::Sidebar => controls::sidebar(controls::SidebarSpec {
            has_header: true,
            has_footer: true,
            ..Default::default()
        }),
        ControlKind::SidebarItem => controls::sidebar_item(controls::SidebarItemSpec {
            label: "Library".into(),
            icon: Some("music.note"),
            badge: Some(3),
            selected: true,
            enabled: true,
            ..Default::default()
        }),
        ControlKind::Detail => controls::detail(controls::DetailSpec {
            title: Some("Settings".into()),
            has_toggle: true,
            has_footer: true,
            ..Default::default()
        }),
        ControlKind::NavigationBar => controls::navigation_bar(controls::NavigationBarSpec {
            title: Some("Details".into()),
        }),
        ControlKind::NavLink => controls::nav_link(controls::NavLinkSpec {
            in_section: false,
            enabled: true,
        }),
        ControlKind::Tabs => controls::tabs(controls::TabsSpec {
            has_accessory: true,
        }),
        ControlKind::TabItem => controls::tab_item(controls::TabItemSpec {
            label: "Home".into(),
            icon: Some("house"),
            selected: true,
            ..Default::default()
        }),
        ControlKind::TabAccessory => controls::tab_accessory(controls::TabAccessorySpec),
        ControlKind::Inspector => controls::inspector(controls::InspectorSpec {
            presented: true,
            width: Some(280.0),
        }),
        ControlKind::Form => controls::form(controls::FormSpec {
            title: Some("Preferences".into()),
            gap: 20.0,
        }),
        ControlKind::Section => controls::section(controls::SectionSpec {
            title: Some("General".into()),
            footer: Some("Applies immediately".into()),
            ..Default::default()
        }),
        ControlKind::Divider => controls::divider(controls::DividerSpec),
        ControlKind::List => controls::list(controls::ListSpec { gap: 2.0 }),
        ControlKind::DropArea => controls::drop_area(controls::DropAreaSpec { active: true }),
        ControlKind::Modal => controls::modal(controls::ModalSpec::default()),
        ControlKind::Alert => controls::alert(controls::AlertSpec {
            title: "Delete?".into(),
            message: Some("Cannot be undone.".into()),
            width: 380.0,
        }),
        ControlKind::BottomSheet => controls::bottom_sheet(controls::BottomSheetSpec {
            width: 640.0,
            max_height: 480.0,
            drag_indicator: true,
        }),
        ControlKind::Tooltip => controls::tooltip(controls::TooltipSpec {
            content: "Play".into(),
            keys: Some("Space".into()),
        }),
        ControlKind::Toast => controls::toast(controls::ToastSpec {
            content: "Saved".into(),
            icon: Some("checkmark"),
        }),
        ControlKind::Disclosure => controls::disclosure(controls::DisclosureSpec {
            label: "Advanced".into(),
            icon: Some("slider.horizontal.3"),
            expanded: true,
        }),
        ControlKind::Banner => controls::banner(controls::BannerSpec {
            severity: controls::BannerSeverity::Warning,
            title: Some("Offline".into()),
            content: "Reconnecting".into(),
            has_actions: true,
            dismissible: true,
            ..Default::default()
        }),
        ControlKind::Badge => controls::badge(controls::BadgeSpec {
            content: "Beta".into(),
            tone: controls::BadgeTone::Accent,
            icon: Some("sparkles"),
        }),
        ControlKind::EmptyState => controls::empty_state(controls::EmptyStateSpec {
            title: "Nothing here".into(),
            message: Some("Add an item to begin.".into()),
            icon: Some("tray"),
            has_actions: true,
        }),
        ControlKind::Avatar => controls::avatar(controls::AvatarSpec {
            label: "Ada Lovelace".into(),
            size: 40.0,
            rounded: false,
            has_image: true,
        }),
        ControlKind::Text => controls::text(controls::TextSpec {
            content: "Hello".into(),
            style: controls::TextStyleKind::Title,
            ..Default::default()
        }),
        ControlKind::Label => controls::label(controls::LabelSpec {
            title: "Audio".into(),
            icon: Some("waveform"),
            ..Default::default()
        }),
        ControlKind::Progress => controls::progress(controls::ProgressSpec {
            fraction: 0.65,
            label: Some("Loading".into()),
        }),
        ControlKind::ProgressRing => controls::progress_ring(controls::ProgressRingSpec {
            fraction: 0.75,
            diameter: 24.0,
            stroke_width: 2.5,
        }),
        ControlKind::Skeleton => controls::skeleton(controls::SkeletonSpec::default()),
        ControlKind::Table => controls::table(controls::TableSpec),
        ControlKind::TreeView => controls::tree_view(controls::TreeViewSpec),
        ControlKind::SplitPane => controls::split_pane(controls::SplitPaneSpec::default()),
        ControlKind::Reorderable => controls::reorderable(controls::ReorderableSpec),
        ControlKind::Refreshable => {
            controls::refreshable(controls::RefreshableSpec { refreshing: true })
        }
        ControlKind::MediaBar => controls::media_bar(controls::MediaBarSpec),
    }
}

fn verify_colors(node: &SurfaceNode, palette: &SurfacePalette, context: &str) {
    let check = |c: ColorValue| match c {
        ColorValue::Role(r) | ColorValue::RoleAlpha(r, _) => {
            assert!(
                palette.try_role(r).is_some(),
                "{context}: unknown semantic role `{r}`"
            );
        }
        ColorValue::Recipe {
            recipe,
            state,
            slot,
        } => {
            let role = SurfacePalette::slot_role(recipe, state, slot);
            assert!(
                palette.try_role(role).is_some(),
                "{context}: unknown recipe role `{role}` for {recipe}.{state}.{slot}"
            );
        }
        ColorValue::Transparent | ColorValue::Fixed { .. } | ColorValue::Rgba(_) => {}
    };
    let check_style = |s: &crate::ir::VisualStyle| {
        if let Some(bg) = s.background {
            check(bg);
        }
        if let Some(fg) = s.foreground {
            check(fg);
        }
        if let Some(b) = s.border {
            check(b.color);
        }
        for sh in &s.shadows {
            check(sh.color);
        }
    };
    match node {
        SurfaceNode::Box(b) => {
            check_style(&b.style);
            for st in [
                &b.states.hover,
                &b.states.pressed,
                &b.states.selected,
                &b.states.disabled,
            ]
            .into_iter()
            .flatten()
            {
                check_style(st);
            }
            if let Some(ring) = b.states.focus_ring {
                check(ring);
            }
            for child in &b.children {
                verify_colors(child, palette, context);
            }
        }
        SurfaceNode::Text(t) => check(t.color),
        SurfaceNode::Icon(i) => check(i.color),
        SurfaceNode::Track(t) => {
            check(t.track_color);
            check(t.fill_color);
            if let Some(thumb) = &t.thumb {
                verify_colors(&SurfaceNode::Box(thumb.clone()), palette, context);
            }
        }
        SurfaceNode::Ring(r) => check(r.color),
        SurfaceNode::Spacer | SurfaceNode::Slot(_) => {}
    }
}

#[test]
fn every_self_drawn_control_lowers_with_valid_roles_in_light_and_dark() {
    for mode in [Mode::Light, Mode::Dark] {
        let palette = SurfacePalette::generate(
            Some(Srgb::new(0xFF, 0x2E, 0x55)),
            mode,
            ContrastPolicy::Standard,
        );
        for &kind in ControlKind::ALL {
            let node = sample_node(kind);
            verify_colors(&node, &palette, &format!("{kind:?} ({mode:?})"));
            let tree = SurfaceTree::new(node);
            assert_eq!(tree.ir_version, IR_VERSION);
            let rendered = DebugRenderer::render_to_string(&tree, &palette);
            assert!(!rendered.is_empty(), "{kind:?} rendered empty");
        }
    }
}

#[test]
fn slider_thumb_is_a_white_perpendicular_pill_with_border_and_shadow() {
    for (axis, expected_w, expected_h) in [
        (
            Axis::Horizontal,
            controls::SLIDER_THUMB_THICKNESS,
            controls::SLIDER_THUMB_LENGTH,
        ),
        (
            Axis::Vertical,
            controls::SLIDER_THUMB_LENGTH,
            controls::SLIDER_THUMB_THICKNESS,
        ),
    ] {
        let thumb = controls::slider_thumb(axis, true);
        assert_eq!(thumb.layout.width, Length::Px(expected_w));
        assert_eq!(thumb.layout.height, Length::Px(expected_h));
        assert!(thumb.style.corners.unwrap().is_pill());
        assert!(thumb.style.border.is_some());
        assert!(!thumb.style.shadows.is_empty());
        for mode in [Mode::Light, Mode::Dark] {
            let palette = SurfacePalette::generate(None, mode, ContrastPolicy::Standard);
            let bg = palette.resolve(thumb.style.background.unwrap());
            assert_eq!(bg, crate::Rgba8::WHITE, "{mode:?} thumb must be white");
            let border = palette.resolve(thumb.style.border.unwrap().color);
            assert!(border.a > 0 && border.a < 80, "{mode:?} subtle border");
            let shadow = palette.resolve(thumb.style.shadows[0].color);
            assert!(shadow.a > 0 && shadow.a < 130, "{mode:?} soft shadow");
        }
    }
}

#[test]
fn translucent_overlays_separators_and_sidebar_match_across_modes() {
    for mode in [Mode::Light, Mode::Dark] {
        let palette = SurfacePalette::generate(None, mode, ContrastPolicy::Standard);
        for role in [
            "fill.subtle",
            "fill.strong",
            "hover.overlay",
            "pressed.overlay",
            "disabled.surface",
            "separator",
            "sidebar.surface",
        ] {
            let c = palette.role(role);
            assert!(
                c.a > 0 && c.a < 40,
                "{mode:?} {role} should be translucent, got alpha={}",
                c.a
            );
        }
        assert!(palette.role("fill.subtle").a < palette.role("fill.strong").a);
        assert!(palette.role("sidebar.surface").a < palette.role("fill.subtle").a);
    }
}

#[test]
fn web_emitter_produces_surface_tokens_and_pill_slider_css() {
    let snap = bern_theme::generate(None, Mode::Dark, ContrastPolicy::Standard);
    let decls = web::theme_declarations(&snap);
    for expected in [
        "--bern-sidebar-surface:",
        "--bern-slider-thumb: #ffffff;",
        "--bern-slider-thumb-border:",
        "--bern-slider-thumb-shadow:",
        "--bern-fill-subtle:",
        "--bern-separator: rgb(",
    ] {
        assert!(
            decls.contains(expected),
            "missing `{expected}` in:\n{decls}"
        );
    }
    let css = web::stylesheet();
    for expected in [
        "--bern-slider-thumb-w: 7px;",
        "--bern-slider-thumb-h: 18px;",
        "background: var(--bern-slider-thumb);",
        "border: 1px solid var(--bern-slider-thumb-border);",
        "box-shadow: var(--bern-slider-thumb-shadow-box);",
        "background: var(--bern-sidebar-surface);",
    ] {
        assert!(css.contains(expected), "missing `{expected}` in:\n{css}");
    }
}
