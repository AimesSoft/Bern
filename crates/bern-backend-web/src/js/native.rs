//! Code generation for `NativeView` elements in Web (`web + browser`).
//!
//! Each `NativeView` providing a `web` implementation file is emitted as a
//! standalone ES module `native/<symbol>.js` and statically imported by
//! `app.js`. The module exports `mount({ props, actions, theme, effect, onCleanup })`
//! returning a DOM `Node` (or appending into `host`). Declared Bern control
//! entries are registered via `B.control` automatically so the CLI works
//! identically.

use super::{Generator, Out, number, string};
use bern_ir::native::{
    NativeEntryInfo, NativeEntryKind, camel_ident, collect_native_views, find_native_source,
    inspect_native_view, view_symbol,
};
use bern_ir::{Action, Application, Element, Expr};
use bern_protocol::Type;
use bern_target_api::GeneratedFile;
use std::path::Path;

/// Emit `native/<symbol>.js` files for every `NativeView` with a `web` source.
pub fn web_native_files(app: &Application, project_dir: &Path) -> Vec<GeneratedFile> {
    let mut files = Vec::new();
    let mut emitted = std::collections::BTreeSet::new();
    for view in collect_native_views(app) {
        let Some(rel_path) = view.info.file_for("web") else {
            continue;
        };
        if !emitted.insert(view.symbol.clone()) {
            continue;
        }
        let source = std::fs::read_to_string(project_dir.join(rel_path))
            .ok()
            .or_else(|| find_native_source(app, rel_path).map(|f| f.source.clone()))
            .unwrap_or_default();
        files.push(GeneratedFile::text(
            format!("native/{}.js", view.symbol),
            source,
        ));
    }
    files
}

impl Generator<'_> {
    /// Emit ES module imports for all `web` native views using a custom path prefix.
    pub(super) fn native_imports_from(&self, out: &mut Out, prefix: &str) {
        let mut emitted = std::collections::BTreeSet::new();
        for view in collect_native_views(self.app) {
            if view.info.file_for("web").is_none() || !emitted.insert(view.symbol.clone()) {
                continue;
            }
            out.line(format!(
                "import * as N_{} from \"{prefix}{}.js\";",
                view.symbol, view.symbol
            ));
        }
    }

    /// Emit top-level ES module imports for all `web` native views in `app.js`.
    pub(super) fn native_imports(&self, out: &mut Out, version: &str) {
        let mut emitted = std::collections::BTreeSet::new();
        for view in collect_native_views(self.app) {
            if view.info.file_for("web").is_none() || !emitted.insert(view.symbol.clone()) {
                continue;
            }
            out.line(format!(
                "import * as N_{} from \"./native/{}.js?v={version}\";",
                view.symbol, view.symbol
            ));
        }
    }

    pub(super) fn native_view(&self, out: &mut Out, element: &Element) {
        let Some(info) = inspect_native_view(element) else {
            return;
        };
        if info.file_for("web").is_none() {
            if let Some(fallback) = info.fallback {
                self.nodes(out, fallback);
            }
            return;
        }
        let scope = self.current_scope.borrow().clone();
        let sym = view_symbol(&scope, info.id);

        out.open("{");
        out.line("const _host = document.createElement(\"div\");");
        out.line("_host.className = \"bern-native-view\";");
        out.line(format!("B.mark(_host, ctx, {});", string(info.id)));
        out.line("p.append(_host);");

        out.open("const _props = {");
        for val in &info.values {
            let key = camel_ident(val.name);
            let expr = self.expr(val.expr);
            let expr = if val.expr.ty == Type::Int {
                format!("Number({expr})")
            } else if matches!(&val.expr.ty, Type::Optional { inner } if **inner == Type::Int) {
                format!("((v) => v == null ? null : Number(v))({expr})")
            } else if matches!(&val.expr.ty, Type::List { inner } if **inner == Type::Int) {
                format!("({expr}).map(Number)")
            } else {
                expr
            };
            out.line(format!("{key}: () => ({expr}),"));
        }
        for text in &info.texts {
            let key = camel_ident(text.name);
            out.line(format!("{key}: () => ({}),", self.text(text.text)));
        }
        for draft in &info.drafts {
            let key = camel_ident(draft.name);
            let cell = self.native_field_cell(draft.field);
            let read = if draft.field.ty == Type::Int {
                format!("Number({cell}.get())")
            } else if matches!(&draft.field.ty, Type::List { inner } if **inner == Type::Int) {
                format!("{cell}.get().map(Number)")
            } else {
                format!("{cell}.get()")
            };
            out.line(format!("{key}: () => ({read}),"));
        }
        for entry in &info.entries {
            let key = camel_ident(entry.id);
            let label_key = camel_ident(&format!("{}_label", entry.id));
            out.line(format!("{label_key}: () => ({}),", self.text(entry.label)));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number {
                    current,
                    min,
                    max,
                    step,
                } => {
                    let expr = self.expr(current);
                    out.line(format!("{key}: () => Number({expr}),"));
                    out.line(format!("{key}Min: {},", number(min)));
                    out.line(format!("{key}Max: {},", number(max)));
                    if let Some(step) = step {
                        out.line(format!("{key}Step: {},", number(step)));
                    }
                }
                NativeEntryKind::Bool { checked } => {
                    out.line(format!("{key}: () => Boolean({}),", self.expr(checked)));
                }
                NativeEntryKind::Input { current } => {
                    out.line(format!("{key}: () => String({}),", self.expr(current)));
                }
            }
        }
        out.close("};");

        out.open("const _actions = {");
        for draft in &info.drafts {
            let setter = camel_ident(&format!("set_{}", draft.name));
            let cell = self.native_field_cell(draft.field);
            let pack = if draft.field.ty == Type::Int {
                "BigInt(Math.round(Number(v)))"
            } else if matches!(&draft.field.ty, Type::List { inner } if **inner == Type::Int) {
                "(v ?? []).map((n) => BigInt(Math.round(Number(n))))"
            } else {
                "v"
            };
            out.line(format!("{setter}: (v) => {cell}.set({pack}),"));
        }
        for entry in &info.entries {
            match entry.kind {
                NativeEntryKind::Action => {
                    let fname = camel_ident(entry.id);
                    out.line(format!(
                        "{fname}: () => {{ {}; }},",
                        self.action(entry.action)
                    ));
                }
                NativeEntryKind::Number {
                    current, min, max, ..
                } => {
                    let setter = camel_ident(&format!("set_{}", entry.id));
                    let commit = self.native_controlled_action(entry.action, current, "clamped");
                    out.line(format!(
                        "{setter}: (raw) => {{ const clamped = Math.min(Math.max(Number(raw), {}), {}); {commit}; }},",
                        number(min),
                        number(max)
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let setter = camel_ident(&format!("set_{}", entry.id));
                    let commit = self.native_controlled_action(entry.action, checked, "val");
                    out.line(format!(
                        "{setter}: (raw) => {{ const val = Boolean(raw); {commit}; }},"
                    ));
                }
                NativeEntryKind::Input { current } => {
                    let setter = camel_ident(&format!("set_{}", entry.id));
                    let commit = self.native_controlled_action(entry.action, current, "val");
                    out.line(format!(
                        "{setter}: (raw) => {{ const val = String(raw); {commit}; }},"
                    ));
                }
            }
        }
        out.close("};");

        out.open("const _theme = {");
        out.line("dark: () => { F.themeTick.get(); return getComputedStyle(_host).colorScheme.includes(\"dark\"); },");
        out.line("role: (role) => { F.themeTick.get(); return F.color(_host, { role }); },");
        out.line(
            "accent: () => { F.themeTick.get(); return F.color(_host, { role: \"accent\" }); },",
        );
        out.line("surface: () => { F.themeTick.get(); return F.color(_host, { role: \"surface.canvas\" }); },");
        out.line("textPrimary: () => { F.themeTick.get(); return F.color(_host, { role: \"text.primary\" }); },");
        out.line("textSecondary: () => { F.themeTick.get(); return F.color(_host, { role: \"text.secondary\" }); },");
        out.line("border: () => { F.themeTick.get(); return F.color(_host, { role: \"control.border\" }); },");
        out.close("};");

        out.line(format!(
            "const _mounted = N_{sym}.mount({{ host: _host, props: _props, actions: _actions, theme: _theme, effect: B.effect, onCleanup: B.onCleanup }});"
        ));
        out.line("if (_mounted instanceof Node && _mounted.parentNode !== _host) _host.append(_mounted);");

        for entry in &info.entries {
            self.emit_native_control(out, info.id, entry);
        }
        out.close("}");
    }

    fn emit_native_control(&self, out: &mut Out, view_id: &str, entry: &NativeEntryInfo<'_>) {
        let local_id = string(&entry.local_target(view_id));
        let role = string(entry.role());
        let label_key = camel_ident(&format!("{}_label", entry.id));
        let key = camel_ident(entry.id);
        let enabled = match entry.enabled {
            Some(expr) => format!("() => Boolean({})", self.expr(expr)),
            None => "() => true".into(),
        };
        let actions = match entry.kind {
            NativeEntryKind::Action => format!("{{ activate: _actions.{key} }}"),
            NativeEntryKind::Number { .. } => {
                let setter = camel_ident(&format!("set_{}", entry.id));
                format!("{{ setNumber: _actions.{setter}, value: () => String(_props.{key}()) }}")
            }
            NativeEntryKind::Bool { .. } => {
                let setter = camel_ident(&format!("set_{}", entry.id));
                format!(
                    "{{ setBool: _actions.{setter}, value: () => (_props.{key}() ? \"true\" : \"false\") }}"
                )
            }
            NativeEntryKind::Input { .. } => {
                let setter = camel_ident(&format!("set_{}", entry.id));
                format!(
                    "{{ edit: _actions.{setter}, submit: () => {{ {}; }}, value: () => _props.{key}() }}",
                    self.action(entry.action)
                )
            }
        };
        out.line(format!(
            "B.control(ctx, {{ target: () => B.target(ctx, {local_id}), role: {role}, label: _props.{label_key}, enabled: {enabled}, actions: {actions} }});"
        ));
    }

    fn native_controlled_action(&self, action: &Action, controlled: &Expr, var: &str) -> String {
        match action {
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values = vec!["ctx".to_string()];
                for arg in args {
                    if arg == controlled {
                        if arg.ty == Type::Int {
                            values.push(format!("BigInt(Math.round(Number({var})))"));
                        } else {
                            values.push(var.to_string());
                        }
                    } else {
                        values.push(self.expr(arg));
                    }
                }
                format!(
                    "C.{}({})",
                    self.app.protocol.command_symbol(spec),
                    values.join(", ")
                )
            }
            Action::Session { field, op } => {
                let cell = self.native_field_cell(field);
                match op {
                    bern_ir::SessionOp::Set { value } if value == controlled => {
                        if field.ty == Type::Int {
                            format!("{cell}.set(BigInt(Math.round(Number({var}))))")
                        } else {
                            format!("{cell}.set({var})")
                        }
                    }
                    _ => self.action(action),
                }
            }
            _ => self.action(action),
        }
    }

    fn native_field_cell(&self, field: &bern_ir::FieldRef) -> String {
        let session = self.app.session(&field.session).expect("session");
        format!(
            "ctx.s.{}.{}",
            field.session, session.fields[field.field].name
        )
    }
}
