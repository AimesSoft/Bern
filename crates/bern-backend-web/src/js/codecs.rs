//! Codecs of the application's types, and JavaScript literals of its
//! values.

use super::{Out, number, string};
use bern_protocol::{EnumDef, RecordDef, Type, TypeTable, Value};

/// Whether `===` compares values of this type correctly.
pub(super) fn is_scalar(ty: &Type, types: &TypeTable) -> bool {
    match ty {
        Type::Unit | Type::Bool | Type::Int | Type::Float | Type::Text => true,
        Type::Enum { name } => types.enum_def(name).is_some_and(EnumDef::is_unit_only),
        _ => false,
    }
}

/// Decoding expression for a value of `ty` read from `r`.
pub(super) fn decode(ty: &Type) -> String {
    match ty {
        Type::Unit => "null".into(),
        Type::Bool => "r.bool()".into(),
        Type::Int => "r.i64()".into(),
        Type::Float => "r.f64()".into(),
        Type::Text => "r.text()".into(),
        Type::Enum { name } | Type::Record { name } => format!("T.{name}.d(r)"),
        Type::Optional { inner } => format!("B.readOptional(r, (r) => {})", decode(inner)),
        Type::List { inner } => format!("B.readList(r, (r) => {})", decode(inner)),
    }
}

/// Statement encoding the value `v` of `ty` into `w`.
pub(super) fn encode(ty: &Type, value: &str) -> String {
    match ty {
        Type::Unit => String::new(),
        Type::Bool => format!("w.u8({value} ? 1 : 0)"),
        Type::Int => format!("w.i64({value})"),
        Type::Float => format!("w.f64({value})"),
        Type::Text => format!("w.text({value})"),
        Type::Enum { name } | Type::Record { name } => format!("T.{name}.e(w, {value})"),
        Type::Optional { inner } => format!(
            "B.writeOptional(w, {value}, (w, v) => {{ {} }})",
            encode(inner, "v")
        ),
        Type::List { inner } => format!(
            "B.writeList(w, {value}, (w, v) => {{ {} }})",
            encode(inner, "v")
        ),
    }
}

pub(super) fn codec(ty: &Type) -> String {
    format!(
        "[(r) => {}, (w, v) => {{ {} }}]",
        decode(ty),
        encode(ty, "v")
    )
}

/// JavaScript literal of a schema value.
pub(super) fn literal(value: &Value, ty: &Type, types: &TypeTable) -> String {
    match (value, ty) {
        (Value::Unit, _) => "null".into(),
        (Value::Bool(value), _) => value.to_string(),
        (Value::Int(value), _) => format!("{value}n"),
        (Value::Float(value), _) => number(*value),
        (Value::Text(value), _) => string(value),
        (Value::Optional(None), _) => "null".into(),
        (Value::Optional(Some(value)), Type::Optional { inner }) => literal(value, inner, types),
        (Value::List(items), Type::List { inner }) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| literal(item, inner, types))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        (Value::Enum { variant, fields }, Type::Enum { name }) => {
            let def = types.enum_def(name).expect("known enum");
            if def.is_unit_only() {
                variant.to_string()
            } else {
                let values: Vec<_> = fields
                    .iter()
                    .zip(&def.variants[*variant as usize].fields)
                    .map(|(value, field)| literal(value, &field.ty, types))
                    .collect();
                format!("({{ tag: {variant}, fields: [{}] }})", values.join(", "))
            }
        }
        (Value::Record(values), Type::Record { name }) => {
            let def = types.record_def(name).expect("known record");
            let fields: Vec<_> = values
                .iter()
                .zip(&def.fields)
                .map(|(value, field)| {
                    format!("{}: {}", field.name, literal(value, &field.ty, types))
                })
                .collect();
            format!("({{ {} }})", fields.join(", "))
        }
        _ => "null".into(),
    }
}

pub(super) fn generate_enum(out: &mut Out, def: &EnumDef) {
    let name = &def.name;
    if def.is_unit_only() {
        out.open(format!("T.{name} = {{"));
        out.open("d(r) {");
        out.line("const v = r.u32();");
        out.line(format!(
            "if (v >= {}) throw new B.DecodeError({});",
            def.variants.len(),
            string(&format!("invalid {name}"))
        ));
        out.line("return v;");
        out.close("},");
        out.line("e(w, v) { w.u32(v); },");
        out.close("};");
    } else {
        out.open(format!("T.{name} = {{"));
        out.open("d(r) {");
        out.open("switch (r.u32()) {");
        for (index, variant) in def.variants.iter().enumerate() {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .map(|field| decode(&field.ty))
                .collect();
            out.line(format!(
                "case {index}: return {{ tag: {index}, fields: [{}] }};",
                fields.join(", ")
            ));
        }
        out.line(format!(
            "default: throw new B.DecodeError({});",
            string(&format!("invalid {name}"))
        ));
        out.close("}");
        out.close("},");
        out.open("e(w, v) {");
        out.line("w.u32(v.tag);");
        out.open("switch (v.tag) {");
        for (index, variant) in def.variants.iter().enumerate() {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .enumerate()
                .map(|(field_index, field)| {
                    format!(
                        "{};",
                        encode(&field.ty, &format!("v.fields[{field_index}]"))
                    )
                })
                .collect();
            out.line(format!("case {index}: {} break;", fields.join(" ")));
        }
        out.close("}");
        out.close("},");
        out.close("};");
    }
    let names: Vec<_> = def
        .variants
        .iter()
        .map(|variant| string(&variant.name))
        .collect();
    out.line(format!("N.{name} = [{}];", names.join(", ")));
    if def.variants.iter().all(|variant| variant.label.is_some()) {
        let keys: Vec<_> = def
            .variants
            .iter()
            .map(|variant| string(&variant.label.as_ref().expect("label").key))
            .collect();
        if def.variants.iter().all(|variant| {
            variant
                .label
                .as_ref()
                .is_some_and(|label| label.params.is_empty())
        }) {
            let index = if def.is_unit_only() { "v" } else { "v.tag" };
            out.line(format!(
                "L.{name} = (v) => B.tr([{}][{index}]);",
                keys.join(", ")
            ));
        } else {
            // Labels that show the variant's fields fill them in.
            out.open(format!("L.{name} = (v) => {{"));
            out.open("switch (v.tag) {");
            for (index, variant) in def.variants.iter().enumerate() {
                let label = variant.label.as_ref().expect("label");
                if label.params.is_empty() {
                    out.line(format!(
                        "case {index}: return B.tr({});",
                        string(&label.key)
                    ));
                    continue;
                }
                let args: Vec<_> = label
                    .params
                    .iter()
                    .filter_map(|param| {
                        let at = variant
                            .fields
                            .iter()
                            .position(|field| field.name == *param)?;
                        let value = format!("v.fields[{at}]");
                        let shown = match &variant.fields[at].ty {
                            Type::Int => format!("B.int({value})"),
                            Type::Float => format!("B.num({value})"),
                            Type::Enum { name } => format!("L.{name}({value})"),
                            _ => value,
                        };
                        Some(format!("{}: {shown}", string(param)))
                    })
                    .collect();
                out.line(format!(
                    "case {index}: return B.fmt({}, {{ {} }});",
                    string(&label.key),
                    args.join(", ")
                ));
            }
            out.close("}");
            out.line("return \"\";");
            out.close("};");
        }
    }
    out.line("");
}

pub(super) fn generate_record(out: &mut Out, def: &RecordDef) {
    let name = &def.name;
    out.open(format!("T.{name} = {{"));
    let fields: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("{}: {}", field.name, decode(&field.ty)))
        .collect();
    out.line(format!("d(r) {{ return {{ {} }}; }},", fields.join(", ")));
    let encoders: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("{};", encode(&field.ty, &format!("v.{}", field.name))))
        .collect();
    out.line(format!("e(w, v) {{ {} }},", encoders.join(" ")));
    out.close("};");
    out.line("");
}
