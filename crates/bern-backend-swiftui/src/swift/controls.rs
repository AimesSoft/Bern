//! Controls: buttons, toggles, pickers, sliders, text input, file
//! drops, gestures and timers.

use super::values::swift_type;
use super::{Ctx, Generator, Out, Platform};
use crate::names::{double, string};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    /// `.keyboardShortcut(…)` arguments for an element's `shortcut`; `cmd`
    /// is Command on Apple systems. None on tvOS, which has no keyboard
    /// shortcuts; see `shortcut_line`.
    pub(super) fn shortcut(&self, element: &Element) -> Option<String> {
        if self.platform == Platform::Tvos {
            return None;
        }
        self.keyboard(element, "shortcut")
    }

    /// The `KeyboardShortcut` of the shortcut prop `name`: a literal's, or
    /// for a text the shortcut follows an optional one read as the app
    /// runs, nil while the text is no shortcut.
    fn keyboard(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Keyword { value } => keyboard_shortcut(value),
            Prop::Expr { expr } => Some(format!("BernShortcuts.keyboard({})", self.expr(expr))),
            _ => None,
        }
    }

    /// The Siri Remote's button for the shortcut prop `name` (see
    /// `remote_key`); for a text the shortcut follows, an optional one.
    fn remote(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Keyword { value } => remote_key(value).map(String::from),
            Prop::Expr { expr } => Some(format!("BernShortcuts.remote({})", self.expr(expr))),
            _ => None,
        }
    }

    /// The modifier that gives a control its `shortcut`: the keyboard
    /// shortcut, or on tvOS the Siri Remote's button that stands for the
    /// key (see `remote_key`), which runs `action` while the control is on
    /// screen and enabled. Keys the remote has no button for, and controls
    /// whose action is not a press, give nothing there.
    pub(super) fn shortcut_line(&self, element: &Element, action: Option<&str>) -> Option<String> {
        if self.platform != Platform::Tvos {
            return self
                .shortcut(element)
                .map(|shortcut| format!(".keyboardShortcut({shortcut})"));
        }
        let key = self.remote(element, "shortcut")?;
        let enabled = element
            .expr("enabled")
            .map(|enabled| self.expr(enabled))
            .unwrap_or_else(|| "true".into());
        Some(format!(
            ".bernRemoteShortcut({key}, enabled: {enabled}) {{ {} }}",
            action?
        ))
    }

    /// `Shortcut`: its key runs the action while the shortcut is on screen
    /// and enabled, without a control; on tvOS the remote's button that
    /// stands for the key, where it has one.
    pub(super) fn key_shortcut(&self, out: &mut Out, element: &Element) {
        let action = element
            .action
            .as_ref()
            .map(|action| self.action(action))
            .unwrap_or_default();
        let enabled = element
            .expr("enabled")
            .map(|enabled| self.expr(enabled))
            .unwrap_or_else(|| "true".into());
        let source = if self.platform == Platform::Tvos {
            self.remote(element, "key")
                .map(|remote| format!("remote: {remote}"))
        } else {
            self.keyboard(element, "key")
                .map(|shortcut| format!("shortcut: {shortcut}"))
        };
        if let Some(source) = source {
            out.line(format!(
                "BernKeyShortcut({source}, enabled: {enabled}) {{ {action} }}"
            ));
        }
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
            if let Some(line) = self.shortcut_line(element, Some(&action)) {
                out.line(line);
            }
            if element.keyword("default_focus") == Some("true") {
                out.line(".bernDefaultFocus()");
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
        if let Some(line) = self.shortcut_line(element, Some(&action)) {
            out.line(line);
        }
        if element.keyword("default_focus") == Some("true") {
            out.line(".bernDefaultFocus()");
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
        let presented = if element.expr("open").is_some() {
            self.control_value(element, "open").1
        } else {
            self.field(element, "presented")
                .map(|field| self.binding_of(&field))
                .unwrap_or_else(|| "nil".into())
        };
        let window_theme =
            bern_target_api::style::popover_window_theme(self.app, self.style_platform(), element);
        let style = self
            .style_of(element)
            .map(|style| format!("BernStyles.{}", style.name))
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernPopoverButton(label: {label}, systemImage: {icon}, presented: {presented}, target: {}, windowTheme: {window_theme}, style: {style}) {{",
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
        if let Some(line) = self.shortcut_line(element, None) {
            out.line(line);
        }
    }

    pub(super) fn split_button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (_, binding, _) = self.control_value(element, "checked");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let more = self
            .text_prop(element, "more_label")
            .unwrap_or_else(|| "BernText.string(\"bern.split.more\")".into());
        let icon = element
            .icon("icon")
            .map(string)
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernSplitButton(label: {label}, systemImage: {icon}, more: {more}, isOn: {binding}, capsule: {}, height: {}, target: {}) {{",
            element.keyword("shape") == Some("capsule"),
            self.length(element, "height").unwrap_or_else(|| "34.0".into()),
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        if let Some(line) = self.shortcut_line(element, None) {
            out.line(line);
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
        let ios = self.platform.uikit() && element.keyword("label_hidden") != Some("true");
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
        if self.platform == Platform::Tvos {
            out.line(".bernFieldLook()");
        }
        // `label_hidden`: the label names the field without showing, also
        // in a macOS form.
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
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
        let action = element.action.as_ref().map(|a| match element.clearing() {
            // The draft empties and the keyboard goes back to the window.
            Some(clear) => format!(
                "{}; {}; bernEndEditing()",
                self.action(a),
                self.action(&clear)
            ),
            None => self.action(a),
        });
        if let Some(action) = &action {
            out.line(format!(".onSubmit {{ {action} }}"));
        }
        if self.platform == Platform::Macos {
            out.line(".modifier(BernComposerFocus())");
        }
        if let Some(key) = element.expr("clear_key") {
            out.line(format!(
                ".onChange(of: {}) {{ {field} = \"\"; bernEndEditing() }}",
                self.expr(key)
            ));
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
            // tvOS has no switch, only a button that says On or Off: the
            // switch of the other platforms, drawn, which the remote's
            // select flips.
            (Some("switch") | Some("checkbox") | None, Platform::Tvos) => {
                out.line(".toggleStyle(BernFocusSwitchStyle())")
            }
            (Some("switch"), _) => out.line(".toggleStyle(.switch)"),
            (Some("checkbox"), Platform::Macos) => out.line(".toggleStyle(.checkbox)"),
            (Some("button"), _) => {
                if element.keyword("shape") == Some("capsule") {
                    out.line(format!(
                        ".toggleStyle(BernCapsuleToggleStyle(height: {}))",
                        self.length(element, "height")
                            .unwrap_or_else(|| "30.0".into())
                    ));
                } else if self.platform != Platform::Tvos {
                    // tvOS's toggles are buttons already.
                    out.line(".toggleStyle(.button)");
                }
                out.line(format!(".help({label})"));
            }
            _ => {}
        }
        if let Some(line) = self.shortcut_line(element, None) {
            out.line(line);
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
            out.line(format!("BernControlledSlider(value: {value}, minimum: {}, maximum: {}, step: {}, label: {label}, labelHidden: {}, continuous: {}, commit: {commit})", double(element.number("min").unwrap_or(0.0)), double(element.number("max").unwrap_or(1.0)), element.number("step").map(double).unwrap_or_else(|| "nil".into()), element.keyword("label_hidden") == Some("true"), element.keyword("continuous") == Some("true")));
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
        // tvOS has no slider: the remote's left and right move a focused
        // track by its step.
        if self.platform == Platform::Tvos {
            let icon = |name: &str| {
                element
                    .icon(name)
                    .map(string)
                    .unwrap_or_else(|| "nil".into())
            };
            out.open(format!(
                "BernFocusSlider(value: {}, minimum: {min}, maximum: {max}, step: {}, label: {label}, minimumIcon: {}, maximumIcon: {}, labelHidden: {}) {{ editing in",
                self.binding_of(&field),
                element.number("step").map(double).unwrap_or_else(|| "nil".into()),
                icon("min_icon"),
                icon("max_icon"),
                element.keyword("label_hidden") == Some("true")
            ));
            match &action {
                Some(action) => out.line(format!("if !editing {{ {action} }}")),
                None => out.line("_ = editing"),
            }
            out.close("}");
            let commit = action
                .map(|action| format!("; {action}"))
                .unwrap_or_default();
            out.line(format!(
                ".bernControl({}, role: \"slider\", label: {label}, actions: BernControlActions(setNumber: {{ value in {field} = value{commit} }}, value: {{ BernText.text({field}) }}))",
                self.target(element, ctx)
            ));
            return;
        }
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
        // The options as `(label, value)` pairs, for the pickers Bern lays
        // out itself.
        let pairs = || {
            if element.children.is_empty() {
                format!("{enum_type}.allCases.map {{ ($0.bernLabel, $0) }}")
            } else {
                let pairs: Vec<String> = element
                    .children
                    .iter()
                    .filter_map(|node| match node {
                        Node::Element(option) => Some(format!(
                            "({}, {})",
                            self.text_prop(option, "label").unwrap_or_default(),
                            self.expr(option.expr("value").expect("picker option value"))
                        )),
                        _ => None,
                    })
                    .collect();
                format!("[{}]", pairs.join(", "))
            }
        };
        let actions = format!(
            "BernControlActions(select: {{ name in guard let value = {enum_type}(bernName: name) else {{ return false }}; ({setter})(value); return true }}, value: {{ {field}.bernName }})"
        );
        // A chip per option; in a flow the chips are the flow's items, so
        // the picker has no view of its own to carry its label.
        if element.keyword("style") == Some("chips") {
            out.line(format!(
                "BernChips(selection: {binding}, options: {}, flat: {}, target: {}, label: {label}, actions: {actions})",
                pairs(),
                ctx.flow && element.keyword("label_hidden") == Some("true"),
                self.target(element, ctx)
            ));
            return;
        }
        // tvOS's segmented control takes a television's size, too large in
        // a scene laid out at the design's: `BernSegments` draws it there.
        if self.platform == Platform::Tvos && element.keyword("style") == Some("segmented") {
            let options = pairs();
            out.line(format!(
                "BernSegments(selection: {binding}, options: {options}).bernAccentTint()"
            ));
            out.line(format!(
                ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
            ));
            out.line(format!(
                ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
                self.target(element, ctx)
            ));
            return;
        }
        // tvOS's menu picker shows its value alone: there the row says what
        // it chooses, and the choice is a capsule menu at its end, as a
        // form's row reads on the other platforms.
        let row = self.platform == Platform::Tvos
            && element.keyword("style") == Some("menu")
            && element.keyword("shape") != Some("capsule")
            && element.keyword("label_hidden") != Some("true");
        let capsule = row || element.keyword("shape") == Some("capsule");
        if row {
            out.open("SwiftUI.HStack(spacing: 12) {");
            out.line(format!("SwiftUI.Text(verbatim: {label})"));
            out.line("SwiftUI.Spacer(minLength: 12)");
        }
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
            Some("radio") if self.platform.uikit() => out.line(".pickerStyle(.inline)"),
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
            let arrow = self.platform.uikit();
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
                self.length(element, "height").unwrap_or_else(|| if row {
                    "46.0".into()
                } else {
                    "32.0".into()
                })
            ));
            out.line(format!(
                ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
            ));
        }
        if row {
            out.close("}");
        }
        out.line(format!(
            ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
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

    /// `ShortcutInput`: a button showing the shortcut that takes the next
    /// key pressed; the action keeps the one chosen. External entries set
    /// it with `ui.edit` and a shortcut in Bern's form.
    pub(super) fn shortcut_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (value, _, commit) = self.control_value(element, "current");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "BernText.string(\"bern.shortcut.prompt\")".into());
        let empty = self
            .text_prop(element, "empty")
            .unwrap_or_else(|| "BernText.string(\"bern.shortcut.none\")".into());
        let enabled = element.expr("enabled").map(|expr| self.expr(expr));
        out.line(format!(
            "BernShortcutInput(current: {value}, label: {label}, labelHidden: {}, prompt: {prompt}, empty: {empty}, commit: {commit})",
            element.keyword("label_hidden") == Some("true")
        ));
        if let Some(enabled) = &enabled {
            out.line(format!(".disabled(!({enabled}))"));
        }
        out.line(format!(
            ".bernControl({}, role: \"shortcut_input\", label: {label}, enabled: {}, actions: BernControlActions(edit: {{ text in if let keys = BernShortcuts.normalized(text) {{ ({commit})(keys) }} }}, value: {{ {value} }}))",
            self.target(element, ctx),
            enabled.unwrap_or_else(|| "true".into())
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

/// `SwiftUI.KeyboardShortcut(…)` for a shortcut literal such as `cmd+o`.
fn keyboard_shortcut(text: &str) -> Option<String> {
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

/// The Siri Remote's button that stands for a key on a television: Back
/// for `escape`, Play/Pause for `space`, and the clickpad's edges for the
/// arrows, when the press moves the focus nowhere. Other keys, and keys
/// with modifiers, have none.
fn remote_key(text: &str) -> Option<&'static str> {
    Some(match text {
        "escape" => ".back",
        "space" => ".playPause",
        "left" => ".left",
        "right" => ".right",
        "up" => ".up",
        "down" => ".down",
        _ => return None,
    })
}
