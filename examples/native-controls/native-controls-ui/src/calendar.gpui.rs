// Co-located GPUI native interactive month calendar grid.

use super::{Actions, Props, Theme};
use bern_gpui::gpui::{self, InteractiveElement, IntoElement, ParentElement, Styled, div, px, rgb};

fn parse_ymd(iso: &str) -> (i32, u32, u32) {
    let mut parts = iso.split('-');
    let y = parts.next().and_then(|s| s.parse().ok()).unwrap_or(2026);
    let m = parts.next().and_then(|s| s.parse().ok()).unwrap_or(11);
    let d = parts.next().and_then(|s| s.parse().ok()).unwrap_or(19);
    (y, m.clamp(1, 12), d.clamp(1, 31))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if leap { 29 } else { 28 },
        _ => 30,
    }
}

pub fn render(props: Props, actions: Actions, theme: Theme) -> impl IntoElement {
    let (year, month, selected_day) = parse_ymd(&props.date);
    let total_days = days_in_month(year, month);
    let on_today = actions.today.clone();

    let mut grid = div().flex().flex_col().gap(px(4.0));
    let mut day = 1u32;
    while day <= total_days {
        let mut week = div().flex().flex_row().gap(px(4.0));
        for _ in 0..7 {
            if day <= total_days {
                let d = day;
                let selected = d == selected_day;
                let set_date = actions.set_date.clone();
                let iso = format!("{year:04}-{month:02}-{d:02}");
                week = week.child(
                    div()
                        .id(("cal-day", d as usize))
                        .flex_1()
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .bg(if selected {
                            theme.accent
                        } else if theme.dark {
                            rgb(0x232329)
                        } else {
                            rgb(0xf3f4f6)
                        })
                        .text_color(if selected {
                            rgb(0xffffff)
                        } else {
                            theme.text_primary
                        })
                        .cursor_pointer()
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                            set_date(iso.clone());
                        })
                        .child(d.to_string()),
                );
                day += 1;
            } else {
                week = week.child(div().flex_1().h(px(28.0)));
            }
        }
        grid = grid.child(week);
    }

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
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(10.0))
                        .items_center()
                        .child(div().text_color(theme.text_secondary).child(props.date))
                        .child(
                            div()
                                .id("cal-today")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border)
                                .text_color(theme.text_primary)
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                                    on_today();
                                })
                                .child(props.today_label),
                        ),
                ),
        )
        .child(grid)
}
