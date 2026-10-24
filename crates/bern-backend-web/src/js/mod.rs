//! JavaScript generation from the IR.
//!
//! The generated module decodes and encodes the application's values,
//! mirrors engine state into reactive cells, and builds every component
//! from the runtime's standard components. Expressions become JavaScript
//! expressions evaluated inside reactive effects; locals (component
//! parameters, row variables, pattern bindings) are getter functions, so a
//! change re-runs only the bindings that read it.
//!
//! Value layout: bool, `BigInt` for Int, number for Float, string; unit-only
//! enums are the variant index; other enums are `{ tag, fields }`; records
//! are objects with their declared field names; `null` is `None`.

use crate::icons;
use bern_ir::{Application, Node, Presentation, Prop, WindowInitial, WindowRole};
use bern_protocol::{TypeTable, Value};
use codecs::{codec, decode, encode, generate_enum, generate_record, literal};
use std::collections::BTreeSet;

mod codecs;
mod content;
mod controls;
mod expr;
mod layout;
mod menus;
mod motion;
mod navigation;
mod nodes;

#[cfg(test)]
mod tests;

/// Indented line writer.
#[derive(Default)]
struct Out {
    lines: Vec<String>,
    indent: usize,
}

impl Out {
    fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            self.lines.push(String::new());
        } else {
            self.lines
                .push(format!("{}{}", "  ".repeat(self.indent), text));
        }
    }

    fn open(&mut self, text: impl AsRef<str>) {
        self.line(text);
        self.indent += 1;
    }

    fn close(&mut self, text: impl AsRef<str>) {
        self.indent = self.indent.saturating_sub(1);
        self.line(text);
    }

    fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

/// A color for the runtime: a role, or a fixed pair for light and dark.
pub fn color_ref(color: &bern_ir::style::ColorRef) -> String {
    match color {
        bern_ir::style::ColorRef::Role { role } => format!("{{ role: {} }}", string(role)),
        bern_ir::style::ColorRef::Fixed { light, dark } => format!(
            "{{ light: \"#{:02x}{:02x}{:02x}\", dark: \"#{:02x}{:02x}{:02x}\" }}",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

/// A JavaScript identifier for a style name.
pub(crate) fn member(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        out.push(if ch.is_ascii_alphanumeric() { ch } else { '_' });
    }
    out
}

/// A JavaScript string literal.
pub fn string(value: &str) -> String {
    serde_json::to_string(value).expect("string")
}

pub(crate) fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value:?}")
    }
}

fn local(name: &str) -> String {
    format!("l_{name}")
}

pub struct Generator<'a> {
    app: &'a Application,
    /// Whether the nodes being written are a sidebar's content, where
    /// `Section` groups sidebar items.
    in_sidebar: std::cell::Cell<bool>,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application) -> Self {
        Self {
            app,
            in_sidebar: std::cell::Cell::new(false),
        }
    }

    fn types(&self) -> &TypeTable {
        &self.app.types
    }

    /// The motion tokens (`M`), transitions (`X`) and patterns (`P`) of
    /// `style!`.
    fn style_tables(&self, out: &mut Out) {
        let style = &self.app.style;
        out.line("// Motion tokens: period and damping ratio of each spring.");
        out.open("const M = {");
        for token in &style.motion {
            out.line(format!(
                "{}: [{}, {}],",
                member(&token.name),
                number(token.period),
                number(token.damping)
            ));
        }
        out.close("};");
        out.line("// Transitions of `appear` and `disappear`.");
        out.open("const X = {");
        for transition in &style.transitions {
            let motion = format!("M.{}", member(&transition.motion));
            let kind = match &transition.kind {
                bern_ir::style::TransitionKind::Fade => "{ kind: \"fade\"".to_string(),
                bern_ir::style::TransitionKind::Grow { edge } => format!(
                    "{{ kind: \"grow\", edge: {}",
                    string(&format!("{edge:?}").to_lowercase())
                ),
                bern_ir::style::TransitionKind::Pop { from } => {
                    format!("{{ kind: \"pop\", from: {}", number(*from))
                }
                bern_ir::style::TransitionKind::Slide { edge, distance } => format!(
                    "{{ kind: \"slide\", edge: {}, distance: {}",
                    string(&format!("{edge:?}").to_lowercase()),
                    number(*distance)
                ),
            };
            out.line(format!(
                "{}: {kind}, motion: {motion} }},",
                member(&transition.name)
            ));
        }
        out.close("};");
        out.line("// Patterns of `style!`.");
        out.open("const P = {");
        for pattern in &style.patterns {
            let bands: Vec<_> = pattern
                .bands
                .iter()
                .map(|band| {
                    format!(
                        "{{ color: {}, width: {}, opacity: {} }}",
                        band.color
                            .as_ref()
                            .map(color_ref)
                            .unwrap_or_else(|| "null".into()),
                        number(band.width),
                        number(band.opacity)
                    )
                })
                .collect();
            out.line(format!(
                "{}: {{ bands: [{}], twill: {} }},",
                member(&pattern.name),
                bands.join(", "),
                pattern.twill
            ));
        }
        out.close("};");
        out.line("// Style overrides of `style!`, as this target shows them.");
        out.open("const S = {");
        for style in crate::style::used(self.app, crate::style::PLATFORM) {
            out.line(format!("{},", crate::style::declaration(&style)));
        }
        out.close("};");
        out.line("");
    }

    /// The page's `app.js`. `version` is appended to the URLs it loads, so
    /// a rebuilt page never runs with a cached runtime or module.
    pub fn generate(
        &self,
        fingerprint: u64,
        strings: &serde_json::Value,
        wasm: &str,
        mode: &str,
        version: &str,
    ) -> String {
        let app = self.app;
        let types = self.types();
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("");
        out.line(format!("import * as B from \"./bern.js?v={version}\";"));
        out.line(format!("import * as F from \"./features.js?v={version}\";"));
        out.line("");
        self.style_tables(&mut out);
        out.line("// Values: codecs, variant names and labels.");
        out.line("const T = {};");
        out.line("const N = {};");
        out.line("const L = {};");
        out.line("");
        for def in &types.enums {
            generate_enum(&mut out, def);
        }
        for def in &types.records {
            generate_record(&mut out, def);
        }

        out.line("// Engine state, mirrored from Rust.");
        out.open("const E = {");
        for field in &app.protocol.state {
            let initial = Value::default_for(&field.ty, types).unwrap_or(Value::Unit);
            out.line(format!(
                "{}: B.cell({}),",
                field.name,
                literal(&initial, &field.ty, types)
            ));
        }
        out.close("};");
        out.open("const engine = {");
        out.open("apply(field, r) {");
        out.open("switch (field) {");
        for (index, field) in app.protocol.state.iter().enumerate() {
            out.line(format!(
                "case {index}: E.{}.set({}); break;",
                field.name,
                decode(&field.ty)
            ));
        }
        out.line("default: throw new B.DecodeError(`unknown state field ${field}`);");
        out.close("}");
        out.close("},");
        out.close("};");
        out.line("");

        out.line("// Engine commands.");
        out.open("const C = {");
        for (index, command) in app.protocol.commands.iter().enumerate() {
            if command.is_query() {
                continue;
            }
            let params: Vec<_> = command
                .params
                .iter()
                .map(|param| local(&param.name))
                .collect();
            let encoders: Vec<_> = command
                .params
                .iter()
                .map(|param| format!("{};", encode(&param.ty, &local(&param.name))))
                .collect();
            let mut signature = vec!["ctx".to_string()];
            signature.extend(params);
            signature.push("done".into());
            out.line(format!(
                "{}: ({}) => B.dispatch(ctx, {index}, (w) => {{ {} }}, done),",
                app.protocol.command_symbol(command),
                signature.join(", "),
                encoders.join(" ")
            ));
        }
        out.close("};");
        out.line("");

        out.line("// Components.");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }

        let sessions: Vec<String> = app
            .sessions
            .iter()
            .map(|session| {
                let fields: Vec<_> = session
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{{ name: {}, codec: {} }}",
                            string(&field.name),
                            codec(&field.ty)
                        )
                    })
                    .collect();
                format!(
                    "{{ name: {}, fields: [{}] }}",
                    string(&session.name),
                    fields.join(", ")
                )
            })
            .collect();
        let windows: Vec<String> = app
            .windows
            .windows
            .iter()
            .map(|window| {
                let settings = window.role == WindowRole::Settings;
                // The main window is the page and other windows are
                // dialogs; only an explicit `floating` gets a window frame.
                let presentation = if window.presentation == Presentation::Floating {
                    "floating"
                } else if window.id == app.windows.primary {
                    "page"
                } else {
                    "dialog"
                };
                let optional = |value: Option<f64>| value.map(number).unwrap_or("null".into());
                let immersive = window
                    .immersive
                    .as_ref()
                    .map(|expr| format!("(ctx) => ({})", self.expr(expr)))
                    .unwrap_or_else(|| "null".into());
                let idle_enabled = window.idle_enabled.as_ref().map(|e| format!("(ctx) => ({})", self.expr(e))).unwrap_or_else(|| "null".into());
                let landscape = window.landscape.as_ref().map(|e| format!("(ctx) => ({})", self.expr(e))).unwrap_or_else(|| "null".into());
                format!(
                    "{{ id: {}, title: {}, presentation: \"{presentation}\", settings: {settings}, resizable: {}, width: {}, height: {}, minWidth: {}, minHeight: {}, initial: {}, render: w_{}, closeCheck: {}, idleAfter: {}, idleEnabled: {idle_enabled}, immersive: {immersive}, landscape: {landscape} }}",
                    string(&window.id),
                    string(&window.title.key),
                    !settings,
                    optional(window.width),
                    optional(window.height),
                    optional(window.min_width),
                    optional(window.min_height),
                    window.initial == WindowInitial::Open,
                    window.id,
                    window
                        .close
                        .as_ref()
                        .map(|close| self.close_check(close))
                        .unwrap_or_else(|| "null".into()),
                    optional(window.idle_after)
                )
            })
            .collect();
        let used = self.icons();
        let icons: Vec<_> = used
            .iter()
            .filter_map(|name| {
                icons::svg(name).map(|svg| format!("{}: {}", string(name), string(&svg)))
            })
            .collect();
        out.line("// File kinds of `files:`, by extension.");
        let kinds: Vec<String> = bern_ir::spec::FILE_KIND_EXTENSIONS
            .iter()
            .map(|(kind, extensions)| {
                let list: Vec<String> = extensions.iter().map(|ext| string(ext)).collect();
                format!("{kind}: [{}]", list.join(", "))
            })
            .collect();
        out.line(format!("F.fileKinds({{ {} }});", kinds.join(", ")));
        let accent = app
            .windows
            .accent
            .as_ref()
            .map(|expr| format!("() => {}", self.variant_name(expr)));
        let appearance = app
            .windows
            .appearance
            .as_ref()
            .map(|expr| format!("() => {}", self.variant_name(expr)));
        if accent.is_some() || appearance.is_some() {
            out.line("// The accent and appearance the application chooses.");
            out.line(format!(
                "F.theme({}, {});",
                accent.unwrap_or_else(|| "null".into()),
                appearance.unwrap_or_else(|| "null".into())
            ));
        }
        out.line("// Startup.");
        out.open("B.start({");
        out.line(format!("mode: {},", string(mode)));
        out.line(format!("wasm: {},", string(wasm)));
        out.line(format!("fingerprint: 0x{fingerprint:016x}n,"));
        out.line("resources: \"resources/\",");
        out.line(format!(
            "languages: {},",
            serde_json::to_string(&app.languages.supported).expect("languages")
        ));
        out.line(format!(
            "defaultLanguage: {},",
            string(&app.languages.default)
        ));
        out.line(format!("primary: {},", string(&app.windows.primary)));
        if app.layouts.is_some() {
            out.line(format!("layouts: {},", self.breakpoints()));
        }
        out.line(format!("strings: {strings},"));
        out.line(
            "// Icons: Lucide (ISC; icons derived from Feather MIT), see THIRD_PARTY_NOTICES.txt.",
        );
        out.line(format!("icons: {{ {} }},", icons.join(", ")));
        out.line("engine,");
        out.line(format!("sessions: [{}],", sessions.join(", ")));
        out.open("windows: [");
        for window in windows {
            out.line(format!("{window},"));
        }
        out.close("],");
        out.close("});");
        out.finish()
    }

    /// Every icon the page draws: declared ones and the runtime's own.
    pub fn icons(&self) -> BTreeSet<String> {
        let mut used: BTreeSet<String> = icons::RUNTIME_ICONS
            .iter()
            .map(|name| name.to_string())
            .collect();
        let mut visit = |node: &Node| {
            if let Node::Element(element) = node {
                for prop in element.props.values() {
                    if let Prop::Icon { name } = prop {
                        used.insert(name.clone());
                    }
                }
            }
        };
        for component in &self.app.components {
            component.body.walk(&mut visit);
        }
        for window in &self.app.windows.windows {
            window.content.walk(&mut visit);
        }
        used
    }
}
