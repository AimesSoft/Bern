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

/// Performance tracing: `let _span = bern::trace::span!("reindex");`
/// measures application code on the trace timeline while tracing is on
/// (`BERN_TRACE`, `trace.start`), and costs one check while it is off.
pub mod trace {
    pub use bern_command::trace::{ALL, DEFAULT, ENGINE, SPAN, UI};
    pub use bern_command::trace::{Span, mask, on};
    pub use bern_command::trace_span as span;
}

pub use bern_command::{CallContext, EngineProtocol, Entry, Host, Subscription};
pub use bern_command::{files, platform, resources, secrets, storage, wake};
pub use bern_macros::{component, features, players, protocol, session, style, tr, ui, value};
/// Keyboard shortcuts as declarations write them, for engines that keep
/// shortcuts the interface follows: [`shortcut::parse_shortcut`]
/// normalizes one a user chose.
pub use bern_protocol::shortcut;
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

/// Target-selected JavaScript contexts.
pub use bern_script as script;
