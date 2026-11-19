//! Code generation for `NativeView` elements in GPUI.
//!
//! Each `NativeView` with a `gpui` implementation file gets a dedicated module
//! `mod native_<symbol>` containing strongly typed `Props`, `Actions` and
//! `Theme` structs plus `#[path = "<abs_path>"] mod impl_;` pointing directly
//! at the co-located `.gpui.rs` file (falling back to inlined source in
//! diskless unit tests). Declared Bern entries are registered synchronously
//! during render via `bern_gpui::register_native` so `windows::controls(cx)`
//! collects them even when the window is not being painted.

use super::{Generator, field_ref, float, string};
use bern_ir::native::{
    CollectedNativeView, NativeEntryInfo, NativeEntryKind, NativeViewInfo, collect_native_views,
    find_native_source, inspect_native_view, snake_ident, view_symbol,
};
use bern_ir::{Action, Element, Expr, SourceLocation};
use bern_protocol::Type;

fn rust_type(ty: &Type) -> &'static str {
    match ty {
        Type::Bool => "bool",
        Type::Int => "i64",
        Type::Float => "f64",
        Type::Text => "String",
        Type::Optional { inner } => match inner.as_ref() {
            Type::Int => "Option<i64>",
            Type::Float => "Option<f64>",
            Type::Text => "Option<String>",
            _ => "Option<f64>",
        },
        _ => "f64",
    }
}

fn unpack_rust(expr_val: &str, ty: &Type) -> String {
    match ty {
        Type::Bool => format!("rt::truthy(&({expr_val}))"),
        Type::Int => format!("rt::as_f64(&({expr_val})).round() as i64"),
        Type::Float => format!("rt::as_f64(&({expr_val}))"),
        Type::Text => format!("rt::plain(&({expr_val}))"),
        Type::Optional { inner } => {
            let map = match inner.as_ref() {
                Type::Int => "|v| rt::as_f64(&v).round() as i64",
                Type::Float => "|v| rt::as_f64(&v)",
                Type::Text => "|v| rt::plain(&v)",
                _ => "|v| rt::as_f64(&v)",
            };
            format!("rt::some({expr_val}).map({map})")
        }
        _ => format!("rt::as_f64(&({expr_val}))"),
    }
}

fn pack_rust(var: &str, ty: &Type) -> String {
    match ty {
        Type::Bool => format!("V::Bool({var})"),
        Type::Int => format!("V::Int({var})"),
        Type::Float => format!("V::Float({var})"),
        Type::Text => format!("V::Text({var})"),
        Type::Optional { inner } => match inner.as_ref() {
            Type::Float => format!("{var}.map(|n| V::some(V::Float(n))).unwrap_or_else(V::none)"),
            Type::Int => format!("{var}.map(|n| V::some(V::Int(n))).unwrap_or_else(V::none)"),
            Type::Text => format!("{var}.map(|s| V::some(V::Text(s))).unwrap_or_else(V::none)"),
            _ => "V::none()".into(),
        },
        _ => format!("V::Float({var})"),
    }
}

impl Generator<'_> {
    pub(super) fn native_view_modules(&self) -> String {
        let views: Vec<CollectedNativeView<'_>> = collect_native_views(self.app)
            .into_iter()
            .filter(|v| v.info.file_for("gpui").is_some())
            .collect();
        if views.is_empty() {
            return String::new();
        }
        let mut out = String::from("// Co-located native UI views.\n");
        let mut emitted = std::collections::BTreeSet::new();
        for view in &views {
            if !emitted.insert(view.symbol.clone()) {
                continue;
            }
            let Some(path) = view.info.file_for("gpui") else {
                continue;
            };
            out.push_str(&format!("mod native_{} {{\n", view.symbol));
            out.push_str("    use bern_gpui::gpui;\n");
            out.push_str("    use std::rc::Rc;\n");
            self.emit_native_types(&mut out, &view.info);
            let disk_path = self
                .project_dir
                .map(|dir| dir.join(path))
                .filter(|p| p.exists());
            if let Some(real) = disk_path {
                let slash = real.display().to_string().replace('\\', "/");
                out.push_str(&format!("    #[path = {}]\n", string(&slash)));
                out.push_str("    mod impl_;\n");
            } else if let Some(def) = find_native_source(self.app, path) {
                out.push_str("    mod impl_ {\n");
                for line in def.source.lines() {
                    out.push_str("        ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push_str("    }\n");
            }
            out.push_str("    pub use impl_::render;\n");
            out.push_str("}\n\n");
        }
        out
    }

    fn emit_native_types(&self, out: &mut String, info: &NativeViewInfo<'_>) {
        out.push_str("    #[derive(Clone, Copy)]\n");
        out.push_str("    pub struct Theme {\n");
        out.push_str("        pub dark: bool,\n");
        out.push_str("        pub accent: gpui::Rgba,\n");
        out.push_str("        pub surface: gpui::Rgba,\n");
        out.push_str("        pub text_primary: gpui::Rgba,\n");
        out.push_str("        pub text_secondary: gpui::Rgba,\n");
        out.push_str("        pub border: gpui::Rgba,\n");
        out.push_str("    }\n");

        out.push_str("    #[derive(Clone)]\n");
        out.push_str("    pub struct Props {\n");
        for val in &info.values {
            out.push_str(&format!(
                "        pub {}: {},\n",
                snake_ident(val.name),
                rust_type(&val.expr.ty)
            ));
        }
        for text in &info.texts {
            out.push_str(&format!(
                "        pub {}: String,\n",
                snake_ident(text.name)
            ));
        }
        for draft in &info.drafts {
            out.push_str(&format!(
                "        pub {}: {},\n",
                snake_ident(draft.name),
                rust_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            out.push_str(&format!("        pub {sid}_label: String,\n"));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number { .. } => {
                    out.push_str(&format!("        pub {sid}: f64,\n"));
                    out.push_str(&format!("        pub {sid}_min: f64,\n"));
                    out.push_str(&format!("        pub {sid}_max: f64,\n"));
                    out.push_str(&format!("        pub {sid}_step: Option<f64>,\n"));
                }
                NativeEntryKind::Bool { .. } => {
                    out.push_str(&format!("        pub {sid}: bool,\n"));
                }
                NativeEntryKind::Input { .. } => {
                    out.push_str(&format!("        pub {sid}: String,\n"));
                }
            }
        }
        out.push_str("    }\n");

        out.push_str("    #[derive(Clone)]\n");
        out.push_str("    pub struct Actions {\n");
        for draft in &info.drafts {
            let name = snake_ident(draft.name);
            out.push_str(&format!(
                "        pub set_{name}: Rc<dyn Fn({})>,\n",
                rust_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            match entry.kind {
                NativeEntryKind::Action => {
                    out.push_str(&format!("        pub {sid}: Rc<dyn Fn()>,\n"));
                }
                NativeEntryKind::Number { .. } => {
                    out.push_str(&format!("        pub set_{sid}: Rc<dyn Fn(f64)>,\n"));
                }
                NativeEntryKind::Bool { .. } => {
                    out.push_str(&format!("        pub set_{sid}: Rc<dyn Fn(bool)>,\n"));
                }
                NativeEntryKind::Input { .. } => {
                    out.push_str(&format!("        pub set_{sid}: Rc<dyn Fn(String)>,\n"));
                }
            }
        }
        out.push_str("    }\n");
    }

    pub(super) fn native_view_code(&self, element: &Element) -> Option<String> {
        let info = inspect_native_view(element)?;
        let at = Some(&element.source);
        if info.file_for("gpui").is_none() {
            let fallback = info.fallback?;
            return Some(format!(
                "b::column(c, b::Flex::default(), {})",
                self.children(fallback, super::Axis::Column)
            ));
        }
        let scope = self.current_scope.borrow().clone();
        let sym = view_symbol(&scope, info.id);
        let ns = format!("native_{sym}");

        let mut code = String::from("{ ");
        code.push_str(&format!("let _props = {ns}::Props {{ "));
        for val in &info.values {
            let field = snake_ident(val.name);
            let expr = self.expr(val.expr, at);
            code.push_str(&format!("{field}: {}, ", unpack_rust(&expr, &val.expr.ty)));
        }
        for text in &info.texts {
            let field = snake_ident(text.name);
            code.push_str(&format!("{field}: {}, ", self.text(text.text, at)));
        }
        for draft in &info.drafts {
            let field = snake_ident(draft.name);
            let s = self.session_index(&draft.field.session);
            let read = format!("c.session({s}, {})", draft.field.field);
            code.push_str(&format!(
                "{field}: {}, ",
                unpack_rust(&read, &draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            code.push_str(&format!("{sid}_label: {}, ", self.text(entry.label, at)));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number {
                    current,
                    min,
                    max,
                    step,
                } => {
                    let expr = self.expr(current, at);
                    code.push_str(&format!("{sid}: rt::as_f64(&({expr})), "));
                    code.push_str(&format!("{sid}_min: {}, ", float(min)));
                    code.push_str(&format!("{sid}_max: {}, ", float(max)));
                    code.push_str(&format!(
                        "{sid}_step: {}, ",
                        step.map(|s| format!("Some({})", float(s)))
                            .unwrap_or_else(|| "None".into())
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let expr = self.expr(checked, at);
                    code.push_str(&format!("{sid}: rt::truthy(&({expr})), "));
                }
                NativeEntryKind::Input { current } => {
                    let expr = self.expr(current, at);
                    code.push_str(&format!("{sid}: rt::plain(&({expr})), "));
                }
            }
        }
        code.push_str("}; ");

        code.push_str(&format!("let _actions = {ns}::Actions {{ "));
        for draft in &info.drafts {
            let name = snake_ident(draft.name);
            let fref = field_ref(self.session_index(&draft.field.session), draft.field.field);
            let packed = pack_rust("v", &draft.field.ty);
            code.push_str(&format!(
                "set_{name}: {{ let c = c.clone(); std::rc::Rc::new(move |v| {{ b::session_set(&c, {fref}, {packed}); }}) }}, "
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            match entry.kind {
                NativeEntryKind::Action => {
                    let body = self.action_body(entry.action, at);
                    code.push_str(&format!(
                        "{sid}: {{ let c = c.clone(); std::rc::Rc::new(move || {{ let c = &c; {body}; }}) }}, "
                    ));
                }
                NativeEntryKind::Number {
                    current, min, max, ..
                } => {
                    let body = self.native_controlled_body(entry.action, current, "clamped", at);
                    code.push_str(&format!(
                        "set_{sid}: {{ let c = c.clone(); std::rc::Rc::new(move |raw: f64| {{ let c = &c; let clamped = raw.clamp({}, {}); {body}; }}) }}, ",
                        float(min),
                        float(max)
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let body = self.native_controlled_body(entry.action, checked, "val", at);
                    code.push_str(&format!(
                        "set_{sid}: {{ let c = c.clone(); std::rc::Rc::new(move |val: bool| {{ let c = &c; {body}; }}) }}, "
                    ));
                }
                NativeEntryKind::Input { current } => {
                    let body = self.native_controlled_body(entry.action, current, "val", at);
                    code.push_str(&format!(
                        "set_{sid}: {{ let c = c.clone(); std::rc::Rc::new(move |val: String| {{ let c = &c; {body}; }}) }}, "
                    ));
                }
            }
        }
        code.push_str("}; ");

        // Register every declared Bern functional entry synchronously for CLI access.
        for entry in &info.entries {
            code.push_str(&self.emit_native_control(info.id, entry, at));
        }

        code.push_str(&format!(
            "let _theme = {ns}::Theme {{ dark: c.theme_is_dark(), accent: c.theme_color(\"accent\"), surface: c.theme_color(\"surface.canvas\"), text_primary: c.theme_color(\"text.primary\"), text_secondary: c.theme_color(\"text.secondary\"), border: c.theme_color(\"control.border\") }}; "
        ));
        if element.prop("width").is_none() {
            code.push_str(&format!(
                "bern_gpui::gpui::IntoElement::into_any_element(bern_gpui::gpui::Styled::w_full(bern_gpui::gpui::ParentElement::child(bern_gpui::gpui::div(), {ns}::render(_props, _actions, _theme)))) }}"
            ));
        } else {
            code.push_str(&format!(
                "bern_gpui::gpui::IntoElement::into_any_element({ns}::render(_props, _actions, _theme)) }}"
            ));
        }
        Some(code)
    }

    fn emit_native_control(
        &self,
        view_id: &str,
        entry: &NativeEntryInfo<'_>,
        at: Option<&SourceLocation>,
    ) -> String {
        let local_id = string(&entry.local_target(view_id));
        let role = string(entry.role());
        let sid = snake_ident(entry.id);
        let enabled = entry
            .enabled
            .map(|e| self.condition(e))
            .unwrap_or_else(|| "true".into());
        let (value, actions) = match entry.kind {
            NativeEntryKind::Action => (
                "None".to_string(),
                format!(
                    "b::NativeControlActions {{ activate: Some(_actions.{sid}.clone()), ..Default::default() }}"
                ),
            ),
            NativeEntryKind::Number { .. } => (
                format!("Some(rt::plain(&V::Float(_props.{sid})))"),
                format!(
                    "b::NativeControlActions {{ set_number: Some(_actions.set_{sid}.clone()), ..Default::default() }}"
                ),
            ),
            NativeEntryKind::Bool { .. } => (
                format!(
                    "Some(if _props.{sid} {{ \"true\".to_string() }} else {{ \"false\".to_string() }})"
                ),
                format!(
                    "b::NativeControlActions {{ set_bool: Some(_actions.set_{sid}.clone()), ..Default::default() }}"
                ),
            ),
            NativeEntryKind::Input { .. } => {
                let submit = self.action_body(entry.action, at);
                (
                    format!("Some(_props.{sid}.clone())"),
                    format!(
                        "b::NativeControlActions {{ edit: Some(_actions.set_{sid}.clone()), submit: Some({{ let c = c.clone(); std::rc::Rc::new(move || {{ let c = &c; {submit}; }}) }}), ..Default::default() }}"
                    ),
                )
            }
        };
        format!(
            "b::register_native(c, {local_id}, {role}, _props.{sid}_label.clone(), {value}, {enabled}, {actions}); "
        )
    }

    fn native_controlled_body(
        &self,
        action: &Action,
        controlled: &Expr,
        var: &str,
        at: Option<&SourceLocation>,
    ) -> String {
        let pack_var = |ty: &Type| match ty {
            Type::Int => format!("V::Int({var}.round() as i64)"),
            Type::Float => format!("V::Float({var})"),
            Type::Bool => format!("V::Bool({var})"),
            Type::Text => format!("V::Text({var}.clone())"),
            _ => format!("V::Float({var})"),
        };
        match action {
            Action::Engine { command, args } => {
                let values: Vec<_> = args
                    .iter()
                    .map(|arg| {
                        if arg == controlled {
                            pack_var(&arg.ty)
                        } else {
                            self.expr(arg, at)
                        }
                    })
                    .collect();
                format!("b::dispatch(c, {command}, vec![{}])", values.join(", "))
            }
            Action::Session { field, op } => {
                let target = field_ref(self.session_index(&field.session), field.field);
                match op {
                    bern_ir::SessionOp::Set { value } if value == controlled => {
                        format!("b::session_set(c, {target}, {})", pack_var(&field.ty))
                    }
                    _ => self.action_body(action, at),
                }
            }
            _ => self.action_body(action, at),
        }
    }
}
