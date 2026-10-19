//! Lowering of declarations to protocol schema, shared by the macros and
//! the build tool so both derive identical descriptors.

use crate::{ProtocolItem, TypeRef, ValueKind};
use bern_protocol::format::{self, Use};
use bern_protocol::{
    CommandPolicy, CommandSpec, Concurrency, DESCRIPTOR_VERSION, EnumDef, EventSpec, ExternalNames,
    FieldDef, LocalizedText, OperationKind, ParamSpec, ProtocolDescriptor, RecordDef, Retry, Scope,
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

/// Whether text is the whole of a type: text, or optional or listed text.
pub fn is_text(ty: &Type) -> bool {
    match ty {
        Type::Text => true,
        Type::Optional { inner } | Type::List { inner } => is_text(inner),
        _ => false,
    }
}

/// `#[content]` of a field, which only text takes.
pub fn content(field: &crate::FieldItem, ty: &Type) -> syn::Result<bool> {
    if field.content && !is_text(ty) {
        return Err(syn::Error::new(
            field.span,
            format!(
                "`#[content]` marks text that is data, such as a title; `{}` is {}",
                field.name,
                ty.display()
            ),
        ));
    }
    Ok(field.content)
}

/// Whether `ty` is the framework's calendar date.
pub fn is_date(ty: &Type) -> bool {
    matches!(ty, Type::Record { name } if name == format::CALENDAR_DATE)
}

/// A variant's label. Its placeholders are the variant's fields, shown as
/// the label's text says: text that is data (`#[content]`), numbers, enums
/// by their labels, dates with `{field, date, …}`.
pub fn label(
    enum_name: &str,
    variant: &str,
    tr: &crate::Tr,
    fields: &[FieldDef],
) -> syn::Result<LocalizedText> {
    let mut params: Vec<String> = Vec::new();
    for entry in &tr.entries {
        let pieces = format::parse(&entry.text)
            .map_err(|message| syn::Error::new(entry.span, message))?;
        for (name, how) in format::uses(&pieces) {
            let Some(field) = fields.iter().find(|field| field.name == name) else {
                return Err(syn::Error::new(
                    entry.span,
                    format!(
                        "`{{{name}}}` is not a field of `{enum_name}::{variant}`; a label shows the variant's own fields"
                    ),
                ));
            };
            let fits = match how {
                Use::Value => match &field.ty {
                    Type::Text => {
                        if !field.content {
                            return Err(syn::Error::new(
                                entry.span,
                                format!(
                                    "the label of `{enum_name}::{variant}` shows `{name}`, text without translations; mark the field `#[content]` if it is data such as a title, or make it a labelled enum"
                                ),
                            ));
                        }
                        true
                    }
                    Type::Int | Type::Float | Type::Enum { .. } => true,
                    _ => false,
                },
                Use::Plural | Use::Duration => matches!(field.ty, Type::Int | Type::Float),
                Use::Date => is_date(&field.ty),
            };
            if !fits {
                return Err(syn::Error::new(
                    entry.span,
                    format!(
                        "`{name}` of `{enum_name}::{variant}` is {}, which `{{{name}{}}}` cannot show",
                        field.ty.display(),
                        match how {
                            Use::Value => String::new(),
                            other => format!(", {}, …", other.name()),
                        }
                    ),
                ));
            }
            if !params.contains(&name) {
                params.push(name);
            }
        }
    }
    Ok(LocalizedText {
        key: bern_protocol::label_key(enum_name, variant),
        translations: tr
            .entries
            .iter()
            .map(|entry| (entry.language.clone(), entry.text.clone()))
            .collect(),
        params,
    })
}

/// Every type written in a protocol: value fields, state and parameters.
fn protocol_types(protocol: &ProtocolItem) -> Vec<&TypeRef> {
    let mut types = Vec::new();
    for value in &protocol.values {
        match &value.kind {
            ValueKind::Enum(variants) => {
                for variant in variants {
                    types.extend(variant.fields.iter().map(|field| &field.ty));
                }
            }
            ValueKind::Record(fields) => types.extend(fields.iter().map(|field| &field.ty)),
        }
    }
    types.extend(protocol.state.fields.iter().map(|field| &field.ty));
    for method in &protocol.commands.methods {
        types.extend(method.params.iter().map(|param| &param.ty));
        types.push(&method.result);
    }
    for method in protocol.events.iter().flat_map(|events| &events.methods) {
        types.extend(method.params.iter().map(|param| &param.ty));
    }
    for method in protocol.queries.iter().flat_map(|queries| &queries.methods) {
        types.extend(method.params.iter().map(|param| &param.ty));
        types.push(&method.result);
    }
    types
}

/// Collect the protocol's types, state and commands into a descriptor.
pub fn protocol_descriptor(protocol: &ProtocolItem) -> syn::Result<ProtocolDescriptor> {
    // Names first, so field types can refer to any declared value.
    let mut types = TypeTable::default();
    for value in &protocol.values {
        for framework in [bern_media::MEDIA_SOURCE, format::CALENDAR_DATE] {
            if value.name == framework {
                return Err(syn::Error::new(
                    value.span,
                    format!(
                        "`{framework}` is a framework type; use it from `bern::prelude` instead of declaring it"
                    ),
                ));
            }
        }
    }
    // Framework value types join the table when the protocol uses them.
    let used = protocol_types(protocol);
    if used
        .iter()
        .any(|ty| crate::mentions(ty, bern_media::MEDIA_SOURCE))
    {
        types.records.push(bern_media::media_source_def());
    }
    if used
        .iter()
        .any(|ty| crate::mentions(ty, format::CALENDAR_DATE))
    {
        types.records.push(format::calendar_date_def());
    }
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
                        let ty = resolve(&field.ty, &lookup, field.span)?;
                        fields.push(FieldDef {
                            name: field.name.to_string(),
                            content: content(field, &ty)?,
                            ty,
                            doc: field.doc.clone(),
                        });
                    }
                    let text = match &variant.label {
                        Some(tr) => Some(label(&name, &variant.name.to_string(), tr, &fields)?),
                        None => None,
                    };
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
                    let ty = resolve(&field.ty, &lookup, field.span)?;
                    defs.push(FieldDef {
                        name: field.name.to_string(),
                        content: content(field, &ty)?,
                        ty,
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
        let ty = resolve(&field.ty, &types, field.span)?;
        state.push(StateField {
            name: field.name.to_string(),
            content: content(field, &ty)?,
            ty,
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
            reads: vec![],
            on: method
                .on
                .as_ref()
                .map(|(module, event)| format!("{module}.{event}")),
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
    // Queries follow the commands in the catalog, so adding one never
    // renumbers a command.
    for method in protocol.queries.iter().flat_map(|queries| &queries.methods) {
        let name = method.name.to_string();
        let mut params = Vec::new();
        for param in &method.params {
            params.push(ParamSpec {
                name: param.name.to_string(),
                ty: resolve(&param.ty, &types, param.name.span())?,
                doc: String::new(),
            });
        }
        let mut reads = Vec::new();
        for field in &method.reads {
            if !state.iter().any(|state| *field == state.name) {
                return Err(syn::Error::new(
                    field.span(),
                    format!(
                        "`{field}` is not a field of the protocol state; `#[reads(…)]` names \
                         the state fields the query's answer depends on"
                    ),
                ));
            }
            reads.push(field.to_string());
        }
        commands.push(CommandSpec {
            id: format!("{namespace}.{name}"),
            external: ExternalNames::derive(&namespace, &name),
            reads,
            on: None,
            name,
            doc: method.doc.clone(),
            kind: OperationKind::Query,
            scope: Scope::Engine,
            params,
            result: resolve(&method.result, &types, method.span)?,
            errors: method.errors.clone(),
            // Reads can always be repeated; answers may overlap and a
            // superseded request is cancelled.
            policy: CommandPolicy {
                concurrency: Concurrency::Overlapping,
                retry: Retry::Idempotent,
                cancellable: true,
            },
        });
    }
    let mut events = Vec::new();
    for event in protocol.events.iter().flat_map(|events| &events.methods) {
        let mut params = Vec::new();
        for param in &event.params {
            params.push(ParamSpec {
                name: param.name.to_string(),
                ty: resolve(&param.ty, &types, param.name.span())?,
                doc: String::new(),
            });
        }
        events.push(EventSpec {
            name: event.name.to_string(),
            doc: event.doc.clone(),
            params,
        });
    }
    let descriptor = ProtocolDescriptor {
        modules: Vec::new(),
        events,
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
/// translations in strict mode, plural forms a language lacks, and
/// placeholder/argument mismatches. `languages` is `(supported, strict)`
/// when a project configuration exists.
pub fn check_translations(
    tr: &crate::Tr,
    languages: Option<(&[String], bool)>,
) -> Vec<(Span, String)> {
    let mut problems = check_languages(tr, languages);
    problems.extend(check_arguments(tr));
    problems
}

/// The language side of [`check_translations`]: tags, missing languages
/// and the plural forms each language needs. Enum labels, whose
/// placeholders are their variant's fields, take only these checks.
pub fn check_languages(
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
                format!(
                    "error[B101]: language `{}` is translated twice",
                    entry.language
                ),
            ));
        }
        if let Some((supported, _)) = languages
            && !supported.contains(&entry.language)
        {
            problems.push((
                entry.span,
                format!(
                    "error[B101]: language tag `{}` is not in [languages].supported ({}); check the spelling",
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
    for entry in &tr.entries {
        if let Ok(pieces) = format::parse(&entry.text) {
            let missing = format::missing_plurals(&pieces, &entry.language);
            if !missing.is_empty() {
                problems.push((
                    entry.span,
                    format!(
                        "error[B104]: the `{}` text needs the plural branch {}, a form every number in the language takes",
                        entry.language,
                        missing.join(", ")
                    ),
                ));
            }
        }
    }
    problems
}

/// The argument side of [`check_translations`]: every placeholder has an
/// argument, every translation uses the same ones, every argument is used.
fn check_arguments(tr: &crate::Tr) -> Vec<(Span, String)> {
    use std::collections::BTreeSet;
    let mut problems = Vec::new();
    let declared: BTreeSet<String> = tr.args.iter().map(|arg| arg.name.to_string()).collect();
    let mut used = BTreeSet::new();
    let mut reference: Option<BTreeSet<String>> = None;
    for entry in &tr.entries {
        match LocalizedText::placeholders(&entry.text) {
            Err(message) => problems.push((entry.span, format!("error[B104]: {message}"))),
            Ok(names) => {
                let names: BTreeSet<String> = names.into_iter().collect();
                for name in &names {
                    if !declared.contains(name) {
                        problems.push((
                            entry.span,
                            format!(
                                "error[B104]: placeholder `{{{name}}}` has no argument; add `; {name}: value`"
                            ),
                        ));
                    }
                }
                match &reference {
                    None => reference = Some(names.clone()),
                    Some(first) if *first != names => problems.push((
                        entry.span,
                        "error[B104]: every translation must use the same placeholders".to_string(),
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
                format!(
                    "error[B104]: argument `{}` is not used by any translation",
                    arg.name
                ),
            ));
        }
    }
    problems
}
