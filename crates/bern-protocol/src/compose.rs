//! One catalog from an application's shell protocol and its modules.
//!
//! Each module keeps its own namespace, so its command ids stay
//! `module.command`. Its state becomes one field of the application state,
//! named after the module, whose type is a record synthesized from the
//! module's state fields. Type names are unique across the application:
//! two different definitions with one name are an error.
//!
//! The build tool composes the descriptors it reads from source and the
//! running engine composes the ones the protocol macros generated, with
//! this same function, so both sides arrive at the same fingerprint.

use crate::{
    CommandPolicy, CommandSpec, DescriptorError, ExternalNames, FieldDef, MODULES_NAMESPACE,
    OperationKind, ParamSpec, ProtocolDescriptor, RecordDef, Retry, Scope, StateField, Type,
    valid_name,
};

/// One module of an application: its catalog, and whether it can be
/// switched off at run time.
#[derive(Clone, Copy, Debug)]
pub struct ModulePart<'a> {
    pub descriptor: &'a ProtocolDescriptor,
    pub optional: bool,
}

/// Record type of the `modules` state field: one `bool` per optional
/// module, true while the module runs.
pub const MODULES_STATE_TYPE: &str = "Modules";

/// Name of the command switching an optional module: `modules.set_enabled`.
pub const SET_ENABLED: &str = "set_enabled";

/// Name of the record holding a module's state: `downloads` gives
/// `DownloadsState`, `network_sources` gives `NetworkSourcesState`.
pub fn module_state_type(namespace: &str) -> String {
    let mut name = String::new();
    for word in namespace.split('_') {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            name.extend(first.to_uppercase());
            name.push_str(chars.as_str());
        }
    }
    name.push_str("State");
    name
}

/// The application catalog: the shell's state and commands first, then
/// each module in order. Module query reads name the module's own fields;
/// in the application they read the module's state field as a whole.
///
/// With optional modules the catalog ends with the `modules` state field,
/// whose record holds a `bool` per optional module, and the command
/// `modules.set_enabled(id, enabled)`, which returns the ids of the modules
/// it switched, dependencies included.
pub fn compose(
    shell: &ProtocolDescriptor,
    parts: &[ModulePart<'_>],
) -> Result<ProtocolDescriptor, DescriptorError> {
    let modules: Vec<&ProtocolDescriptor> = parts.iter().map(|part| part.descriptor).collect();
    let mut composed = shell.clone();
    // In the application every event is named `namespace.event`.
    let qualify = |namespace: &str, events: &[crate::EventSpec]| {
        events
            .iter()
            .map(|event| crate::EventSpec {
                name: format!("{namespace}.{}", event.name),
                ..event.clone()
            })
            .collect::<Vec<_>>()
    };
    composed.events = qualify(&shell.namespace, &shell.events);
    for module in &modules {
        let namespace = &module.namespace;
        if !valid_name(namespace) {
            return Err(DescriptorError(format!(
                "invalid module namespace `{namespace}`; use lower snake case"
            )));
        }
        if namespace == &shell.namespace
            || modules
                .iter()
                .filter(|other| &other.namespace == namespace)
                .count()
                > 1
        {
            return Err(DescriptorError(format!(
                "namespace `{namespace}` is used by more than one protocol"
            )));
        }
        if composed.state_index(namespace).is_some() {
            return Err(DescriptorError(format!(
                "state field `{namespace}` of `{}` has the name of a module",
                shell.namespace
            )));
        }
        composed
            .types
            .merge(&module.types)
            .map_err(|error| DescriptorError(format!("module `{namespace}`: {}", error.message)))?;
        let record = module_state_type(namespace);
        if composed.types.enum_def(&record).is_some()
            || composed.types.record_def(&record).is_some()
        {
            return Err(DescriptorError(format!(
                "type `{record}` is declared by the application; the state of module `{namespace}` uses that name"
            )));
        }
        composed.types.records.push(RecordDef {
            name: record.clone(),
            fields: module
                .state
                .iter()
                .map(|field| FieldDef {
                    name: field.name.clone(),
                    ty: field.ty.clone(),
                    doc: field.doc.clone(),
                    content: field.content,
                })
                .collect(),
        });
        composed.state.push(StateField {
            name: namespace.clone(),
            ty: Type::Record { name: record },
            doc: format!("State of the `{namespace}` module."),
            content: false,
        });
        if !module.modules.is_empty() {
            return Err(DescriptorError(format!(
                "module `{namespace}` is itself composed; modules do not nest"
            )));
        }
        composed.modules.push(namespace.clone());
        composed.events.extend(qualify(namespace, &module.events));
        for command in &module.commands {
            let mut command = command.clone();
            if !command.reads.is_empty() {
                command.reads = vec![namespace.clone()];
            }
            composed.commands.push(command);
        }
    }
    // A command answering an event takes exactly the event's parameters.
    for command in &composed.commands {
        let Some(on) = &command.on else {
            continue;
        };
        let Some(event) = composed.events.iter().find(|event| &event.name == on) else {
            return Err(DescriptorError(format!(
                "command `{}` answers `{on}`, which no protocol of the application emits",
                command.id
            )));
        };
        let wanted: Vec<_> = event.params.iter().map(|p| (&p.name, &p.ty)).collect();
        let given: Vec<_> = command.params.iter().map(|p| (&p.name, &p.ty)).collect();
        if wanted != given {
            return Err(DescriptorError(format!(
                "command `{}` answers `{on}`, so it takes the event's parameters ({})",
                command.id,
                event
                    .params
                    .iter()
                    .map(|param| format!("{}: {}", param.name, param.ty.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    let optional: Vec<&str> = parts
        .iter()
        .filter(|part| part.optional)
        .map(|part| part.descriptor.namespace.as_str())
        .collect();
    if !optional.is_empty() {
        if composed.state_index(MODULES_NAMESPACE).is_some()
            || composed.types.record_def(MODULES_STATE_TYPE).is_some()
            || composed.types.enum_def(MODULES_STATE_TYPE).is_some()
        {
            return Err(DescriptorError(format!(
                "the application declares `{MODULES_NAMESPACE}` or `{MODULES_STATE_TYPE}`, which optional modules use"
            )));
        }
        composed.types.records.push(RecordDef {
            name: MODULES_STATE_TYPE.into(),
            fields: optional
                .iter()
                .map(|namespace| FieldDef {
                    name: (*namespace).into(),
                    ty: Type::Bool,
                    doc: format!("Whether the `{namespace}` module runs."),
                    content: false,
                })
                .collect(),
        });
        composed.state.push(StateField {
            name: MODULES_NAMESPACE.into(),
            ty: Type::Record {
                name: MODULES_STATE_TYPE.into(),
            },
            doc: "Which optional modules run.".into(),
            content: false,
        });
        composed.commands.push(CommandSpec {
            id: format!("{MODULES_NAMESPACE}.{SET_ENABLED}"),
            name: SET_ENABLED.into(),
            doc: "Switch an optional module on or off. Switching one on switches on the modules it depends on; switching one off switches off the modules depending on it. Returns the ids of every module switched.".into(),
            kind: OperationKind::Command,
            scope: Scope::Engine,
            params: vec![
                ParamSpec {
                    name: "id".into(),
                    ty: Type::Text,
                    doc: "Module id".into(),
                },
                ParamSpec {
                    name: "enabled".into(),
                    ty: Type::Bool,
                    doc: "Whether the module runs".into(),
                },
            ],
            result: Type::list(Type::Text),
            errors: vec!["unknown_module".into(), "not_optional".into()],
            policy: CommandPolicy {
                retry: Retry::Idempotent,
                ..CommandPolicy::default()
            },
            external: ExternalNames::derive(MODULES_NAMESPACE, SET_ENABLED),
            reads: vec![],
            on: None,
        });
    }
    composed
        .validate()
        .map_err(|error| DescriptorError(format!("the application catalog is invalid: {error}")))?;
    Ok(composed)
}

/// Where each part's commands start in the composed catalog: the shell at
/// 0, then every module in order. `modules.set_enabled`, if any, follows.
pub fn command_offsets(shell: &ProtocolDescriptor, parts: &[ModulePart<'_>]) -> Vec<usize> {
    let mut offsets = vec![0];
    let mut next = shell.commands.len();
    for part in parts {
        offsets.push(next);
        next += part.descriptor.commands.len();
    }
    offsets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, ExternalNames, OperationKind, Scope,
        TypeTable,
    };

    fn command(namespace: &str, name: &str) -> CommandSpec {
        CommandSpec {
            id: format!("{namespace}.{name}"),
            name: name.into(),
            doc: String::new(),
            kind: OperationKind::Command,
            scope: Scope::Engine,
            params: vec![],
            result: Type::Unit,
            errors: vec![],
            policy: CommandPolicy::default(),
            external: ExternalNames::derive(namespace, name),
            reads: vec![],
            on: None,
        }
    }

    fn part(descriptor: &ProtocolDescriptor) -> ModulePart<'_> {
        ModulePart {
            descriptor,
            optional: false,
        }
    }

    fn protocol(namespace: &str, fields: &[&str], commands: &[&str]) -> ProtocolDescriptor {
        ProtocolDescriptor {
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: namespace.into(),
            version: 1,
            types: TypeTable::default(),
            state: fields
                .iter()
                .map(|name| StateField {
                    name: (*name).into(),
                    ty: Type::Int,
                    doc: String::new(),
                    content: false,
                })
                .collect(),
            commands: commands
                .iter()
                .map(|name| command(namespace, name))
                .collect(),
            modules: Vec::new(),
            events: Vec::new(),
        }
    }

    #[test]
    fn modules_become_state_records_and_keep_their_command_ids() {
        let shell = protocol("app", &["count"], &["reset"]);
        let downloads = protocol("downloads", &["active", "done"], &["add", "remove"]);
        let library = protocol("network_library", &["items"], &["add"]);
        let composed = compose(&shell, &[part(&downloads), part(&library)]).unwrap();
        let names: Vec<_> = composed.state.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["count", "downloads", "network_library"]);
        assert_eq!(
            composed.state[1].ty,
            Type::Record {
                name: "DownloadsState".into()
            }
        );
        let record = composed.types.record_def("NetworkLibraryState").unwrap();
        assert_eq!(record.fields[0].name, "items");
        let ids: Vec<_> = composed.commands.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "app.reset",
                "downloads.add",
                "downloads.remove",
                "network_library.add"
            ]
        );
        assert_eq!(
            command_offsets(&shell, &[part(&downloads), part(&library)]),
            [0, 1, 3]
        );
    }

    #[test]
    fn query_reads_name_the_module_field() {
        let shell = protocol("app", &[], &[]);
        let mut downloads = protocol("downloads", &["active"], &[]);
        let mut search = command("downloads", "search");
        search.kind = OperationKind::Query;
        search.reads = vec!["active".into()];
        downloads.commands.push(search);
        let composed = compose(&shell, &[part(&downloads)]).unwrap();
        assert_eq!(composed.commands[0].reads, ["downloads"]);
    }

    #[test]
    fn clashing_namespaces_fields_and_types_fail() {
        let shell = protocol("app", &["downloads"], &[]);
        let downloads = protocol("downloads", &[], &[]);
        assert!(compose(&shell, &[part(&downloads)]).is_err());
        let shell = protocol("app", &[], &[]);
        assert!(compose(&shell, &[part(&downloads), part(&downloads)]).is_err());
        assert!(compose(&shell, &[part(&protocol("app", &[], &[]))]).is_err());

        let mut a = protocol("a", &[], &[]);
        a.types.records.push(RecordDef {
            name: "Item".into(),
            fields: vec![],
        });
        let mut b = protocol("b", &[], &[]);
        b.types.records.push(RecordDef {
            name: "Item".into(),
            fields: vec![FieldDef {
                name: "id".into(),
                ty: Type::Int,
                doc: String::new(),
                content: false,
            }],
        });
        assert!(compose(&shell, &[part(&a), part(&b)]).is_err());

        let mut shell = protocol("app", &[], &[]);
        shell.types.records.push(RecordDef {
            name: "AState".into(),
            fields: vec![],
        });
        assert!(compose(&shell, &[part(&a)]).is_err());
    }

    #[test]
    fn optional_modules_add_their_switch() {
        let shell = protocol("app", &[], &["reset"]);
        let notes = protocol("notes", &["count"], &["add"]);
        let tags = protocol("tags", &[], &[]);
        let composed = compose(
            &shell,
            &[
                part(&notes),
                ModulePart {
                    descriptor: &tags,
                    optional: true,
                },
            ],
        )
        .unwrap();
        let fields: Vec<_> = composed.state.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(fields, ["notes", "tags", "modules"]);
        let switches = composed.types.record_def(MODULES_STATE_TYPE).unwrap();
        assert_eq!(switches.fields.len(), 1);
        assert_eq!(switches.fields[0].name, "tags");
        let last = composed.commands.last().unwrap();
        assert_eq!(last.id, "modules.set_enabled");
        assert_eq!(last.external.cli, ["modules", "set-enabled"]);
        // Without optional modules nothing is added.
        let plain = compose(&shell, &[part(&notes)]).unwrap();
        assert!(plain.state_index("modules").is_none());
    }

    #[test]
    fn answers_take_the_event_parameters() {
        let shell = protocol("app", &[], &[]);
        let mut notes = protocol("notes", &[], &["add"]);
        let param = |name: &str| crate::ParamSpec {
            name: name.into(),
            ty: Type::Int,
            doc: String::new(),
        };
        notes.events.push(crate::EventSpec {
            name: "added".into(),
            doc: String::new(),
            params: vec![param("id")],
        });
        let mut tags = protocol("tags", &[], &["label"]);
        tags.commands[0].on = Some("notes.added".into());
        tags.commands[0].params = vec![param("id")];
        let composed = compose(&shell, &[part(&notes), part(&tags)]).unwrap();
        assert_eq!(composed.events[0].name, "notes.added");
        // Other parameters, or an event nobody emits, are refused.
        tags.commands[0].params = vec![param("note")];
        assert!(compose(&shell, &[part(&notes), part(&tags)]).is_err());
        tags.commands[0].params = vec![param("id")];
        tags.commands[0].on = Some("notes.removed".into());
        assert!(compose(&shell, &[part(&notes), part(&tags)]).is_err());
    }

    #[test]
    fn state_type_names_are_upper_camel() {
        assert_eq!(module_state_type("downloads"), "DownloadsState");
        assert_eq!(module_state_type("network_sources"), "NetworkSourcesState");
    }
}
