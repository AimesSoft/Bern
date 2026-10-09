//! Expansion of `#[bern::session]` structs.

use bern_syntax::{Expr, Literal, is_bern_attr, parse_session};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Item;

fn initial_tokens(expr: &Expr) -> syn::Result<TokenStream> {
    Ok(match expr {
        Expr::Literal { value, .. } => match value {
            Literal::Text(text) => quote!(::std::string::String::from(#text)),
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
        Expr::Unary {
            op: bern_syntax::UnaryOp::Neg,
            expr,
            ..
        } => {
            let inner = initial_tokens(expr)?;
            quote!(-#inner)
        }
        Expr::Variant(variant) if variant.fields.is_empty() => {
            let ty = &variant.ty;
            let name = &variant.variant;
            quote!(#ty::#name)
        }
        other => {
            return Err(syn::Error::new(
                other.span(),
                "an initial value is a literal or a unit enum variant",
            ));
        }
    })
}

pub fn expand(tokens: TokenStream) -> syn::Result<TokenStream> {
    let item: Item = syn::parse2(tokens)?;
    let session = parse_session(&item)?;
    let Item::Struct(mut item_struct) = item else {
        unreachable!()
    };
    item_struct
        .attrs
        .retain(|attr| !is_bern_attr(attr, "session"));
    for field in &mut item_struct.fields {
        field.attrs.retain(|attr| !is_bern_attr(attr, "initial"));
    }
    let name = &session.name;
    let vis = &item_struct.vis;
    let handle = format_ident!("{}Handle", name);
    let fields: Vec<_> = session.fields.iter().map(|field| &field.name).collect();
    let types: Vec<_> = session.fields.iter().map(|field| &field.syn_ty).collect();
    let mut initial_checks = Vec::new();
    for field in &session.fields {
        if let Some(expr) = &field.initial {
            let ty = &field.syn_ty;
            let value = initial_tokens(expr)?;
            initial_checks.push(quote!(let _: #ty = #value;));
        }
    }
    let check_name = format_ident!("__bern_initial_values_of_{}", name);
    let doc = format!("Writable session fields of `{name}`.");
    let b = quote!(::bern::bind);
    Ok(quote! {
        #[derive(Clone, Debug, PartialEq)]
        #item_struct

        #[doc = #doc]
        #[derive(Clone, Copy)]
        #vis struct #handle {
            #(pub #fields: #b::SessionField<#types>,)*
        }

        impl #b::SessionState for #name {
            type Handle = #handle;
            fn handle() -> #handle {
                #handle { #(#fields: #b::SessionField::declared(),)* }
            }
        }

        #[allow(dead_code, non_snake_case)]
        fn #check_name() {
            #(::bern::__private::assert_value::<#types>();)*
            #(#initial_checks)*
        }
    })
}
