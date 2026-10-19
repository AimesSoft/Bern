use crate::{SchemaError, Type, TypeTable};
use serde::{Deserialize, Serialize};

/// A schema-checked value. Records and enum payloads keep field order from
/// their definition, so encoders never need field names on hot paths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Enum { variant: u32, fields: Vec<Value> },
    Optional(Option<Box<Value>>),
    Record(Vec<Value>),
    List(Vec<Value>),
}

impl Value {
    pub fn unit_variant(index: u32) -> Self {
        Value::Enum {
            variant: index,
            fields: Vec::new(),
        }
    }

    pub fn none() -> Self {
        Value::Optional(None)
    }

    pub fn some(value: Value) -> Self {
        Value::Optional(Some(Box::new(value)))
    }

    /// Verify that this value matches `ty`. Floats must be finite so every
    /// encoding can represent them.
    pub fn check(&self, ty: &Type, types: &TypeTable) -> Result<(), SchemaError> {
        match (self, ty) {
            (Value::Unit, Type::Unit)
            | (Value::Bool(_), Type::Bool)
            | (Value::Int(_), Type::Int)
            | (Value::Text(_), Type::Text) => Ok(()),
            (Value::Float(value), Type::Float) => {
                if value.is_finite() {
                    Ok(())
                } else {
                    Err(SchemaError::new("float values must be finite"))
                }
            }
            (Value::Enum { variant, fields }, Type::Enum { name }) => {
                let def = types
                    .enum_def(name)
                    .ok_or_else(|| SchemaError::new(format!("unknown enum `{name}`")))?;
                let variant_def = def.variants.get(*variant as usize).ok_or_else(|| {
                    SchemaError::new(format!("`{name}` has no variant #{variant}"))
                })?;
                if fields.len() != variant_def.fields.len() {
                    return Err(SchemaError::new(format!(
                        "`{name}::{}` expects {} fields",
                        variant_def.name,
                        variant_def.fields.len()
                    )));
                }
                for (value, field) in fields.iter().zip(&variant_def.fields) {
                    value.check(&field.ty, types).map_err(|error| {
                        SchemaError::new(format!(
                            "{name}::{}.{}: {}",
                            variant_def.name, field.name, error.message
                        ))
                    })?;
                }
                Ok(())
            }
            (Value::Optional(None), Type::Optional { .. }) => Ok(()),
            (Value::Optional(Some(value)), Type::Optional { inner }) => value.check(inner, types),
            (Value::Record(values), Type::Record { name }) => {
                let def = types
                    .record_def(name)
                    .ok_or_else(|| SchemaError::new(format!("unknown record `{name}`")))?;
                if values.len() != def.fields.len() {
                    return Err(SchemaError::new(format!(
                        "record `{name}` expects {} fields",
                        def.fields.len()
                    )));
                }
                for (value, field) in values.iter().zip(&def.fields) {
                    value.check(&field.ty, types).map_err(|error| {
                        SchemaError::new(format!("{name}.{}: {}", field.name, error.message))
                    })?;
                }
                Ok(())
            }
            (Value::List(values), Type::List { inner }) => {
                for (index, value) in values.iter().enumerate() {
                    value.check(inner, types).map_err(|error| {
                        SchemaError::new(format!("[{index}]: {}", error.message))
                    })?;
                }
                Ok(())
            }
            (value, ty) => Err(SchemaError::new(format!(
                "expected {}, found {}",
                ty.display(),
                value.kind_name()
            ))),
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Unit => "unit",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Text(_) => "text",
            Value::Enum { .. } => "enum",
            Value::Optional(_) => "optional",
            Value::Record(_) => "record",
            Value::List(_) => "list",
        }
    }

    /// The type's zero value, used when a declaration omits an initial value.
    pub fn default_for(ty: &Type, types: &TypeTable) -> Result<Value, SchemaError> {
        Ok(match ty {
            Type::Unit => Value::Unit,
            Type::Bool => Value::Bool(false),
            Type::Int => Value::Int(0),
            Type::Float => Value::Float(0.0),
            Type::Text => Value::Text(String::new()),
            Type::Optional { .. } => Value::Optional(None),
            Type::List { .. } => Value::List(Vec::new()),
            Type::Enum { name } => {
                let def = types
                    .enum_def(name)
                    .ok_or_else(|| SchemaError::new(format!("unknown enum `{name}`")))?;
                let first = &def.variants[0];
                let fields = first
                    .fields
                    .iter()
                    .map(|field| Value::default_for(&field.ty, types))
                    .collect::<Result<_, _>>()?;
                Value::Enum { variant: 0, fields }
            }
            Type::Record { name } => {
                let def = types
                    .record_def(name)
                    .ok_or_else(|| SchemaError::new(format!("unknown record `{name}`")))?;
                Value::Record(
                    def.fields
                        .iter()
                        .map(|field| Value::default_for(&field.ty, types))
                        .collect::<Result<_, _>>()?,
                )
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnumDef, FieldDef, RecordDef, VariantDef};

    fn table() -> TypeTable {
        TypeTable {
            enums: vec![EnumDef {
                name: "Kind".into(),
                variants: vec![
                    VariantDef {
                        name: "A".into(),
                        fields: vec![],
                        label: None,
                    },
                    VariantDef {
                        name: "B".into(),
                        fields: vec![FieldDef {
                            name: "id".into(),
                            ty: Type::Int,
                            doc: String::new(),
                            content: false,
                        }],
                        label: None,
                    },
                ],
            }],
            records: vec![RecordDef {
                name: "Item".into(),
                fields: vec![
                    FieldDef {
                        name: "id".into(),
                        ty: Type::Int,
                        doc: String::new(),
                        content: false,
                    },
                    FieldDef {
                        name: "kind".into(),
                        ty: Type::Enum {
                            name: "Kind".into(),
                        },
                        doc: String::new(),
                        content: false,
                    },
                ],
            }],
        }
    }

    #[test]
    fn values_are_checked_against_nested_schema() {
        let types = table();
        types.validate().unwrap();
        let item = Type::Record {
            name: "Item".into(),
        };
        let good = Value::Record(vec![
            Value::Int(1),
            Value::Enum {
                variant: 1,
                fields: vec![Value::Int(3)],
            },
        ]);
        good.check(&item, &types).unwrap();
        let bad = Value::Record(vec![Value::Int(1), Value::unit_variant(1)]);
        assert!(bad.check(&item, &types).is_err());
        assert!(Value::Float(f64::NAN).check(&Type::Float, &types).is_err());
        assert_eq!(
            Value::default_for(&item, &types).unwrap(),
            Value::Record(vec![Value::Int(0), Value::unit_variant(0)])
        );
    }

    #[test]
    fn recursive_records_are_rejected() {
        let types = TypeTable {
            enums: vec![],
            records: vec![RecordDef {
                name: "Node".into(),
                fields: vec![FieldDef {
                    name: "child".into(),
                    ty: Type::optional(Type::Record {
                        name: "Node".into(),
                    }),
                    doc: String::new(),
                    content: false,
                }],
            }],
        };
        assert!(types.validate().is_err());
    }
}
