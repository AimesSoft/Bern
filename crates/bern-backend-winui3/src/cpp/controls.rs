//! Controls: buttons, text fields, search fields, switches, sliders,
//! pickers, menus, and the areas that take drops and gestures.

use super::{Generator, Out, designated, motion_ref, null, string, wide};
use bern_ir::{Element, ExprKind, Node};
use bern_protocol::{EnumDef, Type};

impl Generator<'_> {
    /// `B::Shortcut(…)` for an element with `shortcut:`, running the
    /// control's `activate` (or `focus` for search fields) while mounted.
    fn shortcut(&self, out: &mut Out, element: &Element, action: &str) {
        if let Some(keys) = element.keyword("shortcut") {
            out.line(format!(
                "B::Shortcut(c, {}, {}, {});",
                string(keys),
                self.id(element),
                string(action)
            ));
        }
    }

    /// `Button` and `IconButton`; a `Button` with children shows them
    /// instead of its label.
    pub(super) fn button(&self, out: &mut Out, element: &Element) {
        let kind = element.kind.as_str();
        let props = designated(&[
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            (
                "icon",
                if kind == "IconButton" {
                    Some(string(element.icon("icon").unwrap_or("circle")))
                } else {
                    Self::icon(element, "icon")
                },
            ),
            ("role", Self::keyword(element, "role")),
            ("prominence", Self::keyword(element, "prominence")),
            ("enabled", self.enabled(element)),
            ("action", self.action(element)),
            ("style", self.style(kind, element.keyword("variant"))),
            ("pick", self.field(element, "pick")),
            ("files", Self::files(element)),
            ("shape", self.shape(element)),
            (
                "fill",
                (element.keyword("width") == Some("fill")).then(|| "true".to_string()),
            ),
            ("iconSize", Self::number(element, "font_size")),
        ]);
        if kind == "Button" && !element.children.is_empty() {
            self.call(
                out,
                &format!("B::ContentButton(p, c, {props}, "),
                &element.children,
            );
        } else {
            out.line(format!("B::{kind}(p, c, {props});"));
        }
        self.shortcut(out, element, "activate");
        if element.keyword("default_focus") == Some("true") {
            out.line(format!("B::DefaultFocus(c, {});", self.id(element)));
        }
    }

    pub(super) fn text_input(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::TextInput(p, c, {});",
            designated(&[
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "labelHidden",
                    (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
                ),
                ("field", self.field(element, "value")),
                ("prompt", self.text_prop(element, "prompt")),
                ("kind", Self::keyword(element, "kind")),
                ("submit", self.action(element)),
            ])
        ));
    }

    /// A `Search` field, with its results below it when it has children.
    pub(super) fn search(&self, out: &mut Out, element: &Element) {
        let props = self.search_props(element);
        // A palette opens over the window; its shortcut opens it.
        let (function, run) = if element.keyword("style") == Some("palette") {
            ("SearchPalette", "activate")
        } else {
            ("Search", "focus")
        };
        if element.children.is_empty() {
            out.line(format!("B::{function}(p, c, {props}, nullptr);"));
        } else {
            self.call(
                out,
                &format!("B::{function}(p, c, {props}, "),
                &element.children,
            );
        }
        self.shortcut(out, element, run);
    }

    pub(super) fn search_item(&self, out: &mut Out, element: &Element) {
        let props = designated(&[
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", Self::icon(element, "icon")),
            ("detail", self.text_prop(element, "detail")),
            ("hint", self.text_prop(element, "hint")),
            (
                "navigation",
                element.expr("navigation").map(|e| self.getter(e)),
            ),
            ("action", self.action(element)),
            ("pick", self.field(element, "pick")),
            ("files", Self::files(element)),
        ]);
        if element.children.is_empty() {
            out.line(format!("B::SearchItem(p, c, {props}, nullptr);"));
        } else {
            self.call(
                out,
                &format!("B::SearchItem(p, c, {props}, "),
                &element.children,
            );
        }
    }

    pub(super) fn search_props(&self, element: &Element) -> String {
        designated(&[
            ("id", Some(self.id(element))),
            ("field", self.field(element, "value")),
            ("prompt", self.text_prop(element, "prompt")),
            ("submit", self.action(element)),
            ("panelPrompt", self.text_prop(element, "panel_prompt")),
            ("width", Self::number(element, "width")),
            ("height", Self::number(element, "height")),
            ("fontSize", Self::number(element, "font_size")),
            (
                "suggest",
                (element.keyword("suggest") == Some("true")).then(|| "true".into()),
            ),
            // A palette grows out of its field on the hero spring, follows
            // its results on the bouncy one and goes back on the snappy one.
            (
                "grow",
                (element.keyword("style") == Some("palette")).then(|| motion_ref("hero")),
            ),
            (
                "follow",
                (element.keyword("style") == Some("palette")).then(|| motion_ref("bouncy")),
            ),
            (
                "shrink",
                (element.keyword("style") == Some("palette")).then(|| motion_ref("snappy")),
            ),
        ])
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element) {
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        fields.extend(self.model(element, "checked"));
        fields.extend([
            ("style", Self::keyword(element, "style")),
            ("icon", Self::icon(element, "icon")),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
            ("shape", self.shape(element)),
        ]);
        out.line(format!("B::Switch(p, c, {});", designated(&fields)));
        self.shortcut(out, element, "activate");
    }

    pub(super) fn slider(&self, out: &mut Out, element: &Element) {
        let controlled = element.expr("current").is_some();
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        if controlled {
            fields.extend(self.model(element, "current"));
        } else {
            fields.push(("field", self.field(element, "value")));
        }
        fields.extend([
            ("min", Self::number(element, "min")),
            ("max", Self::number(element, "max")),
            ("step", Self::number(element, "step")),
            ("minIcon", Self::icon(element, "min_icon")),
            ("maxIcon", Self::icon(element, "max_icon")),
            // A controlled slider's action is its change.
            (
                "commit",
                if controlled {
                    None
                } else {
                    self.action(element)
                },
            ),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
        ]);
        out.line(format!("B::Slider(p, c, {});", designated(&fields)));
    }

    pub(super) fn picker(&self, out: &mut Out, element: &Element) {
        let ty = element
            .expr("selected")
            .map(|expr| &expr.ty)
            .or_else(|| element.field("value").map(|field| &field.ty));
        let Some(Type::Enum { name }) = ty else {
            return;
        };
        let def = self.types().enum_def(name).expect("enum");
        // `PickerOption`s list their own values and labels; without them
        // every variant is an option.
        let explicit: Vec<_> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(option) if option.kind == "PickerOption" => Some(option),
                _ => None,
            })
            .filter_map(|option| {
                let value = option.expr("value")?;
                let ExprKind::Variant { variant, .. } = &value.kind else {
                    return None;
                };
                Some(format!(
                    "B::PickOption{{{variant}, {}, {}}}",
                    self.variant_name(value),
                    self.text_prop(option, "label")?
                ))
            })
            .collect();
        let options: Vec<_> = if explicit.is_empty() {
            def.variants
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    format!(
                        "B::PickOption{{{index}, {}, {}}}",
                        string(&variant.name),
                        self.variant_label(def, index)
                    )
                })
                .collect()
        } else {
            explicit
        };
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        fields.extend(self.model(element, "selected"));
        fields.extend([
            ("options", Some(format!("{{{}}}", options.join(", ")))),
            ("style", Self::keyword(element, "style")),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
            ("shape", self.shape(element)),
        ]);
        out.line(format!("B::Picker(p, c, {});", designated(&fields)));
    }

    /// The label getter of a variant: its declared label, or its name.
    fn variant_label(&self, def: &EnumDef, index: usize) -> String {
        let variant = &def.variants[index];
        if variant.label.is_some() {
            format!(
                "[](Ctx const&) {{ return bern::Text::Label({}, {index}); }}",
                string(&def.name)
            )
        } else {
            format!(
                "[](Ctx const&) {{ return winrt::hstring({}); }}",
                wide(&variant.name)
            )
        }
    }

    pub(super) fn language_picker(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::LanguagePicker(p, c, {}, {});",
            self.id(element),
            null(self.text_prop(element, "label")),
        ));
    }

    pub(super) fn popover_button(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::PopoverButton(p, c, {}, ",
                designated(&[
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("icon", Self::icon(element, "icon")),
                    ("presented", self.field(element, "presented")),
                ])
            ),
            &element.children,
        );
        self.shortcut(out, element, "activate");
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::SplitButton(p, c, {}, ",
                designated(&[
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("icon", Self::icon(element, "icon")),
                    ("field", self.field(element, "value")),
                    ("more", self.text_prop(element, "more_label")),
                    ("shape", self.shape(element)),
                ])
            ),
            &element.children,
        );
        self.shortcut(out, element, "activate");
    }

    /// A menu button and its items; dividers are empty items.
    pub(super) fn menu(&self, out: &mut Out, element: &Element) {
        out.open(format!(
            "B::Menu(p, c, {}, {}, {}, {}, std::vector<std::optional<B::MenuItemDef>>{{",
            self.id(element),
            null(self.text_prop(element, "label")),
            null(Self::icon(element, "icon")),
            designated(&[
                (
                    "iconOnly",
                    (element.keyword("style") == Some("icon")).then(|| "true".to_string())
                ),
                ("shape", self.shape(element)),
            ])
            .replacen('{', "B::MenuLook{", 1),
        ));
        let mut shortcuts = Vec::new();
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("std::nullopt,");
                continue;
            }
            if item.keyword("shortcut").is_some() {
                shortcuts.push(item);
            }
            out.line(format!(
                "B::MenuItemDef{},",
                designated(&[
                    ("id", Some(self.id(item))),
                    ("label", self.text_prop(item, "label")),
                    ("icon", Self::icon(item, "icon")),
                    ("role", Self::keyword(item, "role")),
                    ("enabled", self.enabled(item)),
                    ("action", self.action(item)),
                    ("pick", self.field(item, "pick")),
                    ("files", Self::files(item)),
                    ("shortcut", Self::keyword(item, "shortcut")),
                ])
            ));
        }
        out.close("});");
        for item in shortcuts {
            self.shortcut(out, item, "activate");
        }
    }

    /// `Drop`: its content takes a file dragged onto it.
    pub(super) fn drop_area(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Drop(p, c, {}, ",
                designated(&[
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("files", Self::keyword(element, "files")),
                    ("over", self.field(element, "over")),
                    (
                        "opens",
                        (element.keyword("opens") == Some("true")).then(|| "true".to_string())
                    ),
                    ("action", self.action(element)),
                ])
            ),
            &element.children,
        );
    }

    /// `Pinch` and `Swipe`: gestures on their content that move `value`.
    pub(super) fn gesture(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::{}(p, c, {}, ",
                element.kind,
                designated(&[
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("min", Self::number(element, "min")),
                    ("max", Self::number(element, "max")),
                    ("axis", Self::keyword(element, "axis")),
                    ("distance", Self::number(element, "distance")),
                    ("action", self.action(element)),
                ])
            ),
            &element.children,
        );
    }
}
