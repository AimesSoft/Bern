//! The checks of the features a target lists in
//! [`TargetDescriptor::features`]: what an application uses that a target
//! without the feature cannot build, or shows in a plainer form.

use crate::{Diagnostic, TargetDescriptor, feature};
use bern_ir::Application;

/// A check of one feature, for a target that lacks it: the diagnostics
/// of the parts of `app` that use it.
type Check = fn(&TargetDescriptor, &Application, &mut Vec<Diagnostic>);

/// Every check, with the feature it is for, in the order the diagnostics
/// are reported. A check is named after its feature, whose documentation
/// in [`feature`] says what it covers.
const CHECKS: &[(&str, Check)] = &[
    (feature::TEXT_FORMATS, text_formats),
    (feature::SEARCH_PALETTE, search_palette),
    (feature::RAILS, rails),
    (feature::TEXT_WEIGHTS, text_weights),
    (feature::TEXT_LINES, text_lines),
    (feature::TOOLBAR_LEADING, toolbar_leading),
    (feature::AXIS_PADDING, axis_padding),
    (feature::ICON_MENUS, icon_menus),
    (feature::ICON_LABELS, icon_labels),
    (feature::ICON_TABS, icon_tabs),
    (feature::ROW_BASELINE, row_baseline),
    (feature::IMAGE_CORNERS, image_corners),
    (feature::EDGE_GROW, edge_grow),
    (feature::DELAYED_APPEAR, delayed_appear),
    (feature::SCALED_SCENE, scaled_scene),
    (feature::SCALED_SCENE, dynamic_text_sizes),
    (feature::ACCORDION_LAYOUT, accordion_layout),
    (feature::ADAPTIVE_IMAGES, adaptive_images),
    (feature::PAGE_BADGES, page_badges),
    (feature::MODAL_FLOW_TIMER, modal_flow_timer),
    (feature::CAPSULE_CONTROLS, capsule_controls),
    (feature::SIDEBAR_WIDTH, sidebar_width),
    (feature::CONTROLLED_VALUES, controlled_values),
    (feature::IMAGE_TEMPLATE, image_template),
    (feature::SIDEBAR_HEADER, sidebar_header),
    (feature::FILL_BUTTONS, fill_buttons),
    (feature::MODAL_COVER, modal_cover),
    (feature::FORM_LAYOUT, form_layout),
    (feature::PAGE_SEARCH, page_search),
    (feature::TAB_ACCESSORY, tab_accessory),
    (feature::GRID_CELLS, grid_cells),
    (feature::WINDOW_SHORT, window_short),
    (feature::WINDOW_LANDSCAPE, window_landscape),
    (feature::IDLE_CONTROL, idle_control),
    (feature::UNTITLED_DETAIL, untitled_detail),
    (feature::POPOVER_BUTTON, popover_button),
    (feature::SIZE_CLASSES, size_classes),
    (feature::BUTTON_CONTENT, button_content),
    (feature::TOOLBAR_SEARCH, toolbar_search),
    (feature::SEARCH_RESULTS, search_results),
    (feature::SESSION_ACTIONS, session_actions),
    (feature::TOGGLE_BUTTON, toggle_button),
    (feature::DIALOG_WINDOWS, dialog_windows),
    (feature::BOUND_LENGTHS, bound_lengths),
    (feature::APPEARANCE, appearance),
    (feature::SHORTCUTS, shortcuts),
    (feature::RUNTIME_THEME, runtime_theme),
    (feature::RUNTIME_PALETTE, runtime_palette),
    (feature::RUNTIME_IMAGES, runtime_images),
    (feature::FILE_PICK, file_pick),
    (feature::SCROLL_SNAP, scroll_snap),
    (feature::PICTURE_FILLS, picture_fills),
    (feature::REVEAL, reveal),
];

/// Features the application uses that the target does not list.
pub(super) fn check_features(descriptor: &TargetDescriptor, app: &Application) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let lacks = |name: &str| !descriptor.features.contains(&name);
    for &(name, check) in CHECKS {
        if lacks(name) {
            check(descriptor, app, &mut diagnostics);
        }
    }
    diagnostics
}

/// The first element of the application `found` picks.
fn first_element(
    app: &Application,
    found: impl Fn(&bern_ir::Element) -> bool,
) -> Option<&bern_ir::Element> {
    let mut first = None;
    app.walk(&mut |node, _| {
        if let bern_ir::Node::Element(element) = node
            && first.is_none()
            && found(element)
        {
            first = Some(element);
        }
    });
    first
}

/// Whether an expression in `node` (a condition, or a prop of an element)
/// reads one of the window's size fields.
fn reads_window_size(node: &bern_ir::Node) -> bool {
    let mut found = false;
    let mut check = |expr: &bern_ir::Expr| {
        expr.walk(&mut |part| {
            if let bern_ir::ExprKind::Binding {
                binding: bern_ir::Binding::Window { field },
            } = &part.kind
                && bern_ir::LayoutSize::parse(field).is_some()
            {
                found = true;
            }
        });
    };
    match node {
        bern_ir::Node::Element(element) => {
            for prop in element.props.values() {
                if let bern_ir::Prop::Expr { expr } = prop {
                    check(expr);
                }
            }
        }
        bern_ir::Node::If(node) => {
            for (condition, _) in &node.branches {
                check(condition);
            }
        }
        _ => {}
    }
    found
}

/// The first `Search` standing in the page: toolbars and sidebars hold
/// theirs directly.
fn first_page_search(app: &Application) -> Option<&bern_ir::Element> {
    let mut held = std::collections::HashSet::new();
    app.walk(&mut |node, _| {
        if let bern_ir::Node::Element(bar) = node
            && (bar.kind == "Toolbar" || bar.kind == "Sidebar")
        {
            for child in &bar.children {
                if let bern_ir::Node::Element(search) = child
                    && search.kind == "Search"
                {
                    held.insert(search as *const bern_ir::Element);
                }
            }
        }
    });
    first_element(app, |element| {
        element.kind == "Search" && !held.contains(&(element as *const bern_ir::Element))
    })
}

/// Whether any expression of the application reads `window.<field>`.
fn reads_window_field(app: &Application, wanted: &str) -> bool {
    let mut found = false;
    let mut check = |expr: &bern_ir::Expr| {
        expr.walk(&mut |part| {
            if let bern_ir::ExprKind::Binding {
                binding: bern_ir::Binding::Window { field },
            } = &part.kind
                && field == wanted
            {
                found = true;
            }
        });
    };
    app.walk(&mut |node, _| match node {
        bern_ir::Node::Element(element) => {
            for prop in element.props.values() {
                if let bern_ir::Prop::Expr { expr } = prop {
                    check(expr);
                }
            }
        }
        bern_ir::Node::If(node) => {
            for (condition, _) in &node.branches {
                check(condition);
            }
        }
        _ => {}
    });
    found
}

fn text_formats(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if crate::uses_text_formats(app) {
        diagnostics.push(Diagnostic::error(format!(
            "target `{}` cannot yet write plural branches, dates or durations in texts, or show a variant's fields in its label",
            descriptor.id
        )));
    }
}

fn search_palette(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |e| {
        e.kind == "SearchItem" || (e.kind == "Search" && e.keyword("style") == Some("palette"))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot present a search palette",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn rails(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(rail) = first_element(app, |element| element.kind == "Rail") {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot place elements on a `Rail`",
                descriptor.id
            ))
            .at(rail.source.clone()),
        );
    }
}

fn text_weights(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !app.style.text_weights.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "target `{}` cannot give text styles their own weights (`text {{ … }}` in `style!`)",
            descriptor.id
        )));
    }
}

fn text_lines(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(text) = first_element(app, |element| {
        element.kind == "Text"
            && (element.prop("lines").is_some() || element.prop("line_height").is_some())
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot limit the lines or set the line height of a `Text`",
                descriptor.id
            ))
            .at(text.source.clone()),
        );
    }
}

fn toolbar_leading(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(toolbar) = first_element(app, |element| {
        element.kind == "Toolbar"
            && element.children.iter().any(|node| {
                matches!(node, bern_ir::Node::Element(child) if child.kind == "Spacer" || child.kind == "Text")
            })
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot put toolbar items at the leading end or show text in a toolbar",
                descriptor.id
            ))
            .at(toolbar.source.clone()),
        );
    }
}

fn axis_padding(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| {
        element.prop("padding_x").is_some() || element.prop("padding_y").is_some()
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` pads every edge alike; it has no `padding_x` or `padding_y`",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn icon_menus(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(menu) = first_element(app, |element| {
        element.kind == "Menu" && element.keyword("style") == Some("icon")
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot show a `Menu` as an icon button",
                descriptor.id
            ))
            .at(menu.source.clone()),
        );
    }
}

fn icon_labels(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(label) = first_element(app, |element| {
        element.kind == "Label" && element.keyword("display") == Some("icon")
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot show a `Label` as an icon only",
                descriptor.id
            ))
            .at(label.source.clone()),
        );
    }
}

fn icon_tabs(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(tab) = first_element(app, |element| {
        element.kind == "Tab" && element.keyword("display") == Some("icon")
    }) {
        // The tab still shows, with its label beside the icon.
        diagnostics.push(
            Diagnostic::warning(format!(
                "target `{}` shows the label of a `Tab` with `display: icon` beside its icon",
                descriptor.id
            ))
            .at(tab.source.clone()),
        );
    }
}

fn row_baseline(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(row) = first_element(app, |element| {
        element.kind == "Row" && element.keyword("align") == Some("baseline")
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot line a `Row` up on its baseline",
                descriptor.id
            ))
            .at(row.source.clone()),
        );
    }
}

fn image_corners(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(image) = first_element(app, |element| {
        element.kind == "Image" && element.prop("corner").is_some()
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot round the corners of an `Image`",
                descriptor.id
            ))
            .at(image.source.clone()),
        );
    }
}

fn edge_grow(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if app
        .style
        .transitions
        .iter()
        .any(|transition| matches!(transition.kind, bern_ir::style::TransitionKind::Grow { .. }))
    {
        diagnostics.push(Diagnostic::error(format!(
            "target `{}` cannot grow a transition from a fixed edge",
            descriptor.id
        )));
    }
}

fn delayed_appear(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| element.prop("appear_delay").is_some()) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot delay an entrance with `appear_delay`",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn scaled_scene(
    _descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |e| {
        matches!(e.kind.as_str(), "ScaleBox" | "Positioned" | "Marquee")
    }) {
        diagnostics.push(
            Diagnostic::error("this target does not support scaled coordinate scenes")
                .at(element.source.clone()),
        );
    }
}

fn dynamic_text_sizes(
    _descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |e| {
        e.kind == "Text" && matches!(e.prop("font_size"), Some(bern_ir::Prop::Expr { .. }))
    }) {
        diagnostics.push(
            Diagnostic::error("this target does not support dynamic text sizes")
                .at(element.source.clone()),
        );
    }
}

fn accordion_layout(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| element.kind == "Accordion") {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot animate accordion column widths",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn adaptive_images(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| {
        (element.kind == "Image" && element.prop("aspect").is_some())
            || (element.kind == "Grid" && element.prop("row_gap").is_some())
            || (element.kind == "Slider" && element.prop("label_hidden").is_some())
            || (matches!(element.kind.as_str(), "Text" | "Label")
                && element.prop("font_size").is_some())
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot lay out adaptive images or independent grid row spacing",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn page_badges(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |e| {
        e.kind == "SidebarItem" && e.prop("badge").is_some()
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot render live page badges",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn modal_flow_timer(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |e| {
        matches!(e.kind.as_str(), "Modal" | "Flow" | "Timer" | "PickerOption")
            || (e.kind == "Stack" && e.prop("enabled").is_some())
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot render modal, flow, timer or explicit picker options",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn capsule_controls(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| {
        element.kind == "ControlGroup"
            || (matches!(
                element.kind.as_str(),
                "Button" | "IconButton" | "Menu" | "Picker" | "Switch" | "SplitButton"
            ) && (element.prop("shape").is_some() || element.prop("height").is_some()))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot render capsule system controls",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn sidebar_width(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(sidebar) = first_element(app, |e| e.kind == "Sidebar" && e.prop("width").is_some())
    {
        // The sidebar still shows, at the platform's own width.
        diagnostics.push(
            Diagnostic::warning(format!(
                "target `{}` keeps the platform's sidebar width: `width` has no effect there",
                descriptor.id
            ))
            .at(sidebar.source.clone()),
        );
    }
}

fn controlled_values(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(control) = first_element(app, |e| {
        if e.kind == "Slider" {
            // Draft-backed sliders already support commit actions on every target.
            e.prop("current").is_some()
        } else if e.kind == "PopoverButton" {
            e.prop("open").is_some()
        } else {
            matches!(e.kind.as_str(), "Switch" | "Picker")
                && (e.prop("checked").is_some()
                    || e.prop("selected").is_some()
                    || e.prop("label_hidden").is_some()
                    || e.action.is_some())
        }
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot bind controls to engine commands",
                descriptor.id
            ))
            .at(control.source.clone()),
        );
    }
}

fn image_template(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(image) = first_element(app, |element| {
        element.kind == "Image" && element.keyword("rendering") == Some("template")
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot render an `Image` as a template",
                descriptor.id
            ))
            .at(image.source.clone()),
        );
    }
}

fn sidebar_header(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(header) = first_element(app, |element| element.kind == "SidebarHeader") {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot pin a `SidebarHeader` above the sidebar's list",
                descriptor.id
            ))
            .at(header.source.clone()),
        );
    }
}

fn fill_buttons(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(button) = first_element(app, |e| e.kind == "Button" && e.keyword("width").is_some())
    {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot stretch a `Button` across its container (`width: fill`)",
                descriptor.id
            ))
            .at(button.source.clone()),
        );
    }
}

fn modal_cover(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(modal) = first_element(app, |e| e.kind == "Modal" && e.keyword("cover").is_some()) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot let a `Modal` cover the window (`cover: compact`)",
                descriptor.id
            ))
            .at(modal.source.clone()),
        );
    }
}

fn form_layout(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(section) = first_element(app, |e| {
        (e.kind == "Section"
            && (e.keyword("style") == Some("plain")
                || e.prop("padding_x").is_some()
                || e.prop("padding_y").is_some()))
            || (e.kind == "Form"
                && ["gap", "row_height", "padding_x", "padding_y"]
                    .iter()
                    .any(|prop| e.prop(prop).is_some()))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot lay out a form's sections (`Section(style: plain)`, their `padding_x` / `padding_y`, `Form(gap:)`)",
                descriptor.id
            ))
            .at(section.source.clone()),
        );
    }
}

fn page_search(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(search) = first_page_search(app) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot place a `Search` in the page; put it in a `Toolbar` or `Sidebar`",
                descriptor.id
            ))
            .at(search.source.clone()),
        );
    }
}

fn tab_accessory(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(accessory) = first_element(app, |e| e.kind == "TabAccessory") {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot keep a `TabAccessory` with its tabs",
                descriptor.id
            ))
            .at(accessory.source.clone()),
        );
    }
}

fn grid_cells(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(grid) = first_element(app, |e| e.kind == "Grid" && e.prop("cell_ratio").is_some()) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot size the rows of a `Grid` by its column width (`cell_ratio`)",
                descriptor.id
            ))
            .at(grid.source.clone()),
        );
    }
}

fn window_short(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reads_window_field(app, "short") {
        diagnostics.push(Diagnostic::error(format!(
            "target `{}` cannot tell how tall the window is (`window.short`)",
            descriptor.id
        )));
    }
}

fn window_landscape(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(window) = app.windows.windows.iter().find(|w| w.landscape.is_some()) {
        diagnostics.push(Diagnostic::warning(format!(
            "target `{}` keeps window `{}` in the device's orientation: `landscape` has no effect there",
            descriptor.id, window.id
        )));
    }
}

fn idle_control(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(window) = app
        .windows
        .windows
        .iter()
        .find(|w| w.idle_enabled.is_some())
    {
        diagnostics.push(Diagnostic::warning(format!(
            "target `{}` ignores `idle_enabled` of window `{}`: once `idle_after` passes without input, `window.idle` turns on whatever the state",
            descriptor.id, window.id
        )));
    }
}

fn untitled_detail(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(detail) = first_element(app, |element| {
        element.kind == "Detail" && element.prop("titled").is_some()
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` always titles the detail column with the selected destination",
                descriptor.id
            ))
            .at(detail.source.clone()),
        );
    }
}

fn popover_button(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(button) = first_element(app, |element| element.kind == "PopoverButton") {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot build a `PopoverButton`",
                descriptor.id
            ))
            .at(button.source.clone()),
        );
    }
}

fn size_classes(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let layout = first_element(app, |element| {
        element.kind == "Layout"
            && matches!(element.keyword("size"), Some("medium") | Some("large"))
    });
    if let Some(layout) = layout {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` has only the `compact` and `expanded` layout sizes",
                descriptor.id
            ))
            .at(layout.source.clone()),
        );
    }
    let mut reading = false;
    app.walk(&mut |node, _| reading |= reads_window_size(node));
    if reading {
        diagnostics.push(Diagnostic::error(format!(
            "target `{}` cannot read the window's size with `window.compact`, `window.medium`, `window.expanded` or `window.large`",
            descriptor.id
        )));
    }
}

fn button_content(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(button) = first_element(app, |element| {
        element.kind == "Button" && !element.children.is_empty()
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot build a `Button` that shows content instead of its label",
                descriptor.id
            ))
            .at(button.source.clone()),
        );
    }
}

fn toolbar_search(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(toolbar) = first_element(app, |element| {
        element.kind == "Toolbar"
            && element.children.iter().any(
                |child| matches!(child, bern_ir::Node::Element(search) if search.kind == "Search"),
            )
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot build a `Search` in a `Toolbar`",
                descriptor.id
            ))
            .at(toolbar.source.clone()),
        );
    }
}

fn search_results(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(search) = first_element(app, |element| {
        element.kind == "Search" && !element.children.is_empty()
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot show results below a `Search`",
                descriptor.id
            ))
            .at(search.source.clone()),
        );
    }
}

fn session_actions(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| {
        matches!(element.action, Some(bern_ir::Action::Session { .. }))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot build actions that change session drafts",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn toggle_button(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| {
        (element.kind == "Switch" && element.keyword("style") == Some("button"))
            || (element.kind == "Toolbar"
                && element.children.iter().any(
                    |child| matches!(child, bern_ir::Node::Element(switch) if switch.kind == "Switch"),
                ))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot build a `Switch` as a toggle button or in a `Toolbar`",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn dialog_windows(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(window) = app
        .windows
        .windows
        .iter()
        .find(|window| window.role == bern_ir::WindowRole::Dialog)
    {
        diagnostics.push(Diagnostic::warning(format!(
            "target `{}` shows dialog window `{}` as a window of its own, not over the primary window",
            descriptor.id, window.id
        )));
    }
}

fn bound_lengths(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(element) = first_element(app, |element| {
        let spec = bern_ir::spec::component(&element.kind);
        element.props.iter().any(|(name, prop)| {
            (matches!(prop, bern_ir::Prop::Expr { .. })
                && spec
                    .and_then(|spec| spec.prop(name))
                    .is_some_and(|prop| matches!(prop.kind, bern_ir::spec::PropKind::Length)))
                || (element.kind == "Video" && name == "corner")
                || (matches!(element.kind.as_str(), "Column" | "Row" | "Stack")
                    && matches!(name.as_str(), "width" | "height"))
        })
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot build lengths that follow state, sizes of `Column`, `Row` and `Stack`, nor round a `Video`",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn appearance(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(element) = first_element(app, |element| element.props.contains_key("appearance")) {
        diagnostics.push(
            Diagnostic::warning(format!(
                "target `{}` shows `appearance` parts in the window's own appearance",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn shortcuts(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(element) = first_element(app, |element| element.props.contains_key("shortcut")) {
        diagnostics.push(
            Diagnostic::warning(format!(
                "target `{}` ignores keyboard `shortcut`s",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn runtime_theme(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if app.windows.accent.is_some() || app.windows.appearance.is_some() {
        diagnostics.push(Diagnostic::warning(format!(
            "target `{}` keeps the system's accent and appearance; `accent` and `appearance` of the `WindowSet` have no effect there",
            descriptor.id
        )));
    }
}

fn runtime_images(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(image) = first_element(app, |element| {
        element.kind == "Image"
            && matches!(element.prop("source"), Some(bern_ir::Prop::Expr { .. }))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot show pictures from a `MediaSource`, only declared resources",
                descriptor.id
            ))
            .at(image.source.clone()),
        );
    }
}

fn file_pick(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(button) = first_element(app, |element| element.props.contains_key("pick")) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot ask for a file with `pick:`",
                descriptor.id
            ))
            .at(button.source.clone()),
        );
    }
}

fn scroll_snap(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(scroll) = first_element(app, |element| {
        element.kind == "ScrollView"
            && (element.props.contains_key("snap") || element.props.contains_key("position"))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot snap a scroll view to its items or follow its `position`",
                descriptor.id
            ))
            .at(scroll.source.clone()),
        );
    }
}

fn picture_fills(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(shape) = first_element(app, |element| {
        element.kind == "Shape"
            && (element.props.contains_key("fill_from") || element.props.contains_key("fade"))
    }) {
        diagnostics.push(
            Diagnostic::error(format!(
                "target `{}` cannot fill a shape from a picture or fade it",
                descriptor.id
            ))
            .at(shape.source.clone()),
        );
    }
}

fn reveal(descriptor: &TargetDescriptor, app: &Application, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(element) = first_element(app, |element| element.props.contains_key("reveal")) {
        diagnostics.push(
            Diagnostic::warning(format!(
                "target `{}` shows content with `reveal: hover` all the time",
                descriptor.id
            ))
            .at(element.source.clone()),
        );
    }
}

fn runtime_palette(
    descriptor: &TargetDescriptor,
    app: &Application,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if app.windows.palette.is_some() {
        diagnostics.push(Diagnostic::error(format!(
            "target `{}` does not support WindowSet(palette:)",
            descriptor.id
        )));
    }
}
