//! Controls: buttons, inputs, pickers, drop areas, gestures and timers.

use super::{Generator, Out, local, number, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// A `Button` or an `IconButton`; a `Button` with children shows them
    /// as its content.
    pub(super) fn button(&self, out: &mut Out, element: &Element) {
        let double_tap = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "DoubleTap" => Some(self.action_fn(child)),
            _ => None,
        });
        let content: Vec<_> = element.content().cloned().collect();
        let enabled = element
            .expr("enabled")
            .map(|expr| format!("() => {}", self.expr(expr)));
        let (action, set_file, kinds) = self.file_actions(element);
        let style = crate::style::reference(self.app, &element.kind, element.keyword("variant"));
        let default_focus =
            (element.keyword("default_focus") == Some("true")).then(|| "true".to_string());
        if element.kind == "Button" && !content.is_empty() {
            let props = Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("cursor", Self::keyword(element, "cursor")),
                ("enabled", enabled),
                ("style", style),
                ("shape", Self::keyword(element, "shape")),
                ("height", self.length(element, "height")),
                ("setFile", set_file),
                ("fileKinds", kinds),
                ("defaultFocus", default_focus.clone()),
                ("menu", self.context_menu(element)),
                ("doubleTap", double_tap.clone()),
            ]);
            self.children(
                out,
                &format!("F.ContentButton(p, ctx, {props}, {action}, "),
                &content,
                ");",
            );
        } else {
            out.line(format!(
                "B.{}(p, ctx, {}, {action});",
                element.kind,
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("icon", element.icon("icon").map(string)),
                    ("role", Self::keyword(element, "role")),
                    ("prominence", Self::keyword(element, "prominence")),
                    (
                        "cursor",
                        (element.kind == "Button")
                            .then(|| Self::keyword(element, "cursor"))
                            .flatten(),
                    ),
                    (
                        "color",
                        (element.kind == "IconButton")
                            .then(|| Self::keyword(element, "color"))
                            .flatten(),
                    ),
                    ("fontSize", element.number("font_size").map(number)),
                    ("iconProgress", self.value_prop(element, "icon_progress")),
                    ("badgeIcon", element.icon("badge_icon").map(string)),
                    ("badgeVisible", self.value_prop(element, "badge_visible")),
                    (
                        "diameter",
                        (element.kind == "IconButton")
                            .then(|| self.length(element, "height"))
                            .flatten()
                    ),
                    ("enabled", enabled),
                    ("style", style),
                    ("shape", Self::keyword(element, "shape")),
                    ("height", self.length(element, "height")),
                    (
                        "fill",
                        (element.keyword("width") == Some("fill")).then(|| "true".into())
                    ),
                    ("setFile", set_file),
                    ("fileKinds", kinds),
                    ("defaultFocus", default_focus),
                    ("menu", self.context_menu(element)),
                    ("doubleTap", double_tap.clone()),
                ]),
            ));
        }
        self.shortcut(out, element, &action);
    }

    /// The pointer events of an element of `POINTER_OWNERS`, for
    /// `F.PointerEvents`. A `Button` realizes its double tap and menu
    /// itself, exclusive with its action.
    pub(super) fn pointer_events(&self, element: &Element) -> Option<String> {
        if !bern_ir::spec::POINTER_OWNERS.contains(&element.kind.as_str()) {
            return None;
        }
        let event = |kind: &str| {
            element
                .pointer_event(kind)
                .map(|event| self.action_fn(event))
        };
        let button = element.kind == "Button";
        let double_tap = (!button).then(|| event("DoubleTap")).flatten();
        let menu = (!button).then(|| self.context_menu(element)).flatten();
        let (up, down) = (event("WheelUp"), event("WheelDown"));
        let hover_el = element.pointer_event("Hover");
        let hover = hover_el.map(|event| self.action_fn(event));
        let hover_delay = hover_el.and_then(|event| event.number("delay")).map(number);
        if double_tap.is_none()
            && menu.is_none()
            && up.is_none()
            && down.is_none()
            && hover.is_none()
        {
            return None;
        }
        Some(Self::props(vec![
            ("doubleTap", double_tap),
            (
                "label",
                menu.as_ref().and_then(|_| self.text_prop(element, "label")),
            ),
            ("menu", menu),
            ("wheelUp", up),
            ("wheelDown", down),
            ("hover", hover),
            ("hoverDelay", hover_delay),
            // Presses on a slider are its own, not those of a control in it.
            (
                "control",
                (element.kind == "Slider").then(|| "true".to_string()),
            ),
        ]))
    }

    pub(super) fn text_input(&self, out: &mut Out, element: &Element) {
        // A field showing engine state commits each edit through its
        // action; one editing a draft runs the action on submit.
        let controlled = element.expr("current").is_some();
        out.line(format!(
            "B.TextInput(p, ctx, {}, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "labelHidden",
                    Some((element.keyword("label_hidden") == Some("true")).to_string())
                ),
                (
                    "field",
                    if controlled {
                        self.control_field(element, "current")
                    } else {
                        self.field(element, "value")
                    }
                ),
                ("controlled", controlled.then(|| "true".into())),
                ("prompt", self.text_prop(element, "prompt")),
                ("kind", Self::keyword(element, "kind")),
            ]),
            if controlled {
                "null".into()
            } else {
                self.action_fn(element)
            }
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element) {
        // A page does not open at login.
        if element.keyword("login_item") == Some("true") {
            return;
        }
        let field = self.control_field(element, "checked");
        out.line(format!(
            "F.Switch(p, ctx, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("field", field.clone()),
                ("style", Self::keyword(element, "style")),
                (
                    "look",
                    crate::style::reference(self.app, "Switch", element.keyword("variant")),
                ),
                ("shape", Self::keyword(element, "shape")),
                ("height", self.length(element, "height")),
                ("icon", element.icon("icon").map(string)),
                (
                    "labelHidden",
                    Some((element.keyword("label_hidden") == Some("true")).to_string())
                ),
            ])
        ));
        if let Some(field) = field {
            self.shortcut(
                out,
                element,
                &format!("() => {{ const field = {field}; field.set(!field.peek()); }}"),
            );
        }
    }

    pub(super) fn slider(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.Slider(p, ctx, {}, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "field",
                    if element.expr("current").is_some() {
                        self.control_field(element, "current")
                    } else {
                        self.field(element, "value")
                    }
                ),
                (
                    "controlled",
                    Some(element.expr("current").is_some().to_string())
                ),
                (
                    "continuous",
                    Some((element.keyword("continuous") == Some("true")).to_string())
                ),
                ("hover", self.field(element, "hover")),
                ("min", element.number("min").map(number)),
                ("max", element.number("max").map(number)),
                ("step", element.number("step").map(number)),
                ("minIcon", element.icon("min_icon").map(string)),
                ("maxIcon", element.icon("max_icon").map(string)),
                ("axis", Self::keyword(element, "axis")),
                (
                    "labelHidden",
                    Some((element.keyword("label_hidden") == Some("true")).to_string())
                ),
            ]),
            if element.expr("current").is_some() {
                "null".into()
            } else {
                self.action_fn(element)
            }
        ));
    }

    pub(super) fn color_picker(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.ColorPicker(p, ctx, {}, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "field",
                    if element.expr("current").is_some() {
                        self.control_field(element, "current")
                    } else {
                        self.field(element, "value")
                    }
                ),
                (
                    "controlled",
                    Some(element.expr("current").is_some().to_string())
                ),
                (
                    "labelHidden",
                    Some((element.keyword("label_hidden") == Some("true")).to_string())
                ),
            ]),
            if element.expr("current").is_some() {
                "null".into()
            } else {
                self.action_fn(element)
            }
        ));
    }

    pub(super) fn tooltip(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "B.Tooltip(p, ctx, {}, ",
                Self::props(vec![
                    ("content", self.text_prop(element, "content")),
                    ("keys", self.shortcut_keys(element, "keys")),
                    ("placement", Self::keyword(element, "placement")),
                ]),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn banner(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("id", element.id.as_deref().map(string)),
            ("severity", Self::keyword(element, "severity")),
            ("title", self.text_prop(element, "title")),
            ("content", self.text_prop(element, "content")),
            ("icon", element.icon("icon").map(string)),
            ("dismissLabel", self.text_prop(element, "dismiss_label")),
        ]);
        if element.children.is_empty() {
            out.line(format!(
                "B.Banner(p, ctx, {props}, {}, null);",
                self.action_fn(element)
            ));
        } else {
            self.children(
                out,
                &format!("B.Banner(p, ctx, {props}, {}, ", self.action_fn(element)),
                &element.children,
                ");",
            );
        }
    }

    pub(super) fn disclosure(&self, out: &mut Out, element: &Element) {
        let field = if element.expr("open").is_some() {
            self.control_field(element, "open")
        } else {
            self.field(element, "expanded")
                .map(|f| match &element.action {
                    Some(action) => format!(
                        "{{ get: () => {f}.get(), peek: () => {f}.peek(), set: (v) => {{ {f}.set(v); {}; }} }}",
                        self.action(action)
                    ),
                    None => f,
                })
        };
        let on_toggle = if field.is_none() {
            element
                .action
                .as_ref()
                .map(|a| format!("() => {{ {}; }}", self.action(a)))
        } else {
            None
        };
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", element.icon("icon").map(string)),
            ("field", field),
            ("onToggle", on_toggle),
        ]);
        self.children(
            out,
            &format!("B.Disclosure(p, ctx, {props}, "),
            &element.children,
            ");",
        );
    }

    pub(super) fn number_input(&self, out: &mut Out, element: &Element) {
        let is_int = matches!(element.expr("current").map(|e| &e.ty), Some(Type::Int));
        let precision =
            infer_precision(element.number("precision"), element.number("step"), is_int);
        let controlled = element.expr("current").is_some();
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            (
                "field",
                if controlled {
                    self.control_field(element, "current")
                } else {
                    self.field(element, "value")
                },
            ),
            ("min", element.number("min").map(number)),
            ("max", element.number("max").map(number)),
            ("step", element.number("step").map(number)),
            ("precision", Some(precision.to_string())),
            (
                "enabled",
                element
                    .expr("enabled")
                    .map(|e| format!("() => {}", self.expr(e))),
            ),
            (
                "labelHidden",
                Some((element.keyword("label_hidden") == Some("true")).to_string()),
            ),
        ]);
        let commit = if controlled {
            "null".into()
        } else {
            self.action_fn(element)
        };
        out.line(format!("B.NumberInput(p, ctx, {props}, {commit});"));
    }

    pub(super) fn alert(&self, out: &mut Out, element: &Element) {
        let mut actions = Vec::new();
        for node in &element.children {
            let Node::Element(act) = node else { continue };
            if act.kind != "AlertAction" {
                continue;
            }
            actions.push(Self::props(vec![
                ("id", Some(self.id(act))),
                ("label", self.text_prop(act, "label")),
                ("role", Self::keyword(act, "role")),
                (
                    "enabled",
                    act.expr("enabled")
                        .map(|e| format!("() => {}", self.expr(e))),
                ),
                ("action", Some(self.action_fn(act))),
            ]));
        }
        let props = Self::props(vec![
            ("title", self.text_prop(element, "title")),
            ("message", self.text_prop(element, "message")),
            ("actions", Some(format!("[{}]", actions.join(", ")))),
            ("onDismiss", Some(self.action_fn(element))),
        ]);
        out.line(format!("B.Alert(p, ctx, {props});"));
    }

    pub(super) fn date_picker(&self, out: &mut Out, element: &Element) {
        let controlled = element.expr("current").is_some();
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            (
                "field",
                if controlled {
                    self.control_field(element, "current")
                } else {
                    self.field(element, "value")
                },
            ),
            (
                "min",
                element
                    .expr("min")
                    .map(|e| format!("() => {}", self.expr(e))),
            ),
            (
                "max",
                element
                    .expr("max")
                    .map(|e| format!("() => {}", self.expr(e))),
            ),
            (
                "enabled",
                element
                    .expr("enabled")
                    .map(|e| format!("() => {}", self.expr(e))),
            ),
            (
                "labelHidden",
                Some((element.keyword("label_hidden") == Some("true")).to_string()),
            ),
        ]);
        let commit = if controlled {
            "null".into()
        } else {
            self.action_fn(element)
        };
        out.line(format!("B.DatePicker(p, ctx, {props}, {commit});"));
    }

    pub(super) fn time_picker(&self, out: &mut Out, element: &Element) {
        let controlled = element.expr("current").is_some();
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            (
                "field",
                if controlled {
                    self.control_field(element, "current")
                } else {
                    self.field(element, "value")
                },
            ),
            ("min", element.number("min").map(number)),
            ("max", element.number("max").map(number)),
            ("step", element.number("step").map(number)),
            (
                "enabled",
                element
                    .expr("enabled")
                    .map(|e| format!("() => {}", self.expr(e))),
            ),
            (
                "labelHidden",
                Some((element.keyword("label_hidden") == Some("true")).to_string()),
            ),
        ]);
        let commit = if controlled {
            "null".into()
        } else {
            self.action_fn(element)
        };
        out.line(format!("B.TimePicker(p, ctx, {props}, {commit});"));
    }

    pub(super) fn empty_state(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("title", self.text_prop(element, "title")),
            ("message", self.text_prop(element, "message")),
            ("icon", element.icon("icon").map(string)),
        ]);
        if element.children.is_empty() {
            out.line(format!("B.EmptyState(p, ctx, {props}, null);"));
        } else {
            self.children(
                out,
                &format!("B.EmptyState(p, ctx, {props}, "),
                &element.children,
                ");",
            );
        }
    }

    pub(super) fn badge(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.Badge(p, ctx, {});",
            Self::props(vec![
                ("content", self.text_prop(element, "content")),
                ("tone", Self::keyword(element, "tone")),
                ("icon", element.icon("icon").map(string)),
            ])
        ));
    }

    pub(super) fn avatar(&self, out: &mut Out, element: &Element) {
        let url = element.expr("source").map(|expr| {
            if matches!(expr.ty, Type::Optional { .. }) {
                format!("() => ({})?.url ?? null", self.expr(expr))
            } else {
                format!("() => ({}).url", self.expr(expr))
            }
        });
        out.line(format!(
            "B.Avatar(p, ctx, {});",
            Self::props(vec![
                ("label", self.text_prop(element, "label")),
                ("source", element.resource("source").map(string)),
                ("url", url),
                ("size", self.length(element, "size")),
                ("shape", Self::keyword(element, "shape")),
            ])
        ));
    }

    pub(super) fn tree_view(&self, out: &mut Out, element: &Element) {
        let (
            Some(items),
            Some(template),
            Some(Prop::Key { name: key_name, .. }),
            Some(Prop::Key {
                name: parent_name, ..
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
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("items", Some(format!("() => {}", self.expr(items)))),
            ("key", Some(format!("(v) => v.{key_name}"))),
            ("parent", Some(format!("(v) => v.{parent_name}"))),
            ("selection", self.field(element, "selection")),
            ("expanded", self.field(element, "expanded")),
        ]);
        out.open(format!(
            "B.TreeView(p, ctx, {props}, (p, ctx, {}) => {{",
            local(&template.var)
        ));
        self.nodes(out, &element.children);
        out.close(format!("}}, {});", self.action_fn(element)));
    }

    pub(super) fn split_pane(&self, out: &mut Out, element: &Element) {
        if element.children.len() != 2 {
            return;
        }
        let props = Self::props(vec![
            ("id", element.id.as_deref().map(string)),
            ("axis", Self::keyword(element, "axis")),
            ("ratio", self.field(element, "ratio")),
            ("minFirst", self.length(element, "min_first")),
            ("minSecond", self.length(element, "min_second")),
        ]);
        out.open(format!("B.SplitPane(p, ctx, {props},"));
        self.children(out, "", &element.children[0..1], ",");
        self.children(out, "", &element.children[1..2], ");");
        out.indent -= 1;
    }

    pub(super) fn reorderable(&self, out: &mut Out, element: &Element) {
        let (Some(items), Some(template), Some(Prop::Key { name: key_name, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            return;
        };
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("items", Some(format!("() => {}", self.expr(items)))),
            ("key", Some(format!("(v) => v.{key_name}"))),
            ("from", self.field(element, "move_from")),
            ("to", self.field(element, "move_to")),
        ]);
        out.open(format!(
            "B.Reorderable(p, ctx, {props}, (p, ctx, {}) => {{",
            local(&template.var)
        ));
        self.nodes(out, &element.children);
        out.close(format!("}}, {});", self.action_fn(element)));
    }

    pub(super) fn bottom_sheet(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("detents", Self::keyword(element, "detents")),
            (
                "dragIndicator",
                element
                    .keyword("drag_indicator")
                    .map(|v| (v == "true").to_string()),
            ),
        ]);
        self.children(
            out,
            &format!("B.BottomSheet(p, ctx, {props}, "),
            &element.children,
            &format!(", {});", self.action_fn(element)),
        );
    }

    pub(super) fn refreshable(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            (
                "refreshing",
                element
                    .expr("refreshing")
                    .map(|e| format!("() => Boolean({})", self.expr(e))),
            ),
        ]);
        self.children(
            out,
            &format!("B.Refreshable(p, ctx, {props}, "),
            &element.children,
            &format!(", {});", self.action_fn(element)),
        );
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
            let key_name = &def.fields[*key_idx].name;
            let label_def = &def.fields[*label_idx];
            let label_name = &label_def.name;
            let label_expr = match &label_def.ty {
                Type::Enum { name } => format!("L.{name}(row.{label_name})"),
                Type::Text => format!("String(row.{label_name} ?? \"\")"),
                Type::Int => format!("B.int(row.{label_name})"),
                Type::Float => format!("B.num(row.{label_name})"),
                _ => format!("String(row.{label_name} ?? \"\")"),
            };
            let items_code = self.expr(items_expr);
            let options = format!(
                "() => ({items_code}).map((row) => ({{ value: row.{key_name}, name: String(row.{key_name}), label: () => {label_expr} }}))"
            );
            out.line(format!(
                "B.Picker(p, ctx, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.control_field(element, "selected")),
                    ("options", Some(options)),
                    ("prompt", self.text_prop(element, "prompt")),
                    ("style", Self::keyword(element, "style")),
                    ("shape", Self::keyword(element, "shape")),
                    ("height", self.length(element, "height")),
                    (
                        "labelHidden",
                        Some((element.keyword("label_hidden") == Some("true")).to_string())
                    ),
                ])
            ));
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
        let options: Vec<_> = if element.children.is_empty() {
            def.variants
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    format!(
                        "{{ value: {index}, name: {}, label: () => L.{name}({index}) }}",
                        string(&variant.name)
                    )
                })
                .collect()
        } else {
            element
                .children
                .iter()
                .filter_map(|node| {
                    let Node::Element(option) = node else {
                        return None;
                    };
                    let value = option.expr("value")?;
                    Some(format!(
                        "{{ value: {}, name: {}, label: {} }}",
                        self.expr(value),
                        self.variant_name(value),
                        self.text_prop(option, "label")?
                    ))
                })
                .collect()
        };
        out.line(format!(
            "B.Picker(p, ctx, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("field", self.control_field(element, "selected")),
                ("options", Some(format!("[{}]", options.join(", ")))),
                ("prompt", self.text_prop(element, "prompt")),
                ("style", Self::keyword(element, "style")),
                ("shape", Self::keyword(element, "shape")),
                ("height", self.length(element, "height")),
                (
                    "labelHidden",
                    Some((element.keyword("label_hidden") == Some("true")).to_string())
                ),
            ])
        ));
    }

    pub(super) fn language_picker(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.LanguagePicker(p, ctx, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "labelHidden",
                    (element.keyword("label_hidden") == Some("true")).then(|| "true".into())
                ),
            ])
        ));
    }

    pub(super) fn popover_button(&self, out: &mut Out, element: &Element) {
        let presented = if element.expr("open").is_some() {
            self.acknowledged_control_field(element, "open")
        } else {
            self.field(element, "presented")
        };
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", element.icon("icon").map(string)),
            ("prominence", Self::keyword(element, "prominence")),
            ("fontSize", element.number("font_size").map(number)),
            ("shape", Self::keyword(element, "shape")),
            ("height", self.length(element, "height")),
            ("placement", Self::keyword(element, "placement")),
            ("presented", presented),
            (
                "windowTheme",
                Some(
                    (bern_target_api::style::popover_window_theme(self.app, "browser", element))
                        .to_string(),
                ),
            ),
            (
                "style",
                crate::style::reference(self.app, "PopoverButton", element.keyword("variant")),
            ),
        ]);
        self.children(
            out,
            &format!("F.PopoverButton(p, ctx, {props}, "),
            &element.children,
            ");",
        );
        self.shortcut(
            out,
            element,
            &format!("() => F.activateTarget(ctx, {})", self.id(element)),
        );
    }

    pub(super) fn control_group(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("label", self.text_prop(element, "label")),
            ("height", self.length(element, "height")),
        ]);
        self.children(
            out,
            &format!("F.ControlGroup(p, ctx, {props}, "),
            &element.children,
            ");",
        );
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element) {
        let field = self.control_field(element, "checked");
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("shape", Self::keyword(element, "shape")),
            ("height", self.length(element, "height")),
            ("label", self.text_prop(element, "label")),
            ("icon", element.icon("icon").map(string)),
            ("field", field.clone()),
            ("more", self.text_prop(element, "more_label")),
        ]);
        self.children(
            out,
            &format!("F.SplitButton(p, ctx, {props}, "),
            &element.children,
            ");",
        );
        if let Some(field) = field {
            self.shortcut(
                out,
                element,
                &format!("() => {{ const field = {field}; field.set(!field.peek()); }}"),
            );
        }
    }

    /// A `Drop`: an area that takes dropped files.
    pub(super) fn drop_zone(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("field", self.field(element, "value")),
            ("kinds", Self::keyword(element, "files")),
            ("over", self.field(element, "over")),
            (
                "opens",
                (element.keyword("opens") == Some("true")
                    || matches!(element.prop("opens"), Some(Prop::Keyword { .. })))
                .then(|| "true".into()),
            ),
        ]);
        self.children(
            out,
            &format!("F.Drop(p, ctx, {props}, {}, ", self.action_fn(element)),
            &element.children,
            ");",
        );
    }

    /// A nonvisual platform brightness binding.
    pub(super) fn screen_brightness(&self, out: &mut Out, element: &Element) {
        let available = self.field(element, "available").expect("bool field");
        out.line(format!(
            "F.ScreenBrightness(p, ctx, {{ available: {available} }});"
        ));
    }

    /// A `Pinch` or a `Swipe`.
    pub(super) fn gesture(&self, out: &mut Out, element: &Element) {
        let on_commit = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "OnCommit" => Some(self.action_fn(child)),
            _ => None,
        });
        let content: Vec<_> = element
            .children
            .iter()
            .filter(|node| !matches!(node, Node::Element(child) if child.kind == "OnCommit"))
            .cloned()
            .collect();
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("field", self.field(element, "value")),
            ("min", element.number("min").map(number)),
            ("max", element.number("max").map(number)),
            ("axis", Self::keyword(element, "axis")),
            ("distance", element.number("distance").map(number)),
            (
                "continuous",
                (element.keyword("continuous") == Some("true")).then(|| "true".into()),
            ),
            ("active", self.field(element, "active")),
            ("onCommit", on_commit),
        ]);
        self.children(
            out,
            &format!(
                "F.{}(p, ctx, {props}, {}, ",
                element.kind,
                self.action_fn(element)
            ),
            &content,
            ");",
        );
    }

    /// `Shortcut`: its key runs the action while it is mounted and enabled.
    pub(super) fn key_shortcut(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "F.KeyShortcut(p, ctx, {}, {});",
            Self::props(vec![
                ("key", self.shortcut_keys(element, "key")),
                ("enabled", self.value_prop(element, "enabled")),
                (
                    "quiet",
                    (element.keyword("quiet") == Some("true")).then(|| "true".into()),
                ),
                ("hold", self.field(element, "hold")),
                ("holdAfter", element.number("hold_after").map(number)),
            ]),
            self.action_fn(element)
        ));
    }

    /// `ShortcutInput`: a button that takes the next key pressed as the
    /// shortcut, which the action keeps.
    pub(super) fn shortcut_input(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "F.ShortcutInput(p, ctx, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("field", self.control_field(element, "current")),
                ("prompt", self.text_prop(element, "prompt")),
                ("empty", self.text_prop(element, "empty")),
                ("enabled", self.value_prop(element, "enabled")),
                (
                    "labelHidden",
                    (element.keyword("label_hidden") == Some("true")).then(|| "true".into()),
                ),
            ])
        ));
    }

    pub(super) fn watch(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.Watch(p, ctx, {}, {});",
            Self::props(vec![("value", self.value_prop(element, "value"))]),
            self.action_fn(element)
        ));
    }

    pub(super) fn timer(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.Timer(p, ctx, {}, {});",
            Self::props(vec![
                ("interval", element.number("interval").map(number)),
                ("enabled", self.value_prop(element, "enabled")),
            ]),
            self.action_fn(element)
        ));
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
