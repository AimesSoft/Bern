//! Bindings: paths into engine and session state, players and media
//! sources, and the session fields controls write to.

use super::{Checker, FieldExpect, Root, Scope, is_labeled_enum};
use bern_ir as ir;
use bern_protocol::Type;
use bern_syntax as ast;

impl Checker<'_> {
    pub(super) fn media_source(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::Prop> {
        let ast::Expr::Path(path) = expr else {
            self.error(
                expr.span(),
                "expected a `MediaSource` binding such as `app.clip`",
            );
            return None;
        };
        let lowered = self.binding(path, scope)?;
        let source = Type::Record {
            name: bern_media::MEDIA_SOURCE.into(),
        };
        if lowered.ty != source && lowered.ty != Type::optional(source) {
            self.error(
                path.span(),
                format!(
                    "`source` needs a MediaSource or Option<MediaSource>, found {}",
                    lowered.ty.display()
                ),
            );
            return None;
        }
        Some(ir::Prop::Expr { expr: lowered })
    }

    /// `players.preview` names an application player.
    pub(super) fn player_ref(&mut self, segments: &[syn::Ident], scope: &Scope) -> Option<String> {
        let root = segments.first()?;
        if !matches!(scope.get(&root.to_string()), Some(Root::Players)) || segments.len() != 2 {
            self.error(
                root.span(),
                "expected an application player such as `players.preview`",
            );
            return None;
        }
        let id = segments[1].to_string();
        if !self.player_ids.contains(&id) {
            self.error(
                segments[1].span(),
                format!("player `{id}` is not declared in the #[bern::players] catalog"),
            );
            return None;
        }
        Some(id)
    }

    pub(super) fn player(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::Prop> {
        let ast::Expr::Path(path) = expr else {
            self.error(
                expr.span(),
                "expected an application player such as `players.preview`",
            );
            return None;
        };
        self.player_ref(&path.segments, scope)
            .map(|id| ir::Prop::Player { id })
    }

    pub(super) fn field_prop(
        &mut self,
        expr: &ast::Expr,
        scope: &Scope,
        expect: FieldExpect,
    ) -> Option<ir::Prop> {
        self.session_field(expr, scope, expect)
            .map(|field| ir::Prop::Field { field })
    }

    pub(super) fn session_field(
        &mut self,
        expr: &ast::Expr,
        scope: &Scope,
        expect: FieldExpect,
    ) -> Option<ir::FieldRef> {
        let ast::Expr::Path(path) = expr else {
            self.error(expr.span(), "expected a session field such as `draft.name`");
            return None;
        };
        let root = path.segments[0].to_string();
        match scope.get(&root) {
            Some(Root::Session(session)) if path.segments.len() == 2 => {
                let session = session.clone();
                let def = self.sessions.iter().find(|def| def.name == session)?;
                let field_name = path.segments[1].to_string();
                let Some(field) = def.field_index(&field_name) else {
                    self.error(
                        path.segments[1].span(),
                        format!("`{session}` has no field `{field_name}`"),
                    );
                    return None;
                };
                let ty = def.fields[field].ty.clone();
                let ok = match expect {
                    FieldExpect::Text => ty == Type::Text,
                    FieldExpect::Bool => ty == Type::Bool,
                    FieldExpect::Number => ty == Type::Float,
                    FieldExpect::File => {
                        let source = Type::Record {
                            name: bern_media::MEDIA_SOURCE.into(),
                        };
                        ty == source || ty == Type::optional(source)
                    }
                    FieldExpect::Position => matches!(ty, Type::Int | Type::Text),
                    FieldExpect::Picker => is_labeled_enum(&ty, self.types),
                    FieldExpect::Selection => matches!(ty, Type::Enum { .. }),
                    FieldExpect::Path => {
                        matches!(&ty, Type::List { inner } if matches!(**inner, Type::Enum { .. }))
                    }
                };
                if !ok {
                    let wanted = match expect {
                        FieldExpect::Text => "a `String` field",
                        FieldExpect::Bool => "a `bool` field",
                        FieldExpect::Number => "an `f64` field",
                        FieldExpect::File => "a `MediaSource` or `Option<MediaSource>` field",
                        FieldExpect::Position => {
                            "an `i64` or `String` field, like the keys of the items"
                        }
                        FieldExpect::Picker => "an enum field whose variants all have labels",
                        FieldExpect::Selection => "an enum field",
                        FieldExpect::Path => "a `Vec` of a route enum",
                    };
                    self.error(
                        path.span(),
                        format!("expected {wanted}, found {}", ty.display()),
                    );
                    return None;
                }
                Some(ir::FieldRef { session, field, ty })
            }
            Some(Root::Client(_)) => {
                self.error(
                    path.span(),
                    "controls only write to session state; engine state changes through commands",
                );
                None
            }
            None => {
                self.error(path.span(), format!("unknown binding `{root}`"));
                None
            }
            _ => {
                self.error(path.span(), "expected a session field such as `draft.name`");
                None
            }
        }
    }

    fn record_path(&mut self, mut ty: Type, segments: &[syn::Ident]) -> Option<(Vec<usize>, Type)> {
        let mut path = Vec::new();
        for segment in segments {
            let Type::Record { name } = &ty else {
                self.error(
                    segment.span(),
                    format!("{} has no field `{segment}`", ty.display()),
                );
                return None;
            };
            let def = self.types.record_def(name)?;
            let Some(index) = def.field_index(&segment.to_string()) else {
                self.error(segment.span(), format!("`{name}` has no field `{segment}`"));
                return None;
            };
            path.push(index);
            ty = def.fields[index].ty.clone();
        }
        Some((path, ty))
    }

    pub(super) fn binding(&mut self, path: &ast::PathExpr, scope: &Scope) -> Option<ir::Expr> {
        let root_name = path.segments[0].to_string();
        // `window` names the hosting window's UI state unless a parameter
        // or variable takes the name.
        if root_name == "window" && scope.get(&root_name).is_none() {
            let field = path.segments.get(1).map(ToString::to_string);
            let sizes = ir::LayoutSize::ALL.map(ir::LayoutSize::name);
            let known = field.as_deref() == Some("idle")
                || field.as_deref().is_some_and(|name| sizes.contains(&name));
            if !known || path.segments.len() != 2 {
                self.error(
                    path.span(),
                    "the hosting window offers `window.idle` (no input for `idle_after` seconds) and its size: `window.compact`, `window.medium`, `window.expanded`, `window.large`",
                );
                return None;
            }
            let field = field.unwrap_or_default();
            if field != "idle" {
                match self.layouts {
                    None => {
                        self.error(
                            path.span(),
                            format!("error[B402]: `window.{field}` needs the breakpoints of `[layouts]` in Bern.toml, e.g. `compact_below = 600`"),
                        );
                    }
                    Some(config) => {
                        let size = ir::LayoutSize::parse(&field).expect("size");
                        if !config.has(size) {
                            let breakpoint = if size == ir::LayoutSize::Medium {
                                "medium_below"
                            } else {
                                "expanded_below"
                            };
                            self.error(
                                path.span(),
                                format!("`window.{field}` needs `{breakpoint}` in `[layouts]` of Bern.toml; without it the window is never {field}"),
                            );
                        }
                    }
                }
            }
            return Some(ir::Expr {
                kind: ir::ExprKind::Binding {
                    binding: ir::Binding::Window { field },
                },
                ty: Type::Bool,
            });
        }
        let Some(root) = scope.get(&root_name).cloned() else {
            self.error(
                path.span(),
                format!(
                    "unknown binding `{root_name}`; bindings start at a component parameter or template variable"
                ),
            );
            return None;
        };
        let rest = &path.segments[1..];
        let (binding, ty) = match root {
            Root::Client(namespace) => {
                let Some(first) = rest.first() else {
                    self.error(
                        path.span(),
                        "the client is not a value; read a state field such as `app.field`",
                    );
                    return None;
                };
                if namespace == self.protocol.namespace {
                    let Some(field) = self.protocol.state_index(&first.to_string()) else {
                        self.error(
                            first.span(),
                            format!(
                                "protocol `{}` has no state field `{first}`",
                                self.protocol.namespace
                            ),
                        );
                        return None;
                    };
                    let (sub, ty) =
                        self.record_path(self.protocol.state[field].ty.clone(), &rest[1..])?;
                    (ir::Binding::Engine { field, path: sub }, ty)
                } else {
                    // A module's state is the application state field named
                    // after the module.
                    let field = self.protocol.state_index(&namespace)?;
                    let (sub, ty) =
                        self.record_path(self.protocol.state[field].ty.clone(), rest)?;
                    (ir::Binding::Engine { field, path: sub }, ty)
                }
            }
            Root::Windows => {
                self.error(path.span(), "window handles are only used in actions");
                return None;
            }
            Root::Players => {
                self.error(
                    path.span(),
                    "players are used by `Video(player: …)` and in actions such as `players.preview.play()`",
                );
                return None;
            }
            Root::Session(session) => {
                let Some(first) = rest.first() else {
                    self.error(
                        path.span(),
                        "session state is not a value; read a field such as `draft.name`",
                    );
                    return None;
                };
                let def = self.sessions.iter().find(|def| def.name == session)?;
                let Some(field) = def.field_index(&first.to_string()) else {
                    self.error(first.span(), format!("`{session}` has no field `{first}`"));
                    return None;
                };
                let field_ty = def.fields[field].ty.clone();
                let (sub, ty) = self.record_path(field_ty, &rest[1..])?;
                (
                    ir::Binding::Session {
                        session,
                        field,
                        path: sub,
                    },
                    ty,
                )
            }
            Root::Local(root) => {
                let (sub, ty) = self.record_path(root.clone(), rest)?;
                (
                    ir::Binding::Local {
                        name: root_name,
                        path: sub,
                        root,
                    },
                    ty,
                )
            }
        };
        Some(ir::Expr {
            kind: ir::ExprKind::Binding { binding },
            ty,
        })
    }
}
