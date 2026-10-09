//! Bern: a command-centred application framework with declarative UI.
//!
//! Applications declare their protocol with [`protocol`], their UI with
//! [`component`] and [`ui!`], and implement business logic as ordinary Rust
//! behind the generated command trait. `bern build` turns the declarations
//! into native UI for each target.

pub mod bind;

pub use bern_command::{CallContext, EngineProtocol, Entry, Host, Subscription};
pub use bern_macros::{component, protocol, session, tr, ui, value};
pub use bern_protocol::{
    CommandError, CommandOutcome, Invocation, LocalizedText, ProtocolDescriptor, SchemaError,
    StateUpdate, Type, Value,
};

/// Marker returned by component functions. Components are declarations;
/// targets generate the actual views from the collected description.
pub trait View {}

pub mod prelude {
    pub use crate::bind::{Binding, SessionField};
    pub use crate::{CommandError, View, component, tr, ui};
}

#[doc(hidden)]
pub mod __private;
