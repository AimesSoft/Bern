//! The composables of components and windows, with a window's close
//! check.

use super::values::{kotlin_type, session_class, session_var};
use super::{Ctx, Generator, Out};
use crate::names::{self, double, member, string};
use bern_ir::{Action, ComponentDef, ParamKind, WindowDef, WindowRole};

impl Generator<'_> {
    // MARK: Components and windows

    /// Lines binding the window and the sessions a body reads.
    fn locals(&self, out: &mut Out, sessions: &[String], window: &str) {
        for session in sessions {
            out.line(format!(
                "val {} = {window}.session({}::class.java)",
                session_var(session),
                session_class(session)
            ));
        }
    }

    pub(super) fn component(&self, out: &mut Out, component: &ComponentDef) {
        let name = names::component(&component.name);
        let mut sessions = Vec::new();
        let mut params = Vec::new();
        for param in &component.params {
            match &param.kind {
                ParamKind::Value { ty } => {
                    params.push(format!("{}: {}", member(&param.name), kotlin_type(ty)))
                }
                ParamKind::Session { session } => {
                    if !sessions.contains(session) {
                        sessions.push(session.clone());
                    }
                }
                ParamKind::Client { .. } | ParamKind::Windows | ParamKind::Players => {}
            }
        }
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.line("@Composable");
        out.open(format!("fun {name}({}) {{", params.join(", ")));
        out.line("val bernWindow = LocalBernWindow.current");
        self.locals(out, &sessions, "bernWindow");
        // The component may be a direct child of a staggering container.
        let ctx = Ctx {
            component: component.name.clone(),
            stagger: self.staggers,
            ..Ctx::default()
        };
        self.node(out, &component.body, &ctx);
        out.close("}");
        out.line("");
    }

    pub(super) fn window(&self, out: &mut Out, index: usize, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.line("@Composable");
        out.open(format!("fun BernWindow{index}() {{"));
        let sessions: Vec<String> = window
            .sessions
            .iter()
            .map(|session| format!("{}()", session_class(session)))
            .collect();
        out.line(format!(
            "val context = remember {{ BernWindowContext({index}, {}, {}, listOf({})) }}",
            string(&window.id),
            window.role == WindowRole::Settings,
            sessions.join(", ")
        ));
        out.line("val bernWindow = context");
        self.locals(out, &window.sessions, "context");
        if let Some(close) = &window.close {
            self.close_check(out, close);
        }
        let windows = &self.app.windows;
        let brand = bern_target_api::style::brand_accent(self.app);
        if windows.accent.is_some() || windows.appearance.is_some() || brand.is_some() {
            let accent = windows
                .accent
                .as_ref()
                .map(|expr| format!("BernAccents.color(({}).bernName)", self.expr(expr)))
                .or_else(|| brand.map(|color| crate::style::color(&color)))
                .unwrap_or_else(|| "null".into());
            let appearance = windows
                .appearance
                .as_ref()
                .map(|expr| format!("({}).bernName", self.expr(expr)))
                .unwrap_or_else(|| "null".into());
            out.line(format!("BernThemeChoice({accent}, {appearance})"));
        }
        // The application icon, chosen at run time: the launcher alias of
        // the variant's icon, the `Default` one first as the manifest has it.
        if let Some(icon) = &windows.icon {
            let choices = bern_target_api::icon::choices(self.app);
            if choices.iter().any(|(_, alternate)| alternate.is_some()) {
                let package = names::package(&self.app.app.bundle_id);
                let mut aliases = vec![string(&format!("{package}.BernIconDefault"))];
                aliases.extend(
                    choices
                        .iter()
                        .filter(|(_, alternate)| alternate.is_some())
                        .map(|(variant, _)| string(&format!("{package}.BernIcon{variant}"))),
                );
                out.line(format!(
                    "BernIconChoice({} + ({}).bernName, listOf({}))",
                    string(&format!("{package}.BernIcon")),
                    self.expr(icon),
                    aliases.join(", ")
                ));
            }
        }
        // `chrome: unified`: the top bar shows only for what the content
        // puts there; immersion hides the top bar and the system bars.
        let mut args = Vec::new();
        if let Some(idle) = window.idle_after {
            args.push(format!("idleAfter = {}", double(idle)));
        }
        if let Some(immersive) = &window.immersive {
            args.push(format!("immersive = {}", self.expr(immersive)));
        }
        // Idle detection switched on and off by state.
        if let Some(enabled) = &window.idle_enabled {
            args.push(format!("idleEnabled = {}", self.expr(enabled)));
        }
        // A phone turns the window to landscape while this holds.
        if let Some(landscape) = &window.landscape {
            args.push(format!("landscape = {}", self.expr(landscape)));
        }
        if window.chrome == bern_ir::WindowChrome::Unified {
            args.push("unified = true".to_string());
        }
        let args = if args.is_empty() {
            String::new()
        } else {
            format!(", {}", args.join(", "))
        };
        out.open(format!("BernWindowRoot(context{args}) {{"));
        let ctx = Ctx {
            component: window.id.clone(),
            window: true,
            ..Ctx::default()
        };
        self.node(out, &window.content, &ctx);
        out.close("}");
        out.close("}");
        out.line("");
    }

    /// The window's close check: evaluated against its own drafts when
    /// closing is requested, with the declared save command.
    fn close_check(&self, out: &mut Out, close: &bern_ir::CloseCheck) {
        out.open("context.closeCheck = BernCloseCheck(");
        out.line(format!(
            "needsConfirmation = {{ {} }},",
            self.expr(&close.when)
        ));
        out.line(format!("message = {{ {} }},", self.text(&close.message)));
        match &close.save {
            Some(Action::Engine { command, args }) => {
                let spec = &self.app.protocol.commands[*command];
                let mut values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                values.push("context".into());
                values.push("done".into());
                out.line(format!(
                    "save = {{ done -> BernEngine.{}({}) }},",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                ));
            }
            _ => out.line("save = null,"),
        }
        out.close(")");
    }
}
