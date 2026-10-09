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
mod ui;

pub use ast::*;
pub use items::{
    CommandItem, CommandParam, CommandsItem, ComponentItem, FieldItem, Param, ParamKind,
    ProtocolArgs, ProtocolItem, SessionItem, SourceItems, StateItem, ValueItem, ValueKind,
    VariantItem, doc_comment, is_bern_attr, parse_component, parse_protocol, parse_protocol_args,
    parse_session, parse_source_file, parse_value, type_ref,
};

/// Parse the body of a `ui!` invocation.
pub fn parse_ui(tokens: proc_macro2::TokenStream) -> syn::Result<UiBody> {
    syn::parse2(tokens)
}

/// Parse a standalone `tr!` invocation body.
pub fn parse_tr(tokens: proc_macro2::TokenStream) -> syn::Result<Tr> {
    syn::parse2::<expr::TrBody>(tokens).map(|body| body.0)
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
