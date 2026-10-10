//! Build a protocol descriptor at expansion time and emit code that
//! reconstructs exactly the same value at run time.

use crate::types::type_tokens;
use bern_protocol::{
    Concurrency, FieldDef, LocalizedText, OperationKind, ProtocolDescriptor, Retry,
};
use proc_macro2::TokenStream;
use quote::quote;

fn string(value: &str) -> TokenStream {
    quote!(::std::string::String::from(#value))
}

fn text_tokens(text: &LocalizedText) -> TokenStream {
    let p = quote!(::bern::__private::protocol);
    let key = string(&text.key);
    let entries = text.translations.iter().map(|(language, value)| {
        let language = string(language);
        let value = string(value);
        quote!((#language, #value))
    });
    quote! {
        #p::LocalizedText {
            key: #key,
            translations: ::std::collections::BTreeMap::from([#(#entries),*]),
            params: ::std::vec::Vec::new(),
        }
    }
}

fn field_tokens(field: &FieldDef) -> TokenStream {
    let p = quote!(::bern::__private::protocol);
    let name = string(&field.name);
    let ty = type_tokens(&field.ty);
    let doc = string(&field.doc);
    quote!(#p::FieldDef { name: #name, ty: #ty, doc: #doc })
}

/// Emit an expression constructing `descriptor`.
pub fn tokens(descriptor: &ProtocolDescriptor) -> TokenStream {
    let p = quote!(::bern::__private::protocol);
    let enums = descriptor.types.enums.iter().map(|def| {
        let name = string(&def.name);
        let variants = def.variants.iter().map(|variant| {
            let name = string(&variant.name);
            let fields = variant.fields.iter().map(field_tokens);
            let label = match &variant.label {
                Some(text) => {
                    let text = text_tokens(text);
                    quote!(::std::option::Option::Some(#text))
                }
                None => quote!(::std::option::Option::None),
            };
            quote!(#p::VariantDef { name: #name, fields: ::std::vec![#(#fields),*], label: #label })
        });
        quote!(#p::EnumDef { name: #name, variants: ::std::vec![#(#variants),*] })
    });
    let records = descriptor.types.records.iter().map(|def| {
        let name = string(&def.name);
        let fields = def.fields.iter().map(field_tokens);
        quote!(#p::RecordDef { name: #name, fields: ::std::vec![#(#fields),*] })
    });
    let state = descriptor.state.iter().map(|field| {
        let name = string(&field.name);
        let ty = type_tokens(&field.ty);
        let doc = string(&field.doc);
        quote!(#p::StateField { name: #name, ty: #ty, doc: #doc })
    });
    let commands = descriptor.commands.iter().map(|command| {
        let id = string(&command.id);
        let name = string(&command.name);
        let doc = string(&command.doc);
        let params = command.params.iter().map(|param| {
            let name = string(&param.name);
            let ty = type_tokens(&param.ty);
            let doc = string(&param.doc);
            quote!(#p::ParamSpec { name: #name, ty: #ty, doc: #doc })
        });
        let result = type_tokens(&command.result);
        let errors = command.errors.iter().map(|error| string(error));
        let retry = match command.policy.retry {
            Retry::Never => quote!(#p::Retry::Never),
            Retry::Idempotent => quote!(#p::Retry::Idempotent),
        };
        let kind = match command.kind {
            OperationKind::Command => quote!(#p::OperationKind::Command),
            OperationKind::Query => quote!(#p::OperationKind::Query),
        };
        let concurrency = match command.policy.concurrency {
            Concurrency::Serial => quote!(#p::Concurrency::Serial),
            Concurrency::Overlapping => quote!(#p::Concurrency::Overlapping),
        };
        let cancellable = command.policy.cancellable;
        let cli = command.external.cli.iter().map(|word| string(word));
        let mcp = string(&command.external.mcp);
        let reads = command.reads.iter().map(|field| string(field));
        quote! {
            #p::CommandSpec {
                id: #id,
                name: #name,
                doc: #doc,
                kind: #kind,
                scope: #p::Scope::Engine,
                params: ::std::vec![#(#params),*],
                result: #result,
                errors: ::std::vec![#(#errors),*],
                policy: #p::CommandPolicy {
                    concurrency: #concurrency,
                    retry: #retry,
                    cancellable: #cancellable,
                },
                external: #p::ExternalNames { cli: ::std::vec![#(#cli),*], mcp: #mcp },
                reads: ::std::vec![#(#reads),*],
            }
        }
    });
    let namespace = string(&descriptor.namespace);
    let version = descriptor.version;
    let descriptor_version = descriptor.descriptor_version;
    quote! {
        #p::ProtocolDescriptor {
            descriptor_version: #descriptor_version,
            namespace: #namespace,
            version: #version,
            types: #p::TypeTable {
                enums: ::std::vec![#(#enums),*],
                records: ::std::vec![#(#records),*],
            },
            state: ::std::vec![#(#state),*],
            commands: ::std::vec![#(#commands),*],
        }
    }
}
