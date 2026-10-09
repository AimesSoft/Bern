//! Build a protocol descriptor at expansion time and emit code that
//! reconstructs exactly the same value at run time.

use crate::types::{resolve, type_tokens};
use bern_protocol::{
    CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, EnumDef, ExternalNames, FieldDef,
    LocalizedText, OperationKind, ParamSpec, ProtocolDescriptor, RecordDef, Retry, Scope,
    StateField, TypeTable, VariantDef,
};
use bern_syntax::{ProtocolItem, ValueKind};
use proc_macro2::TokenStream;
use quote::quote;

fn label(enum_name: &str, tr: &bern_syntax::Tr) -> LocalizedText {
    LocalizedText {
        key: bern_ir::label_key(enum_name, ""),
        translations: tr
            .entries
            .iter()
            .map(|entry| (entry.language.clone(), entry.text.clone()))
            .collect(),
        params: Vec::new(),
    }
}

/// Collect the protocol's types, state and commands into a descriptor.
pub fn build(protocol: &ProtocolItem) -> syn::Result<ProtocolDescriptor> {
    // Names first, so field types can refer to any declared value.
    let mut types = TypeTable::default();
    for value in &protocol.values {
        match &value.kind {
            ValueKind::Enum(_) => types.enums.push(EnumDef {
                name: value.name.to_string(),
                variants: vec![],
            }),
            ValueKind::Record(_) => types.records.push(RecordDef {
                name: value.name.to_string(),
                fields: vec![],
            }),
        }
    }
    let lookup = types.clone();
    for value in &protocol.values {
        let name = value.name.to_string();
        match &value.kind {
            ValueKind::Enum(variants) => {
                let mut defs = Vec::new();
                for variant in variants {
                    let mut fields = Vec::new();
                    for field in &variant.fields {
                        fields.push(FieldDef {
                            name: field.name.to_string(),
                            ty: resolve(&field.ty, &lookup, field.span)?,
                            doc: field.doc.clone(),
                        });
                    }
                    let mut text = variant.label.as_ref().map(|tr| label(&name, tr));
                    if let Some(text) = &mut text {
                        text.key = bern_ir::label_key(&name, &variant.name.to_string());
                    }
                    defs.push(VariantDef {
                        name: variant.name.to_string(),
                        fields,
                        label: text,
                    });
                }
                types
                    .enums
                    .iter_mut()
                    .find(|def| def.name == name)
                    .unwrap()
                    .variants = defs;
            }
            ValueKind::Record(fields) => {
                let mut defs = Vec::new();
                for field in fields {
                    defs.push(FieldDef {
                        name: field.name.to_string(),
                        ty: resolve(&field.ty, &lookup, field.span)?,
                        doc: field.doc.clone(),
                    });
                }
                types
                    .records
                    .iter_mut()
                    .find(|def| def.name == name)
                    .unwrap()
                    .fields = defs;
            }
        }
    }
    let mut state = Vec::new();
    for field in &protocol.state.fields {
        state.push(StateField {
            name: field.name.to_string(),
            ty: resolve(&field.ty, &types, field.span)?,
            doc: field.doc.clone(),
        });
    }
    let namespace = protocol.args.namespace.clone();
    let mut commands = Vec::new();
    for method in &protocol.commands.methods {
        let name = method.name.to_string();
        let mut params = Vec::new();
        for param in &method.params {
            params.push(ParamSpec {
                name: param.name.to_string(),
                ty: resolve(&param.ty, &types, param.name.span())?,
                doc: String::new(),
            });
        }
        commands.push(CommandSpec {
            id: format!("{namespace}.{name}"),
            external: ExternalNames::derive(&namespace, &name),
            name,
            doc: method.doc.clone(),
            kind: OperationKind::Command,
            scope: Scope::Engine,
            params,
            result: resolve(&method.result, &types, method.span)?,
            errors: method.errors.clone(),
            policy: CommandPolicy {
                retry: if method.idempotent {
                    Retry::Idempotent
                } else {
                    Retry::Never
                },
                ..CommandPolicy::default()
            },
        });
    }
    let descriptor = ProtocolDescriptor {
        descriptor_version: DESCRIPTOR_VERSION,
        namespace,
        version: protocol.args.version,
        types,
        state,
        commands,
    };
    descriptor
        .validate()
        .map_err(|error| syn::Error::new(protocol.args.span, error.0))?;
    Ok(descriptor)
}

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
        let cli = command.external.cli.iter().map(|word| string(word));
        let mcp = string(&command.external.mcp);
        quote! {
            #p::CommandSpec {
                id: #id,
                name: #name,
                doc: #doc,
                kind: #p::OperationKind::Command,
                scope: #p::Scope::Engine,
                params: ::std::vec![#(#params),*],
                result: #result,
                errors: ::std::vec![#(#errors),*],
                policy: #p::CommandPolicy {
                    concurrency: #p::Concurrency::Serial,
                    retry: #retry,
                    cancellable: false,
                },
                external: #p::ExternalNames { cli: ::std::vec![#(#cli),*], mcp: #mcp },
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
