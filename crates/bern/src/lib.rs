//! Bern: a command-centred application framework with declarative UI.
//!
//! Applications declare their protocol with [`protocol`], their UI with
//! [`component`] and [`ui!`], and implement business logic as ordinary Rust
//! behind the generated command trait. `bern build` turns the declarations
//! into native UI for each target.

pub mod bind;
pub mod date;
pub mod media;
pub mod modules;
pub mod query;

pub use bern_command::{CallContext, EngineProtocol, Entry, Host, Subscription};
pub use bern_command::{resources, storage};
pub use bern_macros::{component, features, players, protocol, session, style, tr, ui, value};
pub use bern_protocol::{
    CommandError, CommandOutcome, Invocation, LocalizedText, ProtocolDescriptor, SchemaError,
    StateUpdate, Type, Value,
};

/// Marker returned by component functions. Components are declarations;
/// targets generate the actual views from the collected description.
pub trait View {}

/// A `#[bern::features]` contract: its variants are the features.
pub trait Features: Copy + 'static {}

pub mod prelude {
    pub use crate::bind::{Binding, SessionField};
    pub use crate::date::CalendarDate;
    pub use crate::media::{MediaSource, Player};
    pub use crate::query::Answer;
    pub use crate::{CommandError, Features, View, component, style, tr, ui};
}

#[doc(hidden)]
pub mod __private;
