//! GPUI renderer for the shared `bern-surface` self-drawn UI IR.
//!
//! Translates [`SurfaceNode`], [`BoxNode`], [`TrackNode`], [`RingNode`],
//! [`TextNode`], [`IconNode`], [`Layout`], [`VisualStyle`], and
//! [`StateOverrides`] into GPUI elements and styles so GPUI and Web render
//! identical geometry, colors, shadows, translucent surfaces, and the white
//! vertical capsule slider thumb.

use crate::El;
use crate::ctx::Ctx;
use crate::theme::rgba8;
use crate::units::px;
use bern_surface::{
    Align, Axis, Border, BoxNode, ColorValue, Corners, Cursor, Display, IconNode, Justify, Layout,
    Length, Overflow, Position, RingNode, Shadow, SurfaceNode, SurfacePalette, SurfaceRenderer,
    TextNode, TrackNode, VisualStyle,
};
use gpui::{
    BoxShadow, Div, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    Rgba, StyleRefinement, Styled, canvas, div, point, relative,
};
use std::collections::HashMap;

/// The platform's monospaced font, for `Typography::monospace`.
const MONOSPACE: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Cascadia Mono"
} else {
    "monospace"
};

/// Resolve a `bern-surface` [`ColorValue`] in `c`'s current theme, runtime
/// palette overrides, and subtree `tint`.
pub(crate) fn resolve_color(c: &Ctx, color: ColorValue) -> Rgba {
    match color {
        ColorValue::Transparent => Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        },
        ColorValue::Role(role) => c.color(role),
        ColorValue::RoleAlpha(role, alpha) => {
            let mut resolved = c.color(role);
            resolved.a = alpha.clamp(0.0, 1.0);
            resolved
        }
        ColorValue::Recipe {
            recipe,
            state,
            slot,
        } => c.slot(recipe, state, slot),
        ColorValue::Fixed { light, dark } => rgba8(if c.theme().dark { dark } else { light }),
        ColorValue::Rgba(rgba) => rgba8(rgba),
    }
}

/// Convert a `bern-surface` [`Shadow`] to a GPUI [`BoxShadow`].
pub(crate) fn box_shadow(c: &Ctx, shadow: &Shadow) -> BoxShadow {
    let color: Hsla = resolve_color(c, shadow.color).into();
    BoxShadow {
        color,
        offset: point(px(shadow.offset_x), px(shadow.offset_y)),
        blur_radius: px(shadow.blur),
        spread_radius: px(shadow.spread),
        inset: shadow.inset,
    }
}

/// Apply [`Layout`] properties to any GPUI [`Styled`] element.
pub(crate) fn apply_layout<E: Styled>(el: E, layout: &Layout) -> E {
    let mut el = match layout.display {
        Display::Flex => el.flex(),
        Display::Stack => el.grid().grid_cols(1).grid_rows(1),
        Display::Grid => el.grid(),
    };
    if layout.display == Display::Flex {
        el = match layout.direction {
            Axis::Horizontal => el.flex_row(),
            Axis::Vertical => el.flex_col(),
        };
        if layout.wrap {
            el = el.flex_wrap();
        }
        el = match layout.align {
            Align::Start => el.items_start(),
            Align::Center => el.items_center(),
            Align::End => el.items_end(),
            Align::Baseline => el.items_baseline(),
            Align::Stretch => el,
        };
        el = match layout.justify {
            Justify::Start => el.justify_start(),
            Justify::Center => el.justify_center(),
            Justify::End => el.justify_end(),
            Justify::SpaceBetween => el.justify_between(),
        };
    }
    if layout.gap > 0.0 {
        el = el.gap(px(layout.gap));
    }
    if !layout.padding.is_zero() {
        let p = layout.padding;
        if p.top > 0.0 {
            el = el.pt(px(p.top));
        }
        if p.right > 0.0 {
            el = el.pr(px(p.right));
        }
        if p.bottom > 0.0 {
            el = el.pb(px(p.bottom));
        }
        if p.left > 0.0 {
            el = el.pl(px(p.left));
        }
    }
    if !layout.margin.is_zero() {
        let m = layout.margin;
        if m.top != 0.0 {
            el = el.mt(px(m.top));
        }
        if m.right != 0.0 {
            el = el.mr(px(m.right));
        }
        if m.bottom != 0.0 {
            el = el.mb(px(m.bottom));
        }
        if m.left != 0.0 {
            el = el.ml(px(m.left));
        }
    }
    el = match layout.width {
        Length::Auto => el,
        Length::Px(v) => el.w(px(v)),
        Length::Fraction(f) => el.w(relative(f)),
        Length::Full => el.w_full(),
    };
    el = match layout.height {
        Length::Auto => el,
        Length::Px(v) => el.h(px(v)),
        Length::Fraction(f) => el.h(relative(f)),
        Length::Full => el.h_full(),
    };
    if let Some(min_w) = layout.min_width {
        el = match min_w {
            Length::Auto => el,
            Length::Px(v) => el.min_w(px(v)),
            Length::Fraction(f) => el.min_w(relative(f)),
            Length::Full => el.min_w_full(),
        };
    }
    if let Some(max_w) = layout.max_width {
        el = match max_w {
            Length::Auto => el,
            Length::Px(v) => el.max_w(px(v)),
            Length::Fraction(f) => el.max_w(relative(f)),
            Length::Full => el.max_w_full(),
        };
    }
    if let Some(min_h) = layout.min_height {
        el = match min_h {
            Length::Auto => el,
            Length::Px(v) => el.min_h(px(v)),
            Length::Fraction(f) => el.min_h(relative(f)),
            Length::Full => el.min_h_full(),
        };
    }
    if let Some(max_h) = layout.max_height {
        el = match max_h {
            Length::Auto => el,
            Length::Px(v) => el.max_h(px(v)),
            Length::Fraction(f) => el.max_h(relative(f)),
            Length::Full => el.max_h_full(),
        };
    }
    match (layout.flex_grow, layout.flex_shrink) {
        (Some(g), _) if g > 0.0 => el = el.flex_1(),
        (_, Some(0.0)) => el = el.flex_none(),
        (_, Some(s)) => el = el.flex_shrink(s),
        _ => {}
    }
    el = match layout.position {
        Position::Relative => el.relative(),
        Position::Absolute {
            top,
            right,
            bottom,
            left,
        } => {
            let mut el = el.absolute();
            if let Some(top) = top {
                el = match top {
                    Length::Px(v) => el.top(px(v)),
                    Length::Fraction(f) => el.top(relative(f)),
                    Length::Full => el.top(relative(1.0)),
                    Length::Auto => el,
                };
            }
            if let Some(right) = right {
                el = match right {
                    Length::Px(v) => el.right(px(v)),
                    Length::Fraction(f) => el.right(relative(f)),
                    Length::Full => el.right(relative(1.0)),
                    Length::Auto => el,
                };
            }
            if let Some(bottom) = bottom {
                el = match bottom {
                    Length::Px(v) => el.bottom(px(v)),
                    Length::Fraction(f) => el.bottom(relative(f)),
                    Length::Full => el.bottom(relative(1.0)),
                    Length::Auto => el,
                };
            }
            if let Some(left) = left {
                el = match left {
                    Length::Px(v) => el.left(px(v)),
                    Length::Fraction(f) => el.left(relative(f)),
                    Length::Full => el.left(relative(1.0)),
                    Length::Auto => el,
                };
            }
            el
        }
    };
    match layout.overflow {
        Overflow::Visible => el,
        Overflow::Hidden | Overflow::ScrollY | Overflow::ScrollX | Overflow::ScrollBoth => {
            el.overflow_hidden()
        }
    }
}

fn apply_border<E: Styled>(c: &Ctx, el: E, border: Border) -> E {
    let color = resolve_color(c, border.color);
    let w = px(border.width);
    let s = border.sides;
    if s.top && s.right && s.bottom && s.left {
        el.border(w).border_color(color)
    } else {
        let mut el = el.border_color(color);
        if s.top {
            el = el.border_t(w);
        }
        if s.right {
            el = el.border_r(w);
        }
        if s.bottom {
            el = el.border_b(w);
        }
        if s.left {
            el = el.border_l(w);
        }
        el
    }
}

fn apply_corners<E: Styled>(el: E, corners: Corners) -> E {
    if corners.is_pill() {
        el.rounded_full()
    } else if corners.is_uniform() {
        el.rounded(px(corners.tl))
    } else {
        let mut el = el;
        if corners.tl > 0.0 {
            el = el.rounded_tl(px(corners.tl));
        }
        if corners.tr > 0.0 {
            el = el.rounded_tr(px(corners.tr));
        }
        if corners.br > 0.0 {
            el = el.rounded_br(px(corners.br));
        }
        if corners.bl > 0.0 {
            el = el.rounded_bl(px(corners.bl));
        }
        el
    }
}

/// Apply [`VisualStyle`] properties to any GPUI [`Styled`] element.
pub(crate) fn apply_visual<E: Styled>(c: &Ctx, el: E, style: &VisualStyle) -> E {
    let mut el = el;
    if let Some(bg) = style.background {
        el = el.bg(resolve_color(c, bg));
    }
    if let Some(fg) = style.foreground {
        el = el.text_color(resolve_color(c, fg));
    }
    if let Some(border) = style.border {
        el = apply_border(c, el, border);
    }
    if let Some(corners) = style.corners {
        el = apply_corners(el, corners);
    }
    if !style.shadows.is_empty() {
        let shadows: Vec<BoxShadow> = style.shadows.iter().map(|s| box_shadow(c, s)).collect();
        el = el.shadow(shadows);
    }
    if let Some(opacity) = style.opacity {
        el = el.opacity(opacity);
    }
    if let Some(cursor) = style.cursor {
        el = match cursor {
            Cursor::Default => el.cursor_default(),
            Cursor::Pointer => el.cursor_pointer(),
            Cursor::Text => el.cursor_text(),
            Cursor::ColResize => el.cursor_col_resize(),
            Cursor::RowResize => el.cursor_row_resize(),
            Cursor::Grab => el.cursor_grab(),
        };
    }
    if let Some(typo) = style.typography {
        el = el
            .text_size(px(typo.size))
            .font_weight(FontWeight(f32::from(typo.weight)));
        if let Some(lh) = typo.line_height {
            el = el.line_height(px(lh));
        }
        if typo.monospace {
            el = el.font_family(MONOSPACE);
        }
    }
    el
}

/// Apply a [`VisualStyle`] delta inside a GPUI `.hover(...)` refinement closure.
pub(crate) fn apply_refinement(
    c: &Ctx,
    style: StyleRefinement,
    state: &VisualStyle,
) -> StyleRefinement {
    let mut style = style;
    if let Some(bg) = state.background {
        style = style.bg(resolve_color(c, bg));
    }
    if let Some(fg) = state.foreground {
        style = style.text_color(resolve_color(c, fg));
    }
    if let Some(border) = state.border {
        style = style.border_color(resolve_color(c, border.color));
    }
    if !state.shadows.is_empty() {
        let shadows: Vec<BoxShadow> = state.shadows.iter().map(|s| box_shadow(c, s)).collect();
        style = style.shadow(shadows);
    }
    if let Some(opacity) = state.opacity {
        style = style.opacity(opacity);
    }
    style
}

/// Apply layout, base visual style, and hover state overrides from a lowered
/// [`BoxNode`] onto a GPUI [`Div`].
pub(crate) fn apply_box(c: &Ctx, el: Div, node: &BoxNode) -> Div {
    let mut el = apply_visual(c, apply_layout(el, &node.layout), &node.style);
    if let Some(hover) = node.states.hover.clone() {
        let c = c.clone();
        el = el.hover(move |s| apply_refinement(&c, s, &hover));
    }
    el
}

/// Extract a [`BoxNode`] from a lowered [`SurfaceNode::Box`].
pub(crate) fn as_box(node: &SurfaceNode) -> Option<&BoxNode> {
    match node {
        SurfaceNode::Box(b) => Some(b),
        _ => None,
    }
}

/// Render a [`TextNode`] into a GPUI [`Div`].
pub(crate) fn render_text(c: &Ctx, text: &TextNode) -> Div {
    let mut el = div()
        .text_size(px(text.typography.size))
        .font_weight(FontWeight(f32::from(text.typography.weight)))
        .text_color(resolve_color(c, text.color))
        .min_w(px(0.0));
    if let Some(lh) = text.typography.line_height {
        el = el.line_height(px(lh));
    }
    if text.typography.monospace {
        el = el.font_family(MONOSPACE);
    }
    if text.truncate || text.lines == Some(1) {
        el = el.truncate();
    } else if let Some(lines) = text.lines {
        el = el.line_clamp(lines);
    }
    if text.selectable {
        el = el.cursor_text();
    }
    el.child(text.content.clone())
}

/// Render an [`IconNode`] via GPUI's Lucide icon table.
pub(crate) fn render_icon(c: &Ctx, icon: &IconNode) -> El {
    crate::icons::icon(icon.name, icon.size, resolve_color(c, icon.color))
}

/// Render a [`RingNode`] (`Progress(style: ring)`) as a vector arc with round
/// caps.
pub(crate) fn render_ring(c: &Ctx, ring: &RingNode) -> El {
    let fraction = ring.fraction.clamp(0.0, 1.0);
    let stroke = ring.stroke_width;
    let color = resolve_color(c, ring.color);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let shown = fraction.clamp(0.0001, 0.9999);
            let center = bounds.center();
            let radius = (bounds.size.width.min(bounds.size.height) - px(stroke)) / 2.0;
            let angle = shown * std::f32::consts::TAU;
            let start = point(center.x, center.y - radius);
            let end = point(
                center.x + radius * angle.sin(),
                center.y - radius * angle.cos(),
            );
            let mut arc = PathBuilder::stroke(px(stroke));
            arc.move_to(start);
            arc.arc_to(point(radius, radius), px(0.0), shown > 0.5, true, end);
            if let Ok(path) = arc.build() {
                window.paint_path(path, color);
            }
            let cap = px(stroke / 2.0);
            for at in [start, end] {
                let mut disc = PathBuilder::fill();
                disc.move_to(point(at.x - cap, at.y));
                disc.arc_to(
                    point(cap, cap),
                    px(0.0),
                    false,
                    true,
                    point(at.x + cap, at.y),
                );
                disc.arc_to(
                    point(cap, cap),
                    px(0.0),
                    false,
                    true,
                    point(at.x - cap, at.y),
                );
                disc.close();
                if let Ok(path) = disc.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .flex_none()
    .size(px(ring.diameter))
    .into_any_element()
}

/// Resolved paint parameters for an interactive GPUI slider track lowered from
/// [`TrackNode`].
#[derive(Clone)]
pub(crate) struct ResolvedTrack {
    pub axis: Axis,
    pub fraction: f32,
    pub hit_extent: f32,
    pub min_main_extent: Option<f32>,
    pub thickness: f32,
    pub radius: f32,
    pub track_color: Rgba,
    pub fill_color: Rgba,
    pub disabled: bool,
    pub thumb: Option<ResolvedThumb>,
}

/// Resolved paint parameters for the white vertical capsule slider thumb.
#[derive(Clone)]
pub(crate) struct ResolvedThumb {
    pub width: f32,
    pub height: f32,
    pub scale_active: f32,
    pub background: Rgba,
    pub border: Option<(Rgba, f32)>,
    pub shadows: Vec<BoxShadow>,
    pub opacity: f32,
}

impl ResolvedTrack {
    pub(crate) fn from_node(c: &Ctx, track: &TrackNode) -> Self {
        let thumb = track.thumb.as_ref().map(|node| {
            let width = match node.layout.width {
                Length::Px(v) => v,
                _ => bern_surface::controls::SLIDER_THUMB_THICKNESS,
            };
            let height = match node.layout.height {
                Length::Px(v) => v,
                _ => bern_surface::controls::SLIDER_THUMB_LENGTH,
            };
            let scale_active = node
                .states
                .hover
                .as_ref()
                .and_then(|h| h.scale)
                .map(|(sx, sy)| sx.max(sy))
                .unwrap_or(1.0);
            let background = node
                .style
                .background
                .map(|bg| resolve_color(c, bg))
                .unwrap_or_else(|| c.color("slider.thumb"));
            let border = node
                .style
                .border
                .map(|b| (resolve_color(c, b.color), b.width));
            let shadows = node
                .style
                .shadows
                .iter()
                .map(|s| box_shadow(c, s))
                .collect();
            let opacity = node.style.opacity.unwrap_or(1.0);
            ResolvedThumb {
                width,
                height,
                scale_active,
                background,
                border,
                shadows,
                opacity,
            }
        });
        Self {
            axis: track.axis,
            fraction: track.fraction.clamp(0.0, 1.0),
            hit_extent: track.hit_extent,
            min_main_extent: track.min_main_extent,
            thickness: track.thickness,
            radius: track.radius,
            track_color: resolve_color(c, track.track_color),
            fill_color: resolve_color(c, track.fill_color),
            disabled: track.disabled,
            thumb,
        }
    }

    /// Build the painted rail + optional white perpendicular pill thumb.
    /// `active` enlarges the thumb by `scale_active` (1.12x) on hover/drag.
    pub(crate) fn build_visual(&self, active: bool) -> (Div, Option<Div>) {
        let vertical = self.axis == Axis::Vertical;
        let rail = if vertical {
            div()
                .w(px(self.thickness))
                .h_full()
                .rounded(px(self.radius))
                .bg(self.track_color)
                .flex()
                .flex_col()
                .justify_end()
                .child(
                    div()
                        .w_full()
                        .h(relative(self.fraction))
                        .rounded(px(self.radius))
                        .bg(self.fill_color),
                )
        } else {
            div()
                .w_full()
                .h(px(self.thickness))
                .rounded(px(self.radius))
                .bg(self.track_color)
                .child(
                    div()
                        .h_full()
                        .w(relative(self.fraction))
                        .rounded(px(self.radius))
                        .bg(self.fill_color),
                )
        };
        let thumb = self.thumb.as_ref().map(|t| {
            let scale = if active && !self.disabled {
                t.scale_active
            } else {
                1.0
            };
            let w = t.width * scale;
            let h = t.height * scale;
            let mut el = if vertical {
                div()
                    .absolute()
                    .bottom(relative(self.fraction))
                    .mb(px(-h / 2.0))
                    .w(px(w))
                    .h(px(h))
            } else {
                div()
                    .absolute()
                    .left(relative(self.fraction))
                    .ml(px(-w / 2.0))
                    .w(px(w))
                    .h(px(h))
            };
            el = el.rounded_full().bg(t.background).opacity(t.opacity);
            if let Some((border_color, border_width)) = t.border {
                el = el.border(px(border_width)).border_color(border_color);
            }
            if !t.shadows.is_empty() {
                el = el.shadow(t.shadows.clone());
            }
            el
        });
        (rail, thumb)
    }
}

/// Render a non-interactive [`TrackNode`] (such as `Progress`).
pub(crate) fn render_track(c: &Ctx, track: &TrackNode) -> El {
    let resolved = ResolvedTrack::from_node(c, track);
    let (rail, thumb) = resolved.build_visual(false);
    let mut wrap = if resolved.axis == Axis::Vertical {
        let mut d = div()
            .relative()
            .flex_1()
            .w(px(resolved.hit_extent))
            .flex()
            .justify_center();
        if let Some(min_h) = resolved.min_main_extent {
            d = d.min_h(px(min_h));
        }
        d
    } else {
        div()
            .relative()
            .w_full()
            .h(px(resolved.hit_extent))
            .flex()
            .items_center()
    };
    wrap = wrap.child(rail);
    if let Some(thumb) = thumb {
        wrap = wrap.child(thumb);
    }
    if resolved.disabled {
        wrap = wrap.opacity(0.5);
    }
    wrap.into_any_element()
}

/// GPUI implementation of [`SurfaceRenderer`] with named child slot bindings.
pub(crate) struct GpuiRenderer<'a> {
    ctx: &'a Ctx,
    slots: HashMap<&'static str, Vec<El>>,
}

impl<'a> GpuiRenderer<'a> {
    pub(crate) fn new(ctx: &'a Ctx) -> Self {
        Self {
            ctx,
            slots: HashMap::new(),
        }
    }

    pub(crate) fn slot(mut self, name: &'static str, children: Vec<El>) -> Self {
        self.slots.insert(name, children);
        self
    }

    pub(crate) fn slot_one(mut self, name: &'static str, child: El) -> Self {
        self.slots.insert(name, vec![child]);
        self
    }

    pub(crate) fn render(&mut self, node: &SurfaceNode) -> El {
        match node {
            SurfaceNode::Box(box_node) => {
                let mut el = apply_box(self.ctx, div(), box_node);
                for child in &box_node.children {
                    match child {
                        SurfaceNode::Slot(slot_id) => {
                            if let Some(items) = self.slots.remove(slot_id.0) {
                                el = el.children(items);
                            }
                        }
                        other => {
                            el = el.child(self.render(other));
                        }
                    }
                }
                el.into_any_element()
            }
            SurfaceNode::Text(text) => render_text(self.ctx, text).into_any_element(),
            SurfaceNode::Icon(icon) => render_icon(self.ctx, icon),
            SurfaceNode::Track(track) => render_track(self.ctx, track),
            SurfaceNode::Ring(ring) => render_ring(self.ctx, ring),
            SurfaceNode::Spacer => div().flex_1().into_any_element(),
            SurfaceNode::Slot(slot_id) => {
                let items = self.slots.remove(slot_id.0).unwrap_or_default();
                if items.len() == 1 {
                    items.into_iter().next().unwrap()
                } else {
                    div().flex().flex_col().children(items).into_any_element()
                }
            }
        }
    }
}

impl SurfaceRenderer for GpuiRenderer<'_> {
    type Output = El;

    fn render_node(&mut self, node: &SurfaceNode, _palette: &SurfacePalette) -> Self::Output {
        self.render(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_surface::controls::{
        SLIDER_THUMB_LENGTH, SLIDER_THUMB_SCALE_ACTIVE, SLIDER_THUMB_THICKNESS, slider_thumb,
    };

    #[test]
    fn slider_thumb_spec_is_white_perpendicular_capsule_in_both_modes() {
        for dark in [false, true] {
            let theme = crate::theme::of(dark, None);
            let h_thumb = slider_thumb(Axis::Horizontal, true);
            assert_eq!(h_thumb.layout.width, Length::Px(SLIDER_THUMB_THICKNESS));
            assert_eq!(h_thumb.layout.height, Length::Px(SLIDER_THUMB_LENGTH));
            let v_thumb = slider_thumb(Axis::Vertical, true);
            assert_eq!(v_thumb.layout.width, Length::Px(SLIDER_THUMB_LENGTH));
            assert_eq!(v_thumb.layout.height, Length::Px(SLIDER_THUMB_THICKNESS));
            let bg = theme.color("slider.thumb");
            assert_eq!(bg, rgba8(bern_surface::Rgba8::WHITE));
            assert!(theme.color("slider.thumb.border").a > 0.0);
            assert!(theme.color("slider.thumb.shadow").a > 0.0);
            let scale = h_thumb.states.hover.as_ref().and_then(|s| s.scale).unwrap();
            assert_eq!(
                scale,
                (SLIDER_THUMB_SCALE_ACTIVE, SLIDER_THUMB_SCALE_ACTIVE)
            );
        }
    }
}
