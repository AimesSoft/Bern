//! Expansion of `#[bern::players]` structs.

use bern_syntax::{is_bern_attr, parse_players};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

pub fn expand(tokens: TokenStream) -> syn::Result<TokenStream> {
    let item: Item = syn::parse2(tokens)?;
    let players = parse_players(&item)?;
    let Item::Struct(item_struct) = item else {
        unreachable!()
    };
    let name = &players.name;
    let vis = &item_struct.vis;
    let attrs: Vec<_> = item_struct
        .attrs
        .iter()
        .filter(|attr| !is_bern_attr(attr, "players"))
        .collect();
    let fields = players.players.iter().map(|player| {
        let field = &player.name;
        let doc = &player.doc;
        quote! {
            #[doc = #doc]
            pub #field: ::bern::bind::PlayerRef
        }
    });
    let b = quote!(::bern::bind);
    Ok(quote! {
        #(#attrs)*
        #[derive(Clone, Copy)]
        #vis struct #name {
            #(#fields,)*
        }

        impl #b::Prop for #name {
            type Ref = #name;
        }

        impl #b::ToRef for #name {
            type Ref = #name;
            fn to_ref(&self) -> #name {
                *self
            }
        }
    })
}
