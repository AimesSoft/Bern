//! 全局界面缩放 (whole-UI scale).
//!
//! 引擎级能力：所有控件都可以整体放大，默认 1.0，范围 1.0..=2.0。
//! `Registry::build` 会把整棵界面包进 [`ScaleWrapper`]：
//!
//! - 布局阶段用「虚拟窗口」（窗口尺寸 ÷ scale）排布内容，再整体放大
//!   `scale` 倍，所以内容始终填满窗口、不会因为放大而跑到屏幕外；
//! - 绘制时对整棵子树施加缩放变换；
//! - 交互（鼠标坐标）按 1/scale 反向换算，点击仍然命中正确的控件。
//!
//! 控件代码完全无感；应用只需调用 `registry.scale().set(value)`（例如
//! 拖动一个缩放滑块），下一次构建整棵界面就按新比例渲染。

use iced::advanced::layout::{Limits, Node};
use iced::advanced::overlay;
use iced::advanced::renderer::Style;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Overlay, Renderer, Shell, Widget, mouse};
use iced::event::Event;
use iced::{Element, Length, Point, Rectangle, Size, Transformation, Vector};
use std::sync::{Arc, Mutex};

use crate::core::widget::LayoutMessage;

/// 允许的缩放范围。
pub const MIN_SCALE: f32 = 1.0;
pub const MAX_SCALE: f32 = 2.0;

/// 共享的全局缩放因子（默认 1.0）。
#[derive(Debug, Clone)]
pub struct UiScale(Arc<Mutex<f32>>);

impl UiScale {
    /// 创建默认（1.0）的缩放状态。
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(1.0)))
    }

    /// 设置缩放因子，自动夹在 1.0..=2.0。
    pub fn set(&self, value: f32) {
        if let Ok(mut scale) = self.0.lock() {
            *scale = value.clamp(MIN_SCALE, MAX_SCALE);
        }
    }

    /// 当前缩放因子。
    pub fn get(&self) -> f32 {
        self.0.lock().map(|s| *s).unwrap_or(1.0)
    }
}

impl Default for UiScale {
    fn default() -> Self {
        Self::new()
    }
}

/// 引擎级缩放包装器：包住整棵界面，统一放大。
pub struct ScaleWrapper<'a, Message> {
    scale: UiScale,
    child: Element<'a, Message>,
}

impl<'a, Message> ScaleWrapper<'a, Message> {
    /// 包住 `child`，按 `scale` 整体缩放。
    pub fn new(scale: UiScale, child: Element<'a, Message>) -> Self {
        Self { scale, child }
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for ScaleWrapper<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let scale = self.scale.get();
        // 虚拟窗口：把可用空间缩小 1/scale，让内容在更小的空间里排布，
        // 之后整体放大回窗口尺寸。
        let virtual_limits = Limits::new(
            Size::new(limits.min().width / scale, limits.min().height / scale),
            Size::new(limits.max().width / scale, limits.max().height / scale),
        );
        let child =
            self.child
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &virtual_limits);
        let size = Size::new(child.size().width * scale, child.size().height * scale);
        Node::with_children(size, vec![child])
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<()>()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.child.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.child));
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: iced::advanced::layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let scale = self.scale.get();
        // 鼠标坐标反向换算到虚拟空间，子控件用虚拟坐标做命中检测。
        let virtual_cursor = match cursor.position() {
            Some(position) => {
                mouse::Cursor::Available(Point::new(position.x / scale, position.y / scale))
            }
            None => cursor,
        };
        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget_mut().update(
                &mut tree.children[0],
                event,
                child_layout,
                virtual_cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &Style,
        layout: iced::advanced::layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let scale = self.scale.get();
        if let Some(child_layout) = layout.children().next() {
            if scale == 1.0 {
                self.child.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    child_layout,
                    cursor,
                    viewport,
                );
            } else {
                renderer.with_transformation(Transformation::scale(scale), |renderer| {
                    self.child.as_widget().draw(
                        &tree.children[0],
                        renderer,
                        theme,
                        style,
                        child_layout,
                        cursor,
                        viewport,
                    );
                });
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: iced::advanced::layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let scale = self.scale.get();
        let virtual_cursor = match cursor.position() {
            Some(position) => {
                mouse::Cursor::Available(Point::new(position.x / scale, position.y / scale))
            }
            None => cursor,
        };
        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget().mouse_interaction(
                &tree.children[0],
                child_layout,
                virtual_cursor,
                viewport,
                renderer,
            )
        } else {
            mouse::Interaction::default()
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: iced::advanced::layout::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let scale = self.scale.get();
        let child_overlay = if let Some(child_layout) = layout.children().next() {
            self.child.as_widget_mut().overlay(
                &mut tree.children[0],
                child_layout,
                renderer,
                viewport,
                translation,
            )
        } else {
            None
        };
        child_overlay.map(|inner| overlay::Element::new(Box::new(ScaledOverlay { scale, inner })))
    }
}

impl<'a> From<ScaleWrapper<'a, LayoutMessage>> for Element<'a, LayoutMessage> {
    fn from(wrapper: ScaleWrapper<'a, LayoutMessage>) -> Self {
        Element::new(wrapper)
    }
}

/// 把子控件 overlay 整体放大 `scale` 倍。
///
/// iced 运行时对 overlay 是两段式：update 阶段调 `overlay()` 得到
/// overlay A 并布局（布局节点被存下来），draw 阶段**重新**调 `overlay()`
/// 得到新实例 overlay B，再用 A 的布局节点绘制。因此这里不能缓存虚拟
/// 节点，也不能在 draw 里重建节点树——`Layout::children()` 会给子节点
/// 叠加父节点偏移，重建必然二次偏移。
///
/// 正确做法：布局阶段用「虚拟窗口」（尺寸 ÷ scale）让内层排布，节点
/// 原样保留（虚拟坐标）；绘制时对整个 overlay 施加缩放变换；update /
/// 鼠标交互直接用虚拟坐标布局，鼠标坐标按 1/scale 反向换算。
struct ScaledOverlay<'a, Message> {
    scale: f32,
    inner: overlay::Element<'a, Message, iced::Theme, iced::Renderer>,
}

impl<Message> Overlay<Message, iced::Theme, iced::Renderer> for ScaledOverlay<'_, Message> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        let virtual_bounds = Size::new(bounds.width / self.scale, bounds.height / self.scale);
        self.inner.as_overlay_mut().layout(renderer, virtual_bounds)
    }

    fn update(
        &mut self,
        event: &Event,
        layout: iced::advanced::layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let virtual_cursor = match cursor.position() {
            Some(position) => mouse::Cursor::Available(Point::new(
                position.x / self.scale,
                position.y / self.scale,
            )),
            None => cursor,
        };
        self.inner.as_overlay_mut().update(
            event,
            layout,
            virtual_cursor,
            renderer,
            clipboard,
            shell,
        );
    }

    fn mouse_interaction(
        &self,
        layout: iced::advanced::layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let virtual_cursor = match cursor.position() {
            Some(position) => mouse::Cursor::Available(Point::new(
                position.x / self.scale,
                position.y / self.scale,
            )),
            None => cursor,
        };
        self.inner
            .as_overlay()
            .mouse_interaction(layout, virtual_cursor, renderer)
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &Style,
        layout: iced::advanced::layout::Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let virtual_cursor = match cursor.position() {
            Some(position) => mouse::Cursor::Available(Point::new(
                position.x / self.scale,
                position.y / self.scale,
            )),
            None => cursor,
        };
        renderer.with_transformation(Transformation::scale(self.scale), |renderer| {
            self.inner
                .as_overlay()
                .draw(renderer, theme, style, layout, virtual_cursor);
        });
    }

    fn index(&self) -> f32 {
        self.inner.as_overlay().index()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_one_and_clamps_range() {
        let scale = UiScale::new();
        assert_eq!(scale.get(), 1.0);

        scale.set(0.5);
        assert_eq!(scale.get(), 1.0, "below minimum clamps to 1.0");

        scale.set(3.0);
        assert_eq!(scale.get(), 2.0, "above maximum clamps to 2.0");

        scale.set(1.5);
        assert_eq!(scale.get(), 1.5);
    }
}
