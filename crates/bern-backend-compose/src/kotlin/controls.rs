//! Controls: buttons, toggles, pickers, sliders, text input, file
//! drops and gestures.

use super::values::kotlin_type;
use super::{Ctx, Generator, Out, Scope};
use crate::names::{double, float, member, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// The element's keyboard shortcut running `action`.
    pub(super) fn shortcut(&self, out: &mut Out, element: &Element, action: &str) {
        if let Some(shortcut) = self.shortcut_keys(element, "shortcut") {
            out.line(format!("BernShortcut({shortcut}) {{ {action} }}"));
        }
    }

    /// The keys of the shortcut prop `name`: a literal, or the text the
    /// shortcut follows, which registers anew as it changes.
    pub(super) fn shortcut_keys(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            bern_ir::Prop::Keyword { value } => Some(string(value)),
            bern_ir::Prop::Expr { expr } => Some(self.expr(expr)),
            _ => None,
        }
    }

    /// `ShortcutInput`: a button that takes the next key pressed as the
    /// shortcut, which the action keeps.
    pub(super) fn shortcut_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let Some((value, change)) = self.model(element, "current", &|_| "it".into()) else {
            return;
        };
        let label = self.text_prop(element, "label").unwrap_or_default();
        let optional = |name| {
            self.text_prop(element, name)
                .unwrap_or_else(|| "null".into())
        };
        out.line(format!(
            "BernShortcutInput({}, {label}, {}, {value}, {}, {}, {}) {{ it: String -> {change} }}",
            self.target(element, ctx),
            element.keyword("label_hidden") == Some("true"),
            optional("prompt"),
            optional("empty"),
            self.enabled(element)
        ));
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
        let double_tap = self
            .event_action(element, "DoubleTap")
            .map(|action| format!(", onDoubleTap = {{ {action} }}"))
            .unwrap_or_default();
        let content: Vec<_> = element.content().cloned().collect();
        let wheel = self.wheel_modifier(element);
        let hover = self.hover_modifier(element);
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
        let widens = element.kind == "Button" && widens(&content);
        let in_row = (fills || widens) && ctx.scope == Scope::Row;
        let weight = if in_row { ".weight(1f)" } else { "" };
        if element.kind == "Button" && !content.is_empty() {
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
                "BernContentButton({label}, {enabled}, {style}, Modifier{start}{width}.testTag({target}){wheel}{hover}{}{prominent}{double_tap}, onClick = {{ {action} }}) {{",
                self.shape_arg(element)
            ));
            self.nodes(out, &content, &ctx.within(Scope::Other));
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
            if let Some(value) = element.expr("icon_progress") {
                extra.push_str(&format!(
                    ", iconProgress = ({}).toDouble()",
                    self.expr(value)
                ));
            }
            if let Some(icon) = element.icon("badge_icon") {
                extra.push_str(&format!(", badgeIcon = {}", Self::icon(Some(icon))));
            }
            if let Some(value) = element.expr("badge_visible") {
                extra.push_str(&format!(", badgeVisible = {}", self.expr(value)));
            }
            if let Some(size) = element.number("font_size") {
                extra.push_str(&format!(", iconSize = {}", float(size)));
            }
            if let Some(height) = self.length(element, "height") {
                extra.push_str(&format!(", diameter = {height}"));
            }
            if element.keyword("prominence").is_some() {
                extra.push_str(&format!(", prominence = {prominence}"));
            }
            if let Some(role) = element.keyword("color") {
                extra.push_str(&format!(", color = BernColor.Role({})", string(role)));
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
            "BernButton({label}, {}, {prominence}, destructive = {}, enabled = {enabled}{style}{}{fill}, modifier = Modifier{start}{weight}.testTag({target}){wheel}{hover}{double_tap}) {{ {action} }}",
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
        let (presented, on_presented) = if element.expr("open").is_some() {
            let (value, change) = self
                .model(element, "open", &|_| "it".into())
                .expect("controlled popover");
            (value, format!("{{ it: Boolean -> {change} }}"))
        } else {
            match self.field(element, "presented") {
                Some(field) => (field.clone(), format!("{{ {field} = it }}")),
                None => ("null".into(), "null".into()),
            }
        };
        let shortcut = self
            .shortcut_keys(element, "shortcut")
            .unwrap_or_else(|| "null".into());
        let plain = plain || element.keyword("prominence") == Some("plain");
        let icon_size = element
            .number("font_size")
            .map(|size| format!(", iconSize = {}", float(size)))
            .unwrap_or_default();
        let placement = element
            .keyword("placement")
            .map(|place| format!(", placement = {}", string(place)))
            .unwrap_or_default();
        out.open(format!(
            "BernPopoverButton({}, {label}, {}, {presented}, {on_presented}, {shortcut}, {plain}, windowTheme = {}{}{icon_size}{}{placement}) {{",
            self.target(element, ctx),
            Self::icon(element.icon("icon")),
            bern_target_api::style::popover_window_theme(self.app, "android", element),
            self.style_arg(element),
            self.shape_arg(element)
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
    }

    pub(super) fn text_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "null".into());
        let kind = element.keyword("kind").unwrap_or("plain");
        // In a row the field takes the room its neighbours leave, as a
        // text field does in a SwiftUI stack.
        let weight = if ctx.scope == Scope::Row {
            ".weight(1f)"
        } else {
            ""
        };
        // `current:`: text from state whose every edit runs the action.
        if element.expr("current").is_some() {
            let Some((value, change)) = self.model(element, "current", &|_| "it".into()) else {
                return;
            };
            let target = self.target(element, ctx);
            let value_action = if kind == "secure" {
                String::new()
            } else {
                format!(", value = {{ {value} }}")
            };
            if self.watch {
                out.open("run {");
                out.line("val bernInputFocus = remember { BernWearInputFocus() }");
                out.line(format!(
                    "BernControl({target}, \"text_input\", {label}, actions = BernControlActions(edit = {{ {change} }}, focus = {{ bernInputFocus.request() }}{value_action}))"
                ));
                out.line(format!(
                    "BernHeldTextInput({value}, {{ {change} }}, {label}, {prompt}, {}, Modifier.testTag({target}){weight}, labelHidden = {}, inputFocus = bernInputFocus)",
                    string(kind),
                    element.keyword("label_hidden") == Some("true")
                ));
                out.close("}");
            } else {
                out.line(format!(
                    "BernControl({target}, \"text_input\", {label}, actions = BernControlActions(edit = {{ {change} }}{value_action}))"
                ));
                out.line(format!(
                    "BernHeldTextInput({value}, {{ {change} }}, {label}, {prompt}, {}, Modifier.testTag({target}){weight}, labelHidden = {})",
                    string(kind),
                    element.keyword("label_hidden") == Some("true")
                ));
            }
            return;
        }
        let field = self.field(element, "value").expect("text field");
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
        if self.watch {
            out.open("run {");
            out.line("val bernInputFocus = remember { BernWearInputFocus() }");
            out.line(format!(
                "BernControl({target}, \"text_input\", {label}, actions = BernControlActions(edit = {{ {field} = it }}, focus = {{ bernInputFocus.request() }}{submit}{value}))"
            ));
            out.line(format!(
                "BernTextInput({field}, {{ {field} = it }}, {label}, {prompt}, {}, {}, Modifier.testTag({target}){weight}, labelHidden = {}, inputFocus = bernInputFocus)",
                string(kind),
                action
                    .map(|action| format!("{{ {action} }}"))
                    .unwrap_or_else(|| "null".into()),
                element.keyword("label_hidden") == Some("true")
            ));
            out.close("}");
        } else {
            out.line(format!(
                "BernControl({target}, \"text_input\", {label}, actions = BernControlActions(edit = {{ {field} = it }}{submit}{value}))"
            ));
            out.line(format!(
                "BernTextInput({field}, {{ {field} = it }}, {label}, {prompt}, {}, {}, Modifier.testTag({target}){weight}, labelHidden = {})",
                string(kind),
                action
                    .map(|action| format!("{{ {action} }}"))
                    .unwrap_or_else(|| "null".into()),
                element.keyword("label_hidden") == Some("true")
            ));
        }
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        // Android applications do not open at login.
        if element.keyword("login_item") == Some("true") {
            return;
        }
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
                "BernSwitch({label}, {value}, {{ it: Boolean -> {change} }}, checkbox = {}, labelHidden = {}, modifier = Modifier.testTag({target}){})",
                element.keyword("style") == Some("checkbox"),
                element.keyword("label_hidden") == Some("true"),
                self.style_arg(element)
            ));
        }
    }

    /// In a row a horizontal slider takes what the others leave; in a column a
    /// vertical slider without a declared height takes the vertical share.
    pub(super) fn slider_flex(element: &Element, ctx: &Ctx) -> &'static str {
        let vertical = element.keyword("axis") == Some("vertical");
        if vertical {
            if ctx.scope == Scope::Column && !ctx.scroll_content && element.prop("height").is_none()
            {
                ".weight(1f)"
            } else {
                ""
            }
        } else if ctx.scope == Scope::Row {
            ".weight(1f)"
        } else {
            ""
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
        let vertical = element.keyword("axis") == Some("vertical");
        let vertical_arg = if vertical { ", vertical = true" } else { "" };
        let weight = Self::slider_flex(element, ctx);
        // Where a mouse or stylus is along the track, into its draft.
        let hover = self
            .field(element, "hover")
            .filter(|_| !self.watch)
            .map(|field| format!(", onHover = {{ it: Double? -> {field} = it }}"))
            .unwrap_or_default();
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
                "BernControlledSlider({label}, ({value}).toDouble(), {{ it: Double -> {change} }}, {min}, {max}, {step}, {icons}, labelHidden = {label_hidden}, continuous = {}, modifier = Modifier{weight}.testTag({target}){hover}{vertical_arg})",
                element.keyword("continuous") == Some("true")
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
            "BernSlider({label}, {read}, {{ {write} }}, {min}, {max}, {step}, {}, {icons}, labelHidden = {label_hidden}, modifier = Modifier{weight}.testTag({target}){hover}{vertical_arg})",
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into()),
        ));
    }

    pub(super) fn color_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let target = self.target(element, ctx);
        let label_hidden = element.keyword("label_hidden") == Some("true");
        let convert = |ty: &Type| match ty {
            Type::Int => "Math.round(it)".to_string(),
            _ => "it".to_string(),
        };
        if element.expr("current").is_some() {
            let Some((value, change)) = self.model(element, "current", &convert) else {
                return;
            };
            out.line(format!(
                "BernControl({target}, \"color_picker\", {label}, actions = BernControlActions(setNumber = {{ it: Double -> {change} }}, value = {{ BernText.text({value}) }}))"
            ));
            out.line(format!(
                "BernColorPicker({label}, ({value}).toLong(), {{ next: Long -> val it = next.toDouble(); {change} }}, labelHidden = {label_hidden}, modifier = Modifier.testTag({target}))"
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
                format!("{field}.toLong()"),
                format!("{field} = Math.round(it)"),
            )
        } else {
            (format!("({field}).toLong()"), format!("{field} = it"))
        };
        let commit = action
            .as_ref()
            .map(|action| format!("; {action}"))
            .unwrap_or_default();
        out.line(format!(
            "BernControl({target}, \"color_picker\", {label}, actions = BernControlActions(setNumber = {{ {write}{commit} }}, value = {{ BernText.text({field}) }}))"
        ));
        out.line(format!(
            "BernColorPicker({label}, {read}, {{ next: Long -> val it = next.toDouble(); {write}{commit} }}, labelHidden = {label_hidden}, modifier = Modifier.testTag({target}))"
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

    pub(super) fn tooltip(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let keys = self
            .shortcut_keys(element, "keys")
            .map(|keys| format!(", keys = {keys}"))
            .unwrap_or_default();
        let placement = element
            .keyword("placement")
            .map(|place| format!(", placement = {}", string(place)))
            .unwrap_or_default();
        out.open(format!("BernTooltip({content}{keys}{placement}) {{"));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn banner(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let severity = element.keyword("severity").unwrap_or("info");
        let default_icon = match severity {
            "success" => "checkmark.circle",
            "warning" => "exclamationmark.triangle",
            "error" => "xmark.circle.fill",
            _ => "info.circle",
        };
        let icon = Self::icon(Some(element.icon("icon").unwrap_or(default_icon)));
        let title = self
            .text_prop(element, "title")
            .unwrap_or_else(|| "null".into());
        let content = self.text_prop(element, "content").unwrap_or_default();
        let dismiss_label = self
            .text_prop(element, "dismiss_label")
            .unwrap_or_else(|| "null".into());
        let (dismiss_target, on_dismiss) = match &element.action {
            Some(action) => {
                let target = element
                    .id
                    .as_ref()
                    .map(|_| self.target(element, ctx))
                    .unwrap_or_else(|| "null".into());
                (target, format!("{{ {} }}", self.action(action)))
            }
            None => ("null".into(), "null".into()),
        };
        out.open(format!(
            "BernBanner({}, {title}, {content}, {icon}, {dismiss_label}, {dismiss_target}, {on_dismiss}) {{",
            string(severity)
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Row));
        out.close("}");
    }

    pub(super) fn disclosure(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let icon = Self::icon(element.icon("icon"));
        let target = self.target(element, ctx);
        let (external_val, on_change) = if element.expr("open").is_some() {
            let Some((val, change)) = self.model(element, "open", &|_| "it".into()) else {
                return;
            };
            (val, format!("{{ it: Boolean -> {change} }}"))
        } else if let Some(field) = self.field(element, "expanded") {
            let then = element
                .action
                .as_ref()
                .map(|a| format!("; {}", self.action(a)))
                .unwrap_or_default();
            (
                field.clone(),
                format!("{{ it: Boolean -> {field} = it{then} }}"),
            )
        } else {
            let on_toggle = element
                .action
                .as_ref()
                .map(|a| format!("{{ _: Boolean -> {} }}", self.action(a)))
                .unwrap_or_else(|| "null".into());
            ("null".into(), on_toggle)
        };
        out.open(format!(
            "BernDisclosure({target}, {label}, {icon}, {external_val}, {on_change}) {{"
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
    }

    pub(super) fn number_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let target = self.target(element, ctx);
        let label_hidden = element.keyword("label_hidden") == Some("true");
        let min = element
            .number("min")
            .map(double)
            .unwrap_or_else(|| "null".into());
        let max = element
            .number("max")
            .map(double)
            .unwrap_or_else(|| "null".into());
        let step = double(element.number("step").unwrap_or(1.0));
        let is_int = matches!(element.expr("current").map(|e| &e.ty), Some(Type::Int));
        let precision =
            infer_precision(element.number("precision"), element.number("step"), is_int);
        let enabled = element
            .expr("enabled")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        let convert = |ty: &Type| match ty {
            Type::Int => "Math.round(it)".to_string(),
            _ => "it".to_string(),
        };
        let Some((value, change)) = self.model(element, "current", &convert) else {
            return;
        };
        out.line(format!(
            "BernNumberInput({target}, {label}, ({value}).toDouble(), {{ it: Double -> {change} }}, {min}, {max}, {step}, {precision}, {enabled}, {label_hidden}, Modifier.testTag({target}))"
        ));
    }

    pub(super) fn alert(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let title = self.text_prop(element, "title").unwrap_or_default();
        let message = self
            .text_prop(element, "message")
            .unwrap_or_else(|| "null".into());
        let on_dismiss = element
            .action
            .as_ref()
            .map(|a| format!("{{ {} }}", self.action(a)))
            .unwrap_or_else(|| "null".into());
        let mut specs = Vec::new();
        for node in &element.children {
            let Node::Element(act) = node else { continue };
            if act.kind != "AlertAction" {
                continue;
            }
            let target = self.target(act, ctx);
            let label = self.text_prop(act, "label").unwrap_or_default();
            let role = act.keyword("role").unwrap_or("default");
            let enabled = act
                .expr("enabled")
                .map(|e| self.expr(e))
                .unwrap_or_else(|| "true".into());
            let code = act
                .action
                .as_ref()
                .map(|a| self.action(a))
                .unwrap_or_else(|| "Unit".into());
            specs.push(format!(
                "BernAlertActionSpec({target}, {label}, {}, {enabled}) {{ {code} }}",
                string(role)
            ));
        }
        out.line(format!(
            "BernAlert({title}, {message}, listOf({}), {on_dismiss})",
            specs.join(", ")
        ));
    }

    pub(super) fn date_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let target = self.target(element, ctx);
        let label_hidden = element.keyword("label_hidden") == Some("true");
        let enabled = element
            .expr("enabled")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        let date_type = crate::names::value_type(bern_protocol::format::CALENDAR_DATE);
        let min_arg = element
            .expr("min")
            .map(|e| {
                let v = self.expr(e);
                format!("Triple(({v}).year, ({v}).month, ({v}).day)")
            })
            .unwrap_or_else(|| "null".into());
        let max_arg = element
            .expr("max")
            .map(|e| {
                let v = self.expr(e);
                format!("Triple(({v}).year, ({v}).month, ({v}).day)")
            })
            .unwrap_or_else(|| "null".into());
        let Some((value, change)) = self.model(element, "current", &|_| "it".into()) else {
            return;
        };
        out.line(format!(
            "BernDatePicker({target}, {label}, ({value}).year, ({value}).month, ({value}).day, {{ y, m, d -> val it = {date_type}(y, m, d); {change} }}, {min_arg}, {max_arg}, {enabled}, {label_hidden}, Modifier.testTag({target}))"
        ));
    }

    pub(super) fn time_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let target = self.target(element, ctx);
        let label_hidden = element.keyword("label_hidden") == Some("true");
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1439.0));
        let step = double(element.number("step").unwrap_or(1.0));
        let enabled = element
            .expr("enabled")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        let convert = |ty: &Type| match ty {
            Type::Int => "Math.round(it)".to_string(),
            _ => "it".to_string(),
        };
        let Some((value, change)) = self.model(element, "current", &convert) else {
            return;
        };
        out.line(format!(
            "BernTimePicker({target}, {label}, ({value}).toDouble(), {{ it: Double -> {change} }}, {min}, {max}, {step}, {enabled}, {label_hidden}, Modifier.testTag({target}))"
        ));
    }

    pub(super) fn empty_state(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let title = self.text_prop(element, "title").unwrap_or_default();
        let message = self
            .text_prop(element, "message")
            .unwrap_or_else(|| "null".into());
        let icon = Self::icon(element.icon("icon"));
        out.open(format!("BernEmptyState({title}, {message}, {icon}) {{"));
        self.nodes(out, &element.children, &ctx.within(Scope::Row));
        out.close("}");
    }

    pub(super) fn badge(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let tone = element.keyword("tone").unwrap_or("neutral");
        let icon = Self::icon(element.icon("icon"));
        out.line(format!("BernBadge({content}, {}, {icon})", string(tone)));
    }

    pub(super) fn avatar(&self, out: &mut Out, element: &Element) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let size = self
            .length(element, "size")
            .unwrap_or_else(|| "40.dp".into());
        let rounded = element.keyword("shape") == Some("rounded");
        let (name_arg, url_arg) = match element.expr("source") {
            Some(expr) if matches!(expr.ty, Type::Optional { .. }) => {
                ("null".into(), format!("({})?.url", self.expr(expr)))
            }
            Some(expr) => ("null".into(), format!("({}).url", self.expr(expr))),
            None => (
                element
                    .resource("source")
                    .map(string)
                    .unwrap_or_else(|| "null".into()),
                "null".into(),
            ),
        };
        out.line(format!(
            "BernAvatar({label}, {name_arg}, {url_arg}, {size}, {rounded})"
        ));
    }

    pub(super) fn tree_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (
            Some(items_expr),
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
        let label = self.text_prop(element, "label").unwrap_or_default();
        let items = self.expr(items_expr);
        let key_prop = member(key_name);
        let parent_prop = member(parent_name);
        let var = member(&template.var);
        let (sel_val, on_sel) = match self.field(element, "selection") {
            Some(field) => (field.clone(), format!("{{ {field} = it }}")),
            None => ("null".into(), "null".into()),
        };
        let (exp_val, on_exp) = match self.field(element, "expanded") {
            Some(field) => (field.clone(), format!("{{ {field} = it }}")),
            None => ("null".into(), "null".into()),
        };
        let on_activate = element
            .action
            .as_ref()
            .map(|a| format!("{{ {} }}", self.action(a)))
            .unwrap_or_else(|| "null".into());
        out.open(format!(
            "BernTreeView(target = {}, label = {label}, entries = ({items}).map {{ BernTreeEntry(it, it.{key_prop}, it.{parent_prop}) }}, selectedKey = {sel_val}, onSelect = {on_sel}, externalExpanded = {exp_val}, onExpandedChange = {on_exp}, onActivate = {on_activate}, keyToString = {{ it.toString() }}) {{ {var} ->",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Row));
        out.close("}");
    }

    pub(super) fn split_pane(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        if element.children.len() < 2 {
            return;
        }
        let target = element
            .id
            .as_ref()
            .map(|_| self.target(element, ctx))
            .unwrap_or_else(|| "null".into());
        let vertical = element.keyword("axis") == Some("vertical");
        let (ratio_val, on_ratio) = match self.field(element, "ratio") {
            Some(field) => (field.clone(), format!("{{ {field} = it }}")),
            None => ("null".into(), "null".into()),
        };
        let min_first = self
            .length(element, "min_first")
            .unwrap_or_else(|| "120.dp".into());
        let min_second = self
            .length(element, "min_second")
            .unwrap_or_else(|| "120.dp".into());
        out.open(format!(
            "BernSplitPane(target = {target}, vertical = {vertical}, externalRatio = {ratio_val}, onRatioChange = {on_ratio}, minFirst = {min_first}, minSecond = {min_second}, first = {{"
        ));
        self.node(out, &element.children[0], &ctx.within(Scope::Other));
        out.close("}, second = {");
        out.indent += 1;
        self.node(out, &element.children[1], &ctx.within(Scope::Other));
        out.close("})");
    }

    pub(super) fn reorderable(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (Some(items_expr), Some(template), Some(from_field), Some(to_field), Some(action)) = (
            element.expr("items"),
            &element.template,
            self.field(element, "move_from"),
            self.field(element, "move_to"),
            &element.action,
        ) else {
            return;
        };
        let label = self.text_prop(element, "label").unwrap_or_default();
        let items = self.expr(items_expr);
        let var = member(&template.var);
        let act = self.action(action);
        out.open(format!(
            "BernReorderable(target = {}, label = {label}, items = {items}, onMove = {{ fromIndex, toIndex -> {from_field} = fromIndex; {to_field} = toIndex; {act} }}) {{ {var} ->",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Row));
        out.close("}");
    }

    pub(super) fn bottom_sheet(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let detents = element.keyword("detents").unwrap_or("medium");
        let drag_indicator = element.keyword("drag_indicator") != Some("false");
        let on_dismiss = element
            .action
            .as_ref()
            .map(|a| format!("{{ {} }}", self.action(a)))
            .unwrap_or_else(|| "null".into());
        out.open(format!(
            "BernBottomSheet(detents = {}, dragIndicator = {drag_indicator}, onDismiss = {on_dismiss}) {{",
            string(detents)
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
    }

    pub(super) fn refreshable(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let refreshing = element
            .expr("refreshing")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "false".into());
        let act = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_else(|| "Unit".into());
        out.open(format!(
            "BernRefreshable(target = {}, refreshing = {refreshing}, onRefresh = {{ {act} }}) {{",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    /// A picker; in a top bar (`toolbar`) the chosen option opening a menu.
    pub(super) fn picker_in(&self, out: &mut Out, element: &Element, ctx: &Ctx, toolbar: bool) {
        let prompt_arg = self
            .text_prop(element, "prompt")
            .map(|p| format!(", prompt = {p}"))
            .unwrap_or_default();
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
            let key_member = member(&def.fields[*key_idx].name);
            let label_def = &def.fields[*label_idx];
            let label_member = member(&label_def.name);
            let item_label_code = match &label_def.ty {
                Type::Text => format!("it.{label_member}"),
                Type::Int | Type::Float | Type::Enum { .. } => {
                    format!("BernText.text(it.{label_member})")
                }
                _ => format!("it.{label_member}.toString()"),
            };
            let items_code = self.expr(items_expr);
            let options = format!("({items_code}).map {{ it.{key_member} }}");
            let option_label = format!(
                "{{ key -> ({items_code}).firstOrNull {{ it.{key_member} == key }}?.let {{ {item_label_code} }} ?: \"\" }}"
            );
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
            let flow = match ctx.scope {
                Scope::Flow => ", inFlow = true",
                Scope::Row => ", inRow = true",
                _ => "",
            };
            out.line(format!(
                "BernPicker({label}, {options}, {value}, {option_label}, {{ {change} }}, {}, Modifier.testTag({target}), labelHidden = {}{}{flow}{prompt_arg})",
                string(style),
                element.keyword("label_hidden") == Some("true"),
                self.shape_arg(element)
            ));
            return;
        }
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
        // the flow; in a row it keeps its own width beside its neighbours.
        let flow = match ctx.scope {
            Scope::Flow => ", inFlow = true",
            Scope::Row => ", inRow = true",
            _ => "",
        };
        out.line(format!(
            "BernPicker({label}, {options}, {value}, {option_label}, {{ {change} }}, {}, Modifier.testTag({target}), labelHidden = {}{}{flow}{prompt_arg})",
            string(style),
            element.keyword("label_hidden") == Some("true"),
            self.shape_arg(element)
        ));
    }

    pub(super) fn language_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let hidden = if element.keyword("label_hidden") == Some("true") {
            ", labelHidden = true"
        } else {
            ""
        };
        let row = if ctx.scope == Scope::Row {
            ", inRow = true"
        } else {
            ""
        };
        out.line(format!(
            "BernLanguagePicker({label}, {}{hidden}{row})",
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

    pub(super) fn screen_brightness(&self, out: &mut Out, element: &Element) {
        let value = self.field(element, "value").expect("number field");
        let available = self.field(element, "available").expect("bool field");
        out.line(format!(
            "BernScreenBrightness({value}, {{ {value} = it }}, {{ {available} = it }})"
        ));
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
        let on_commit = element
            .children
            .iter()
            .find_map(|node| match node {
                Node::Element(child) if child.kind == "OnCommit" => {
                    child.action.as_ref().map(|action| self.action(action))
                }
                _ => None,
            })
            .unwrap_or_else(|| "Unit".into());
        let clear_active = self
            .field(element, "active")
            .map(|field| format!("{field} = false"))
            .unwrap_or_else(|| "Unit".into());
        let set_number = if element.kind == "Swipe" {
            format!(
                "val next = it.coerceIn({min}, {max}); bernCommitSwipe({{ {field} = next }}, {{ {action} }}, {{ {on_commit} }}, {{ {clear_active} }})"
            )
        } else {
            format!("{field} = it.coerceIn({min}, {max}); {action}")
        };
        out.line(format!(
            "BernControl({}, {}, {label}, actions = BernControlActions(setNumber = {{ {set_number} }}, value = {{ {field}.toString() }}))",
            self.target(element, ctx),
            string(&element.kind.to_lowercase())
        ));
        let active = self
            .field(element, "active")
            .map(|field| format!("{{ {field} = it }}"))
            .unwrap_or_else(|| "{}".into());
        if element.kind == "Pinch" {
            out.open(format!(
                "BernPinch({label}, {field}, {{ {field} = it }}, {min}, {max}, {{ {action} }}) {{"
            ));
        } else {
            out.open(format!(
                "BernSwipe({label}, {field}, {{ {field} = it }}, {min}, {max}, {}, {}, {{ {action} }}, continuous = {}, onActive = {active}, onCommit = {{ {on_commit} }}) {{",
                element.keyword("axis") == Some("horizontal"),
                double(element.number("distance").unwrap_or(300.0)),
                element.keyword("continuous") == Some("true")
            ));
        }
        let content: Vec<_> = element
            .children
            .iter()
            .filter(|node| !matches!(node, Node::Element(child) if child.kind == "OnCommit"))
            .cloned()
            .collect();
        self.nodes(out, &content, &ctx.within(Scope::Other));
        out.close("}");
    }
}

/// Whether content stretches across the width it is given: a row with a
/// spacer, something that fills the width, or a column or stack holding
/// such content, also through conditions.
pub(super) fn widens(nodes: &[Node]) -> bool {
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
