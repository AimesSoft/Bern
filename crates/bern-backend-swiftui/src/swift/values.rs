//! The values the views read: enums and records, and the engine and
//! session state that holds them.

use super::Out;
use crate::names::{case, double, member, string};
use bern_ir::Application;
use bern_protocol::{EnumDef, RecordDef, Type, TypeTable, Value};

pub fn swift_type(ty: &Type) -> String {
    match ty {
        Type::Unit => "Void".into(),
        Type::Bool => "Bool".into(),
        Type::Int => "Int64".into(),
        Type::Float => "Double".into(),
        Type::Text => "String".into(),
        Type::Enum { name } | Type::Record { name } => name.clone(),
        Type::Optional { inner } => format!("Optional<{}>", swift_type(inner)),
        Type::List { inner } => format!("[{}]", swift_type(inner)),
    }
}

/// Swift expression for a schema value.
pub(super) fn value_literal(value: &Value, ty: &Type, types: &TypeTable) -> String {
    match (value, ty) {
        (Value::Bool(value), _) => value.to_string(),
        (Value::Int(value), _) => format!("Int64({value})"),
        (Value::Float(value), _) => format!("Double({})", double(*value)),
        (Value::Text(value), _) => string(value),
        (Value::Optional(None), _) => "nil".into(),
        (Value::Optional(Some(value)), Type::Optional { inner }) => {
            value_literal(value, inner, types)
        }
        (Value::List(items), Type::List { inner }) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| value_literal(item, inner, types))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        (Value::Enum { variant, fields }, Type::Enum { name }) => {
            let def = types.enum_def(name).expect("known enum");
            let variant_def = &def.variants[*variant as usize];
            if variant_def.fields.is_empty() {
                format!("{name}.{}", case(&variant_def.name))
            } else {
                let args: Vec<_> = fields
                    .iter()
                    .zip(&variant_def.fields)
                    .map(|(value, field)| {
                        format!(
                            "{}: {}",
                            member(&field.name),
                            value_literal(value, &field.ty, types)
                        )
                    })
                    .collect();
                format!("{name}.{}({})", case(&variant_def.name), args.join(", "))
            }
        }
        (Value::Record(values), Type::Record { name }) => {
            let def = types.record_def(name).expect("known record");
            let args: Vec<_> = values
                .iter()
                .zip(&def.fields)
                .map(|(value, field)| {
                    format!(
                        "{}: {}",
                        member(&field.name),
                        value_literal(value, &field.ty, types)
                    )
                })
                .collect();
            format!("{name}({})", args.join(", "))
        }
        _ => "()".into(),
    }
}

fn default_literal(ty: &Type, types: &TypeTable) -> String {
    let value = Value::default_for(ty, types).unwrap_or(Value::Unit);
    value_literal(&value, ty, types)
}

pub(super) fn generate_enum(out: &mut Out, def: &EnumDef) {
    let name = &def.name;
    let unit_only = def.is_unit_only();
    let mut conformances = vec!["Hashable", "Sendable", "BernCodable"];
    if unit_only {
        conformances.push("CaseIterable");
    }
    out.open(format!("enum {name}: {} {{", conformances.join(", ")));
    for variant in &def.variants {
        if variant.fields.is_empty() {
            out.line(format!("case {}", case(&variant.name)));
        } else {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("{}: {}", member(&field.name), swift_type(&field.ty)))
                .collect();
            out.line(format!(
                "case {}({})",
                case(&variant.name),
                fields.join(", ")
            ));
        }
    }
    out.line("");
    out.open("init(bern reader: inout BernReader) throws {");
    out.open("switch try reader.u32() {");
    for (index, variant) in def.variants.iter().enumerate() {
        if variant.fields.is_empty() {
            out.line(format!("case {index}: self = .{}", case(&variant.name)));
        } else {
            let args: Vec<_> = variant
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: try {}(bern: &reader)",
                        member(&field.name),
                        swift_type(&field.ty)
                    )
                })
                .collect();
            out.line(format!(
                "case {index}: self = .{}({})",
                case(&variant.name),
                args.join(", ")
            ));
        }
    }
    out.line(format!(
        "default: throw BernDecodeError(message: {})",
        string(&format!("invalid {name}"))
    ));
    out.close("}");
    out.close("}");
    out.line("");
    out.open("func encode(bern writer: inout BernWriter) {");
    out.open("switch self {");
    for (index, variant) in def.variants.iter().enumerate() {
        if variant.fields.is_empty() {
            out.line(format!(
                "case .{}: writer.u32({index})",
                case(&variant.name)
            ));
        } else {
            let names: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("let {}", member(&field.name)))
                .collect();
            out.open(format!(
                "case .{}({}):",
                case(&variant.name),
                names.join(", ")
            ));
            out.line(format!("writer.u32({index})"));
            for field in &variant.fields {
                out.line(format!("{}.encode(bern: &writer)", member(&field.name)));
            }
            out.indent -= 1;
        }
    }
    out.close("}");
    out.close("}");
    out.line("");
    out.line("/// The declared variant name, used by external entries.");
    out.open("var bernName: String {");
    out.open("switch self {");
    for variant in &def.variants {
        out.line(format!(
            "case .{}: {}",
            case(&variant.name),
            string(&variant.name)
        ));
    }
    out.close("}");
    out.close("}");
    if unit_only {
        out.line("");
        out.open("init?(bernName: String) {");
        out.open("switch bernName {");
        for variant in &def.variants {
            out.line(format!(
                "case {}: self = .{}",
                string(&variant.name),
                case(&variant.name)
            ));
        }
        out.line("default: return nil");
        out.close("}");
        out.close("}");
    }
    out.close("}");
    if def.variants.iter().all(|variant| variant.label.is_some()) {
        out.line("");
        out.open(format!("extension {name}: BernLabeled {{"));
        out.open("var bernLabel: String {");
        out.open("switch self {");
        for variant in &def.variants {
            let label = variant.label.as_ref().unwrap();
            if label.params.is_empty() {
                out.line(format!(
                    "case .{}: BernText.string({})",
                    case(&variant.name),
                    string(&label.key)
                ));
                continue;
            }
            // A label that shows the variant's fields fills them in.
            let bindings: Vec<_> = variant
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    if label.params.contains(&field.name) {
                        format!("let v{index}")
                    } else {
                        "_".to_string()
                    }
                })
                .collect();
            let args: Vec<_> = label
                .params
                .iter()
                .filter_map(|param| {
                    let at = variant
                        .fields
                        .iter()
                        .position(|field| field.name == *param)?;
                    Some(format!(
                        "{}: {}",
                        string(param),
                        argument(&format!("v{at}"), &variant.fields[at].ty)
                    ))
                })
                .collect();
            out.line(format!(
                "case .{}({}): BernText.format({}, [{}])",
                case(&variant.name),
                bindings.join(", "),
                string(&label.key),
                args.join(", ")
            ));
        }
        out.close("}");
        out.close("}");
        out.close("}");
    }
    out.line("");
}

/// A Swift `BernArg` for a `tr!` argument: dates stay dates for their
/// patterns, numbers keep their value for plural branches.
pub(super) fn argument(value: &str, ty: &Type) -> String {
    match ty {
        Type::Record { name } if name == bern_protocol::format::CALENDAR_DATE => {
            format!("({value}).bernArg")
        }
        _ => format!("BernText.arg({value})"),
    }
}

pub(super) fn generate_record(out: &mut Out, def: &RecordDef) {
    let name = &def.name;
    out.open(format!("struct {name}: Hashable, Sendable {{"));
    for field in &def.fields {
        out.line(format!(
            "var {}: {}",
            member(&field.name),
            swift_type(&field.ty)
        ));
    }
    out.close("}");
    out.line("");
    out.open(format!("extension {name}: BernCodable {{"));
    out.open("init(bern reader: inout BernReader) throws {");
    let args: Vec<_> = def
        .fields
        .iter()
        .map(|field| {
            format!(
                "{}: try {}(bern: &reader)",
                member(&field.name),
                swift_type(&field.ty)
            )
        })
        .collect();
    out.line(format!("self.init({})", args.join(", ")));
    out.close("}");
    out.line("");
    out.open("func encode(bern writer: inout BernWriter) {");
    for field in &def.fields {
        out.line(format!("{}.encode(bern: &writer)", member(&field.name)));
    }
    out.close("}");
    out.close("}");
    out.line("");
    if *name == bern_protocol::format::CALENDAR_DATE {
        out.open(format!("extension {name} {{"));
        out.line("/// The date as an argument of a translated text.");
        out.line("var bernArg: BernArg { .date(year: year, month: month, day: day) }");
        out.close("}");
        out.line("");
    }
}

pub(super) fn generate_engine(out: &mut Out, app: &Application) {
    let protocol = &app.protocol;
    out.line(format!(
        "/// Read-only state of the `{}` engine, mirrored from Rust.",
        protocol.namespace
    ));
    out.line("@MainActor");
    out.line("@Observable");
    out.open("final class BernEngine {");
    out.line("static let shared = BernEngine()");
    out.line("");
    for field in &protocol.state {
        if !field.doc.is_empty() {
            out.line(format!("/// {}", field.doc.replace('\n', " ")));
        }
        out.line(format!(
            "private(set) var {}: {} = {}",
            member(&field.name),
            swift_type(&field.ty),
            default_literal(&field.ty, &app.types)
        ));
    }
    out.line("");
    out.open("func bernApply(_ field: UInt32, _ reader: inout BernReader) throws {");
    out.open("switch field {");
    for (index, field) in protocol.state.iter().enumerate() {
        out.line(format!(
            "case {index}: {} = try {}(bern: &reader)",
            member(&field.name),
            swift_type(&field.ty)
        ));
    }
    out.line("default: throw BernDecodeError(message: \"unknown state field \\(field)\")");
    out.close("}");
    out.close("}");
    for (index, command) in protocol.commands.iter().enumerate() {
        out.line("");
        if !command.doc.is_empty() {
            out.line(format!("/// {}", command.doc.replace('\n', " ")));
        }
        let mut params: Vec<String> = command
            .params
            .iter()
            .map(|param| format!("_ {}: {}", member(&param.name), swift_type(&param.ty)))
            .collect();
        params.push("from window: BernWindowContext?".into());
        params.push("completion: (@MainActor (BernCommandFailure?) -> Void)? = nil".into());
        out.open(format!(
            "func {}({}) {{",
            member(&app.protocol.command_symbol(command)),
            params.join(", ")
        ));
        if command.params.is_empty() {
            out.line(format!(
                "BernHost.shared.dispatch({index}, window: window?.id, encode: {{ _ in }}, completion: completion)"
            ));
        } else {
            out.open(format!(
                "BernHost.shared.dispatch({index}, window: window?.id, encode: {{ writer in"
            ));
            for param in &command.params {
                out.line(format!("{}.encode(bern: &writer)", member(&param.name)));
            }
            out.close("}, completion: completion)");
        }
        out.close("}");
    }
    out.close("}");
    out.line("");
}

pub(super) fn session_class(name: &str) -> String {
    format!("{name}Session")
}

pub(super) fn session_var(name: &str) -> String {
    format!("bern{name}")
}

pub(super) fn generate_sessions(out: &mut Out, app: &Application) {
    for (index, session) in app.sessions.iter().enumerate() {
        let class = session_class(&session.name);
        out.line(format!(
            "/// Drafts of `{}`, one instance per hosting window.",
            session.name
        ));
        out.line("@MainActor");
        out.line("@Observable");
        out.open(format!("final class {class}: BernSessionState {{"));
        out.line(format!("static let bernIndex: UInt32 = {index}"));
        out.line("@ObservationIgnored weak var window: BernWindowContext?");
        for (field_index, field) in session.fields.iter().enumerate() {
            let name = member(&field.name);
            let storage = format!("bern_{}", field.name);
            let ty = swift_type(&field.ty);
            out.line("");
            out.line(format!(
                "private var {storage}: {ty} = {}",
                value_literal(&field.initial, &field.ty, &app.types)
            ));
            out.open(format!("var {name}: {ty} {{"));
            out.line(format!("get {{ {storage} }}"));
            out.open("set {");
            out.line(format!(
                "guard window?.set(session: {index}, field: {field_index}, {{ newValue.encode(bern: &$0) }}) == true else {{ return }}"
            ));
            out.line(format!("{storage} = newValue"));
            out.close("}");
            out.close("}");
        }
        out.line("");
        out.open("func load(_ reader: inout BernReader) throws {");
        for field in &session.fields {
            out.line(format!(
                "bern_{} = try {}(bern: &reader)",
                field.name,
                swift_type(&field.ty)
            ));
        }
        out.close("}");
        out.close("}");
        out.line("");
    }
}
