//! Code generation for `NativeView` elements in GTK 4.
//!
//! Each `NativeView` with a `gtk` implementation file gets a dedicated module
//! `mod native_<symbol>` containing strongly typed `Props`, `Actions` and
//! `Theme` structs plus `#[path = "<abs_path>"] mod impl_;` pointing directly
//! at the co-located `.gtk.rs` file (falling back to inlined source in
//! diskless unit tests). Declared Bern entries are registered in
//! `bern_gtk::register_native` automatically so the CLI works out of the box.

use super::{Generator, Out, float, string};
use bern_ir::native::{
    CollectedNativeView, NativeEntryInfo, NativeEntryKind, NativeViewInfo, collect_native_views,
    find_native_source, inspect_native_view, snake_ident,
};
use bern_ir::{Action, Element, Expr};
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
    /// Emit per-view `mod native_<symbol>` modules at the top of `app/src/ui.rs`.
    pub(super) fn native_view_modules(&self, out: &mut Out) {
        let views: Vec<CollectedNativeView<'_>> = collect_native_views(self.app)
            .into_iter()
            .filter(|v| v.info.file_for("gtk").is_some())
            .collect();
        if views.is_empty() {
            return;
        }
        out.line("// Co-located native UI views.");
        let mut emitted = std::collections::BTreeSet::new();
        for view in &views {
            if !emitted.insert(view.symbol.clone()) {
                continue;
            }
            let Some(path) = view.info.file_for("gtk") else {
                continue;
            };
            out.open(format!("mod native_{} {{", view.symbol));
            out.line("use bern_gtk as b;");
            out.line("use bern_gtk::gtk;");
            out.line("use std::rc::Rc;");
            self.emit_native_types(out, &view.info);
            let disk_path = self
                .project_dir
                .map(|dir| dir.join(path))
                .filter(|p| p.exists());
            if let Some(real) = disk_path {
                let slash = real.display().to_string().replace('\\', "/");
                out.line(format!("#[path = {}]", string(&slash)));
                out.line("mod impl_;");
            } else if let Some(def) = find_native_source(self.app, path) {
                out.open("mod impl_ {");
                for line in def.source.lines() {
                    out.line(line);
                }
                out.close("}");
            }
            out.line("pub use impl_::build;");
            out.close("}");
            out.line("");
        }
    }

    fn emit_native_types(&self, out: &mut Out, info: &NativeViewInfo<'_>) {
        out.line("#[derive(Clone)]");
        out.open("pub struct Theme;");
        out.open("impl Theme {");
        out.line("pub fn is_dark(&self) -> bool { b::theme::track(); b::theme::is_dark() }");
        out.line("pub fn role(&self, role: &'static str, widget: &gtk::Widget) -> gtk::gdk::RGBA { b::theme::track(); b::theme::rgba(&b::Color::Role(role), widget) }");
        out.line("pub fn accent(&self, widget: &gtk::Widget) -> gtk::gdk::RGBA { self.role(\"accent\", widget) }");
        out.line("pub fn surface(&self, widget: &gtk::Widget) -> gtk::gdk::RGBA { self.role(\"surface.canvas\", widget) }");
        out.line("pub fn text_primary(&self, widget: &gtk::Widget) -> gtk::gdk::RGBA { self.role(\"text.primary\", widget) }");
        out.line("pub fn text_secondary(&self, widget: &gtk::Widget) -> gtk::gdk::RGBA { self.role(\"text.secondary\", widget) }");
        out.line("pub fn border(&self, widget: &gtk::Widget) -> gtk::gdk::RGBA { self.role(\"control.border\", widget) }");
        out.close("}");

        out.line("#[derive(Clone)]");
        out.open("pub struct Props {");
        for val in &info.values {
            out.line(format!(
                "pub {}: Rc<dyn Fn() -> {}>,",
                snake_ident(val.name),
                rust_type(&val.expr.ty)
            ));
        }
        for text in &info.texts {
            out.line(format!(
                "pub {}: Rc<dyn Fn() -> String>,",
                snake_ident(text.name)
            ));
        }
        for draft in &info.drafts {
            out.line(format!(
                "pub {}: Rc<dyn Fn() -> {}>,",
                snake_ident(draft.name),
                rust_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            out.line(format!("pub {sid}_label: Rc<dyn Fn() -> String>,"));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number { .. } => {
                    out.line(format!("pub {sid}: Rc<dyn Fn() -> f64>,"));
                    out.line(format!("pub {sid}_min: f64,"));
                    out.line(format!("pub {sid}_max: f64,"));
                    out.line(format!("pub {sid}_step: Option<f64>,"));
                }
                NativeEntryKind::Bool { .. } => {
                    out.line(format!("pub {sid}: Rc<dyn Fn() -> bool>,"));
                }
                NativeEntryKind::Input { .. } => {
                    out.line(format!("pub {sid}: Rc<dyn Fn() -> String>,"));
                }
            }
        }
        out.close("}");

        out.line("#[derive(Clone)]");
        out.open("pub struct Actions {");
        for draft in &info.drafts {
            let name = snake_ident(draft.name);
            out.line(format!(
                "pub set_{name}: Rc<dyn Fn({})>,",
                rust_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            match entry.kind {
                NativeEntryKind::Action => {
                    out.line(format!("pub {sid}: Rc<dyn Fn()>,"));
                }
                NativeEntryKind::Number { .. } => {
                    out.line(format!("pub set_{sid}: Rc<dyn Fn(f64)>,"));
                }
                NativeEntryKind::Bool { .. } => {
                    out.line(format!("pub set_{sid}: Rc<dyn Fn(bool)>,"));
                }
                NativeEntryKind::Input { .. } => {
                    out.line(format!("pub set_{sid}: Rc<dyn Fn(String)>,"));
                }
            }
        }
        out.close("}");
    }

    pub(super) fn native_view(&self, out: &mut Out, element: &Element) {
        let Some(info) = inspect_native_view(element) else {
            return;
        };
        if info.file_for("gtk").is_none() {
            if let Some(fallback) = info.fallback {
                self.nodes(out, fallback);
            }
            return;
        }
        let scope = self.current_scope.borrow().clone();
        let sym = bern_ir::native::view_symbol(&scope, info.id);
        let ns = format!("native_{sym}");

        out.open("{");
        out.open(format!("let _props = {ns}::Props {{"));
        for val in &info.values {
            let field = snake_ident(val.name);
            let expr = self.expr(val.expr);
            let unpacked = unpack_rust(&expr, &val.expr.ty);
            out.line(format!(
                "{field}: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; {unpacked} }}) }},"
            ));
        }
        for text in &info.texts {
            let field = snake_ident(text.name);
            let txt = self.text(text.text);
            out.line(format!(
                "{field}: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; {txt} }}) }},"
            ));
        }
        for draft in &info.drafts {
            let field = snake_ident(draft.name);
            let s = self.session_index(&draft.field.session);
            let read = format!("c.session({s}, {})", draft.field.field);
            let unpacked = unpack_rust(&read, &draft.field.ty);
            out.line(format!(
                "{field}: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; {unpacked} }}) }},"
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            let lbl = self.text(entry.label);
            out.line(format!(
                "{sid}_label: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; {lbl} }}) }},"
            ));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number {
                    current,
                    min,
                    max,
                    step,
                } => {
                    let expr = self.expr(current);
                    out.line(format!(
                        "{sid}: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; rt::as_f64(&({expr})) }}) }},"
                    ));
                    out.line(format!("{sid}_min: {},", float(min)));
                    out.line(format!("{sid}_max: {},", float(max)));
                    out.line(format!(
                        "{sid}_step: {},",
                        step.map(|s| format!("Some({})", float(s)))
                            .unwrap_or_else(|| "None".into())
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let expr = self.expr(checked);
                    out.line(format!(
                        "{sid}: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; rt::truthy(&({expr})) }}) }},"
                    ));
                }
                NativeEntryKind::Input { current } => {
                    let expr = self.expr(current);
                    out.line(format!(
                        "{sid}: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; rt::plain(&({expr})) }}) }},"
                    ));
                }
            }
        }
        out.close("};");

        out.open(format!("let _actions = {ns}::Actions {{"));
        for draft in &info.drafts {
            let name = snake_ident(draft.name);
            let fref = self.field_ref(draft.field);
            let packed = pack_rust("v", &draft.field.ty);
            out.line(format!(
                "set_{name}: {{ let c = c.clone(); Rc::new(move |v| {{ b::session_set(&c, {fref}, {packed}); }}) }},"
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            match entry.kind {
                NativeEntryKind::Action => {
                    let body = self.action_body(entry.action);
                    out.line(format!(
                        "{sid}: {{ let c = c.clone(); Rc::new(move || {{ let c = &c; {body}; }}) }},"
                    ));
                }
                NativeEntryKind::Number {
                    current, min, max, ..
                } => {
                    let body = self.native_controlled_body(entry.action, current, "clamped");
                    out.line(format!(
                        "set_{sid}: {{ let c = c.clone(); Rc::new(move |raw: f64| {{ let c = &c; let clamped = raw.clamp({}, {}); {body}; }}) }},",
                        float(min),
                        float(max)
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let body = self.native_controlled_body(entry.action, checked, "val");
                    out.line(format!(
                        "set_{sid}: {{ let c = c.clone(); Rc::new(move |val: bool| {{ let c = &c; {body}; }}) }},"
                    ));
                }
                NativeEntryKind::Input { current } => {
                    let body = self.native_controlled_body(entry.action, current, "val");
                    out.line(format!(
                        "set_{sid}: {{ let c = c.clone(); Rc::new(move |val: String| {{ let c = &c; {body}; }}) }},"
                    ));
                }
            }
        }
        out.close("};");

        out.line(format!(
            "let _widget = {ns}::build(_props.clone(), _actions.clone(), {ns}::Theme);"
        ));
        if element.prop("width").is_none() {
            out.line("{ use bern_gtk::gtk::prelude::WidgetExt as _; _widget.set_hexpand(true); }");
        }
        out.line("p.add(&_widget);");

        for entry in &info.entries {
            self.emit_native_control_registration(out, info.id, entry);
        }
        out.close("}");
    }

    fn emit_native_control_registration(
        &self,
        out: &mut Out,
        view_id: &str,
        entry: &NativeEntryInfo<'_>,
    ) {
        let local_id = string(&entry.local_target(view_id));
        let role = string(entry.role());
        let sid = snake_ident(entry.id);
        let enabled = match entry.enabled {
            Some(expr) => {
                let e = self.expr(expr);
                format!(
                    "Some({{ let c = c.clone(); Rc::new(move || {{ let c = &c; rt::truthy(&({e})) }}) }})"
                )
            }
            None => "None".into(),
        };
        let actions = match entry.kind {
            NativeEntryKind::Action => format!(
                "b::NativeControlActions {{ activate: Some(_actions.{sid}.clone()), ..Default::default() }}"
            ),
            NativeEntryKind::Number { .. } => format!(
                "b::NativeControlActions {{ set_number: Some(_actions.set_{sid}.clone()), value: Some({{ let p = _props.{sid}.clone(); Rc::new(move || Some(rt::plain(&V::Float(p())))) }}), ..Default::default() }}"
            ),
            NativeEntryKind::Bool { .. } => format!(
                "b::NativeControlActions {{ set_bool: Some(_actions.set_{sid}.clone()), value: Some({{ let p = _props.{sid}.clone(); Rc::new(move || Some(if p() {{ \"true\".to_string() }} else {{ \"false\".to_string() }})) }}), ..Default::default() }}"
            ),
            NativeEntryKind::Input { .. } => {
                let submit = self.action_body(entry.action);
                format!(
                    "b::NativeControlActions {{ edit: Some(_actions.set_{sid}.clone()), submit: Some({{ let c = c.clone(); Rc::new(move || {{ let c = &c; {submit}; }}) }}), value: Some({{ let p = _props.{sid}.clone(); Rc::new(move || Some(p())) }}), ..Default::default() }}"
                )
            }
        };
        out.line(format!(
            "b::register_native(c, {local_id}, {role}, Some(_props.{sid}_label.clone()), {enabled}, {actions});"
        ));
    }

    fn native_controlled_body(&self, action: &Action, controlled: &Expr, var: &str) -> String {
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
                            self.expr(arg)
                        }
                    })
                    .collect();
                format!(
                    "b::dispatch(c, {command}, vec![{}], None)",
                    values.join(", ")
                )
            }
            Action::Session { field, op } => {
                let target = self.field_ref(field);
                match op {
                    bern_ir::SessionOp::Set { value } if value == controlled => {
                        format!("b::session_set(c, {target}, {})", pack_var(&field.ty))
                    }
                    _ => self.action_body(action),
                }
            }
            _ => self.action_body(action),
        }
    }
}
