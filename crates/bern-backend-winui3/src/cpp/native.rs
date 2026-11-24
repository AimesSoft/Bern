//! Code generation for `NativeView` elements in WinUI 3 / XAML Islands.
//!
//! Each `NativeView` with a `winui3` implementation file gets its own
//! `namespace native_<symbol>` inside `app_ui.inl` (compiled for both
//! `bern::winui` and `bern::islands`), containing strongly typed `Props`,
//! `Actions` and `Theme` structs plus `#include "<real_path>"` pointing
//! directly at the co-located `.winui3.cpp` file (falling back to inlined
//! source in diskless unit tests). Declared Bern entries are registered in
//! `Controls::Register` automatically so the CLI works out of the box.

use super::{Generator, Out, float, std_string, string};
use bern_ir::native::{
    CollectedNativeView, NativeEntryInfo, NativeEntryKind, NativeViewInfo, collect_native_views,
    find_native_source, inspect_native_view, snake_ident,
};
use bern_ir::{Action, Element, Expr};
use bern_protocol::Type;

fn cpp_type(ty: &Type) -> &'static str {
    match ty {
        Type::Bool => "bool",
        Type::Int => "int64_t",
        Type::Float => "double",
        Type::Text => "winrt::hstring",
        Type::Optional { inner } => match inner.as_ref() {
            Type::Int => "std::optional<int64_t>",
            Type::Float => "std::optional<double>",
            Type::Text => "std::optional<winrt::hstring>",
            _ => "std::optional<double>",
        },
        Type::List { inner } => match inner.as_ref() {
            Type::Bool => "std::vector<bool>",
            Type::Int => "std::vector<int64_t>",
            Type::Text => "std::vector<winrt::hstring>",
            _ => "std::vector<double>",
        },
        _ => "double",
    }
}

fn unpack_cpp(expr_val: &str, ty: &Type) -> String {
    match ty {
        Type::Bool => format!("R::Truthy({expr_val})"),
        Type::Int => format!("static_cast<int64_t>(std::llround(R::AsDouble({expr_val})))"),
        Type::Float => format!("R::AsDouble({expr_val})"),
        Type::Text => format!("winrt::to_hstring(R::Plain({expr_val}))"),
        Type::Optional { inner } => {
            let some = match inner.as_ref() {
                Type::Int => "static_cast<int64_t>(std::llround(R::AsDouble(*opt)))",
                Type::Float => "R::AsDouble(*opt)",
                Type::Text => "winrt::to_hstring(R::Plain(*opt))",
                _ => "R::AsDouble(*opt)",
            };
            format!(
                "[&]() -> {} {{ auto opt = R::Some({expr_val}); return opt ? std::optional{{{some}}} : std::nullopt; }}()",
                cpp_type(ty)
            )
        }
        Type::List { inner } => {
            let item = match inner.as_ref() {
                Type::Bool => "R::Truthy(item)",
                Type::Int => "static_cast<int64_t>(std::llround(R::AsDouble(item)))",
                Type::Text => "winrt::to_hstring(R::Plain(item))",
                _ => "R::AsDouble(item)",
            };
            format!(
                "[&]() -> {} {{ {} out; for (auto const& item : R::AsList({expr_val})) out.push_back({item}); return out; }}()",
                cpp_type(ty),
                cpp_type(ty)
            )
        }
        _ => format!("R::AsDouble({expr_val})"),
    }
}

fn pack_cpp(var: &str, ty: &Type) -> String {
    match ty {
        Type::Bool => format!("Value::Bool({var})"),
        Type::Int => format!("Value::Int({var})"),
        Type::Float => format!("Value::Float({var})"),
        Type::Text => format!("Value::Text(winrt::to_string({var}))"),
        Type::Optional { inner } => match inner.as_ref() {
            Type::Float => format!("({var} ? Value::Some(Value::Float(*{var})) : Value::None())"),
            Type::Int => format!("({var} ? Value::Some(Value::Int(*{var})) : Value::None())"),
            Type::Text => {
                format!(
                    "({var} ? Value::Some(Value::Text(winrt::to_string(*{var}))) : Value::None())"
                )
            }
            _ => "Value::None()".into(),
        },
        _ => format!("Value::Float({var})"),
    }
}

impl Generator<'_> {
    /// Emit per-view `namespace native_<symbol>` definitions that include
    /// ejected relative files under `../native/<symbol>.inl`, returning both
    /// the wrapper header content and the copied native files.
    pub(super) fn native_view_namespaces_split(&self) -> (String, Vec<(String, String)>) {
        let views: Vec<CollectedNativeView<'_>> = collect_native_views(self.app)
            .into_iter()
            .filter(|v| v.info.file_for("winui3").is_some())
            .collect();
        if views.is_empty() {
            return (String::new(), Vec::new());
        }
        let mut out = Out::default();
        out.line("// Co-located native UI views.");
        out.line("// Ejected from Bern — edit freely.");
        out.line("");
        let mut emitted = std::collections::BTreeSet::new();
        let mut native_files = Vec::new();
        for view in &views {
            if !emitted.insert(view.symbol.clone()) {
                continue;
            }
            let Some(path) = view.info.file_for("winui3") else {
                continue;
            };
            let source = self
                .project_dir
                .map(|dir| dir.join(path))
                .filter(|p| p.exists())
                .and_then(|p| std::fs::read_to_string(p).ok())
                .or_else(|| find_native_source(self.app, path).map(|def| def.source.clone()))
                .unwrap_or_default();
            let rel = format!("native/{}.inl", view.symbol);
            native_files.push((rel.clone(), source));
            out.open(format!("namespace native_{} {{", view.symbol));
            self.emit_native_types(&mut out, &view.info);
            out.line(format!("#include \"../{rel}\""));
            out.close("}");
            out.line("");
        }
        (out.finish(), native_files)
    }

    /// Emit per-view `namespace native_<symbol>` definitions at the top of
    /// `app_ui.inl` for every `NativeView` providing a `winui3` file.
    pub(super) fn native_view_namespaces(&self, out: &mut Out) {
        let views: Vec<CollectedNativeView<'_>> = collect_native_views(self.app)
            .into_iter()
            .filter(|v| v.info.file_for("winui3").is_some())
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
            let Some(path) = view.info.file_for("winui3") else {
                continue;
            };
            out.open(format!("namespace native_{} {{", view.symbol));
            self.emit_native_types(out, &view.info);
            let disk_path = self
                .project_dir
                .map(|dir| dir.join(path))
                .filter(|p| p.exists());
            if let Some(real) = disk_path {
                let slash = real.display().to_string().replace('\\', "/");
                out.line(format!("#include \"{slash}\""));
            } else if let Some(def) = find_native_source(self.app, path) {
                for line in def.source.lines() {
                    out.line(line);
                }
            }
            out.close("}");
            out.line("");
        }
    }

    fn emit_native_types(&self, out: &mut Out, info: &NativeViewInfo<'_>) {
        out.open("struct Theme {");
        out.line("std::function<bool()> is_dark;");
        out.line("std::function<winrt::Windows::UI::Color(char const*)> role;");
        out.line("winrt::Windows::UI::Color accent() const { return role(\"accent\"); }");
        out.line("winrt::Windows::UI::Color surface() const { return role(\"surface.canvas\"); }");
        out.line(
            "winrt::Windows::UI::Color text_primary() const { return role(\"text.primary\"); }",
        );
        out.line(
            "winrt::Windows::UI::Color text_secondary() const { return role(\"text.secondary\"); }",
        );
        out.line("winrt::Windows::UI::Color border() const { return role(\"control.border\"); }");
        out.close("};");

        out.open("struct Props {");
        for val in &info.values {
            out.line(format!(
                "std::function<{}()> {};",
                cpp_type(&val.expr.ty),
                snake_ident(val.name)
            ));
        }
        for text in &info.texts {
            out.line(format!(
                "std::function<winrt::hstring()> {};",
                snake_ident(text.name)
            ));
        }
        for draft in &info.drafts {
            out.line(format!(
                "std::function<{}()> {};",
                cpp_type(&draft.field.ty),
                snake_ident(draft.name)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            out.line(format!("std::function<winrt::hstring()> {sid}_label;"));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number { min, max, step, .. } => {
                    out.line(format!("std::function<double()> {sid};"));
                    out.line(format!("double {sid}_min = {};", float(min)));
                    out.line(format!("double {sid}_max = {};", float(max)));
                    if let Some(step) = step {
                        out.line(format!("double {sid}_step = {};", float(step)));
                    }
                }
                NativeEntryKind::Bool { .. } => {
                    out.line(format!("std::function<bool()> {sid};"));
                }
                NativeEntryKind::Input { .. } => {
                    out.line(format!("std::function<winrt::hstring()> {sid};"));
                }
            }
        }
        out.close("};");

        out.open("struct Actions {");
        for draft in &info.drafts {
            let name = snake_ident(draft.name);
            out.line(format!(
                "std::function<void({} const&)> set_{name};",
                cpp_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            match entry.kind {
                NativeEntryKind::Action => {
                    out.line(format!("std::function<void()> {sid};"));
                }
                NativeEntryKind::Number { .. } => {
                    out.line(format!("std::function<void(double)> set_{sid};"));
                }
                NativeEntryKind::Bool { .. } => {
                    out.line(format!("std::function<void(bool)> set_{sid};"));
                }
                NativeEntryKind::Input { .. } => {
                    out.line(format!(
                        "std::function<void(winrt::hstring const&)> set_{sid};"
                    ));
                }
            }
        }
        out.close("};");
    }

    pub(super) fn native_view(&self, out: &mut Out, element: &Element) {
        let Some(info) = inspect_native_view(element) else {
            return;
        };
        if info.file_for("winui3").is_none() {
            if let Some(fallback) = info.fallback {
                self.nodes(out, fallback);
            }
            return;
        }
        let scope = self.current_scope.borrow().clone();
        let sym = bern_ir::native::view_symbol(&scope, info.id);
        let ns = format!("native_{sym}");

        out.open("{");
        out.line(format!("{ns}::Props _props;"));
        for val in &info.values {
            let field = snake_ident(val.name);
            let expr = self.expr(val.expr);
            let unpacked = unpack_cpp(&expr, &val.expr.ty);
            out.line(format!("_props.{field} = [c]() {{ return {unpacked}; }};"));
        }
        for text in &info.texts {
            let field = snake_ident(text.name);
            let txt = self.text(text.text);
            out.line(format!("_props.{field} = [c]() {{ return {txt}; }};"));
        }
        for draft in &info.drafts {
            let field = snake_ident(draft.name);
            let fref = self.field_ref(draft.field);
            let read = format!("c.Read({fref})");
            let unpacked = unpack_cpp(&read, &draft.field.ty);
            out.line(format!("_props.{field} = [c]() {{ return {unpacked}; }};"));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            let lbl = self.text(entry.label);
            out.line(format!("_props.{sid}_label = [c]() {{ return {lbl}; }};"));
            match entry.kind {
                NativeEntryKind::Action => {}
                NativeEntryKind::Number { current, .. } => {
                    let expr = self.expr(current);
                    out.line(format!(
                        "_props.{sid} = [c]() {{ return R::AsDouble({expr}); }};"
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let expr = self.expr(checked);
                    out.line(format!(
                        "_props.{sid} = [c]() {{ return R::Truthy({expr}); }};"
                    ));
                }
                NativeEntryKind::Input { current } => {
                    let expr = self.expr(current);
                    out.line(format!(
                        "_props.{sid} = [c]() {{ return winrt::to_hstring(R::Plain({expr})); }};"
                    ));
                }
            }
        }

        out.line(format!("{ns}::Actions _actions;"));
        for draft in &info.drafts {
            let field = snake_ident(draft.name);
            let fref = self.field_ref(draft.field);
            let packed = pack_cpp("v", &draft.field.ty);
            out.line(format!(
                "_actions.set_{field} = [c]({} const& v) {{ c.Write({fref}, {packed}); }};",
                cpp_type(&draft.field.ty)
            ));
        }
        for entry in &info.entries {
            let sid = snake_ident(entry.id);
            match entry.kind {
                NativeEntryKind::Action => {
                    let body = self.action_body(entry.action);
                    out.line(format!("_actions.{sid} = [c]() {{ {body}; }};"));
                }
                NativeEntryKind::Number {
                    current, min, max, ..
                } => {
                    let body = self.native_controlled_body(entry.action, current, "clamped");
                    out.line(format!(
                        "_actions.set_{sid} = [c](double raw) {{ double clamped = (std::clamp)(raw, {}, {}); {body}; }};",
                        float(min),
                        float(max)
                    ));
                }
                NativeEntryKind::Bool { checked } => {
                    let body = self.native_controlled_body(entry.action, checked, "val");
                    out.line(format!("_actions.set_{sid} = [c](bool val) {{ {body}; }};"));
                }
                NativeEntryKind::Input { current } => {
                    let body = self.native_controlled_body(entry.action, current, "utf8");
                    out.line(format!(
                        "_actions.set_{sid} = [c](winrt::hstring const& val) {{ std::string utf8 = winrt::to_string(val); {body}; }};"
                    ));
                }
            }
        }

        out.line(format!("{ns}::Theme _theme;"));
        out.line("_theme.is_dark = [c]() { return c.win && c.win->root ? B::IsDark(c.win->root) : false; };");
        out.line("_theme.role = [c](char const* role) { bool dark = c.win && c.win->root ? B::IsDark(c.win->root) : false; return B::ColorOf(c.win, dark, B::Paint(role)); };");

        out.line(format!("auto _el = {ns}::build(_props, _actions, _theme);"));
        if element.prop("width").is_none() {
            // Without a width of its own it fills across; with no height of its
            // own either it is `bern-native`, which in a stack's layers also
            // fills down and covers them, as a picture does.
            out.line("if (_el) { if (auto _fw = _el.try_as<xaml::FrameworkElement>(); _fw && std::isnan(_fw.Width())) { _fw.HorizontalAlignment(xaml::HorizontalAlignment::Stretch); SetRole(_fw, std::isnan(_fw.Height()) ? L\"bern-native\" : L\"bern-fill-width\"); } p.Add(_el); }");
        } else {
            out.line("if (_el) { p.Add(_el); }");
        }

        // Register all declared Bern functional entries in `Controls` for CLI access.
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
        let role = std_string(entry.role());
        let enabled = match entry.enabled {
            Some(expr) => format!("[c]() {{ return R::Truthy({}); }}", self.expr(expr)),
            None => "[]() { return true; }".into(),
        };
        let sid = snake_ident(entry.id);
        let actions = match entry.kind {
            NativeEntryKind::Action => {
                format!("Actions{{.activate = _actions.{sid}}}")
            }
            NativeEntryKind::Number { .. } => {
                format!(
                    "Actions{{.setNumber = _actions.set_{sid}, .value = [_p = _props.{sid}]() -> std::optional<std::string> {{ return winrt::to_string(R::Plain(Value::Float(_p()))); }}}}"
                )
            }
            NativeEntryKind::Bool { .. } => {
                format!(
                    "Actions{{.setBool = _actions.set_{sid}, .value = [_p = _props.{sid}]() -> std::optional<std::string> {{ return std::string(_p() ? \"true\" : \"false\"); }}}}"
                )
            }
            NativeEntryKind::Input { .. } => {
                let submit = self.action_body(entry.action);
                format!(
                    "Actions{{.edit = [_a = _actions.set_{sid}](std::string const& s) {{ _a(winrt::to_hstring(s)); }}, .submit = [c]() {{ {submit}; }}, .value = [_p = _props.{sid}]() -> std::optional<std::string> {{ return winrt::to_string(_p()); }}}}"
                )
            }
        };
        out.line(format!(
            "Controls::Register(c, {local_id}, {role}, _props.{sid}_label, {enabled}, {actions});"
        ));
    }

    fn native_controlled_body(&self, action: &Action, controlled: &Expr, var: &str) -> String {
        let pack_var = |ty: &Type| match ty {
            Type::Int => format!("Value::Int(static_cast<int64_t>(std::llround({var})))"),
            Type::Float => format!("Value::Float({var})"),
            Type::Bool => format!("Value::Bool({var})"),
            Type::Text => format!("Value::Text({var})"),
            _ => format!("Value::Float({var})"),
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
                format!("Dispatch(c, {command}, Values{{{}}})", values.join(", "))
            }
            Action::Session { field, op } => {
                let target = self.field_ref(field);
                match op {
                    bern_ir::SessionOp::Set { value } if value == controlled => {
                        format!("c.Write({target}, {})", pack_var(&field.ty))
                    }
                    _ => self.action_body(action),
                }
            }
            _ => self.action_body(action),
        }
    }
}
