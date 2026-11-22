//! The application's interface, `app_ui.inl`: a function per component
//! and window, the close checks of windows, and the window kinds.

use super::{Generator, Out, float, string};
use bern_ir::style::ColorRef;
use bern_ir::{Action, ComponentDef, Node, WindowChrome, WindowDef, WindowInitial, WindowRole};

impl Generator<'_> {
    fn component(&self, out: &mut Out, component: &ComponentDef) {
        *self.current_scope.borrow_mut() = component.name.clone();
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!(
            "static void c_{}(Parent const& p, Ctx const& c) {{",
            component.name
        ));
        self.node(out, &component.body);
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, window: &WindowDef) {
        *self.current_scope.borrow_mut() = window.id.clone();
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.open(format!(
            "static void w_{}(Parent const& p, Ctx const& c) {{",
            window.id
        ));
        self.node(out, &window.content);
        out.close("}");
        out.line("");
        if let Some(close) = &window.close {
            let save = match &close.save {
                Some(Action::Engine { command, args }) => format!(
                    "[](Ctx const& c, Done done) {{ Dispatch(c, {command}, {}, std::move(done)); }}",
                    self.values(args)
                ),
                _ => "nullptr".into(),
            };
            out.line(format!(
                "/// The close check of the `{}` window, read against its own drafts.",
                window.id
            ));
            out.open(format!(
                "static CloseCheck close_{}(Ctx const& c) {{",
                window.id
            ));
            out.line(format!(
                "return CloseCheck{{{}, {}, {save}}};",
                self.condition(&close.when),
                self.text(&close.message)
            ));
            out.close("}");
            out.line("");
        }
    }

    /// The application's ejected interface, split into `ui/app_ui.inl`,
    /// `ui/styles.inl`, `ui/views/<Component>.inl`, `ui/windows/<Window>.inl`,
    /// and copied `native/<symbol>.inl` files (paths relative to `src/`).
    pub fn interface_split(&self) -> Vec<(String, String)> {
        let app = self.app;
        let mut files: Vec<(String, String)> = Vec::new();
        let mut entry = Out::default();
        entry.line("// Application UI entry point.");
        entry.line("// Ejected from Bern — edit freely.");
        entry.line("// Included once per active front end (WinUI 3 / XAML Islands).");
        entry.line("");

        // Styles & theme helpers.
        {
            let mut styles = Out::default();
            styles.line("// Styles and theme helpers.");
            styles.line("// Ejected from Bern — edit freely.");
            styles.line("");
            self.style_tables(&mut styles);
            self.theme_functions(&mut styles);
            self.accent_colors(&mut styles);
            files.push(("ui/styles.inl".into(), styles.finish()));
            entry.line("#include \"styles.inl\"");
        }

        // Co-located native views (copied locally; zero external paths).
        let (native_header, native_files) = self.native_view_namespaces_split();
        if !native_header.is_empty() {
            files.push(("ui/native_views.inl".into(), native_header));
            for (rel, content) in native_files {
                files.push((rel, content));
            }
            entry.line("#include \"native_views.inl\"");
        }
        entry.line("");

        // Forward declarations so split view files can reference each other freely.
        entry.line("// Component forward declarations.");
        for component in &app.components {
            entry.line(format!(
                "static void c_{}(Parent const& p, Ctx const& c);",
                component.name
            ));
        }
        entry.line("");

        // One file per component.
        let view_stems = bern_target_api::eject::unique_stems(
            app.components.iter().map(|c| c.name.as_str()),
            bern_target_api::eject::pascal_filename,
        );
        entry.line("// Components.");
        for (component, stem) in app.components.iter().zip(view_stems) {
            let mut view = Out::default();
            view.line(format!("// Component: {}", component.name));
            view.line("// Ejected from Bern — edit freely.");
            view.line("");
            self.component(&mut view, component);
            let rel = format!("ui/views/{stem}.inl");
            files.push((rel, view.finish()));
            entry.line(format!("#include \"views/{stem}.inl\""));
        }
        entry.line("");

        // One file per window.
        let window_stems = bern_target_api::eject::unique_stems(
            app.windows.windows.iter().map(|w| w.id.as_str()),
            |id| format!("{}Window", bern_target_api::eject::pascal_filename(id)),
        );
        entry.line("// Windows.");
        for (window, stem) in app.windows.windows.iter().zip(window_stems) {
            let mut win = Out::default();
            win.line(format!("// Window: {}", window.id));
            win.line("// Ejected from Bern — edit freely.");
            win.line("");
            self.window(&mut win, window);
            let rel = format!("ui/windows/{stem}.inl");
            files.push((rel, win.finish()));
            entry.line(format!("#include \"windows/{stem}.inl\""));
        }
        entry.line("");
        self.window_table(&mut entry);
        entry.line("");
        self.status_item(&mut entry);
        files.push(("ui/app_ui.inl".into(), entry.finish()));
        files
    }

    /// The application's interface, `app_ui.inl`: its components, windows
    /// and their close checks, included once per front end.
    pub fn interface(&self) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("// Included by the runtime once per front end.");
        out.line("");
        self.style_tables(&mut out);
        self.theme_functions(&mut out);
        self.native_view_namespaces(&mut out);
        out.line("// Components.");
        for component in &app.components {
            out.line(format!(
                "static void c_{}(Parent const& p, Ctx const& c);",
                component.name
            ));
        }
        out.line("");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }
        self.window_table(&mut out);
        out.line("");
        self.status_item(&mut out);
        out.line("");
        self.accent_colors(&mut out);
        out.finish()
    }

    /// `StatusItem()`: the status item's tooltip and the function that adds
    /// the application's items to its menu each time it opens, reading the
    /// state of that moment; none without a `StatusItem`.
    fn status_item(&self, out: &mut Out) {
        let Some(item) = &self.app.windows.status_item else {
            out.line("static std::optional<StatusItemDef> StatusItem() { return std::nullopt; }");
            return;
        };
        out.line("/// The application's items of the status item's menu.");
        out.open("static void StatusMenu(Ctx const& c, StatusArea::Menu& m) {");
        self.status_nodes(out, &item.children);
        out.close("}");
        out.line("");
        out.line("/// The status item in the notification area.");
        out.open("static std::optional<StatusItemDef> StatusItem() {");
        out.line("StatusItemDef item;");
        if let Some(label) = self.text_prop(item, "label") {
            out.line(format!("item.label = {label};"));
        }
        out.line("item.menu = StatusMenu;");
        if item.keyword("login_item") == Some("true") {
            out.line("item.loginItem = true;");
        }
        if item.resource("image").is_some() {
            out.line("item.image = true;");
        }
        if let Some(shown) = item.expr("shown") {
            out.line(format!(
                "item.shown = [](Ctx const& c) -> bool {{ return {}; }};",
                self.condition(shown)
            ));
        }
        out.line("return item;");
        out.close("}");
    }

    /// Statements that add menu items to `m` in the context `c`: branches
    /// and rows are C++ control flow, as the menu is built at once.
    fn status_nodes(&self, out: &mut Out, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Element(element) => match element.kind.as_str() {
                    "CommandItem" => {
                        let flag = |name: &str| {
                            element
                                .expr(name)
                                .map(|expr| self.condition(expr))
                                .unwrap_or_else(|| (name == "enabled").to_string())
                        };
                        out.line(format!(
                            "m.Item(c, {}, {}, {}, {});",
                            element
                                .text("label")
                                .map(|text| self.text(text))
                                .unwrap_or_else(|| "winrt::hstring()".into()),
                            flag("enabled"),
                            flag("checked"),
                            self.action(element)
                                .unwrap_or_else(|| "[](Ctx const&) {}".into())
                        ));
                    }
                    "CommandMenu" => {
                        out.open(format!(
                            "m.Sub({}, [&](StatusArea::Menu& m) {{",
                            element
                                .text("label")
                                .map(|text| self.text(text))
                                .unwrap_or_else(|| "winrt::hstring()".into())
                        ));
                        self.status_nodes(out, &element.children);
                        out.close("});");
                    }
                    "Divider" => out.line("m.Divider();"),
                    "ForEach" => {
                        let (Some(items), Some(template)) =
                            (element.expr("items"), &element.template)
                        else {
                            continue;
                        };
                        out.open(format!(
                            "for (auto const& row : R::AsList({})) {{",
                            self.expr(items)
                        ));
                        out.open("[&](Ctx const& c) {");
                        self.status_nodes(out, &element.children);
                        out.close(format!(
                            "}}(c.WithLocal({}, [row] {{ return row; }}));",
                            string(&template.var)
                        ));
                        out.close("}");
                    }
                    _ => {}
                },
                Node::If(node) => {
                    for (index, (condition, body)) in node.branches.iter().enumerate() {
                        let keyword = if index == 0 { "if" } else { "} else if" };
                        if index == 0 {
                            out.open(format!("{keyword} ({}) {{", self.condition(condition)));
                        } else {
                            out.indent -= 1;
                            out.open(format!("{keyword} ({}) {{", self.condition(condition)));
                        }
                        self.status_nodes(out, body);
                    }
                    if !node.otherwise.is_empty() {
                        out.indent -= 1;
                        out.open("} else {");
                        self.status_nodes(out, &node.otherwise);
                    }
                    out.close("}");
                }
                Node::IfLet(node) => {
                    out.open(format!(
                        "if (auto value = {}; value.IsSome()) {{",
                        self.expr(&node.value)
                    ));
                    out.open("[&](Ctx const& c) {");
                    self.status_nodes(out, &node.then);
                    out.close(format!(
                        "}}(c.WithLocal({}, [value] {{ return R::Some(value).value_or(Value::Unit()); }}));",
                        string(&node.var)
                    ));
                    if !node.otherwise.is_empty() {
                        out.indent -= 1;
                        out.open("} else {");
                        self.status_nodes(out, &node.otherwise);
                    }
                    out.close("}");
                }
                Node::Match(node) => {
                    out.open("{");
                    out.line(format!("auto value = {};", self.expr(&node.value)));
                    out.line("auto tag = R::Tag(value);");
                    // Arms of a variant first, then the wildcard, as the
                    // runtime's `match` chooses.
                    let arms = node
                        .arms
                        .iter()
                        .filter(|arm| arm.variant.is_some())
                        .chain(node.arms.iter().filter(|arm| arm.variant.is_none()));
                    for (index, arm) in arms.enumerate() {
                        let test = match arm.variant {
                            Some(variant) => format!("(tag == {variant}u)"),
                            None => "(true)".into(),
                        };
                        if index == 0 {
                            out.open(format!("if {test} {{"));
                        } else {
                            out.indent -= 1;
                            out.open(format!("}} else if {test} {{"));
                        }
                        let mut inner = "c".to_string();
                        for binding in &arm.bindings {
                            inner = format!(
                                "{inner}.WithLocal({}, [value] {{ return R::Payload(value, {}); }})",
                                string(&binding.name),
                                binding.field
                            );
                        }
                        out.open("[&](Ctx const& c) {");
                        self.status_nodes(out, &arm.body);
                        out.close(format!("}}({inner});"));
                    }
                    if !node.arms.is_empty() {
                        out.close("}");
                    }
                    out.close("}");
                }
                Node::Component(_) => {}
            }
        }
    }

    /// `Windows()`: the window kinds, as the runtime sees them.
    fn window_table(&self, out: &mut Out) {
        let app = self.app;
        out.line("/// The window kinds, as the runtime sees them.");
        out.open("static std::vector<WindowDef> Windows() {");
        out.line("std::vector<WindowDef> windows;");
        for window in &app.windows.windows {
            let optional = |value: Option<f64>| match value {
                Some(value) => float(value),
                None => "std::nullopt".into(),
            };
            let sessions: Vec<_> = window
                .sessions
                .iter()
                .map(|name| self.session_index(name).to_string())
                .collect();
            out.open("{");
            out.line("WindowDef w;");
            out.line(format!("w.id = {};", string(&window.id)));
            out.line(format!("w.title = {};", string(&window.title.key)));
            out.line(format!(
                "w.settings = {};",
                window.role == WindowRole::Settings
            ));
            out.line(format!("w.dialog = {};", window.role == WindowRole::Dialog));
            out.line(format!(
                "w.utility = {};",
                window.role == WindowRole::Utility
            ));
            if let Some(overlay) = &window.overlay {
                out.line(format!("w.overlay = {};", string(overlay)));
            }
            out.line(format!("w.width = {};", optional(window.width)));
            out.line(format!("w.height = {};", optional(window.height)));
            out.line(format!("w.minWidth = {};", optional(window.min_width)));
            out.line(format!("w.minHeight = {};", optional(window.min_height)));
            out.line(format!(
                "w.initialOpen = {};",
                window.initial == WindowInitial::Open
            ));
            out.line(format!("w.sessions = {{{}}};", sessions.join(", ")));
            out.line(format!("w.render = w_{};", window.id));
            if window.close.is_some() {
                out.line(format!("w.close = close_{};", window.id));
            }
            out.line(format!(
                "w.unified = {};",
                window.chrome == WindowChrome::Unified
            ));
            out.line(format!("w.idleAfter = {};", optional(window.idle_after)));
            if let Some(enabled) = &window.idle_enabled {
                out.line(format!(
                    "w.idleEnabled = [](Ctx const& c) {{ return {}; }};",
                    self.condition(enabled)
                ));
            }
            if let Some(immersive) = &window.immersive {
                out.line(format!(
                    "w.immersive = [](Ctx const& c) {{ return {}; }};",
                    self.condition(immersive)
                ));
            }
            if let Some(palette) = &app.windows.palette {
                out.line(format!(
                    "w.palette = [](Ctx const& c) {{ return R::AsText({}); }};",
                    self.expr(palette)
                ));
            }
            if app.windows.accent.is_some() {
                out.line("w.accent = ThemeAccent;");
            }
            if app.windows.appearance.is_some() {
                out.line("w.appearance = ThemeAppearance;");
            }
            if app.windows.icon.is_some() {
                out.line("w.icon = ThemeIcon;");
            }
            out.line("windows.push_back(std::move(w));");
            out.close("}");
        }
        out.line("return windows;");
        out.close("}");
    }

    /// `AccentColors()`: the accents of `style!`, for `WindowSet(accent:)`.
    fn accent_colors(&self, out: &mut Out) {
        let app = self.app;
        out.line("/// The accents of `style!`, for `WindowSet(accent:)`: light and dark sRGB.");
        out.open("static std::vector<std::pair<uint32_t, uint32_t>> AccentColors() {");
        let accents: Vec<String> = app
            .style
            .accents
            .iter()
            .map(|accent| match &accent.color {
                ColorRef::Fixed { light, dark } => format!(
                    "{{0x{:02X}{:02X}{:02X}u, 0x{:02X}{:02X}{:02X}u}}",
                    light.red, light.green, light.blue, dark.red, dark.green, dark.blue
                ),
                ColorRef::Role { .. } => "{0u, 0u}".to_string(),
            })
            .collect();
        out.line(format!("return {{{}}};", accents.join(", ")));
        out.close("}");
    }
}
