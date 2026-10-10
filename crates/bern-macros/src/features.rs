//! Expansion of `#[bern::features]` enums.

use bern_syntax::{is_bern_attr, parse_features};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

/// Attributes a feature variant carries for the build tool.
const FEATURE_ATTRS: &[&str] = &["commands", "layouts", "guaranteed", "exempt"];

pub fn expand(tokens: TokenStream) -> syn::Result<TokenStream> {
    let item: Item = syn::parse2(tokens)?;
    let features = parse_features(&item)?;
    let Item::Enum(mut item_enum) = item else {
        unreachable!()
    };
    item_enum
        .attrs
        .retain(|attr| !is_bern_attr(attr, "features"));
    for variant in &mut item_enum.variants {
        variant
            .attrs
            .retain(|attr| !FEATURE_ATTRS.iter().any(|name| is_bern_attr(attr, name)));
    }
    let name = &features.name;
    Ok(quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #item_enum

        impl ::bern::Features for #name {}
    })
}
