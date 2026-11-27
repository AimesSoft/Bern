//! Controls: buttons, toggles, pickers, sliders, text input, file
//! drops, gestures and timers.

use super::values::swift_type;
use super::{Ctx, Generator, Out, Platform};
use crate::names::{double, member, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// `.keyboardShortcut(…)` arguments for an element's `shortcut`; `cmd`
    /// is Command on Apple systems. None on tvOS, which has no keyboard
    /// shortcuts; see `shortcut_line`.
    pub(super) fn shortcut(&self, element: &Element) -> Option<String> {
        if matches!(self.platform, Platform::Tvos | Platform::Watchos) {
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
        if self.platform == Platform::Watchos {
            return None;
        }
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
        if self.platform == Platform::Watchos {
            return;
        }
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
        let quiet = if element.keyword("quiet") == Some("true") {
            ", quiet: true"
        } else {
            ""
        };
        if let Some(hold) = self.field(element, "hold") {
            if !matches!(self.platform, Platform::Tvos | Platform::Watchos)
                && let Some(shortcut) = self.keyboard(element, "key")
            {
                let after = double(element.number("hold_after").unwrap_or(0.3));
                out.line(format!(
                    "BernHoldKeyShortcut(shortcut: {shortcut}, enabled: {enabled}{quiet}, hold: {}, holdAfter: {after}) {{ {action} }}",
                    self.binding_of(&hold)
                ));
            }
            return;
        }
        if let Some(source) = source {
            out.line(format!(
                "BernKeyShortcut({source}, enabled: {enabled}{quiet}) {{ {action} }}"
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
        if element.kind == "Button" && element.has_content() {
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
                    if element.keyword("shape") == Some("capsule") && !style.draws_base_look() =>
                {
                    self.capsule_control(out, element, Some(&style.name));
                    if !style.draws_look() {
                        out.line(format!(".bernFeedback(BernStyles.{})", style.name));
                    }
                }
                None if element.keyword("shape") == Some("capsule") => {
                    self.capsule_control(out, element, None);
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
            if let Some(cursor) = element.keyword("cursor") {
                out.line(format!(
                    ".bernCursor({}, enabled: {})",
                    string(cursor),
                    enabled.as_deref().unwrap_or("true")
                ));
            }
            out.line(format!(
                ".bernControl({}, role: \"button\", label: {label}, enabled: {}, actions: BernControlActions(activate: {{ {action} }}{file}))",
                self.target(element, ctx),
                enabled.unwrap_or_else(|| "true".into())
            ));
            self.double_tap(out, element, &action);
            return;
        }
        let icon = element.icon("icon");
        match (element.kind.as_str(), icon) {
            ("IconButton", Some(icon)) => {
                if element.prop("icon_progress").is_some() || element.icon("badge_icon").is_some() {
                    let progress = element
                        .expr("icon_progress")
                        .map(|value| format!("Double({})", self.expr(value)))
                        .unwrap_or_else(|| "nil".into());
                    let badge = element
                        .icon("badge_icon")
                        .map(string)
                        .unwrap_or_else(|| "nil".into());
                    let visible = element
                        .expr("badge_visible")
                        .map(|value| self.expr(value))
                        .unwrap_or_else(|| "true".into());
                    out.line(format!("BernIconGlyph(name: {}, progress: {progress}, badge: {badge}, badgeVisible: {visible})", string(icon)));
                    out.line(format!(".accessibilityElement(children: .ignore).accessibilityLabel(SwiftUI.Text(verbatim: {label}))"));
                } else if Self::uses_custom_icon(icon) {
                    let size = element
                        .number("font_size")
                        .map(|s| format!(", size: {}", double(s)))
                        .unwrap_or_default();
                    out.line(format!("BernIcon(name: {}{size})", string(icon)));
                    out.line(format!(
                        ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
                    ));
                } else {
                    out.line(format!(
                        "SwiftUI.Label({label}, systemImage: {})",
                        string(icon)
                    ));
                    out.line(".labelStyle(.iconOnly)");
                }
                if let Some(size) = element.number("font_size") {
                    out.line(format!(".font(.system(size: {}))", double(size)));
                }
                if let Some(role) = element.keyword("color") {
                    out.line(format!(
                        ".foregroundStyle(BernColor.role({}).color)",
                        string(role)
                    ));
                }
            }
            // The design's capsule (6.3): the symbol in a 16-point box,
            // 14 on a 30-point chip, 6 before the words.
            (_, Some(icon)) if element.keyword("shape") == Some("capsule") && !ctx.toolbar => {
                let chip = element
                    .number("height")
                    .is_some_and(|height| height <= 30.0);
                out.open("SwiftUI.HStack(spacing: 6) {");
                let box_sz = if chip { "14" } else { "16" };
                if Self::uses_custom_icon(icon) {
                    out.line(format!(
                        "BernIcon(name: {}, size: {box_sz}).frame(width: {box_sz}, height: {box_sz}).accessibilityHidden(true)",
                        string(icon)
                    ));
                } else {
                    out.line(format!(
                        "SwiftUI.Image(systemName: {}).frame(width: {box_sz}, height: {box_sz}).accessibilityHidden(true)",
                        string(icon),
                    ));
                }
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
            }
            (_, Some(icon)) if Self::uses_custom_icon(icon) => {
                out.line(format!(
                    "BernIconLabel(title: {label}, icon: {})",
                    string(icon)
                ));
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
            Some(style)
                if element.kind == "Button"
                    && !ctx.toolbar
                    && element.keyword("shape") == Some("capsule")
                    && !style.draws_base_look() =>
            {
                self.capsule_control(out, element, Some(&style.name));
                if let Some(size) = style.size() {
                    out.line(format!(
                        ".bernControlSize({})",
                        crate::style::control_size(size)
                    ));
                }
                if !style.draws_look() {
                    out.line(format!(".bernFeedback(BernStyles.{})", style.name));
                }
            }
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
                        let prominent = element.keyword("prominence") == Some("prominent");
                        let plain = element.keyword("prominence") == Some("plain");
                        if plain {
                            out.line(format!(
                                ".bernCircleControl(diameter: {diameter}, prominent: {prominent}, plain: true)"
                            ));
                        } else {
                            out.line(format!(
                                ".bernCircleControl(diameter: {diameter}, prominent: {prominent})"
                            ));
                        }
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
                        self.capsule_control(out, element, style.as_ref().map(|s| s.name.as_str()));
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
                            ".bernControlSize({})",
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
        if let Some(cursor) = element.keyword("cursor") {
            out.line(format!(
                ".bernCursor({}, enabled: {})",
                string(cursor),
                enabled.as_deref().unwrap_or("true")
            ));
        }
        out.line(format!(
            ".bernControl({}, role: \"button\", label: {label}, enabled: {}, actions: BernControlActions(activate: {{ {action} }}{file}))",
            self.target(element, ctx),
            enabled.unwrap_or_else(|| "true".into())
        ));
        self.double_tap(out, element, &action);
    }

    fn double_tap(&self, out: &mut Out, element: &Element, single: &str) {
        if let Some(double) = element.children.iter().find_map(|node| match node {
            bern_ir::Node::Element(event) if event.kind == "DoubleTap" => event.action.as_ref(),
            _ => None,
        }) {
            out.line(format!(
                ".bernDoubleTap(single: {{ {single} }}, double: {{ {} }})",
                self.action(double)
            ));
        }
    }

    /// The platform's capsule button, `height` tall.
    fn capsule_control(&self, out: &mut Out, element: &Element, style: Option<&str>) {
        let style_arg = style
            .map(|name| format!(", style: BernStyles.{name}"))
            .unwrap_or_default();
        out.line(format!(
            ".bernCapsuleControl(height: {}, prominent: {}{style_arg})",
            self.length(element, "height")
                .unwrap_or_else(|| "32.0".into()),
            element.keyword("prominence") == Some("prominent")
        ));
    }

    /// The pointer events of an element other than a button: a double
    /// click or tap, its context menu and wheel turns, as modifiers that
    /// keep its layout as it is.
    pub(super) fn pointer_events(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let has_modifier = if let Some(events) = self.pointer_modifier(element, true) {
            out.line(format!(".modifier({events})"));
            true
        } else {
            false
        };
        if !has_modifier && element.context_menu().is_some() && element.kind != "Button" {
            out.line(".contentShape(Rectangle())");
        }
        self.context_menu(out, element, ctx);
    }

    /// `BernPointerEvents(…)` for an element's `DoubleTap`, unless its
    /// button arbitrates that itself, its wheel turns, and its hover
    /// action where the system has a pointer; none when it declares none.
    pub(super) fn pointer_modifier(&self, element: &Element, double_tap: bool) -> Option<String> {
        let event = |kind: &str| {
            element
                .pointer_event(kind)
                .and_then(|event| event.action.as_ref())
                .map(|action| format!("{{ {} }}", self.action(action)))
        };
        let double_tap = double_tap.then(|| event("DoubleTap")).flatten();
        let pointer = matches!(self.platform, Platform::Macos | Platform::Ios);
        let up = pointer.then(|| event("WheelUp")).flatten();
        let down = pointer.then(|| event("WheelDown")).flatten();
        let hover = pointer.then(|| event("Hover")).flatten();
        let hover_delay = hover
            .as_ref()
            .and_then(|_| element.pointer_event("Hover")?.number("delay"))
            .unwrap_or(0.0);
        if double_tap.is_none() && up.is_none() && down.is_none() && hover.is_none() {
            return None;
        }
        let closure = |closure: Option<String>| closure.unwrap_or_else(|| "nil".into());
        Some(format!(
            "BernPointerEvents(doubleTap: {}, wheelUp: {}, wheelDown: {}, hover: {}, hoverDelay: {}, shaped: {})",
            closure(double_tap),
            closure(up),
            closure(down),
            closure(hover),
            double(hover_delay),
            element.kind != "Button"
        ))
    }

    /// An element's `ContextMenu`: the system's menu on a secondary click or
    /// a long press. Its items are controls too, reached while it is closed.
    pub(super) fn context_menu(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let Some(items) = element.context_menu() else {
            return;
        };
        if self.platform == Platform::Watchos {
            let title = self
                .text_prop(element, "label")
                .unwrap_or_else(|| "\"\"".into());
            out.open(format!(
                ".modifier(BernWatchContextMenu(title: {title}) {{ dismiss in"
            ));
            self.table_menu(out, &items, "dismiss(); ");
            out.close("})");
        } else {
            out.open(".contextMenu {");
            self.table_menu(out, &items, "");
            out.close("}");
        }
        for item in items.iter().filter(|item| item.kind == "MenuItem") {
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, file) = self.pressed(item);
            let enabled = item
                .expr("enabled")
                .map(|expr| self.expr(expr))
                .unwrap_or_else(|| "true".into());
            let checked = item
                .expr("checked")
                .map(|expr| {
                    format!(
                        ", value: {{ ({}) ? \"checked\" : \"unchecked\" }}",
                        self.expr(expr)
                    )
                })
                .unwrap_or_default();
            out.line(format!(
                ".bernControl({}, role: \"button\", label: {item_label}, enabled: {enabled}, actions: BernControlActions(activate: {{ {action} }}{file}{checked}))",
                self.target(item, ctx)
            ));
        }
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
        let icon_size = element
            .number("font_size")
            .map(|size| format!(", iconSize: {}", double(size)))
            .unwrap_or_default();
        let placement = element
            .keyword("placement")
            .map(|where_| format!(", placement: {}", string(where_)))
            .unwrap_or_default();
        out.open(format!(
            "BernPopoverButton(label: {label}, systemImage: {icon}, presented: {presented}, target: {}, windowTheme: {window_theme}, style: {style}{icon_size}{placement}) {{",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        if !ctx.toolbar {
            let height = self.length(element, "height");
            if element.icon("icon").is_some()
                && (element.keyword("shape") == Some("capsule") || height.is_some())
            {
                let diameter = height.unwrap_or_else(|| "32.0".into());
                if element.keyword("prominence") == Some("plain") {
                    out.line(format!(
                        ".bernCircleControl(diameter: {diameter}, prominent: false, plain: true)"
                    ));
                } else {
                    out.line(format!(
                        ".bernCircleControl(diameter: {diameter}, prominent: false)"
                    ));
                }
            } else if element.keyword("shape") == Some("capsule") {
                self.capsule_control(out, element, None);
            } else {
                // `plain` draws no background, as a plain `IconButton` beside it.
                out.line(match element.keyword("prominence") {
                    Some("plain") => ".bernButtonLook(.flat)",
                    _ if element.icon("icon").is_some() => ".bernButtonLook(.icon)",
                    _ => ".bernButtonLook(.secondary)",
                });
                if let Some(height) = height {
                    out.line(format!(".frame(height: {height})"));
                }
            }
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
        if element.expr("current").is_some() {
            return self.held_text_input(out, element, ctx);
        }
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
            // On iOS a field outside a form is otherwise bare text.
            if self.platform == Platform::Ios {
                out.line(".bernFieldBox()");
            }
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

    /// `TextInput(current:)`: the field shows engine state and each edit
    /// runs the action; it holds the user's text while being edited.
    fn held_text_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (value, _, setter) = self.control_value(element, "current");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .map(|prompt| format!("SwiftUI.Text(verbatim: {prompt})"))
            .unwrap_or_else(|| "nil".into());
        let kind = element.keyword("kind").unwrap_or("plain");
        let ios = self.platform.uikit() && element.keyword("label_hidden") != Some("true");
        if ios {
            if kind == "multiline" {
                out.open("SwiftUI.VStack(alignment: .leading, spacing: 6) {");
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
            } else {
                out.open("SwiftUI.LabeledContent {");
            }
        }
        out.open(format!(
            "BernHeldText(value: {value}, commit: {setter}) {{ text in"
        ));
        match kind {
            "secure" => out.open(format!(
                "SwiftUI.SecureField(text: text, prompt: {prompt}) {{"
            )),
            "multiline" => out.open(format!(
                "SwiftUI.TextField(text: text, prompt: {prompt}, axis: .vertical) {{"
            )),
            _ => out.open(format!(
                "SwiftUI.TextField(text: text, prompt: {prompt}) {{"
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
        out.close("}");
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
        if self.platform == Platform::Macos {
            out.line(".modifier(BernComposerFocus())");
        }
        let shown = if kind == "secure" {
            String::new()
        } else {
            format!(", value: {{ {value} }}")
        };
        out.line(format!(
            ".bernControl({}, role: \"text_input\", label: {label}, actions: BernControlActions(edit: {setter}{shown}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn switch(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        if element.keyword("login_item") == Some("true") {
            // Only macOS opens applications at login.
            if self.platform != Platform::Macos {
                out.line("SwiftUI.EmptyView()");
                return;
            }
            let label = self.text_prop(element, "label").unwrap_or_default();
            out.line(format!("BernLoginItemToggle(label: {label})"));
            if element.keyword("label_hidden") == Some("true") {
                out.line(".labelsHidden()");
            }
            out.line(".bernAccentTint()");
            out.line(format!(
                ".bernControl({}, role: \"switch\", label: {label}, actions: BernControlActions(setBool: {{ on in BernLoginItem.set(on) }}, value: {{ BernLoginItem.isOn ? \"true\" : \"false\" }}))",
                self.target(element, ctx)
            ));
            return;
        }
        let (field, binding, setter) = self.control_value(element, "checked");
        let label = self.text_prop(element, "label").unwrap_or_default();
        out.open(format!("SwiftUI.Toggle(isOn: {binding}) {{"));
        match element.icon("icon") {
            Some(icon) if Self::uses_custom_icon(icon) => out.line(format!(
                "BernIconLabel(title: {label}, icon: {})",
                string(icon)
            )),
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
        let vertical = element.keyword("axis") == Some("vertical");
        let icon_arg = |name: &str| {
            element
                .icon(name)
                .map(string)
                .unwrap_or_else(|| "nil".into())
        };
        if let Some(current) = element.expr("current") {
            let (value, _, commit) = self.control_value(element, "current");
            // The slider shows a whole number as a `Double`.
            let value = if current.ty == Type::Int {
                format!("Double({value})")
            } else {
                value
            };
            let label = self.text_prop(element, "label").unwrap_or_default();
            if vertical {
                out.line(format!(
                    "BernControlledVerticalSlider(value: {value}, minimum: {}, maximum: {}, step: {}, label: {label}, labelHidden: {}, continuous: {}, minimumIcon: {}, maximumIcon: {}, commit: {commit})",
                    double(element.number("min").unwrap_or(0.0)),
                    double(element.number("max").unwrap_or(1.0)),
                    element.number("step").map(double).unwrap_or_else(|| "nil".into()),
                    element.keyword("label_hidden") == Some("true"),
                    element.keyword("continuous") == Some("true"),
                    icon_arg("min_icon"),
                    icon_arg("max_icon"),
                ));
            } else {
                out.line(format!("BernControlledSlider(value: {value}, minimum: {}, maximum: {}, step: {}, label: {label}, labelHidden: {}, continuous: {}, commit: {commit})", double(element.number("min").unwrap_or(0.0)), double(element.number("max").unwrap_or(1.0)), element.number("step").map(double).unwrap_or_else(|| "nil".into()), element.keyword("label_hidden") == Some("true"), element.keyword("continuous") == Some("true")));
            }
            self.slider_hover(out, element);
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
        if vertical && self.platform != Platform::Tvos {
            out.open(format!(
                "BernVerticalSlider(value: {}, minimum: {min}, maximum: {max}, step: {}, label: {label}, labelHidden: {}, minimumIcon: {}, maximumIcon: {}, onEditingChanged: {{ editing in",
                self.binding_of(&field),
                element.number("step").map(double).unwrap_or_else(|| "nil".into()),
                element.keyword("label_hidden") == Some("true"),
                icon_arg("min_icon"),
                icon_arg("max_icon"),
            ));
            match &action {
                Some(action) => out.line(format!("if !editing {{ {action} }}")),
                None => out.line("_ = editing"),
            }
            out.close("})");
            out.line(".bernAccentTint()");
            self.slider_hover(out, element);
            let commit = action
                .map(|action| format!("; {action}"))
                .unwrap_or_default();
            out.line(format!(
                ".bernControl({}, role: \"slider\", label: {label}, actions: BernControlActions(setNumber: {{ value in {field} = value{commit} }}, value: {{ BernText.text({field}) }}))",
                self.target(element, ctx)
            ));
            return;
        }
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
            "BernSliderInteraction(value: {}, minimum: {min}, maximum: {max}, step: {}, onEditingChanged: {{ editing in",
            self.binding_of(&field),
            element.number("step").map(double).unwrap_or_else(|| "nil".into())
        ));
        match &action {
            Some(action) => out.line(format!("if !editing {{ {action} }}")),
            None => out.line("_ = editing"),
        }
        out.close("}) { binding, editing in");
        out.indent += 1;
        out.open(format!(
            "SwiftUI.Slider(value: binding, in: {min}...{max}{step}) {{"
        ));
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        let (min_icon, max_icon) = (element.icon("min_icon"), element.icon("max_icon"));
        if min_icon.is_some() || max_icon.is_some() {
            out.close("} minimumValueLabel: {");
            out.indent += 1;
            match min_icon {
                Some(icon) if Self::uses_custom_icon(icon) => {
                    out.line(format!("BernIcon(name: {})", string(icon)))
                }
                Some(icon) => out.line(format!("SwiftUI.Image(systemName: {})", string(icon))),
                None => out.line("SwiftUI.EmptyView()"),
            }
            out.close("} maximumValueLabel: {");
            out.indent += 1;
            match max_icon {
                Some(icon) if Self::uses_custom_icon(icon) => {
                    out.line(format!("BernIcon(name: {})", string(icon)))
                }
                Some(icon) => out.line(format!("SwiftUI.Image(systemName: {})", string(icon))),
                None => out.line("SwiftUI.EmptyView()"),
            }
        }
        out.close("} onEditingChanged: { editingValue in");
        out.indent += 1;
        out.line("editing(editingValue)");
        out.close("}");
        out.close("}");
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
        }
        out.line(".bernAccentTint()");
        self.slider_hover(out, element);
        let commit = action
            .map(|action| format!("; {action}"))
            .unwrap_or_default();
        out.line(format!(
            ".bernControl({}, role: \"slider\", label: {label}, actions: BernControlActions(setNumber: {{ value in {field} = value{commit} }}, value: {{ BernText.text({field}) }}))",
            self.target(element, ctx)
        ));
    }

    /// `hover`: where the pointer is along the track while it is over the
    /// slider, on the systems with a pointer.
    fn slider_hover(&self, out: &mut Out, element: &Element) {
        if !matches!(self.platform, Platform::Macos | Platform::Ios) {
            return;
        }
        if let Some(hover) = self.field(element, "hover") {
            if element.keyword("axis") == Some("vertical") {
                out.line(format!(
                    ".bernSliderHover({}, vertical: true)",
                    self.binding_of(&hover)
                ));
            } else {
                out.line(format!(".bernSliderHover({})", self.binding_of(&hover)));
            }
        }
    }

    pub(super) fn color_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let label_hidden = element.keyword("label_hidden") == Some("true");
        if let Some(current) = element.expr("current") {
            let (value, _, commit) = self.control_value(element, "current");
            let value = if current.ty == Type::Int {
                format!("Double({value})")
            } else {
                value
            };
            out.line(format!(
                "BernControlledColorPicker(value: {value}, label: {label}, labelHidden: {label_hidden}, commit: {commit})"
            ));
            out.line(format!(
                ".bernControl({}, role: \"color_picker\", label: {label}, actions: BernControlActions(setNumber: {commit}, value: {{ BernText.text({value}) }}))",
                self.target(element, ctx)
            ));
            return;
        }
        let field = self.field(element, "value").expect("number field");
        let integer = matches!(
            element.field("value").map(|field| &field.ty),
            Some(Type::Int)
        );
        let commit = element
            .action
            .as_ref()
            .map(|action| self.action(action))
            .unwrap_or_default();
        let binding = self.binding_of(&field);
        let read = if integer {
            format!("Double({binding}.wrappedValue)")
        } else {
            format!("{binding}.wrappedValue")
        };
        let assign_next = if integer {
            format!("{binding}.wrappedValue = Int64(next.rounded())")
        } else {
            format!("{binding}.wrappedValue = next")
        };
        let assign_value = if integer {
            format!("{field} = Int64(value.rounded())")
        } else {
            format!("{field} = value")
        };
        out.line(format!(
            "BernControlledColorPicker(value: {read}, label: {label}, labelHidden: {label_hidden}) {{ next in {assign_next}; {commit} }}"
        ));
        out.line(format!(
            ".bernControl({}, role: \"color_picker\", label: {label}, actions: BernControlActions(setNumber: {{ value in {assign_value}; {commit} }}, value: {{ BernText.text({field}) }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn tooltip(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let keys = match element.prop("keys") {
            Some(Prop::Keyword { value }) => Some(string(value)),
            Some(Prop::Expr { expr }) => Some(self.expr(expr)),
            _ => None,
        };
        if let Some(child) = element.children.first() {
            self.node(out, child, ctx);
            match keys {
                Some(keys) => out.line(format!(".bernTooltip({content}, keys: {keys})")),
                None => out.line(format!(".bernTooltip({content})")),
            }
        }
    }

    pub(super) fn banner(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let severity = element.keyword("severity").unwrap_or("info");
        let default_icon = match severity {
            "success" => "checkmark.circle",
            "warning" => "exclamationmark.triangle",
            "error" => "xmark.circle.fill",
            _ => "info.circle",
        };
        let icon = element.icon("icon").unwrap_or(default_icon);
        let title = self
            .text_prop(element, "title")
            .unwrap_or_else(|| "nil".into());
        let content = self.text_prop(element, "content").unwrap_or_default();
        let dismiss_label = self
            .text_prop(element, "dismiss_label")
            .unwrap_or_else(|| "nil".into());
        let (dismiss_target, on_dismiss) = match &element.action {
            Some(action) => (
                element
                    .id
                    .as_ref()
                    .map(|_| self.target(element, ctx))
                    .unwrap_or_else(|| "nil".into()),
                format!("{{ {} }}", self.action(action)),
            ),
            None => ("nil".into(), "nil".into()),
        };
        out.open(format!(
            "BernBanner(severity: {}, title: {title}, content: {content}, systemImage: {}, dismissLabel: {dismiss_label}, dismissTarget: {dismiss_target}, onDismiss: {on_dismiss}) {{",
            string(severity),
            string(icon),
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn disclosure(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let icon = element
            .icon("icon")
            .map(string)
            .unwrap_or_else(|| "nil".into());
        let (external_binding, on_toggle) = if element.expr("open").is_some() {
            let (_, binding, _) = self.control_value(element, "open");
            (binding, "nil".into())
        } else if let Some(field) = self.field(element, "expanded") {
            let action = element
                .action
                .as_ref()
                .map(|a| format!("; {}", self.action(a)))
                .unwrap_or_default();
            (
                format!(
                    "SwiftUI.Binding(get: {{ {field} }}, set: {{ newValue in {field} = newValue{action} }})"
                ),
                "nil".into(),
            )
        } else {
            let on_toggle = element
                .action
                .as_ref()
                .map(|a| format!("{{ _ in {} }}", self.action(a)))
                .unwrap_or_else(|| "nil".into());
            ("nil".into(), on_toggle)
        };
        out.open(format!(
            "BernDisclosure(label: {label}, systemImage: {icon}, externalExpanded: {external_binding}, onToggle: {on_toggle}, target: {}) {{",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn number_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let label_hidden = element.keyword("label_hidden") == Some("true");
        let min_arg = element
            .number("min")
            .map(double)
            .unwrap_or_else(|| "nil".into());
        let max_arg = element
            .number("max")
            .map(double)
            .unwrap_or_else(|| "nil".into());
        let step_val = element.number("step").unwrap_or(1.0);
        let is_int = matches!(element.expr("current").map(|e| &e.ty), Some(Type::Int));
        let precision =
            infer_precision(element.number("precision"), element.number("step"), is_int);
        let enabled = element
            .expr("enabled")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        let clamp_expr = |raw: &str| {
            let mut expr = raw.to_string();
            if let Some(min) = element.number("min") {
                expr = format!("max({expr}, {})", double(min));
            }
            if let Some(max) = element.number("max") {
                expr = format!("min({expr}, {})", double(max));
            }
            let factor = 10_f64.powi(precision as i32);
            format!(
                "(({expr} * {}).rounded() / {})",
                double(factor),
                double(factor)
            )
        };
        if let Some(current) = element.expr("current") {
            let (raw_val, _, commit) = self.control_value(element, "current");
            let value = if current.ty == Type::Int {
                format!("Double({raw_val})")
            } else {
                raw_val
            };
            let clamped = clamp_expr("v");
            out.line(format!(
                "BernNumberInput(value: {value}, minimum: {min_arg}, maximum: {max_arg}, step: {}, precision: {precision}, label: {label}, labelHidden: {label_hidden}, enabled: {enabled}, commit: {commit})",
                double(step_val)
            ));
            out.line(format!(
                ".bernControl({}, role: \"number_input\", label: {label}, enabled: {enabled}, actions: BernControlActions(edit: {{ text in if let v = Double(text.trimmingCharacters(in: .whitespacesAndNewlines)), v.isFinite {{ ({commit})({clamped}) }} }}, setNumber: {{ v in guard v.isFinite else {{ return }}; ({commit})({clamped}) }}, value: {{ BernText.text({value}) }}))",
                self.target(element, ctx)
            ));
            return;
        }
        let field = self.field(element, "value").expect("number field");
        let action = element
            .action
            .as_ref()
            .map(|a| format!("; {}", self.action(a)))
            .unwrap_or_default();
        let clamped = clamp_expr("v");
        let commit = format!("{{ next in {field} = next{action} }}");
        out.line(format!(
            "BernNumberInput(value: {field}, minimum: {min_arg}, maximum: {max_arg}, step: {}, precision: {precision}, label: {label}, labelHidden: {label_hidden}, enabled: {enabled}, commit: {commit})",
            double(step_val)
        ));
        out.line(format!(
            ".bernControl({}, role: \"number_input\", label: {label}, enabled: {enabled}, actions: BernControlActions(edit: {{ text in if let v = Double(text.trimmingCharacters(in: .whitespacesAndNewlines)), v.isFinite {{ {field} = {clamped}{action} }} }}, setNumber: {{ v in guard v.isFinite else {{ return }}; {field} = {clamped}{action} }}, value: {{ BernText.text({field}) }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn date_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
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
                format!("({v}.year, {v}.month, {v}.day)")
            })
            .unwrap_or_else(|| "nil".into());
        let max_arg = element
            .expr("max")
            .map(|e| {
                let v = self.expr(e);
                format!("({v}.year, {v}.month, {v}.day)")
            })
            .unwrap_or_else(|| "nil".into());
        let (val_expr, commit_date) = if element.expr("current").is_some() {
            let (raw_val, _, commit) = self.control_value(element, "current");
            (
                raw_val,
                format!("{{ y, m, d in ({commit})({date_type}(year: y, month: m, day: d)) }}"),
            )
        } else {
            let field = self.field(element, "value").expect("date field");
            let action = element
                .action
                .as_ref()
                .map(|a| format!("; {}", self.action(a)))
                .unwrap_or_default();
            (
                field.clone(),
                format!(
                    "{{ y, m, d in {field} = {date_type}(year: y, month: m, day: d){action} }}"
                ),
            )
        };
        out.line(format!(
            "BernDatePicker(year: ({val_expr}).year, month: ({val_expr}).month, day: ({val_expr}).day, minDate: {min_arg}, maxDate: {max_arg}, label: {label}, labelHidden: {label_hidden}, enabled: {enabled}, commit: {commit_date})"
        ));
        out.line(format!(
            ".bernControl({}, role: \"date_picker\", label: {label}, enabled: {enabled}, actions: BernControlActions(edit: {{ text in if let parsed = BernDateCodec.parseISO(text) {{ let c = BernDateCodec.clamp(parsed, min: {min_arg}, max: {max_arg}); ({commit_date})(c.0, c.1, c.2) }} }}, value: {{ BernDateCodec.formatISO(({val_expr}).year, ({val_expr}).month, ({val_expr}).day) }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn time_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let label_hidden = element.keyword("label_hidden") == Some("true");
        let min_val = double(element.number("min").unwrap_or(0.0));
        let max_val = double(element.number("max").unwrap_or(1439.0));
        let step_val = double(element.number("step").unwrap_or(1.0));
        let enabled = element
            .expr("enabled")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        let (val_expr, commit) = if let Some(current) = element.expr("current") {
            let (raw_val, _, commit) = self.control_value(element, "current");
            let value = if current.ty == Type::Int {
                format!("Double({raw_val})")
            } else {
                raw_val
            };
            (value, commit)
        } else {
            let field = self.field(element, "value").expect("time field");
            let action = element
                .action
                .as_ref()
                .map(|a| format!("; {}", self.action(a)))
                .unwrap_or_default();
            (
                field.clone(),
                format!("{{ next in {field} = next{action} }}"),
            )
        };
        out.line(format!(
            "BernTimePicker(minutes: {val_expr}, minMinutes: {min_val}, maxMinutes: {max_val}, step: {step_val}, label: {label}, labelHidden: {label_hidden}, enabled: {enabled}, commit: {commit})"
        ));
        out.line(format!(
            ".bernControl({}, role: \"time_picker\", label: {label}, enabled: {enabled}, actions: BernControlActions(edit: {{ text in if let parsed = BernTimeCodec.parse(text) {{ ({commit})(BernTimeCodec.clamp(parsed, min: {min_val}, max: {max_val}, step: {step_val})) }} }}, setNumber: {{ v in guard v.isFinite else {{ return }}; ({commit})(BernTimeCodec.clamp(v, min: {min_val}, max: {max_val}, step: {step_val})) }}, value: {{ BernTimeCodec.format({val_expr}) }}))",
            self.target(element, ctx)
        ));
    }

    pub(super) fn empty_state(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let title = self.text_prop(element, "title").unwrap_or_default();
        let message = self
            .text_prop(element, "message")
            .unwrap_or_else(|| "nil".into());
        let icon = element
            .icon("icon")
            .map(string)
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernEmptyState(title: {title}, message: {message}, systemImage: {icon}) {{"
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn badge(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let tone = element.keyword("tone").unwrap_or("neutral");
        let icon = element
            .icon("icon")
            .map(string)
            .unwrap_or_else(|| "nil".into());
        out.line(format!(
            "BernBadge(content: {content}, tone: {}, systemImage: {icon})",
            string(tone)
        ));
    }

    pub(super) fn avatar(&self, out: &mut Out, element: &Element) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let size = self
            .length(element, "size")
            .unwrap_or_else(|| "40.0".into());
        let rounded = element.keyword("shape") == Some("rounded");
        let (name_arg, url_arg) = match element.expr("source") {
            Some(expr) if matches!(expr.ty, Type::Optional { .. }) => {
                ("nil".into(), format!("({})?.url", self.expr(expr)))
            }
            Some(expr) => ("nil".into(), format!("({}).url", self.expr(expr))),
            None => (
                element
                    .resource("source")
                    .map(string)
                    .unwrap_or_else(|| "nil".into()),
                "nil".into(),
            ),
        };
        out.line(format!(
            "BernAvatar(label: {label}, name: {name_arg}, url: {url_arg}, size: CGFloat({size}), rounded: {rounded})"
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
        let selection = match self.field(element, "selection") {
            Some(field) => format!("SwiftUI.Binding(get: {{ {field} }}, set: {{ {field} = $0 }})"),
            None => "nil".into(),
        };
        let expanded = match self.field(element, "expanded") {
            Some(field) => format!("SwiftUI.Binding(get: {{ {field} }}, set: {{ {field} = $0 }})"),
            None => "nil".into(),
        };
        let on_activate = element
            .action
            .as_ref()
            .map(|a| format!("{{ {} }}", self.action(a)))
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernTreeView(target: {}, label: {label}, entries: ({items}).map {{ BernTreeEntry(item: $0, key: $0.{key_prop}, parent: $0.{parent_prop}) }}, selection: {selection}, externalExpanded: {expanded}, onActivate: {on_activate}, keyToString: {{ String(describing: $0) }}) {{ {var} in",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, ctx);
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
            .unwrap_or_else(|| "nil".into());
        let vertical = element.keyword("axis") == Some("vertical");
        let ratio = match self.field(element, "ratio") {
            Some(field) => format!("SwiftUI.Binding(get: {{ {field} }}, set: {{ {field} = $0 }})"),
            None => "nil".into(),
        };
        let min_first = self
            .length(element, "min_first")
            .unwrap_or_else(|| "120.0".into());
        let min_second = self
            .length(element, "min_second")
            .unwrap_or_else(|| "120.0".into());
        out.open(format!(
            "BernSplitPane(target: {target}, vertical: {vertical}, externalRatio: {ratio}, minFirst: CGFloat({min_first}), minSecond: CGFloat({min_second})) {{"
        ));
        self.node(out, &element.children[0], ctx);
        out.close("} second: {");
        out.indent += 1;
        self.node(out, &element.children[1], ctx);
        out.close("}");
    }

    pub(super) fn reorderable(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (
            Some(items_expr),
            Some(template),
            Some(Prop::Key { name: key_name, .. }),
            Some(from_field),
            Some(to_field),
            Some(action),
        ) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
            self.field(element, "move_from"),
            self.field(element, "move_to"),
            &element.action,
        )
        else {
            return;
        };
        let label = self.text_prop(element, "label").unwrap_or_default();
        let items = self.expr(items_expr);
        let key_prop = member(key_name);
        let var = member(&template.var);
        let act = self.action(action);
        out.open(format!(
            "BernReorderable(target: {}, label: {label}, items: {items}, keyOf: {{ $0.{key_prop} }}, onMove: {{ fromIndex, toIndex in {from_field} = fromIndex; {to_field} = toIndex; {act} }}) {{ {var} in",
            self.target(element, ctx)
        ));
        out.open("SwiftUI.HStack(spacing: 6) {");
        self.nodes(
            out,
            &element.children,
            &Ctx {
                row: true,
                ..ctx.clone()
            },
        );
        out.close("}");
        out.close("}");
    }

    pub(super) fn bottom_sheet(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let detents = element.keyword("detents").unwrap_or("medium");
        let drag_indicator = element.keyword("drag_indicator") != Some("false");
        let on_dismiss = element
            .action
            .as_ref()
            .map(|a| format!("{{ {} }}", self.action(a)))
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernBottomSheet(detents: {}, dragIndicator: {drag_indicator}, onDismiss: {on_dismiss}) {{",
            string(detents)
        ));
        self.nodes(out, &element.children, ctx);
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
            .unwrap_or_default();
        out.open(format!(
            "BernRefreshable(target: {}, refreshing: {refreshing}, onRefresh: {{ {act} }}) {{",
            self.target(element, ctx)
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn alert(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let title = self.text_prop(element, "title").unwrap_or_default();
        let message = self
            .text_prop(element, "message")
            .unwrap_or_else(|| "nil".into());
        let on_dismiss = element
            .action
            .as_ref()
            .map(|a| format!("{{ {} }}", self.action(a)))
            .unwrap_or_else(|| "nil".into());
        let mut specs = Vec::new();
        for (idx, child) in element.children.iter().enumerate() {
            let Node::Element(act) = child else {
                continue;
            };
            if act.kind != "AlertAction" {
                continue;
            }
            let id = act.id.as_deref().unwrap_or("action");
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
                .unwrap_or_default();
            specs.push(format!(
                "BernAlertActionSpec(id: {}, target: {target}, label: {label}, role: {}, enabled: {enabled}, action: {{ {code} }})",
                string(&format!("{id}_{idx}")),
                string(role)
            ));
        }
        out.line(format!(
            "BernAlertView(title: {title}, message: {message}, actions: [{}], onDismiss: {on_dismiss})",
            specs.join(", ")
        ));
    }

    pub(super) fn picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        if element.expr("items").is_some() {
            return self.dynamic_picker(out, element, ctx);
        }
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
        if element.keyword("style") == Some("combobox") {
            let prompt = self
                .text_prop(element, "prompt")
                .unwrap_or_else(|| "\"\"".into());
            let height = self
                .length(element, "height")
                .unwrap_or_else(|| "nil".into());
            out.line(format!(
                "BernComboBox(selection: {binding}, label: {label}, labelHidden: {}, prompt: {prompt}, options: {}, capsule: {}, height: {height})",
                element.keyword("label_hidden") == Some("true"),
                pairs(),
                element.keyword("shape") == Some("capsule"),
            ));
            out.line(format!(
                ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
                self.target(element, ctx)
            ));
            return;
        }
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
        if self.platform == Platform::Watchos && element.keyword("style") != Some("inline") {
            out.line(format!(
                "BernWatchPicker(selection: {binding}, label: {label}, options: {}, labelHidden: {}, capsule: {}, height: {})",
                pairs(),
                element.keyword("label_hidden") == Some("true"),
                element.keyword("shape") == Some("capsule"),
                self.length(element, "height").unwrap_or_else(|| "32.0".into())
            ));
            out.line(format!(
                ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
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
        // form's row reads on the other platforms. iOS keeps its menu, but
        // its value stands in the capsule too, in the words' color rather
        // than the accent the system gives a picker's value.
        let menu = matches!(
            element.keyword("style"),
            None | Some("automatic") | Some("menu")
        );
        let row = matches!(self.platform, Platform::Tvos | Platform::Ios)
            && menu
            && element.keyword("shape") != Some("capsule")
            && element.keyword("label_hidden") != Some("true");
        let segmented = element.keyword("style") == Some("segmented");
        let capsule = row || (element.keyword("shape") == Some("capsule") && !segmented);
        if row {
            if self.platform == Platform::Ios {
                out.open("SwiftUI.LabeledContent {");
            } else {
                out.open("SwiftUI.HStack(spacing: 12) {");
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.line("SwiftUI.Spacer(minLength: 12)");
            }
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
            _ if menu && capsule => out.line(".pickerStyle(.inline)"),
            _ if menu => out.line(format!(".pickerStyle(.menu){}", menu_tint(self.platform))),
            Some("segmented") => out.line(".pickerStyle(.segmented).tint(SwiftUI.Color.secondary)"),
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
                ".bernCapsuleControl(height: {}{})",
                self.length(element, "height").unwrap_or_else(|| {
                    if row && self.platform == Platform::Tvos {
                        "46.0".into()
                    } else {
                        "32.0".into()
                    }
                }),
                if row && self.platform == Platform::Ios {
                    ", filled: true"
                } else {
                    ""
                }
            ));
            out.line(format!(
                ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
            ));
        }
        if row {
            if self.platform == Platform::Ios {
                out.close("} label: {");
                out.indent += 1;
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
            }
            out.close("}");
        }
        out.line(format!(
            ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
            self.target(element, ctx)
        ));
    }

    fn dynamic_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let items_expr = element.expr("items").expect("dynamic picker items");
        let items_code = self.expr(items_expr);
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
        let key_def = &def.fields[*key_idx];
        let label_def = &def.fields[*label_idx];
        let key_member = member(&key_def.name);
        let label_member = member(&label_def.name);
        let item_label_expr = |var: &str| match &label_def.ty {
            Type::Text => format!("{var}.{label_member}"),
            Type::Int | Type::Float | Type::Enum { .. } => {
                format!("BernText.text({var}.{label_member})")
            }
            _ => format!("String(describing: {var}.{label_member})"),
        };
        let (field, binding, setter) = self.control_value(element, "selected");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "\"\"".into());
        let pairs = format!(
            "({items_code}).map {{ item in ({}, item.{key_member}) }}",
            item_label_expr("item")
        );
        let actions = if key_def.ty == Type::Int {
            format!(
                "BernControlActions(select: {{ name in guard let parsed = Int64(name), ({items_code}).contains(where: {{ $0.{key_member} == parsed }}) else {{ return false }}; ({setter})(parsed); return true }}, value: {{ String({field}) }})"
            )
        } else {
            format!(
                "BernControlActions(select: {{ name in guard ({items_code}).contains(where: {{ $0.{key_member} == name }}) else {{ return false }}; ({setter})(name); return true }}, value: {{ {field} }})"
            )
        };
        if element.keyword("style") == Some("combobox") {
            let height = self
                .length(element, "height")
                .unwrap_or_else(|| "nil".into());
            out.line(format!(
                "BernComboBox(selection: {binding}, label: {label}, labelHidden: {}, prompt: {prompt}, options: {pairs}, capsule: {}, height: {height})",
                element.keyword("label_hidden") == Some("true"),
                element.keyword("shape") == Some("capsule"),
            ));
            out.line(format!(
                ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
                self.target(element, ctx)
            ));
            return;
        }
        if element.keyword("style") == Some("chips") {
            out.line(format!(
                "BernChips(selection: {binding}, options: {pairs}, flat: {}, target: {}, label: {label}, actions: {actions})",
                ctx.flow && element.keyword("label_hidden") == Some("true"),
                self.target(element, ctx)
            ));
            return;
        }
        if self.platform == Platform::Watchos && element.keyword("style") != Some("inline") {
            out.line(format!(
                "BernWatchPicker(selection: {binding}, label: {label}, options: {pairs}, labelHidden: {}, capsule: {}, height: {})",
                element.keyword("label_hidden") == Some("true"),
                element.keyword("shape") == Some("capsule"),
                self.length(element, "height").unwrap_or_else(|| "32.0".into())
            ));
            out.line(format!(
                ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
                self.target(element, ctx)
            ));
            return;
        }
        if self.platform == Platform::Tvos && element.keyword("style") == Some("segmented") {
            out.line(format!(
                "BernSegments(selection: {binding}, options: {pairs}).bernAccentTint()"
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
        let menu = matches!(
            element.keyword("style"),
            None | Some("automatic") | Some("menu")
        );
        let row = matches!(self.platform, Platform::Tvos | Platform::Ios)
            && menu
            && element.keyword("shape") != Some("capsule")
            && element.keyword("label_hidden") != Some("true");
        let segmented = element.keyword("style") == Some("segmented");
        let capsule = row || (element.keyword("shape") == Some("capsule") && !segmented);
        if row {
            if self.platform == Platform::Ios {
                out.open("SwiftUI.LabeledContent {");
            } else {
                out.open("SwiftUI.HStack(spacing: 12) {");
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.line("SwiftUI.Spacer(minLength: 12)");
            }
        }
        if capsule {
            out.open("SwiftUI.Menu {");
        }
        out.open(format!("SwiftUI.Picker(selection: {binding}) {{"));
        out.open(format!(
            "SwiftUI.ForEach({items_code}, id: \\.{key_member}) {{ item in"
        ));
        out.line(format!(
            "SwiftUI.Text(verbatim: {}).tag(item.{key_member})",
            item_label_expr("item")
        ));
        out.close("}");
        out.close("} label: {");
        out.indent += 1;
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        out.close("}");
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
        }
        match element.keyword("style") {
            _ if menu && capsule => out.line(".pickerStyle(.inline)"),
            _ if menu => out.line(format!(".pickerStyle(.menu){}", menu_tint(self.platform))),
            Some("segmented") => out.line(".pickerStyle(.segmented).tint(SwiftUI.Color.secondary)"),
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
            let arrow = self.platform.uikit();
            if arrow {
                out.open("SwiftUI.HStack(spacing: 6) {");
            }
            out.line(format!(
                "SwiftUI.Text(verbatim: ({items_code}).first(where: {{ $0.{key_member} == {field} }}).map {{ {} }} ?? ({prompt}.isEmpty ? {label} : {prompt}))",
                item_label_expr("$0")
            ));
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
                ".bernCapsuleControl(height: {}{})",
                self.length(element, "height").unwrap_or_else(|| {
                    if row && self.platform == Platform::Tvos {
                        "46.0".into()
                    } else {
                        "32.0".into()
                    }
                }),
                if row && self.platform == Platform::Ios {
                    ", filled: true"
                } else {
                    ""
                }
            ));
            out.line(format!(
                ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
            ));
        }
        if row {
            if self.platform == Platform::Ios {
                out.close("} label: {");
                out.indent += 1;
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
            }
            out.close("}");
        }
        out.line(format!(
            ".bernControl({}, role: \"picker\", label: {label}, actions: {actions})",
            self.target(element, ctx)
        ));
    }

    pub(super) fn language_picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        if self.platform == Platform::Watchos {
            out.line(format!(
                "BernWatchPicker(selection: SwiftUI.Binding(get: {{ BernLanguage.shared.selection }}, set: {{ BernLanguage.shared.select($0) }}), label: {label}, options: [(BernText.string(\"bern.language.system\"), \"system\")] + BernLanguage.shared.supported.map {{ (BernLanguage.autonym($0), $0) }}, labelHidden: {})",
                element.keyword("label_hidden") == Some("true")
            ));
            out.line(format!(
                ".bernControl({}, role: \"picker\", label: {label}, actions: BernControlActions(select: {{ BernLanguage.shared.select($0) }}, value: {{ BernLanguage.shared.current }}))",
                self.target(element, ctx)
            ));
            return;
        }
        out.open(
            "SwiftUI.Picker(selection: SwiftUI.Binding(get: { BernLanguage.shared.selection }, set: { BernLanguage.shared.select($0) })) {",
        );
        out.line(
            "SwiftUI.Text(verbatim: BernText.string(\"bern.language.system\")).tag(\"system\")",
        );
        out.open("SwiftUI.ForEach(BernLanguage.shared.supported, id: \\.self) { language in");
        out.line("SwiftUI.Text(verbatim: BernLanguage.autonym(language)).tag(language)");
        out.close("}");
        out.close("} label: {");
        out.indent += 1;
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        out.close("}");
        if element.keyword("label_hidden") == Some("true") {
            out.line(".labelsHidden()");
        }
        out.line(format!(".pickerStyle(.menu){}", menu_tint(self.platform)));
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
        let finish = element
            .children
            .iter()
            .find_map(|node| match node {
                bern_ir::Node::Element(event) if event.kind == "OnCommit" => event.action.as_ref(),
                _ => None,
            })
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
            let active = self
                .field(element, "active")
                .map(|field| self.binding_of(&field))
                .unwrap_or_else(|| "nil".into());
            out.line(format!(
                ".bernSwipe({}, in: {min}...{max}, axis: {axis}, distance: {}, continuous: {}, active: {active}, change: {{ {action} }}, commit: {{ {finish} }})",
                self.binding_of(&field),
                double(element.number("distance").unwrap_or(300.0)),
                element.keyword("continuous") == Some("true")
            ));
        }
        out.line(format!(
            ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
        ));
        let set_number = if element.kind == "Swipe" {
            let active = self
                .field(element, "active")
                .map(|field| self.binding_of(&field))
                .unwrap_or_else(|| "nil".into());
            format!(
                "let gestureActive: SwiftUI.Binding<Bool>? = {active}; gestureActive?.wrappedValue = true; bernCommitSwipe(update: {{ {field} = min(max(value, {min}), {max}) }}, change: {{ {action} }}, commit: {{ {finish} }}, clear: {{ gestureActive?.wrappedValue = false }})"
            )
        } else {
            format!("{field} = min(max(value, {min}), {max}); {action}")
        };
        out.line(format!(
            ".bernControl({}, role: {}, label: {label}, actions: BernControlActions(setNumber: {{ value in guard value.isFinite else {{ return }}; {set_number} }}, value: {{ BernText.text({field}) }}))",
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

    pub(super) fn watch(&self, out: &mut Out, element: &Element) {
        let value = element
            .expr("value")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "0".into());
        let action = self.action(element.action.as_ref().expect("watch action"));
        out.line(format!("BernWatch(value: {value}) {{ {action} }}"));
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

/// What a menu picker's value and arrows are drawn in: not the app's
/// accent, which the system would give them. macOS draws the system's own
/// pop-up button, in the words' color as in System Settings; iOS and tvOS
/// the value in the secondary color, as Settings lists a choice.
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

fn menu_tint(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => ".tint(nil as SwiftUI.Color?)",
        _ => ".tint(SwiftUI.Color.secondary)",
    }
}
