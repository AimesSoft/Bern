//! Procedural macros for Bern declarations.
//!
//! The macros parse with `bern-syntax`, the same parser the build tool
//! uses, check declarations against the shared component specifications
//! and expand them into Rust that `rustc` type-checks. They never generate
//! UI code: targets do that from the collected description.

mod component;
mod config;
mod descriptor;
mod protocol;
mod session;
mod types;
mod values;

use proc_macro::TokenStream;

/// Declare an application protocol: value types, a read-only state struct
/// and a command trait.
///
/// ```ignore
/// #[bern::protocol(namespace = "player", version = 1)]
/// pub mod player {
///     use bern::prelude::*;
///
///     #[state]
///     pub struct State { pub position_ms: i64 }
///
///     #[commands]
///     pub trait Commands {
///         /// Move the playback position
///         fn seek(&mut self, position_ms: i64) -> Result<(), CommandError>;
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn protocol(args: TokenStream, item: TokenStream) -> TokenStream {
    protocol::expand(args.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare an enum or record used by UI declarations outside a protocol.
#[proc_macro_attribute]
pub fn value(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(proc_macro2::Span::call_site(), "`value` takes no arguments")
            .into_compile_error()
            .into();
    }
    values::expand_standalone(item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare typed session state owned by the window hosting a component.
#[proc_macro_attribute]
pub fn session(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "`session` takes no arguments",
        )
        .into_compile_error()
        .into();
    }
    session::expand(item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare an exportable component whose body is one `ui!` declaration.
#[proc_macro_attribute]
pub fn component(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "`component` takes no arguments",
        )
        .into_compile_error()
        .into();
    }
    component::expand(item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// The UI declaration grammar. Only valid as the body of a `#[component]`.
#[proc_macro]
pub fn ui(_input: TokenStream) -> TokenStream {
    syn::Error::new(
        proc_macro2::Span::call_site(),
        "`ui!` is only valid as the whole body of a `#[component]` function",
    )
    .into_compile_error()
    .into()
}

/// Localized text. Only valid inside `ui!` and `#[label(...)]`.
#[proc_macro]
pub fn tr(_input: TokenStream) -> TokenStream {
    syn::Error::new(
        proc_macro2::Span::call_site(),
        "`tr!` is only valid inside `ui!` declarations and `#[label(...)]` attributes",
    )
    .into_compile_error()
    .into()
}
