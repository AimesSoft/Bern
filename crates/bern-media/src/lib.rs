//! The media contract shared by every UI target and media backend.
//!
//! It defines what can be played (`MediaSource`), the standard media
//! commands with their schema, the merged state of one player, and the
//! protocol between the [`MediaHub`] and a media executor — the component
//! that really loads and plays media, such as AVFoundation in a macOS app.
//! Nothing here depends on a UI SDK or a decoder.
//!
//! Every player has a stable `player_id`. The UI, the CLI, JSON clients and
//! the MCP server all address it through the same commands; the hub routes
//! them to the executor that holds the player and merges what the executor
//! reports back into one state that every entry observes.

mod hub;

pub use hub::{APPLICATION_OWNER, MediaHub, MediaSubscription, MediaUpdate};

use bern_protocol::{
    CodecError, CommandPolicy, CommandSpec, EnumDef, ExternalNames, FieldDef, OperationKind,
    ParamSpec, Reader, RecordDef, Retry, Scope, Type, TypeTable, VariantDef, Writer,
};

/// Version of the media contract: commands, player state and the executor
/// protocol. Targets and media backends declare the versions they support.
pub const MEDIA_CONTRACT_VERSION: u32 = 1;

/// Name of the framework value type describing playable media.
pub const MEDIA_SOURCE: &str = "MediaSource";

fn field(name: &str, ty: Type, doc: &str) -> FieldDef {
    FieldDef {
        name: name.into(),
        ty,
        doc: doc.into(),
    }
}

fn unit_enum(name: &str, variants: &[&str]) -> EnumDef {
    EnumDef {
        name: name.into(),
        variants: variants
            .iter()
            .map(|variant| VariantDef {
                name: (*variant).into(),
                fields: vec![],
                label: None,
            })
            .collect(),
    }
}

/// Schema of `MediaSource`, the framework type applications use for media
/// in their protocol state, commands and UI values.
pub fn media_source_def() -> RecordDef {
    RecordDef {
        name: MEDIA_SOURCE.into(),
        fields: vec![field(
            "url",
            Type::Text,
            "Location of the media: an http(s) URL or a file URL",
        )],
    }
}

fn record(name: &str) -> Type {
    Type::Record { name: name.into() }
}

fn enum_type(name: &str) -> Type {
    Type::Enum { name: name.into() }
}

/// Types of the media catalog and of the player state.
pub fn media_types() -> TypeTable {
    TypeTable {
        enums: vec![
            unit_enum("MediaPhase", Phase::NAMES),
            unit_enum("MediaRequestStatus", &["Accepted", "Executed", "Failed"]),
        ],
        records: vec![
            media_source_def(),
            RecordDef {
                name: "MediaRequest".into(),
                fields: vec![
                    field("command", Type::Text, "Command id of the latest request"),
                    field(
                        "status",
                        enum_type("MediaRequestStatus"),
                        "Accepted by the hub, executed by the media backend, or failed",
                    ),
                    field(
                        "source_version",
                        Type::Int,
                        "Source version the request applied to",
                    ),
                    field(
                        "error",
                        Type::optional(Type::Text),
                        "Error code when the request failed",
                    ),
                ],
            },
            RecordDef {
                name: "MediaPlayer".into(),
                fields: PLAYER_FIELDS
                    .iter()
                    .map(|(name, ty, doc)| field(name, ty(), doc))
                    .collect(),
            },
        ],
    }
}

type FieldType = fn() -> Type;

/// Fields of the merged player state, in encoding order.
pub const PLAYER_FIELDS: &[(&str, FieldType, &str)] = &[
    ("player_id", || Type::Text, "Stable player identity"),
    (
        "owner",
        || Type::Text,
        "`application`, or the window instance whose component owns the player",
    ),
    (
        "source_bound",
        || Type::Bool,
        "The source follows a declaration binding and cannot be replaced by media.open",
    ),
    (
        "source",
        || Type::optional(record(MEDIA_SOURCE)),
        "Current media source",
    ),
    (
        "source_version",
        || Type::Int,
        "Incremented whenever the source changes; older reports are ignored",
    ),
    (
        "phase",
        || enum_type("MediaPhase"),
        "Playback phase reported by the media backend",
    ),
    ("position_ms", || Type::Int, "Playback position"),
    (
        "duration_ms",
        || Type::optional(Type::Int),
        "Duration; absent while unknown or for live media",
    ),
    (
        "seekable",
        || Type::Bool,
        "Whether media.seek can move the position",
    ),
    ("volume", || Type::Float, "Volume between 0 and 1"),
    ("muted", || Type::Bool, "Whether audio is muted"),
    (
        "error",
        || Type::optional(Type::Text),
        "Why the media failed",
    ),
    ("views", || Type::Int, "Number of attached video views"),
    (
        "request",
        || Type::optional(record("MediaRequest")),
        "The latest command request and its status",
    ),
];

/// Playback phase. `Unavailable` is set by the hub when no executor can
/// run the player; every other phase is reported by the executor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Buffering,
    Ended,
    Failed,
    Unavailable,
}

impl Phase {
    pub const ALL: [Phase; 9] = [
        Phase::Idle,
        Phase::Loading,
        Phase::Ready,
        Phase::Playing,
        Phase::Paused,
        Phase::Buffering,
        Phase::Ended,
        Phase::Failed,
        Phase::Unavailable,
    ];

    const NAMES: &'static [&'static str] = &[
        "Idle",
        "Loading",
        "Ready",
        "Playing",
        "Paused",
        "Buffering",
        "Ended",
        "Failed",
        "Unavailable",
    ];

    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|phase| *phase == self).unwrap() as u32
    }

    pub fn from_index(index: u32) -> Option<Phase> {
        Self::ALL.get(index as usize).copied()
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self.index() as usize]
    }
}

/// One standard media command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaCommand {
    Players,
    State,
    Open,
    Play,
    Pause,
    Seek,
    SetVolume,
    SetMuted,
}

impl MediaCommand {
    pub const ALL: [MediaCommand; 8] = [
        MediaCommand::Players,
        MediaCommand::State,
        MediaCommand::Open,
        MediaCommand::Play,
        MediaCommand::Pause,
        MediaCommand::Seek,
        MediaCommand::SetVolume,
        MediaCommand::SetMuted,
    ];

    pub fn id(self) -> &'static str {
        match self {
            MediaCommand::Players => "media.players",
            MediaCommand::State => "media.state",
            MediaCommand::Open => "media.open",
            MediaCommand::Play => "media.play",
            MediaCommand::Pause => "media.pause",
            MediaCommand::Seek => "media.seek",
            MediaCommand::SetVolume => "media.set_volume",
            MediaCommand::SetMuted => "media.set_muted",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }

    /// Index in [`media_catalog`], used by in-process bridges.
    pub fn index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|command| *command == self)
            .unwrap() as u32
    }

    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }
}

fn param(name: &str, ty: Type, doc: &str) -> ParamSpec {
    ParamSpec {
        name: name.into(),
        ty,
        doc: doc.into(),
    }
}

/// Error codes media commands may return besides the generic ones.
pub const ERRORS: &[&str] = &[
    "player_not_found",
    "executor_unavailable",
    "source_bound",
    "no_source",
    "not_seekable",
    "out_of_range",
    "media_failed",
    "stale_source",
    "media_timeout",
];

/// Type table and command specs of the standard media commands. They act
/// on the engine's players and never need a UI session.
pub fn media_catalog() -> (TypeTable, Vec<CommandSpec>) {
    let player = || {
        param(
            "player_id",
            Type::Text,
            "Player identity from media.players",
        )
    };
    let spec = |command: MediaCommand,
                kind: OperationKind,
                doc: &str,
                params: Vec<ParamSpec>,
                result: Type,
                retry: Retry| {
        let (namespace, name) = command.id().split_once('.').expect("namespaced id");
        CommandSpec {
            id: command.id().into(),
            name: name.into(),
            doc: doc.into(),
            kind,
            scope: Scope::Engine,
            params,
            result,
            errors: ERRORS.iter().map(|code| (*code).to_string()).collect(),
            policy: CommandPolicy {
                retry,
                ..CommandPolicy::default()
            },
            external: ExternalNames::derive(namespace, name),
        }
    };
    let version = || Type::Int;
    let commands = vec![
        spec(
            MediaCommand::Players,
            OperationKind::Query,
            "List every player with its merged state",
            vec![],
            Type::list(record("MediaPlayer")),
            Retry::Idempotent,
        ),
        spec(
            MediaCommand::State,
            OperationKind::Query,
            "Read the merged state of one player",
            vec![player()],
            record("MediaPlayer"),
            Retry::Idempotent,
        ),
        spec(
            MediaCommand::Open,
            OperationKind::Command,
            "Load a new source into a player that is not bound to a declaration; returns the new source version",
            vec![
                player(),
                param("source", record(MEDIA_SOURCE), "Media to load"),
            ],
            version(),
            Retry::Never,
        ),
        spec(
            MediaCommand::Play,
            OperationKind::Command,
            "Start or resume playback; returns the source version it applied to",
            vec![player()],
            version(),
            Retry::Idempotent,
        ),
        spec(
            MediaCommand::Pause,
            OperationKind::Command,
            "Pause playback; returns the source version it applied to",
            vec![player()],
            version(),
            Retry::Idempotent,
        ),
        spec(
            MediaCommand::Seek,
            OperationKind::Command,
            "Move the playback position of seekable media",
            vec![
                player(),
                param("position_ms", Type::Int, "Target position in milliseconds"),
            ],
            version(),
            Retry::Idempotent,
        ),
        spec(
            MediaCommand::SetVolume,
            OperationKind::Command,
            "Set the volume between 0 and 1",
            vec![
                player(),
                param("volume", Type::Float, "Volume between 0 and 1"),
            ],
            version(),
            Retry::Idempotent,
        ),
        spec(
            MediaCommand::SetMuted,
            OperationKind::Command,
            "Mute or unmute audio",
            vec![
                player(),
                param("muted", Type::Bool, "Whether audio is muted"),
            ],
            version(),
            Retry::Idempotent,
        ),
    ];
    (media_types(), commands)
}

/// What the media backend and its video presenter support, reported when
/// an executor attaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaCapabilities {
    /// Backend identity and version, for example `avfoundation`.
    pub backend: String,
    pub backend_version: String,
    /// Players keep playing while no view is attached.
    pub background_playback: bool,
    /// Several views may show one player at the same time.
    pub multi_view: bool,
}

/// One operation the hub asks the executor to perform.
#[derive(Clone, Debug, PartialEq)]
pub enum ExecOp {
    Load { url: String },
    Unload,
    Play,
    Pause,
    Seek { position_ms: i64 },
    SetVolume { volume: f64 },
    SetMuted { muted: bool },
    Release,
}

impl ExecOp {
    fn code(&self) -> u32 {
        match self {
            ExecOp::Load { .. } => 0,
            ExecOp::Unload => 1,
            ExecOp::Play => 2,
            ExecOp::Pause => 3,
            ExecOp::Seek { .. } => 4,
            ExecOp::SetVolume { .. } => 5,
            ExecOp::SetMuted { .. } => 6,
            ExecOp::Release => 7,
        }
    }
}

/// A request routed to the executor that holds `player`. The executor
/// rejects it with `stale_source` when it already runs a newer source.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecRequest {
    pub request: u64,
    pub player: String,
    pub source_version: u64,
    pub op: ExecOp,
}

impl ExecRequest {
    /// Binary layout for in-process executors: request u64, player text,
    /// source version u64, operation u32, then the operation's payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.u64(self.request);
        writer.text(&self.player);
        writer.u64(self.source_version);
        writer.u32(self.op.code());
        match &self.op {
            ExecOp::Load { url } => writer.text(url),
            ExecOp::Seek { position_ms } => writer.i64(*position_ms),
            ExecOp::SetVolume { volume } => writer.f64(*volume),
            ExecOp::SetMuted { muted } => writer.u8(u8::from(*muted)),
            ExecOp::Unload | ExecOp::Play | ExecOp::Pause | ExecOp::Release => {}
        }
        writer.bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::new(bytes);
        let request = reader.u64()?;
        let player = reader.text()?;
        let source_version = reader.u64()?;
        let op = match reader.u32()? {
            0 => ExecOp::Load {
                url: reader.text()?,
            },
            1 => ExecOp::Unload,
            2 => ExecOp::Play,
            3 => ExecOp::Pause,
            4 => ExecOp::Seek {
                position_ms: reader.i64()?,
            },
            5 => ExecOp::SetVolume {
                volume: reader.f64()?,
            },
            6 => ExecOp::SetMuted {
                muted: reader.u8()? != 0,
            },
            7 => ExecOp::Release,
            other => return Err(CodecError(format!("unknown media operation {other}"))),
        };
        Ok(Self {
            request,
            player,
            source_version,
            op,
        })
    }
}

/// What the executor observed for one player and source version. These are
/// facts, never requests: merging them must not dispatch commands.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub phase: Phase,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub seekable: bool,
    pub volume: f64,
    pub muted: bool,
    pub error: Option<String>,
}

impl Observation {
    /// The state of a player that has not loaded anything yet.
    pub fn idle() -> Self {
        Self {
            phase: Phase::Idle,
            position_ms: 0,
            duration_ms: None,
            seekable: false,
            volume: 1.0,
            muted: false,
            error: None,
        }
    }

    /// Binary layout: player text, source version u64, phase u32,
    /// position i64, duration flag u8 + i64, seekable u8, volume f64,
    /// muted u8, error flag u8 + text.
    pub fn encode(&self, player: &str, source_version: u64) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.text(player);
        writer.u64(source_version);
        writer.u32(self.phase.index());
        writer.i64(self.position_ms);
        match self.duration_ms {
            Some(duration) => {
                writer.u8(1);
                writer.i64(duration);
            }
            None => writer.u8(0),
        }
        writer.u8(u8::from(self.seekable));
        writer.f64(self.volume);
        writer.u8(u8::from(self.muted));
        match &self.error {
            Some(error) => {
                writer.u8(1);
                writer.text(error);
            }
            None => writer.u8(0),
        }
        writer.bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<(String, u64, Self), CodecError> {
        let mut reader = Reader::new(bytes);
        let player = reader.text()?;
        let version = reader.u64()?;
        let phase = Phase::from_index(reader.u32()?)
            .ok_or_else(|| CodecError("unknown media phase".into()))?;
        let position_ms = reader.i64()?;
        let duration_ms = match reader.u8()? {
            0 => None,
            _ => Some(reader.i64()?),
        };
        let seekable = reader.u8()? != 0;
        let volume = reader.f64()?;
        let muted = reader.u8()? != 0;
        let error = match reader.u8()? {
            0 => None,
            _ => Some(reader.text()?),
        };
        if !volume.is_finite() {
            return Err(CodecError("volume must be finite".into()));
        }
        Ok((
            player,
            version,
            Self {
                phase,
                position_ms,
                duration_ms,
                seekable,
                volume,
                muted,
                error,
            },
        ))
    }
}

/// The component that really plays media. `execute` must not block: the
/// executor answers later with [`MediaHub::complete`] and reports facts
/// with [`MediaHub::observe`]. It never dispatches media commands itself.
pub trait MediaExecutor: Send + Sync {
    fn execute(&self, request: ExecRequest);
}

#[cfg(test)]
mod tests;
