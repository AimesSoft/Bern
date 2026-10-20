//! The values the composables read: enums and records, and the engine
//! and session state that holds them.

use super::Out;
use crate::names::{double, member, string, value_type};
use bern_ir::Application;
use bern_protocol::{EnumDef, RecordDef, Type, TypeTable, Value};

pub fn kotlin_type(ty: &Type) -> String {
    spell(ty, false)
}

/// The type of a field of a variant class, which sits inside its enum's
/// sealed class next to the other variants: a variant named like a type
/// of Kotlin's standard library (`String`, `List`) hides it there, so
/// those are written in full.
fn field_type(ty: &Type) -> String {
    spell(ty, true)
}

fn spell(ty: &Type, full: bool) -> String {
    let std = |name: &str, package: &str| {
        if full {
            format!("{package}.{name}")
        } else {
            name.to_string()
        }
    };
    match ty {
        Type::Unit => std("Unit", "kotlin"),
        Type::Bool => std("Boolean", "kotlin"),
        Type::Int => std("Long", "kotlin"),
        Type::Float => std("Double", "kotlin"),
        Type::Text => std("String", "kotlin"),
        Type::Enum { name } | Type::Record { name } => value_type(name),
        Type::Optional { inner } => {
            let inner = spell(inner, full);
            if inner.ends_with('?') {
                inner
            } else {
                format!("{inner}?")
            }
        }
        Type::List { inner } => format!(
            "{}<{}>",
            std("List", "kotlin.collections"),
            spell(inner, full)
        ),
    }
}

/// An expression reading a value of `ty` from `reader`.
pub(super) fn decode(ty: &Type, reader: &str) -> String {
    match ty {
        Type::Unit => "Unit".into(),
        Type::Bool => format!("{reader}.bool()"),
        Type::Int => format!("{reader}.i64()"),
        Type::Float => format!("{reader}.f64()"),
        Type::Text => format!("{reader}.text()"),
        Type::Enum { name } | Type::Record { name } => {
            format!("{}.decode({reader})", value_type(name))
        }
        Type::Optional { inner } => format!("{reader}.optional {{ {} }}", decode(inner, reader)),
        Type::List { inner } => format!("{reader}.list {{ {} }}", decode(inner, reader)),
    }
}

/// A statement writing `value` of `ty` to `writer`.
pub(super) fn encode(ty: &Type, value: &str, writer: &str) -> String {
    match ty {
        Type::Unit => String::new(),
        Type::Bool => format!("{writer}.bool({value})"),
        Type::Int => format!("{writer}.i64({value})"),
        Type::Float => format!("{writer}.f64({value})"),
        Type::Text => format!("{writer}.text({value})"),
        Type::Enum { .. } | Type::Record { .. } => format!("{value}.encode({writer})"),
        Type::Optional { inner } => format!(
            "{writer}.optional({value}) {{ {} }}",
            encode(inner, "it", writer)
        ),
        Type::List { inner } => format!(
            "{writer}.list({value}) {{ {} }}",
            encode(inner, "it", writer)
        ),
    }
}

/// Kotlin expression for a schema value.
pub(super) fn value_literal(value: &Value, ty: &Type, types: &TypeTable) -> String {
    match (value, ty) {
        (Value::Bool(value), _) => value.to_string(),
        (Value::Int(value), _) => format!("{value}L"),
        (Value::Float(value), _) => double(*value),
        (Value::Text(value), _) => string(value),
        (Value::Optional(None), _) => "null".into(),
        (Value::Optional(Some(value)), Type::Optional { inner }) => {
            value_literal(value, inner, types)
        }
        (Value::List(items), Type::List { inner }) => format!(
            "listOf<{}>({})",
            kotlin_type(inner),
            items
                .iter()
                .map(|item| value_literal(item, inner, types))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        (Value::Enum { variant, fields }, Type::Enum { name }) => {
            let def = types.enum_def(name).expect("known enum");
            let variant_def = &def.variants[*variant as usize];
            let name = value_type(name);
            if variant_def.fields.is_empty() {
                format!("{name}.{}", variant_def.name)
            } else {
                let args: Vec<_> = fields
                    .iter()
                    .zip(&variant_def.fields)
                    .map(|(value, field)| {
                        format!(
                            "{} = {}",
                            member(&field.name),
                            value_literal(value, &field.ty, types)
                        )
                    })
                    .collect();
                format!("{name}.{}({})", variant_def.name, args.join(", "))
            }
        }
        (Value::Record(values), Type::Record { name }) => {
            let def = types.record_def(name).expect("known record");
            let args: Vec<_> = values
                .iter()
                .zip(&def.fields)
                .map(|(value, field)| {
                    format!(
                        "{} = {}",
                        member(&field.name),
                        value_literal(value, &field.ty, types)
                    )
                })
                .collect();
            format!("{}({})", value_type(name), args.join(", "))
        }
        _ => "Unit".into(),
    }
}

fn default_literal(ty: &Type, types: &TypeTable) -> String {
    let value = Value::default_for(ty, types).unwrap_or(Value::Unit);
    value_literal(&value, ty, types)
}

pub(super) fn generate_enum(out: &mut Out, def: &EnumDef) {
    let name = value_type(&def.name);
    let labeled = def.variants.iter().all(|variant| variant.label.is_some());
    let supertypes = if labeled { " : BernLabeled" } else { "" };
    // Variant classes of a sealed class hide Kotlin's `String` inside it.
    let label = |out: &mut Out, prefix: &str, text: &str| {
        if labeled {
            out.open(format!("override val bernLabel: {text}"));
            out.open("get() = when (this) {");
            for variant in &def.variants {
                let label = variant.label.as_ref().unwrap();
                // A label that shows the variant's fields fills them in.
                let args: Vec<_> = label
                    .params
                    .iter()
                    .filter_map(|param| {
                        let field = variant.fields.iter().find(|field| field.name == *param)?;
                        Some(format!(
                            "{} to {}",
                            string(param),
                            argument(&member(&field.name), &field.ty)
                        ))
                    })
                    .collect();
                if args.is_empty() {
                    out.line(format!(
                        "{prefix}{} -> BernText.string({})",
                        variant.name,
                        string(&label.key)
                    ));
                } else {
                    out.line(format!(
                        "{prefix}{} -> BernText.format({}, mapOf({}))",
                        variant.name,
                        string(&label.key),
                        args.join(", ")
                    ));
                }
            }
            out.close("}");
            out.indent -= 1;
            out.line("");
        }
    };
    if def.is_unit_only() {
        out.open(format!("enum class {name}{supertypes} {{"));
        let names: Vec<_> = def.variants.iter().map(|v| v.name.clone()).collect();
        out.line(format!("{};", names.join(", ")));
        out.line("");
        label(out, "", "String");
        out.line("/// The declared variant name, used by external entries.");
        out.line("val bernName: String get() = name");
        out.line("");
        out.line("fun encode(writer: BernWriter) = writer.u32(ordinal)");
        out.line("");
        out.open("companion object {");
        out.open(format!(
            "fun decode(reader: BernReader): {name} = when (reader.u32()) {{"
        ));
        for (index, variant) in def.variants.iter().enumerate() {
            out.line(format!("{index} -> {}", variant.name));
        }
        out.line(format!(
            "else -> throw BernDecodeError({})",
            string(&format!("invalid {}", def.name))
        ));
        out.close("}");
        out.line("");
        out.line(format!(
            "fun fromBernName(name: String): {name}? = entries.firstOrNull {{ it.name == name }}"
        ));
        out.close("}");
        out.close("}");
        out.line("");
        return;
    }
    out.open(format!("sealed class {name}{supertypes} {{"));
    for variant in &def.variants {
        if variant.fields.is_empty() {
            out.line(format!("data object {} : {name}()", variant.name));
        } else {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("val {}: {}", member(&field.name), field_type(&field.ty)))
                .collect();
            out.line(format!(
                "data class {}({}) : {name}()",
                variant.name,
                fields.join(", ")
            ));
        }
    }
    out.line("");
    label(out, "is ", "kotlin.String");
    out.open("val bernName: kotlin.String");
    out.open("get() = when (this) {");
    for variant in &def.variants {
        out.line(format!("is {} -> {}", variant.name, string(&variant.name)));
    }
    out.close("}");
    out.indent -= 1;
    out.line("");
    out.open("fun encode(writer: BernWriter) {");
    out.open("when (this) {");
    for (index, variant) in def.variants.iter().enumerate() {
        out.open(format!("is {} -> {{", variant.name));
        out.line(format!("writer.u32({index})"));
        for field in &variant.fields {
            out.line(encode(&field.ty, &member(&field.name), "writer"));
        }
        out.close("}");
    }
    out.close("}");
    out.close("}");
    out.line("");
    out.open("companion object {");
    out.open(format!(
        "fun decode(reader: BernReader): {name} = when (reader.u32()) {{"
    ));
    for (index, variant) in def.variants.iter().enumerate() {
        if variant.fields.is_empty() {
            out.line(format!("{index} -> {}", variant.name));
        } else {
            let args: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("{} = {}", member(&field.name), decode(&field.ty, "reader")))
                .collect();
            out.line(format!("{index} -> {}({})", variant.name, args.join(", ")));
        }
    }
    out.line(format!(
        "else -> throw BernDecodeError({})",
        string(&format!("invalid {}", def.name))
    ));
    out.close("}");
    out.close("}");
    out.close("}");
    out.line("");
}

/// A Kotlin `BernArg` for a `tr!` argument: dates stay dates for their
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
    let name = value_type(&def.name);
    let fields: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("val {}: {}", member(&field.name), kotlin_type(&field.ty)))
        .collect();
    out.open(format!("data class {name}({}) {{", fields.join(", ")));
    if def.name == bern_protocol::format::CALENDAR_DATE {
        out.line("/// The date as an argument of a translated text.");
        out.line("val bernArg: BernArg get() = BernArg.Date(year, month, day)");
        out.line("");
    }
    out.open("fun encode(writer: BernWriter) {");
    for field in &def.fields {
        out.line(encode(&field.ty, &member(&field.name), "writer"));
    }
    out.close("}");
    out.line("");
    out.open("companion object {");
    let args: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("{} = {}", member(&field.name), decode(&field.ty, "reader")))
        .collect();
    out.line(format!(
        "fun decode(reader: BernReader): {name} = {name}({})",
        args.join(", ")
    ));
    out.close("}");
    out.close("}");
    out.line("");
}

pub(super) fn generate_engine(out: &mut Out, app: &Application) {
    let protocol = &app.protocol;
    out.line(format!(
        "/// Read-only state of the `{}` engine, mirrored from Rust.",
        protocol.namespace
    ));
    out.open("object BernEngine {");
    for field in &protocol.state {
        if !field.doc.is_empty() {
            out.line(format!("/// {}", field.doc.replace('\n', " ")));
        }
        out.line(format!(
            "var {}: {} by mutableStateOf({})",
            member(&field.name),
            kotlin_type(&field.ty),
            default_literal(&field.ty, &app.types)
        ));
        out.line("    private set");
    }
    out.line("");
    out.open("fun bernApply(field: Int, reader: BernReader) {");
    out.open("when (field) {");
    for (index, field) in protocol.state.iter().enumerate() {
        out.line(format!(
            "{index} -> {} = {}",
            member(&field.name),
            decode(&field.ty, "reader")
        ));
    }
    out.line("else -> throw BernDecodeError(\"unknown state field $field\")");
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
            .map(|param| format!("{}: {}", member(&param.name), kotlin_type(&param.ty)))
            .collect();
        params.push("bernWindow: BernWindowContext?".into());
        params.push("bernCompletion: ((BernCommandFailure?) -> Unit)? = null".into());
        out.open(format!(
            "fun {}({}) {{",
            member(&app.protocol.command_symbol(command)),
            params.join(", ")
        ));
        out.open(format!(
            "BernHost.dispatch({index}, bernWindow?.id, {{ writer ->"
        ));
        for param in &command.params {
            out.line(encode(&param.ty, &member(&param.name), "writer"));
        }
        out.close("}, completion = bernCompletion)");
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
        out.open(format!("class {class} : BernSessionState({index}) {{"));
        for (field_index, field) in session.fields.iter().enumerate() {
            let name = member(&field.name);
            let storage = format!("bern_{}", field.name);
            let ty = kotlin_type(&field.ty);
            out.line(format!(
                "private var {storage}: {ty} by mutableStateOf({})",
                value_literal(&field.initial, &field.ty, &app.types)
            ));
            out.open(format!("var {name}: {ty}"));
            out.line(format!("get() = {storage}"));
            out.open("set(value) {");
            out.line(format!(
                "if (window?.set({index}, {field_index}) {{ writer -> {} }} != true) return",
                encode(&field.ty, "value", "writer")
            ));
            out.line(format!("{storage} = value"));
            out.close("}");
            out.indent -= 1;
            out.line("");
        }
        out.open("override fun load(reader: BernReader) {");
        for field in &session.fields {
            out.line(format!(
                "bern_{} = {}",
                field.name,
                decode(&field.ty, "reader")
            ));
        }
        out.close("}");
        out.close("}");
        out.line("");
    }
}
