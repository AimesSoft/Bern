//! The application's `MenuBar` in the menu bar of macOS: SwiftUI
//! `Commands`, a `CommandMenu` for each menu of its own and a
//! `CommandGroup` for the items added to a menu of the system's. Its
//! `StatusItem` is a `MenuBarExtra` on the right of the menu bar. iOS and
//! tvOS show neither.

use super::values::{session_class, session_var};
use super::{Ctx, Generator, Out, Platform};
use crate::names::string;
use bern_ir::{Element, Node};

/// Where the items of a `CommandMenu` with `role` go among the system's
/// menus: after the system's own items of that menu, the application menu's
/// after its settings, the file menu's in place of its new-document items
/// (single windows have none), and the help menu's in place of a help book
/// the application does not have. `about`'s stand in place of the About
/// item, so the system's standard about panel no longer opens.
fn placement(role: &str) -> &'static str {
    match role {
        "app" => "after: .appSettings",
        "about" => "replacing: .appInfo",
        "file" => "replacing: .newItem",
        "edit" => "after: .pasteboard",
        "view" => "before: .toolbar",
        "window" => "after: .windowArrangement",
        _ => "replacing: .help",
    }
}

impl Generator<'_> {
    /// `BernMenuBar`, the scene's commands, when the application declares a
    /// `MenuBar` and runs on macOS; whether it did. Each menu's items are a
    /// view of their own, `BernMenuN`, so that they follow the state they
    /// read, as views do, which commands alone do not.
    pub(super) fn menu_bar(&self, out: &mut Out) -> bool {
        let Some(bar) = self.app.windows.menu_bar.as_ref() else {
            return false;
        };
        if self.platform != Platform::Macos {
            return false;
        }
        let app = self.app;
        let sessions: &[String] = app
            .window(&app.windows.primary)
            .map_or(&[], |window| &window.sessions);
        let menus: Vec<&Element> = bar
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(menu) => Some(menu),
                _ => None,
            })
            .collect();
        out.line("// MARK: - Menu bar");
        out.line("");
        out.line("/// The `MenuBar`: menus of the application's own and items in the");
        out.line("/// system's.");
        out.line("@MainActor");
        out.open("struct BernMenuBar: Commands {");
        out.open("var body: some Commands {");
        for (number, menu) in menus.iter().enumerate() {
            match menu.keyword("role") {
                Some(role) => out.line(format!(
                    "SwiftUI.CommandGroup({}) {{ BernMenu{number}() }}",
                    placement(role)
                )),
                None => out.line(format!(
                    "SwiftUI.CommandMenu(SwiftUI.Text(verbatim: {})) {{ BernMenu{number}() }}",
                    self.text_prop(menu, "label").unwrap_or_default()
                )),
            }
        }
        out.close("}");
        out.close("}");
        out.line("");
        let ctx = Ctx {
            component: "menu_bar".into(),
            ..Ctx::default()
        };
        for (number, menu) in menus.iter().enumerate() {
            out.line(format!(
                "/// The items of the menu bar's menu {}.",
                number + 1
            ));
            out.open(format!("struct BernMenu{number}: View {{"));
            out.line("private var bernEngine: BernEngine { BernEngine.shared }");
            out.line("private var bernWindow: BernWindowContext { BernMenuBarWindow.primary }");
            for session in sessions {
                out.line(format!(
                    "private var {}: {} {{ bernWindow.session({}.self) }}",
                    session_var(session),
                    session_class(session),
                    session_class(session)
                ));
            }
            out.line("");
            out.open("var body: some View {");
            self.nodes(out, &menu.children, &ctx);
            out.close("}");
            out.close("}");
            out.line("");
        }
        true
    }

    /// `BernMenuBarWindow`, the window the items of the menu bar and the
    /// status item act on: the primary window, whose drafts the menu bar's
    /// items read and write, or fresh drafts while it is closed. Generated
    /// on macOS when the application has either.
    pub(super) fn menu_window(&self, out: &mut Out) {
        let app = self.app;
        if self.platform != Platform::Macos
            || (app.windows.menu_bar.is_none() && app.windows.status_item.is_none())
        {
            return;
        }
        let index = app
            .windows
            .windows
            .iter()
            .position(|window| window.id == app.windows.primary)
            .unwrap_or(0);
        let sessions: &[String] = app
            .window(&app.windows.primary)
            .map_or(&[], |window| &window.sessions);
        out.line("/// The window the menu bar's items act on: the primary window, whose");
        out.line("/// drafts they read and write, or fresh drafts while it is closed.");
        out.line("@MainActor");
        out.open("enum BernMenuBarWindow {");
        out.line(format!(
            "static let closed = BernWindowContext(kind: {index}, kindID: {}, isSettings: false, sessions: [{}])",
            string(&app.windows.primary),
            sessions
                .iter()
                .map(|session| format!("{}()", session_class(session)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.open("static var primary: BernWindowContext {");
        out.line("_ = BernHost.shared.windowRevision");
        out.line(format!(
            "return BernHost.shared.windows[{}] ?? closed",
            string(&app.windows.primary)
        ));
        out.close("}");
        out.close("}");
        out.line("");
    }

    /// `BernStatusMenu`, the menu of the `StatusItem` on macOS; whether
    /// there is one. It opens the primary window, runs the application's
    /// items, has “Open at Login” with `login_item`, and quits, as menu bar
    /// extras do.
    pub(super) fn status_menu(&self, out: &mut Out) -> bool {
        let Some(item) = self.app.windows.status_item.as_ref() else {
            return false;
        };
        if self.platform != Platform::Macos {
            return false;
        }
        let app = self.app;
        let title = app
            .window(&app.windows.primary)
            .map(|window| format!("BernText.string({})", string(&window.title.key)))
            .unwrap_or_else(|| "\"\"".into());
        out.line("// MARK: - Status item");
        out.line("");
        out.line("/// The menu of the `StatusItem` in the menu bar.");
        out.open("struct BernStatusMenu: View {");
        out.line("private var bernEngine: BernEngine { BernEngine.shared }");
        out.line("private var bernWindow: BernWindowContext { BernMenuBarWindow.primary }");
        out.line("");
        out.open("var body: some View {");
        out.line(format!(
            "SwiftUI.Button(BernText.format(\"bern.window.open\", [{title}])) {{ BernStatusArea.open({}) }}",
            string(&app.windows.primary)
        ));
        if !item.children.is_empty() {
            out.line("SwiftUI.Divider()");
            let ctx = Ctx {
                component: "status_item".into(),
                ..Ctx::default()
            };
            self.nodes(out, &item.children, &ctx);
        }
        out.line("SwiftUI.Divider()");
        if item.keyword("login_item") == Some("true") {
            out.line("BernLoginItemToggle()");
        }
        out.line("SwiftUI.Button(BernText.string(\"bern.app.quit\")) { NSApplication.shared.terminate(nil) }");
        out.close("}");
        out.close("}");
        out.line("");
        true
    }

    /// The `MenuBarExtra` scene of the `StatusItem`: `icon` is a symbol,
    /// `image` the picture the build drew from its SVG; the menu bar draws
    /// either as a template in its own colour.
    pub(super) fn status_scene(&self, out: &mut Out) {
        let Some(item) = self.app.windows.status_item.as_ref() else {
            return;
        };
        let image = if item.resource("image").is_some() {
            "SwiftUI.Image(nsImage: BernStatusArea.picture)".to_string()
        } else {
            format!(
                "SwiftUI.Image(systemName: {})",
                string(item.icon("icon").unwrap_or("circle"))
            )
        };
        out.open("SwiftUI.MenuBarExtra {");
        out.line("BernStatusMenu()");
        out.line("    .environment(BernEngine.shared)");
        out.close("} label: {");
        out.indent += 1;
        out.line(format!(
            "{image}.accessibilityLabel(SwiftUI.Text(verbatim: {}))",
            self.text_prop(item, "label").unwrap_or_default()
        ));
        out.close("}");
    }

    /// A submenu of the menu bar, opening beside its item.
    pub(super) fn submenu(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open("SwiftUI.Menu {");
        self.nodes(out, &element.children, ctx);
        out.close("} label: {");
        out.indent += 1;
        out.line(format!(
            "SwiftUI.Text(verbatim: {})",
            self.text_prop(element, "label").unwrap_or_default()
        ));
        out.close("}");
    }

    /// A `CommandItem`: a button of the menu, or a checked toggle; external
    /// entries list it and run it as choosing it does.
    pub(super) fn command_item(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (action, _) = self.pressed(element);
        let enabled = element.expr("enabled").map(|expr| self.expr(expr));
        out.line(format!(
            "let _ = BernMenuItems.register({}, label: {label}, enabled: {}) {{ {action} }}",
            self.target(element, ctx),
            enabled.clone().unwrap_or_else(|| "true".into())
        ));
        match element.expr("checked") {
            Some(checked) => {
                out.open(format!(
                    "SwiftUI.Toggle(isOn: SwiftUI.Binding(get: {{ {} }}, set: {{ _ in {action} }})) {{",
                    self.expr(checked)
                ));
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
            }
            None => {
                out.open("SwiftUI.Button(action: {");
                out.line(&action);
                out.close("}) {");
                out.indent += 1;
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
            }
        }
        if let Some(shortcut) = self.shortcut(element) {
            out.line(format!(".keyboardShortcut({shortcut})"));
        }
        if let Some(enabled) = &enabled {
            out.line(format!(".disabled(!({enabled}))"));
        }
    }
}
