//! The `morph_icon` control: a vector icon that jelly-morphs between glyphs.
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "toggle", kind: "morph_icon", area: "actions",
//!        props: { "icon": "dark_mode_rounded", "size": "20" })
//! ```
//!
//! When the `icon` prop changes (e.g. the application swaps the glyph in
//! response to a theme toggle), the previous glyph distortion-morphs into
//! the new one with a jelly easing — the shape itself interpolates, it is
//! not a cross-fade. The engine-level [`crate::core::morph`] module does the
//! outline extraction and interpolation; this widget only drives it.
//!
//! The color follows the active iced theme's text color, and following the
//! theme reveal is automatic (the registry wraps this control). `duration_ms`
//! adjusts the morph speed.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::morph::{self, Morph};
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef};
use iced::advanced::graphics::geometry::Renderer as GeometryRenderer;
use iced::advanced::layout::{self, Layout, Limits, Node};
use iced::advanced::renderer::Style;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget, mouse};
use iced::event::Event;
use iced::widget::canvas::{self, Fill};
use iced::window;
use iced::{Color, Element, Length, Point, Rectangle, Size};
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "morph_icon";

/// The control itself.
#[derive(Default)]
pub struct MorphIcon;

impl WidgetDef for MorphIcon {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let Some(glyph) = crate::icons::glyph(node.str_prop("icon").unwrap_or("")) else {
            // 非 Material 字形无法提取轮廓，退化为普通文本。
            return iced::widget::text(node.str_prop("icon").unwrap_or(""))
                .size(
                    node.prop("size")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(20.0),
                )
                .color(ctx.theme.palette().text)
                .into();
        };
        let size = node
            .prop("size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(20.0);
        let duration = node
            .prop("duration_ms")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(420.0)
            / 1000.0;
        MorphIconView::new(glyph, ctx.theme.palette().text, size, duration).into()
    }
}

/// The morphing vector icon widget.
pub struct MorphIconView {
    glyph: char,
    color: Color,
    size: f32,
    duration: f32,
}

impl MorphIconView {
    /// Creates a morphing icon that starts at `glyph`.
    pub fn new(glyph: char, color: Color, size: f32, duration: f32) -> Self {
        Self {
            glyph,
            color,
            size,
            duration,
        }
    }
}

/// Widget-tree state: the settled glyph, the requested glyph, and the
/// running morph (if any).
#[derive(Default)]
struct MorphState {
    /// Glyph requested by the latest build.
    target: Option<char>,
    /// Glyph currently drawn (settled).
    current: Option<char>,
    /// Active morph animation.
    run: Option<MorphRun>,
}

/// One morph animation run.
#[derive(Default)]
struct MorphRun {
    morph: Option<Morph>,
    /// Raw clock progress 0..1 (before jelly easing).
    progress: f32,
    last: Option<Instant>,
}

impl Widget<LayoutMessage, iced::Theme, iced::Renderer> for MorphIconView {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        layout::atomic(limits, Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MorphState>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(MorphState {
            target: Some(self.glyph),
            current: Some(self.glyph),
            run: None,
        }))
    }

    fn children(&self) -> Vec<Tree> {
        vec![]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<MorphState>();
        if state.target != Some(self.glyph) {
            let run = match (state.current, Some(self.glyph)) {
                (Some(from), Some(to)) if from != to => {
                    Morph::new(from, to).map(|morph| MorphRun {
                        morph: Some(morph),
                        progress: 0.0,
                        last: None,
                    })
                }
                _ => None,
            };
            state.target = Some(self.glyph);
            if let Some(run) = run {
                state.run = Some(run);
            } else {
                // 形变不可用（字形缺失等）：直接切换。
                state.current = Some(self.glyph);
            }
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, LayoutMessage>,
        _viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<MorphState>();
            if let Some(run) = &mut state.run {
                let dt = if let Some(last) = run.last {
                    now.duration_since(last).as_secs_f32()
                } else {
                    run.last = Some(*now);
                    0.0
                };
                if dt > 0.0 {
                    run.last = Some(*now);
                    run.progress += dt / self.duration;
                }
                if run.progress >= 1.0 {
                    state.current = state.target;
                    state.run = None;
                } else {
                    shell.request_redraw();
                }
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return;
        }

        let state = tree.state.downcast_ref::<MorphState>();
        let contours = if let Some(run) = &state.run {
            run.morph
                .as_ref()
                .map(|morph| morph.interpolate(morph::jelly(run.progress)))
        } else {
            state
                .current
                .and_then(morph::glyph_shape)
                .map(|shape| shape.contours.clone())
        };
        let Some(contours) = contours else {
            return;
        };

        // 注意：不在这里使用 `with_translation` + 本地坐标。iced 0.14 的
        // tiny-skia 后端会把 Group 几何的裁剪区变换两次（存储时一次、
        // 渲染时再一次），导致非原点位置的几何被错误裁剪。因此直接把
        // 屏幕坐标写进路径，并把 frame 裁剪区也设为控件绝对位置；
        // hover 放大是围绕控件中心缩放，裁剪区只会变大，仍然覆盖图标。
        let mut frame = canvas::Frame::with_bounds(renderer, bounds);
        // 归一化字形在 0..24 网格内，缩放到控件尺寸并留 4% 边距。
        let scale = bounds.width / morph::GLYPH_SIZE * 0.92;
        let offset = (bounds.width - morph::GLYPH_SIZE * scale) / 2.0;
        let mut builder = canvas::path::Builder::new();
        for contour in &contours {
            let mut first = true;
            for [x, y] in &contour.points {
                let point =
                    Point::new(bounds.x + offset + x * scale, bounds.y + offset + y * scale);
                if first {
                    builder.move_to(point);
                    first = false;
                } else {
                    builder.line_to(point);
                }
            }
            builder.close();
        }
        frame.fill(&builder.build(), Fill::from(self.color));
        renderer.draw_geometry(frame.into_geometry());
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

impl From<MorphIconView> for Element<'_, LayoutMessage> {
    fn from(widget: MorphIconView) -> Self {
        Element::new(widget)
    }
}
