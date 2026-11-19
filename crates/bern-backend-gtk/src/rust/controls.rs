//! Controls: buttons, inputs, pickers, drop areas and gestures.

use super::{Generator, Out, float, some, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// Non-rendering action children belong to the immediate control only.
    pub(super) fn child_action(&self, element: &Element, kind: &str) -> String {
        element
            .children
            .iter()
            .find_map(|node| match node {
                Node::Element(child) if child.kind == kind => Some(self.action(child)),
                _ => None,
            })
            .unwrap_or_else(|| "None".into())
    }

    /// A `Button` with children, which it shows as its content.
    pub(super) fn content_button(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::ContentButton(p, c, b::ContentButtonProps {{ id: {}, label: {}, enabled: {}, style: {}, pick: {}, paste: {}, shape: {}, menu: {}, double_tap: {} }}, {}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                self.enabled(element),
                self.style("Button", element),
                self.pick(element),
                some(self.field(element, "paste")),
                self.shape(element),
                self.context_menu(element),
                self.child_action(element, "DoubleTap"),
                self.action(element),
            ),
            &element.children,
            ");",
        );
        self.shortcut(
            out,
            element,
            &format!("b::activate_target(c, {})", self.id(element)),
        );
        self.default_focus(out, element);
    }

    pub(super) fn button(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Button(p, c, b::ButtonProps {{ id: {}, label: {}, icon: {}, role: {}, prominence: {}, enabled: {}, style: {}, pick: {}, paste: {}, shape: {}, fill: {}, menu: {}, double_tap: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
            Self::keyword(element, "role"),
            Self::keyword(element, "prominence"),
            self.enabled(element),
            self.style("Button", element),
            self.pick(element),
            some(self.field(element, "paste")),
            self.shape(element),
            element.keyword("width") == Some("fill"),
            self.context_menu(element),
            self.child_action(element, "DoubleTap"),
            self.action(element),
        ));
        self.shortcut(
            out,
            element,
            &format!("b::activate_target(c, {})", self.id(element)),
        );
        self.default_focus(out, element);
    }

    pub(super) fn icon_button(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::IconButton(p, c, b::IconButtonProps {{ id: {}, icon: {}, label: {}, enabled: {}, style: {}, pick: {}, paste: {}, font_size: {}, diameter: {}, prominence: {}, icon_progress: {}, badge_icon: {}, badge_visible: {} }}, {});",
            self.id(element),
            string(element.icon("icon").unwrap_or("circle")),
            some(self.text_prop(element, "label")),
            self.enabled(element),
            self.style("IconButton", element),
            self.pick(element),
            some(self.field(element, "paste")),
            Self::number(element, "font_size"),
            some(self.num(element, "height")),
            Self::keyword(element, "prominence"),
            some(self.num(element, "icon_progress")),
            some(element.icon("badge_icon").map(string)),
            some(element.expr("badge_visible").map(|value| self.getter(value))),
            self.action(element),
        ));
        self.shortcut(
            out,
            element,
            &format!("b::activate_target(c, {})", self.id(element)),
        );
        self.default_focus(out, element);
    }

    pub(super) fn text_input(&self, out: &mut Out, element: &Element) {
        // A field showing engine state runs its action on each edit; one
        // editing a draft, when the text is submitted.
        let controlled = element.expr("current").is_some();
        out.line(format!(
            "b::TextInput(p, c, b::TextInputProps {{ id: {}, label: {}, label_hidden: {}, model: {}, prompt: {}, kind: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            element.keyword("label_hidden") == Some("true"),
            some(self.model(element, "current", false)),
            some(self.text_prop(element, "prompt")),
            Self::keyword(element, "kind"),
            if controlled {
                "None".into()
            } else {
                self.action(element)
            },
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element) {
        let model = if element.keyword("login_item") == Some("true") {
            Some("b::login_model()".to_string())
        } else {
            self.model(element, "checked", true)
        };
        out.line(format!(
            "b::Switch(p, c, b::SwitchProps {{ id: {}, label: {}, model: {}, style: {}, icon: {}, label_hidden: {}, shape: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(model.clone()),
            Self::keyword(element, "style"),
            Self::icon(element, "icon"),
            element.keyword("label_hidden") == Some("true"),
            self.shape(element),
        ));
        if let Some(model) = model {
            self.shortcut(out, element, &format!("b::toggle(c, &{model})"));
        }
    }

    pub(super) fn slider(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Slider(p, c, b::SliderProps {{ id: {}, label: {}, model: {}, min: {}, max: {}, step: {}, min_icon: {}, max_icon: {}, label_hidden: {}, continuous: {}, hover: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.model(element, "current", false)),
            Self::number(element, "min"),
            Self::number(element, "max"),
            Self::number(element, "step"),
            Self::icon(element, "min_icon"),
            Self::icon(element, "max_icon"),
            element.keyword("label_hidden") == Some("true"),
            element.keyword("continuous") == Some("true"),
            some(self.field(element, "hover")),
            // A controlled slider's action is its change.
            if element.expr("current").is_some() {
                "None".into()
            } else {
                self.action(element)
            },
        ));
    }

    pub(super) fn color_picker(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::ColorPicker(p, c, b::ColorPickerProps {{ id: {}, label: {}, model: {}, label_hidden: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.model(element, "current", false)),
            element.keyword("label_hidden") == Some("true"),
            if element.expr("current").is_some() {
                "None".into()
            } else {
                self.action(element)
            },
        ));
    }

    pub(super) fn tooltip(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Tooltip(p, c, {}, ",
                some(self.text_prop(element, "content")),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn banner(&self, out: &mut Out, element: &Element) {
        let props = format!(
            "b::BannerProps {{ id: {}, severity: {}, title: {}, content: {}, icon: {}, dismiss_label: {} }}",
            some(element.id.as_deref().map(string)),
            Self::keyword(element, "severity"),
            some(self.text_prop(element, "title")),
            some(self.text_prop(element, "content")),
            Self::icon(element, "icon"),
            some(self.text_prop(element, "dismiss_label")),
        );
        if element.children.is_empty() {
            out.line(format!(
                "b::Banner(p, c, {props}, {}, None);",
                self.action(element)
            ));
        } else {
            self.children(
                out,
                &format!("b::Banner(p, c, {props}, {}, Some(", self.action(element)),
                &element.children,
                "));",
            );
        }
    }

    pub(super) fn disclosure(&self, out: &mut Out, element: &Element) {
        let model = if element.expr("open").is_some() {
            self.model(element, "open", true)
        } else {
            self.field(element, "expanded")
                .map(|field| format!("b::Model::Field({field}, {})", self.action(element)))
        };
        let then = if model.is_some() {
            "None".into()
        } else {
            self.action(element)
        };
        self.children(
            out,
            &format!(
                "b::Disclosure(p, c, b::DisclosureProps {{ id: {}, label: {}, icon: {}, model: {}, then: {then} }}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                some(model),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn number_input(&self, out: &mut Out, element: &Element) {
        let is_int = matches!(element.expr("current").map(|e| &e.ty), Some(Type::Int));
        let precision =
            infer_precision(element.number("precision"), element.number("step"), is_int);
        out.line(format!(
            "b::NumberInput(p, c, b::NumberInputProps {{ id: {}, label: {}, model: {}, min: {}, max: {}, step: {}, precision: {precision}, enabled: {}, label_hidden: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.model(element, "current", true)),
            Self::number(element, "min"),
            Self::number(element, "max"),
            Self::number(element, "step"),
            some(element.expr("enabled").map(|e| self.getter(e))),
            element.keyword("label_hidden") == Some("true"),
        ));
    }

    pub(super) fn alert(&self, out: &mut Out, element: &Element) {
        let mut actions = Vec::new();
        for node in &element.children {
            let Node::Element(act) = node else { continue };
            if act.kind != "AlertAction" {
                continue;
            }
            actions.push(format!(
                "b::AlertActionDef {{ id: {}, label: {}, role: {}, enabled: {}, action: {} }}",
                self.id(act),
                some(self.text_prop(act, "label")),
                Self::keyword(act, "role"),
                some(act.expr("enabled").map(|e| self.getter(e))),
                self.action(act),
            ));
        }
        out.line(format!(
            "b::Alert(p, c, b::AlertProps {{ title: {}, message: {}, actions: vec![{}], dismiss: {} }});",
            some(self.text_prop(element, "title")),
            some(self.text_prop(element, "message")),
            actions.join(", "),
            self.action(element),
        ));
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
            let label_enum = match &def.fields[*label_idx].ty {
                Type::Enum { name } => format!("Some({})", string(name)),
                _ => "None".into(),
            };
            out.line(format!(
                "b::DynamicPicker(p, c, b::DynamicPickerProps {{ id: {}, label: {}, model: {}, items: {}, key_field: {key_idx}, label_field: {label_idx}, label_enum: {label_enum}, prompt: {}, style: {}, label_hidden: {}, shape: {} }});",
                self.id(element),
                some(self.text_prop(element, "label")),
                some(self.model(element, "selected", true)),
                self.getter(items_expr),
                some(self.text_prop(element, "prompt")),
                Self::keyword(element, "style"),
                element.keyword("label_hidden") == Some("true"),
                self.shape(element),
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
                Some(format!(
                    "b::option({}, {}, {})",
                    self.expr(value),
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
                        "b::option(V::unit_variant({index}), {}, {})",
                        string(&variant.name),
                        self.variant_label(def, index)
                    )
                })
                .collect()
        } else {
            explicit
        };
        out.line(format!(
            "b::Picker(p, c, b::PickerProps {{ id: {}, label: {}, model: {}, options: vec![{}], style: {}, label_hidden: {}, shape: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.model(element, "selected", true)),
            options.join(", "),
            Self::keyword(element, "style"),
            element.keyword("label_hidden") == Some("true"),
            self.shape(element),
        ));
    }

    pub(super) fn language_picker(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::LanguagePicker(p, c, b::LanguagePickerProps {{ id: {}, label: {}, label_hidden: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            element.keyword("label_hidden") == Some("true"),
        ));
    }

    pub(super) fn popover_button(&self, out: &mut Out, element: &Element) {
        let presented = if element.expr("open").is_some() {
            self.model(element, "open", true)
        } else {
            self.field(element, "presented")
                .map(|field| format!("b::Model::Field({field}, None)"))
        };
        self.children(
            out,
            &format!(
                "b::PopoverButton(p, c, b::PopoverButtonProps {{ id: {}, label: {}, icon: {}, presented: {}, window_theme: {}, style: {}, plain: {}, font_size: {} }}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                some(presented),
                bern_target_api::style::popover_window_theme(self.app, "linux", element),
                self.style("PopoverButton", element),
                element.keyword("prominence") == Some("plain"),
                Self::number(element, "font_size"),
            ),
            &element.children,
            ");",
        );
        self.shortcut(
            out,
            element,
            &format!("b::activate_target(c, {})", self.id(element)),
        );
        self.default_focus(out, element);
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element) {
        let model = self.model(element, "checked", true);
        self.children(
            out,
            &format!(
                "b::SplitButton(p, c, b::SplitButtonProps {{ id: {}, label: {}, icon: {}, model: {}, more: {}, shape: {} }}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                some(model.clone()),
                some(self.text_prop(element, "more_label")),
                self.shape(element),
            ),
            &element.children,
            ");",
        );
        if let Some(model) = model {
            self.shortcut(out, element, &format!("b::toggle(c, &{model})"));
        }
    }

    /// A `Drop`: an area that takes dropped files.
    pub(super) fn drop_zone(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Drop(p, c, b::DropProps {{ id: {}, label: {}, field: {}, kinds: {}, over: {}, opens: {} }}, {}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                self.field(element, "value")
                    .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into()),
                string(element.keyword("files").unwrap_or("any")),
                some(self.field(element, "over")),
                element.keyword("opens") == Some("true"),
                self.action(element),
            ),
            &element.children,
            ");",
        );
    }

    /// A `Pinch` or a `Swipe`.
    pub(super) fn gesture(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::{}(p, c, b::GestureProps {{ id: {}, label: {}, field: {}, min: {}, max: {}, vertical: {}, distance: {}, continuous: {}, active: {}, on_commit: {} }}, {}, ",
                element.kind,
                self.id(element),
                some(self.text_prop(element, "label")),
                self.field(element, "value")
                    .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into()),
                float(element.number("min").unwrap_or(0.0)),
                float(element.number("max").unwrap_or(1.0)),
                element.keyword("axis") != Some("horizontal"),
                Self::number(element, "distance"),
                element.keyword("continuous") == Some("true"),
                some(self.field(element, "active")),
                self.child_action(element, "OnCommit"),
                self.action(element),
            ),
            &element.children,
            ");",
        );
    }
}

impl Generator<'_> {
    /// `default_focus`: the keyboard focus starts on the control once its
    /// window shows it.
    fn default_focus(&self, out: &mut Out, element: &Element) {
        if element.keyword("default_focus") == Some("true") {
            out.line(format!("b::default_focus(c, {});", self.id(element)));
        }
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
