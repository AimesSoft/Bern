//! Typed references used by declarations.
//!
//! A `Binding<T>` names read-only state of type `T`; a `SessionField<T>`
//! names writable session state. They carry no data: `rustc` uses them to
//! check that every binding in a `ui!` declaration has the right type and
//! that controls only write to session state.

use bern_protocol::{SchemaError, Value};
use std::marker::PhantomData;

/// A value type that can cross a Bern boundary.
pub trait BernValue: Sized + 'static {
    /// The reference type used for this value inside declarations.
    type Ref: Clone;
    fn reference() -> Self::Ref;
    fn to_value(&self) -> Value;
    fn from_value(value: &Value) -> Result<Self, SchemaError>;
}

/// Implemented for every `#[bern::value]` enum.
pub trait BernEnum: BernValue {}

/// Implemented for enums whose variants all declare `#[label(...)]`, which
/// makes them displayable and usable in pickers.
pub trait LabeledEnum: BernEnum {}

/// Read-only reference to state of type `T`.
pub struct Binding<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> Binding<T> {
    pub const fn declared() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Binding<T> {}

/// Writable reference to a session field of type `T`.
pub struct SessionField<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> SessionField<T> {
    pub const fn declared() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for SessionField<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SessionField<T> {}

/// References that can be read as a value of type `T`.
pub trait Readable<T> {}

impl<T> Readable<T> for Binding<T> {}
impl<T> Readable<T> for SessionField<T> {}

/// Values and references that have a localized or textual presentation.
pub trait Displayable {}

impl Displayable for String {}
impl Displayable for i64 {}
impl Displayable for f64 {}
impl Displayable for Binding<String> {}
impl Displayable for Binding<i64> {}
impl Displayable for Binding<f64> {}
impl Displayable for SessionField<String> {}
impl Displayable for SessionField<i64> {}
impl Displayable for SessionField<f64> {}
impl<E: LabeledEnum> Displayable for Binding<E> {}
impl<E: LabeledEnum> Displayable for SessionField<E> {}

/// Types usable as a stable list key.
pub trait ListKey {}

impl ListKey for Binding<i64> {}
impl ListKey for Binding<String> {}

/// Implemented by `#[bern::session]` structs.
pub trait SessionState {
    type Handle: Copy;
    fn handle() -> Self::Handle;
}

/// How a component parameter of this type is passed.
pub trait Prop {
    type Ref;
}

impl<T: BernValue> Prop for T {
    type Ref = T::Ref;
}

/// Convert a reference to the form a component parameter expects.
pub trait ToRef {
    type Ref;
    fn to_ref(&self) -> Self::Ref;
}

impl<T: BernValue> ToRef for Binding<T> {
    type Ref = T::Ref;
    fn to_ref(&self) -> T::Ref {
        T::reference()
    }
}

impl<T: BernValue> ToRef for SessionField<T> {
    type Ref = T::Ref;
    fn to_ref(&self) -> T::Ref {
        T::reference()
    }
}

/// A command reference produced by a typed client method.
#[must_use = "a command reference only has meaning as an action"]
pub struct CommandRef {
    _private: (),
}

impl CommandRef {
    pub const fn declared() -> Self {
        Self { _private: () }
    }
}

/// A window kind in the generated window handle.
#[derive(Clone, Copy)]
pub struct WindowRef {
    _private: (),
}

impl WindowRef {
    pub const fn declared() -> Self {
        Self { _private: () }
    }
    /// Open the window, or activate its existing instance.
    pub fn open(&self) -> CommandRef {
        CommandRef::declared()
    }
    /// Request closing the window instance.
    pub fn close(&self) -> CommandRef {
        CommandRef::declared()
    }
    /// Bring the window instance to the front.
    pub fn activate(&self) -> CommandRef {
        CommandRef::declared()
    }
}

macro_rules! scalar {
    ($ty:ty, $variant:ident, $name:literal) => {
        impl BernValue for $ty {
            type Ref = Binding<$ty>;
            fn reference() -> Self::Ref {
                Binding::declared()
            }
            fn to_value(&self) -> Value {
                Value::$variant(self.clone())
            }
            fn from_value(value: &Value) -> Result<Self, SchemaError> {
                match value {
                    Value::$variant(value) => Ok(value.clone()),
                    other => Err(SchemaError::new(format!(
                        concat!("expected ", $name, ", found {}"),
                        other.kind_name()
                    ))),
                }
            }
        }
    };
}

scalar!(bool, Bool, "bool");
scalar!(i64, Int, "int");
scalar!(f64, Float, "float");
scalar!(String, Text, "text");

impl BernValue for () {
    type Ref = Binding<()>;
    fn reference() -> Self::Ref {
        Binding::declared()
    }
    fn to_value(&self) -> Value {
        Value::Unit
    }
    fn from_value(value: &Value) -> Result<Self, SchemaError> {
        match value {
            Value::Unit => Ok(()),
            other => Err(SchemaError::new(format!(
                "expected unit, found {}",
                other.kind_name()
            ))),
        }
    }
}

impl<T: BernValue> BernValue for Option<T> {
    type Ref = Binding<Option<T>>;
    fn reference() -> Self::Ref {
        Binding::declared()
    }
    fn to_value(&self) -> Value {
        match self {
            None => Value::Optional(None),
            Some(value) => Value::some(value.to_value()),
        }
    }
    fn from_value(value: &Value) -> Result<Self, SchemaError> {
        match value {
            Value::Optional(None) => Ok(None),
            Value::Optional(Some(value)) => T::from_value(value).map(Some),
            other => Err(SchemaError::new(format!(
                "expected optional, found {}",
                other.kind_name()
            ))),
        }
    }
}

impl<T: BernValue> BernValue for Vec<T> {
    type Ref = Binding<Vec<T>>;
    fn reference() -> Self::Ref {
        Binding::declared()
    }
    fn to_value(&self) -> Value {
        Value::List(self.iter().map(BernValue::to_value).collect())
    }
    fn from_value(value: &Value) -> Result<Self, SchemaError> {
        match value {
            Value::List(items) => items.iter().map(T::from_value).collect(),
            other => Err(SchemaError::new(format!(
                "expected list, found {}",
                other.kind_name()
            ))),
        }
    }
}
