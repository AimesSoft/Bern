//! Controls: buttons, toggles, pickers, sliders, text input, file
//! drops and gestures.

use super::values::kotlin_type;
use super::{Ctx, Generator, Out, Scope};
use crate::names::{double, string};
use bern_ir::Element;
use bern_protocol::Type;

impl Generator<'_> {
    /// The element's keyboard shortcut running `action`.
    pub(super) fn shortcut(&self, out: &mut Out, element: &Element, action: &str) {
        if let Some(shortcut) = element.keyword("shortcut") {
            out.line(format!("BernShortcut({}) {{ {action} }}", string(shortcut)));
        }
    }

    pub(super) fn button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (action, file) = self.pressed(element);
        let enabled = self.enabled(element);
        let target = self.target(element, ctx);
        out.line(format!(
            "BernControl({target}, \"button\", {label}, {enabled}, BernControlActions(activate = {{ {action} }}{file}))"
        ));
        self.shortcut(out, element, &action);
        if element.kind == "Button" && !element.children.is_empty() {
            // The content is what the button shows; the label names it.
            let style = self
                .style_of(element)
                .map(|style| format!("BernStyles.{}", style.name))
                .unwrap_or_else(|| "null".into());
            out.open(format!(
                "BernContentButton({label}, {enabled}, {style}, Modifier.testTag({target}), onClick = {{ {action} }}) {{"
            ));
            self.nodes(out, &element.children, &ctx.within(Scope::Other));
            out.close("}");
            return;
        }
        let style = self.style_arg(element);
        if element.kind == "IconButton" {
            out.line(format!(
                "BernIconButton({label}, {}, {enabled}, Modifier.testTag({target}){style}) {{ {action} }}",
                Self::icon(element.icon("icon"))
            ));
            return;
        }
        let prominence = match element.keyword("prominence") {
            Some("prominent") => "BernProminence.Prominent",
            Some("plain") => "BernProminence.Plain",
            _ => "BernProminence.Standard",
        };
        out.line(format!(
            "BernButton({label}, {}, {prominence}, destructive = {}, enabled = {enabled}{style}, modifier = Modifier.testTag({target})) {{ {action} }}",
            Self::icon(element.icon("icon")),
            element.keyword("role") == Some("destructive")
        ));
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("bool field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let more = self
            .text_prop(element, "more_label")
            .unwrap_or_else(|| "BernText.string(\"bern.split.more\")".into());
        self.shortcut(out, element, &format!("{field} = !{field}"));
        out.open(format!(
            "BernSplitButton({}, {label}, {}, {more}, {field}, {{ {field} = it }}) {{",
            self.target(element, ctx),
            Self::icon(element.icon("icon"))
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
    }

    /// A button that shows its children in a popover; in a top bar a
    /// label shows as a text button.
    pub(super) fn popover_button(&self, out: &mut Out, element: &Element, ctx: &Ctx, plain: bool) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (presented, on_presented) = match self.field(element, "presented") {
            Some(field) => (field.clone(), format!("{{ {field} = it }}")),
            None => ("null".into(), "null".into()),
        };
        let shortcut = element
            .keyword("shortcut")
            .map(string)
            .unwrap_or_else(|| "null".into());
        out.open(format!(
            "BernPopoverButton({}, {label}, {}, {presented}, {on_presented}, {shortcut}, {plain}) {{",
            self.target(element, ctx),
            Self::icon(element.icon("icon"))
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
    }

    pub(super) fn text_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("text field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "null".into());
        let kind = element.keyword("kind").unwrap_or("plain");
        let action = element.action.as_ref().map(|a| self.action(a));
        let target = self.target(element, ctx);
        let submit = action
            .as_ref()
            .map(|action| format!(", submit = {{ {action} }}"))
            .unwrap_or_default();
        // Secure text is never exposed to external entries.
        let value = if kind == "secure" {
            String::new()
        } else {
            format!(", value = {{ {field} }}")
        };
        out.line(format!(
            "BernControl({target}, \"text_input\", {label}, actions = BernControlActions(edit = {{ {field} = it }}{submit}{value}))"
        ));
        out.line(format!(
            "BernTextInput({field}, {{ {field} = it }}, {label}, {prompt}, {}, {}, Modifier.testTag({target}))",
            string(kind),
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into())
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("bool field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let target = self.target(element, ctx);
        out.line(format!(
            "BernControl({target}, \"switch\", {label}, actions = BernControlActions(setBool = {{ {field} = it }}, value = {{ if ({field}) \"true\" else \"false\" }}))"
        ));
        self.shortcut(out, element, &format!("{field} = !{field}"));
        if element.keyword("style") == Some("button") {
            out.line(format!(
                "BernToggleButton({label}, {}, {field}, {{ {field} = it }}, Modifier.testTag({target}))",
                Self::icon(element.icon("icon"))
            ));
        } else {
            out.line(format!(
                "BernSwitch({label}, {field}, {{ {field} = it }}, checkbox = {}, modifier = Modifier.testTag({target}))",
                element.keyword("style") == Some("checkbox")
            ));
        }
    }

    pub(super) fn slider(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("number field");
        let integer = matches!(
            element.field("value").map(|field| &field.ty),
            Some(Type::Int)
        );
        let label = self.text_prop(element, "label").unwrap_or_default();
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1.0));
        let step = element
            .number("step")
            .map(double)
            .unwrap_or_else(|| "null".into());
        let action = element.action.as_ref().map(|a| self.action(a));
        let target = self.target(element, ctx);
        let (read, write) = if integer {
            (
                format!("{field}.toDouble()"),
                format!("{field} = Math.round(it)"),
            )
        } else {
            (field.clone(), format!("{field} = it"))
        };
        let commit = action
            .as_ref()
            .map(|action| format!("; {action}"))
            .unwrap_or_default();
        out.line(format!(
            "BernControl({target}, \"slider\", {label}, actions = BernControlActions(setNumber = {{ {write}{commit} }}, value = {{ BernText.text({field}) }}))"
        ));
        out.line(format!(
            "BernSlider({label}, {read}, {{ {write} }}, {min}, {max}, {step}, {}, {}, {}, Modifier.testTag({target}))",
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into()),
            Self::icon(element.icon("min_icon")),
            Self::icon(element.icon("max_icon"))
        ));
    }

    pub(super) fn picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field_ref = element.field("value").expect("picker field");
        let field = self.field(element, "value").expect("picker field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let enum_type = kotlin_type(&field_ref.ty);
        let labeled = match &field_ref.ty {
            Type::Enum { name } => self
                .types()
                .enum_def(name)
                .is_some_and(|def| def.variants.iter().all(|variant| variant.label.is_some())),
            _ => false,
        };
        let option_label = if labeled {
            "it.bernLabel"
        } else {
            "it.bernName"
        };
        let style = element.keyword("style").unwrap_or("menu");
        let target = self.target(element, ctx);
        out.line(format!(
            "BernControl({target}, \"picker\", {label}, actions = BernControlActions(select = {{ name -> {enum_type}.fromBernName(name)?.let {{ {field} = it; true }} ?: false }}, value = {{ {field}.bernName }}))"
        ));
        out.line(format!(
            "BernPicker({label}, {enum_type}.entries, {field}, {{ {option_label} }}, {{ {field} = it }}, {}, Modifier.testTag({target}))",
            string(style)
        ));
    }

    pub(super) fn language_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        out.line(format!(
            "BernLanguagePicker({label}, {})",
            self.target(element, ctx)
        ));
    }

    pub(super) fn drop_zone(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_else(|| "Unit".into());
        let (give, actions) = self
            .file_target(element, "value", &action)
            .expect("file field");
        let over = self
            .field(element, "over")
            .map(|field| format!("{{ over: Boolean -> {field} = over }}"))
            .unwrap_or_else(|| "null".into());
        out.line(format!(
            "BernControl({}, \"drop\", {label}, actions = BernControlActions({actions}))",
            self.target(element, ctx)
        ));
        out.open(format!(
            "BernDrop({}, {over}, {}, {give}) {{",
            string(element.keyword("files").unwrap_or("any")),
            element.keyword("opens") == Some("true")
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    pub(super) fn gesture(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("number field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_else(|| "Unit".into());
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1.0));
        out.line(format!(
            "BernControl({}, {}, {label}, actions = BernControlActions(setNumber = {{ {field} = it.coerceIn({min}, {max}); {action} }}, value = {{ {field}.toString() }}))",
            self.target(element, ctx),
            string(&element.kind.to_lowercase())
        ));
        if element.kind == "Pinch" {
            out.open(format!(
                "BernPinch({label}, {field}, {{ {field} = it }}, {min}, {max}, {{ {action} }}) {{"
            ));
        } else {
            out.open(format!(
                "BernSwipe({label}, {field}, {{ {field} = it }}, {min}, {max}, {}, {}, {{ {action} }}) {{",
                element.keyword("axis") == Some("horizontal"),
                double(element.number("distance").unwrap_or(300.0))
            ));
        }
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }
}
