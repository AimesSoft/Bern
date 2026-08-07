//! The widget registry and the layout runtime: turns a flat RON layout into
//! an iced widget tree.

use crate::core::layout::{Area, AreaKind, Layout, Widget};
use crate::core::store::LayoutStore;
use crate::core::theme::ThemeRouter;
use crate::core::widget::{BuildContext, BuildError, LayoutMessage, WidgetDef};
use iced::widget::{Column, Row, Stack};
use iced::Element;
use std::collections::HashMap;

/// Maps widget type names (from layout files) to their implementations.
#[derive(Default)]
pub struct Registry {
    widgets: HashMap<&'static str, Box<dyn WidgetDef>>,
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
        let ctx = BuildContext::root(router.theme(), self, store);
        self.build_embedded(layout, &ctx)
    }

    /// Builds a layout (possibly an embedded one) with the given context.
    pub fn build_embedded<'a>(
        &'a self,
        layout: &'a Layout,
        ctx: &BuildContext<'a>,
    ) -> Result<Element<'a, LayoutMessage>, BuildError> {
        self.validate(layout)?;
        let roots: Vec<&Area> = layout
            .areas
            .iter()
            .filter(|a| a.parent.is_none())
            .collect();
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
        ctx: &BuildContext<'a>,
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
            let element = def.build(widget, widget.size, ctx);
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
                    Widget(id: "icon", kind: "icon_button", area: "root", props: { "icon": "add" }),
                    Widget(id: "heart", kind: "icon", area: "root", props: { "name": "favorite", "size": "16" }),
                ],
            )
            "#,
        )
        .expect("layout parses");

        let registry = crate::builtin_registry();
        let store = LayoutStore::new();
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
        let store = LayoutStore::load("layouts/desktop", "layouts/common")
            .expect("layout store loads");
        assert!(store.resolve("login_form").is_some(), "common block missing");

        let page = store.resolve("login_page").expect("page layout resolves");
        let router = ThemeRouter::new(iced::Theme::Dark);
        let _ = registry
            .build(page, &router, &store)
            .expect("page tree builds (with embedded login_form)");
    }
}
