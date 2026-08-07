//! The widget registry and the layout runtime: turns a flat RON layout into
//! an iced widget tree.

use crate::core::id::IdRegistry;
use crate::core::layout::{Area, AreaKind, Layout, Widget};
use crate::core::store::LayoutStore;
use crate::core::theme::ThemeRouter;
use crate::core::ui::{PressOrigin, ThemeReveal};
use crate::core::widget::{BuildContext, BuildError, LayoutMessage, WidgetDef};
use crate::widgets::reveal_wrapper::{Rebuild, RevealWrapper};
use iced::Element;
use iced::widget::{Column, Row, Stack};
use std::collections::HashMap;
use std::sync::Arc;

/// Maps widget type names (from layout files) to their implementations.
#[derive(Default)]
pub struct Registry {
    widgets: HashMap<&'static str, Box<dyn WidgetDef>>,
    press_origin: PressOrigin,
    theme_reveal: ThemeReveal,
    ids: IdRegistry,
}

impl Registry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a control under its [`WidgetDef::name`].
    pub fn register(&mut self, widget: impl WidgetDef + 'static) {
        self.widgets.insert(widget.name(), Box::new(widget));
    }

    /// Returns the control registered under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&dyn WidgetDef> {
        self.widgets.get(name).map(|b| b.as_ref())
    }

    /// Whether a control with this name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.widgets.contains_key(name)
    }

    /// The names of all registered controls.
    pub fn widget_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.widgets.keys().copied()
    }

    /// The shared press-origin store.
    pub fn press_origin(&self) -> &PressOrigin {
        &self.press_origin
    }

    /// The theme-reveal notification hub.
    pub fn theme_reveal(&self) -> &ThemeReveal {
        &self.theme_reveal
    }

    /// The central widget-id registry.
    pub fn ids(&self) -> &IdRegistry {
        &self.ids
    }

    /// Builds a whole layout into one iced element.
    ///
    /// Ordering rule inside an area: child areas first, then widgets (sorted
    /// by `z`, which matters in [`AreaKind::Stack`]).
    pub fn build<'a>(
        &'a self,
        layout: &'a Layout,
        router: &'a ThemeRouter,
        store: &'a LayoutStore,
    ) -> Result<Element<'a, LayoutMessage>, BuildError> {
        let ctx = BuildContext::root(
            router.theme(),
            self,
            store,
            &self.press_origin,
            &self.theme_reveal,
            &self.ids,
        );
        self.build_embedded(layout, &ctx)
    }

    /// Builds a layout (possibly an embedded one) with the given context.
    pub fn build_embedded<'a>(
        &'a self,
        layout: &'a Layout,
        ctx: &BuildContext<'a, '_>,
    ) -> Result<Element<'a, LayoutMessage>, BuildError> {
        self.validate(layout)?;
        let roots: Vec<&Area> = layout.areas.iter().filter(|a| a.parent.is_none()).collect();
        match roots.as_slice() {
            [root] => self.build_area(ctx, root, layout),
            [] => Err(BuildError::InvalidLayout(
                "no root area: every area has a `parent`".into(),
            )),
            _ => Err(BuildError::InvalidLayout(
                "multiple root areas: exactly one area must have no `parent`".into(),
            )),
        }
    }

    /// Validates ids, area references, and widget types before building.
    fn validate(&self, layout: &Layout) -> Result<(), BuildError> {
        let mut ids: HashMap<&str, &str> = HashMap::new();
        for area in &layout.areas {
            if ids.insert(&area.id, "area").is_some() {
                return Err(BuildError::InvalidLayout(format!(
                    "duplicate id `{}`",
                    area.id
                )));
            }
        }
        for widget in &layout.widgets {
            if ids.insert(&widget.id, "widget").is_some() {
                return Err(BuildError::InvalidLayout(format!(
                    "duplicate id `{}`",
                    widget.id
                )));
            }
            if !self.contains(&widget.kind) {
                return Err(BuildError::UnknownWidget(widget.kind.clone()));
            }
            if self.get(&widget.kind).is_some_and(WidgetDef::interactive)
                && !self.ids.contains(&widget.id)
            {
                return Err(BuildError::UnregisteredId(widget.id.clone()));
            }
            // `h_tab` declares per-item interaction ids in its `items` prop;
            // each key must be registered in the central ids.rs as well.
            if widget.kind == crate::widgets::h_tab::NAME {
                let keys =
                    crate::widgets::h_tab::validate_items(widget.str_prop("items").unwrap_or(""))
                        .map_err(|reason| {
                        BuildError::InvalidLayout(format!("h_tab `{}`: {reason}", widget.id))
                    })?;
                for key in keys {
                    if !self.ids.contains(&key) {
                        return Err(BuildError::UnregisteredId(key));
                    }
                }
            }
            if !layout.areas.iter().any(|a| a.id == widget.area) {
                return Err(BuildError::InvalidLayout(format!(
                    "widget `{}` references unknown area `{}`",
                    widget.id, widget.area
                )));
            }
        }
        for area in &layout.areas {
            if let Some(parent) = &area.parent
                && !layout.areas.iter().any(|a| &a.id == parent)
            {
                return Err(BuildError::InvalidLayout(format!(
                    "area `{}` references unknown parent `{}`",
                    area.id, parent
                )));
            }
        }
        Ok(())
    }

    /// Builds one area and everything inside it.
    fn build_area<'a>(
        &'a self,
        ctx: &BuildContext<'a, '_>,
        area: &'a Area,
        layout: &'a Layout,
    ) -> Result<Element<'a, LayoutMessage>, BuildError> {
        // Items in this area: child areas first (z = 0), then widgets
        // (z from the file).
        let mut items: Vec<(i32, u8, usize, Element<'a, LayoutMessage>)> = Vec::new();
        for (index, child) in layout
            .areas
            .iter()
            .filter(|a| a.parent.as_deref() == Some(area.id.as_str()))
            .enumerate()
        {
            items.push((0, 0, index, self.build_area(ctx, child, layout)?));
        }

        let mut widgets: Vec<&Widget> = layout
            .widgets
            .iter()
            .filter(|w| w.area == area.id)
            .collect();
        widgets.sort_by_key(|w| w.z);
        for (index, widget) in widgets.into_iter().enumerate() {
            let def = self
                .get(&widget.kind)
                .ok_or_else(|| BuildError::UnknownWidget(widget.kind.clone()))?;
            let element = self.build_widget(def, widget, ctx);
            items.push((widget.z, 1, index, element));
        }

        // Stack respects `z`; flow containers order by declaration
        // (child areas first, then widgets).
        let children: Vec<Element<'a, LayoutMessage>> = match area.kind {
            AreaKind::Stack => {
                items.sort_by_key(|(z, priority, index, _)| (*z, *priority, *index));
                items.into_iter().map(|(_, _, _, e)| e).collect()
            }
            AreaKind::Row | AreaKind::Column => {
                items.sort_by_key(|(_, priority, index, _)| (*priority, *index));
                items.into_iter().map(|(_, _, _, e)| e).collect()
            }
        };

        Ok(match area.kind {
            AreaKind::Row => {
                let mut row = Row::with_children(children);
                if let Some(spacing) = area.spacing {
                    row = row.spacing(spacing);
                }
                if let Some(padding) = area.padding {
                    row = row.padding(padding);
                }
                row.into()
            }
            AreaKind::Column => {
                let mut column = Column::with_children(children);
                if let Some(spacing) = area.spacing {
                    column = column.spacing(spacing);
                }
                if let Some(padding) = area.padding {
                    column = column.padding(padding);
                }
                column.into()
            }
            AreaKind::Stack => Stack::with_children(children).into(),
        })
    }

    /// Builds one widget element, wrapping it in the engine-level theme-reveal
    /// wrapper so it automatically follows the background sweep (the
    /// background control itself opts out).
    fn build_widget<'a, 't>(
        &'a self,
        def: &'a dyn WidgetDef,
        widget: &'a Widget,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let element = def.build(widget, widget.size, ctx);
        if !def.follows_theme_reveal() {
            return element;
        }

        // Rebuild closure: re-runs the same control's build with another
        // theme, so the wrapper can switch its colors when the sweep reaches
        // it without any per-control code.
        let size = widget.size;
        let registry = self;
        let store = ctx.store;
        let press_origin = ctx.press_origin;
        let theme_reveal = ctx.theme_reveal;
        let ids = ctx.ids;
        let rebuild: Rebuild<'a, LayoutMessage> = Arc::new(move |theme: &iced::Theme| {
            let build_ctx =
                BuildContext::root(theme, registry, store, press_origin, theme_reveal, ids);
            def.build(widget, size, &build_ctx)
        });

        RevealWrapper::new(
            element,
            rebuild,
            ctx.theme.clone(),
            ctx.theme_reveal.clone(),
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::store::LayoutStore;
    use crate::core::theme::ThemeRouter;

    #[test]
    fn builds_a_tree_from_ron_files() {
        let layout = Layout::parse(
            r#"
            Layout(
                name: "test",
                areas: [
                    Area(id: "root", kind: Column, padding: 8, spacing: 4),
                ],
                widgets: [
                    Widget(id: "greeting", kind: "text", area: "root", props: { "text": "hi" }),
                    Widget(id: "go", kind: "button", area: "root", props: { "label": "Go" }),
                    Widget(id: "icon", kind: "icon_button", area: "root", props: { "icon": "add_rounded" }),
                    Widget(id: "heart", kind: "icon", area: "root", props: { "name": "favorite_rounded", "size": "16" }),
                ],
            )
            "#,
        )
        .expect("layout parses");

        let registry = crate::builtin_registry();
        let store = LayoutStore::new();
        registry.ids().register_all(["go", "icon"]);
        let router = ThemeRouter::new(iced::Theme::Dark);
        let element = registry
            .build(&layout, &router, &store)
            .expect("tree builds");
        let _ = element;
    }

    #[test]
    fn unknown_widget_is_reported() {
        let layout = Layout::parse(
            r#"
            Layout(
                areas: [
                    Area(id: "root", kind: Column),
                ],
                widgets: [
                    Widget(id: "x", kind: "no_such_widget", area: "root"),
                ],
            )
            "#,
        )
        .expect("layout parses");
        let registry = crate::builtin_registry();
        let store = LayoutStore::new();
        let router = ThemeRouter::new(iced::Theme::Light);
        let result = registry.build(&layout, &router, &store);
        assert_eq!(
            result.err(),
            Some(BuildError::UnknownWidget("no_such_widget".into()))
        );
    }

    #[test]
    fn interactive_widget_without_declared_id_is_rejected() {
        let layout = Layout::parse(
            r#"
            Layout(
                areas: [
                    Area(id: "root", kind: Column),
                ],
                widgets: [
                    Widget(id: "go", kind: "button", area: "root", props: { "label": "Go" }),
                ],
            )
            "#,
        )
        .expect("layout parses");
        let registry = crate::builtin_registry();
        let store = LayoutStore::new();
        let router = ThemeRouter::new(iced::Theme::Light);
        let result = registry.build(&layout, &router, &store);
        assert_eq!(result.err(), Some(BuildError::UnregisteredId("go".into())));
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let layout = Layout::parse(
            r#"
            Layout(
                areas: [
                    Area(id: "root", kind: Column),
                ],
                widgets: [
                    Widget(id: "a", kind: "text", area: "root", props: { "text": "1" }),
                    Widget(id: "a", kind: "text", area: "root", props: { "text": "2" }),
                ],
            )
            "#,
        )
        .expect("layout parses");
        let registry = crate::builtin_registry();
        let store = LayoutStore::new();
        let router = ThemeRouter::new(iced::Theme::Light);
        let err = registry
            .build(&layout, &router, &store)
            .err()
            .expect("duplicate ids rejected");
        assert!(
            matches!(err, BuildError::InvalidLayout(ref msg) if msg.contains("duplicate id")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn example_files_load_and_build() {
        let registry = crate::builtin_registry();
        let store =
            LayoutStore::load("layouts/desktop", "layouts/common").expect("layout store loads");
        assert!(
            store.resolve("login_form").is_some(),
            "common block missing"
        );
        registry
            .ids()
            .register_all(["username", "password", "login"]);

        let page = store.resolve("login_page").expect("page layout resolves");
        let router = ThemeRouter::new(iced::Theme::Dark);
        let _ = registry
            .build(page, &router, &store)
            .expect("page tree builds (with embedded login_form)");
    }
}
