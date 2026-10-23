//! What `trace.report` answers: per-command distributions, the slowest
//! interactions and the long frames. The values follow the `Trace…`
//! records of the session catalog (`bern_protocol::session_catalog`).

use bern_protocol::Value;

/// A distribution in whole units (microseconds or bytes): p50 and p95 of
/// the recent samples, the maximum of all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Distribution {
    pub count: u64,
    pub p50: u64,
    pub p95: u64,
    pub max: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandReport {
    pub id: String,
    pub entry: String,
    pub total_us: Distribution,
    pub queue_us: Distribution,
    pub exec_us: Distribution,
    pub publish_us: Distribution,
    pub apply_us: Distribution,
    pub bytes: Distribution,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueryReport {
    pub id: String,
    pub cancelled: u64,
    pub queue_us: Distribution,
    pub provider_us: Distribution,
    pub round_trip_us: Distribution,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldReport {
    pub name: String,
    pub sent: u64,
    pub bytes_max: u64,
    pub items_max: Option<u64>,
    pub snapshot_us: Distribution,
    pub compare_us: Distribution,
    pub encode_us: Distribution,
    pub apply_us: Distribution,
}

/// One named part of an interaction or a frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Segment {
    pub name: String,
    pub us: u64,
}

/// An interaction, or a command that came from outside the UI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InteractionReport {
    pub control: Option<String>,
    pub entry: String,
    pub commands: Vec<String>,
    pub at_ms: u64,
    pub total_us: u64,
    pub revision: Option<u64>,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameReport {
    pub at_ms: u64,
    pub us: u64,
    pub refresh_us: u64,
    pub bern_us: u64,
    pub outside_us: u64,
    pub segments: Vec<Segment>,
    pub engine: Vec<Segment>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FramesReport {
    pub spacing_us: Distribution,
    pub long: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpanReport {
    pub name: String,
    pub us: Distribution,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub running: bool,
    pub window_ms: u64,
    pub events: u64,
    pub dropped: u64,
    pub commands: Vec<CommandReport>,
    pub queries: Vec<QueryReport>,
    pub fields: Vec<FieldReport>,
    pub interactions: Vec<InteractionReport>,
    pub frames: FramesReport,
    pub long_frames: Vec<FrameReport>,
    pub spans: Vec<SpanReport>,
}

fn int(value: u64) -> Value {
    Value::Int(value.min(i64::MAX as u64) as i64)
}

fn text(value: &str) -> Value {
    Value::Text(value.to_string())
}

fn optional(value: Option<Value>) -> Value {
    Value::Optional(value.map(Box::new))
}

impl Distribution {
    pub fn to_value(&self) -> Value {
        Value::Record(vec![
            int(self.count),
            int(self.p50),
            int(self.p95),
            int(self.max),
        ])
    }
}

impl Segment {
    fn to_value(&self) -> Value {
        Value::Record(vec![text(&self.name), int(self.us)])
    }
}

fn segments(list: &[Segment]) -> Value {
    Value::List(list.iter().map(Segment::to_value).collect())
}

impl Report {
    /// The report as a `TraceReport` record.
    pub fn to_value(&self) -> Value {
        let commands = self
            .commands
            .iter()
            .map(|command| {
                Value::Record(vec![
                    text(&command.id),
                    text(&command.entry),
                    command.total_us.to_value(),
                    command.queue_us.to_value(),
                    command.exec_us.to_value(),
                    command.publish_us.to_value(),
                    command.apply_us.to_value(),
                    command.bytes.to_value(),
                ])
            })
            .collect();
        let queries = self
            .queries
            .iter()
            .map(|query| {
                Value::Record(vec![
                    text(&query.id),
                    int(query.cancelled),
                    query.queue_us.to_value(),
                    query.provider_us.to_value(),
                    query.round_trip_us.to_value(),
                ])
            })
            .collect();
        let fields = self
            .fields
            .iter()
            .map(|field| {
                Value::Record(vec![
                    text(&field.name),
                    int(field.sent),
                    int(field.bytes_max),
                    optional(field.items_max.map(int)),
                    field.snapshot_us.to_value(),
                    field.compare_us.to_value(),
                    field.encode_us.to_value(),
                    field.apply_us.to_value(),
                ])
            })
            .collect();
        let interactions = self
            .interactions
            .iter()
            .map(|interaction| {
                Value::Record(vec![
                    optional(interaction.control.as_deref().map(text)),
                    text(&interaction.entry),
                    Value::List(interaction.commands.iter().map(|id| text(id)).collect()),
                    int(interaction.at_ms),
                    int(interaction.total_us),
                    optional(interaction.revision.map(int)),
                    segments(&interaction.segments),
                ])
            })
            .collect();
        let long_frames = self
            .long_frames
            .iter()
            .map(|frame| {
                Value::Record(vec![
                    int(frame.at_ms),
                    int(frame.us),
                    int(frame.refresh_us),
                    int(frame.bern_us),
                    int(frame.outside_us),
                    segments(&frame.segments),
                    segments(&frame.engine),
                ])
            })
            .collect();
        let spans = self
            .spans
            .iter()
            .map(|span| Value::Record(vec![text(&span.name), span.us.to_value()]))
            .collect();
        Value::Record(vec![
            Value::Bool(self.running),
            int(self.window_ms),
            int(self.events),
            int(self.dropped),
            Value::List(commands),
            Value::List(queries),
            Value::List(fields),
            Value::List(interactions),
            Value::Record(vec![
                self.frames.spacing_us.to_value(),
                int(self.frames.long),
            ]),
            Value::List(long_frames),
            Value::List(spans),
        ])
    }
}
