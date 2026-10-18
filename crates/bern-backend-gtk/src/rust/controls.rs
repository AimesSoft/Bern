//! Controls: buttons, inputs, pickers, drop areas and gestures.

use super::{Generator, Out, float, some, string};
use bern_ir::Element;
use bern_protocol::Type;

impl Generator<'_> {
    /// A `Button` with children, which it shows as its content.
    pub(super) fn content_button(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::ContentButton(p, c, b::ContentButtonProps {{ id: {}, label: {}, enabled: {}, style: {}, pick: {} }}, {}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                self.enabled(element),
                self.style("Button", element),
                self.pick(element),
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
    }

    pub(super) fn button(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Button(p, c, b::ButtonProps {{ id: {}, label: {}, icon: {}, role: {}, prominence: {}, enabled: {}, style: {}, pick: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
            Self::keyword(element, "role"),
            Self::keyword(element, "prominence"),
            self.enabled(element),
            self.style("Button", element),
            self.pick(element),
            self.action(element),
        ));
        self.shortcut(
            out,
            element,
            &format!("b::activate_target(c, {})", self.id(element)),
        );
    }

    pub(super) fn icon_button(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::IconButton(p, c, b::IconButtonProps {{ id: {}, icon: {}, label: {}, enabled: {}, style: {}, pick: {} }}, {});",
            self.id(element),
            string(element.icon("icon").unwrap_or("circle")),
            some(self.text_prop(element, "label")),
            self.enabled(element),
            self.style("IconButton", element),
            self.pick(element),
            self.action(element),
        ));
        self.shortcut(
            out,
            element,
            &format!("b::activate_target(c, {})", self.id(element)),
        );
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
        let field = self.field(element, "value");
        out.line(format!(
            "b::Switch(p, c, b::SwitchProps {{ id: {}, label: {}, field: {}, style: {}, icon: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(field.clone()),
            Self::keyword(element, "style"),
            Self::icon(element, "icon"),
        ));
        if let Some(field) = field {
            self.shortcut(out, element, &format!("b::session_toggle(c, {field})"));
        }
    }

    pub(super) fn slider(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Slider(p, c, b::SliderProps {{ id: {}, label: {}, field: {}, min: {}, max: {}, step: {}, min_icon: {}, max_icon: {} }}, {});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.field(element, "value")),
            Self::number(element, "min"),
            Self::number(element, "max"),
            Self::number(element, "step"),
            Self::icon(element, "min_icon"),
            Self::icon(element, "max_icon"),
            self.action(element),
        ));
    }

    pub(super) fn picker(&self, out: &mut Out, element: &Element) {
        let Some(field) = element.field("value") else {
            return;
        };
        let Type::Enum { name } = &field.ty else {
            return;
        };
        let def = self.types().enum_def(name).expect("enum");
        let options: Vec<_> = def
            .variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                format!(
                    "b::option({index}, {}, {})",
                    string(&variant.name),
                    self.variant_label(def, index)
                )
            })
            .collect();
        out.line(format!(
            "b::Picker(p, c, b::PickerProps {{ id: {}, label: {}, field: {}, options: vec![{}], style: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.field(element, "value")),
            options.join(", "),
            Self::keyword(element, "style"),
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
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element) {
        let field = self
            .field(element, "value")
            .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into());
        self.children(
            out,
            &format!(
                "b::SplitButton(p, c, b::SplitButtonProps {{ id: {}, label: {}, icon: {}, field: {field}, more: {} }}, ",
                self.id(element),
                some(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                some(self.text_prop(element, "more_label")),
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
