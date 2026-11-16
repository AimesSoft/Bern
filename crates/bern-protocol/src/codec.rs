//! Compact, schema-directed binary layout for in-process bridges.
//!
//! Both sides know the schema, so the layout carries no field names or type
//! tags: bool = u8, int = i64 LE, float = f64 LE, text = u32 LE length + UTF-8,
//! enum = u32 variant + payload fields, optional = u8 flag + value,
//! record = fields in order, list = u32 count + items. It is not used on
//! control channels, which keep the JSON encoding.

use crate::{StateField, Type, TypeTable, Value};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecError(pub String);

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodecError {}

#[derive(Default)]
pub struct Writer {
    pub bytes: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }
    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    pub fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    pub fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    pub fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    pub fn text(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes.extend_from_slice(value.as_bytes());
    }
}

pub struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CodecError> {
        let end = self
            .position
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| CodecError("unexpected end of buffer".into()))?;
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Result<i64, CodecError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f64(&mut self) -> Result<f64, CodecError> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn text(&mut self) -> Result<String, CodecError> {
        let length = self.u32()? as usize;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| CodecError("text is not UTF-8".into()))
    }
    pub fn is_finished(&self) -> bool {
        self.position == self.bytes.len()
    }
}

/// Encode a value that has already been checked against `ty`.
pub fn encode(value: &Value, ty: &Type, types: &TypeTable, writer: &mut Writer) {
    match (value, ty) {
        (Value::Unit, _) => {}
        (Value::Bool(value), _) => writer.u8(u8::from(*value)),
        (Value::Int(value), _) => writer.i64(*value),
        (Value::Float(value), _) => writer.f64(*value),
        (Value::Text(value), _) => writer.text(value),
        (Value::Enum { variant, fields }, Type::Enum { name }) => {
            writer.u32(*variant);
            let def = types.enum_def(name).expect("checked enum");
            for (field, def) in fields.iter().zip(&def.variants[*variant as usize].fields) {
                encode(field, &def.ty, types, writer);
            }
        }
        (Value::Optional(value), Type::Optional { inner }) => match value {
            None => writer.u8(0),
            Some(value) => {
                writer.u8(1);
                encode(value, inner, types, writer);
            }
        },
        (Value::Record(fields), Type::Record { name }) => {
            let def = types.record_def(name).expect("checked record");
            for (field, def) in fields.iter().zip(&def.fields) {
                encode(field, &def.ty, types, writer);
            }
        }
        (Value::List(items), Type::List { inner }) => {
            writer.u32(items.len() as u32);
            for item in items {
                encode(item, inner, types, writer);
            }
        }
        _ => unreachable!("encode called with an unchecked value"),
    }
}

/// Decode and validate a value of type `ty`.
pub fn decode(ty: &Type, types: &TypeTable, reader: &mut Reader<'_>) -> Result<Value, CodecError> {
    Ok(match ty {
        Type::Unit => Value::Unit,
        Type::Bool => match reader.u8()? {
            0 => Value::Bool(false),
            1 => Value::Bool(true),
            other => return Err(CodecError(format!("invalid bool byte {other}"))),
        },
        Type::Int => Value::Int(reader.i64()?),
        Type::Float => {
            let value = reader.f64()?;
            if !value.is_finite() {
                return Err(CodecError("float values must be finite".into()));
            }
            Value::Float(value)
        }
        Type::Text => Value::Text(reader.text()?),
        Type::Enum { name } => {
            let def = types
                .enum_def(name)
                .ok_or_else(|| CodecError(format!("unknown enum `{name}`")))?;
            let variant = reader.u32()?;
            let variant_def = def
                .variants
                .get(variant as usize)
                .ok_or_else(|| CodecError(format!("`{name}` has no variant #{variant}")))?;
            let fields = variant_def
                .fields
                .iter()
                .map(|field| decode(&field.ty, types, reader))
                .collect::<Result<_, _>>()?;
            Value::Enum { variant, fields }
        }
        Type::Optional { inner } => match reader.u8()? {
            0 => Value::Optional(None),
            1 => Value::some(decode(inner, types, reader)?),
            other => return Err(CodecError(format!("invalid optional flag {other}"))),
        },
        Type::Record { name } => {
            let def = types
                .record_def(name)
                .ok_or_else(|| CodecError(format!("unknown record `{name}`")))?;
            Value::Record(
                def.fields
                    .iter()
                    .map(|field| decode(&field.ty, types, reader))
                    .collect::<Result<_, _>>()?,
            )
        }
        Type::List { inner } => {
            let count = reader.u32()? as usize;
            let mut items = Vec::with_capacity(count.min(4096));
            for _ in 0..count {
                items.push(decode(inner, types, reader)?);
            }
            Value::List(items)
        }
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ItemKey<'a> {
    Int(i64),
    Text(&'a str),
}

fn item_key(item: &Value, key_index: usize) -> Option<ItemKey<'_>> {
    let Value::Record(fields) = item else {
        return None;
    };
    match fields.get(key_index)? {
        Value::Int(id) => Some(ItemKey::Int(*id)),
        Value::Text(id) => Some(ItemKey::Text(id.as_str())),
        _ => None,
    }
}

enum ListSegment {
    Copy { start: usize, len: usize },
    Items { start: usize, len: usize },
}

fn build_keyed_patch(
    prev_items: &[Value],
    next_items: &[Value],
    key_index: usize,
) -> Option<Vec<ListSegment>> {
    if prev_items.is_empty() || next_items.is_empty() {
        return None;
    }
    let mut by_key = HashMap::with_capacity(prev_items.len());
    for (index, item) in prev_items.iter().enumerate() {
        let key = item_key(item, key_index)?;
        if by_key.insert(key, index).is_some() {
            return None;
        }
    }
    let mut segments: Vec<ListSegment> = Vec::new();
    let mut copied = 0usize;
    for (next_index, item) in next_items.iter().enumerate() {
        let unchanged = item_key(item, key_index)
            .and_then(|key| by_key.get(&key).copied())
            .filter(|&old_index| &prev_items[old_index] == item);
        if let Some(old_index) = unchanged {
            copied += 1;
            match segments.last_mut() {
                Some(ListSegment::Copy { start, len }) if *start + *len == old_index => {
                    *len += 1;
                }
                _ => segments.push(ListSegment::Copy {
                    start: old_index,
                    len: 1,
                }),
            }
        } else {
            match segments.last_mut() {
                Some(ListSegment::Items { start, len }) if *start + *len == next_index => {
                    *len += 1;
                }
                _ => segments.push(ListSegment::Items {
                    start: next_index,
                    len: 1,
                }),
            }
        }
    }
    (copied > 0 && segments.len() <= copied).then_some(segments)
}

/// Encode a state field update. Fields without `#[keyed]` use [`encode`];
/// keyed record lists emit either `0` followed by the full list or `1`
/// followed by `Copy`/`Items` segments referencing `previous`.
pub fn encode_state_field(
    previous: Option<&Value>,
    value: &Value,
    field: &StateField,
    types: &TypeTable,
    writer: &mut Writer,
) {
    let Some(key_name) = &field.keyed else {
        encode(value, &field.ty, types, writer);
        return;
    };
    let (Type::List { inner }, Value::List(next_items)) = (&field.ty, value) else {
        writer.u8(0);
        encode(value, &field.ty, types, writer);
        return;
    };
    let key_index = match inner.as_ref() {
        Type::Record { name } => types
            .record_def(name)
            .and_then(|def| def.field_index(key_name)),
        _ => None,
    };
    if let (Some(Value::List(prev_items)), Some(key_index)) = (previous, key_index)
        && let Some(segments) = build_keyed_patch(prev_items, next_items, key_index)
    {
        writer.u8(1);
        writer.u32(next_items.len() as u32);
        writer.u32(segments.len() as u32);
        for segment in segments {
            match segment {
                ListSegment::Copy { start, len } => {
                    writer.u8(0);
                    writer.u32(start as u32);
                    writer.u32(len as u32);
                }
                ListSegment::Items { start, len } => {
                    writer.u8(1);
                    writer.u32(len as u32);
                    for item in &next_items[start..start + len] {
                        encode(item, inner, types, writer);
                    }
                }
            }
        }
        return;
    }
    writer.u8(0);
    encode(value, &field.ty, types, writer);
}

/// Decode a state field encoded by [`encode_state_field`].
pub fn decode_state_field(
    previous: Option<&Value>,
    field: &StateField,
    types: &TypeTable,
    reader: &mut Reader<'_>,
) -> Result<Value, CodecError> {
    if field.keyed.is_none() {
        return decode(&field.ty, types, reader);
    }
    match reader.u8()? {
        0 => decode(&field.ty, types, reader),
        1 => {
            let Type::List { inner } = &field.ty else {
                return Err(CodecError("keyed patch on non-list field".into()));
            };
            let Some(Value::List(prev_items)) = previous else {
                return Err(CodecError(
                    "keyed list patch without a previous list".into(),
                ));
            };
            let total = reader.u32()? as usize;
            let segment_count = reader.u32()? as usize;
            let mut out = Vec::with_capacity(total.min(4096));
            for _ in 0..segment_count {
                match reader.u8()? {
                    0 => {
                        let start = reader.u32()? as usize;
                        let len = reader.u32()? as usize;
                        let end = start
                            .checked_add(len)
                            .filter(|end| *end <= prev_items.len())
                            .ok_or_else(|| {
                                CodecError("keyed list copy segment out of bounds".into())
                            })?;
                        out.extend_from_slice(&prev_items[start..end]);
                    }
                    1 => {
                        let len = reader.u32()? as usize;
                        for _ in 0..len {
                            out.push(decode(inner, types, reader)?);
                        }
                    }
                    other => {
                        return Err(CodecError(format!(
                            "invalid keyed list segment tag {other}"
                        )));
                    }
                }
            }
            if out.len() != total {
                return Err(CodecError(format!(
                    "keyed list patch produced {} items, expected {total}",
                    out.len()
                )));
            }
            Ok(Value::List(out))
        }
        other => Err(CodecError(format!("invalid keyed list mode {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FieldDef, RecordDef};

    #[test]
    fn round_trip_preserves_values() {
        let types = TypeTable {
            enums: vec![],
            records: vec![RecordDef {
                name: "Row".into(),
                fields: vec![
                    FieldDef {
                        name: "title".into(),
                        ty: Type::Text,
                        doc: String::new(),
                        content: false,
                    },
                    FieldDef {
                        name: "score".into(),
                        ty: Type::optional(Type::Float),
                        doc: String::new(),
                        content: false,
                    },
                ],
            }],
        };
        let ty = Type::list(Type::Record { name: "Row".into() });
        let value = Value::List(vec![
            Value::Record(vec![
                Value::Text("你好".into()),
                Value::some(Value::Float(1.5)),
            ]),
            Value::Record(vec![Value::Text(String::new()), Value::none()]),
        ]);
        value.check(&ty, &types).unwrap();
        let mut writer = Writer::new();
        encode(&value, &ty, &types, &mut writer);
        let mut reader = Reader::new(&writer.bytes);
        assert_eq!(decode(&ty, &types, &mut reader).unwrap(), value);
        assert!(reader.is_finished());
        let mut short = Reader::new(&writer.bytes[..writer.bytes.len() - 1]);
        assert!(decode(&ty, &types, &mut short).is_err());
    }

    #[test]
    fn keyed_list_patches_send_only_changed_items() {
        let types = TypeTable {
            enums: vec![],
            records: vec![RecordDef {
                name: "Song".into(),
                fields: vec![
                    FieldDef {
                        name: "id".into(),
                        ty: Type::Int,
                        doc: String::new(),
                        content: false,
                    },
                    FieldDef {
                        name: "title".into(),
                        ty: Type::Text,
                        doc: String::new(),
                        content: true,
                    },
                    FieldDef {
                        name: "liked".into(),
                        ty: Type::Bool,
                        doc: String::new(),
                        content: false,
                    },
                ],
            }],
        };
        let field = StateField {
            name: "songs".into(),
            ty: Type::list(Type::Record {
                name: "Song".into(),
            }),
            doc: String::new(),
            content: false,
            stream: false,
            keyed: Some("id".into()),
        };
        let song = |id: i64, liked: bool| {
            Value::Record(vec![
                Value::Int(id),
                Value::Text(format!("Track #{id} with a long title")),
                Value::Bool(liked),
            ])
        };
        let prev = Value::List((0..200).map(|i| song(i, false)).collect());
        let mut full = Writer::new();
        encode_state_field(None, &prev, &field, &types, &mut full);
        assert_eq!(full.bytes[0], 0);
        let mut reader = Reader::new(&full.bytes);
        assert_eq!(
            decode_state_field(None, &field, &types, &mut reader).unwrap(),
            prev
        );

        let mut next_items: Vec<Value> = (0..200).map(|i| song(i, i == 77)).collect();
        next_items.push(song(200, true));
        let next = Value::List(next_items);
        let mut patch = Writer::new();
        encode_state_field(Some(&prev), &next, &field, &types, &mut patch);
        assert_eq!(patch.bytes[0], 1, "expected incremental patch mode");
        assert!(
            patch.bytes.len() * 10 < full.bytes.len(),
            "patch ({} B) should be much smaller than full list ({} B)",
            patch.bytes.len(),
            full.bytes.len()
        );
        let mut reader = Reader::new(&patch.bytes);
        assert_eq!(
            decode_state_field(Some(&prev), &field, &types, &mut reader).unwrap(),
            next
        );
        assert!(reader.is_finished());
    }
}
