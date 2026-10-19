//! Application protocol shared by every Bern entry point.
//!
//! The schema here is independent of UI frameworks and of any transport
//! encoding. Adapters translate it to a wire format (JSON for control
//! channels, a compact binary layout for in-process bridges) but the meaning
//! of commands, state and values is defined once.

mod codec;
mod compose;
mod descriptor;
mod error;
pub mod format;
mod message;
mod resource;
mod session;
mod text;
mod types;
mod value;

pub use codec::{CodecError, Reader, Writer, decode, encode};
pub use compose::{
    MODULES_STATE_TYPE, ModulePart, SET_ENABLED, command_offsets, compose, module_state_type,
};
pub use descriptor::{
    CommandPolicy, CommandSpec, Concurrency, DescriptorError, EventSpec, ExternalNames,
    MODULES_NAMESPACE, OperationKind, ParamSpec, ProtocolDescriptor, RESERVED_NAMESPACES, Retry,
    Scope, StateField, check_external_names, valid_name,
};
pub use error::{CommandError, SchemaError};
pub use message::{Cancellation, CommandOutcome, CommandStatus, Invocation, StateUpdate};
pub use resource::check_resource_name;
pub use session::{SessionCommand, session_catalog};
pub use text::{LocalizedText, label_key};
pub use types::{EnumDef, FieldDef, RecordDef, Type, TypeTable, VariantDef};
pub use value::Value;

/// Version of the application protocol messages exchanged between entries
/// and an engine host. Incompatible peers must fail during the handshake.
pub const PROTOCOL_VERSION: u32 = 1;

/// Version of the serialized protocol descriptor layout.
pub const DESCRIPTOR_VERSION: u32 = 1;

/// Stable 64-bit FNV-1a hash used for catalog fingerprints.
pub fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}
