//! Controls: buttons, toggles, pickers, sliders, text input, file
//! drops, gestures and timers.

use super::values::swift_type;
use super::{Ctx, Generator, Out, Platform};
use crate::names::{double, string};
use bern_ir::{Element, Node};

impl Generator<'_> {
    /// `.keyboardShortcut(…)` arguments for an element's `shortcut`; `cmd`
    /// is Command on Apple systems.
    pub(super) fn shortcut(&self, element: &Element) -> Option<String> {
        let text = element.keyword("shortcut")?;
        let mut parts: Vec<&str> = text.split('+').collect();
        let key = parts.pop()?;
        let key = match key {
            "space" => ".space".to_string(),
            "escape" => ".escape".into(),
            "return" => ".return".into(),
            "tab" => ".tab".into(),
            "delete" => ".delete".into(),
            "up" => ".upArrow".into(),
            "down" => ".downArrow".into(),
            "left" => ".leftArrow".into(),
            "right" => ".rightArrow".into(),
            "home" => ".home".into(),
            "end" => ".end".into(),
            "pageup" => ".pageUp".into(),
            "pagedown" => ".pageDown".into(),
            other => format!("SwiftUI.KeyEquivalent(Character({}))", string(other)),
        };
        let modifiers: Vec<&str> = parts
            .iter()
            .map(|modifier| match *modifier {
                "cmd" => ".command",
                "ctrl" => ".control",
                "alt" => ".option",
                _ => ".shift",
            })
            .collect();
        Some(format!(
            "SwiftUI.KeyboardShortcut({key}, modifiers: [{}])",
            modifiers.join(", ")
        ))
    }

    pub(super) fn button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (action, file) = self.pressed(element);
        let role = match element.keyword("role") {
            Some("destructive") => "role: .destructive, ",
            Some("cancel") => "role: .cancel, ",
            _ => "",
        };
        out.open(format!("SwiftUI.Button({role}action: {{"));
        out.line(&action);
        out.close("}) {");
        out.indent += 1;
        let enabled = element.expr("enabled").map(|expr| self.expr(expr));
        if element.kind == "Button" && !element.children.is_empty() {
            // The content is what the button shows; the label names it.
            self.nodes(out, &element.children, ctx);
            if element.keyword("width") == Some("fill") {
                out.line(".frame(maxWidth: .infinity)");
            }
            out.close("}");
            let style = self.style_of(element);
            match &style {
                // A capsule is the platform's button around the content,
                // as it is around a label.
                Some(style)
                    if element.keyword("shape") == Some("capsule") && !style.draws_look() =>
                {
                    self.capsule_control(out, element);
                    out.line(format!(".bernFeedback(BernStyles.{})", style.name));
                }
                None if element.keyword("shape") == Some("capsule") => {
                    self.capsule_control(out, element);
                }
                _ => out.line(format!(
                    ".buttonStyle(BernContentButtonStyle(style: {}))",
                    style
                        .map(|style| format!("BernStyles.{}", style.name))
                        .unwrap_or_else(|| "nil".into())
                )),
            }
            out.line(format!(
                ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
            ));
            if let Some(shortcut) = self.shortcut(element) {
                out.line(format!(".keyboardShortcut({shortcut})"));
            }
            if let Some(enabled) = &enabled {
                out.line(format!(".disabled(!({enabled}))"));
            }
            out.line(format!(
                ".bernControl({}, role: \"button\", label: {label}, enabled: {}, actions: BernControlActions(activate: {{ {action} }}{file}))",
                self.target(element, ctx),
                enabled.unwrap_or_else(|| "true".into())
            ));
            return;
        }
        let icon = element.icon("icon");
        match (element.kind.as_str(), icon) {
            ("IconButton", Some(icon)) => {
                out.line(format!(
                    "SwiftUI.Label({label}, systemImage: {})",
                    string(icon)
                ));
                out.line(".labelStyle(.iconOnly)");
                if let Some(size) = element.number("font_size") {
                    out.line(format!(".font(.system(size: {}))", double(size)));
                }
            }
            // The design's capsule (6.3): the symbol in a 16-point box,
            // 14 on a 30-point chip, 6 before the words.
            (_, Some(icon)) if element.keyword("shape") == Some("capsule") && !ctx.toolbar => {
                let chip = element
                    .number("height")
                    .is_some_and(|height| height <= 30.0);
                out.open("SwiftUI.HStack(spacing: 6) {");
                out.line(format!(
                    "SwiftUI.Image(systemName: {}).frame(width: {box}, height: {box}).accessibilityHidden(true)",
                    string(icon),
                    box = if chip { "14" } else { "16" }
                ));
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
            }
            (_, Some(icon)) => {
                out.line(format!(
                    "SwiftUI.Label({label}, systemImage: {})",
                    string(icon)
                ));
                // A toolbar shows only the icon of a label; a `Button`
                // shows its words too, unlike an `IconButton`.
                if ctx.toolbar {
                    out.line(".labelStyle(.titleAndIcon)");
                }
            }
            _ => out.line(format!("SwiftUI.Text(verbatim: {label})")),
        }
        // The system button grows with its label.
        if element.keyword("width") == Some("fill") {
            out.line(".frame(maxWidth: .infinity)");
        }
        out.close("}");
        let style = self.style_of(element);
        match &style {
            // The style draws the button; its transforms follow the
            // button's own pressed state.
            Some(style) if style.draws_look() => {
                out.line(format!(
                    ".buttonStyle(BernButtonStyle(style: BernStyles.{}))",
                    style.name
                ));
                if element.kind == "IconButton" {
                    out.line(format!(".help({label})"));
                }
            }
            _ => {
                // The platform's buttons (the design's chapter on controls):
                // a toolbar draws its own.
                if element.kind == "IconButton" {
                    if let Some(diameter) = self.length(element, "height").filter(|_| !ctx.toolbar)
                    {
                        out.line(format!(
                            ".bernCircleControl(diameter: {diameter}, prominent: {})",
                            element.keyword("prominence") == Some("prominent")
                        ));
                    } else if !ctx.toolbar {
                        match element.keyword("prominence") {
                            Some("plain") => out.line(".bernButtonLook(.flat)"),
                            Some("prominent") => out.line(".bernButtonLook(.prominent).buttonBorderShape(.roundedRectangle(radius: 12))"),
                            _ => out.line(".bernButtonLook(.icon)"),
                        }
                    }
                    out.line(format!(".help({label})"));
                } else if !ctx.toolbar {
                    if element.keyword("shape") == Some("capsule") {
                        self.capsule_control(out, element);
                    } else {
                        match element.keyword("prominence") {
                            Some("prominent") => out.line(".bernButtonLook(.prominent)"),
                            // Words without a border, in the secondary color.
                            Some("plain") => out.line(".bernButtonLook(.flat)"),
                            _ => out.line(".bernButtonLook(.secondary)"),
                        }
                    }
                }
                if let Some(style) = &style {
                    if let Some(size) = style.size() {
                        out.line(format!(
                            ".controlSize({})",
                            crate::style::control_size(size)
                        ));
                    }
                    out.line(format!(".bernFeedback(BernStyles.{})", style.name));
                }
            }
        }
        if let Some(shortcut) = self.shortcut(element) {
            out.line(format!(".keyboardShortcut({shortcut})"));
        }
        if let Some(enabled) = &enabled {
            out.line(format!(".disabled(!({enabled}))"));
        }
        out.line(format!(
            ".bernControl({}, role: \"button\", label: {label}, enabled: {}, actions: BernControlActions(activate: {{ {action} }}{file}))",
            self.target(element, ctx),
            enabled.unwrap_or_else(|| "true".into())
        ));
    }

    /// The platform's capsule button, `height` tall.
    fn capsule_control(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            ".bernCapsuleControl(height: {}, prominent: {})",
            self.length(element, "height")
                .unwrap_or_else(|| "32.0".into()),
            element.keyword("prominence") == Some("prominent")
        ));
    }

    pub(super) fn popover_button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let icon = element
            .icon("icon")
            .map(string)
            .unwrap_or_else(|| "nil".into());
        let presented = self
            .field(element, "presented")
            .map(|field| self.binding_of(&field))
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernPopoverButton(label: {label}, systemImage: {icon}, presented: {presented}, target: {}) {{",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        if !ctx.toolbar {
            out.line(if element.icon("icon").is_some() {
                ".bernButtonLook(.icon)"
            } else {
                ".bernButtonLook(.secondary)"
            });
        }
        if let Some(shortcut) = self.shortcut(element) {
            out.line(format!(".keyboardShortcut({shortcut})"));
        }
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("bool field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let more = self
            .text_prop(element, "more_label")
            .unwrap_or_else(|| "BernText.string(\"bern.split.more\")".into());
        let icon = element
            .icon("icon")
            .map(string)
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernSplitButton(label: {label}, systemImage: {icon}, more: {more}, isOn: {}, capsule: {}, height: {}, target: {}) {{",
            self.binding_of(&field),
            element.keyword("shape") == Some("capsule"),
            self.length(element, "height").unwrap_or_else(|| "34.0".into()),
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        if let Some(shortcut) = self.shortcut(element) {
            out.line(format!(".keyboardShortcut({shortcut})"));
        }
    }

    pub(super) fn control_group(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernControlGroup(height: {}) {{",
            self.length(element, "height")
                .unwrap_or_else(|| "40.0".into())
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        if let Some(label) = self.text_prop(element, "label") {
            out.line(format!(
                ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
            ));
        }
    }

    pub(super) fn text_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("text field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .map(|prompt| format!("SwiftUI.Text(verbatim: {prompt})"))
            .unwrap_or_else(|| "nil".into());
        let binding = self.binding_of(&field);
        let kind = element.keyword("kind").unwrap_or("plain");
        // An iOS form shows a text field's prompt, not its label: the label
        // goes beside a single line, as in Settings, and above several.
        let ios = self.platform == Platform::Ios && element.keyword("label_hidden") != Some("true");
        if ios {
            if kind == "multiline" {
                out.open("SwiftUI.VStack(alignment: .leading, spacing: 6) {");
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
            } else {
                out.open("SwiftUI.LabeledContent {");
            }
        }
        match kind {
            "secure" => out.open(format!(
                "SwiftUI.SecureField(text: {binding}, prompt: {prompt}) {{"
            )),
            "multiline" => out.open(format!(
                "SwiftUI.TextField(text: {binding}, prompt: {prompt}, axis: .vertical) {{"
            )),
            _ => out.open(format!(
                "SwiftUI.TextField(text: {binding}, prompt: {prompt}) {{"
            )),
        }
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        out.close("}");
        if kind == "multiline" {
            out.line(".lineLimit(3...8)");
        }
        if ios {
            if kind == "multiline" {
                out.close("}");
            } else {
                out.line(".multilineTextAlignment(.trailing)");
                out.close("} label: {");
                out.indent += 1;
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
            }
        }
        let action = element.action.as_ref().map(|a| self.action(a));
        if let Some(action) = &action {
            out.line(format!(".onSubmit {{ {action} }}"));
        }
        let submit = action
            .map(|action| format!(", submit: {{ {action} }}"))
            .unwrap_or_default();
        // Secure text is never exposed to external entries.
        let value = if kind == "secure" {
            String::new()
        } else {
            format!(", value: {{ {field} }}")
        };
        out.line(format!(
            ".bernControl({}, role: \"text_input\", label: {label}, actions: BernControlActions(edit: {{ {field} = $0 }}{submit}{value}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (field, binding, setter) = self.control_value(element, "checked");
        let label = self.text_prop(element, "label").unwrap_or_default();
        out.open(format!("SwiftUI.Toggle(isOn: {binding}) {{"));
        match element.icon("icon") {
            Some(icon) => out.line(format!(
                "SwiftUI.Label({label}, systemImage: {})",
                string(icon)
            )),
            None => out.line(format!("SwiftUI.Text(verbatim: {label})")),
        }
        out.close("}");
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
        }
        // iOS has no checkboxes; its toggles are switches.
        out.line(".bernAccentTint()");
        match (element.keyword("style"), self.platform) {
            (Some("switch"), _) => out.line(".toggleStyle(.switch)"),
            (Some("checkbox"), Platform::Macos) => out.line(".toggleStyle(.checkbox)"),
            (Some("button"), _) => {
                if element.keyword("shape") == Some("capsule") {
                    out.line(format!(
                        ".toggleStyle(BernCapsuleToggleStyle(height: {}))",
                        self.length(element, "height")
                            .unwrap_or_else(|| "30.0".into())
                    ));
                } else {
                    out.line(".toggleStyle(.button)");
                }
                out.line(format!(".help({label})"));
            }
            _ => {}
        }
        if let Some(shortcut) = self.shortcut(element) {
            out.line(format!(".keyboardShortcut({shortcut})"));
        }
        out.line(format!(
            ".bernControl({}, role: \"switch\", label: {label}, actions: BernControlActions(setBool: {setter}, value: {{ {field} ? \"true\" : \"false\" }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn slider(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        if element.expr("current").is_some() {
            let (value, _, commit) = self.control_value(element, "current");
            let label = self.text_prop(element, "label").unwrap_or_default();
            out.line(format!("BernControlledSlider(value: {value}, minimum: {}, maximum: {}, step: {}, label: {label}, labelHidden: {}, commit: {commit})", double(element.number("min").unwrap_or(0.0)), double(element.number("max").unwrap_or(1.0)), element.number("step").map(double).unwrap_or_else(|| "nil".into()), element.keyword("label_hidden") == Some("true")));
            out.line(format!(".bernControl({}, role: \"slider\", label: {label}, actions: BernControlActions(setNumber: {commit}, value: {{ BernText.text({value}) }}))", self.target(element, ctx)));
            return;
        }
        let field = self.field(element, "value").expect("number field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1.0));
        let step = element
            .number("step")
            .map(|step| format!(", step: {}", double(step)))
            .unwrap_or_default();
        let action = element.action.as_ref().map(|a| self.action(a));
        out.open(format!(
            "SwiftUI.Slider(value: {}, in: {min}...{max}{step}) {{",
            self.binding_of(&field)
        ));
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        let (min_icon, max_icon) = (element.icon("min_icon"), element.icon("max_icon"));
        if min_icon.is_some() || max_icon.is_some() {
            out.close("} minimumValueLabel: {");
            out.indent += 1;
            match min_icon {
                Some(icon) => out.line(format!("SwiftUI.Image(systemName: {})", string(icon))),
                None => out.line("SwiftUI.EmptyView()"),
            }
            out.close("} maximumValueLabel: {");
            out.indent += 1;
            match max_icon {
                Some(icon) => out.line(format!("SwiftUI.Image(systemName: {})", string(icon))),
                None => out.line("SwiftUI.EmptyView()"),
            }
        }
        out.close("} onEditingChanged: { editing in");
        out.indent += 1;
        match &action {
            Some(action) => out.line(format!("if !editing {{ {action} }}")),
            None => out.line("_ = editing"),
        }
        out.close("}");
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
        }
        out.line(".bernAccentTint()");
        let commit = action
            .map(|action| format!("; {action}"))
            .unwrap_or_default();
        out.line(format!(
            ".bernControl({}, role: \"slider\", label: {label}, actions: BernControlActions(setNumber: {{ value in {field} = value{commit} }}, value: {{ BernText.text({field}) }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let ty = element
            .expr("selected")
            .map(|e| &e.ty)
            .unwrap_or_else(|| &element.field("value").expect("picker field").ty);
        let (field, binding, setter) = self.control_value(element, "selected");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let enum_type = swift_type(ty);
        let capsule = element.keyword("shape") == Some("capsule");
        if capsule {
            out.open("SwiftUI.Menu {");
        }
        out.open(format!("SwiftUI.Picker(selection: {binding}) {{"));
        if element.children.is_empty() {
            out.open(format!(
                "SwiftUI.ForEach({enum_type}.allCases, id: \\.self) {{ option in"
            ));
            out.line("SwiftUI.Text(verbatim: option.bernLabel).tag(option)");
            out.close("}");
        } else {
            for node in &element.children {
                if let Node::Element(option) = node {
                    let label = self.text_prop(option, "label").unwrap_or_default();
                    let value = self.expr(option.expr("value").expect("picker option value"));
                    out.line(format!("SwiftUI.Text(verbatim: {label}).tag({value})"));
                }
            }
        }
        out.close("} label: {");
        out.indent += 1;
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        out.close("}");
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
        }
        match element.keyword("style") {
            Some("menu") if capsule => out.line(".pickerStyle(.inline)"),
            Some("menu") => out.line(".pickerStyle(.menu)"),
            Some("segmented") => out.line(".pickerStyle(.segmented).bernAccentTint()"),
            // iOS lists the options with a checkmark instead of
            // radio buttons.
            Some("radio") if self.platform == Platform::Ios => out.line(".pickerStyle(.inline)"),
            Some("radio") => out.line(".pickerStyle(.radioGroup)"),
            Some("inline") => out.line(".pickerStyle(.inline)"),
            _ => {}
        }
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
        }
        if capsule {
            out.close("} label: {");
            out.indent += 1;
            // iOS draws no arrow on the menu's button; the capsule
            // shows the design's pull-down arrow, as a capsule `Menu`
            // does.
            let arrow = self.platform == Platform::Ios;
            if arrow {
                out.open("SwiftUI.HStack(spacing: 6) {");
            }
            if element.children.is_empty() {
                out.line(format!("SwiftUI.Text(verbatim: {field}.bernLabel)"));
            } else {
                out.open(format!("switch {field} {{"));
                for node in &element.children {
                    if let Node::Element(option) = node {
                        let text = self.text_prop(option, "label").unwrap_or_default();
                        let value = self.expr(option.expr("value").expect("picker option value"));
                        out.line(format!("case {value}: SwiftUI.Text(verbatim: {text})"));
                    }
                }
                out.line(format!(
                    "default: SwiftUI.Text(verbatim: {field}.bernLabel)"
                ));
                out.close("}");
            }
            if arrow {
                out.line("SwiftUI.Image(systemName: \"chevron.down\").font(.system(size: 10, weight: .semibold)).frame(width: 12, height: 12).opacity(0.6)");
                out.close("}");
                out.line(".padding(.trailing, -3)");
            }
            out.close("}");
            if self.platform == Platform::Macos {
                out.line(".menuStyle(.borderlessButton)");
            }
            out.line(format!(
                ".bernCapsuleControl(height: {})",
                self.length(element, "height")
                    .unwrap_or_else(|| "32.0".into())
            ));
            out.line(format!(
                ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
            ));
        }
        out.line(format!(
            ".bernControl({}, role: \"picker\", label: {label}, actions: BernControlActions(select: {{ name in guard let value = {enum_type}(bernName: name) else {{ return false }}; ({setter})(value); return true }}, value: {{ {field}.bernName }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn language_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        out.open(
            "SwiftUI.Picker(selection: SwiftUI.Binding(get: { BernLanguage.shared.current }, set: { BernLanguage.shared.select($0) })) {",
        );
        out.open("SwiftUI.ForEach(BernLanguage.shared.supported, id: \\.self) { language in");
        out.line("SwiftUI.Text(verbatim: BernLanguage.autonym(language)).tag(language)");
        out.close("}");
        out.close("} label: {");
        out.indent += 1;
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        out.close("}");
        out.line(format!(
            ".bernControl({}, role: \"picker\", label: {label}, actions: BernControlActions(select: {{ BernLanguage.shared.select($0) }}, value: {{ BernLanguage.shared.current }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn drop_zone(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_default();
        let (give, actions) = self
            .file_target(element, "value", &action)
            .expect("file field");
        let over = self
            .field(element, "over")
            .map(|field| self.binding_of(&field))
            .unwrap_or_else(|| "nil".into());
        out.open("SwiftUI.Group {");
        self.nodes(out, &element.children, ctx);
        out.close("}");
        out.line(format!(
            ".modifier(BernDrop(kinds: {}, over: {over}, opens: {}, give: {give}))",
            string(element.keyword("files").unwrap_or("any")),
            element.keyword("opens") == Some("true")
        ));
        out.line(format!(
            ".bernControl({}, role: \"drop\", label: {label}, actions: BernControlActions({actions}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn gesture(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("number field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1.0));
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_default();
        out.open("SwiftUI.Group {");
        self.nodes(out, &element.children, ctx);
        out.close("}");
        if element.kind == "Pinch" {
            out.line(format!(
                ".bernPinch({}, in: {min}...{max}, commit: {{ {action} }})",
                self.binding_of(&field)
            ));
        } else {
            let axis = match element.keyword("axis") {
                Some("horizontal") => ".horizontal",
                _ => ".vertical",
            };
            out.line(format!(
                ".bernSwipe({}, in: {min}...{max}, axis: {axis}, distance: {}, commit: {{ {action} }})",
                self.binding_of(&field),
                double(element.number("distance").unwrap_or(300.0))
            ));
        }
        out.line(format!(
            ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
        ));
        out.line(format!(
            ".bernControl({}, role: {}, label: {label}, actions: BernControlActions(setNumber: {{ value in {field} = min(max(value, {min}), {max}); {action} }}, value: {{ BernText.text({field}) }}))",
            self.target(element, ctx),
            string(&element.kind.to_lowercase())
        ));
    }

    pub(super) fn timer(&self, out: &mut Out, element: &Element) {
        let enabled = element
            .expr("enabled")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        let action = self.action(element.action.as_ref().expect("timer action"));
        out.line(format!(
            "BernTimer(interval: {}, enabled: {enabled}) {{ {action} }}",
            double(element.number("interval").unwrap_or(1.0))
        ));
    }
}
