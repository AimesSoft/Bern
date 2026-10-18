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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_paths_reach_nested_fields() {
        let item = Value::Record(vec![
            Value::Text("a".into()),
            Value::Record(vec![Value::Int(1), Value::Bool(true)]),
        ]);
        assert_eq!(at(item.clone(), &[1, 1]), Value::Bool(true));
        assert_eq!(at(item.clone(), &[0]), Value::Text("a".into()));
        assert_eq!(at(item.clone(), &[]), item);
        assert_eq!(at(item.clone(), &[2]), Value::Unit);
        assert_eq!(at(item, &[0, 0]), Value::Unit);
    }

    #[test]
    fn logic_takes_only_true_as_true() {
        assert!(truthy(&Value::Bool(true)));
        assert!(!truthy(&Value::Int(1)));
        assert_eq!(not(Value::Bool(true)), Value::Bool(false));
        assert_eq!(not(Value::Unit), Value::Bool(true));
        // The right side runs only when it decides the result.
        let unused = || -> Value { panic!("evaluated") };
        assert_eq!(and(Value::Bool(false), unused), Value::Bool(false));
        assert_eq!(or(Value::Bool(true), unused), Value::Bool(true));
        assert_eq!(and(bool(true), || bool(true)), Value::Bool(true));
        assert_eq!(or(bool(false), || bool(false)), Value::Bool(false));
    }

    #[test]
    fn numbers_and_texts_order() {
        assert_eq!(lt(Value::Int(1), Value::Int(2)), Value::Bool(true));
        assert_eq!(le(Value::Int(2), Value::Int(2)), Value::Bool(true));
        assert_eq!(gt(Value::Float(2.5), Value::Float(2.0)), Value::Bool(true));
        assert_eq!(ge(Value::Float(2.0), Value::Float(2.5)), Value::Bool(false));
        assert_eq!(lt(text("apple"), text("banana")), Value::Bool(true));
        // NaN and values without an order compare false every way.
        for compare in [lt, le, gt, ge] {
            assert_eq!(
                compare(Value::Float(f64::NAN), Value::Float(1.0)),
                Value::Bool(false)
            );
            assert_eq!(
                compare(Value::Bool(true), Value::Int(1)),
                Value::Bool(false)
            );
        }
    }

    #[test]
    fn equality_is_structural() {
        let list = |items: &[i64]| Value::List(items.iter().copied().map(Value::Int).collect());
        assert_eq!(eq(list(&[1, 2]), list(&[1, 2])), Value::Bool(true));
        assert_eq!(ne(list(&[1, 2]), list(&[2, 1])), Value::Bool(true));
        assert_eq!(
            eq(variant(1, vec![text("x")]), variant(1, vec![text("y")])),
            Value::Bool(false)
        );
    }

    #[test]
    fn negation_wraps_and_keeps_other_values() {
        assert_eq!(neg(Value::Int(5)), Value::Int(-5));
        assert_eq!(neg(Value::Int(i64::MIN)), Value::Int(i64::MIN));
        assert_eq!(neg(Value::Float(1.5)), Value::Float(-1.5));
        assert_eq!(neg(text("a")), text("a"));
    }

    #[test]
    fn numbers_print_without_needless_decimals() {
        assert_eq!(number(3.0), "3");
        assert_eq!(number(-2.0), "-2");
        assert_eq!(number(-0.0), "0");
        assert_eq!(number(2.5), "2.5");
        assert_eq!(number(0.1), "0.1");
        assert_eq!(number(1e20), "100000000000000000000");
        assert_eq!(plain(&Value::Float(4.0)), "4");
        assert_eq!(plain(&Value::Int(-7)), "-7");
        assert_eq!(plain(&Value::Bool(true)), "true");
        assert_eq!(plain(&text("é")), "é");
        assert_eq!(plain(&Value::Unit), "");
    }

    #[test]
    fn conversions_give_neutral_values_for_other_kinds() {
        assert_eq!(as_f64(&Value::Int(2)), 2.0);
        assert_eq!(as_f64(&Value::Float(0.5)), 0.5);
        assert_eq!(as_f64(&text("2")), 0.0);
        assert_eq!(as_text(&text("hi")), "hi");
        assert_eq!(as_text(&Value::Int(1)), "");
        assert_eq!(as_list(Value::List(vec![Value::Unit])), vec![Value::Unit]);
        assert_eq!(as_list(Value::Unit), Vec::<Value>::new());
        assert_eq!(some(Value::some(Value::Int(1))), Some(Value::Int(1)));
        assert_eq!(some(Value::none()), None);
        assert_eq!(some(Value::Int(1)), None);
    }

    #[test]
    fn enum_values_give_their_variant_and_payload() {
        let value = variant(2, vec![Value::Int(10), text("ten")]);
        assert_eq!(tag(&value), Some(2));
        assert_eq!(tag(&Value::Int(2)), None);
        assert_eq!(payload(value.clone(), 1), text("ten"));
        assert_eq!(payload(value, 2), Value::Unit);
        assert_eq!(payload(Value::Int(1), 0), Value::Unit);
    }

    #[test]
    fn list_keys_are_texts() {
        assert_eq!(key_text(&Value::Int(42)), "42");
        assert_eq!(key_text(&text("id-1")), "id-1");
    }
}
