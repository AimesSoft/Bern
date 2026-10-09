use bern_protocol::{Type, TypeTable};
use bern_syntax::TypeRef;
use proc_macro2::{Span, TokenStream};
use quote::quote;

/// Resolve a syntax type against a self-contained type table.
pub fn resolve(ty: &TypeRef, types: &TypeTable, span: Span) -> syn::Result<Type> {
    Ok(match ty {
        TypeRef::Unit => Type::Unit,
        TypeRef::Bool => Type::Bool,
        TypeRef::Int => Type::Int,
        TypeRef::Float => Type::Float,
        TypeRef::Text => Type::Text,
        TypeRef::Optional(inner) => Type::optional(resolve(inner, types, span)?),
        TypeRef::List(inner) => Type::list(resolve(inner, types, span)?),
        TypeRef::Named(path) => {
            let name = path.last().unwrap();
            if path.len() > 1 {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "refer to `{name}` by its name; protocol types are declared in the protocol module"
                    ),
                ));
            }
            if types.enum_def(name).is_some() {
                Type::Enum { name: name.clone() }
            } else if types.record_def(name).is_some() {
                Type::Record { name: name.clone() }
            } else {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "type `{name}` is not declared in this protocol; protocols are \
                         self-contained, so declare it here with `#[value]`"
                    ),
                ));
            }
        }
    })
}

pub fn type_tokens(ty: &Type) -> TokenStream {
    let p = quote!(::bern::__private::protocol);
    match ty {
        Type::Unit => quote!(#p::Type::Unit),
        Type::Bool => quote!(#p::Type::Bool),
        Type::Int => quote!(#p::Type::Int),
        Type::Float => quote!(#p::Type::Float),
        Type::Text => quote!(#p::Type::Text),
        Type::Enum { name } => quote!(#p::Type::Enum { name: ::std::string::String::from(#name) }),
        Type::Record { name } => {
            quote!(#p::Type::Record { name: ::std::string::String::from(#name) })
        }
        Type::Optional { inner } => {
            let inner = type_tokens(inner);
            quote!(#p::Type::optional(#inner))
        }
        Type::List { inner } => {
            let inner = type_tokens(inner);
            quote!(#p::Type::list(#inner))
        }
    }
}
