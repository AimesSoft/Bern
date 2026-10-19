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
    let optional =
        crate::config::optional_modules().map_err(|message| syn::Error::new(span, message))?;
    for value in &protocol.values {
        if let bern_syntax::ValueKind::Enum(variants) = &value.kind {
            for variant in variants {
                if let Some(label) = &variant.label {
                    values::check_label(label, languages.as_ref())?;
                }
            }
        }
    }
    let descriptor_value = bern_syntax::lower::protocol_descriptor(&protocol)?;
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
                values::strip_content(&mut cleaned.fields);
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
                            !is_bern_attr(attr, "errors")
                                && !is_bern_attr(attr, "idempotent")
                                && !is_bern_attr(attr, "on")
                        });
                    }
                }
                kept.push(quote!(#cleaned));
            }
            // Events are emitted through the generated `emit` functions;
            // the trait itself only declares them.
            Item::Trait(item_trait)
                if item_trait
                    .attrs
                    .iter()
                    .any(|attr| is_bern_attr(attr, "events")) => {}
            Item::Trait(item_trait)
                if item_trait
                    .attrs
                    .iter()
                    .any(|attr| is_bern_attr(attr, "queries")) =>
            {
                let mut cleaned = item_trait.clone();
                cleaned.attrs.retain(|attr| !is_bern_attr(attr, "queries"));
                for trait_item in &mut cleaned.items {
                    if let syn::TraitItem::Fn(method) = trait_item {
                        method.attrs.retain(|attr| {
                            !is_bern_attr(attr, "errors") && !is_bern_attr(attr, "reads")
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

    let queries: Vec<&bern_syntax::QueryItem> = protocol
        .queries
        .iter()
        .flat_map(|queries| &queries.methods)
        .collect();
    let queries_name = protocol.queries.as_ref().map(|queries| &queries.name);
    let engine_bound = match queries_name {
        Some(queries_name) => quote!(#commands_name + #queries_name),
        None => quote!(#commands_name),
    };
    let first_query = protocol.commands.methods.len();
    let query_arms = queries.iter().enumerate().map(|(offset, method)| {
        let index = first_query + offset;
        let name = &method.name;
        let queries_name = queries_name.expect("queries trait");
        let params: Vec<_> = (0..method.params.len())
            .map(|position| format_ident!("p{}", position))
            .collect();
        let types: Vec<_> = method.params.iter().map(|param| &param.syn_ty).collect();
        let positions: Vec<_> = (0..method.params.len()).collect();
        quote! {
            #index => {
                let decoded = (|| -> ::std::result::Result<_, #p::CommandError> {
                    ::std::result::Result::Ok((#(<#types as #b::BernValue>::from_value(&invocation.params[#positions])?,)*))
                })();
                match decoded {
                    ::std::result::Result::Ok((#(#params,)*)) => #queries_name::#name(
                        &self.0,
                        #(#params,)*
                        ::bern::query::Answer::new(answer),
                    ),
                    ::std::result::Result::Err(error) => answer.send(::std::result::Result::Err(error)),
                }
            }
        }
    });
    let query_fn = if queries.is_empty() {
        TokenStream::new()
    } else {
        quote! {
            fn query(
                &self,
                invocation: &#p::Invocation,
                answer: ::bern::__private::command::QueryAnswer,
            ) {
                match invocation.command {
                    #(#query_arms)*
                    _ => answer.send(::std::result::Result::Err(#p::CommandError::new(
                        "unknown_command",
                        "unknown query index",
                    ))),
                }
            }
        }
    };
    let query_methods = queries.iter().map(|method| {
        let name = &method.name;
        let params: Vec<_> = method.params.iter().map(|param| &param.name).collect();
        let types: Vec<_> = method.params.iter().map(|param| &param.syn_ty).collect();
        let result = &method.result_ty;
        // A `Query` over an optional result shows the value inside it.
        let shown = match (&method.result, option_inner(result)) {
            (bern_syntax::TypeRef::Optional(_), Some(inner)) => quote!(#inner),
            _ => quote!(#result),
        };
        let doc = if method.doc.is_empty() {
            format!("Binding of the `{}` query.", method.name)
        } else {
            method.doc.clone()
        };
        quote! {
            #[doc = #doc]
            #[allow(unused_variables)]
            pub fn #name(&self, #(#params: #types),*) -> #b::QueryRef<#result, #shown> {
                #b::QueryRef::declared()
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

    // What other modules reach this one through: queued commands, events
    // and the current state, all through the application engine's outbox.
    let o = quote!(::bern::__private::command::outbox);
    let namespace = &protocol.args.namespace;
    let calls = protocol.commands.methods.iter().enumerate().map(|(index, method)| {
        let name = &method.name;
        let params: Vec<_> = method.params.iter().map(|param| &param.name).collect();
        let types: Vec<_> = method.params.iter().map(|param| &param.syn_ty).collect();
        let doc = format!(
            "Queue `{namespace}.{name}` to run after the command being handled."
        );
        quote! {
            #[doc = #doc]
            pub fn #name(#(#params: #types),*) {
                #o::call(#namespace, #index, ::std::vec![#(#b::BernValue::to_value(&#params)),*]);
            }
        }
    });
    let events = protocol.events.iter().flat_map(|events| &events.methods).map(|event| {
        let name = &event.name;
        let event_name = name.to_string();
        let params: Vec<_> = event.params.iter().map(|param| &param.name).collect();
        let types: Vec<_> = event.params.iter().map(|param| &param.syn_ty).collect();
        let doc = if event.doc.is_empty() {
            format!("Emit `{namespace}.{name}`.")
        } else {
            event.doc.clone()
        };
        quote! {
            #[doc = #doc]
            pub fn #name(#(#params: #types),*) {
                #o::emit(#namespace, #event_name, ::std::vec![#(#b::BernValue::to_value(&#params)),*]);
            }
        }
    });
    let state_positions: Vec<_> = (0..state_fields.len()).collect();
    let reach = quote! {
        /// Commands of this module for the modules depending on it. Each
        /// queues its command to run after the command being handled.
        pub mod calls {
            #![allow(unused_imports)]
            use super::*;
            use ::bern::prelude::*;
            #(#calls)*
        }

        /// The events this module emits to the modules depending on it,
        /// which answer them after the command being handled.
        pub mod emit {
            #![allow(unused_imports)]
            use super::*;
            use ::bern::prelude::*;
            #(#events)*
        }

        /// This module's current state, for a module depending on it while
        /// it handles a command; `None` elsewhere or while this module is
        /// off.
        pub fn current_state() -> ::std::option::Option<#state_name> {
            let values = #o::state_of(#namespace)?;
            ::std::option::Option::Some(#state_name {
                #(#state_fields: #b::BernValue::from_value(values.get(#state_positions)?).ok()?,)*
            })
        }
    };
    let vis = &module.vis;
    let ident = &module.ident;
    let attrs: Vec<_> = module
        .attrs
        .iter()
        .filter(|attr| !is_bern_attr(attr, "protocol"))
        .collect();
    let track = crate::component::track(languages.as_ref());
    // The application composes a `modules` state field with a switch per
    // optional module; the shell's client carries it for `app.modules.<id>`.
    let module_manifests = optional
        .manifests
        .iter()
        .map(|path| path.display().to_string());
    let track_modules = quote!(#(const _: &[u8] = ::core::include_bytes!(#module_manifests);)*);
    let (modules_field, modules_record) = if optional.ids.is_empty() {
        (TokenStream::new(), TokenStream::new())
    } else {
        let ids = optional
            .ids
            .iter()
            .map(|id| {
                syn::parse_str::<syn::Ident>(id).map_err(|_| {
                    syn::Error::new(span, format!("module id `{id}` is not a Rust name"))
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;
        let docs = optional
            .ids
            .iter()
            .map(|id| format!("Whether the `{id}` module runs."));
        (
            quote!(
                /// Which optional modules run.
                pub modules: Modules,
            ),
            quote! {
                /// The application's optional modules, one switch each.
                #[derive(Clone)]
                pub struct Modules {
                    #(#[doc = #docs] pub #ids: <bool as #b::BernValue>::Ref,)*
                }
            },
        )
    };
    Ok(quote! {
        #(#attrs)*
        #vis mod #ident {
            #(#kept)*

            /// Implemented by the application engine next to the command
            /// and query traits.
            pub trait Engine: #engine_bound + ::std::marker::Send + 'static {
                /// The current read-only state snapshot.
                fn state(&self) -> &#state_name;

                /// For an application with optional modules: the modules it
                /// starts with, as saved from `modules_changed`. `None`
                /// keeps each module's default.
                fn initial_modules(&self) -> ::std::option::Option<::std::vec::Vec<::std::string::String>> {
                    ::std::option::Option::None
                }

                /// For an application with optional modules: the modules
                /// running after a change, for the engine to save.
                fn modules_changed(&mut self, enabled: &[::std::string::String]) {
                    let _ = enabled;
                }
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

                #query_fn

                fn snapshot(&self) -> ::std::vec::Vec<#p::Value> {
                    let state = Engine::state(&self.0);
                    ::std::vec![#(#b::BernValue::to_value(&state.#state_fields)),*]
                }

                fn initial_modules(&self) -> ::std::option::Option<::std::vec::Vec<::std::string::String>> {
                    Engine::initial_modules(&self.0)
                }

                fn modules_changed(&mut self, enabled: &[::std::string::String]) {
                    Engine::modules_changed(&mut self.0, enabled)
                }
            }

            /// Typed client used by UI declarations: read-only state bindings
            /// and command references.
            #[derive(Clone)]
            pub struct Client {
                #(pub #state_fields: <#state_types as #b::BernValue>::Ref,)*
                #modules_field
            }

            #modules_record

            impl Client {
                #(#client_methods)*
                #(#query_methods)*
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

            #reach

            #track
            #track_modules
        }
    })
}

/// `T` when `ty` is written `Option<T>`.
fn option_inner(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    if last.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner)) => Some(inner),
        _ => None,
    }
}
