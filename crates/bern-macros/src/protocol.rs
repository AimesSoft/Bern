//! Expansion of `#[bern::protocol]` modules.

use crate::descriptor;
use crate::values;
use bern_syntax::{is_bern_attr, parse_protocol, parse_protocol_args};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Item, ItemMod};

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let module: ItemMod = syn::parse2(item)?;
    let span = module.ident.span();
    let args = parse_protocol_args(args, Span::call_site())?;
    let protocol = parse_protocol(args, module.clone())?;
    let languages = crate::config::languages().map_err(|message| syn::Error::new(span, message))?;
    for value in &protocol.values {
        if let bern_syntax::ValueKind::Enum(variants) = &value.kind {
            for variant in variants {
                if let Some(label) = &variant.label {
                    values::check_label(label, languages.as_ref())?;
                }
            }
        }
    }
    let descriptor_value = descriptor::build(&protocol)?;
    let descriptor_tokens = descriptor::tokens(&descriptor_value);

    let p = quote!(::bern::__private::protocol);
    let b = quote!(::bern::bind);

    // Keep the user's items, minus Bern helper attributes.
    let (_, items) = module.content.as_ref().unwrap();
    let mut kept = Vec::new();
    for item in items {
        match item {
            Item::Enum(syn::ItemEnum { attrs, .. })
            | Item::Struct(syn::ItemStruct { attrs, .. })
                if attrs.iter().any(|attr| is_bern_attr(attr, "value")) =>
            {
                let value = bern_syntax::parse_value(&{
                    let mut copy = item.clone();
                    if let Item::Enum(e) = &mut copy {
                        e.attrs.retain(|attr| !is_bern_attr(attr, "value"));
                    }
                    if let Item::Struct(s) = &mut copy {
                        s.attrs.retain(|attr| !is_bern_attr(attr, "value"));
                    }
                    copy
                })?;
                let cleaned = values::clean_item(item);
                let impls = values::impls(&value, &values::visibility(item));
                kept.push(quote!(#cleaned #impls));
            }
            Item::Struct(item_struct)
                if item_struct
                    .attrs
                    .iter()
                    .any(|attr| is_bern_attr(attr, "state")) =>
            {
                let mut cleaned = item_struct.clone();
                cleaned.attrs.retain(|attr| !is_bern_attr(attr, "state"));
                kept.push(quote! {
                    #[derive(Clone, Debug, PartialEq)]
                    #cleaned
                });
            }
            Item::Trait(item_trait)
                if item_trait
                    .attrs
                    .iter()
                    .any(|attr| is_bern_attr(attr, "commands")) =>
            {
                let mut cleaned = item_trait.clone();
                cleaned.attrs.retain(|attr| !is_bern_attr(attr, "commands"));
                for trait_item in &mut cleaned.items {
                    if let syn::TraitItem::Fn(method) = trait_item {
                        method.attrs.retain(|attr| {
                            !is_bern_attr(attr, "errors") && !is_bern_attr(attr, "idempotent")
                        });
                    }
                }
                kept.push(quote!(#cleaned));
            }
            other => kept.push(quote!(#other)),
        }
    }

    let state_name = &protocol.state.name;
    let commands_name = &protocol.commands.name;
    let state_fields: Vec<_> = protocol.state.fields.iter().map(|f| &f.name).collect();
    let state_types: Vec<_> = protocol.state.fields.iter().map(|f| &f.syn_ty).collect();

    let invoke_arms = protocol.commands.methods.iter().enumerate().map(|(index, method)| {
        let name = &method.name;
        let params: Vec<_> = (0..method.params.len())
            .map(|position| format_ident!("p{}", position))
            .collect();
        let types: Vec<_> = method.params.iter().map(|param| &param.syn_ty).collect();
        let positions: Vec<_> = (0..method.params.len()).collect();
        quote! {
            #index => {
                #(let #params: #types = #b::BernValue::from_value(&invocation.params[#positions])?;)*
                let result = #commands_name::#name(&mut self.0, #(#params),*)?;
                ::std::result::Result::Ok(#b::BernValue::to_value(&result))
            }
        }
    });

    let client_methods = protocol.commands.methods.iter().map(|method| {
        let name = &method.name;
        let params: Vec<_> = method.params.iter().map(|param| &param.name).collect();
        let types: Vec<_> = method.params.iter().map(|param| &param.syn_ty).collect();
        let doc = if method.doc.is_empty() {
            format!("Reference to the `{}` command.", method.name)
        } else {
            method.doc.clone()
        };
        quote! {
            #[doc = #doc]
            #[allow(unused_variables)]
            pub fn #name(&self, #(#params: #types),*) -> #b::CommandRef {
                #b::CommandRef::declared()
            }
        }
    });

    let vis = &module.vis;
    let ident = &module.ident;
    let attrs: Vec<_> = module
        .attrs
        .iter()
        .filter(|attr| !is_bern_attr(attr, "protocol"))
        .collect();
    let track = crate::component::track(languages.as_ref());
    Ok(quote! {
        #(#attrs)*
        #vis mod #ident {
            #(#kept)*

            /// Implemented by the application engine next to the command trait.
            pub trait Engine: #commands_name + ::std::marker::Send + 'static {
                /// The current read-only state snapshot.
                fn state(&self) -> &#state_name;
            }

            /// The protocol descriptor: types, state and command catalog.
            pub fn descriptor() -> &'static #p::ProtocolDescriptor {
                static DESCRIPTOR: ::std::sync::OnceLock<#p::ProtocolDescriptor> =
                    ::std::sync::OnceLock::new();
                DESCRIPTOR.get_or_init(|| #descriptor_tokens)
            }

            /// Connects an engine to the generic command host.
            pub struct Adapter<E>(pub E);

            impl<E: Engine> ::bern::__private::command::EngineProtocol for Adapter<E> {
                fn descriptor(&self) -> &'static #p::ProtocolDescriptor {
                    descriptor()
                }

                fn invoke(
                    &mut self,
                    invocation: &#p::Invocation,
                ) -> ::std::result::Result<#p::Value, #p::CommandError> {
                    match invocation.command {
                        #(#invoke_arms)*
                        _ => ::std::result::Result::Err(#p::CommandError::new(
                            "unknown_command",
                            "unknown command index",
                        )),
                    }
                }

                fn snapshot(&self) -> ::std::vec::Vec<#p::Value> {
                    let state = Engine::state(&self.0);
                    ::std::vec![#(#b::BernValue::to_value(&state.#state_fields)),*]
                }
            }

            /// Typed client used by UI declarations: read-only state bindings
            /// and command references.
            #[derive(Clone)]
            pub struct Client {
                #(pub #state_fields: <#state_types as #b::BernValue>::Ref,)*
            }

            impl Client {
                #(#client_methods)*
            }

            impl #b::Prop for Client {
                type Ref = Client;
            }

            impl #b::ToRef for Client {
                type Ref = Client;
                fn to_ref(&self) -> Client {
                    self.clone()
                }
            }

            #track
        }
    })
}
