//! What modules contribute to `Slot`, `Pages` and `PageContent`,
//! written as ordinary declarations and checked like them.

use super::{Checker, Ctx, Root, Scope, slot_client};
use bern_ir as ir;
use bern_ir::spec::{self, ElementShape};
use bern_syntax as ast;

/// The window handle a slot's expansion offers its contributed components.
const SLOT_WINDOWS: &str = "bern_slot_windows";

impl Checker<'_> {
    /// Replace a `Slot`, `Pages` or `PageContent` with what the modules
    /// contribute, written as ordinary declarations and checked like them.
    pub(super) fn expand(
        &mut self,
        element: &ast::Element,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Vec<ir::Node> {
        let name = element.name.to_string();
        let prop_names: Vec<String> = element
            .props
            .iter()
            .map(|prop| prop.name.to_string())
            .collect();
        let shape = ElementShape {
            name: &name,
            props: prop_names.iter().map(String::as_str).collect(),
            has_template: element.template.is_some(),
            children: element.children.as_ref().map(|_| Vec::new()),
            has_action: element.action.is_some(),
            parent: ctx.parent.as_deref(),
            is_root: ctx.is_root,
        };
        let errors = spec::check_shape(&shape);
        if !errors.is_empty() {
            for error in errors {
                self.error(element.span, error.message);
            }
            return Vec::new();
        }
        let ident = |prop: &str| match element.prop(prop).map(|prop| &prop.value) {
            Some(ast::Expr::Path(path)) => path.single().map(ToString::to_string),
            _ => None,
        };
        let key = if name == "Slot" { "id" } else { "slot" };
        let Some(slot) = ident(key) else {
            self.error(
                element.span,
                format!("`{name}` names its slot with `{key}: name`"),
            );
            return Vec::new();
        };
        let text = match name.as_str() {
            "Slot" => {
                let contributed = self
                    .contributions
                    .slots
                    .get(&slot)
                    .cloned()
                    .unwrap_or_default();
                let mut calls = Vec::new();
                for (module, component) in &contributed {
                    match self.slot_call(component, scope) {
                        Ok(call) => calls.push(self.contributions.guarded(module, call)),
                        Err(message) => self.error(element.span, message),
                    }
                }
                calls.join("\n")
            }
            "Pages" => {
                let Some(pages) = self.contributions.pages.get(&slot).cloned() else {
                    return Vec::new();
                };
                // In `Tabs`, every page is a tab showing its component.
                let tabs = ctx.parent.as_deref() == Some("Tabs");
                let mut failures = Vec::new();
                let text = pages
                    .pages
                    .iter()
                    .filter_map(|page| {
                        let label: Vec<String> = page
                            .label
                            .iter()
                            .map(|(language, text)| format!("{language:?} => {text:?}"))
                            .collect();
                        let icon = page
                            .icon
                            .as_ref()
                            .map(|icon| format!(", icon: {icon:?}"))
                            .unwrap_or_default();
                        if tabs {
                            return match self.slot_call(&page.component, scope) {
                                Ok(call) => Some(self.contributions.guarded(
                                    &page.module,
                                    format!(
                                        "Tab(value: {}::{}, label: tr! {{ {} }}{icon}) {{ {call} }}",
                                        pages.enum_name,
                                        page.variant,
                                        label.join(", ")
                                    ),
                                )),
                                Err(message) => {
                                    failures.push(message);
                                    None
                                }
                            };
                        }
                        let badge = page
                            .badge
                            .as_ref()
                            .map(|field| format!(", badge: {}.{field}", slot_client(&page.module)))
                            .unwrap_or_default();
                        Some(self.contributions.guarded(
                            &page.module,
                            format!(
                                "SidebarItem(value: {}::{}, label: tr! {{ {} }}{icon}{badge})",
                                pages.enum_name,
                                page.variant,
                                label.join(", ")
                            ),
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                for message in failures {
                    self.error(element.span, message);
                }
                text
            }
            _ => {
                let selected = match element.prop("selected").map(|prop| &prop.value) {
                    Some(ast::Expr::Path(path)) => path.dotted(),
                    _ => {
                        self.error(
                            element.span,
                            "`selected:` binds the session field holding the page",
                        );
                        return Vec::new();
                    }
                };
                let Some(pages) = self.contributions.pages.get(&slot) else {
                    return Vec::new();
                };
                let mut arms = Vec::new();
                let mut failures = Vec::new();
                for page in &pages.pages {
                    match self.slot_call(&page.component, scope) {
                        Ok(call) => arms.push(format!(
                            "{}::{} => {{ {} }}",
                            pages.enum_name,
                            page.variant,
                            self.contributions.guarded(&page.module, call)
                        )),
                        Err(message) => failures.push(message),
                    }
                }
                for message in failures {
                    self.error(element.span, message);
                }
                if arms.is_empty() {
                    return Vec::new();
                }
                format!("match {selected} {{ {} _ => {{}} }}", arms.join(" "))
            }
        };
        if text.is_empty() {
            return Vec::new();
        }
        let parsed = match text
            .parse::<proc_macro2::TokenStream>()
            .map_err(|error| error.to_string())
            .and_then(|tokens| ast::parse_ui(tokens).map_err(|error| error.to_string()))
        {
            Ok(body) => body,
            Err(error) => {
                self.error(element.span, format!("cannot expand `{name}`: {error}"));
                return Vec::new();
            }
        };
        let pushed = self.contributions.namespaces.clone();
        for namespace in &pushed {
            scope.push(slot_client(namespace), Root::Client(namespace.clone()));
        }
        // Window handles name windows only, so a contributed component
        // gets the application's whether or not the slot's component has it.
        scope.push(SLOT_WINDOWS.to_string(), Root::Windows);
        let lowered = self.nodes(&parsed.nodes, &format!("{path}.{name}"), ctx, scope);
        scope.pop();
        for _ in &pushed {
            scope.pop();
        }
        lowered
    }

    /// The call of a contributed component: its clients from the slot's
    /// scope, the window handle and players by the names the shell gave
    /// them; session state comes from the hosting window as usual.
    fn slot_call(&self, component: &str, scope: &Scope) -> Result<String, String> {
        let signature = self
            .signatures
            .get(component)
            .ok_or_else(|| format!("contributed component `{component}` is not declared"))?;
        let mut args = Vec::new();
        for (param, kind) in &signature.params {
            match kind {
                ir::ParamKind::Client { namespace } => {
                    args.push(format!("{param}: {}", slot_client(namespace)));
                }
                ir::ParamKind::Windows => {
                    let name = scope
                        .name_of(|root| matches!(root, Root::Windows))
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| SLOT_WINDOWS.to_string());
                    args.push(format!("{param}: {name}"));
                }
                ir::ParamKind::Players => {
                    let name = scope
                        .name_of(|root| matches!(root, Root::Players))
                        .ok_or_else(|| format!("contributed component `{component}` takes the players; give the component holding the slot a `players` parameter"))?;
                    args.push(format!("{param}: {name}"));
                }
                ir::ParamKind::Session { .. } => {}
                ir::ParamKind::Value { .. } => {
                    return Err(format!(
                        "contributed component `{component}` takes the value `{param}`; components placed in slots take clients, the window handle, players and session state only"
                    ));
                }
            }
        }
        Ok(format!("{component}({})", args.join(", ")))
    }
}
