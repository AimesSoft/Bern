//! Lowering of declarations to protocol schema, shared by the macros and
//! the build tool so both derive identical descriptors.

use crate::{ProtocolItem, TypeRef, ValueKind};
use bern_protocol::{
    CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, EnumDef, ExternalNames, FieldDef,
    LocalizedText, OperationKind, ParamSpec, ProtocolDescriptor, RecordDef, Retry, Scope,
    StateField, Type, TypeTable, VariantDef,
};
use proc_macro2::Span;

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

fn label(enum_name: &str, variant: &str, tr: &crate::Tr) -> LocalizedText {
    LocalizedText {
        key: bern_protocol::label_key(enum_name, variant),
        translations: tr
            .entries
            .iter()
            .map(|entry| (entry.language.clone(), entry.text.clone()))
            .collect(),
        params: Vec::new(),
    }
}

/// Collect the protocol's types, state and commands into a descriptor.
pub fn protocol_descriptor(protocol: &ProtocolItem) -> syn::Result<ProtocolDescriptor> {
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
                    let text = variant
                        .label
                        .as_ref()
                        .map(|tr| label(&name, &variant.name.to_string(), tr));
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

/// Problems with one `tr!`: duplicate or unknown language tags, missing
/// translations in strict mode, and placeholder/argument mismatches.
/// `languages` is `(supported, strict)` when a project configuration exists.
pub fn check_translations(
    tr: &crate::Tr,
    languages: Option<(&[String], bool)>,
) -> Vec<(Span, String)> {
    use std::collections::BTreeSet;
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in &tr.entries {
        if !seen.insert(entry.language.as_str()) {
            problems.push((
                entry.span,
                format!("language `{}` is translated twice", entry.language),
            ));
        }
        if let Some((supported, _)) = languages
            && !supported.contains(&entry.language)
        {
            problems.push((
                entry.span,
                format!(
                    "language tag `{}` is not in [languages].supported ({}); check the spelling",
                    entry.language,
                    supported.join(", ")
                ),
            ));
        }
    }
    if let Some((supported, true)) = languages {
        let missing: Vec<_> = supported
            .iter()
            .filter(|language| !seen.contains(language.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            problems.push((
                tr.span,
                format!(
                    "error[B102]: Missing translation\nMissing language: {}\nRequired languages: {}",
                    missing.join(", "),
                    supported.join(", ")
                ),
            ));
        }
    }
    let declared: BTreeSet<String> = tr.args.iter().map(|arg| arg.name.to_string()).collect();
    let mut used = BTreeSet::new();
    let mut reference: Option<BTreeSet<String>> = None;
    for entry in &tr.entries {
        match LocalizedText::placeholders(&entry.text) {
            Err(message) => problems.push((entry.span, message)),
            Ok(names) => {
                let names: BTreeSet<String> = names.into_iter().collect();
                for name in &names {
                    if !declared.contains(name) {
                        problems.push((
                            entry.span,
                            format!(
                                "placeholder `{{{name}}}` has no argument; add `; {name}: value`"
                            ),
                        ));
                    }
                }
                match &reference {
                    None => reference = Some(names.clone()),
                    Some(first) if *first != names => problems.push((
                        entry.span,
                        "every translation must use the same placeholders".to_string(),
                    )),
                    _ => {}
                }
                used.extend(names);
            }
        }
    }
    for arg in &tr.args {
        if !used.contains(&arg.name.to_string()) {
            problems.push((
                arg.name.span(),
                format!("argument `{}` is not used by any translation", arg.name),
            ));
        }
    }
    problems
}
