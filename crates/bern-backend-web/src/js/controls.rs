//! Controls: buttons, inputs, pickers, drop areas, gestures and timers.

use super::{Generator, Out, number, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// A `Button` or an `IconButton`; a `Button` with children shows them
    /// as its content.
    pub(super) fn button(&self, out: &mut Out, element: &Element) {
        let enabled = element
            .expr("enabled")
            .map(|expr| format!("() => {}", self.expr(expr)));
        let (action, set_file, kinds) = self.file_actions(element);
        let style = crate::style::reference(self.app, &element.kind, element.keyword("variant"));
        if element.kind == "Button" && !element.children.is_empty() {
            let props = Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("enabled", enabled),
                ("style", style),
                ("shape", Self::keyword(element, "shape")),
                ("height", self.length(element, "height")),
                ("setFile", set_file),
                ("fileKinds", kinds),
            ]);
            self.children(
                out,
                &format!("F.ContentButton(p, ctx, {props}, {action}, "),
                &element.children,
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
                    ("fontSize", element.number("font_size").map(number)),
                    ("enabled", enabled),
                    ("style", style),
                    ("shape", Self::keyword(element, "shape")),
                    ("height", self.length(element, "height")),
                    ("setFile", set_file),
                    ("fileKinds", kinds),
                ]),
            ));
        }
        self.shortcut(out, element, &action);
    }

    pub(super) fn text_input(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.TextInput(p, ctx, {}, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "labelHidden",
                    Some((element.keyword("label_hidden") == Some("true")).to_string())
                ),
                ("field", self.field(element, "value")),
                ("prompt", self.text_prop(element, "prompt")),
                ("kind", Self::keyword(element, "kind")),
            ]),
            self.action_fn(element)
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element) {
        let field = self.control_field(element, "checked");
        out.line(format!(
            "F.Switch(p, ctx, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("field", field.clone()),
                ("style", Self::keyword(element, "style")),
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
                ("min", element.number("min").map(number)),
                ("max", element.number("max").map(number)),
                ("step", element.number("step").map(number)),
                ("minIcon", element.icon("min_icon").map(string)),
                ("maxIcon", element.icon("max_icon").map(string)),
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

    pub(super) fn picker(&self, out: &mut Out, element: &Element) {
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
            ])
        ));
    }

    pub(super) fn popover_button(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", element.icon("icon").map(string)),
            ("presented", self.field(element, "presented")),
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
        let field = self.field(element, "value");
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
            self.shortcut(out, element, &format!("() => {field}.set(!{field}.peek())"));
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

    /// A `Pinch` or a `Swipe`.
    pub(super) fn gesture(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("field", self.field(element, "value")),
            ("min", element.number("min").map(number)),
            ("max", element.number("max").map(number)),
            ("axis", Self::keyword(element, "axis")),
            ("distance", element.number("distance").map(number)),
        ]);
        self.children(
            out,
            &format!(
                "F.{}(p, ctx, {props}, {}, ",
                element.kind,
                self.action_fn(element)
            ),
            &element.children,
            ");",
        );
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
