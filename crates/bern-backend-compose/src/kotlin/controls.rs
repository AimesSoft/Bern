//! Controls: buttons, toggles, pickers, sliders, text input, file
//! drops and gestures.

use super::values::kotlin_type;
use super::{Ctx, Generator, Out, Scope};
use crate::names::{double, float, string};
use bern_ir::{Element, Node};
use bern_protocol::Type;

impl Generator<'_> {
    /// The element's keyboard shortcut running `action`.
    pub(super) fn shortcut(&self, out: &mut Out, element: &Element, action: &str) {
        if let Some(shortcut) = element.keyword("shortcut") {
            out.line(format!("BernShortcut({}) {{ {action} }}", string(shortcut)));
        }
    }

    /// `, shape = BernShape(…)` for a system control with `shape: capsule`
    /// or a `height`.
    pub(super) fn shape_arg(&self, element: &Element) -> String {
        let capsule = element.keyword("shape") == Some("capsule");
        let height = self.length(element, "height");
        if !capsule && height.is_none() {
            return String::new();
        }
        format!(
            ", shape = BernShape({capsule}, {})",
            height.unwrap_or_else(|| "null".into())
        )
    }

    pub(super) fn button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (action, file) = self.pressed(element);
        let enabled = self.enabled(element);
        let target = self.target(element, ctx);
        // `default_focus`: the focus starts on the button once it shows.
        let start = if element.keyword("default_focus") == Some("true") {
            ".then(bernDefaultFocus())"
        } else {
            ""
        };
        out.line(format!(
            "BernControl({target}, \"button\", {label}, {enabled}, BernControlActions(activate = {{ {action} }}{file}))"
        ));
        self.shortcut(out, element, &action);
        // `width: fill`: in a row the buttons that fill share its width;
        // elsewhere one takes the whole width. Content that stretches,
        // such as a row with a spacer, takes the room its neighbours in a
        // row leave, as it does in a SwiftUI stack.
        let fills = element.keyword("width") == Some("fill");
        let widens = element.kind == "Button" && widens(&element.children);
        let in_row = (fills || widens) && ctx.scope == Scope::Row;
        let weight = if in_row { ".weight(1f)" } else { "" };
        if element.kind == "Button" && !element.children.is_empty() {
            // The content is what the button shows; the label names it.
            let style = self
                .style_of(element)
                .map(|style| format!("BernStyles.{}", style.name))
                .unwrap_or_else(|| "null".into());
            let prominent = if element.keyword("prominence") == Some("prominent") {
                ", prominent = true"
            } else {
                ""
            };
            let width = match (in_row, fills) {
                (true, _) => weight,
                (false, true) => ".fillMaxWidth()",
                _ => "",
            };
            out.open(format!(
                "BernContentButton({label}, {enabled}, {style}, Modifier{start}{width}.testTag({target}){}{prominent}, onClick = {{ {action} }}) {{",
                self.shape_arg(element)
            ));
            self.nodes(out, &element.children, &ctx.within(Scope::Other));
            out.close("}");
            return;
        }
        let style = self.style_arg(element);
        let prominence = match element.keyword("prominence") {
            Some("prominent") => "BernProminence.Prominent",
            Some("plain") => "BernProminence.Plain",
            _ => "BernProminence.Standard",
        };
        if element.kind == "IconButton" {
            // With `height`, a round button of that diameter; `font_size`
            // is the size of its icon.
            let mut extra = String::new();
            if let Some(size) = element.number("font_size") {
                extra.push_str(&format!(", iconSize = {}", float(size)));
            }
            if let Some(height) = self.length(element, "height") {
                extra.push_str(&format!(", diameter = {height}"));
            }
            if element.keyword("prominence").is_some() {
                extra.push_str(&format!(", prominence = {prominence}"));
            }
            out.line(format!(
                "BernIconButton({label}, {}, {enabled}, Modifier{start}.testTag({target}){style}{extra}) {{ {action} }}",
                Self::icon(element.icon("icon"))
            ));
            return;
        }
        let fill = if fills && !in_row {
            ", fill = true"
        } else {
            ""
        };
        out.line(format!(
            "BernButton({label}, {}, {prominence}, destructive = {}, enabled = {enabled}{style}{}{fill}, modifier = Modifier{start}{weight}.testTag({target})) {{ {action} }}",
            Self::icon(element.icon("icon")),
            element.keyword("role") == Some("destructive"),
            self.shape_arg(element)
        ));
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let Some((value, change)) = self.model(element, "checked", &|_| "it".into()) else {
            return;
        };
        let label = self.text_prop(element, "label").unwrap_or_default();
        let more = self
            .text_prop(element, "more_label")
            .unwrap_or_else(|| "BernText.string(\"bern.split.more\")".into());
        self.shortcut(
            out,
            element,
            &format!("run {{ val it = !({value}); {change} }}"),
        );
        out.open(format!(
            "BernSplitButton({}, {label}, {}, {more}, {value}, {{ it: Boolean -> {change} }}{}) {{",
            self.target(element, ctx),
            Self::icon(element.icon("icon")),
            self.shape_arg(element)
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
        let action = element.action.as_ref().map(|a| match element.clearing() {
            Some(clear) => format!("{}; {}", self.action(a), self.action(&clear)),
            None => self.action(a),
        });
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
        // In a row the field takes the room its neighbours leave, as a
        // text field does in a SwiftUI stack.
        let weight = if ctx.scope == Scope::Row {
            ".weight(1f)"
        } else {
            ""
        };
        out.line(format!(
            "BernTextInput({field}, {{ {field} = it }}, {label}, {prompt}, {}, {}, Modifier.testTag({target}){weight}, labelHidden = {})",
            string(kind),
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into()),
            element.keyword("label_hidden") == Some("true")
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let Some((value, change)) = self.model(element, "checked", &|_| "it".into()) else {
            return;
        };
        let label = self.text_prop(element, "label").unwrap_or_default();
        let target = self.target(element, ctx);
        out.line(format!(
            "BernControl({target}, \"switch\", {label}, actions = BernControlActions(setBool = {{ it: Boolean -> {change} }}, value = {{ if ({value}) \"true\" else \"false\" }}))"
        ));
        self.shortcut(
            out,
            element,
            &format!("run {{ val it = !({value}); {change} }}"),
        );
        if element.keyword("style") == Some("button") {
            out.line(format!(
                "BernToggleButton({label}, {}, {value}, {{ it: Boolean -> {change} }}, Modifier.testTag({target}){})",
                Self::icon(element.icon("icon")),
                self.shape_arg(element)
            ));
        } else {
            out.line(format!(
                "BernSwitch({label}, {value}, {{ it: Boolean -> {change} }}, checkbox = {}, labelHidden = {}, modifier = Modifier.testTag({target}))",
                element.keyword("style") == Some("checkbox"),
                element.keyword("label_hidden") == Some("true")
            ));
        }
    }

    pub(super) fn slider(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1.0));
        let step = element
            .number("step")
            .map(double)
            .unwrap_or_else(|| "null".into());
        let target = self.target(element, ctx);
        let label_hidden = element.keyword("label_hidden") == Some("true");
        let icons = format!(
            "{}, {}",
            Self::icon(element.icon("min_icon")),
            Self::icon(element.icon("max_icon"))
        );
        // A whole number takes the slider's value rounded.
        let convert = |ty: &Type| match ty {
            Type::Int => "Math.round(it)".to_string(),
            _ => "it".to_string(),
        };
        // Showing engine state: dragging moves only the slider, letting go
        // runs the command with the new value.
        if element.expr("current").is_some() {
            let Some((value, change)) = self.model(element, "current", &convert) else {
                return;
            };
            out.line(format!(
                "BernControl({target}, \"slider\", {label}, actions = BernControlActions(setNumber = {{ it: Double -> {change} }}, value = {{ BernText.text({value}) }}))"
            ));
            out.line(format!(
                "BernControlledSlider({label}, ({value}).toDouble(), {{ it: Double -> {change} }}, {min}, {max}, {step}, {icons}, labelHidden = {label_hidden}, modifier = Modifier.testTag({target}))"
            ));
            return;
        }
        let Some(field) = self.field(element, "value") else {
            return;
        };
        let integer = matches!(
            element.field("value").map(|field| &field.ty),
            Some(Type::Int)
        );
        let action = element.action.as_ref().map(|a| self.action(a));
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
            "BernSlider({label}, {read}, {{ {write} }}, {min}, {max}, {step}, {}, {icons}, labelHidden = {label_hidden}, modifier = Modifier.testTag({target}))",
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into()),
        ));
    }

    /// The options of a picker: its `PickerOption`s, or every variant of
    /// its enum; a Kotlin list of the values and a lambda naming each.
    pub(super) fn picker_options(&self, element: &Element, ty: &Type) -> Option<(String, String)> {
        let explicit: Vec<(String, String)> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(option) if option.kind == "PickerOption" => Some(option),
                _ => None,
            })
            .filter_map(|option| {
                Some((
                    self.expr(option.expr("value")?),
                    self.text_prop(option, "label")?,
                ))
            })
            .collect();
        let kotlin = kotlin_type(ty);
        if !explicit.is_empty() {
            let values: Vec<_> = explicit.iter().map(|(value, _)| value.clone()).collect();
            let arms: Vec<_> = explicit
                .iter()
                .map(|(value, label)| format!("{value} -> {label}"))
                .collect();
            return Some((
                format!("listOf<{kotlin}>({})", values.join(", ")),
                format!(
                    "{{ option: {kotlin} -> when (option) {{ {}; else -> \"\" }} }}",
                    arms.join("; ")
                ),
            ));
        }
        let Type::Enum { name } = ty else {
            return None;
        };
        let def = self.types().enum_def(name)?;
        let labeled = def.variants.iter().all(|variant| variant.label.is_some());
        let label = if labeled {
            "it.bernLabel"
        } else {
            "it.bernName"
        };
        Some((format!("{kotlin}.entries"), format!("{{ {label} }}")))
    }

    pub(super) fn picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        self.picker_in(out, element, ctx, false);
    }

    /// A picker; in a top bar (`toolbar`) the chosen option opening a menu.
    pub(super) fn picker_in(&self, out: &mut Out, element: &Element, ctx: &Ctx, toolbar: bool) {
        let Some(ty) = element
            .expr("selected")
            .map(|expr| expr.ty.clone())
            .or_else(|| element.field("value").map(|field| field.ty.clone()))
        else {
            return;
        };
        let Some((options, option_label)) = self.picker_options(element, &ty) else {
            return;
        };
        let Some((value, change)) = self.model(element, "selected", &|_| "it".into()) else {
            return;
        };
        let label = self.text_prop(element, "label").unwrap_or_default();
        let target = self.target(element, ctx);
        out.line(format!(
            "BernControl({target}, \"picker\", {label}, actions = BernControlActions(select = {{ name -> ({options}).firstOrNull {{ bernOptionName(it) == name }}?.let {{ {change}; true }} ?: false }}, value = {{ bernOptionName({value}) }}))"
        ));
        if toolbar {
            out.line(format!(
                "BernToolbarPicker({label}, {options}, {value}, {option_label}, {{ {change} }}, Modifier.testTag({target}))"
            ));
            return;
        }
        let style = element.keyword("style").unwrap_or("menu");
        // In a flow the picker keeps its own width, and chips are items of
        // the flow.
        let flow = if ctx.scope == Scope::Flow {
            ", inFlow = true"
        } else {
            ""
        };
        out.line(format!(
            "BernPicker({label}, {options}, {value}, {option_label}, {{ {change} }}, {}, Modifier.testTag({target}), labelHidden = {}{}{flow})",
            string(style),
            element.keyword("label_hidden") == Some("true"),
            self.shape_arg(element)
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

/// Whether content stretches across the width it is given: a row with a
/// spacer, something that fills the width, or a column or stack holding
/// such content, also through conditions.
fn widens(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(element) => {
            element.keyword("width") == Some("fill")
                || match element.kind.as_str() {
                    "Row" => {
                        element.children.iter().any(
                            |child| matches!(child, Node::Element(child) if child.kind == "Spacer"),
                        ) || widens(&element.children)
                    }
                    "Column" | "Stack" | "ForEach" => widens(&element.children),
                    _ => false,
                }
        }
        Node::If(branch) => {
            branch.branches.iter().any(|(_, nodes)| widens(nodes)) || widens(&branch.otherwise)
        }
        _ => false,
    })
}
