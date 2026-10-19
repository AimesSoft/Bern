//! Rust generation from the IR.
//!
//! The generated module builds every component and window from the
//! runtime's standard components (`bern-gtk`). Expressions become Rust
//! expressions over schema values evaluated in a context `c`; locals
//! (component parameters, row variables, pattern bindings) live in the
//! context, so closures capture nothing and each binding re-runs only when
//! a cell it read changes.

use crate::style;
use bern_ir::style::{ColorRef, TransitionKind};
use bern_ir::{Application, Expr, WindowChrome, WindowInitial, WindowRole};
use bern_protocol::{Type, TypeTable, Value};
use std::cell::Cell;

mod content;
mod controls;
mod expr;
mod layout;
mod menus;
mod motion;
mod navigation;
mod nodes;

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
                .push(format!("{}{}", "    ".repeat(self.indent), text));
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

/// A Rust string literal.
pub fn string(value: &str) -> String {
    format!("{value:?}")
}

pub(crate) fn float(value: f64) -> String {
    let text = format!("{value:?}");
    if text.contains('.') || text.contains('e') || text.contains("inf") || text.contains("NaN") {
        text
    } else {
        format!("{text}.0")
    }
}

fn some(value: Option<String>) -> String {
    value
        .map(|value| format!("Some({value})"))
        .unwrap_or_else(|| "None".into())
}

/// A Rust constant name for a style name: `pop_in` → `POP_IN`.
pub(crate) fn constant(prefix: &str, name: &str) -> String {
    let mut out = format!("{prefix}_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

/// A color of `style!` for the runtime.
pub(crate) fn color(color: &ColorRef) -> String {
    match color {
        ColorRef::Role { role } => format!("b::Color::Role({})", string(role)),
        ColorRef::Fixed { light, dark } => format!(
            "b::Color::Fixed(({}, {}, {}), ({}, {}, {}))",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

/// A schema value as a Rust expression.
fn literal(value: &Value) -> String {
    let list = |values: &[Value]| values.iter().map(literal).collect::<Vec<_>>().join(", ");
    match value {
        Value::Unit => "V::Unit".into(),
        Value::Bool(value) => format!("V::Bool({value})"),
        Value::Int(value) => format!("V::Int({value})"),
        Value::Float(value) => format!("V::Float({})", float(*value)),
        Value::Text(value) => format!("V::Text({}.into())", string(value)),
        Value::Enum { variant, fields } => {
            format!(
                "V::Enum {{ variant: {variant}, fields: vec![{}] }}",
                list(fields)
            )
        }
        Value::Optional(None) => "V::Optional(None)".into(),
        Value::Optional(Some(value)) => format!("V::some({})", literal(value)),
        Value::Record(values) => format!("V::Record(vec![{}])", list(values)),
        Value::List(values) => format!("V::List(vec![{}])", list(values)),
    }
}

pub struct Generator<'a> {
    app: &'a Application,
    /// Whether the nodes being generated are a sidebar's items.
    in_sidebar: Cell<bool>,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application) -> Self {
        Self {
            app,
            in_sidebar: Cell::new(false),
        }
    }

    fn types(&self) -> &TypeTable {
        &self.app.types
    }

    fn session_index(&self, name: &str) -> usize {
        self.app
            .sessions
            .iter()
            .position(|session| session.name == name)
            .expect("session")
    }

    /// The motion tokens, transitions, patterns and style overrides of
    /// `style!`.
    fn style_tables(&self, out: &mut Out) {
        let sheet = &self.app.style;
        out.line("// Motion tokens: period and damping ratio of each spring.");
        for token in &sheet.motion {
            out.line(format!(
                "const {}: b::Curve = ({}, {});",
                constant("M", &token.name),
                float(token.period),
                float(token.damping)
            ));
        }
        // Tokens the runtime uses even when the application changes none.
        for (name, period, damping) in bern_ir::style::BUILT_IN_MOTION {
            if sheet.token(name).is_none() {
                out.line(format!(
                    "const {}: b::Curve = ({}, {});",
                    constant("M", name),
                    float(period),
                    float(damping)
                ));
            }
        }
        out.line("");
        out.line("// Transitions of `appear` and `disappear`.");
        for transition in &sheet.transitions {
            let kind = match &transition.kind {
                TransitionKind::Fade => "b::TransitionKind::Fade".to_string(),
                TransitionKind::Grow { edge } => {
                    format!("b::TransitionKind::Grow(b::Edge::{edge:?})")
                }
                TransitionKind::Pop { from } => format!("b::TransitionKind::Pop({})", float(*from)),
                TransitionKind::Slide { edge, distance } => format!(
                    "b::TransitionKind::Slide(b::Edge::{edge:?}, {})",
                    float(*distance)
                ),
            };
            out.line(format!(
                "static {}: b::Transition = b::Transition {{ kind: {kind}, motion: {} }};",
                constant("X", &transition.name),
                self.motion(&transition.motion)
            ));
        }
        out.line("");
        out.line("// Patterns of `style!`.");
        for pattern in &sheet.patterns {
            let bands: Vec<_> = pattern
                .bands
                .iter()
                .map(|band| {
                    format!(
                        "b::Band {{ color: {}, width: {}, opacity: {} }}",
                        some(band.color.as_ref().map(color)),
                        float(band.width),
                        float(band.opacity)
                    )
                })
                .collect();
            out.line(format!(
                "static {}: b::PatternDef = b::PatternDef {{ bands: &[{}], twill: {} }};",
                constant("P", &pattern.name),
                bands.join(", "),
                pattern.twill
            ));
        }
        out.line("");
        out.line("/// Style overrides of `style!`, as this target shows them.");
        out.open("static STYLES: &[b::StyleDef] = &[");
        for resolved in style::used(self.app) {
            out.line(format!("{},", style::declaration(self.app, &resolved)));
        }
        out.close("];");
        out.line("");
    }

    /// The accent and appearance of `WindowSet`, chosen at run time.
    fn theme(&self, out: &mut Out) {
        let windows = &self.app.windows;
        if windows.accent.is_none() && windows.appearance.is_none() {
            out.line("theme: None,");
            return;
        }
        let variants = |expr: &Expr| match &expr.ty {
            Type::Enum { name } => self
                .types()
                .enum_def(name)
                .map(|def| {
                    def.variants
                        .iter()
                        .map(|variant| variant.name.clone())
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let accents: Vec<String> = windows
            .accent
            .as_ref()
            .map(|expr| {
                variants(expr)
                    .iter()
                    .enumerate()
                    .filter_map(|(index, name)| {
                        let accent = self.app.style.accent(name)?;
                        let ColorRef::Fixed { light, dark } = &accent.color else {
                            return None;
                        };
                        Some(format!(
                            "({index}, ({}, {}, {}), ({}, {}, {}))",
                            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let appearances: Vec<String> = windows
            .appearance
            .as_ref()
            .map(|expr| {
                variants(expr)
                    .iter()
                    .enumerate()
                    .map(|(index, name)| format!("({index}, {})", string(&name.to_lowercase())))
                    .collect()
            })
            .unwrap_or_default();
        out.open("theme: Some(b::ThemeDef {");
        out.line(format!(
            "accent: {},",
            some(
                windows
                    .accent
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.expr(expr)))
            )
        ));
        out.line(format!("accents: &[{}],", accents.join(", ")));
        out.line(format!(
            "appearance: {},",
            some(
                windows
                    .appearance
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.expr(expr)))
            )
        ));
        out.line(format!("appearances: &[{}],", appearances.join(", ")));
        out.close("}),");
    }

    /// The application's `ui.rs`.
    pub fn generate(&self, fingerprint: u64, descriptor: &str) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line(
            "#![allow(unused_imports, unused_variables, unused_parens, dead_code, clippy::all)]",
        );
        out.line("");
        out.line("use bern_gtk as b;");
        out.line("use bern_gtk::protocol::Value as V;");
        out.line("use bern_gtk::value as rt;");
        out.line("use std::rc::Rc;");
        out.line("");
        self.style_tables(&mut out);
        out.line("// Components.");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }

        out.line("/// Strings of every supported language.");
        out.open("static STRINGS: b::text::StringTable = &[");
        for (language, table) in bern_target_api::texts::tables(app) {
            out.open(format!("({}, &[", string(&language)));
            for (key, text) in table {
                out.line(format!("({}, {}),", string(&key), string(&text)));
            }
            out.close("]),");
        }
        out.close("];");
        out.line("");

        out.line("/// The application, as the runtime sees it.");
        out.open("pub fn app() -> b::AppDef {");
        out.open("b::AppDef {");
        out.line(format!("id: {},", string(&app.app.id)));
        out.line(format!("bundle_id: {},", string(&app.app.bundle_id)));
        out.line("name: \"app.name\",");
        out.line(format!("version: {},", string(&app.app.version)));
        out.line(format!("fingerprint: 0x{fingerprint:016x},"));
        let languages: Vec<_> = app
            .languages
            .supported
            .iter()
            .map(|language| string(language))
            .collect();
        out.line(format!("languages: &[{}],", languages.join(", ")));
        out.line(format!(
            "default_language: {},",
            string(&app.languages.default)
        ));
        out.line("strings: STRINGS,");
        out.line(format!("descriptor: ({descriptor}).clone(),"));
        out.line(format!(
            "types: {},",
            bern_target_api::glue::types_expr(&app.types)
        ));
        out.open("sessions: vec![");
        for session in &app.sessions {
            let fields: Vec<_> = session
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "({}, {}, {})",
                        string(&field.name),
                        bern_target_api::glue::type_expr(&field.ty),
                        literal(&field.initial)
                    )
                })
                .collect();
            out.line(format!(
                "b::SessionDef {{ name: {}, fields: vec![{}] }},",
                string(&session.name),
                fields.join(", ")
            ));
        }
        out.close("],");
        out.open("windows: vec![");
        for window in &app.windows.windows {
            let optional = |value: Option<f64>| some(value.map(float));
            let sessions: Vec<_> = window
                .sessions
                .iter()
                .map(|name| self.session_index(name).to_string())
                .collect();
            let immersive = some(
                window
                    .immersive
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.condition(expr))),
            );
            let idle_enabled = some(
                window
                    .idle_enabled
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.condition(expr))),
            );
            out.line(format!(
                "b::WindowDef {{ id: {}, title: {}, settings: {}, width: {}, height: {}, min_width: {}, min_height: {}, initial_open: {}, sessions: vec![{}], render: w_{}, close: {}, dialog: {}, unified: {}, idle_after: {}, immersive: {immersive}, idle_enabled: {idle_enabled} }},",
                string(&window.id),
                string(&window.title.key),
                window.role == WindowRole::Settings,
                optional(window.width),
                optional(window.height),
                optional(window.min_width),
                optional(window.min_height),
                window.initial == WindowInitial::Open,
                sessions.join(", "),
                window.id,
                if window.close.is_some() {
                    format!("Some(close_{})", window.id)
                } else {
                    "None".into()
                },
                window.role == WindowRole::Dialog,
                window.chrome == WindowChrome::Unified,
                optional(window.idle_after),
            ));
        }
        out.close("],");
        out.line(format!("primary: {},", string(&app.windows.primary)));
        let players: Vec<_> = app
            .players
            .iter()
            .map(|player| string(&player.id))
            .collect();
        out.line(format!("players: vec![{}],", players.join(", ")));
        out.line(format!(
            "compact_below: {},",
            some(app.layouts.map(|layouts| float(layouts.compact_below)))
        ));
        out.line(format!(
            "medium_below: {},",
            some(
                app.layouts
                    .and_then(|layouts| layouts.medium_below)
                    .map(float)
            )
        ));
        out.line(format!(
            "expanded_below: {},",
            some(
                app.layouts
                    .and_then(|layouts| layouts.expanded_below)
                    .map(float)
            )
        ));
        out.line(format!(
            "brand_seed: {},",
            some(
                app.theme
                    .brand_seed
                    .map(|seed| format!("({}, {}, {})", seed.red, seed.green, seed.blue))
            )
        ));
        out.line("styles: STYLES,");
        // The weights and sizes `text { … }` gives text styles.
        let text_styles: Vec<String> = app
            .style
            .text_weights
            .iter()
            .map(|style| {
                format!(
                    "({}, {}, {})",
                    string(&style.style),
                    string(&style.weight),
                    some(style.size.map(float))
                )
            })
            .collect();
        out.line(format!("text_styles: &[{}],", text_styles.join(", ")));
        self.theme(&mut out);
        let kinds: Vec<String> = bern_ir::spec::FILE_KIND_EXTENSIONS
            .iter()
            .map(|(kind, extensions)| {
                let extensions: Vec<String> = extensions.iter().map(|ext| string(ext)).collect();
                format!("({}, &[{}])", string(kind), extensions.join(", "))
            })
            .collect();
        out.line(format!("file_kinds: &[{}],", kinds.join(", ")));
        out.close("}");
        out.close("}");
        out.finish()
    }
}
