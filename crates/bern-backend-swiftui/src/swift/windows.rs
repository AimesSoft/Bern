//! The views of components and windows, with a window's close check.

use super::values::{session_class, session_var, swift_type};
use super::{Ctx, Generator, Out, Platform};
use crate::names::{self, double, member, string};
use bern_ir::{Action, ComponentDef, Expr, Node, ParamKind, WindowDef, WindowRole};

impl Generator<'_> {
    fn uses_engine(&self, nodes: &[&Node]) -> bool {
        let mut uses = false;
        for node in nodes {
            node.walk(&mut |node| {
                if let Node::Element(element) = node
                    && matches!(element.action, Some(Action::Engine { .. }))
                {
                    uses = true;
                }
            });
        }
        uses
    }

    pub(super) fn component(&self, out: &mut Out, component: &ComponentDef) {
        let name = names::component(&component.name);
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!("struct {name}: View {{"));
        let mut sessions = Vec::new();
        let mut client = false;
        for param in &component.params {
            match &param.kind {
                ParamKind::Value { ty } => {
                    out.line(format!("let {}: {}", member(&param.name), swift_type(ty)))
                }
                ParamKind::Client { .. } => client = true,
                ParamKind::Session { session } => {
                    if !sessions.contains(session) {
                        sessions.push(session.clone());
                    }
                }
                ParamKind::Windows | ParamKind::Players => {}
            }
        }
        if client || self.uses_engine(&[&component.body]) {
            out.line("@Environment(BernEngine.self) private var bernEngine");
        }
        for session in &sessions {
            out.line(format!(
                "@Environment({}.self) private var {}",
                session_class(session),
                session_var(session)
            ));
        }
        out.line("@Environment(BernWindowContext.self) private var bernWindow");
        out.line("");
        out.open("var body: some View {");
        let ctx = Ctx {
            component: component.name.clone(),
            ..Ctx::default()
        };
        self.node(out, &component.body, &ctx);
        out.close("}");
        out.close("}");
        out.line("");
    }

    pub(super) fn window(&self, out: &mut Out, index: usize, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.open(format!("struct BernWindow{index}: View {{"));
        let sessions: Vec<String> = window
            .sessions
            .iter()
            .map(|session| format!("{}()", session_class(session)))
            .collect();
        out.line(format!(
            "@State private var context = BernWindowContext(kind: {index}, kindID: {}, isSettings: {}, sessions: [{}])",
            string(&window.id),
            window.role == WindowRole::Settings,
            sessions.join(", ")
        ));
        out.line("@Environment(BernEngine.self) private var bernEngine");
        // Content written inline in the window sees the same names as a
        // component body: the window and its sessions.
        out.line("private var bernWindow: BernWindowContext { context }");
        for session in &window.sessions {
            out.line(format!(
                "private var {}: {} {{ context.session({}.self) }}",
                session_var(session),
                session_class(session),
                session_class(session)
            ));
        }
        out.line("");
        out.open("var body: some View {");
        out.open("BernWindowRoot(context: context) {");
        let ctx = Ctx {
            component: window.id.clone(),
            window: true,
            ..Ctx::default()
        };
        out.open("BernSearchPaletteHost {");
        self.node(out, &window.content, &ctx);
        out.close("}");
        out.close("}");
        if window.chrome == bern_ir::WindowChrome::Unified
            || window.immersive.is_some()
            || window.idle_after.is_some()
            || window.landscape.is_some()
        {
            out.line(format!(
                ".modifier(BernWindowBehavior(context: context, unified: {}, idleAfter: {}, immersive: {}, idleEnabled: {}, landscape: {}))",
                window.chrome == bern_ir::WindowChrome::Unified,
                window
                    .idle_after
                    .map(double)
                    .unwrap_or_else(|| "nil".into()),
                window
                    .immersive
                    .as_ref()
                    .map(|expr| self.expr(expr))
                    .unwrap_or_else(|| "false".into()),
                window.idle_enabled.as_ref().map(|e| self.expr(e)).unwrap_or_else(|| "true".into()),
                window.landscape.as_ref().map(|e| self.expr(e)).unwrap_or_else(|| "nil".into())
            ));
        }
        // The application's accent and appearance, chosen at run time.
        let catalog = &self.app.windows;
        if catalog.accent.is_some() || catalog.appearance.is_some() {
            out.line(format!(
                ".modifier(BernThemeChoice(accent: {}, appearance: {}))",
                catalog
                    .accent
                    .as_ref()
                    .map(|expr| format!("BernAccents.color(({}).bernName)", self.expr(expr)))
                    .unwrap_or_else(|| "nil".into()),
                catalog
                    .appearance
                    .as_ref()
                    .map(|expr| format!("({}).bernName", self.expr(expr)))
                    .unwrap_or_else(|| "nil".into())
            ));
        }
        for session in &window.sessions {
            out.line(format!(
                ".environment(context.session({}.self))",
                session_class(session)
            ));
        }
        let frame: Vec<String> = [
            window.min_width.map(|v| format!("minWidth: {}", double(v))),
            window
                .min_height
                .map(|v| format!("minHeight: {}", double(v))),
        ]
        .into_iter()
        .flatten()
        .collect();
        match self.platform {
            Platform::Macos if window.role == WindowRole::Settings => {
                if let Some(width) = window.width {
                    out.line(format!(".frame(width: {})", double(width)));
                }
                // macOS settings windows fit their content and resize per tab.
                out.line(".scrollDisabled(true)");
                out.line(".fixedSize(horizontal: false, vertical: true)");
            }
            Platform::Macos if !frame.is_empty() => {
                out.line(format!(".frame({})", frame.join(", ")));
            }
            // The minimum size applies where people resize windows (iPad);
            // iPhone screens and sheets size them.
            Platform::Ios
                if window.role != WindowRole::Settings
                    && (window.min_width.is_some() || window.min_height.is_some()) =>
            {
                let side = |value: Option<f64>| value.map(double).unwrap_or_else(|| "nil".into());
                out.line(format!(
                    ".bernMinimumSize(width: {}, height: {})",
                    side(window.min_width),
                    side(window.min_height)
                ));
            }
            _ => {}
        }
        if window.close.is_some() {
            out.line(".onAppear { context.closeCheck = bernCloseCheck() }");
        }
        out.close("}");
        if let Some(close) = &window.close {
            self.close_check(out, close);
        }
        out.close("}");
        out.line("");
    }

    /// Lines that bind the sessions and engine an expression reads.
    fn locals(&self, out: &mut Out, exprs: &[&Expr]) {
        let mut sessions = Vec::new();
        for expr in exprs {
            for session in expr.sessions() {
                if !sessions.contains(&session) {
                    sessions.push(session);
                }
            }
        }
        for session in sessions {
            out.line(format!(
                "let {} = context.session({}.self)",
                session_var(&session),
                session_class(&session)
            ));
        }
    }

    /// The window's close check: evaluated against its own drafts when
    /// closing is requested, with the declared save command.
    fn close_check(&self, out: &mut Out, close: &bern_ir::CloseCheck) {
        let engine = close.exprs().iter().any(|expr| expr.reads_engine())
            || matches!(close.save, Some(Action::Engine { .. }));
        out.line("");
        out.open("private func bernCloseCheck() -> BernCloseCheck {");
        out.line("let context = self.context");
        if engine {
            out.line("let bernEngine = self.bernEngine");
        }
        out.open("return BernCloseCheck(");
        out.open("needsConfirmation: {");
        self.locals(out, &[&close.when]);
        out.line(format!("return {}", self.expr(&close.when)));
        out.close("},");
        out.open("message: {");
        self.locals(out, &close.message.exprs());
        out.line(format!("return {}", self.text(&close.message)));
        out.close("},");
        match &close.save {
            Some(Action::Engine { command, args }) => {
                let spec = &self.app.protocol.commands[*command];
                out.open("save: { done in");
                self.locals(out, &args.iter().collect::<Vec<_>>());
                let mut values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                values.push("from: context".into());
                values.push("completion: done".into());
                out.line(format!(
                    "bernEngine.{}({})",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                ));
                out.close("}");
            }
            _ => out.line("save: nil"),
        }
        out.close(")");
        out.close("}");
    }
}
