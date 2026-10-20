//! Controls: buttons, inputs, pickers, drop areas and gestures.

use super::{Generator, Out, float, some, string};
use bern_ir::{Element, Node};
use bern_protocol::Type;

impl Generator<'_> {
    /// A `Button` with children, which it shows as its content.
    pub(super) fn content_button(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::ContentButton(p, c, b::ContentButtonProps {{ id: {}, label: {}, enabled: {}, style: {}, pick: {}, shape: {} }}, {}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                self.enabled(element),
                self.style("Button", element),
                self.pick(element),
                self.shape(element),
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
            "b::Button(p, c, b::ButtonProps {{ id: {}, label: {}, icon: {}, role: {}, prominence: {}, enabled: {}, style: {}, pick: {}, shape: {}, fill: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
            Self::keyword(element, "role"),
            Self::keyword(element, "prominence"),
            self.enabled(element),
            self.style("Button", element),
            self.pick(element),
            self.shape(element),
            element.keyword("width") == Some("fill"),
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
            "b::IconButton(p, c, b::IconButtonProps {{ id: {}, icon: {}, label: {}, enabled: {}, style: {}, pick: {}, font_size: {}, diameter: {}, prominence: {} }}, {});",
            self.id(element),
            string(element.icon("icon").unwrap_or("circle")),
            some(self.text_prop(element, "label")),
            self.enabled(element),
            self.style("IconButton", element),
            self.pick(element),
            Self::number(element, "font_size"),
            some(self.num(element, "height")),
            Self::keyword(element, "prominence"),
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
        out.line(format!(
            "b::TextInput(p, c, b::TextInputProps {{ id: {}, label: {}, field: {}, prompt: {}, kind: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.field(element, "value")),
            some(self.text_prop(element, "prompt")),
            Self::keyword(element, "kind"),
            self.action(element),
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element) {
        let model = self.model(element, "checked", true);
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
            "b::Slider(p, c, b::SliderProps {{ id: {}, label: {}, model: {}, min: {}, max: {}, step: {}, min_icon: {}, max_icon: {}, label_hidden: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.model(element, "current", false)),
            Self::number(element, "min"),
            Self::number(element, "max"),
            Self::number(element, "step"),
            Self::icon(element, "min_icon"),
            Self::icon(element, "max_icon"),
            element.keyword("label_hidden") == Some("true"),
            // A controlled slider's action is its change.
            if element.expr("current").is_some() {
                "None".into()
            } else {
                self.action(element)
            },
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
            "b::LanguagePicker(p, c, b::LanguagePickerProps {{ id: {}, label: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
        ));
    }

    pub(super) fn popover_button(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::PopoverButton(p, c, b::PopoverButtonProps {{ id: {}, label: {}, icon: {}, presented: {} }}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                some(self.field(element, "presented")),
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
        let field = self
            .field(element, "value")
            .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into());
        self.children(
            out,
            &format!(
                "b::SplitButton(p, c, b::SplitButtonProps {{ id: {}, label: {}, icon: {}, field: {field}, more: {}, shape: {} }}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                some(self.text_prop(element, "more_label")),
                self.shape(element),
            ),
            &element.children,
            ");",
        );
        self.shortcut(out, element, &format!("b::session_toggle(c, {field})"));
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
                "b::{}(p, c, b::GestureProps {{ id: {}, label: {}, field: {}, min: {}, max: {}, vertical: {}, distance: {} }}, {}, ",
                element.kind,
                self.id(element),
                some(self.text_prop(element, "label")),
                self.field(element, "value")
                    .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into()),
                float(element.number("min").unwrap_or(0.0)),
                float(element.number("max").unwrap_or(1.0)),
                element.keyword("axis") != Some("horizontal"),
                Self::number(element, "distance"),
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
