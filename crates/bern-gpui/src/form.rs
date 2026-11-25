//! Forms: a scrolling page of grouped sections, each a card of rows with
//! an optional title above and footer below; a plain section sets its rows
//! on the page itself. Lowered through `bern-surface::controls::{form, section}`.

use crate::El;
use crate::ctx::Ctx;
use crate::layout::{Style, styled};
use crate::surface::{GpuiRenderer, apply_box, as_box};
use crate::units::px;
use bern_surface::controls::form::{
    SECTION_ROW_MIN_HEIGHT, SECTION_ROW_PADDING_X, SECTION_ROW_PADDING_Y,
};
use bern_surface::controls::{FormSpec, SectionSpec, form as ir_form, section as ir_section};
use gpui::prelude::FluentBuilder;
use gpui::{
    FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div,
};

/// How a `Form` lays out its sections and rows.
#[derive(Clone, Copy, Debug, Default)]
pub struct FormProps {
    pub gap: f32,
    /// The least height of a row.
    pub row_height: Option<f32>,
    /// The rows' insets in every section.
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub style: Option<&'static Style>,
}

/// `Form`: sections down a page that scrolls, under the page's title. In a
/// `Modal` there is no page, and the title is not shown, as on the web.
pub fn form(c: &Ctx, props: FormProps, title: Option<String>, children: Vec<El>) -> El {
    let title = title.filter(|_| !c.in_modal_content());
    let ir = ir_form(FormSpec {
        title: title.clone(),
        gap: props.gap,
    });
    let element = as_box(&ir)
        .map(|b| apply_box(c, div(), b))
        .unwrap_or_else(div)
        .id(c.element_id("form"))
        .overflow_y_scroll()
        .children(title.map(|title| {
            div()
                .pb(px(4.0))
                .text_size(px(24.0))
                .line_height(px(32.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title)
        }))
        .children(children);
    let element = styled(c, div(), props.style)
        .flex()
        .flex_col()
        .size_full()
        .child(element);
    c.faded(c.grow().apply(element)).into_any_element()
}

/// How a `Section` shows its rows.
#[derive(Clone, Copy, Debug, Default)]
pub struct SectionProps {
    /// Rows on the page itself, without the card, as a heading.
    pub plain: bool,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
}

/// `Section`: its rows on a card, a hairline between them.
pub fn section(
    c: &Ctx,
    title: Option<String>,
    footer: Option<String>,
    props: SectionProps,
    rows: Vec<El>,
) -> El {
    let form = c.form();
    // In a sidebar a section is a titled group of its items.
    let props = if c.sidebar().is_some() {
        SectionProps {
            plain: true,
            ..props
        }
    } else {
        props
    };
    let separator = c.color("separator");
    let count = rows.len();
    let padding_x = props
        .padding_x
        .or(form.padding_x)
        .unwrap_or(SECTION_ROW_PADDING_X);
    let padding_y = props
        .padding_y
        .or(form.padding_y)
        .unwrap_or(SECTION_ROW_PADDING_Y);
    let row_height = form.row_height.unwrap_or(SECTION_ROW_MIN_HEIGHT);
    let rows: Vec<El> = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let row = div().flex().flex_col().justify_center().child(row);
            let row = if props.plain {
                row.py(px(4.0))
            } else {
                row.min_h(px(row_height))
                    .px(px(padding_x))
                    .py(px(padding_y))
                    .when(index + 1 < count, |row| {
                        row.border_b_1().border_color(separator)
                    })
            };
            row.into_any_element()
        })
        .collect();
    let ir = ir_section(SectionSpec {
        title,
        footer,
        plain: props.plain,
        padding_x: Some(padding_x),
        padding_y: Some(padding_y),
        row_height: Some(row_height),
    });
    GpuiRenderer::new(c).slot("rows", rows).render(&ir)
}
