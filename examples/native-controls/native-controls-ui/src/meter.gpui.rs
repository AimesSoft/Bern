// Co-located GPUI native segmented level meter with pointer hover & click.

use super::{Actions, Props, Theme};
use bern_gpui::gpui::{self, InteractiveElement, IntoElement, ParentElement, Styled, div, px, rgb};

pub fn render(props: Props, actions: Actions, theme: Theme) -> impl IntoElement {
    let steps = 10usize;
    let active_color = if props.boosted {
        rgb(0xf97316)
    } else {
        theme.accent
    };
    let inactive_color = if theme.dark {
        rgb(0x2a2a30)
    } else {
        rgb(0xe5e7eb)
    };

    let mut bars = div().flex().flex_row().gap(px(6.0)).items_end().h(px(44.0));
    for idx in 1..=steps {
        let target_val = idx as f64;
        let filled = props.level + 1e-6 >= target_val;
        let hovered = props.hover.is_some_and(|h| (h - target_val).abs() < 0.5);
        let height = px(14.0 + (idx as f32) * 3.0);
        let set_level = actions.set_level.clone();
        let set_hover = actions.set_hover.clone();
        bars = bars.child(
            div()
                .id(("meter-seg", idx))
                .flex_1()
                .h(height)
                .rounded(px(4.0))
                .bg(if filled || hovered {
                    active_color
                } else {
                    inactive_color
                })
                .cursor_pointer()
                .on_hover(move |over, _, _| {
                    if *over {
                        set_hover(Some(target_val));
                    } else {
                        set_hover(None);
                    }
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                    set_level(target_val);
                }),
        );
    }

    let toggle_boost = actions.set_boosted.clone();
    let next_boost = !props.boosted;
    let on_reset = actions.reset.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .p(px(14.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(div().text_color(theme.text_primary).child(props.heading))
                .child(div().text_color(theme.text_secondary).child(props.readout)),
        )
        .child(bars)
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .id("meter-boost")
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(if props.boosted {
                            active_color
                        } else {
                            theme.border
                        })
                        .text_color(theme.text_primary)
                        .cursor_pointer()
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                            toggle_boost(next_boost);
                        })
                        .child(props.boosted_label),
                )
                .child(
                    div()
                        .id("meter-reset")
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(theme.border)
                        .text_color(theme.text_primary)
                        .cursor_pointer()
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                            on_reset();
                        })
                        .child(props.reset_label),
                ),
        )
}
