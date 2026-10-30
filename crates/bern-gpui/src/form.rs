//! Forms: a scrolling page of grouped sections, each a card of rows with
//! an optional title above and footer below; a plain section sets its rows
//! on the page itself.

use crate::El;
use crate::ctx::Ctx;
use crate::layout::{Style, styled};
use crate::units::px;
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

/// `Form`: sections down a page that scrolls, under the page's title.
pub fn form(c: &Ctx, props: FormProps, title: Option<String>, children: Vec<El>) -> El {
    let element = div()
        .id(c.element_id("form"))
        .flex()
        .flex_col()
        .gap(px(props.gap))
        .p(px(16.0))
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
    c.grow().apply(element).into_any_element()
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
    let caption = |text: String| {
        div()
            .px(px(4.0))
            .text_size(px(12.0))
            .line_height(px(16.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c.color("text.secondary"))
            .child(text)
    };
    let separator = c.color("separator");
    let count = rows.len();
    let padding_x = props.padding_x.or(form.padding_x).unwrap_or(16.0);
    let padding_y = props.padding_y.or(form.padding_y).unwrap_or(8.0);
    let row_height = form.row_height.unwrap_or(44.0);
    let rows = rows.into_iter().enumerate().map(|(index, row)| {
        let row = div().flex().flex_col().justify_center().child(row);
        if props.plain {
            return row.py(px(4.0));
        }
        row.min_h(px(row_height))
            .px(px(padding_x))
            .py(px(padding_y))
            .when(index + 1 < count, |row| {
                row.border_b_1().border_color(separator)
            })
    });
    let body = div().flex().flex_col().w_full().children(rows);
    let body = if props.plain {
        body
    } else {
        body.rounded(px(10.0))
            .border_1()
            .border_color(separator)
            .bg(c.color("surface.raised"))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .w_full()
        .children(title.map(caption))
        .child(body)
        .children(footer.map(caption))
        .into_any_element()
}
