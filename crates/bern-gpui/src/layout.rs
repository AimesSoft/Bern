//! Containers: columns, rows, flows, layers, scrolling, adaptive layouts
//! and tabs.
//!
//! Some elements take the space they are given, as a scroll view, a
//! spacer, a divider or a text field do; a container holding one takes it
//! too. The generator works this out from the declaration and tells each
//! element through its context ([`Grow`]), so containers keep their
//! content's size unless something in them wants more.

use crate::ctx::Ctx;
use crate::theme::rgba;
use crate::units::px;
use crate::{Color, El};
use gpui::prelude::FluentBuilder;
use gpui::{
    BoxShadow, Div, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, hsla, point,
};

/// How an element takes the space its container gives: along the
/// container's axis (`flex`), and across its width or height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Grow {
    pub flex: bool,
    pub width: bool,
    pub height: bool,
}

impl Grow {
    pub const NONE: Grow = Grow {
        flex: false,
        width: false,
        height: false,
    };
    pub const ALL: Grow = Grow {
        flex: true,
        width: true,
        height: true,
    };

    pub(crate) fn apply<E: Styled>(self, element: E) -> E {
        let element = if self.flex {
            element.flex_1().min_w(px(0.0)).min_h(px(0.0))
        } else {
            element
        };
        let element = if self.width {
            element.w_full()
        } else {
            element
        };
        if self.height {
            element.h_full()
        } else {
            element
        }
    }
}

/// A length: the content's, a number of points, or all the space given.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Len {
    #[default]
    Auto,
    Fixed(f32),
}

/// Padding: on every edge, or across and down, which win.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pad {
    pub all: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
}

/// Where children sit across a column or row, or where a layer sits in
/// a `Stack`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Baseline,
    TopLeading,
    Top,
    TopTrailing,
    Leading,
    Trailing,
    BottomLeading,
    Bottom,
    BottomTrailing,
}

/// A `style!` rule for a container's surface.
#[derive(Clone, Copy, Debug, Default)]
pub struct Style {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub corner: Option<f32>,
    /// Blur radius, offset down and opacity of the shadow.
    pub shadow: Option<(f32, f32, f32)>,
}

impl Style {
    /// No rule: the base generated rules fill in.
    pub const NONE: Style = Style {
        background: None,
        foreground: None,
        corner: None,
        shadow: None,
    };
}

/// The color of `color` in the context's appearance.
pub(crate) fn color(c: &Ctx, color: Color) -> gpui::Rgba {
    match color {
        Color::Role(role) => c.color(role),
        Color::Fixed { light, dark } => {
            let (red, green, blue) = if c.theme().dark { dark } else { light };
            rgba(bern_theme::Srgb::new(red, green, blue), 0xff)
        }
    }
}

pub(crate) fn styled(c: &Ctx, element: Div, style: Option<&Style>) -> Div {
    let Some(style) = style else {
        return element;
    };
    let mut element = element;
    if let Some(background) = style.background {
        // Under a video the surface paints nothing, so the video shows.
        let window = c.win().id.clone();
        element = if crate::media::has_video(&window) {
            element.child(crate::media::backdrop(
                &window,
                color(c, background),
                style.corner.unwrap_or(0.0) * crate::units::scale(),
            ))
        } else {
            element.bg(color(c, background))
        };
    }
    if let Some(foreground) = style.foreground {
        element = element.text_color(color(c, foreground));
    }
    if let Some(corner) = style.corner {
        element = element.rounded(px(corner)).overflow_hidden();
    }
    if let Some((radius, y, opacity)) = style.shadow {
        element = element.shadow(vec![BoxShadow {
            color: hsla(0.0, 0.0, 0.0, opacity),
            offset: point(px(0.0), px(y)),
            blur_radius: px(radius),
            spread_radius: px(0.0),
            inset: false,
        }]);
    }
    element
}

/// Spacing, size and alignment of a column, row or flow.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flex {
    pub gap: f32,
    pub pad: Pad,
    pub width: Len,
    pub height: Len,
    pub align: Align,
}

fn sized(element: Div, width: Len, height: Len) -> Div {
    let element = match width {
        Len::Fixed(width) => element.w(px(width)).flex_none(),
        Len::Auto => element,
    };
    match height {
        Len::Fixed(height) => element.h(px(height)),
        Len::Auto => element,
    }
}

fn padded(element: Div, pad: Pad) -> Div {
    let x = pad.x.unwrap_or(pad.all);
    let y = pad.y.unwrap_or(pad.all);
    element.px(px(x)).py(px(y))
}

/// The group of containers whose `reveal: hover` content shows while the
/// pointer is over them.
pub(crate) const REVEAL: &str = "bern-reveal";

/// Content shown only while the pointer is over its container.
pub fn reveal(c: &Ctx, content: El) -> El {
    c.grow()
        .apply(
            div()
                .flex()
                .flex_col()
                .opacity(0.0)
                .group_hover(REVEAL, |style| style.opacity(1.0))
                .child(content),
        )
        .into_any_element()
}

fn flex(c: &Ctx, props: Flex, children: Vec<El>) -> Div {
    // A column or row is never wider than its container and may be
    // narrower than its content's single lines, so the text in it wraps
    // instead of running past the container (GPUI measures text's
    // narrowest width as its single line).
    let element = div()
        .flex()
        .min_w(px(0.0))
        .max_w(gpui::relative(1.0))
        .gap(px(props.gap));
    let element = if c.reveals() {
        element.group(REVEAL)
    } else {
        element
    };
    let element = match props.align {
        Align::Center => element.items_center(),
        Align::End | Align::Trailing | Align::Bottom => element.items_end(),
        Align::Baseline => element.items_baseline(),
        _ => element.items_start(),
    };
    let element = padded(element, props.pad);
    let element = c.grow().apply(element);
    sized(element, props.width, props.height).children(children)
}

/// `Column`: children top to bottom.
pub fn column(c: &Ctx, props: Flex, children: Vec<El>) -> El {
    flex(c, props, children).flex_col().into_any_element()
}

/// `Row`: children from the leading edge.
pub fn row(c: &Ctx, props: Flex, children: Vec<El>) -> El {
    flex(c, props, children).flex_row().into_any_element()
}

/// `Flow`: children from the leading edge, wrapping onto new lines.
pub fn flow(c: &Ctx, props: Flex, children: Vec<El>) -> El {
    flex(c, props, children)
        .flex_row()
        .flex_wrap()
        .into_any_element()
}

/// `Spacer`: takes the free space along its container.
pub fn spacer(_c: &Ctx) -> El {
    div().flex_1().into_any_element()
}

/// `Divider`: a hairline across its container.
pub fn divider(c: &Ctx) -> El {
    div()
        .flex_none()
        .w_full()
        .h(px(1.0))
        .bg(c.color("separator"))
        .into_any_element()
}

/// Size, alignment and surface of a `Stack`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Layers {
    /// Fill the window to its edges, under a transparent title bar.
    pub under_bars: bool,
    pub align: Align,
    pub width: Len,
    pub height: Len,
    pub style: Option<&'static Style>,
}

/// `Stack`: layers on top of each other in one cell of a grid, later ones
/// above earlier ones, the stack as large as the largest; each layer sits
/// where `align` says. A canvas without a size fills the stack without
/// widening it.
pub fn stack(c: &Ctx, props: Layers, children: Vec<El>) -> El {
    let element = styled(c, div().grid().grid_cols(1).grid_rows(1), props.style);
    let element = if c.reveals() {
        element.group(REVEAL)
    } else {
        element
    };
    let element = if props.under_bars && c.win().title_bar > 0.0 {
        let top = c.win().title_bar;
        element
            .mt(px(-top))
            .pt(px(0.0))
            .min_h(px(c.win().height.get() + top))
    } else {
        element
    };
    let element = c.grow().apply(element);
    let element = sized(element, props.width, props.height);
    let layers = children.into_iter().map(|layer| {
        let place = div()
            .row_start(1)
            .col_start(1)
            .flex()
            .flex_col()
            .min_w(px(0.0))
            .min_h(px(0.0));
        let place = match props.align {
            Align::TopLeading | Align::Start => place.justify_start().items_start(),
            Align::Top => place.justify_start().items_center(),
            Align::TopTrailing => place.justify_start().items_end(),
            Align::Leading => place.justify_center().items_start(),
            Align::Center | Align::Baseline => place.justify_center().items_center(),
            Align::Trailing => place.justify_center().items_end(),
            Align::BottomLeading => place.justify_end().items_start(),
            Align::Bottom => place.justify_end().items_center(),
            Align::BottomTrailing | Align::End => place.justify_end().items_end(),
        };
        place.child(layer)
    });
    element.children(layers).into_any_element()
}

/// `ScrollView`: its content, scrolling down, or across with
/// `horizontal`, when it does not fit.
pub fn scroll(c: &Ctx, id: &'static str, horizontal: bool, children: Vec<El>) -> El {
    crate::scroll::scroll_view(
        c,
        id,
        crate::scroll::ScrollProps {
            horizontal,
            ..Default::default()
        },
        children,
    )
}

/// The layout sizes of `Adaptive`, smallest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    Compact,
    Medium,
    Expanded,
    Large,
}

impl Size {
    fn current(c: &Ctx) -> Size {
        let is = |name: &str| crate::value::truthy(&c.window(name));
        if is("compact") {
            Size::Compact
        } else if is("medium") {
            Size::Medium
        } else if is("expanded") {
            Size::Expanded
        } else {
            Size::Large
        }
    }
}

/// Content built only if it is chosen.
pub type Later<'a> = Box<dyn FnOnce() -> Vec<El> + 'a>;

/// `Adaptive`: the layout declared for the window's size, or the largest
/// declared one below it, or the smallest declared.
pub fn adaptive(c: &Ctx, layouts: Vec<(Size, Later<'_>)>) -> El {
    let current = Size::current(c);
    let chosen = layouts
        .iter()
        .map(|(size, _)| *size)
        .filter(|size| *size <= current)
        .max()
        .or_else(|| layouts.iter().map(|(size, _)| *size).min());
    let children = layouts
        .into_iter()
        .find(|(size, _)| Some(*size) == chosen)
        .map(|(_, build)| build())
        .unwrap_or_default();
    let element = div().flex().flex_col().children(children);
    c.grow().apply(element).into_any_element()
}

/// `Tabs`: the selected tab's content over a bar of the tabs, each an icon
/// over its label. `select` runs with the tab's value when one is chosen.
pub fn tabs(
    c: &Ctx,
    id: &'static str,
    tabs: Vec<(crate::protocol::Value, String, Option<&'static str>, bool)>,
    selected: crate::protocol::Value,
    model: crate::Model,
    content: Vec<El>,
    accessory: Option<Vec<El>>,
) -> El {
    let bar =
        div()
            .flex()
            .flex_row()
            .flex_none()
            .w_full()
            .h(px(56.0))
            .border_t_1()
            .border_color(c.color("separator"))
            .bg(c.color("surface.panel"))
            .children(tabs.into_iter().enumerate().map(
                |(index, (value, label, icon, icon_only))| {
                    let chosen = value == selected;
                    let color = if chosen {
                        c.color("accent.text")
                    } else {
                        c.color("text.secondary")
                    };
                    let target = format!("{id}.{index}");
                    let ctx = c.clone();
                    let model = model.clone();
                    let pick = value.clone();
                    let choose: std::rc::Rc<dyn Fn()> =
                        std::rc::Rc::new(move || model.set(&ctx, pick.clone()));
                    c.list(crate::session::Control {
                        target: c.target(&target),
                        window: c.win().id.clone(),
                        role: "tab",
                        label: label.clone(),
                        value: Some(chosen.to_string()),
                        enabled: c.enabled(),
                        actions: crate::session::Actions {
                            activate: Some(std::rc::Rc::new({
                                let choose = choose.clone();
                                move |_: &mut gpui::App| choose()
                            })),
                            ..Default::default()
                        },
                    });
                    div()
                        .id(c.element_id(&target))
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(2.0))
                        .text_color(color)
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .children(icon.map(|icon| crate::icons::icon(icon, 22.0, color)))
                        // With `display: icon` the label is only the accessible name.
                        .when(!(icon_only && icon.is_some()), |tab| {
                            tab.child(label.clone())
                        })
                        .on_click(move |_, _, _| choose())
                },
            ));
    let element = div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .children(content),
        )
        .children(accessory.map(|accessory| {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .border_t_1()
                .border_color(c.color("separator"))
                .bg(c.color("surface.panel"))
                .children(accessory)
        }))
        .child(bar);
    Grow::ALL.apply(element).into_any_element()
}

thread_local! {
    /// The width each grid was last given, by target.
    static GRID_WIDTHS: std::cell::RefCell<std::collections::HashMap<String, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// How a `Grid` lays out its children.
#[derive(Clone, Copy, Debug, Default)]
pub struct GridProps {
    pub min_width: f32,
    pub gap: f32,
    pub row_gap: Option<f32>,
    pub pad: Pad,
    /// Rows the column width divided by this, plus `cell_extra`, tall.
    pub cell_ratio: Option<f32>,
    pub cell_extra: f32,
}

/// `Grid`: equal columns, as many as fit at least `min_width` wide, filled
/// row by row; with `cell_ratio`, rows of a height that follows the column
/// width, their content at the top. A masonry grid lays out as rows here.
pub fn grid(c: &Ctx, id: &str, props: GridProps, children: Vec<El>) -> El {
    let target = c.target(id);
    let given = GRID_WIDTHS
        .with(|widths| widths.borrow().get(&target).copied())
        .unwrap_or(c.win().width.get());
    let scale = crate::units::scale();
    let inner = given / scale - props.pad.x.unwrap_or(props.pad.all) * 2.0;
    let columns = (((inner + props.gap) / (props.min_width + props.gap)).floor() as usize).max(1);
    let column_width = (inner - props.gap * (columns as f32 - 1.0)) / columns as f32;
    let row_height = props
        .cell_ratio
        .filter(|ratio| *ratio > 0.0)
        .map(|ratio| column_width / ratio + props.cell_extra);
    let measured = target.clone();
    let element = div()
        .relative()
        .grid()
        .grid_cols(columns as u16)
        .gap_x(px(props.gap))
        .gap_y(px(props.row_gap.unwrap_or(props.gap)))
        .child(
            gpui::canvas(
                move |bounds, _, _| {
                    let width = f32::from(bounds.size.width);
                    let changed = GRID_WIDTHS.with(|widths| {
                        widths.borrow_mut().insert(measured.clone(), width) != Some(width)
                    });
                    if changed {
                        crate::host::redraw();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .children(children.into_iter().map(|child| {
            let cell = div().flex().flex_col().min_w(px(0.0));
            match row_height {
                Some(height) => cell.h(px(height)).child(child),
                None => cell.child(child),
            }
        }));
    let element = padded(element, props.pad);
    c.grow().apply(element.w_full()).into_any_element()
}

thread_local! {
    /// The height each preview list was last given, by target.
    static PREVIEW_HEIGHTS: std::cell::RefCell<std::collections::HashMap<String, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// How a `PreviewList` lays out its rows.
#[derive(Clone, Copy, Debug, Default)]
pub struct PreviewProps {
    pub item_height: f32,
    pub gap: f32,
    /// The height of the `+N` badge for the rows left out.
    pub more_height: f32,
}

/// `PreviewList`: as many whole rows as its height holds, then a `+N`
/// badge for the rest when not all fit; it does not scroll.
pub fn preview_list(
    c: &Ctx,
    id: &str,
    items: Vec<crate::protocol::Value>,
    key: usize,
    props: PreviewProps,
    row: &dyn Fn(&Ctx, crate::protocol::Value) -> Vec<El>,
) -> El {
    let target = c.target(id);
    let height = PREVIEW_HEIGHTS.with(|heights| heights.borrow().get(&target).copied());
    let step = props.item_height + props.gap;
    let fits = |room: f32| (((room + props.gap) / step).floor().max(0.0)) as usize;
    let total = items.len();
    let shown = match height {
        // Before it is measured, every row.
        None => total,
        Some(height) if fits(height / crate::units::scale()) >= total => total,
        Some(height) => fits(height / crate::units::scale() - props.more_height - props.gap),
    };
    let measured = target.clone();
    let rows = items.into_iter().take(shown).map(|item| {
        let key = crate::value::at(item.clone(), &[key]);
        let c = c.with_item(&key);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .h(px(props.item_height))
            .overflow_hidden()
            .children(row(&c, item))
    });
    let element = div()
        .relative()
        .flex()
        .flex_col()
        .gap(px(props.gap))
        .overflow_hidden()
        .children(rows)
        .when(shown < total, |element| {
            element.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .h(px(props.more_height))
                    .text_size(px(12.0))
                    .text_color(c.color("text.secondary"))
                    .child(format!("+{}", total - shown)),
            )
        })
        .child(
            gpui::canvas(
                move |bounds, _, _| {
                    let height = f32::from(bounds.size.height);
                    let changed = PREVIEW_HEIGHTS.with(|heights| {
                        heights.borrow_mut().insert(measured.clone(), height) != Some(height)
                    });
                    if changed {
                        crate::host::redraw();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );
    Grow::ALL.apply(element).into_any_element()
}

/// How an `Accordion` lays out its columns.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccordionProps {
    pub selected: usize,
    pub collapsed_width: f32,
    pub height: Option<f32>,
    pub gap: f32,
    pub corner: f32,
}

/// `Accordion`: columns side by side, the chosen one taking the width the
/// others, each `collapsed_width` wide, leave.
pub fn accordion(c: &Ctx, props: AccordionProps, columns: Vec<El>) -> El {
    let columns = columns.into_iter().enumerate().map(|(index, column)| {
        let element = div()
            .flex()
            .flex_col()
            .h_full()
            .rounded(px(props.corner))
            .overflow_hidden()
            .child(column);
        if index == props.selected {
            element.flex_1().min_w(px(0.0))
        } else {
            element.flex_none().w(px(props.collapsed_width))
        }
    });
    let element = div().flex().flex_row().gap(px(props.gap)).children(columns);
    let element = match props.height {
        Some(height) => element.h(px(height)).w_full(),
        None => c.grow().apply(element.size_full()),
    };
    element.into_any_element()
}
