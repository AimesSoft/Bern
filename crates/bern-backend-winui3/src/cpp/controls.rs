//! Controls: buttons, text fields, search fields, switches, sliders,
//! pickers, menus, and the areas that take drops and gestures.

use super::{BUILDER, Generator, Out, designated, motion_ref, null, string, wide};
use bern_ir::{Element, ExprKind, Node, Prop};
use bern_protocol::{EnumDef, Type};

impl Generator<'_> {
    /// Non-rendering action children belong to the immediate control only.
    fn child_action(&self, element: &Element, kind: &str) -> Option<String> {
        element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == kind => self.action(child),
            _ => None,
        })
    }

    /// `B::Shortcut(…)` for an element with `shortcut:`, running the
    /// control's `activate` (or `focus` for search fields) while mounted.
    pub(super) fn shortcut(&self, out: &mut Out, element: &Element, action: &str) {
        if let Some(keys) = self
            .shortcut_keys(element, "shortcut")
            .or_else(|| element.keyword("shortcut").map(string))
        {
            out.line(format!(
                "B::Shortcut(c, {keys}, {}, {});",
                self.id(element),
                string(action)
            ));
        }
    }

    /// The text a shortcut prop `name` follows, as a getter, when it is not
    /// a literal.
    pub(super) fn shortcut_keys(&self, element: &Element, name: &str) -> Option<String> {
        element
            .expr(name)
            .map(|expr| format!("[](Ctx const& c) {{ return {}; }}", self.display(expr)))
    }

    /// `ShortcutInput`: a button that takes the next key pressed as the
    /// shortcut, which the action keeps.
    pub(super) fn shortcut_input(&self, out: &mut Out, element: &Element) {
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        fields.extend(self.model(element, "current"));
        fields.extend([
            ("prompt", self.text_prop(element, "prompt")),
            ("empty", self.text_prop(element, "empty")),
            ("enabled", self.enabled(element)),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
        ]);
        out.line(format!("B::ShortcutInput(p, c, {});", designated(&fields)));
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
            ("cursor", Self::keyword(element, "cursor")),
            ("enabled", self.enabled(element)),
            ("action", self.action(element)),
            ("style", self.style(kind, element.keyword("variant"))),
            ("pick", self.field(element, "pick")),
            ("save", self.field(element, "save")),
            ("suggestedName", self.text_prop(element, "suggested_name")),
            ("files", Self::files(element)),
            ("paste", self.field(element, "paste")),
            ("shape", self.shape(element)),
            (
                "fill",
                (element.keyword("width") == Some("fill")).then(|| "true".to_string()),
            ),
            ("iconSize", Self::number(element, "font_size")),
            (
                "iconProgress",
                element
                    .expr("icon_progress")
                    .map(|value| self.getter(value)),
            ),
            ("badgeIcon", Self::icon(element, "badge_icon")),
            (
                "badgeVisible",
                element
                    .expr("badge_visible")
                    .map(|value| self.getter(value)),
            ),
            (
                "color",
                (kind == "IconButton")
                    .then(|| Self::keyword(element, "color"))
                    .flatten(),
            ),
            ("menu", self.context_menu_def(element)),
            ("doubleTap", self.child_action(element, "DoubleTap")),
        ]);
        if kind == "Button" && element.has_content() {
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

    /// `TextInput`: a draft field, or text read from state (`current:`)
    /// whose every edit runs the action.
    pub(super) fn text_input(&self, out: &mut Out, element: &Element) {
        let controlled = element.expr("current").is_some();
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
        ];
        if controlled {
            fields.extend(self.model(element, "current"));
        } else {
            fields.push(("field", self.field(element, "value")));
        }
        fields.extend([
            ("prompt", self.text_prop(element, "prompt")),
            ("kind", Self::keyword(element, "kind")),
            (
                "submit",
                if controlled {
                    None
                } else {
                    self.action(element)
                },
            ),
        ]);
        out.line(format!("B::TextInput(p, c, {});", designated(&fields)));
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
            (
                "iconOnly",
                (element.keyword("style") == Some("palette")
                    && element.keyword("display") == Some("icon"))
                .then(|| "true".into()),
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
        if element.keyword("login_item") == Some("true") {
            fields.extend([
                ("current", Some("StatusArea::LoginCurrent()".to_string())),
                ("change", Some("StatusArea::LoginChange()".to_string())),
            ]);
        } else {
            fields.extend(self.model(element, "checked"));
        }
        fields.extend([
            ("style", Self::keyword(element, "style")),
            ("icon", Self::icon(element, "icon")),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
            ("shape", self.shape(element)),
            ("look", self.style("Switch", element.keyword("variant"))),
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
            (
                "continuous",
                (element.keyword("continuous") == Some("true")).then(|| "true".to_string()),
            ),
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
            ("hover", self.field(element, "hover")),
            (
                "vertical",
                (element.keyword("axis") == Some("vertical")).then(|| "true".to_string()),
            ),
        ]);
        out.line(format!("B::Slider(p, c, {});", designated(&fields)));
    }

    pub(super) fn color_picker(&self, out: &mut Out, element: &Element) {
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
        out.line(format!("B::ColorPicker(p, c, {});", designated(&fields)));
    }

    pub(super) fn tooltip(&self, out: &mut Out, element: &Element) {
        let props = designated(&[
            ("content", self.text_prop(element, "content")),
            ("keys", Self::keyword(element, "keys")),
            ("placement", Self::keyword(element, "placement")),
        ])
        .replacen('{', "B::TooltipProps{", 1);
        self.call(
            out,
            &format!("B::Tooltip(p, c, {props}, "),
            &element.children,
        );
    }

    pub(super) fn banner(&self, out: &mut Out, element: &Element) {
        let props = designated(&[
            ("id", element.id.as_deref().map(string)),
            ("severity", Self::keyword(element, "severity")),
            ("title", self.text_prop(element, "title")),
            ("content", self.text_prop(element, "content")),
            ("icon", Self::icon(element, "icon")),
            ("dismissLabel", self.text_prop(element, "dismiss_label")),
            ("dismiss", self.action(element)),
        ]);
        if element.children.is_empty() {
            out.line(format!("B::Banner(p, c, {props}, nullptr);"));
        } else {
            self.call(
                out,
                &format!("B::Banner(p, c, {props}, "),
                &element.children,
            );
        }
    }

    pub(super) fn disclosure(&self, out: &mut Out, element: &Element) {
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", Self::icon(element, "icon")),
        ];
        if element.expr("open").is_some() {
            fields.extend(self.model(element, "open"));
        } else if let Some(field) = self.field(element, "expanded") {
            fields.push(("field", Some(field)));
            fields.push(("then", self.action(element)));
        } else {
            fields.push(("then", self.action(element)));
        }
        self.call(
            out,
            &format!("B::Disclosure(p, c, {}, ", designated(&fields)),
            &element.children,
        );
    }

    pub(super) fn number_input(&self, out: &mut Out, element: &Element) {
        let controlled = element.expr("current").is_some();
        let is_int = matches!(element.expr("current").map(|e| &e.ty), Some(Type::Int));
        let precision =
            infer_precision(element.number("precision"), element.number("step"), is_int);
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        if controlled {
            fields.extend(self.model(element, "current"));
        } else {
            fields.push(("field", self.field(element, "value")));
            fields.push(("then", self.action(element)));
        }
        fields.extend([
            ("min", Self::number(element, "min")),
            ("max", Self::number(element, "max")),
            ("step", Self::number(element, "step")),
            ("precision", Some(precision.to_string())),
            ("enabled", self.enabled(element)),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
        ]);
        out.line(format!("B::NumberInput(p, c, {});", designated(&fields)));
    }

    pub(super) fn alert(&self, out: &mut Out, element: &Element) {
        let mut actions = Vec::new();
        for node in &element.children {
            let Node::Element(act) = node else { continue };
            if act.kind != "AlertAction" {
                continue;
            }
            actions.push(format!(
                "B::AlertActionDef{}",
                designated(&[
                    ("id", Some(self.id(act))),
                    ("label", self.text_prop(act, "label")),
                    ("role", Self::keyword(act, "role")),
                    ("enabled", self.enabled(act)),
                    ("action", self.action(act)),
                ])
            ));
        }
        let props = designated(&[
            ("title", self.text_prop(element, "title")),
            ("message", self.text_prop(element, "message")),
            ("actions", Some(format!("{{{}}}", actions.join(", ")))),
            ("dismiss", self.action(element)),
        ]);
        out.line(format!("B::Alert(p, c, {props});"));
    }

    pub(super) fn picker(&self, out: &mut Out, element: &Element) {
        if let Some(items_expr) = element.expr("items") {
            let Type::List { inner } = &items_expr.ty else {
                return;
            };
            let Type::Record { name: rec_name } = inner.as_ref() else {
                return;
            };
            let def = self.types().record_def(rec_name).expect("picker record");
            let Some(Prop::Key { field: key_idx, .. }) = element.prop("key") else {
                return;
            };
            let Some(Prop::Key {
                field: label_idx, ..
            }) = element.prop("item_label")
            else {
                return;
            };
            let key_is_int = def.fields[*key_idx].ty == Type::Int;
            let label_enum = match &def.fields[*label_idx].ty {
                Type::Enum { name } => Some(string(name)),
                _ => None,
            };
            let mut fields = vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
            ];
            fields.extend(self.model(element, "selected"));
            fields.extend([
                ("items", Some(self.getter(items_expr))),
                ("keyField", Some(key_idx.to_string())),
                ("labelField", Some(label_idx.to_string())),
                ("keyIsInt", key_is_int.then(|| "true".to_string())),
                ("labelEnum", label_enum),
                ("prompt", self.text_prop(element, "prompt")),
                ("style", Self::keyword(element, "style")),
                (
                    "labelHidden",
                    (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
                ),
                ("shape", self.shape(element)),
            ]);
            out.line(format!("B::DynamicPicker(p, c, {});", designated(&fields)));
            return;
        }
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
            ("prompt", self.text_prop(element, "prompt")),
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
            "B::LanguagePicker(p, c, {}, {}, {});",
            self.id(element),
            null(self.text_prop(element, "label")),
            element.keyword("label_hidden") == Some("true"),
        ));
    }

    pub(super) fn date_picker(&self, out: &mut Out, element: &Element) {
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        fields.extend(self.model(element, "current"));
        fields.extend([
            ("min", element.expr("min").map(|e| self.getter(e))),
            ("max", element.expr("max").map(|e| self.getter(e))),
            ("enabled", self.enabled(element)),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
        ]);
        out.line(format!("B::DatePicker(p, c, {});", designated(&fields)));
    }

    pub(super) fn time_picker(&self, out: &mut Out, element: &Element) {
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        fields.extend(self.model(element, "current"));
        fields.extend([
            ("min", Self::number(element, "min")),
            ("max", Self::number(element, "max")),
            ("step", Self::number(element, "step")),
            ("enabled", self.enabled(element)),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
        ]);
        out.line(format!("B::TimePicker(p, c, {});", designated(&fields)));
    }

    pub(super) fn empty_state(&self, out: &mut Out, element: &Element) {
        let props = designated(&[
            ("title", self.text_prop(element, "title")),
            ("message", self.text_prop(element, "message")),
            ("icon", Self::icon(element, "icon")),
        ]);
        if element.children.is_empty() {
            out.line(format!("B::EmptyState(p, c, {props}, nullptr);"));
        } else {
            self.call(
                out,
                &format!("B::EmptyState(p, c, {props}, "),
                &element.children,
            );
        }
    }

    pub(super) fn badge(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Badge(p, c, {});",
            designated(&[
                ("content", self.text_prop(element, "content")),
                ("tone", Self::keyword(element, "tone")),
                ("icon", Self::icon(element, "icon")),
            ])
        ));
    }

    pub(super) fn avatar(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Avatar(p, c, {});",
            designated(&[
                ("label", self.text_prop(element, "label")),
                ("source", element.resource("source").map(string)),
                ("url", element.expr("source").map(|e| self.getter(e))),
                ("size", self.len(element, "size")),
                ("shape", Self::keyword(element, "shape")),
            ])
        ));
    }

    pub(super) fn tree_view(&self, out: &mut Out, element: &Element) {
        let (
            Some(items),
            Some(template),
            Some(Prop::Key { field: key_idx, .. }),
            Some(Prop::Key {
                field: parent_idx, ..
            }),
        ) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
            element.prop("parent"),
        )
        else {
            return;
        };
        let props = designated(&[
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("items", Some(self.getter(items))),
            ("key", Some(key_idx.to_string())),
            ("parent", Some(parent_idx.to_string())),
            ("variable", Some(string(&template.var))),
            ("selection", self.field(element, "selection")),
            ("expanded", self.field(element, "expanded")),
            ("action", self.action(element)),
        ]);
        self.call(
            out,
            &format!("B::TreeView(p, c, {props}, "),
            &element.children,
        );
    }

    pub(super) fn split_pane(&self, out: &mut Out, element: &Element) {
        if element.children.len() < 2 {
            return;
        }
        let props = designated(&[
            ("id", element.id.as_deref().map(string)),
            (
                "vertical",
                (element.keyword("axis") == Some("vertical")).then(|| "true".to_string()),
            ),
            ("ratio", self.field(element, "ratio")),
            ("minFirst", self.len(element, "min_first")),
            ("minSecond", self.len(element, "min_second")),
        ]);
        out.open(format!("B::SplitPane(p, c, {props}, {BUILDER}"));
        self.node(out, &element.children[0]);
        out.close(format!("}}, {BUILDER}"));
        out.indent += 1;
        self.node(out, &element.children[1]);
        out.close("});");
    }

    pub(super) fn reorderable(&self, out: &mut Out, element: &Element) {
        let (Some(items), Some(template), Some(Prop::Key { field: key_idx, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            return;
        };
        let props = designated(&[
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("items", Some(self.getter(items))),
            ("key", Some(key_idx.to_string())),
            ("variable", Some(string(&template.var))),
            ("from", self.field(element, "move_from")),
            ("to", self.field(element, "move_to")),
            ("action", self.action(element)),
        ]);
        self.call(
            out,
            &format!("B::Reorderable(p, c, {props}, "),
            &element.children,
        );
    }

    pub(super) fn bottom_sheet(&self, out: &mut Out, element: &Element) {
        let props = designated(&[
            ("detents", Self::keyword(element, "detents")),
            (
                "dragIndicator",
                (element.keyword("drag_indicator") == Some("false")).then(|| "false".to_string()),
            ),
            ("dismiss", self.action(element)),
        ]);
        self.call(
            out,
            &format!("B::BottomSheet(p, c, {props}, "),
            &element.children,
        );
    }

    pub(super) fn refreshable(&self, out: &mut Out, element: &Element) {
        let props = designated(&[
            ("id", Some(self.id(element))),
            (
                "refreshing",
                element.expr("refreshing").map(|e| self.getter(e)),
            ),
            ("action", self.action(element)),
        ]);
        self.call(
            out,
            &format!("B::Refreshable(p, c, {props}, "),
            &element.children,
        );
    }

    pub(super) fn popover_button(&self, out: &mut Out, element: &Element) {
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", Self::icon(element, "icon")),
            ("presented", self.field(element, "presented")),
        ];
        fields.push((
            "windowTheme",
            Some(
                (bern_target_api::style::popover_window_theme(self.app, "windows", element))
                    .to_string(),
            ),
        ));
        fields.push((
            "style",
            self.style("PopoverButton", element.keyword("variant")),
        ));
        if element.expr("open").is_some() {
            fields.extend(self.model(element, "open"));
        }
        fields.push((
            "plain",
            (element.keyword("prominence") == Some("plain")).then(|| "true".to_string()),
        ));
        fields.push(("iconSize", Self::number(element, "font_size")));
        fields.push(("shape", self.shape(element)));
        fields.push(("placement", Self::keyword(element, "placement")));
        self.call(
            out,
            &format!("B::PopoverButton(p, c, {}, ", designated(&fields)),
            &element.children,
        );
        self.shortcut(out, element, "activate");
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element) {
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", Self::icon(element, "icon")),
        ];
        fields.extend(self.model(element, "checked"));
        fields.extend([
            ("more", self.text_prop(element, "more_label")),
            ("shape", self.shape(element)),
        ]);
        self.call(
            out,
            &format!("B::SplitButton(p, c, {}, ", designated(&fields)),
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
                (
                    "hideDisabled",
                    (element.keyword("hide_disabled") == Some("true")).then(|| "true".into()),
                ),
                ("placement", Self::keyword(element, "placement")),
                ("bold", (element.keyword("bold") == Some("true")).then(|| "true".into())),
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
            if item.prop("shortcut").is_some() {
                shortcuts.push(item);
            }
            out.line(format!("B::MenuItemDef{},", self.menu_item_def(item)));
        }
        out.close("});");
        for item in shortcuts {
            self.shortcut(out, item, "activate");
        }
    }

    /// The pointer events an element declares, for `B::PointerEvents`; a
    /// `Button` keeps its double tap and menu among its own props.
    pub(super) fn pointer_events(&self, element: &Element) -> Option<String> {
        if !bern_ir::spec::POINTER_OWNERS.contains(&element.kind.as_str())
            || !element.has_pointer_events()
        {
            return None;
        }
        let button = element.kind == "Button";
        let events = [
            (
                "doubleTap",
                if button {
                    None
                } else {
                    self.child_action(element, "DoubleTap")
                },
            ),
            (
                "menu",
                if button {
                    None
                } else {
                    self.context_menu_def(element)
                },
            ),
            ("wheelUp", self.child_action(element, "WheelUp")),
            ("wheelDown", self.child_action(element, "WheelDown")),
        ];
        if events.iter().all(|(_, value)| value.is_none()) {
            return None;
        }
        let mut fields = events.to_vec();
        // The native slider is the element's own: a double tap on its thumb
        // is the slider's, and the wheel reaches it even where it takes it.
        fields.push((
            "own",
            (element.kind == "Slider").then(|| "true".to_string()),
        ));
        Some(designated(&fields))
    }

    /// The items of an element's `ContextMenu`, a missing one for each
    /// divider.
    fn context_menu_def(&self, element: &Element) -> Option<String> {
        let items = element.context_menu()?;
        let entries: Vec<String> = items
            .iter()
            .map(|item| {
                if item.kind == "Divider" {
                    "std::nullopt".to_string()
                } else {
                    format!("B::MenuItemDef{}", self.menu_item_def(item))
                }
            })
            .collect();
        Some(format!(
            "std::vector<std::optional<B::MenuItemDef>>{{{}}}",
            entries.join(", ")
        ))
    }

    /// The fields of a `MenuItem`.
    pub(super) fn menu_item_def(&self, item: &Element) -> String {
        designated(&[
            ("id", Some(self.id(item))),
            ("label", self.text_prop(item, "label")),
            ("icon", Self::icon(item, "icon")),
            ("role", Self::keyword(item, "role")),
            ("enabled", self.enabled(item)),
            (
                "checked",
                item.expr("checked").map(|checked| self.getter(checked)),
            ),
            ("action", self.action(item)),
            ("pick", self.field(item, "pick")),
            ("save", self.field(item, "save")),
            ("suggestedName", self.text_prop(item, "suggested_name")),
            ("files", Self::files(item)),
            ("paste", self.field(item, "paste")),
            ("shortcut", Self::keyword(item, "shortcut")),
            ("boundShortcut", self.shortcut_keys(item, "shortcut")),
        ])
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
                    (
                        "continuous",
                        (element.keyword("continuous") == Some("true")).then(|| "true".into())
                    ),
                    ("active", self.field(element, "active")),
                    ("onCommit", self.child_action(element, "OnCommit")),
                ])
            ),
            &element.children,
        );
    }
}

fn infer_precision(precision: Option<f64>, step: Option<f64>, is_int: bool) -> u32 {
    if let Some(p) = precision {
        return p.round().clamp(0.0, 6.0) as u32;
    }
    if is_int {
        return 0;
    }
    if let Some(s) = step {
        let formatted = format!("{s:.4}");
        let trimmed = formatted.trim_end_matches('0');
        if let Some((_, frac)) = trimmed.split_once('.') {
            return (frac.len() as u32).min(4);
        }
    }
    0
}
