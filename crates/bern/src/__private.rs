//! Support for macro expansions. Not a public API.
//!
//! The functions here only exist to give `rustc` something to type-check:
//! component bodies expand into code that is never executed but fails to
//! compile when a binding, command argument or property has the wrong type.

#![allow(clippy::missing_panics_doc)]

use crate::bind::*;

pub use bern_command as command;
pub use bern_protocol as protocol;

/// The value every component function returns.
pub struct Declared;

impl crate::View for Declared {}

/// The type of `tr!` text inside type-checked declarations.
pub struct Localized;

impl Displayable for Localized {}

pub fn localized() -> Localized {
    Localized
}

pub fn value_of<T, R: Readable<T>>(_: &R) -> T {
    unreachable!("declarations are only type-checked")
}

pub fn reref<T: BernValue>(_: &T) -> T::Ref {
    T::reference()
}

pub fn to_ref<R: ToRef>(reference: &R) -> R::Ref {
    reference.to_ref()
}

pub fn display<D: Displayable + ?Sized>(_: &D) {}

pub fn text_field(_: &SessionField<String>) {}

pub fn bool_field(_: &SessionField<bool>) {}

pub fn number_field(_: &SessionField<f64>) {}

pub fn picker_field<E: LabeledEnum>(_: &SessionField<E>) {}

pub fn selection_field<E: BernEnum>(_: &SessionField<E>) -> E {
    unreachable!("declarations are only type-checked")
}

pub fn path_field<R: BernEnum>(_: &SessionField<Vec<R>>) -> R {
    unreachable!("declarations are only type-checked")
}

pub fn list_item<T: BernValue, L: Readable<Vec<T>>>(_: &L) -> T::Ref {
    T::reference()
}

pub fn key<K: ListKey>(_: &K) {}

pub fn same_type<T>(_: &T, _: &T) {}

pub fn command(_: CommandRef) {}

pub fn query_result<T, S: BernValue>(_: &QueryRef<T, S>) -> S::Ref {
    S::reference()
}

pub fn query_empty<T: EmptyResult, S>(_: &QueryRef<T, S>) {}

pub fn enum_value<E: BernEnum>(_: &E) {}

pub fn assert_value<T: BernValue>() {}

pub fn media_source<S: SourceBinding>(_: &S) {}

pub fn player(_: &PlayerRef) {}

pub fn feature<F: crate::Features>(_: &F) {}

pub fn features<F: crate::Features>() {}
