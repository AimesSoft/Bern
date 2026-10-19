//! Actions: the commands, window, session and media operations a
//! control runs.

use super::{Checker, Root, Scope};
use bern_ir as ir;
use bern_protocol::Type;
use bern_syntax as ast;

impl Checker<'_> {
    pub(super) fn action(&mut self, action: &ast::Action, scope: &Scope) -> Option<ir::Action> {
        let root = action.receiver[0].to_string();
        if root == "pages" && scope.get(&root).is_none() {
            return self.page_action(action);
        }
        match scope.get(&root) {
            Some(Root::Client(namespace)) => {
                if action.receiver.len() != 1 {
                    self.error(
                        action.span,
                        "commands are called directly on the client, e.g. `app.save()`",
                    );
                    return None;
                }
                let method = action.method.to_string();
                let id = format!("{namespace}.{method}");
                let Some(command) = self.protocol.commands.iter().position(|c| c.id == id) else {
                    self.error(
                        action.method.span(),
                        format!(
                            "protocol `{namespace}` has no command `{method}`; register it in the command trait"
                        ),
                    );
                    return None;
                };
                let spec = self.protocol.commands[command].clone();
                if spec.is_query() {
                    self.error(
                        action.method.span(),
                        format!(
                            "`{method}` is a query: it only reads, so it is shown with \
                             `Query(query: {root}.{method}(…))`; actions run commands"
                        ),
                    );
                    return None;
                }
                if spec.params.len() != action.args.len() {
                    self.error(
                        action.method.span(),
                        format!(
                            "`{}` takes {} argument(s), found {}",
                            spec.id,
                            spec.params.len(),
                            action.args.len()
                        ),
                    );
                    return None;
                }
                let mut args = Vec::new();
                for (arg, param) in action.args.iter().zip(&spec.params) {
                    args.push(self.expect(
                        arg,
                        &param.ty,
                        scope,
                        &format!("argument `{}`", param.name),
                    )?);
                }
                Some(ir::Action::Engine { command, args })
            }
            Some(Root::Windows) => {
                if action.receiver.len() != 2 || !action.args.is_empty() {
                    self.error(
                        action.span,
                        "window actions look like `windows.settings.open()`",
                    );
                    return None;
                }
                let window = action.receiver[1].to_string();
                if !self.window_ids.contains(&window) {
                    self.error(
                        action.receiver[1].span(),
                        format!("window `{window}` is not declared"),
                    );
                    return None;
                }
                let op = match action.method.to_string().as_str() {
                    "open" => ir::WindowOp::Open,
                    "close" => ir::WindowOp::Close,
                    "activate" => ir::WindowOp::Activate,
                    other => {
                        self.error(
                            action.method.span(),
                            format!("windows support open, close and activate, not `{other}`"),
                        );
                        return None;
                    }
                };
                Some(ir::Action::Window { op, window })
            }
            Some(Root::Session(session)) => {
                if action.receiver.len() != 2 {
                    self.error(
                        action.span,
                        "session actions look like `draft.panel.set(true)` or `draft.panel.toggle()`",
                    );
                    return None;
                }
                let session = session.clone();
                let def = self.sessions.iter().find(|def| def.name == session)?;
                let name = action.receiver[1].to_string();
                let Some(field) = def.field_index(&name) else {
                    self.error(
                        action.receiver[1].span(),
                        format!("`{session}` has no field `{name}`"),
                    );
                    return None;
                };
                let ty = def.fields[field].ty.clone();
                let field = ir::FieldRef { session, field, ty };
                let op = match (action.method.to_string().as_str(), action.args.as_slice()) {
                    ("set", [value]) => ir::SessionOp::Set {
                        value: self.expect(value, &field.ty, scope, "the new value")?,
                    },
                    ("toggle", []) if field.ty == Type::Bool => ir::SessionOp::Toggle,
                    ("toggle", []) => {
                        self.error(
                            action.method.span(),
                            format!(
                                "`toggle()` flips a `bool`; `{name}` is {}",
                                field.ty.display()
                            ),
                        );
                        return None;
                    }
                    _ => {
                        self.error(
                            action.method.span(),
                            "session fields support `set(value)` and, for a `bool`, `toggle()`",
                        );
                        return None;
                    }
                };
                Some(ir::Action::Session { field, op })
            }
            Some(Root::Players) => {
                let player = self.player_ref(&action.receiver, scope)?;
                let method = action.method.to_string();
                let Some(op) = ir::MediaOp::ALL
                    .into_iter()
                    .find(|op| op.method() == method)
                else {
                    self.error(
                        action.method.span(),
                        format!(
                            "players support {}, not `{method}`",
                            ir::MediaOp::ALL.map(ir::MediaOp::method).join(", ")
                        ),
                    );
                    return None;
                };
                let params: Vec<(&str, Type)> = match op {
                    ir::MediaOp::Open => vec![(
                        "source",
                        Type::Record {
                            name: bern_media::MEDIA_SOURCE.into(),
                        },
                    )],
                    ir::MediaOp::Seek => vec![("position_ms", Type::Int)],
                    ir::MediaOp::SetVolume => vec![("volume", Type::Float)],
                    ir::MediaOp::SetMuted => vec![("muted", Type::Bool)],
                    ir::MediaOp::Play | ir::MediaOp::Pause => vec![],
                };
                if params.len() != action.args.len() {
                    self.error(
                        action.method.span(),
                        format!(
                            "`{}` takes {} argument(s), found {}",
                            op.command(),
                            params.len(),
                            action.args.len()
                        ),
                    );
                    return None;
                }
                let mut args = Vec::new();
                for (arg, (name, ty)) in action.args.iter().zip(&params) {
                    args.push(self.expect(arg, ty, scope, &format!("argument `{name}`"))?);
                }
                Some(ir::Action::Media { player, op, args })
            }
            _ => {
                self.error(
                    action.receiver[0].span(),
                    format!("unknown client `{root}`; actions call a typed client parameter"),
                );
                None
            }
        }
    }

    /// `pages.new.show()`: show a page of a page slot, wherever the
    /// shell shows that slot. It sets the session field of the slot's enum,
    /// which the shell's `Sidebar`, `Tabs` or `PageContent` select by, so a
    /// module can link to its own pages without knowing the shell.
    fn page_action(&mut self, action: &ast::Action) -> Option<ir::Action> {
        if action.receiver.len() != 2 || action.method != "show" || !action.args.is_empty() {
            self.error(action.span, "page actions look like `pages.new.show()`");
            return None;
        }
        let id = action.receiver[1].to_string();
        let variant = ast::pascal_case(&id);
        let slots: Vec<(String, usize)> = self
            .contributions
            .pages
            .values()
            .filter_map(|slot| {
                let def = self.types.enum_def(&slot.enum_name)?;
                Some((slot.enum_name.clone(), def.variant_index(&variant)?))
            })
            .collect();
        let [(enum_name, index)] = slots.as_slice() else {
            self.error(
                action.receiver[1].span(),
                if slots.is_empty() {
                    format!("no page slot has a page `{id}`")
                } else {
                    format!("several page slots have a page `{id}`")
                },
            );
            return None;
        };
        let ty = Type::Enum {
            name: enum_name.clone(),
        };
        let fields: Vec<ir::FieldRef> = self
            .sessions
            .iter()
            .flat_map(|def| {
                def.fields
                    .iter()
                    .enumerate()
                    .filter(|(_, field)| field.ty == ty)
                    .map(|(field, _)| ir::FieldRef {
                        session: def.name.clone(),
                        field,
                        ty: ty.clone(),
                    })
            })
            .collect();
        let [field] = fields.as_slice() else {
            self.error(
                action.span,
                format!(
                    "`pages.{id}.show()` sets the session field that selects `{enum_name}`; \
                     declare exactly one such field, found {}",
                    fields.len()
                ),
            );
            return None;
        };
        self.page_sessions
            .insert((self.component.clone(), field.session.clone()));
        Some(ir::Action::Session {
            field: field.clone(),
            op: ir::SessionOp::Set {
                value: ir::Expr {
                    ty,
                    kind: ir::ExprKind::Variant {
                        variant: *index as u32,
                        fields: Vec::new(),
                    },
                },
            },
        })
    }
}
