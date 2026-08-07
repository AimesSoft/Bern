//! The `dropdown` control: a blur-style dropdown menu, ported from nipaplay's
//! `BlurDropdown`.
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "lib_sort", kind: "dropdown", area: "root",
//!        props: { "items": "名称:sort_name,大小:sort_size,时间:sort_time",
//!                 "selected": "sort_name" })
//! ```
//!
//! Appearance and behavior follow nipaplay:
//!
//! - a 40 px rounded trigger (border turns accent while open, the chevron
//!   rotates 180°), theme-adaptive background and border;
//! - clicking opens an overlay menu below the trigger: fade + 0.95→1 scale
//!   pop (200 ms), rounded 6 panel with a soft shadow, items with
//!   separator lines, selected-item background and hover highlight;
//! - clicking an item publishes `(item_key, Pressed)` and closes; clicking
//!   outside (the scrim) closes without selecting.
//!
//! The selected item is driven by the layout `selected` prop (key or index,
//! default the first item) — the app patches it after receiving the event,
//! like `h_tab` and the slider. Item keys are interaction ids declared in
//! the app's central `ids.rs`.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent};
use crate::widgets::h_tab;
use iced::advanced::graphics::geometry::Renderer as GeometryRenderer;
use iced::advanced::layout::{self, Layout, Limits, Node};
use iced::advanced::overlay::{self, Overlay};
use iced::advanced::renderer::Style;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Renderer, Shell, Widget, mouse};
use iced::event::Event;
use iced::widget::canvas;
use iced::widget::{Column, Space, button, container};
use iced::window;
use iced::{
    Background, Border, Color, Element, Length, Point, Rectangle, Shadow, Size, Transformation,
    Vector,
};
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "dropdown";

/// Internal marker message used by the close-scrim (intercepted by the
/// overlay, never forwarded to the application).
const CLOSE_TAG: &str = "__dropdown_close";

/// Trigger height (nipaplay uses 40 px).
const CONTROL_HEIGHT: f32 = 40.0;
/// Menu pop animation duration (nipaplay uses 200 ms).
const ANIM_MS: f32 = 200.0;

/// Colors resolved from the active theme (nipaplay's light/dark values).
#[derive(Debug, Clone, Copy)]
struct Colors {
    control_bg: Color,
    border_idle: Color,
    border_active: Color,
    text: Color,
    chevron: Color,
    menu_bg: Color,
    menu_border: Color,
    item_separator: Color,
    item_selected_bg: Color,
    item_hover_bg: Color,
}

/// The control itself (the [`WidgetDef`]).
#[derive(Default)]
pub struct Dropdown;

impl WidgetDef for Dropdown {
    fn name(&self) -> &'static str {
        NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let items: Vec<(String, String)> = h_tab::parse_items(node.str_prop("items").unwrap_or(""))
            .into_iter()
            .map(|(label, key)| (label, ctx.qualify(&key)))
            .collect();
        let selected = node
            .prop("selected")
            .and_then(|s| {
                items
                    .iter()
                    .position(|(_, key)| key == s || key.ends_with(&format!(".{s}")))
                    .or_else(|| s.parse::<usize>().ok().filter(|&i| i < items.len()))
            })
            .unwrap_or(0);
        let font_size = node
            .prop("font_size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(16.0);
        let (width, _height) = crate::core::widget::size_lengths(size);

        let is_dark = ctx.theme.extended_palette().is_dark;
        let accent = ctx.theme.extended_palette().primary.base.color;
        let text = ctx.theme.palette().text;
        let with_alpha = |c: Color, a: f32| Color::from_rgba(c.r, c.g, c.b, a);
        let colors = Colors {
            control_bg: if is_dark {
                with_alpha(Color::WHITE, 0.12)
            } else {
                Color::WHITE
            },
            border_idle: with_alpha(text, 0.1),
            border_active: accent,
            text,
            chevron: if is_dark { Color::WHITE } else { Color::BLACK },
            menu_bg: if is_dark {
                Color::from_rgb8(44, 44, 44)
            } else {
                Color::WHITE
            },
            menu_border: with_alpha(text, 0.1),
            item_separator: if is_dark {
                with_alpha(Color::WHITE, 0.1)
            } else {
                with_alpha(Color::BLACK, 0.05)
            },
            item_selected_bg: if is_dark {
                with_alpha(Color::WHITE, 0.1)
            } else {
                with_alpha(Color::BLACK, 0.05)
            },
            item_hover_bg: with_alpha(accent, 0.2),
        };

        let selected_label = items
            .get(selected)
            .map(|(label, _)| label.clone())
            .unwrap_or_default();

        DropdownView {
            items,
            selected,
            id: ctx.qualify(&node.id),
            width: width.unwrap_or(Length::Fixed(160.0)),
            font_size,
            colors,
            label: iced::widget::text(selected_label).size(font_size).into(),
        }
        .into()
    }
}

/// Widget-tree state: open/close + pop animation + the overlay content tree.
struct State {
    open: bool,
    /// Pop progress 0..1 (scale 0.95→1 while opening, reverse while closing).
    anim: f32,
    last: Option<Instant>,
    /// Tree of the overlay content (scrim + menu), reconciled each frame.
    overlay_tree: Tree,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            anim: 0.0,
            last: None,
            overlay_tree: Tree::empty(),
        }
    }
}

/// The trigger + menu host.
pub struct DropdownView<'a> {
    /// `(label, qualified event key)` for every item.
    items: Vec<(String, String)>,
    selected: usize,
    /// Qualified control id (used for the internal close marker).
    id: String,
    width: Length,
    font_size: f32,
    colors: Colors,
    label: Element<'a, LayoutMessage>,
}

impl<'a> DropdownView<'a> {
    /// Builds the overlay content: a full-viewport transparent scrim plus
    /// the menu panel (positioned by the overlay's layout).
    fn build_overlay(
        &self,
        menu_width: f32,
        anim: f32,
    ) -> (Element<'a, LayoutMessage>, Element<'a, LayoutMessage>) {
        let colors = self.colors;
        let fade = |c: Color| Color::from_rgba(c.r, c.g, c.b, c.a * anim);
        let separator_faded = fade(colors.item_separator);
        let menu_bg_faded = fade(colors.menu_bg);
        let menu_border_faded = fade(colors.menu_border);

        // 全屏透明遮罩：点击任意处关闭。
        let close_msg = LayoutMessage::Event(WidgetEvent {
            widget_id: self.id.clone(),
            kind: EventKind::Other(CLOSE_TAG.into()),
        });
        let scrim = button(Space::new().width(Length::Fill).height(Length::Fill))
            .on_press(close_msg)
            .style(|_theme, _status| button::Style {
                background: None,
                text_color: Color::TRANSPARENT,
                ..Default::default()
            })
            .into();

        // 菜单项。
        let mut rows: Vec<Element<'a, LayoutMessage>> = Vec::new();
        for (index, (label, key)) in self.items.iter().enumerate() {
            let selected = index == self.selected;
            let key = key.clone();
            let label = label.clone();
            rows.push(
                DropdownItem {
                    label: iced::widget::text(label).size(self.font_size).into(),
                    on_press: LayoutMessage::Event(WidgetEvent {
                        widget_id: key,
                        kind: EventKind::Pressed,
                    }),
                    colors,
                    selected,
                    anim,
                }
                .into(),
            );
            if index + 1 < self.items.len() {
                rows.push(
                    container(Space::new().width(Length::Fill).height(1.0))
                        .style(move |_theme| container::Style {
                            background: Some(Background::Color(separator_faded)),
                            ..Default::default()
                        })
                        .into(),
                );
            }
        }

        // 菜单面板：圆角 6、细边框、柔和阴影。
        let menu = container(Column::with_children(rows))
            .width(menu_width)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(menu_bg_faded)),
                border: Border::default()
                    .width(0.5)
                    .color(menu_border_faded)
                    .rounded(6),
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.1 * anim),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 10.0,
                },
                ..Default::default()
            });

        (scrim, menu.into())
    }
}

impl<'a> Widget<LayoutMessage, iced::Theme, iced::Renderer> for DropdownView<'a> {
    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(CONTROL_HEIGHT))
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let control = layout::atomic(limits, self.width, Length::Fixed(CONTROL_HEIGHT));
        let label_node = self
            .label
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let center_y = CONTROL_HEIGHT / 2.0;
        let mut label_node = label_node;
        label_node.move_to_mut(Point::new(12.0, center_y - label_node.size().height / 2.0));

        Node::with_children(control.size(), vec![label_node])
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State::default()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.label.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children[0].diff(self.label.as_widget());
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, LayoutMessage>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();

        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) && cursor.is_over(layout.bounds())
        {
            state.open = !state.open;
            shell.request_redraw();
        }

        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let dt = match state.last {
                Some(last) => {
                    let elapsed = now.duration_since(last).as_secs_f32();
                    if elapsed > 0.1 { 0.0 } else { elapsed }
                }
                None => 0.0,
            };
            state.last = Some(*now);
            let target = if state.open { 1.0 } else { 0.0 };
            let remaining = target - state.anim;
            if remaining.abs() > 0.0005 {
                let step = dt / (ANIM_MS / 1000.0);
                state.anim = if remaining > 0.0 {
                    (state.anim + step).min(target)
                } else {
                    (state.anim - step).max(target)
                };
                shell.request_redraw();
            } else {
                state.anim = target;
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let open = state.open;
        let accent = self.colors.border_active;
        let border_color = if open {
            accent
        } else {
            self.colors.border_idle
        };
        let border_width = if open { 1.5 } else { 1.0 };

        renderer.fill_quad(
            iced::advanced::renderer::Quad {
                bounds,
                border: Border::default()
                    .rounded(8)
                    .width(border_width)
                    .color(border_color),
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                },
                ..Default::default()
            },
            self.colors.control_bg,
        );

        if let Some(label_layout) = layout.children().next() {
            self.label.as_widget().draw(
                &tree.children[0],
                renderer,
                _theme,
                &Style {
                    text_color: self.colors.text,
                },
                label_layout,
                cursor,
                viewport,
            );
        }

        // chevron：V 形箭头，打开时绕中心旋转 180°、颜色变强调色。
        draw_chevron(
            renderer,
            bounds,
            state.anim,
            if open { accent } else { self.colors.chevron },
        );
        let _ = style;
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, LayoutMessage, iced::Theme, iced::Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        if !state.open && state.anim <= 0.001 {
            return None;
        }

        let anim = state.anim;
        let State {
            open, overlay_tree, ..
        } = state;
        let (scrim, menu) = self.build_overlay(layout.bounds().width, anim);
        overlay_tree.diff_children(&[&scrim, &menu]);

        Some(overlay::Element::new(Box::new(MenuOverlay {
            open,
            overlay_tree,
            scrim,
            menu,
            position: layout.position() + translation,
            control_height: layout.bounds().height,
            viewport: *viewport,
            anim,
            id: self.id.clone(),
            items: self.items.clone(),
        })))
    }
}

/// The overlay that hosts the scrim + menu panel below the trigger.
struct MenuOverlay<'a> {
    open: &'a mut bool,
    overlay_tree: &'a mut Tree,
    scrim: Element<'a, LayoutMessage>,
    menu: Element<'a, LayoutMessage>,
    position: Point,
    control_height: f32,
    viewport: Rectangle,
    anim: f32,
    id: String,
    items: Vec<(String, String)>,
}

impl Overlay<LayoutMessage, iced::Theme, iced::Renderer> for MenuOverlay<'_> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        let scrim_node = self.scrim.as_widget_mut().layout(
            &mut self.overlay_tree.children[0],
            renderer,
            &Limits::new(Size::ZERO, bounds),
        );

        let menu_limits = Limits::new(
            Size::ZERO,
            Size::new(
                (bounds.width - self.position.x).max(0.0),
                (bounds.height - self.position.y).max(0.0),
            ),
        );
        let mut menu_node = self.menu.as_widget_mut().layout(
            &mut self.overlay_tree.children[1],
            renderer,
            &menu_limits,
        );
        menu_node.move_to_mut(Point::new(
            self.position.x,
            self.position.y + self.control_height + 5.0,
        ));

        Node::with_children(bounds, vec![scrim_node, menu_node])
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, LayoutMessage>,
    ) {
        let bounds = layout.bounds();

        let (captured, redraw_request, local) = {
            let mut local = Vec::new();
            let mut local_shell = Shell::new(&mut local);

            // 菜单优先处理：菜单项点击会捕获事件；遮罩只在菜单没处理时
            // 响应，避免遮罩把菜单的点击吃掉。
            let mut menu_handled = false;
            if let Some(menu_layout) = layout.children().nth(1) {
                self.menu.as_widget_mut().update(
                    &mut self.overlay_tree.children[1],
                    event,
                    menu_layout,
                    cursor,
                    renderer,
                    clipboard,
                    &mut local_shell,
                    &bounds,
                );
                menu_handled = local_shell.is_event_captured();
            }
            if let Some(scrim_layout) = layout.children().next()
                && !menu_handled
            {
                self.scrim.as_widget_mut().update(
                    &mut self.overlay_tree.children[0],
                    event,
                    scrim_layout,
                    cursor,
                    renderer,
                    clipboard,
                    &mut local_shell,
                    &bounds,
                );
            }

            (
                local_shell.is_event_captured(),
                local_shell.redraw_request(),
                local,
            )
        };

        for message in local {
            match &message {
                LayoutMessage::Event(WidgetEvent {
                    widget_id,
                    kind: EventKind::Other(tag),
                }) if tag == CLOSE_TAG && widget_id == &self.id => {
                    // 点击遮罩：关闭，并调度重绘让收折动画真正跑起来
                    // （否则没有 RedrawRequested 帧，菜单会一直留在屏幕上）。
                    *self.open = false;
                    shell.request_redraw();
                }
                LayoutMessage::Event(WidgetEvent {
                    widget_id,
                    kind: EventKind::Pressed,
                }) if self.items.iter().any(|(_, key)| key == widget_id) => {
                    // 选中一项：关闭、转发事件给应用，并调度收折动画。
                    *self.open = false;
                    shell.publish(message);
                    shell.request_redraw();
                }
                _ => shell.publish(message),
            }
        }

        // 把菜单/遮罩内部的事件捕获与重绘请求传播给运行时：
        // 否则运行时以为 overlay 没处理事件，点击还会“漏”给底层控件
        // （例如点遮罩时误触发触发器）。
        if captured {
            shell.capture_event();
        }
        if !matches!(redraw_request, window::RedrawRequest::Wait) {
            shell.request_redraw_at(redraw_request);
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if let Some(menu_layout) = layout.children().nth(1) {
            self.menu.as_widget().mouse_interaction(
                &self.overlay_tree.children[1],
                menu_layout,
                cursor,
                &self.viewport,
                renderer,
            )
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let scale = 0.95 + 0.05 * self.anim;
        if let Some(menu_layout) = layout.children().nth(1) {
            let bounds = menu_layout.bounds();
            // 以面板右上角为锚点缩放（nipaplay 的 topRight）。
            let anchor = Point::new(bounds.x + bounds.width, bounds.y);
            renderer.with_transformation(
                Transformation::translate(anchor.x, anchor.y)
                    * Transformation::scale(scale)
                    * Transformation::translate(-anchor.x, -anchor.y),
                |renderer| {
                    self.menu.as_widget().draw(
                        &self.overlay_tree.children[1],
                        renderer,
                        theme,
                        style,
                        menu_layout,
                        cursor,
                        &self.viewport,
                    );
                },
            );
        }
    }

    fn index(&self) -> f32 {
        2.0
    }
}

/// 下拉菜单项（自绘）。
///
/// 不能用 iced 的 `Button`：它的 hover 状态存在控件实例字段上，而
/// overlay 的绘制阶段会**重新创建**实例（iced 两段式 overlay），
/// update 阶段算出的悬浮在 draw 时丢失，回退成 `Disabled`。这里悬浮在
/// draw 里用光标即时判定，跨实例仍然正确；press/release 用树状态处理。
struct DropdownItem<'a> {
    label: Element<'a, LayoutMessage>,
    /// 按下时发布的事件（`(item_key, Pressed)`）。
    on_press: LayoutMessage,
    colors: Colors,
    selected: bool,
    /// 打开/关闭动画进度（0..1），用于文字淡入淡出。
    anim: f32,
}

/// 悬浮/按下标记：存在树里，跨 overlay 两段式实例仍然有效。
#[derive(Default)]
struct ItemState {
    is_pressed: bool,
    hovered: bool,
}

impl<'a> Widget<LayoutMessage, iced::Theme, iced::Renderer> for DropdownItem<'a> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        layout::padded(
            limits,
            Length::Fill,
            Length::Shrink,
            [8, 16],
            |limits| {
                self.label
                    .as_widget_mut()
                    .layout(&mut tree.children[0], renderer, limits)
            },
        )
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ItemState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ItemState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.label.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children[0].diff(self.label.as_widget());
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, LayoutMessage>,
        _viewport: &Rectangle,
    ) {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let state = tree.state.downcast_mut::<ItemState>();
                let over = cursor.is_over(layout.bounds());
                // 悬浮变化必须请求重绘，否则没有 RedrawRequested 帧，
                // 高亮不会实时刷新（iced 自己的菜单浮层也是这么做的）。
                if state.hovered != over {
                    state.hovered = over;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.is_over(layout.bounds()) {
                    tree.state.downcast_mut::<ItemState>().is_pressed = true;
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let state = tree.state.downcast_mut::<ItemState>();
                if state.is_pressed {
                    state.is_pressed = false;
                    if cursor.is_over(layout.bounds()) {
                        shell.publish(self.on_press.clone());
                    }
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let is_pressed = tree.state.downcast_ref::<ItemState>().is_pressed;
        let background = if is_pressed || cursor.is_over(bounds) {
            self.colors.item_hover_bg
        } else if self.selected {
            self.colors.item_selected_bg
        } else {
            Color::TRANSPARENT
        };
        renderer.fill_quad(
            iced::advanced::renderer::Quad {
                bounds,
                ..Default::default()
            },
            background,
        );

        if let Some(label_layout) = layout.children().next() {
            let text = self.colors.text;
            self.label.as_widget().draw(
                &tree.children[0],
                renderer,
                _theme,
                &Style {
                    text_color: Color::from_rgba(text.r, text.g, text.b, text.a * self.anim),
                },
                label_layout,
                cursor,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> From<DropdownItem<'a>> for Element<'a, LayoutMessage> {
    fn from(widget: DropdownItem<'a>) -> Self {
        Element::new(widget)
    }
}

impl<'a> From<DropdownView<'a>> for Element<'a, LayoutMessage> {
    fn from(widget: DropdownView<'a>) -> Self {
        Element::new(widget)
    }
}

/// 绘制触发器右侧的 V 形箭头，随 `anim` 绕中心旋转 0..180°。
///
/// iced 的 `Transformation` 不支持旋转，直接旋转路径顶点；几何使用
/// 无限裁剪区 + 绝对坐标（绕开 tiny-skia 对非原点几何的双重裁剪 bug）。
fn draw_chevron(renderer: &mut iced::Renderer, bounds: Rectangle, anim: f32, color: Color) {
    let angle = std::f32::consts::PI * anim;
    let size = 12.0_f32;
    let cx = bounds.x + bounds.width - 12.0 - size / 2.0;
    let cy = bounds.y + bounds.height / 2.0;
    let local = [[-3.0, -2.0], [0.0, 2.0], [3.0, -2.0]];

    let mut builder = canvas::path::Builder::new();
    let mut first = true;
    for [dx, dy] in local {
        let x = cx + dx * angle.cos() - dy * angle.sin();
        let y = cy + dx * angle.sin() + dy * angle.cos();
        let point = Point::new(x, y);
        if first {
            builder.move_to(point);
            first = false;
        } else {
            builder.line_to(point);
        }
    }

    let mut frame = canvas::Frame::with_bounds(renderer, Rectangle::INFINITE);
    frame.stroke(
        &builder.build(),
        canvas::Stroke {
            style: canvas::Style::Solid(color),
            width: 1.6,
            line_cap: canvas::LineCap::Round,
            line_join: canvas::LineJoin::Round,
            ..Default::default()
        },
    );
    renderer.draw_geometry(frame.into_geometry());
}
