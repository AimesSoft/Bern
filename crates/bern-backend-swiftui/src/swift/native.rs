//! Code generation for `NativeView` elements in SwiftUI.
//!
//! For each `NativeView` with a `swiftui` implementation file, Bern generates
//! strongly typed `<Id>NativeProps` and `<Id>NativeActions` structs plus a
//! read-only `BernNativeTheme` helper. The co-located `.swift` file defines
//! `struct <Id>NativeView: View`. Declared control entries (`NativeAction`,
//! `NativeNumber`, `NativeBool`, `NativeInput`) are automatically registered
//! with `BernControls` so Bern's CLI (`ui inspect`, `ui activate`,
//! `ui set-number`, `ui set-bool`, `ui edit`, `ui submit`) works identically
//! without piercing the native view.

use super::values::swift_type;
use super::{Ctx, Generator, Out};
use crate::names::{double, member, string};
use bern_ir::native::{
    CollectedNativeView, NativeEntryInfo, NativeEntryKind, NativeViewInfo, camel_ident,
    collect_native_views, inspect_native_view, pascal_ident,
};
use bern_ir::{Action, Element};
use bern_protocol::Type;

impl Generator<'_> {
    /// Emit `BernNativeTheme`, `BernNativeHost`, and the typed `Props`/`Actions`
    /// definitions for every `NativeView` with a `swiftui` file.
    pub(super) fn native_view_types(&self, out: &mut Out) {
        let views: Vec<CollectedNativeView<'_>> = collect_native_views(self.app)
            .into_iter()
            .filter(|v| v.info.file_for("swiftui").is_some())
            .collect();
        if views.is_empty() {
            return;
        }

        out.line("// MARK: - Native Views");
        out.line("");
        out.open("struct BernNativeTheme {");
        out.line("let dark: Bool");
        out.line("let accent: Color");
        out.line("let surface: Color");
        out.line("let textPrimary: Color");
        out.line("let textSecondary: Color");
        out.line("let border: Color");
        out.close("}");
        out.line("");
        out.open("struct BernNativeHost<Content: View>: View {");
        out.line("@Environment(\\.colorScheme) private var colorScheme");
        out.line("let content: (BernNativeTheme) -> Content");
        out.open("var body: some View {");
        out.open("content(BernNativeTheme(");
        out.line("dark: colorScheme == .dark,");
        out.line("accent: BernColor.role(\"accent\").color,");
        out.line("surface: BernColor.role(\"surface.canvas\").color,");
        out.line("textPrimary: BernColor.role(\"text.primary\").color,");
        out.line("textSecondary: BernColor.role(\"text.secondary\").color,");
        out.line("border: BernColor.role(\"control.border\").color");
        out.close("))");
        out.close("}");
        out.close("}");
        out.line("");

        let mut emitted = std::collections::BTreeSet::new();
        for view in &views {
            let base = pascal_ident(view.info.id);
            if !emitted.insert(base.clone()) {
                continue;
            }
            self.emit_native_structs(out, &base, &view.info);
        }
    }

    fn emit_native_structs(&self, out: &mut Out, base: &str, info: &NativeViewInfo<'_>) {
        out.open(format!("struct {base}NativeProps {{"));
        for val in &info.values {
            out.line(format!(
                "let {}: {}",
                member(&camel_ident(val.name)),
                swift_type(&val.expr.ty)
            ));
        }
        for text in &info.texts {
            out.line(format!("let {}: String", member(&camel_ident(text.name))));
        }
        for draft in &info.drafts {
            out.line(format!(
                "let {}: {}",
                member(&camel_ident(draft.name)),
                swift_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let label_field = member(&camel_ident(&format!("{}_label", entry.id)));
            out.line(format!("let {label_field}: String"));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number { min, max, step, .. } => {
                    let field = member(&camel_ident(entry.id));
                    out.line(format!("let {field}: Double"));
                    out.line(format!("let {}Min: Double = {}", field, double(min)));
                    out.line(format!("let {}Max: Double = {}", field, double(max)));
                    if let Some(step) = step {
                        out.line(format!("let {}Step: Double = {}", field, double(step)));
                    }
                }
                NativeEntryKind::Bool { .. } => {
                    out.line(format!("let {}: Bool", member(&camel_ident(entry.id))));
                }
                NativeEntryKind::Input { .. } => {
                    out.line(format!("let {}: String", member(&camel_ident(entry.id))));
                }
            }
        }
        out.close("}");
        out.line("");

        out.open(format!("struct {base}NativeActions {{"));
        for draft in &info.drafts {
            let setter = member(&camel_ident(&format!("set_{}", draft.name)));
            out.line(format!(
                "let {setter}: @MainActor ({}) -> Void",
                swift_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            match entry.kind {
                NativeEntryKind::Action => {
                    let fname = member(&camel_ident(entry.id));
                    out.line(format!("let {fname}: @MainActor () -> Void"));
                }
                NativeEntryKind::Number { .. } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    out.line(format!("let {setter}: @MainActor (Double) -> Void"));
                }
                NativeEntryKind::Bool { .. } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    out.line(format!("let {setter}: @MainActor (Bool) -> Void"));
                }
                NativeEntryKind::Input { .. } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    out.line(format!("let {setter}: @MainActor (String) -> Void"));
                }
            }
        }
        out.close("}");
        out.line("");
    }

    pub(super) fn native_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let Some(info) = inspect_native_view(element) else {
            return;
        };

        if info.file_for("swiftui").is_none() {
            if let Some(fallback) = info.fallback {
                out.open("SwiftUI.Group {");
                self.nodes(out, fallback, ctx);
                out.close("}");
                self.context_menu(out, element, ctx);
                if let Some(events) = self.pointer_modifier(element, true) {
                    out.line(format!(".modifier({events})"));
                }
            } else {
                out.line("SwiftUI.EmptyView()");
            }
            return;
        }

        let base = pascal_ident(info.id);
        out.open("BernNativeHost { bernNativeTheme in");
        out.open(format!("{base}NativeView("));

        // Build Props initializer
        out.open(format!("props: {base}NativeProps("));
        let mut prop_lines = Vec::new();
        for val in &info.values {
            prop_lines.push(format!(
                "{}: {}",
                member(&camel_ident(val.name)),
                self.expr(val.expr)
            ));
        }
        for text in &info.texts {
            prop_lines.push(format!(
                "{}: {}",
                member(&camel_ident(text.name)),
                self.text(text.text)
            ));
        }
        for draft in &info.drafts {
            let field = self.field_ref(draft.field);
            prop_lines.push(format!("{}: {field}", member(&camel_ident(draft.name))));
        }
        for entry in &info.entries {
            let label_field = member(&camel_ident(&format!("{}_label", entry.id)));
            prop_lines.push(format!("{label_field}: {}", self.text(entry.label)));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number { current, .. } => {
                    let expr = if current.ty == Type::Int {
                        format!("Double({})", self.expr(current))
                    } else {
                        self.expr(current)
                    };
                    prop_lines.push(format!("{}: {expr}", member(&camel_ident(entry.id))));
                }
                NativeEntryKind::Bool { checked } => {
                    prop_lines.push(format!(
                        "{}: {}",
                        member(&camel_ident(entry.id)),
                        self.expr(checked)
                    ));
                }
                NativeEntryKind::Input { current } => {
                    prop_lines.push(format!(
                        "{}: {}",
                        member(&camel_ident(entry.id)),
                        self.expr(current)
                    ));
                }
            }
        }
        for (idx, line) in prop_lines.iter().enumerate() {
            let comma = if idx + 1 < prop_lines.len() { "," } else { "" };
            out.line(format!("{line}{comma}"));
        }
        out.close("),");

        // Build Actions initializer
        out.open(format!("actions: {base}NativeActions("));
        let mut action_lines = Vec::new();
        for draft in &info.drafts {
            let setter = member(&camel_ident(&format!("set_{}", draft.name)));
            let field = self.field_ref(draft.field);
            action_lines.push(format!("{setter}: {{ newValue in {field} = newValue }}"));
        }
        for entry in &info.entries {
            match entry.kind {
                NativeEntryKind::Action => {
                    let fname = member(&camel_ident(entry.id));
                    action_lines.push(format!("{fname}: {{ {} }}", self.action(entry.action)));
                }
                NativeEntryKind::Number {
                    current, min, max, ..
                } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    let commit = self.native_controlled_commit(entry.action, current, "clamped");
                    action_lines.push(format!(
                        "{setter}: {{ rawValue in let clamped = min(max(rawValue, {}), {}); {commit} }}",
                        double(min),
                        double(max)
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    let commit = self.native_controlled_commit(entry.action, checked, "newValue");
                    action_lines.push(format!("{setter}: {{ newValue in {commit} }}"));
                }
                NativeEntryKind::Input { current } => {
                    let setter = member(&camel_ident(&format!("set_{}", entry.id)));
                    let commit = self.native_controlled_commit(entry.action, current, "newValue");
                    action_lines.push(format!("{setter}: {{ newValue in {commit} }}"));
                }
            }
        }
        for (idx, line) in action_lines.iter().enumerate() {
            let comma = if idx + 1 < action_lines.len() {
                ","
            } else {
                ""
            };
            out.line(format!("{line}{comma}"));
        }
        out.close("),");

        out.line("theme: bernNativeTheme");
        out.close(")");
        out.close("}");
        match (
            element.prop("width").is_none(),
            ctx.in_stack && element.prop("height").is_none(),
        ) {
            (true, true) => {
                out.line(".frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)")
            }
            (true, false) => out.line(".frame(maxWidth: .infinity, alignment: .leading)"),
            (false, true) => out.line(".frame(maxHeight: .infinity, alignment: .leading)"),
            (false, false) => {}
        }

        // Register every declared Bern entry in `BernControls` via non-visual
        // background anchors so multiple entries never collide on accessibilityIdentifier.
        out.open(".background {");
        out.open("SwiftUI.ZStack {");
        for entry in &info.entries {
            self.emit_native_control_anchor(out, info.id, entry, ctx);
        }
        out.close("}");
        out.line(".allowsHitTesting(false)");
        out.close("}");

        self.context_menu(out, element, ctx);
        if let Some(events) = self.pointer_modifier(element, true) {
            out.line(format!(".modifier({events})"));
        }
    }

    fn emit_native_control_anchor(
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
            .map(|e| format!(", enabled: {}", self.expr(e)))
            .unwrap_or_default();
        let actions = match entry.kind {
            NativeEntryKind::Action => {
                format!(
                    "BernControlActions(activate: {{ {} }})",
                    self.action(entry.action)
                )
            }
            NativeEntryKind::Number {
                current, min, max, ..
            } => {
                let commit = self.native_controlled_commit(entry.action, current, "clamped");
                let cur_expr = self.expr(current);
                format!(
                    "BernControlActions(setNumber: {{ rawValue in let clamped = min(max(rawValue, {}), {}); {commit} }}, value: {{ BernText.text({cur_expr}) }})",
                    double(min),
                    double(max)
                )
            }
            NativeEntryKind::Bool { checked } => {
                let commit = self.native_controlled_commit(entry.action, checked, "newValue");
                let cur_expr = self.expr(checked);
                format!(
                    "BernControlActions(setBool: {{ newValue in {commit} }}, value: {{ {cur_expr} ? \"true\" : \"false\" }})"
                )
            }
            NativeEntryKind::Input { current } => {
                let commit = self.native_controlled_commit(entry.action, current, "newValue");
                let cur_expr = self.expr(current);
                format!(
                    "BernControlActions(edit: {{ newValue in {commit} }}, submit: {{ {} }}, value: {{ {cur_expr} }})",
                    self.action(entry.action)
                )
            }
        };
        out.line(format!(
            "SwiftUI.Color.clear.bernControl({target}, role: {}, label: {label}{enabled}, actions: {actions})",
            string(entry.role())
        ));
    }

    fn native_controlled_commit(
        &self,
        action: &Action,
        controlled: &bern_ir::Expr,
        param_name: &str,
    ) -> String {
        match action {
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values: Vec<_> = args
                    .iter()
                    .map(|arg| {
                        if arg != controlled {
                            self.expr(arg)
                        } else if arg.ty == Type::Int {
                            format!("Int64({param_name}.rounded())")
                        } else {
                            param_name.to_string()
                        }
                    })
                    .collect();
                values.push("from: bernWindow".into());
                format!(
                    "bernEngine.{}({})",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                )
            }
            Action::Session { field, op } => {
                let target = self.field_ref(field);
                match op {
                    bern_ir::SessionOp::Set { value } if value == controlled => {
                        if field.ty == Type::Int {
                            format!("{target} = Int64({param_name}.rounded())")
                        } else {
                            format!("{target} = {param_name}")
                        }
                    }
                    _ => self.action(action),
                }
            }
            _ => self.action(action),
        }
    }

    fn field_ref(&self, field: &bern_ir::FieldRef) -> String {
        let session = self.app.session(&field.session).expect("session");
        format!(
            "{}.{}",
            super::values::session_var(&field.session),
            member(&session.fields[field.field].name)
        )
    }
}
