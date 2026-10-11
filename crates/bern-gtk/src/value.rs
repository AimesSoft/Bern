//! Operations generated expressions use on schema values.
//!
//! The compiler type-checked every expression, so these helpers only see
//! the value kinds their operator allows; anything else is a generator bug
//! and yields a neutral value instead of a crash.

use crate::protocol::Value;
use std::cmp::Ordering;

/// The value at a record path, such as `item.kind`.
pub fn at(value: Value, path: &[usize]) -> Value {
    let mut value = value;
    for index in path {
        value = match value {
            Value::Record(mut fields) if *index < fields.len() => fields.swap_remove(*index),
            _ => return Value::Unit,
        };
    }
    value
}

pub fn truthy(value: &Value) -> bool {
    matches!(value, Value::Bool(true))
}

pub fn bool(value: bool) -> Value {
    Value::Bool(value)
}

pub fn not(value: Value) -> Value {
    Value::Bool(!truthy(&value))
}

pub fn neg(value: Value) -> Value {
    match value {
        Value::Int(value) => Value::Int(value.wrapping_neg()),
        Value::Float(value) => Value::Float(-value),
        other => other,
    }
}

pub fn eq(lhs: Value, rhs: Value) -> Value {
    Value::Bool(lhs == rhs)
}

pub fn ne(lhs: Value, rhs: Value) -> Value {
    Value::Bool(lhs != rhs)
}

fn order(lhs: &Value, rhs: &Value) -> Option<Ordering> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

pub fn lt(lhs: Value, rhs: Value) -> Value {
    Value::Bool(order(&lhs, &rhs) == Some(Ordering::Less))
}

pub fn le(lhs: Value, rhs: Value) -> Value {
    Value::Bool(matches!(
        order(&lhs, &rhs),
        Some(Ordering::Less | Ordering::Equal)
    ))
}

pub fn gt(lhs: Value, rhs: Value) -> Value {
    Value::Bool(order(&lhs, &rhs) == Some(Ordering::Greater))
}

pub fn ge(lhs: Value, rhs: Value) -> Value {
    Value::Bool(matches!(
        order(&lhs, &rhs),
        Some(Ordering::Greater | Ordering::Equal)
    ))
}

/// `lhs && rhs`, evaluating `rhs` only when needed.
pub fn and(lhs: Value, rhs: impl FnOnce() -> Value) -> Value {
    Value::Bool(truthy(&lhs) && truthy(&rhs()))
}

pub fn or(lhs: Value, rhs: impl FnOnce() -> Value) -> Value {
    Value::Bool(truthy(&lhs) || truthy(&rhs()))
}

pub fn text(value: &str) -> Value {
    Value::Text(value.to_string())
}

pub fn variant(index: u32, fields: Vec<Value>) -> Value {
    Value::Enum {
        variant: index,
        fields,
    }
}

/// The inner value of `Some`, or `None` for `None`.
pub fn some(value: Value) -> Option<Value> {
    match value {
        Value::Optional(Some(inner)) => Some(*inner),
        _ => None,
    }
}

/// Bern's basic, locale-independent number text, the same on every target.
pub fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Text of an integer, float or text value.
pub fn plain(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => number(*value),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

pub fn as_f64(value: &Value) -> f64 {
    match value {
        Value::Float(value) => *value,
        Value::Int(value) => *value as f64,
        _ => 0.0,
    }
}

pub fn as_text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

pub fn as_list(value: Value) -> Vec<Value> {
    match value {
        Value::List(items) => items,
        _ => Vec::new(),
    }
}

/// The variant index of an enum value.
pub fn tag(value: &Value) -> Option<u32> {
    match value {
        Value::Enum { variant, .. } => Some(*variant),
        _ => None,
    }
}

/// A field of an enum variant's payload.
pub fn payload(value: Value, field: usize) -> Value {
    match value {
        Value::Enum { mut fields, .. } if field < fields.len() => fields.swap_remove(field),
        _ => Value::Unit,
    }
}

/// The text form of a list key: integers and texts.
pub fn key_text(value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Text(text) => text.clone(),
        other => format!("{other:?}"),
    }
}
