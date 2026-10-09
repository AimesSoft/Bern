use crate::{LocalizedText, SchemaError};
use serde::{Deserialize, Serialize};

/// Types that may cross a Bern boundary. The first version deliberately
/// limits itself to booleans, numbers, text, enums, optionals, records and
/// lists, each with a defined mapping in every target language.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Type {
    Unit,
    Bool,
    Int,
    Float,
    Text,
    Enum { name: String },
    Record { name: String },
    Optional { inner: Box<Type> },
    List { inner: Box<Type> },
}

impl Type {
    pub fn optional(inner: Type) -> Self {
        Type::Optional {
            inner: Box::new(inner),
        }
    }

    pub fn list(inner: Type) -> Self {
        Type::List {
            inner: Box::new(inner),
        }
    }

    /// Short human readable spelling used in diagnostics and help output.
    pub fn display(&self) -> String {
        match self {
            Type::Unit => "()".into(),
            Type::Bool => "bool".into(),
            Type::Int => "int".into(),
            Type::Float => "float".into(),
            Type::Text => "text".into(),
            Type::Enum { name } | Type::Record { name } => name.clone(),
            Type::Optional { inner } => format!("{}?", inner.display()),
            Type::List { inner } => format!("[{}]", inner.display()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantDef {
    pub name: String,
    /// Named payload fields. Unit variants have none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldDef>,
    /// User-visible label, required when the enum is shown in a UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<LocalizedText>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
}

impl EnumDef {
    pub fn is_unit_only(&self) -> bool {
        self.variants
            .iter()
            .all(|variant| variant.fields.is_empty())
    }

    pub fn variant_index(&self, name: &str) -> Option<usize> {
        self.variants
            .iter()
            .position(|variant| variant.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

impl RecordDef {
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|field| field.name == name)
    }
}

/// Named enum and record definitions referenced by [`Type`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeTable {
    #[serde(default)]
    pub enums: Vec<EnumDef>,
    #[serde(default)]
    pub records: Vec<RecordDef>,
}

impl TypeTable {
    pub fn enum_def(&self, name: &str) -> Option<&EnumDef> {
        self.enums.iter().find(|def| def.name == name)
    }

    pub fn record_def(&self, name: &str) -> Option<&RecordDef> {
        self.records.iter().find(|def| def.name == name)
    }

    /// Merge another table, rejecting different definitions with one name.
    pub fn merge(&mut self, other: &TypeTable) -> Result<(), SchemaError> {
        for def in &other.enums {
            match self.enum_def(&def.name) {
                Some(existing) if existing != def => {
                    return Err(SchemaError::new(format!(
                        "enum `{}` is defined twice with different shapes",
                        def.name
                    )));
                }
                Some(_) => {}
                None => self.enums.push(def.clone()),
            }
        }
        for def in &other.records {
            match self.record_def(&def.name) {
                Some(existing) if existing != def => {
                    return Err(SchemaError::new(format!(
                        "record `{}` is defined twice with different shapes",
                        def.name
                    )));
                }
                Some(_) => {}
                None => self.records.push(def.clone()),
            }
        }
        Ok(())
    }

    /// Check that every referenced name exists and that records do not
    /// contain themselves, which the first version cannot represent.
    pub fn validate(&self) -> Result<(), SchemaError> {
        let mut names = std::collections::BTreeSet::new();
        for name in self
            .enums
            .iter()
            .map(|def| &def.name)
            .chain(self.records.iter().map(|def| &def.name))
        {
            if !names.insert(name) {
                return Err(SchemaError::new(format!("type `{name}` is declared twice")));
            }
        }
        for def in &self.enums {
            if def.variants.is_empty() {
                return Err(SchemaError::new(format!(
                    "enum `{}` has no variants",
                    def.name
                )));
            }
            let mut seen = std::collections::BTreeSet::new();
            for variant in &def.variants {
                if !seen.insert(&variant.name) {
                    return Err(SchemaError::new(format!(
                        "enum `{}` repeats variant `{}`",
                        def.name, variant.name
                    )));
                }
                for field in &variant.fields {
                    self.check_type(&field.ty, &mut Vec::new())?;
                }
            }
        }
        for def in &self.records {
            self.check_type(
                &Type::Record {
                    name: def.name.clone(),
                },
                &mut Vec::new(),
            )?;
        }
        Ok(())
    }

    pub fn check_type(&self, ty: &Type, stack: &mut Vec<String>) -> Result<(), SchemaError> {
        match ty {
            Type::Unit | Type::Bool | Type::Int | Type::Float | Type::Text => Ok(()),
            Type::Enum { name } => self
                .enum_def(name)
                .map(|_| ())
                .ok_or_else(|| SchemaError::new(format!("unknown enum `{name}`"))),
            Type::Record { name } => {
                let def = self
                    .record_def(name)
                    .ok_or_else(|| SchemaError::new(format!("unknown record `{name}`")))?;
                if stack.contains(name) {
                    return Err(SchemaError::new(format!(
                        "record `{name}` contains itself; recursive data needs a dedicated design"
                    )));
                }
                stack.push(name.clone());
                for field in &def.fields {
                    self.check_type(&field.ty, stack)?;
                }
                stack.pop();
                Ok(())
            }
            Type::Optional { inner } => {
                if matches!(**inner, Type::Optional { .. }) {
                    return Err(SchemaError::new("nested optionals are not supported"));
                }
                self.check_type(inner, stack)
            }
            Type::List { inner } => self.check_type(inner, stack),
        }
    }
}
