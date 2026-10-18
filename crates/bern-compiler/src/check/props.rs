//! Props written as names, literals and resources, each checked for
//! the kind of prop it fills.

use super::{Checker, Scope};
use bern_ir as ir;
use bern_ir::spec::{self, PropKind};
use bern_protocol::Type;
use bern_syntax::{self as ast, Literal, UnaryOp};

impl Checker<'_> {
    pub(super) fn id(&mut self, expr: &ast::Expr) -> Option<String> {
        let (value, span) = match expr {
            ast::Expr::Path(path) if path.single().is_some() => {
                (path.single().unwrap().to_string(), path.span())
            }
            ast::Expr::Literal {
                value: Literal::Text(text),
                span,
            } => (text.clone(), *span),
            other => {
                self.error(other.span(), "expected an identifier");
                return None;
            }
        };
        if value.is_empty()
            || !value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            self.error(span, "ids use letters, digits, `_` and `-`");
        }
        if !self.ids.insert(value.clone()) {
            self.error(
                span,
                format!("control id `{value}` is used twice in this component"),
            );
        }
        Some(value)
    }

    /// `feature: Contract::Feature`, naming a declared feature.
    pub(super) fn feature(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        let ast::Expr::Variant(variant) = expr else {
            self.error(
                expr.span(),
                "expected a feature of the component's contract, e.g. `MediaFeature::Clip`",
            );
            return None;
        };
        let contract = variant.ty.to_string();
        let feature = variant.variant.to_string();
        let Some(features) = self.contracts.get(&contract) else {
            self.error(
                variant.ty.span(),
                format!("`{contract}` is not a `#[bern::features]` contract"),
            );
            return None;
        };
        if !features.contains(&feature) {
            self.error(
                variant.variant.span(),
                format!(
                    "contract `{contract}` has no feature `{feature}`; features: {}",
                    features.join(", ")
                ),
            );
            return None;
        }
        Some(ir::Prop::Feature { contract, feature })
    }

    pub(super) fn keyword(&mut self, expr: &ast::Expr, allowed: &[&str]) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Path(path)
                if path
                    .single()
                    .is_some_and(|word| allowed.contains(&word.to_string().as_str())) =>
            {
                Some(ir::Prop::Keyword {
                    value: path.single().unwrap().to_string(),
                })
            }
            other => {
                self.error(
                    other.span(),
                    format!("expected one of: {}", allowed.join(", ")),
                );
                None
            }
        }
    }

    pub(super) fn icon(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Literal {
                value: Literal::Text(name),
                span,
            } => {
                if name.is_empty()
                    || !name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
                {
                    self.error(
                        *span,
                        "icon names use the Bern icon set (SF Symbols naming), e.g. \"gearshape\"",
                    );
                    return None;
                }
                Some(ir::Prop::Icon { name: name.clone() })
            }
            other => {
                self.error(other.span(), "expected an icon name literal");
                None
            }
        }
    }

    /// `"images/cover.png"`: the name of a PNG or JPEG resource. Whether
    /// `[resources]` declares it is checked for the whole application,
    /// library components included, once the components are lowered.
    pub(super) fn svg_resource(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        let ast::Expr::Literal {
            value: Literal::Text(name),
            span,
        } = expr
        else {
            self.error(expr.span(), "expected a declared .svg resource");
            return None;
        };
        if let Err(message) = bern_protocol::check_resource_name(name) {
            self.error(*span, message);
            return None;
        }
        if !name.to_ascii_lowercase().ends_with(".svg") {
            self.error(*span, "Svg requires a .svg resource");
            return None;
        }
        Some(ir::Prop::Resource { name: name.clone() })
    }

    pub(super) fn image_resource(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        let ast::Expr::Literal {
            value: Literal::Text(name),
            span,
        } = expr
        else {
            self.error(
                expr.span(),
                "expected the name of a declared resource, e.g. \"images/cover.png\"",
            );
            return None;
        };
        if let Err(message) = bern_protocol::check_resource_name(name) {
            self.error(*span, message);
            return None;
        }
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase());
        if !extension.is_some_and(|ext| spec::IMAGE_EXTENSIONS.contains(&ext.as_str())) {
            self.error(
                *span,
                format!("`{name}` is not a PNG or JPEG image (.png, .jpg or .jpeg)"),
            );
            return None;
        }
        Some(ir::Prop::Resource { name: name.clone() })
    }

    fn number(&mut self, expr: &ast::Expr) -> Option<f64> {
        match expr {
            ast::Expr::Literal {
                value: Literal::Int(value),
                ..
            } => Some(*value as f64),
            ast::Expr::Literal {
                value: Literal::Float(value),
                ..
            } => Some(*value),
            ast::Expr::Unary {
                op: UnaryOp::Neg,
                expr,
                ..
            } => self.number(expr).map(|value| -value),
            other => {
                self.error(other.span(), "expected a numeric literal");
                None
            }
        }
    }

    pub(super) fn key(
        &mut self,
        expr: &ast::Expr,
        template: Option<&ir::Template>,
    ) -> Option<ir::Prop> {
        let (Some(field), Some(template)) = (
            match expr {
                ast::Expr::Path(path) => path.single().cloned(),
                _ => None,
            },
            template,
        ) else {
            self.error(
                expr.span(),
                "expected the name of a row field, e.g. `key: id`",
            );
            return None;
        };
        let Type::Record { name } = &template.ty else {
            return None;
        };
        let def = self.types.record_def(name)?;
        let Some(index) = def.field_index(&field.to_string()) else {
            self.error(field.span(), format!("`{name}` has no field `{field}`"));
            return None;
        };
        if !matches!(def.fields[index].ty, Type::Int | Type::Text) {
            self.error(
                field.span(),
                "list keys must be `i64` or `String` fields; positions are not identities",
            );
            return None;
        }
        Some(ir::Prop::Key {
            field: index,
            name: field.to_string(),
        })
    }

    pub(super) fn window_id(
        &mut self,
        expr: &ast::Expr,
        element: &str,
        prop: &str,
    ) -> Option<ir::Prop> {
        let ast::Expr::Path(path) = expr else {
            self.error(expr.span(), "expected a window identifier");
            return None;
        };
        let Some(id) = path.single() else {
            self.error(expr.span(), "expected a window identifier");
            return None;
        };
        let id = id.to_string();
        if !(element == "Window" && prop == "id") && !self.window_ids.contains(&id) {
            self.error(
                expr.span(),
                format!("window `{id}` is not declared in the WindowSet"),
            );
            return None;
        }
        Some(ir::Prop::Window { id })
    }

    /// `true` or `false`.
    pub(super) fn flag(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Literal {
                value: Literal::Bool(value),
                ..
            } => Some(ir::Prop::Keyword {
                value: value.to_string(),
            }),
            other => {
                self.error(other.span(), "expected `true` or `false`");
                None
            }
        }
    }

    /// A keyboard shortcut written as a literal, such as `"cmd+k"`.
    pub(super) fn shortcut(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Literal {
                value: Literal::Text(text),
                span,
            } => match spec::parse_shortcut(text) {
                Ok(value) => Some(ir::Prop::Keyword { value }),
                Err(message) => {
                    self.error(*span, format!("shortcut \"{text}\": {message}"));
                    None
                }
            },
            other => {
                self.error(other.span(), "a shortcut is a literal such as \"cmd+k\"");
                None
            }
        }
    }

    /// The kinds of file a control takes, written as a literal such as
    /// `"video .srt"`.
    pub(super) fn file_kinds(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Literal {
                value: Literal::Text(text),
                span,
            } => match spec::parse_file_kinds(text) {
                Ok(value) => Some(ir::Prop::Keyword { value }),
                Err(message) => {
                    self.error(*span, format!("files \"{text}\": {message}"));
                    None
                }
            },
            other => {
                self.error(
                    other.span(),
                    "the kinds of file are a literal such as \"video .srt\"",
                );
                None
            }
        }
    }

    /// A length that follows a numeric expression.
    pub(super) fn length(
        &mut self,
        prop_name: &str,
        expr: &ast::Expr,
        scope: &Scope,
    ) -> Option<ir::Prop> {
        match self.expr(expr, scope) {
            Some(lowered) if matches!(lowered.ty, Type::Int | Type::Float) => {
                Some(ir::Prop::Expr { expr: lowered })
            }
            Some(lowered) => {
                self.error(
                    expr.span(),
                    format!("`{prop_name}` is a length, found {}", lowered.ty.display()),
                );
                None
            }
            None => None,
        }
    }

    /// A numeric literal in the range its prop takes on `element`.
    pub(super) fn literal_number(
        &mut self,
        element: &str,
        prop_name: &str,
        expr: &ast::Expr,
    ) -> Option<ir::Prop> {
        self.number(expr).and_then(|value| {
            // Sizes of pictures and grid cells, and the total of a
            // progress, are lengths and amounts above zero.
            let positive = matches!(
                (element, prop_name),
                ("Image" | "Svg", "width" | "height")
                    | ("Grid", "min_width")
                    | ("Progress", "total")
            );
            let positive = positive
                || matches!(
                    (element, prop_name),
                    (_, "scroll_scale") | ("Swipe", "distance")
                );
            if positive && value <= 0.0 {
                self.error(expr.span(), format!("`{prop_name}` must be above zero"));
                return None;
            }
            if prop_name == "nudge_scale" && (!value.is_finite() || value <= 0.0) {
                self.error(
                    expr.span(),
                    "`nudge_scale` must be finite and greater than zero",
                );
                return None;
            }
            if prop_name == "appear_delay" && (!value.is_finite() || value < 0.0) {
                self.error(
                    expr.span(),
                    "`appear_delay` must be a finite, nonnegative number of seconds",
                );
                return None;
            }
            if prop_name == "stagger" && !(0.0..=1.0).contains(&value) {
                self.error(
                    expr.span(),
                    "`stagger` is a delay from 0 to 1 second between children",
                );
                return None;
            }
            if prop_name == "scroll_opacity" && !(0.0..=1.0).contains(&value) {
                self.error(expr.span(), "`scroll_opacity` is an opacity from 0 to 1");
                return None;
            }
            Some(ir::Prop::Number { value })
        })
    }

    /// A numeric expression.
    pub(super) fn number_value(
        &mut self,
        prop_name: &str,
        expr: &ast::Expr,
        scope: &Scope,
    ) -> Option<ir::Prop> {
        match self.expr(expr, scope) {
            Some(lowered) if matches!(lowered.ty, Type::Int | Type::Float) => {
                Some(ir::Prop::Expr { expr: lowered })
            }
            Some(lowered) => {
                self.error(
                    expr.span(),
                    format!(
                        "`{prop_name}` must be a number, found {}",
                        lowered.ty.display()
                    ),
                );
                None
            }
            None => None,
        }
    }

    /// A value of the `selection` an enclosing `Sidebar` or `Tabs` binds.
    pub(super) fn selection_value(
        &mut self,
        expr: &ast::Expr,
        selection: Option<Type>,
        scope: &Scope,
    ) -> Option<ir::Prop> {
        match selection {
            Some(selection) => self
                .expect(expr, &selection, scope, "this value")
                .map(|expr| ir::Prop::Expr { expr }),
            None => {
                self.error(
                    expr.span(),
                    "this value needs an enclosing `Sidebar` or `Tabs` selection",
                );
                None
            }
        }
    }

    /// A value of the `route` an enclosing `Navigation` binds.
    pub(super) fn route(
        &mut self,
        expr: &ast::Expr,
        route: Option<Type>,
        scope: &Scope,
    ) -> Option<ir::Prop> {
        match route {
            Some(route) => self
                .expect(expr, &route, scope, "the route")
                .map(|expr| ir::Prop::Expr { expr }),
            None => {
                self.error(expr.span(), "`NavLink` must be inside a `Navigation`");
                None
            }
        }
    }

    /// The name of a transition or motion token `style!` provides, or of
    /// a hero.
    pub(super) fn style_name(
        &mut self,
        kind: PropKind,
        prop_name: &str,
        expr: &ast::Expr,
    ) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Path(path) if path.single().is_some() => {
                let value = path.single().unwrap().to_string();
                let known = match kind {
                    PropKind::Transition => self.transitions.contains(&value),
                    PropKind::MotionToken => self.tokens.contains(&value),
                    _ => true,
                };
                if known {
                    Some(ir::Prop::Keyword { value })
                } else {
                    let what = if kind == PropKind::Transition {
                        "transition; declare it under `transition { … }` in `style!`"
                    } else {
                        "motion token; declare it under `motion { … }` in `style!`"
                    };
                    self.error(expr.span(), format!("`{value}` is not a {what}"));
                    None
                }
            }
            other => {
                self.error(other.span(), format!("`{prop_name}` takes a name"));
                None
            }
        }
    }

    /// A semantic color role, such as `accent` or `fill.subtle`.
    pub(super) fn color_role(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Path(path) => {
                let role = path.dotted();
                if bern_ir::style::COLOR_ROLES.contains(&role.as_str()) {
                    Some(ir::Prop::Keyword { value: role })
                } else {
                    self.error(
                        expr.span(),
                        format!(
                            "`{role}` is not a color role; use one of {}",
                            bern_ir::style::COLOR_ROLES.join(", ")
                        ),
                    );
                    None
                }
            }
            other => {
                self.error(
                    other.span(),
                    "a color is a role such as `accent` or `fill.subtle`",
                );
                None
            }
        }
    }

    /// The name of a pattern `style!` declares.
    pub(super) fn pattern_name(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Path(path) if path.single().is_some() => {
                let value = path.single().unwrap().to_string();
                if self.patterns.contains(&value) {
                    Some(ir::Prop::Keyword { value })
                } else {
                    self.error(
                        expr.span(),
                        format!("`{value}` is not a pattern; declare it under `pattern {{ … }}` in `style!`"),
                    );
                    None
                }
            }
            other => {
                self.error(other.span(), "`name` takes a pattern's name");
                None
            }
        }
    }

    /// A list of numbers.
    pub(super) fn number_list(
        &mut self,
        prop_name: &str,
        expr: &ast::Expr,
        scope: &Scope,
    ) -> Option<ir::Prop> {
        match self.expr(expr, scope) {
            Some(lowered) if matches!(&lowered.ty, Type::List { inner } if matches!(**inner, Type::Int | Type::Float)) => {
                Some(ir::Prop::Expr { expr: lowered })
            }
            Some(lowered) => {
                self.error(
                    expr.span(),
                    format!(
                        "`{prop_name}` is a list of numbers, found {}",
                        lowered.ty.display()
                    ),
                );
                None
            }
            None => None,
        }
    }

    /// A value that identifies something: text, a number or an enum.
    pub(super) fn key_value(
        &mut self,
        prop_name: &str,
        expr: &ast::Expr,
        scope: &Scope,
    ) -> Option<ir::Prop> {
        match self.expr(expr, scope) {
            Some(lowered) if matches!(lowered.ty, Type::Int | Type::Text | Type::Enum { .. }) => {
                Some(ir::Prop::Expr { expr: lowered })
            }
            Some(lowered) => {
                self.error(
                    expr.span(),
                    format!(
                        "`{prop_name}` identifies with text, a number or an enum, found {}",
                        lowered.ty.display()
                    ),
                );
                None
            }
            None => None,
        }
    }

    /// A variant `style!` declares for `element`.
    pub(super) fn style_variant(&mut self, element: &str, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Path(path) if path.single().is_some() => {
                let variant = path.single().unwrap().to_string();
                if self
                    .variants
                    .iter()
                    .any(|(component, declared)| *component == element && *declared == variant)
                {
                    Some(ir::Prop::Keyword { value: variant })
                } else {
                    self.error(
                        expr.span(),
                        format!("`style!` declares no variant `{element}.{variant}`"),
                    );
                    None
                }
            }
            other => {
                self.error(other.span(), "a variant is a name, e.g. `variant: hero`");
                None
            }
        }
    }

    /// The name of a type.
    pub(super) fn type_name(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Path(path) if path.single().is_some() => Some(ir::Prop::TypeName {
                name: path.single().unwrap().to_string(),
            }),
            other => {
                self.error(other.span(), "expected a type name");
                None
            }
        }
    }
}
