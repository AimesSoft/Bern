//! The one parser for Bern declarations.
//!
//! Procedural macros use it to expand declarations for `rustc`, and the
//! metadata collector uses it to read the same source files without running
//! application code. Keeping a single parser means both sides always agree
//! on what a declaration means.

pub mod ast;
mod expr;
pub mod items;
pub mod lower;
pub mod style;
mod ui;

pub use ast::*;
pub use items::{
    CommandItem, CommandParam, CommandRef, CommandsItem, ComponentItem, EventItem, EventsItem,
    FeatureItem, FeaturesItem, FieldItem, Param, ParamKind, PlayerField, PlayersItem, ProtocolArgs,
    ProtocolItem, QueriesItem, QueryItem, SessionItem, SourceItems, StateItem, ValueItem,
    ValueKind, VariantItem, doc_comment, is_bern_attr, parse_component, parse_component_args,
    parse_component_with, parse_features, parse_players, parse_protocol, parse_protocol_args,
    parse_session, parse_source_file, parse_value, parse_value_args, type_ref,
};
pub use style::{
    CornerBlock, MotionDecl, StyleArg, StyleItem, StyleProp, StyleRuleItem, StyleValue, parse_style,
};

/// Parse the body of a `ui!` invocation.
pub fn parse_ui(tokens: proc_macro2::TokenStream) -> syn::Result<UiBody> {
    syn::parse2(tokens)
}

/// Parse a standalone `tr!` invocation body.
pub fn parse_tr(tokens: proc_macro2::TokenStream) -> syn::Result<Tr> {
    syn::parse2::<expr::TrBody>(tokens).map(|body| body.0)
}

/// A number literal's value. One too large for `f64` is an error, as in
/// Rust, so the IR holds no infinity a target would have to write.
pub(crate) fn finite(value: f64, span: proc_macro2::Span) -> syn::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(syn::Error::new(
            span,
            "number literal is out of range for f64",
        ))
    }
}

/// Lower snake case identifiers used for ids: `preview`, `main_window`.
pub fn valid_id(name: &str) -> bool {
    bern_protocol::valid_name(name)
}

/// Whether a type mentions the named type anywhere.
pub fn mentions(ty: &TypeRef, name: &str) -> bool {
    match ty {
        TypeRef::Named(path) => path.last().is_some_and(|last| last == name),
        TypeRef::Optional(inner) | TypeRef::List(inner) => mentions(inner, name),
        _ => false,
    }
}

/// Convert `snake_case` to `PascalCase`.
pub fn pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// Convert `snake_case` to `camelCase`.
pub fn camel_case(name: &str) -> String {
    let pascal = pascal_case(name);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests;
