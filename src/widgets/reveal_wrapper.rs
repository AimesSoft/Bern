//! The engine-level theme-reveal wrapper.
//!
//! The registry wraps **every** control (except the background itself) in a
//! [`RevealWrapper`]. During a two-phase theme reveal the mode has not
//! switched yet, so controls would otherwise keep their old colors until the
//! whole animation finishes. This wrapper:
//!
//! 1. subscribes the control's position to the [`ThemeReveal`] hub
//!    (event-driven, no polling);
//! 2. when the sweep reaches it, receives the one-shot command and rebuilds
//!    the control with the **target** theme via its build closure;
//! 3. restores the current theme color when the reveal ends.
//!
//! Controls do not implement any of this themselves — it is automatic.

use crate::core::ui::ThemeReveal;
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::{Element, Length, Rectangle, Size};
use std::sync::Arc;

/// Rebuilds a control element for a given theme.
pub type Rebuild<'a, Message> = Arc<dyn Fn(&iced::Theme) -> Element<'a, Message> + 'a>;

/// Wraps a control element and switches it to the target theme when the
/// reveal sweep covers its position.
pub struct RevealWrapper<'a, Message> {
    child: Element<'a, Message>,
    /// Rebuilds the control for a given theme.
    rebuild: Rebuild<'a, Message>,
    /// The theme in effect when the wrapper was built (for reset).
    current_theme: iced::Theme,
    reveal: ThemeReveal,
}

impl<'a, Message> RevealWrapper<'a, Message> {
    /// Wraps a control element.
    pub fn new(
        child: Element<'a, Message>,
        rebuild: Rebuild<'a, Message>,
        current_theme: iced::Theme,
        reveal: ThemeReveal,
    ) -> Self {
        Self {
            child,
            rebuild,
            current_theme,
            reveal,
        }
    }
}

/// Subscriber state stored in the widget tree.
#[derive(Default)]
struct State {
    subscriber: Option<u64>,
    subscribed_epoch: u64,
    covered: bool,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for RevealWrapper<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.child.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State::default()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.child.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        // 必须走 `Tree::diff_children`（内部按 tag 检查、类型变了就重建
        // 子树状态）。直接调 `child.diff()` 会绕过检查，切页/换内容时
        // 子树还是旧类型，后续 layout/update 的 downcast 会崩溃。
        tree.diff_children(std::slice::from_ref(&self.child));
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();

        if self.reveal.is_active() {
            let center = layout.bounds().center();
            if state.subscribed_epoch != self.reveal.epoch() {
                state.subscriber = Some(self.reveal.subscribe((center.x, center.y)));
                state.subscribed_epoch = self.reveal.epoch();
            } else if let Some(subscriber) = state.subscriber {
                self.reveal
                    .update_position(subscriber, (center.x, center.y));
            }

            // The sweep reached this control: rebuild it with the target
            // theme so its colors match the new background underneath.
            if !state.covered
                && let Some(subscriber) = state.subscriber
                && self.reveal.take_command(subscriber)
            {
                state.covered = true;
                if let Some(target) = self.reveal.target() {
                    // The next build diff reconciles the child tree.
                    self.child = (self.rebuild)(&target);
                }
                shell.request_redraw();
            }
        } else if state.covered {
            // Reveal over: restore the current theme.
            state.covered = false;
            self.child = (self.rebuild)(&self.current_theme);
            shell.request_redraw();
        }

        // Forward events to the wrapped control (clicks, hover, ...). Our
        // layout node is the child's own node, so the same layout applies.
        self.child.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.child.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.child.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<RevealWrapper<'a, Message>> for Element<'a, Message> {
    fn from(widget: RevealWrapper<'a, Message>) -> Self {
        Element::new(widget)
    }
}
