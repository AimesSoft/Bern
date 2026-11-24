//! Code generation for `NativeView` elements in Jetpack Compose (Android / Wear OS).
//!
//! Each `NativeView` providing a `compose` implementation file receives its own
//! Kotlin sub-package `<app_package>.native_<symbol>` containing strongly typed
//! `Theme`, `Props`, `Actions` data classes plus the user's co-located
//! `.compose.kt` source (with any user `import` lines hoisted cleanly after the
//! package declaration). Declared Bern entries are automatically registered in
//! `BernControl(...)` so the CLI works out of the box.

use super::values::{kotlin_type, session_var};
use super::{Ctx, Generator, Out};
use crate::names::{double, member, string};
use bern_ir::native::{
    NativeEntryInfo, NativeEntryKind, NativeViewInfo, camel_ident, collect_native_views,
    find_native_source, inspect_native_view, view_symbol,
};
use bern_ir::{Action, Application, Element, Expr};
use bern_protocol::Type;
use std::path::{Path, PathBuf};

/// One generated Kotlin file for a co-located `*.compose.kt` view.
pub struct ComposeNativeFile {
    /// Path relative to `project/app/src/main/kotlin`.
    pub rel_kotlin_path: PathBuf,
    pub contents: String,
}

/// Emit all per-view Kotlin files for `NativeView` elements with a `compose` file.
pub fn compose_native_files(
    app: &Application,
    package: &str,
    project_dir: &Path,
) -> Vec<ComposeNativeFile> {
    let mut out_files = Vec::new();
    let mut emitted = std::collections::BTreeSet::new();
    let pkg_dir = package.replace('.', "/");

    for view in collect_native_views(app) {
        let Some(rel_path) = view.info.file_for("compose") else {
            continue;
        };
        if !emitted.insert(view.symbol.clone()) {
            continue;
        }
        let user_source = std::fs::read_to_string(project_dir.join(rel_path))
            .ok()
            .or_else(|| find_native_source(app, rel_path).map(|f| f.source.clone()))
            .unwrap_or_default();

        let (user_imports, user_body) = split_kotlin_header(&user_source);
        let sub_pkg = format!("{package}.native_{}", view.symbol);

        let mut out = Out::default();
        out.line("// Generated wrapper + co-located Compose native view.");
        out.line("@file:Suppress(\"UNUSED_PARAMETER\", \"unused\", \"RedundantSuppression\")");
        out.line(format!("package {sub_pkg}"));
        out.line("");
        out.line("import androidx.compose.ui.graphics.Color");
        for imp in user_imports {
            out.line(imp);
        }
        out.line("");
        emit_kotlin_types(&mut out, &view.info);
        out.line("");
        for line in user_body {
            out.line(line);
        }

        out_files.push(ComposeNativeFile {
            rel_kotlin_path: PathBuf::from(&pkg_dir)
                .join(format!("native_{}", view.symbol))
                .join("NativeContent.kt"),
            contents: out.finish(),
        });
    }
    out_files
}

fn split_kotlin_header(source: &str) -> (Vec<String>, Vec<String>) {
    let mut imports = Vec::new();
    let mut body = Vec::new();
    let mut in_header = true;
    for line in source.lines() {
        let trimmed = line.trim();
        if in_header {
            if trimmed.starts_with("package ") {
                continue;
            }
            if trimmed.starts_with("import ") {
                imports.push(trimmed.to_string());
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            in_header = false;
        }
        body.push(line.to_string());
    }
    (imports, body)
}

fn emit_kotlin_types(out: &mut Out, info: &NativeViewInfo<'_>) {
    out.open("data class Theme(");
    out.line("val dark: Boolean,");
    out.line("val accent: Color,");
    out.line("val surface: Color,");
    out.line("val textPrimary: Color,");
    out.line("val textSecondary: Color,");
    out.line("val border: Color,");
    out.close(")");
    out.line("");

    let mut prop_fields = Vec::new();
    for val in &info.values {
        prop_fields.push(format!(
            "val {}: {}",
            member(&camel_ident(val.name)),
            kotlin_type(&val.expr.ty)
        ));
    }
    for text in &info.texts {
        prop_fields.push(format!("val {}: String", member(&camel_ident(text.name))));
    }
    for draft in &info.drafts {
        prop_fields.push(format!(
            "val {}: {}",
            member(&camel_ident(draft.name)),
            kotlin_type(&draft.field.ty)
        ));
    }
    for entry in &info.entries {
        let label_field = member(&camel_ident(&format!("{}_label", entry.id)));
        prop_fields.push(format!("val {label_field}: String"));
        match entry.kind {
            NativeEntryKind::Action => {}
            NativeEntryKind::Number { min, max, step, .. } => {
                let f = member(&camel_ident(entry.id));
                prop_fields.push(format!("val {f}: Double"));
                prop_fields.push(format!("val {f}Min: Double = {}", double(min)));
                prop_fields.push(format!("val {f}Max: Double = {}", double(max)));
                if let Some(step) = step {
                    prop_fields.push(format!("val {f}Step: Double = {}", double(step)));
                }
            }
            NativeEntryKind::Bool { .. } => {
                prop_fields.push(format!("val {}: Boolean", member(&camel_ident(entry.id))));
            }
            NativeEntryKind::Input { .. } => {
                prop_fields.push(format!("val {}: String", member(&camel_ident(entry.id))));
            }
        }
    }
    if prop_fields.is_empty() {
        out.line("class Props");
    } else {
        out.open("data class Props(");
        for field in &prop_fields {
            out.line(format!("{field},"));
        }
        out.close(")");
    }
    out.line("");

    out.open("class Actions(");
    for draft in &info.drafts {
        let setter = member(&camel_ident(&format!("set_{}", draft.name)));
        out.line(format!(
            "val {setter}: ({}) -> Unit,",
            kotlin_type(&draft.field.ty)
        ));
    }
    for entry in &info.entries {
        match entry.kind {
            NativeEntryKind::Action => {
                let fname = member(&camel_ident(entry.id));
                out.line(format!("val {fname}: () -> Unit,"));
            }
            NativeEntryKind::Number { .. } => {
                let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                out.line(format!("val {setter}: (Double) -> Unit,"));
            }
            NativeEntryKind::Bool { .. } => {
                let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                out.line(format!("val {setter}: (Boolean) -> Unit,"));
            }
            NativeEntryKind::Input { .. } => {
                let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                out.line(format!("val {setter}: (String) -> Unit,"));
            }
        }
    }
    out.close(")");
}

impl Generator<'_> {
    pub(super) fn native_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let Some(info) = inspect_native_view(element) else {
            return;
        };
        if info.file_for("compose").is_none() {
            if let Some(fallback) = info.fallback {
                self.nodes(out, fallback, ctx);
            }
            return;
        }
        let package = self.package.borrow().clone();
        let sym = view_symbol(&ctx.component, info.id);
        let fq = format!("{package}.native_{sym}");

        // Register all declared Bern entries for CLI access.
        for entry in &info.entries {
            self.emit_native_control(out, info.id, entry, ctx);
        }

        out.open("run {");
        out.line("val _bernNativePal = bernPalette()");
        out.open(format!("val _bernNativeTheme = {fq}.Theme("));
        out.line("dark = bernDark(),");
        out.line("accent = _bernNativePal.role(\"accent\"),");
        out.line("surface = _bernNativePal.role(\"surface.canvas\"),");
        out.line("textPrimary = _bernNativePal.role(\"text.primary\"),");
        out.line("textSecondary = _bernNativePal.role(\"text.secondary\"),");
        out.line("border = _bernNativePal.role(\"control.border\"),");
        out.close(")");

        out.open(format!("val _bernNativeProps = {fq}.Props("));
        for val in &info.values {
            out.line(format!(
                "{} = {},",
                member(&camel_ident(val.name)),
                self.expr(val.expr)
            ));
        }
        for text in &info.texts {
            out.line(format!(
                "{} = {},",
                member(&camel_ident(text.name)),
                self.text(text.text)
            ));
        }
        for draft in &info.drafts {
            let fref = self.native_field_ref(draft.field);
            out.line(format!("{} = {fref},", member(&camel_ident(draft.name))));
        }
        for entry in &info.entries {
            let label_field = member(&camel_ident(&format!("{}_label", entry.id)));
            out.line(format!("{label_field} = {},", self.text(entry.label)));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number { current, .. } => {
                    let expr = if current.ty == Type::Int {
                        format!("({}).toDouble()", self.expr(current))
                    } else {
                        self.expr(current)
                    };
                    out.line(format!("{} = {expr},", member(&camel_ident(entry.id))));
                }
                NativeEntryKind::Bool { checked } => {
                    out.line(format!(
                        "{} = {},",
                        member(&camel_ident(entry.id)),
                        self.expr(checked)
                    ));
                }
                NativeEntryKind::Input { current } => {
                    out.line(format!(
                        "{} = {},",
                        member(&camel_ident(entry.id)),
                        self.expr(current)
                    ));
                }
            }
        }
        out.close(")");

        out.open(format!("val _bernNativeActions = {fq}.Actions("));
        for draft in &info.drafts {
            let setter = member(&camel_ident(&format!("set_{}", draft.name)));
            let fref = self.native_field_ref(draft.field);
            out.line(format!("{setter} = {{ v -> {fref} = v }},"));
        }
        for entry in &info.entries {
            match entry.kind {
                NativeEntryKind::Action => {
                    let fname = member(&camel_ident(entry.id));
                    out.line(format!("{fname} = {{ {} }},", self.action(entry.action)));
                }
                NativeEntryKind::Number {
                    current, min, max, ..
                } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    let commit = self.native_controlled_commit(entry.action, current, "clamped");
                    out.line(format!(
                        "{setter} = {{ raw -> val clamped = raw.coerceIn({}, {}); {commit} }},",
                        double(min),
                        double(max)
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    let commit = self.native_controlled_commit(entry.action, checked, "newValue");
                    out.line(format!("{setter} = {{ newValue -> {commit} }},"));
                }
                NativeEntryKind::Input { current } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    let commit = self.native_controlled_commit(entry.action, current, "newValue");
                    out.line(format!("{setter} = {{ newValue -> {commit} }},"));
                }
            }
        }
        out.close(")");

        if ctx.layer && element.prop("width").is_none() && element.prop("height").is_none() {
            out.open("Box(Modifier.fillMaxSize()) {");
            out.line(format!(
                "{fq}.NativeContent(props = _bernNativeProps, actions = _bernNativeActions, theme = _bernNativeTheme)"
            ));
            out.close("}");
        } else if element.prop("width").is_none() && ctx.scope != super::Scope::Row {
            out.open("Box(Modifier.fillMaxWidth()) {");
            out.line(format!(
                "{fq}.NativeContent(props = _bernNativeProps, actions = _bernNativeActions, theme = _bernNativeTheme)"
            ));
            out.close("}");
        } else {
            out.line(format!(
                "{fq}.NativeContent(props = _bernNativeProps, actions = _bernNativeActions, theme = _bernNativeTheme)"
            ));
        }
        out.close("}");
    }

    fn emit_native_control(
        &self,
        out: &mut Out,
        view_id: &str,
        entry: &NativeEntryInfo<'_>,
        ctx: &Ctx,
    ) {
        let target = string(&format!("{}.{}.{}", ctx.component, view_id, entry.id));
        let label = self.text(entry.label);
        let enabled = entry
            .enabled
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        let actions = match entry.kind {
            NativeEntryKind::Action => {
                format!(
                    "BernControlActions(activate = {{ {} }})",
                    self.action(entry.action)
                )
            }
            NativeEntryKind::Number {
                current, min, max, ..
            } => {
                let commit = self.native_controlled_commit(entry.action, current, "clamped");
                let cur = self.expr(current);
                format!(
                    "BernControlActions(setNumber = {{ raw -> val clamped = raw.coerceIn({}, {}); {commit} }}, value = {{ BernText.text({cur}) }})",
                    double(min),
                    double(max)
                )
            }
            NativeEntryKind::Bool { checked } => {
                let commit = self.native_controlled_commit(entry.action, checked, "newValue");
                let cur = self.expr(checked);
                format!(
                    "BernControlActions(setBool = {{ newValue -> {commit} }}, value = {{ if ({cur}) \"true\" else \"false\" }})"
                )
            }
            NativeEntryKind::Input { current } => {
                let commit = self.native_controlled_commit(entry.action, current, "newValue");
                let cur = self.expr(current);
                format!(
                    "BernControlActions(edit = {{ newValue -> {commit} }}, submit = {{ {} }}, value = {{ {cur} }})",
                    self.action(entry.action)
                )
            }
        };
        out.line(format!(
            "BernControl(target = {target}, role = {}, label = {label}, enabled = {enabled}, actions = {actions})",
            string(entry.role())
        ));
    }

    fn native_controlled_commit(&self, action: &Action, controlled: &Expr, param: &str) -> String {
        match action {
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values: Vec<_> = args
                    .iter()
                    .map(|arg| {
                        if arg != controlled {
                            self.expr(arg)
                        } else if arg.ty == Type::Int {
                            format!("kotlin.math.round({param}).toLong()")
                        } else {
                            param.to_string()
                        }
                    })
                    .collect();
                values.push("bernWindow".into());
                format!(
                    "BernEngine.{}({})",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                )
            }
            Action::Session { field, op } => {
                let target = self.native_field_ref(field);
                match op {
                    bern_ir::SessionOp::Set { value } if value == controlled => {
                        if field.ty == Type::Int {
                            format!("{target} = kotlin.math.round({param}).toLong()")
                        } else {
                            format!("{target} = {param}")
                        }
                    }
                    _ => self.action(action),
                }
            }
            _ => self.action(action),
        }
    }

    fn native_field_ref(&self, field: &bern_ir::FieldRef) -> String {
        let session = self.app.session(&field.session).expect("session");
        format!(
            "{}.{}",
            session_var(&field.session),
            member(&session.fields[field.field].name)
        )
    }
}
