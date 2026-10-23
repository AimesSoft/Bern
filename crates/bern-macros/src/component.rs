//! Expansion of `#[component]` functions.
//!
//! The body becomes never-executed Rust that `rustc` type-checks: each
//! binding is a typed reference, each action calls a typed client method,
//! each component reference constructs the callee's property struct.
//! Structural rules come from the shared component specifications.

use crate::config::Languages;
use bern_ir::spec::{self, ChildShape, ElementShape, PropKind};
use bern_syntax::*;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashSet;
use syn::Ident;

/// Re-run the macro when `Bern.toml` changes.
pub fn track(languages: Option<&Languages>) -> TokenStream {
    match languages {
        Some(languages) => {
            let path = languages.path.display().to_string();
            quote!(
                const _: &[u8] = ::core::include_bytes!(#path);
            )
        }
        None => TokenStream::new(),
    }
}

/// Check one `tr!` against the project languages and its own arguments.
pub fn check_translations(tr: &Tr, languages: Option<&Languages>) -> syn::Result<()> {
    let config = languages.map(|languages| (languages.supported.as_slice(), languages.strict));
    combine(
        bern_syntax::lower::check_translations(tr, config)
            .into_iter()
            .map(|(span, message)| syn::Error::new(span, message))
            .collect(),
    )
}

/// Whether a branch of an `if` value is, or chains to, a fractional literal.
fn float_leaf(expr: &Expr) -> bool {
    match expr {
        Expr::Literal {
            value: Literal::Float(_),
            ..
        } => true,
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
            ..
        } => float_leaf(expr),
        Expr::If {
            then, otherwise, ..
        } => float_leaf(then) || float_leaf(otherwise),
        _ => false,
    }
}

fn combine(errors: Vec<syn::Error>) -> syn::Result<()> {
    let mut iter = errors.into_iter();
    match iter.next() {
        None => Ok(()),
        Some(mut first) => {
            for error in iter {
                first.combine(error);
            }
            Err(first)
        }
    }
}

#[derive(Clone, Default)]
struct Ctx {
    parent: Option<String>,
    is_root: bool,
    selection: Option<Ident>,
    route: Option<Ident>,
}

struct Gen<'a> {
    languages: Option<&'a Languages>,
    strict: bool,
    errors: Vec<syn::Error>,
    counter: usize,
    scope: Vec<String>,
    ids: HashSet<String>,
    items: Vec<TokenStream>,
}

impl<'a> Gen<'a> {
    fn error(&mut self, span: Span, message: impl std::fmt::Display) {
        self.errors.push(syn::Error::new(span, message.to_string()));
    }

    fn fresh(&mut self, prefix: &str) -> Ident {
        self.counter += 1;
        format_ident!("__bern_{}_{}", prefix, self.counter)
    }

    fn in_scope(&self, name: &str) -> bool {
        self.scope.iter().any(|entry| entry == name)
    }

    fn path(&mut self, path: &PathExpr) -> TokenStream {
        let root = &path.segments[0];
        // `window.idle`: the hosting window's UI state.
        if root == "window" && !self.in_scope("window") {
            let rest = &path.segments[1..];
            return quote!(::bern::__private::window() #(.#rest)*);
        }
        if !self.in_scope(&root.to_string()) {
            self.error(
                root.span(),
                format!(
                    "unknown binding `{root}`; bindings start at a component parameter or \
                     template variable"
                ),
            );
        }
        let segments = &path.segments;
        quote!(#(#segments).*)
    }

    fn value(&mut self, expr: &Expr) -> TokenStream {
        match expr {
            Expr::Literal { value, span } => match value {
                Literal::Text(text) => quote_spanned!(*span=> ::std::string::String::from(#text)),
                Literal::Int(value) => {
                    let literal = proc_macro2::Literal::i64_suffixed(*value);
                    quote!(#literal)
                }
                Literal::Float(value) => {
                    let literal = proc_macro2::Literal::f64_suffixed(*value);
                    quote!(#literal)
                }
                Literal::Bool(value) => quote!(#value),
            },
            Expr::Tr(tr) => {
                self.error(
                    tr.span,
                    "translations are display text and cannot be used as values",
                );
                quote!(())
            }
            Expr::Path(path) => {
                let tokens = self.path(path);
                quote_spanned!(path.span()=> ::bern::__private::value_of(&#tokens))
            }
            Expr::Variant(variant) => {
                let ty = &variant.ty;
                let name = &variant.variant;
                if variant.has_braces {
                    let names: Vec<_> = variant.fields.iter().map(|(name, _)| name).collect();
                    let values: Vec<_> = variant
                        .fields
                        .iter()
                        .map(|(_, value)| self.value(value))
                        .collect();
                    quote!(#ty::#name { #(#names: #values),* })
                } else {
                    quote!(#ty::#name)
                }
            }
            Expr::Unary { op, expr, .. } => {
                let inner = self.value(expr);
                match op {
                    UnaryOp::Not => quote!(!(#inner)),
                    UnaryOp::Neg => quote!(-(#inner)),
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let lhs = self.value(lhs);
                let rhs = self.value(rhs);
                let op = match op {
                    BinaryOp::Eq => quote!(==),
                    BinaryOp::Ne => quote!(!=),
                    BinaryOp::Lt => quote!(<),
                    BinaryOp::Le => quote!(<=),
                    BinaryOp::Gt => quote!(>),
                    BinaryOp::Ge => quote!(>=),
                    BinaryOp::And => quote!(&&),
                    BinaryOp::Or => quote!(||),
                };
                // Parenthesized whole, so a reference taken to it, as for a
                // component argument, takes the result.
                quote!(((#lhs) #op (#rhs)))
            }
            Expr::Query(call) => {
                self.error(
                    call.span,
                    "a query call is only valid as the `query:` property of `Query`",
                );
                quote!(())
            }
            Expr::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                // `if a { 0 } else { 40.5 }` is a number like the build
                // tool reads it: whole literals beside a fraction are f64.
                let float = float_leaf(then) || float_leaf(otherwise);
                let cond = self.value(cond);
                let then = self.branch(then, float);
                let otherwise = self.branch(otherwise, float);
                quote!(if #cond { #then } else { #otherwise })
            }
        }
    }

    /// A branch of an `if` value, its whole literals written as f64 when
    /// `float`.
    fn branch(&mut self, expr: &Expr, float: bool) -> TokenStream {
        match expr {
            Expr::Literal {
                value: Literal::Int(value),
                ..
            } if float => {
                let literal = proc_macro2::Literal::f64_suffixed(*value as f64);
                quote!(#literal)
            }
            Expr::Unary {
                op: UnaryOp::Neg,
                expr,
                ..
            } if float
                && matches!(
                    **expr,
                    Expr::Literal {
                        value: Literal::Int(_),
                        ..
                    }
                ) =>
            {
                let inner = self.branch(expr, float);
                quote!(-(#inner))
            }
            Expr::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                let cond = self.value(cond);
                let then = self.branch(then, float);
                let otherwise = self.branch(otherwise, float);
                quote!(if #cond { #then } else { #otherwise })
            }
            other => self.value(other),
        }
    }

    /// An expression whose presentation is shown: references stay
    /// references, other expressions become values.
    fn display_arg(&mut self, expr: &Expr) -> TokenStream {
        match expr {
            Expr::Path(path) => {
                let tokens = self.path(path);
                quote!(&#tokens)
            }
            other => {
                let value = self.value(other);
                quote!(&#value)
            }
        }
    }

    fn display(&mut self, expr: &Expr, localized_only: bool) -> TokenStream {
        match expr {
            Expr::Tr(tr) => {
                if let Err(error) = check_translations(tr, self.languages) {
                    self.errors.push(error);
                }
                let args: Vec<_> = tr
                    .args
                    .iter()
                    .map(|arg| {
                        let value = self.display_arg(&arg.value);
                        let span = arg.value.span();
                        quote_spanned!(span=> ::bern::__private::interpolate(#value);)
                    })
                    .collect();
                quote!(#(#args)* ::bern::__private::display(&::bern::__private::localized());)
            }
            Expr::Literal {
                value: Literal::Text(_),
                span,
            } => {
                if self.strict {
                    self.error(
                        *span,
                        "error[B103]: user-visible text must be declared with its translations; \
                         use `tr! { \"zh-Hans\" => \"…\", \"en\" => \"…\" }`",
                    );
                }
                TokenStream::new()
            }
            Expr::Path(path) if !localized_only => {
                let tokens = self.path(path);
                quote_spanned!(path.span()=> ::bern::__private::display(&#tokens);)
            }
            other => {
                self.error(
                    other.span(),
                    if localized_only {
                        "expected `tr! { … }` text"
                    } else {
                        "expected `tr! { … }` text or a displayable binding"
                    },
                );
                TokenStream::new()
            }
        }
    }

    fn writable(&mut self, expr: &Expr, check: TokenStream) -> TokenStream {
        match expr {
            Expr::Path(path) if path.segments.len() >= 2 => {
                let tokens = self.path(path);
                quote_spanned!(path.span()=> #check(&#tokens);)
            }
            other => {
                self.error(
                    other.span(),
                    "expected a session field such as `draft.name`; controls only write to \
                     session state, engine state changes through commands",
                );
                TokenStream::new()
            }
        }
    }

    fn literal_number(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal {
                value: Literal::Int(_) | Literal::Float(_),
                ..
            } => {}
            Expr::Unary {
                op: UnaryOp::Neg,
                expr,
                ..
            } if matches!(
                **expr,
                Expr::Literal {
                    value: Literal::Int(_) | Literal::Float(_),
                    ..
                }
            ) => {}
            other => self.error(other.span(), "expected a numeric literal"),
        }
    }

    fn keyword(&mut self, expr: &Expr, allowed: &[&str]) -> Option<String> {
        match expr {
            Expr::Path(path) if path.single().is_some() => {
                let word = path.single().unwrap().to_string();
                if allowed.contains(&word.as_str()) {
                    Some(word)
                } else {
                    self.error(
                        path.span(),
                        format!("expected one of: {}", allowed.join(", ")),
                    );
                    None
                }
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

    fn ident_prop(&mut self, expr: &Expr, what: &str) -> Option<Ident> {
        match expr {
            Expr::Path(path) if path.single().is_some() => path.single().cloned(),
            Expr::Literal {
                value: Literal::Text(text),
                span,
            } if what == "id" => {
                if text.is_empty()
                    || !text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    self.error(*span, "ids use letters, digits, `_` and `-`");
                }
                Some(Ident::new("id", *span))
            }
            other => {
                self.error(other.span(), format!("expected an identifier for `{what}`"));
                None
            }
        }
    }

    fn nodes(&mut self, nodes: &[Node], ctx: &Ctx) -> TokenStream {
        let tokens: Vec<_> = nodes.iter().map(|node| self.node(node, ctx)).collect();
        quote!(#(#tokens)*)
    }

    fn node(&mut self, node: &Node, ctx: &Ctx) -> TokenStream {
        match node {
            Node::Element(element) => self.element(element, ctx),
            Node::Component(call) => self.component_call(call),
            Node::If(node) => {
                let child_ctx = Ctx {
                    is_root: false,
                    ..ctx.clone()
                };
                let mut branches = Vec::new();
                for (condition, body) in &node.branches {
                    let condition = self.value(condition);
                    let body = self.nodes(body, &child_ctx);
                    branches.push(quote!(if #condition { #body }));
                }
                let otherwise = node
                    .otherwise
                    .as_ref()
                    .map(|body| self.nodes(body, &child_ctx))
                    .unwrap_or_default();
                quote!(#(#branches else)* { #otherwise })
            }
            Node::IfLet(node) => {
                let child_ctx = Ctx {
                    is_root: false,
                    ..ctx.clone()
                };
                let value = match &node.value {
                    Expr::Path(path) => self.path(path),
                    other => {
                        self.error(other.span(), "`if let` needs an optional binding");
                        quote!(())
                    }
                };
                let binding = &node.binding;
                self.scope.push(binding.to_string());
                let then = self.nodes(&node.then, &child_ctx);
                self.scope.pop();
                let otherwise = node
                    .otherwise
                    .as_ref()
                    .map(|body| self.nodes(body, &child_ctx))
                    .unwrap_or_default();
                quote! {
                    if let ::std::option::Option::Some(#binding) = ::bern::__private::value_of(&#value) {
                        let #binding = ::bern::__private::reref(&#binding);
                        #then
                    } else {
                        #otherwise
                    }
                }
            }
            Node::Match(node) => {
                let child_ctx = Ctx {
                    is_root: false,
                    ..ctx.clone()
                };
                let value = self.value(&node.value);
                let mut arms = Vec::new();
                for arm in &node.arms {
                    match &arm.pattern {
                        Pattern::Wildcard(_) => {
                            let body = self.nodes(&arm.body, &child_ctx);
                            arms.push(quote!(_ => { #body }));
                        }
                        Pattern::Variant {
                            ty,
                            variant,
                            bindings,
                        } => {
                            for binding in bindings {
                                self.scope.push(binding.to_string());
                            }
                            let body = self.nodes(&arm.body, &child_ctx);
                            for _ in bindings {
                                self.scope.pop();
                            }
                            arms.push(quote! {
                                #ty::#variant { #(#bindings,)* .. } => {
                                    #(let #bindings = ::bern::__private::reref(&#bindings);)*
                                    #body
                                }
                            });
                        }
                    }
                }
                quote!(match #value { #(#arms)* })
            }
        }
    }

    fn component_call(&mut self, call: &ComponentCall) -> TokenStream {
        let path = &call.path;
        let mut props_path: Vec<Ident> = call.path.clone();
        let last = props_path.pop().unwrap();
        props_path.push(format_ident!("{}_props", last));
        let names: Vec<_> = call.args.iter().map(|arg| &arg.name).collect();
        let values: Vec<_> = call
            .args
            .iter()
            .map(|arg| match &arg.value {
                Expr::Path(path_expr) => {
                    let tokens = self.path(path_expr);
                    quote_spanned!(path_expr.span()=> ::bern::__private::to_ref(&#tokens))
                }
                other => {
                    let value = self.value(other);
                    let span = other.span();
                    quote_spanned!(span=> ::bern::__private::reref(&#value))
                }
            })
            .collect();
        let span = call.span;
        quote_spanned! {span=>
            let _ = #(#path)::*(#(#props_path)::* { #(#names: #values),* });
        }
    }

    fn element(&mut self, element: &Element, ctx: &Ctx) -> TokenStream {
        let name = element.name.to_string();
        let prop_names: Vec<String> = element.props.iter().map(|p| p.name.to_string()).collect();
        let child_names: Option<Vec<Option<String>>> = element.children.as_ref().map(|children| {
            bern_syntax::child_names(children, spec::DYNAMIC_ITEMS.contains(&name.as_str()))
        });
        let shape = ElementShape {
            name: &name,
            props: prop_names.iter().map(String::as_str).collect(),
            has_template: element.template.is_some(),
            children: child_names.as_ref().map(|children| {
                children
                    .iter()
                    .map(|child| match child {
                        Some(name) => ChildShape::Element(name),
                        None => ChildShape::Other,
                    })
                    .collect()
            }),
            has_action: element.action.is_some(),
            parent: ctx.parent.as_deref(),
            is_root: ctx.is_root,
        };
        for error in spec::check_shape(&shape) {
            let span = error
                .prop
                .as_ref()
                .and_then(|prop| element.prop(prop))
                .map(|prop| prop.name.span())
                .unwrap_or(element.span);
            self.error(span, error.message);
        }
        let Some(component) = spec::component(&name) else {
            return TokenStream::new();
        };
        let mut pre = Vec::new();
        let mut post = Vec::new();
        let mut child_ctx = Ctx {
            // A repetition places its template where it stands itself.
            parent: if name == "ForEach" {
                ctx.parent.clone()
            } else {
                Some(name.clone())
            },
            is_root: false,
            selection: ctx.selection.clone(),
            route: ctx.route.clone(),
        };
        let mut pushed = 0;
        let mut query = None;

        // Bindings that children depend on come first.
        for prop in &element.props {
            let Some(prop_spec) = component.prop(&prop.name.to_string()) else {
                continue;
            };
            // Named corner radii are resolved against style! by the compiler.
            // They are static tokens, not Rust bindings in the macro check.
            if prop.name == "corner"
                && matches!(prop_spec.kind, PropKind::Number | PropKind::Length)
                && matches!(&prop.value, Expr::Path(path) if path.single().is_some())
            {
                continue;
            }
            match prop_spec.kind {
                PropKind::Query => match &prop.value {
                    Expr::Query(call) => {
                        let tokens = self.call(call, "queries");
                        let span = call.method.span();
                        // Spanned at the call so type errors point at it.
                        let mut var = self.fresh("query");
                        var.set_span(span);
                        pre.push(quote_spanned!(span=> let #var = #tokens;));
                        if let Some(template) = &element.template {
                            pre.push(quote_spanned!(span=> let #template = ::bern::__private::query_result(&#var);));
                            self.scope.push(template.to_string());
                            pushed += 1;
                        }
                        query = Some(var);
                    }
                    other => self.error(
                        other.span(),
                        "expected a query call such as `app.search(draft.text)`",
                    ),
                },
                PropKind::SelectionField => {
                    if let Expr::Path(path) = &prop.value {
                        let tokens = self.path(path);
                        let var = self.fresh("selection");
                        pre.push(quote_spanned!(path.span()=> let #var = ::bern::__private::selection_field(&#tokens);));
                        child_ctx.selection = Some(var);
                    } else {
                        self.error(prop.value.span(), "expected a session field");
                    }
                }
                PropKind::PathField => {
                    if let Expr::Path(path) = &prop.value {
                        let tokens = self.path(path);
                        let var = self.fresh("route");
                        pre.push(quote_spanned!(path.span()=> let #var = ::bern::__private::path_field(&#tokens);));
                        child_ctx.route = Some(var);
                    } else {
                        self.error(prop.value.span(), "expected a session field");
                    }
                }
                PropKind::Items => {
                    if let (Expr::Path(path), Some(template)) = (&prop.value, &element.template) {
                        let tokens = self.path(path);
                        pre.push(quote_spanned!(path.span()=> let #template = ::bern::__private::list_item(&#tokens);));
                        self.scope.push(template.to_string());
                        pushed += 1;
                    } else if !matches!(prop.value, Expr::Path(_)) {
                        self.error(prop.value.span(), "expected a list binding");
                    }
                }
                _ => {}
            }
        }
        if name == "Destination"
            && let Some(template) = &element.template
        {
            match &ctx.route {
                Some(route) => {
                    pre.push(quote!(let #template = ::bern::__private::reref(&#route);));
                    self.scope.push(template.to_string());
                    pushed += 1;
                }
                None => self.error(element.span, "`Destination` must be inside `Navigation`"),
            }
        }

        let mut window_ids = Vec::new();
        for prop in &element.props {
            let prop_name = prop.name.to_string();
            let Some(prop_spec) = component.prop(&prop_name) else {
                continue;
            };
            // Named corner radii are resolved against style! by the compiler.
            // They are static tokens, not Rust bindings in the macro check.
            if prop.name == "corner"
                && matches!(prop_spec.kind, PropKind::Number | PropKind::Length)
                && matches!(&prop.value, Expr::Path(path) if path.single().is_some())
            {
                continue;
            }
            match prop_spec.kind {
                PropKind::Id => {
                    if let Some(id) = self.ident_prop(&prop.value, "id") {
                        let id = match &prop.value {
                            Expr::Literal {
                                value: Literal::Text(text),
                                ..
                            } => text.clone(),
                            _ => id.to_string(),
                        };
                        if !self.ids.insert(id.clone()) {
                            self.error(
                                prop.value.span(),
                                format!("control id `{id}` is used twice in this component"),
                            );
                        }
                    }
                }
                PropKind::Keyword(allowed) => {
                    self.keyword(&prop.value, allowed);
                }
                PropKind::Flag => {
                    if !matches!(
                        &prop.value,
                        Expr::Literal {
                            value: Literal::Bool(_),
                            ..
                        }
                    ) {
                        self.error(prop.value.span(), "expected `true` or `false`");
                    }
                }
                PropKind::DisplayText => pre.push(self.display(&prop.value, false)),
                PropKind::LocalizedText => pre.push(self.display(&prop.value, true)),
                PropKind::Shortcut => match &prop.value {
                    Expr::Literal {
                        value: Literal::Text(text),
                        span,
                    } => {
                        if let Err(message) = bern_ir::spec::parse_shortcut(text) {
                            self.error(*span, format!("shortcut \"{text}\": {message}"));
                        }
                    }
                    other => self.error(other.span(), "a shortcut is a literal such as \"cmd+k\""),
                },
                PropKind::Icon => match &prop.value {
                    Expr::Literal {
                        value: Literal::Text(icon),
                        span,
                    } => {
                        if icon.is_empty()
                            || !icon
                                .chars()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
                        {
                            self.error(
                                *span,
                                "icon names use the Bern icon set (SF Symbols naming), e.g. \"gearshape\"",
                            );
                        }
                    }
                    other => self.error(other.span(), "expected an icon name literal"),
                },
                PropKind::SvgResource => match &prop.value {
                    Expr::Literal { value: Literal::Text(name), span } => {
                        if let Err(message) = bern_protocol::check_resource_name(name) {
                            self.error(*span, message);
                        } else if !name.to_ascii_lowercase().ends_with(".svg") {
                            self.error(*span, "Svg requires a .svg resource");
                        }
                    }
                    other => self.error(other.span(), "expected a declared .svg resource"),
                },
                PropKind::ImageResource => match &prop.value {
                    Expr::Literal {
                        value: Literal::Text(name),
                        span,
                    } => {
                        if let Err(message) = bern_protocol::check_resource_name(name) {
                            self.error(*span, message);
                        } else if !name.rsplit_once('.').is_some_and(|(_, ext)| {
                            spec::IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
                        }) {
                            self.error(
                                *span,
                                format!(
                                    "`{name}` is not a PNG or JPEG image (.png, .jpg or .jpeg)"
                                ),
                            );
                        }
                    }
                    Expr::Path(path) => {
                        let tokens = self.path(path);
                        pre.push(quote_spanned!(path.span()=> ::bern::__private::media_source(&#tokens);));
                    }
                    other => self.error(
                        other.span(),
                        "expected the name of a declared resource, e.g. \"images/cover.png\", or a `MediaSource` binding",
                    ),
                },
                PropKind::Number => self.literal_number(&prop.value),
                // A length is a literal or a numeric expression.
                PropKind::Length => match &prop.value {
                    Expr::Literal { .. } | Expr::Unary { .. } => self.literal_number(&prop.value),
                    other => {
                        let value = self.value(other);
                        let span = other.span();
                        pre.push(quote_spanned!(span=> ::bern::__private::number_value(&#value);));
                    }
                },
                PropKind::NumberValue => {
                    let value = self.value(&prop.value);
                    let span = prop.value.span();
                    pre.push(quote_spanned!(span=> ::bern::__private::number_value(&#value);));
                }
                PropKind::Bool => {
                    let value = self.value(&prop.value);
                    let span = prop.value.span();
                    pre.push(quote_spanned!(span=> let _: bool = #value;));
                }
                PropKind::TextField => {
                    pre.push(self.writable(&prop.value, quote!(::bern::__private::text_field)))
                }
                PropKind::BoolField => {
                    pre.push(self.writable(&prop.value, quote!(::bern::__private::bool_field)))
                }
                PropKind::NumberField => {
                    pre.push(self.writable(&prop.value, quote!(::bern::__private::number_field)))
                }
                PropKind::FileField => {
                    pre.push(self.writable(&prop.value, quote!(::bern::__private::file_field)))
                }
                PropKind::PositionField => pre.push(
                    self.writable(&prop.value, quote!(::bern::__private::position_field)),
                ),
                PropKind::FileKinds => match &prop.value {
                    Expr::Literal {
                        value: Literal::Text(text),
                        span,
                    } => {
                        if let Err(message) = bern_ir::spec::parse_file_kinds(text) {
                            self.error(*span, format!("files \"{text}\": {message}"));
                        }
                    }
                    other => self.error(
                        other.span(),
                        "the kinds of file are a literal such as \"video .srt\"",
                    ),
                },
                PropKind::PickerField => {
                    pre.push(self.writable(&prop.value, quote!(::bern::__private::picker_field)))
                }
                PropKind::SelectionValue => {
                    let value = self.value(&prop.value);
                    // A container's `fallback` is one of its own values.
                    let selection = if prop.name == "fallback" {
                        &child_ctx.selection
                    } else {
                        &ctx.selection
                    };
                    match selection {
                        Some(selection) => {
                            let span = prop.value.span();
                            pre.push(quote_spanned!(span=> ::bern::__private::same_type(&#selection, &#value);))
                        }
                        None => self.error(
                            prop.value.span(),
                            "this value needs an enclosing `Sidebar` or `Tabs` selection",
                        ),
                    }
                }
                PropKind::Route => {
                    let value = self.value(&prop.value);
                    match &ctx.route {
                        Some(route) => {
                            let span = prop.value.span();
                            pre.push(quote_spanned!(span=> ::bern::__private::same_type(&#route, &#value);))
                        }
                        None => {
                            self.error(prop.value.span(), "`NavLink` must be inside a `Navigation`")
                        }
                    }
                }
                PropKind::KeyField => match (&prop.value, &element.template) {
                    (Expr::Path(path), Some(template)) if path.single().is_some() => {
                        let field = path.single().unwrap();
                        pre.push(quote_spanned!(field.span()=> ::bern::__private::key(&#template.#field);));
                    }
                    _ => self.error(
                        prop.value.span(),
                        "expected the name of a row field, e.g. `key: id`",
                    ),
                },
                PropKind::WindowId => {
                    if let Some(id) = self.ident_prop(&prop.value, &prop_name) {
                        window_ids.push(id);
                    }
                }
                PropKind::ColorRole => {}
                PropKind::NumberList => {
                    let value = self.value(&prop.value);
                    let span = prop.value.span();
                    pre.push(quote_spanned!(span=> let _ = &#value;));
                }
                PropKind::KeyValue => {
                    let value = self.value(&prop.value);
                    let span = prop.value.span();
                    pre.push(quote_spanned!(span=> let _ = &#value;));
                }
                PropKind::TypeName
                | PropKind::SlotName
                | PropKind::StyleVariant
                | PropKind::Transition
                | PropKind::MotionToken
                | PropKind::HeroName
                | PropKind::PatternName => {
                    self.ident_prop(&prop.value, &prop_name);
                }
                PropKind::MediaSource => match &prop.value {
                    Expr::Path(path) => {
                        let tokens = self.path(path);
                        pre.push(quote_spanned!(path.span()=> ::bern::__private::media_source(&#tokens);));
                    }
                    other => self.error(
                        other.span(),
                        "expected a `MediaSource` binding such as `app.clip`",
                    ),
                },
                PropKind::Player => match &prop.value {
                    Expr::Path(path) if path.segments.len() == 2 => {
                        let tokens = self.path(path);
                        pre.push(
                            quote_spanned!(path.span()=> ::bern::__private::player(&#tokens);),
                        );
                    }
                    other => self.error(
                        other.span(),
                        "expected an application player such as `players.preview`",
                    ),
                },
                PropKind::Feature => match &prop.value {
                    Expr::Variant(variant) if variant.fields.is_empty() && !variant.has_braces => {
                        let ty = &variant.ty;
                        let feature = &variant.variant;
                        pre.push(quote_spanned!(feature.span()=> ::bern::__private::feature(&#ty::#feature);));
                    }
                    other => self.error(
                        other.span(),
                        "expected a feature of the component's contract, e.g. `MediaFeature::Clip`",
                    ),
                },
                PropKind::SelectionField
                | PropKind::PathField
                | PropKind::Items
                | PropKind::Query => {}
            }
        }
        // Only results that can be empty take an `empty:` state.
        if let (Some(var), Some(empty)) = (&query, element.prop("empty")) {
            let span = empty.name.span();
            let var = Ident::new(&var.to_string(), span);
            pre.push(quote_spanned!(span=> ::bern::__private::query_empty(&#var);));
        }

        if name == "WindowSet" {
            self.window_set(element);
        }

        let children = match (&element.children, name.as_str()) {
            // The layouts of one `Adaptive` are alternatives: the same
            // control id in two of them names the same control.
            (Some(children), "Adaptive") => {
                let before = self.ids.clone();
                let mut all = before.clone();
                let mut tokens = TokenStream::new();
                for child in children {
                    self.ids = before.clone();
                    tokens.extend(self.node(child, &child_ctx));
                    all.extend(self.ids.drain());
                }
                self.ids = all;
                tokens
            }
            (Some(children), _) => self.nodes(children, &child_ctx),
            (None, _) => TokenStream::new(),
        };
        for _ in 0..pushed {
            self.scope.pop();
        }
        if let Some(action) = &element.action {
            post.push(self.action(action));
        }
        quote!({ #(#pre)* #children #(#post)* })
    }

    fn action(&mut self, action: &Action) -> TokenStream {
        // `pages.new.show()` names a page of a slot, which the compiler
        // resolves against what the modules contribute; there is no value
        // here to call.
        if action.receiver[0] == "pages" && !self.in_scope("pages") {
            if action.receiver.len() != 2 || action.method != "show" || !action.args.is_empty() {
                self.error(
                    action.method.span(),
                    "page actions look like `pages.new.show()`",
                );
            }
            return TokenStream::new();
        }
        let call = self.call(action, "actions");
        let span = action.method.span();
        quote_spanned!(span=> ::bern::__private::command(#call);)
    }

    /// A typed client call; `rustc` checks the method and its arguments.
    fn call(&mut self, call: &Action, what: &str) -> TokenStream {
        let root = &call.receiver[0];
        if !self.in_scope(&root.to_string()) {
            self.error(
                root.span(),
                format!("unknown client `{root}`; {what} call a typed client parameter"),
            );
        }
        let receiver = &call.receiver;
        let method = &call.method;
        let args: Vec<_> = call.args.iter().map(|arg| self.value(arg)).collect();
        let span = call.method.span();
        quote_spanned!(span=> #(#receiver).*.#method(#(#args),*))
    }

    fn window_set(&mut self, element: &Element) {
        let Some(handle) = element.prop("handle").and_then(|prop| match &prop.value {
            Expr::Path(path) => path.single().cloned(),
            _ => None,
        }) else {
            return;
        };
        let mut windows: Vec<Ident> = Vec::new();
        for child in element.children.iter().flatten() {
            if let Node::Element(window) = child
                && let Some(Expr::Path(path)) = window.prop("id").map(|prop| &prop.value)
                && let Some(id) = path.single()
            {
                if windows.iter().any(|existing| existing == id) {
                    self.error(id.span(), format!("window `{id}` is declared twice"));
                }
                windows.push(id.clone());
            }
        }
        if let Some(Expr::Path(path)) = element.prop("primary").map(|prop| &prop.value)
            && let Some(primary) = path.single()
            && !windows.iter().any(|window| window == primary)
        {
            self.error(
                primary.span(),
                format!("primary window `{primary}` is not declared in this WindowSet"),
            );
        }
        let doc = "Typed window catalog generated from the root `WindowSet`.";
        self.items.push(quote! {
            #[doc = #doc]
            #[derive(Clone, Copy)]
            pub struct #handle {
                #(pub #windows: ::bern::bind::WindowRef,)*
            }
            impl ::bern::bind::Prop for #handle {
                type Ref = #handle;
            }
            impl ::bern::bind::ToRef for #handle {
                type Ref = #handle;
                fn to_ref(&self) -> #handle {
                    *self
                }
            }
        });
    }
}

pub fn expand(args: TokenStream, tokens: TokenStream) -> syn::Result<TokenStream> {
    let mut func: syn::ItemFn = syn::parse2(tokens)?;
    let modules = crate::config::build_modules()?;
    modules.function(&mut func)?;
    let mut stripped = func.clone();
    stripped
        .attrs
        .retain(|attr| !is_bern_attr(attr, "component"));
    let features = bern_syntax::parse_component_args(args)?;
    let mut component = bern_syntax::parse_component_with(stripped.clone(), features)?;
    modules.component(&mut component)?;
    let languages =
        crate::config::languages().map_err(|message| syn::Error::new(component.span, message))?;
    let strict = languages.as_ref().is_none_or(|languages| languages.strict);
    let mut generator = Gen {
        languages: languages.as_ref(),
        strict,
        errors: Vec::new(),
        counter: 0,
        scope: component
            .params
            .iter()
            .map(|param| param.name.to_string())
            .collect(),
        ids: HashSet::new(),
        items: Vec::new(),
    };
    if component.body.nodes.len() != 1 {
        generator.error(
            component.span,
            "a component body has exactly one root node; wrap siblings in `Column { … }`",
        );
    }
    let checks = generator.nodes(
        &component.body.nodes,
        &Ctx {
            is_root: true,
            ..Ctx::default()
        },
    );

    let name = &component.name;
    let vis = &stripped.vis;
    let attrs = &stripped.attrs;
    let props_name = format_ident!("{}_props", name);
    let prop_params: Vec<_> = component
        .params
        .iter()
        .filter(|param| param.kind == ParamKind::Prop)
        .collect();
    let prop_names: Vec<_> = prop_params.iter().map(|param| &param.name).collect();
    let prop_types: Vec<_> = prop_params.iter().map(|param| &param.syn_ty).collect();
    let session_params: Vec<_> = component
        .params
        .iter()
        .filter(|param| param.kind == ParamKind::Session)
        .collect();
    let session_names: Vec<_> = session_params.iter().map(|param| &param.name).collect();
    let session_types: Vec<_> = session_params.iter().map(|param| &param.syn_ty).collect();
    let all_names: Vec<_> = component.params.iter().map(|param| &param.name).collect();
    let items = &generator.items;
    let errors = combine(std::mem::take(&mut generator.errors))
        .err()
        .map(syn::Error::into_compile_error);
    let checks = if errors.is_some() {
        TokenStream::new()
    } else {
        checks
    };
    let track = track(languages.as_ref());
    let track_build = crate::config::track_build();
    let props_doc = format!("Properties of the `{name}` component.");
    let contract = component
        .features
        .as_ref()
        .map(|path| quote!(::bern::__private::features::<#path>();));
    Ok(quote! {
        #errors
        #track_build

        #[doc = #props_doc]
        #[allow(non_camel_case_types)]
        #vis struct #props_name {
            #(pub #prop_names: <#prop_types as ::bern::bind::Prop>::Ref,)*
        }

        #(#attrs)*
        #vis fn #name(__props: #props_name) -> impl ::bern::View {
            let #props_name { #(#prop_names),* } = __props;
            #(let #session_names = <#session_types as ::bern::bind::SessionState>::handle();)*
            #[allow(
                unused_variables,
                unused_mut,
                unused_must_use,
                unreachable_code,
                clippy::all
            )]
            if false {
                #checks
            }
            #(let _ = &#all_names;)*
            #contract
            ::bern::__private::Declared
        }

        #(#items)*
        #track
    })
}
