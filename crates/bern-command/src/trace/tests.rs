use super::collect::Collector;
use super::ring::{self, Record, Ring};
use super::rules::{Rules, TimelineFile, duration, from_env, glob_matches};
use super::*;
use crate::{CallContext, EngineProtocol, Entry, Host};
use bern_protocol::{
    CommandError, CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, ExternalNames, Invocation,
    OperationKind, ProtocolDescriptor, Scope, StateField, Type, TypeTable, session_catalog,
};
use std::sync::{Mutex, OnceLock};

/// Tests that start or stop the global trace run one at a time.
static GLOBAL: Mutex<()> = Mutex::new(());

fn global() -> std::sync::MutexGuard<'static, ()> {
    GLOBAL.lock().unwrap_or_else(|error| error.into_inner())
}

#[test]
fn rules_parse_categories_filters_and_options() {
    let rules = Rules::parse("engine,ui,slow:16ms").unwrap();
    assert_eq!(rules.mask, ENGINE | UI);
    assert_eq!(rules.slow_ns, 16_000_000);
    let rules = Rules::parse("command:library.*").unwrap();
    // Filters alone record the default categories.
    assert_eq!(rules.mask, DEFAULT);
    assert_eq!(rules.commands, vec!["library.*".to_string()]);
    let rules = Rules::parse(
        "all,-frame,query:*.search,control:Library.add*,field:items,feature:Media.Clip,long:20ms,file:/tmp/a:b.json,quiet,buffer:1024",
    )
    .unwrap();
    assert_eq!(rules.mask, ALL & !FRAME);
    assert_eq!(rules.queries, vec!["*.search".to_string()]);
    assert_eq!(rules.controls, vec!["Library.add*".to_string()]);
    assert_eq!(rules.fields, vec!["items".to_string()]);
    assert_eq!(rules.features, vec!["Media.Clip".to_string()]);
    assert_eq!(rules.long_ns, Some(20_000_000));
    // Everything after `file:` is the path, colons too.
    assert_eq!(rules.file, TimelineFile::Path("/tmp/a:b.json".into()));
    assert!(rules.quiet);
    assert_eq!(rules.buffer, 1024);
    assert_eq!(Rules::parse("1").unwrap().mask, DEFAULT);
    assert_eq!(Rules::parse("on,motion.values").unwrap().mask, DEFAULT | MOTION_VALUES);
    assert_eq!(Rules::parse("file:none").unwrap().file, TimelineFile::None);
}

#[test]
fn rules_print_in_a_spelling_that_parses_back() {
    for text in [
        "engine,ui,slow:16ms",
        "all,command:library.*,field:items",
        "motion.values",
        "all,motion.values,long:20ms,file:none,quiet,buffer:64",
        "frame,span,query:a.*,control:x.y,feature:F.g,slow:250us",
    ] {
        let rules = Rules::parse(text).unwrap();
        assert_eq!(Rules::parse(&rules.to_string()).unwrap(), rules, "{text}");
    }
    assert_eq!(Rules::parse("ui,engine").unwrap().to_string(), "engine,ui");
}

#[test]
fn rules_reject_what_they_do_not_know() {
    for (text, expected) in [
        ("engin", "unknown trace rule `engin`"),
        ("slow:fast", "expected a duration"),
        ("long:0ms", "expected a duration"),
        ("buffer:2", "16 to 16777216"),
        ("command:", "needs a name"),
        ("-all", "no category"),
        ("-nothing", "unknown trace rule `-nothing`"),
    ] {
        let error = Rules::parse(text).unwrap_err();
        assert!(error.contains(expected), "{text}: {error}");
    }
}

#[test]
fn durations_take_units_and_bare_milliseconds() {
    assert_eq!(duration("16ms"), Some(16_000_000));
    assert_eq!(duration("250us"), Some(250_000));
    assert_eq!(duration("1.5s"), Some(1_500_000_000));
    assert_eq!(duration("8"), Some(8_000_000));
    assert_eq!(duration("-1ms"), None);
}

#[test]
fn globs_match_any_run_and_single_characters() {
    assert!(glob_matches("library.*", "library.add_item"));
    assert!(glob_matches("*.add_item", "library.add_item"));
    assert!(glob_matches("lib*add*", "library.add_item"));
    assert!(glob_matches("library.add_ite?", "library.add_item"));
    assert!(glob_matches("*", ""));
    assert!(!glob_matches("library.*", "librar.add"));
    assert!(!glob_matches("a?c", "ac"));
    assert!(glob_matches("Card.open*", "Card.open[3]"));
}

#[test]
fn old_variables_become_rules() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    };
    assert_eq!(from_env(env(&[])), None);
    assert_eq!(
        from_env(env(&[("BERN_TRACE", "engine")])).as_deref(),
        Some("engine")
    );
    assert_eq!(
        from_env(env(&[("BERN_FRAME_LOG", "1")])).as_deref(),
        Some("motion,frame,style")
    );
    // An empty BERN_FRAME_LOG was off; BERN_MOTION_LOG only had to be set.
    assert_eq!(from_env(env(&[("BERN_FRAME_LOG", "")])), None);
    assert_eq!(
        from_env(env(&[("BERN_MOTION_LOG", "")])).as_deref(),
        Some("motion.values")
    );
    let rules = from_env(env(&[("BERN_TRACE", "engine"), ("BERN_FRAME_LOG", "1")])).unwrap();
    assert_eq!(
        Rules::parse(&rules).unwrap().mask,
        ENGINE | MOTION | FRAME | STYLE
    );
}

#[test]
fn a_full_ring_drops_and_counts() {
    let ring = Ring::new(4, 1, "test".into());
    let record = |key| Record {
        key,
        ..Record::default()
    };
    assert!(!ring.push(record(1)));
    assert!(ring.push(record(2)), "half full");
    ring.push(record(3));
    ring.push(record(4));
    ring.push(record(5));
    assert_eq!(ring.dropped(), 1);
    let mut out = Vec::new();
    ring.drain(&mut out);
    assert_eq!(out.iter().map(|r| r.key).collect::<Vec<_>>(), [1, 2, 3, 4]);
    // Room again after reading, and the slots wrap around.
    ring.push(record(6));
    ring.push(record(7));
    out.clear();
    ring.drain(&mut out);
    assert_eq!(out.iter().map(|r| r.key).collect::<Vec<_>>(), [6, 7]);
}

#[test]
fn rings_read_while_threads_write() {
    let ring = std::sync::Arc::new(Ring::new(1024, 1, "test".into()));
    let writer = {
        let ring = std::sync::Arc::clone(&ring);
        std::thread::spawn(move || {
            let mut key = 0;
            while key < 100_000 {
                if ring.dropped() == 0 || key % 2 == 0 {
                    ring.push(Record {
                        key,
                        ..Record::default()
                    });
                }
                key += 1;
            }
        })
    };
    let mut seen = Vec::new();
    while !writer.is_finished() {
        ring.drain(&mut seen);
    }
    writer.join().unwrap();
    ring.drain(&mut seen);
    // What was kept arrives once and in order.
    assert!(seen.windows(2).all(|pair| pair[0].key < pair[1].key));
    assert_eq!(seen.len() as u64 + ring.dropped(), seen.len() as u64 + ring.dropped());
    assert!(!seen.is_empty());
}

#[test]
fn off_points_read_no_clock_and_make_no_ring() {
    let _global = global();
    assert_eq!(mask(), 0, "no trace running");
    CLOCK_READS.with(|reads| reads.set(0));
    {
        let _span = crate::trace_span!("idle");
        let _formatted = crate::trace_span!("idle {}", 1);
    }
    let value = snapshot_field(0, || Value::Int(1));
    assert_eq!(value, Value::Int(1));
    with_origin(Origin::Token(1), || ());
    local_engine(|| ());
    let job = super::job(1, 0, &context());
    assert!(job.is_some(), "jobs are only asked for while tracing");
    assert_eq!(CLOCK_READS.with(|reads| reads.get()), 1, "only `job` read the clock");
    assert!(!ring::has_ring(), "nothing recorded");
}

// ---------------------------------------------------------------------------
// The collector, fed by hand

fn descriptor() -> &'static ProtocolDescriptor {
    static DESCRIPTOR: OnceLock<ProtocolDescriptor> = OnceLock::new();
    DESCRIPTOR.get_or_init(|| {
        let command = |name: &str| CommandSpec {
            id: format!("tracetest.{name}"),
            name: name.into(),
            doc: String::new(),
            kind: OperationKind::Command,
            scope: Scope::Engine,
            params: vec![],
            result: Type::Unit,
            errors: vec![],
            policy: CommandPolicy::default(),
            external: ExternalNames::derive("tracetest", name),
            reads: vec![],
            on: None,
        };
        let field = |name: &str, ty: Type| StateField {
            name: name.into(),
            ty,
            doc: String::new(),
            content: false,
        };
        let descriptor = ProtocolDescriptor {
            modules: Vec::new(),
            events: Vec::new(),
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "tracetest".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![
                field("items", Type::list(Type::Int)),
                field("count", Type::Int),
            ],
            commands: vec![command("add_item"), command("noop")],
        };
        descriptor.validate().unwrap();
        descriptor
    })
}

fn context() -> CallContext {
    CallContext {
        entry: Entry::Test,
        request_id: "t".into(),
    }
}

const MS: u64 = 1_000_000;

fn rec(kind: u16, t0: u64, t1: u64, key: u64) -> Record {
    Record {
        t0: t0 * MS / 10,
        t1: t1 * MS / 10,
        key,
        kind,
        catalog: register_catalog(descriptor()),
        track: 2,
        ..Record::default()
    }
}

/// Records of one UI interaction: the control's action dispatches token 7
/// at 1.0 ms, the engine runs it as job 40 and publishes revision 5 with
/// a large `items`, the UI applies it and the next frame shows it.
fn interaction_records() -> Vec<Record> {
    let ui = |mut record: Record| {
        record.track = 1;
        record
    };
    vec![
        ui(Record {
            name: intern("Library.add"),
            ..rec(kind::ACT, 10, 12, 3)
        }),
        ui(Record {
            b: 1,
            ..rec(kind::DECODE, 10, 11, 7)
        }),
        ui(Record {
            a: 0,
            b: 3,
            ..rec(kind::DISPATCH, 10, 11, 7)
        }),
        Record {
            a: 7,
            extra: entry::UI,
            name: 0,
            ..rec(kind::QUEUE, 11, 13, 40)
        },
        Record {
            a: 1,
            ..rec(kind::EXEC, 13, 15, 40)
        },
        Record {
            name: 0,
            ..rec(kind::SNAPSHOT_FIELD, 15, 30, 40)
        },
        Record {
            name: 1,
            ..rec(kind::SNAPSHOT_FIELD, 30, 31, 40)
        },
        rec(kind::SNAPSHOT, 15, 31, 40),
        Record {
            name: 0,
            a: 1,
            ..rec(kind::COMPARE_FIELD, 31, 50, 40)
        },
        Record {
            name: 1,
            a: 0,
            ..rec(kind::COMPARE_FIELD, 50, 51, 40)
        },
        Record {
            name: 0,
            a: 5,
            b: 1_310_720,
            extra: 10_002,
            ..rec(kind::ENCODE_FIELD, 51, 60, 40)
        },
        Record {
            b: 1_310_740,
            extra: CALLBACK_STATE,
            ..rec(kind::CALLBACK, 60, 60, 5)
        },
        Record {
            a: 5,
            ..rec(kind::PUBLISH, 15, 61, 40)
        },
        Record {
            extra: CALLBACK_REPLY,
            ..rec(kind::CALLBACK, 61, 61, 7)
        },
        ui(Record {
            a: 1_310_740,
            ..rec(kind::RECEIVE_STATE, 62, 130, 5)
        }),
        ui(Record {
            a: 0,
            b: 1_310_720,
            ..rec(kind::APPLY_FIELD, 63, 129, 5)
        }),
        ui(rec(kind::RECEIVE_REPLY, 131, 132, 7)),
        ui(Record {
            a: 16_666_667,
            b: 400 * MS / 10,
            ..rec(kind::FRAME, 300, 320, 1)
        }),
    ]
}

fn new_collector(rules: &str) -> Collector {
    Collector::new(Rules::parse(rules).unwrap(), 0, None)
}

#[test]
fn an_interaction_is_matched_across_threads_and_out_of_order() {
    let mut records = interaction_records();
    // The UI ring was read before the engine's.
    records.rotate_left(14);
    let (ui, engine) = records.split_at(4);
    let mut collector = new_collector("all");
    collector.process(ui.to_vec(), Vec::new(), 0, &[], 40 * MS);
    assert!(collector.take_lines().is_empty(), "waits for the engine");
    collector.process(engine.to_vec(), Vec::new(), 0, &[], 41 * MS);
    let lines = collector.take_lines().join("\n");
    assert!(lines.starts_with("bern-trace: interaction Library.add 39.0 ms (action 0.20)"), "{lines}");
    assert!(lines.contains("tracetest.add_item: dispatch 0.10, queue 0.20, exec 0.20, publish 4.60 -> rev 5"), "{lines}");
    assert!(lines.contains("publish: snapshot 1.60, compare 2.00, encode 0.90"), "{lines}");
    assert!(
        lines.contains("items 1.25 MiB, 10001 items, sent whole (large): snapshot 1.50, compare 1.90, encode 0.90, apply 6.60"),
        "{lines}"
    );
    assert!(lines.contains("1 unchanged field: snapshot 0.10, compare 0.10"), "{lines}");
    assert!(lines.contains("to UI 0.20, apply 6.80"), "{lines}");
    assert!(lines.contains("on screen +26.8"), "{lines}");
    let report = collector.report(10, 41 * MS);
    assert_eq!(report.interactions.len(), 1);
    let interaction = &report.interactions[0];
    assert_eq!(interaction.control.as_deref(), Some("Library.add"));
    assert_eq!(interaction.total_us, 39_000);
    assert_eq!(interaction.revision, Some(5));
    let segment = |name: &str| {
        interaction
            .segments
            .iter()
            .find(|segment| segment.name == name)
            .map(|segment| segment.us)
    };
    assert_eq!(segment("queue"), Some(200));
    assert_eq!(segment("apply"), Some(6_800));
    assert_eq!(segment("to_frame"), Some(26_800));
    let command = &report.commands[0];
    assert_eq!(command.id, "tracetest.add_item");
    assert_eq!(command.entry, "ui");
    assert_eq!(command.exec_us.max, 200);
    assert_eq!(command.bytes.max, 1_310_720);
    let field = &report.fields[0];
    assert_eq!(field.name, "items");
    assert_eq!(field.items_max, Some(10_001));
    assert_eq!(field.apply_us.max, 6_600);
}

#[test]
fn engine_only_commands_print_at_once() {
    let records: Vec<Record> = interaction_records()
        .into_iter()
        .filter(|record| !matches!(record.kind, kind::ACT | kind::DISPATCH | kind::DECODE))
        .filter(|record| record.track != 1)
        .map(|mut record| {
            if record.kind == kind::QUEUE {
                record.extra = entry::CLI;
            }
            record
        })
        .collect();
    let mut collector = new_collector("engine");
    collector.process(records, Vec::new(), 0, &[], 7 * MS);
    let lines = collector.take_lines();
    assert!(
        lines[0].starts_with("bern-trace: command tracetest.add_item (cli) 5.00 ms: queue 0.20"),
        "{lines:?}"
    );
}

#[test]
fn slow_hides_fast_blocks_and_control_filters_interactions() {
    let mut collector = new_collector("all,slow:50ms");
    collector.process(interaction_records(), Vec::new(), 0, &[], 41 * MS);
    assert!(collector.take_lines().is_empty(), "39 ms is under 50 ms");
    assert_eq!(collector.report(10, 41 * MS).interactions.len(), 1, "still reported");
    let mut filtered = new_collector("all,control:Other.*");
    filtered.process(interaction_records(), Vec::new(), 0, &[], 41 * MS);
    assert!(filtered.take_lines().is_empty());
    assert!(filtered.report(10, 41 * MS).interactions.is_empty());
}

#[test]
fn long_frames_list_bern_work_and_what_is_outside() {
    let ui = |mut record: Record| {
        record.track = 1;
        record
    };
    let frame = |t: u64| {
        ui(Record {
            a: 16_666_667,
            ..rec(kind::FRAME, t, t + 10, 0)
        })
    };
    let records = vec![
        frame(0),
        ui(Record {
            a: 100,
            ..rec(kind::RECEIVE_STATE, 200, 500, 9)
        }),
        Record {
            a: 10,
            ..rec(kind::PUBLISH, 100, 150, 99)
        },
        frame(600),
        frame(767),
    ];
    let mut collector = new_collector("all");
    collector.process(records, Vec::new(), 0, &[], 200 * MS);
    let lines = collector.take_lines().join("\n");
    assert!(
        lines.contains("long frame 60.0 ms at 0.06 s (refresh 16.7 ms): Bern 30.0 (apply rev 9 30.0), outside Bern 30.0; engine meanwhile: publish rev 10 5.00"),
        "{lines}"
    );
    let report = collector.report(10, 200 * MS);
    assert_eq!(report.frames.long, 1);
    assert_eq!(report.long_frames[0].bern_us, 30_000);
    assert_eq!(report.long_frames[0].outside_us, 30_000);
}

#[test]
fn frequent_commands_fold_into_one_line_a_second() {
    let mut collector = new_collector("engine");
    let mut records = Vec::new();
    for job in 0..30u64 {
        let t = job * 10;
        records.push(Record {
            extra: entry::CLI,
            name: 1,
            ..rec(kind::QUEUE, t, t + 1, 100 + job)
        });
        records.push(Record {
            a: 1,
            ..rec(kind::EXEC, t + 1, t + 2, 100 + job)
        });
        records.push(rec(kind::PUBLISH, t + 2, t + 3, 100 + job));
    }
    collector.process(records, Vec::new(), 0, &[], 31 * MS);
    let lines = collector.take_lines();
    assert_eq!(lines.len(), 10, "{lines:?}");
    collector.process(Vec::new(), Vec::new(), 0, &[], 2_000 * MS);
    let lines = collector.take_lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("tracetest.noop x20 more within a second"), "{lines:?}");
}

#[test]
fn reports_match_the_session_catalog() {
    let mut collector = new_collector("all");
    collector.process(interaction_records(), Vec::new(), 0, &[], 41 * MS);
    let (types, specs) = session_catalog();
    let result = |id: &str| specs.iter().find(|spec| spec.id == id).unwrap().result.clone();
    collector
        .report(10, 41 * MS)
        .to_value()
        .check(&result("trace.report"), &types)
        .unwrap();
    Report::default()
        .to_value()
        .check(&result("trace.report"), &types)
        .unwrap();
    Started {
        rules: "engine".into(),
        file: Some("/tmp/x.json".into()),
    }
    .to_value()
    .check(&result("trace.start"), &types)
    .unwrap();
    Stopped {
        files: vec!["/tmp/x.json".into()],
        events: 3,
        dropped: 0,
        timeline: None,
    }
    .to_value()
    .check(&result("trace.stop"), &types)
    .unwrap();
}

// ---------------------------------------------------------------------------
// A real engine, traced end to end

struct Items(Vec<Value>);

impl EngineProtocol for Items {
    fn descriptor(&self) -> &'static ProtocolDescriptor {
        descriptor()
    }
    fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
        if invocation.command == 0 {
            let _span = crate::trace_span!("make item");
            self.0.push(Value::Int(self.0.len() as i64));
        }
        Ok(Value::Unit)
    }
    fn snapshot(&self) -> Vec<Value> {
        vec![
            snapshot_field(0, || Value::List(self.0.clone())),
            snapshot_field(1, || Value::Int(self.0.len() as i64)),
        ]
    }
}

#[test]
fn a_traced_engine_reports_and_writes_a_timeline() {
    let _global = global();
    let dir = std::env::temp_dir().join(format!("bern-trace-test-{}", std::process::id()));
    let file = dir.join("timeline.json");
    let started = start(&format!(
        "engine,command:tracetest.*,quiet,file:{}",
        file.display()
    ))
    .unwrap();
    assert_eq!(started.file.as_deref(), Some(file.display().to_string().as_str()));
    let host = Host::start(Items(Vec::new()));
    for _ in 0..3 {
        host.dispatch_blocking(
            Invocation {
                command: 0,
                params: vec![],
            },
            context(),
        );
    }
    host.dispatch_blocking(
        Invocation {
            command: 1,
            params: vec![],
        },
        context(),
    );
    let report = report(10, false);
    let add = report
        .commands
        .iter()
        .find(|command| command.id == "tracetest.add_item")
        .expect("add_item reported");
    assert_eq!(add.exec_us.count, 3);
    assert_eq!(add.entry, "test");
    // Changed fields count without a UI to send them to.
    assert!(report.fields.iter().any(|field| field.name == "items" && field.sent == 3));
    assert!(report.fields.iter().all(|field| field.bytes_max == 0));
    assert!(report.spans.iter().any(|span| span.name == "make item" && span.us.count == 3));
    let stopped = stop(true);
    host.stop();
    assert_eq!(mask(), 0);
    assert_eq!(stopped.files, vec![file.display().to_string()]);
    let timeline: serde_json::Value =
        serde_json::from_str(stopped.timeline.as_deref().unwrap()).expect("valid JSON");
    let events = timeline.as_array().unwrap();
    let named = |name: &str| events.iter().any(|event| event["name"] == name);
    assert!(named("tracetest.add_item"));
    assert!(named("snapshot items"));
    assert!(named("compare items"));
    assert!(named("publish rev 3"));
    assert!(named("make item"));
    assert!(events.iter().any(|event| event["name"] == "thread_name"
        && event["args"]["name"] == "bern-engine"));
    // Untraced again: nothing more is recorded.
    host.dispatch_blocking(
        Invocation {
            command: 0,
            params: vec![],
        },
        context(),
    );
    assert_eq!(report.dropped, 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn filtered_commands_are_not_traced() {
    let _global = global();
    start("engine,command:tracetest.noop,quiet,file:none").unwrap();
    let host = Host::start(Items(Vec::new()));
    for command in [0, 1, 1] {
        host.dispatch_blocking(
            Invocation {
                command,
                params: vec![],
            },
            context(),
        );
    }
    let report = report(10, false);
    stop(false);
    host.stop();
    let ids: Vec<&str> = report.commands.iter().map(|command| command.id.as_str()).collect();
    assert_eq!(ids, ["tracetest.noop"]);
    assert_eq!(report.commands[0].exec_us.count, 2);
}
