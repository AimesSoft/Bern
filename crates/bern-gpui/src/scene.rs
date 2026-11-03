//! Scenes: `ScaleBox`, a fixed coordinate space fitted into the space it is
//! given; `Positioned`, a layer placed in the coordinates of its scene or
//! container; and `Rail`, a strip that places its children at fractions of
//! its length.

use crate::El;
use crate::ctx::Ctx;
use crate::units::{self, px};
use gpui::{IntoElement, ParentElement, Styled, canvas, div, relative};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// The size each scale box was last given, by target.
    static GIVEN: RefCell<HashMap<String, (f32, f32)>> = RefCell::new(HashMap::new());
    /// The margins between the scene being built and its box: left, top.
    static MARGIN: RefCell<(f32, f32)> = const { RefCell::new((0.0, 0.0)) };
}

/// `ScaleBox`: a `width` × `height` scene, fitted whole into the space the
/// box is given and centred in it; everything in it is measured in the
/// scene's scale.
pub fn scale_box(
    c: &Ctx,
    id: &str,
    width: f32,
    height: f32,
    build: impl FnOnce() -> Vec<El>,
) -> El {
    let target = c.key(id);
    let window = (c.win().width.get(), c.win().height.get());
    let given = GIVEN
        .with(|given| given.borrow().get(&target).copied())
        .unwrap_or(window);
    let outer = units::scale();
    let available = (given.0 / outer, given.1 / outer);
    let factor = if width > 0.0 && height > 0.0 {
        (available.0 / width).min(available.1 / height).max(0.01)
    } else {
        1.0
    };
    let margin = (
        ((available.0 - width * factor) / 2.0).max(0.0) / factor,
        ((available.1 - height * factor) / 2.0).max(0.0) / factor,
    );
    let saved = MARGIN.with(|current| current.replace(margin));
    let children = units::scaled(factor, build);
    MARGIN.with(|current| *current.borrow_mut() = saved);
    let scene = units::scaled(factor, || {
        div()
            .relative()
            .flex_none()
            .w(px(width))
            .h(px(height))
            .children(children)
    });
    let measured = target.clone();
    let element = div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .child(
            canvas(
                move |bounds, _, _| {
                    let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let changed = GIVEN.with(|given| {
                        given.borrow_mut().insert(measured.clone(), size) != Some(size)
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
        .child(scene);
    c.grow().apply(element).into_any_element()
}

/// Where a `Positioned` layer is, in points of its coordinate space.
#[derive(Clone, Copy, Debug, Default)]
pub struct Place {
    pub x: f32,
    pub y: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Continue past each scene edge it reaches to the edge of the box.
    pub bleed: bool,
    /// Where its motion has it now.
    pub pose: crate::Pose,
}

/// `Positioned`: its content at a place from the top leading corner of the
/// scene or container it stands in.
pub fn positioned(c: &Ctx, place: Place, children: Vec<El>) -> El {
    let (mut x, mut y) = (place.x, place.y);
    let (mut width, mut height) = (place.width, place.height);
    if place.bleed {
        let (left, top) = MARGIN.with(|margin| *margin.borrow());
        if x <= 0.0 {
            width = width.map(|width| width - x + left);
            x = -left;
        }
        if y <= 0.0 {
            height = height.map(|height| height - y + top);
            y = -top;
        }
        width = width.map(|width| width + left);
        height = height.map(|height| height + top);
    }
    // In motion it is built at its scale (`motion::moving`), so its place
    // is given in the scaled lengths: about its center, moved by the pose.
    let pose = place.pose;
    let scale = pose.scale.max(0.001);
    let (full_width, full_height) = (width.unwrap_or(0.0), height.unwrap_or(0.0));
    let left = (x + pose.x + full_width * (1.0 - scale) / 2.0) / scale;
    let top = (y + pose.y + full_height * (1.0 - scale) / 2.0) / scale;
    let element = div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .flex()
        .flex_col()
        .opacity(pose.opacity);
    let (shown_width, shown_height) = match pose.grow {
        Some((crate::Edge::Top | crate::Edge::Bottom, shown)) => {
            (width, height.map(|height| height * shown))
        }
        Some((_, shown)) => (width.map(|width| width * shown), height),
        None => (width, height),
    };
    let element = match shown_width {
        Some(width) => element.w(px(width)),
        None => element,
    };
    let element = match shown_height {
        Some(height) => element.h(px(height)),
        None => element,
    };
    let _ = c;
    match pose.grow {
        // Growing from an edge, it shows that much of itself from there.
        Some((edge, shown)) => {
            let inside = div().absolute().flex().flex_col();
            let inside = match width {
                Some(width) => inside.w(px(width)),
                None => inside,
            };
            let inside = match height {
                Some(height) => inside.h(px(height)),
                None => inside,
            };
            let inside = match edge {
                crate::Edge::Bottom => inside.bottom(px(0.0)).left(px(0.0)),
                crate::Edge::Trailing => inside.top(px(0.0)).right(px(0.0)),
                _ => inside.top(px(0.0)).left(px(0.0)),
            };
            // The edge it grows from stays where it is.
            let element = match edge {
                crate::Edge::Bottom => element.top(px(top + full_height * (1.0 - shown))),
                crate::Edge::Trailing => element.left(px(left + full_width * (1.0 - shown))),
                _ => element,
            };
            element
                .overflow_hidden()
                .child(inside.children(children))
                .into_any_element()
        }
        None => element.children(children).into_any_element(),
    }
}

/// Where a child stands on a `Rail`: at a fraction of its length, or
/// between two.
#[derive(Clone, Copy, Debug)]
pub enum Placement {
    At(f32),
    Span(f32, f32),
}

/// How the point a child stands at lines up with the child.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    Start,
    #[default]
    Center,
    End,
}

/// `Rail`: a strip along its axis placing each child at its fraction of the
/// length; the children overlap in one cell, the strip as tall (or wide)
/// as the largest.
pub fn rail(c: &Ctx, vertical: bool, anchor: Anchor, children: Vec<(El, Placement)>) -> El {
    let element = div().grid().grid_cols(1).grid_rows(1);
    let element = if vertical {
        element.h_full()
    } else {
        element.w_full()
    };
    let children = children.into_iter().map(|(child, placement)| {
        let cell = div().row_start(1).col_start(1).flex();
        let cell = if vertical {
            cell.flex_col()
        } else {
            cell.flex_row()
        };
        match placement {
            Placement::At(at) => {
                let at = at.clamp(0.0, 1.0);
                let cell = if vertical {
                    cell.pt(relative(at))
                } else {
                    cell.pl(relative(at))
                };
                // A point without size, the child laid over it as anchored.
                let point = div().flex();
                let point = if vertical {
                    point.flex_col().h(px(0.0))
                } else {
                    point.flex_row().w(px(0.0))
                };
                let point = match anchor {
                    Anchor::Start => point.justify_start(),
                    Anchor::Center => point.justify_center(),
                    Anchor::End => point.justify_end(),
                };
                cell.child(point.child(div().flex_none().child(child)))
            }
            Placement::Span(from, to) => {
                let (from, to) = (from.clamp(0.0, 1.0), to.clamp(0.0, 1.0).max(from));
                let cell = if vertical {
                    cell.pt(relative(from)).pb(relative(1.0 - to))
                } else {
                    cell.pl(relative(from)).pr(relative(1.0 - to))
                };
                cell.child(div().flex_1().flex().flex_col().child(child))
            }
        }
    });
    c.grow()
        .apply(element.children(children))
        .into_any_element()
}
