use super::*;
use bern_protocol::{
    CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, ExternalNames, OperationKind, ParamSpec, Scope,
    StateField, Type, TypeTable,
};
use std::sync::OnceLock;

fn descriptor() -> &'static ProtocolDescriptor {
    static DESCRIPTOR: OnceLock<ProtocolDescriptor> = OnceLock::new();
    DESCRIPTOR.get_or_init(|| {
        let command = |name: &str, params: Vec<ParamSpec>| CommandSpec {
            id: format!("counter.{name}"),
            name: name.into(),
            doc: String::new(),
            kind: OperationKind::Command,
            scope: Scope::Engine,
            params,
            result: Type::Unit,
            errors: vec![],
            policy: CommandPolicy::default(),
            external: ExternalNames::derive("counter", name),
            reads: vec![],
            on: None,
        };
        let descriptor = ProtocolDescriptor {
            modules: Vec::new(),
            events: Vec::new(),
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "counter".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![StateField {
                name: "value".into(),
                ty: Type::Int,
                doc: String::new(),
            }],
            commands: vec![
                command(
                    "add",
                    vec![ParamSpec {
                        name: "amount".into(),
                        ty: Type::Int,
                        doc: String::new(),
                    }],
                ),
                command("noop", vec![]),
                command("explode", vec![]),
            ],
        };
        descriptor.validate().unwrap();
        descriptor
    })
}

struct Counter(i64);

impl EngineProtocol for Counter {
    fn descriptor(&self) -> &'static ProtocolDescriptor {
        descriptor()
    }
    fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
        match invocation.command {
            0 => {
                let Value::Int(amount) = invocation.params[0] else {
                    unreachable!()
                };
                self.0 = self
                    .0
                    .checked_add(amount)
                    .ok_or_else(|| CommandError::new("overflow", "counter overflow"))?;
                Ok(Value::Unit)
            }
            1 => Ok(Value::Unit),
            _ => panic!("boom"),
        }
    }
    fn snapshot(&self) -> Vec<Value> {
        vec![Value::Int(self.0)]
    }
}

fn context() -> CallContext {
    CallContext {
        entry: Entry::Test,
        request_id: "t".into(),
    }
}

#[test]
fn commands_update_state_with_monotonic_revisions() {
    let host = Host::start(Counter(0));
    let (sender, receiver) = std::sync::mpsc::channel();
    let _subscription = host.subscribe(move |update| {
        sender.send(update.clone()).unwrap();
    });
    let outcome = host.dispatch_blocking(
        Invocation {
            command: 0,
            params: vec![Value::Int(2)],
        },
        context(),
    );
    assert_eq!(outcome.result, Ok(Value::Unit));
    assert_eq!(outcome.revision, 1);
    let update = receiver.recv().unwrap();
    assert_eq!(update.revision, 1);
    assert_eq!(update.changes, vec![(0, Value::Int(2))]);
    // A command that changes nothing does not create a revision.
    let outcome = host.dispatch_blocking(
        Invocation {
            command: 1,
            params: vec![],
        },
        context(),
    );
    assert_eq!(outcome.revision, 1);
    assert_eq!(host.snapshot(), (1, vec![Value::Int(2)]));
}

#[test]
fn invalid_parameters_are_rejected_before_the_engine_runs() {
    let host = Host::start(Counter(0));
    let outcome = host.dispatch_blocking(
        Invocation {
            command: 0,
            params: vec![Value::Text("2".into())],
        },
        context(),
    );
    assert_eq!(outcome.result.unwrap_err().code, "invalid_params");
    let outcome = host.dispatch_blocking(
        Invocation {
            command: 0,
            params: vec![],
        },
        context(),
    );
    assert_eq!(outcome.result.unwrap_err().code, "invalid_params");
    assert_eq!(host.snapshot().1, vec![Value::Int(0)]);
}

#[test]
fn engine_errors_keep_state_and_panics_poison_the_engine() {
    let host = Host::start(Counter(i64::MAX));
    let outcome = host.dispatch_blocking(
        Invocation {
            command: 0,
            params: vec![Value::Int(1)],
        },
        context(),
    );
    assert_eq!(outcome.result.unwrap_err().code, "overflow");
    assert_eq!(host.snapshot().1, vec![Value::Int(i64::MAX)]);
    let outcome = host.dispatch_blocking(
        Invocation {
            command: 2,
            params: vec![],
        },
        context(),
    );
    assert_eq!(outcome.result.unwrap_err().code, "engine_failed");
    let outcome = host.dispatch_blocking(
        Invocation {
            command: 1,
            params: vec![],
        },
        context(),
    );
    assert_eq!(outcome.result.unwrap_err().code, "engine_failed");
}

#[test]
fn a_local_host_runs_jobs_on_the_calling_thread_in_order() {
    let host = Host::start_local(Counter(0));
    let add = |amount: i64| Invocation {
        command: 0,
        params: vec![Value::Int(amount)],
    };
    // A listener that dispatches again: the nested command waits until the
    // running one has finished and published.
    let seen = Arc::new(Mutex::new(Vec::new()));
    let nested = host.clone();
    let log = Arc::clone(&seen);
    let _subscription = host.subscribe(move |update| {
        log.lock().unwrap().push(update.revision);
        if update.revision == 1 {
            nested.dispatch(add(10), context(), |_| {});
        }
    });
    let replied = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&replied);
    host.dispatch(add(1), context(), move |outcome| {
        assert_eq!(outcome.revision, 1);
        flag.store(true, Ordering::SeqCst);
    });
    // Everything ran before `dispatch` returned.
    assert!(replied.load(Ordering::SeqCst));
    assert_eq!(*seen.lock().unwrap(), vec![1, 2]);
    assert_eq!(host.snapshot(), (2, vec![Value::Int(11)]));
    assert_eq!(
        host.dispatch_blocking(add(1), context()).result,
        Ok(Value::Unit)
    );
    host.stop();
    let outcome = host.dispatch_blocking(add(1), context());
    assert_eq!(outcome.result.unwrap_err().code, "engine_stopped");
    assert_eq!(host.snapshot().1, vec![Value::Int(12)]);
}

#[test]
fn dropping_a_subscription_stops_updates() {
    let host = Host::start(Counter(0));
    let (sender, receiver) = std::sync::mpsc::channel();
    let subscription = host.subscribe(move |update| {
        let _ = sender.send(update.revision);
    });
    drop(subscription);
    host.dispatch_blocking(
        Invocation {
            command: 0,
            params: vec![Value::Int(1)],
        },
        context(),
    );
    assert!(receiver.try_recv().is_err());
}

/// A counter with read-only queries whose answers the test releases.
mod queries {
    use super::*;
    use std::time::Duration;

    fn descriptor() -> &'static ProtocolDescriptor {
        static DESCRIPTOR: OnceLock<ProtocolDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| {
            let spec = |name: &str, kind: OperationKind, params: Vec<ParamSpec>| CommandSpec {
                id: format!("counter.{name}"),
                name: name.into(),
                doc: String::new(),
                kind,
                scope: Scope::Engine,
                params,
                result: if kind == OperationKind::Query {
                    Type::Int
                } else {
                    Type::Unit
                },
                errors: vec![],
                policy: CommandPolicy::default(),
                external: ExternalNames::derive("counter", name),
                reads: if kind == OperationKind::Query {
                    vec!["value".into()]
                } else {
                    vec![]
                },
                on: None,
            };
            let amount = || {
                vec![ParamSpec {
                    name: "amount".into(),
                    ty: Type::Int,
                    doc: String::new(),
                }]
            };
            let descriptor = ProtocolDescriptor {
                modules: Vec::new(),
                events: Vec::new(),
                descriptor_version: DESCRIPTOR_VERSION,
                namespace: "counter".into(),
                version: 1,
                types: TypeTable::default(),
                state: vec![StateField {
                    name: "value".into(),
                    ty: Type::Int,
                    doc: String::new(),
                }],
                commands: vec![
                    spec("add", OperationKind::Command, amount()),
                    // Answers `value + amount`; 0 answers at once, others wait
                    // until the test releases them.
                    spec("peek", OperationKind::Query, amount()),
                    spec("wrong", OperationKind::Query, vec![]),
                    spec("crash", OperationKind::Query, vec![]),
                ],
            };
            descriptor.validate().unwrap();
            descriptor
        })
    }

    type Parked = Arc<Mutex<Vec<(i64, QueryAnswer)>>>;

    struct Counter {
        value: i64,
        parked: Parked,
    }

    impl EngineProtocol for Counter {
        fn descriptor(&self) -> &'static ProtocolDescriptor {
            descriptor()
        }
        fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
            let Value::Int(amount) = invocation.params[0] else {
                unreachable!()
            };
            self.value += amount;
            Ok(Value::Unit)
        }
        fn query(&self, invocation: &Invocation, answer: QueryAnswer) {
            match invocation.command {
                1 => {
                    let Value::Int(amount) = invocation.params[0] else {
                        unreachable!()
                    };
                    if amount == 0 {
                        answer.send(Ok(Value::Int(self.value)));
                    } else {
                        self.parked
                            .lock()
                            .unwrap()
                            .push((self.value + amount, answer));
                    }
                }
                2 => answer.send(Ok(Value::Text("not an int".into()))),
                _ => panic!("boom"),
            }
        }
        fn snapshot(&self) -> Vec<Value> {
            vec![Value::Int(self.value)]
        }
    }

    fn start() -> (Host, Parked) {
        let parked = Parked::default();
        let host = Host::start(Counter {
            value: 0,
            parked: Arc::clone(&parked),
        });
        (host, parked)
    }

    fn call(command: usize, params: Vec<Value>) -> Invocation {
        Invocation { command, params }
    }

    fn wait_parked(parked: &Parked, count: usize) {
        for _ in 0..500 {
            if parked.lock().unwrap().len() >= count {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("the provider did not start");
    }

    #[test]
    fn queries_read_state_after_earlier_commands_without_changing_it() {
        let (host, _) = start();
        assert!(host.is_query(1) && !host.is_query(0));
        host.dispatch(call(0, vec![Value::Int(5)]), context(), |_| {});
        let outcome = host.dispatch_blocking(call(1, vec![Value::Int(0)]), context());
        assert_eq!(outcome.result, Ok(Value::Int(5)));
        assert_eq!(outcome.revision, 1);
        assert_eq!(host.snapshot(), (1, vec![Value::Int(5)]));
    }

    #[test]
    fn answers_arrive_later_from_another_thread_in_any_order() {
        let (host, parked) = start();
        let (sender, receiver) = channel();
        for amount in [1, 2] {
            let sender = sender.clone();
            host.dispatch(
                call(1, vec![Value::Int(amount)]),
                context(),
                move |outcome| {
                    sender.send(outcome.result).unwrap();
                },
            );
        }
        wait_parked(&parked, 2);
        // The engine keeps serving while both queries are pending.
        host.dispatch_blocking(call(0, vec![Value::Int(10)]), context());
        let mut answers = std::mem::take(&mut *parked.lock().unwrap());
        let (second, answer) = answers.pop().unwrap();
        thread::spawn(move || answer.send(Ok(Value::Int(second))))
            .join()
            .unwrap();
        assert_eq!(receiver.recv().unwrap(), Ok(Value::Int(2)));
        let (first, answer) = answers.pop().unwrap();
        answer.send(Ok(Value::Int(first)));
        assert_eq!(receiver.recv().unwrap(), Ok(Value::Int(1)));
    }

    #[test]
    fn cancelled_dropped_and_invalid_answers_fail_clearly() {
        let (host, parked) = start();
        let (sender, receiver) = channel();
        let cancellation = Cancellation::new();
        let reply = sender.clone();
        host.dispatch_cancellable(
            call(1, vec![Value::Int(1)]),
            context(),
            cancellation.clone(),
            move |outcome| reply.send(outcome.result).unwrap(),
        );
        wait_parked(&parked, 1);
        let (_, answer) = parked.lock().unwrap().pop().unwrap();
        assert!(!answer.is_cancelled());
        cancellation.cancel();
        assert!(answer.is_cancelled());
        // A provider that still answers is reported as cancelled.
        answer.send(Ok(Value::Int(1)));
        assert_eq!(receiver.recv().unwrap().unwrap_err().code, "cancelled");

        let reply = sender.clone();
        host.dispatch(call(1, vec![Value::Int(1)]), context(), move |outcome| {
            reply.send(outcome.result).unwrap()
        });
        wait_parked(&parked, 1);
        drop(parked.lock().unwrap().pop());
        assert_eq!(receiver.recv().unwrap().unwrap_err().code, "query_dropped");

        let outcome = host.dispatch_blocking(call(2, vec![]), context());
        assert_eq!(outcome.result.unwrap_err().code, "invalid_result");
        let outcome = host.dispatch_blocking(call(1, vec![Value::Text("x".into())]), context());
        assert_eq!(outcome.result.unwrap_err().code, "invalid_params");

        // A query that never answers times out and is cancelled.
        let outcome = host.query_blocking(
            call(1, vec![Value::Int(3)]),
            context(),
            Duration::from_millis(50),
        );
        assert_eq!(outcome.result.unwrap_err().code, "query_timeout");
        wait_parked(&parked, 1);
        assert!(parked.lock().unwrap()[0].1.is_cancelled());
    }

    #[test]
    fn a_panicking_query_poisons_the_engine() {
        let (host, _) = start();
        let outcome = host.dispatch_blocking(call(3, vec![]), context());
        assert_eq!(outcome.result.unwrap_err().code, "engine_failed");
        let outcome = host.dispatch_blocking(call(0, vec![Value::Int(1)]), context());
        assert_eq!(outcome.result.unwrap_err().code, "engine_failed");
    }
}

mod composite {
    use super::*;
    use crate::{Composite, ModuleEngine};
    use std::sync::Mutex;

    fn tally(optional: bool, enabled: bool) -> ModuleEngine {
        ModuleEngine {
            descriptor: tally_descriptor(),
            start: Box::new(|| Box::new(Tally(0))),
            optional,
            enabled,
            depends: Vec::new(),
        }
    }

    fn tally_descriptor() -> &'static ProtocolDescriptor {
        static DESCRIPTOR: OnceLock<ProtocolDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ProtocolDescriptor {
            modules: Vec::new(),
            events: Vec::new(),
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "tally".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![StateField {
                name: "count".into(),
                ty: Type::Int,
                doc: String::new(),
            }],
            commands: vec![CommandSpec {
                id: "tally.bump".into(),
                name: "bump".into(),
                doc: String::new(),
                kind: OperationKind::Command,
                scope: Scope::Engine,
                params: vec![],
                result: Type::Int,
                errors: vec![],
                policy: CommandPolicy::default(),
                external: ExternalNames::derive("tally", "bump"),
                reads: vec![],
                on: None,
            }],
        })
    }

    /// A module engine counting its own bumps.
    struct Tally(i64);

    impl EngineProtocol for Tally {
        fn descriptor(&self) -> &'static ProtocolDescriptor {
            tally_descriptor()
        }
        fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
            assert_eq!(invocation.command, 0, "commands arrive with module indices");
            self.0 += 1;
            Ok(Value::Int(self.0))
        }
        fn snapshot(&self) -> Vec<Value> {
            vec![Value::Int(self.0)]
        }
    }

    #[test]
    fn modules_answer_their_own_commands_inside_one_engine() {
        let engine = Composite::new(Box::new(Counter(5)), vec![tally(false, true)]).unwrap();
        let host = Host::start(engine);
        let descriptor = host.descriptor();
        assert_eq!(descriptor.modules, ["tally"]);
        let names: Vec<_> = descriptor.state.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["value", "tally"]);

        let bump = host.resolve("tally.bump").unwrap();
        assert_eq!(bump, 3, "module commands follow the shell's three");
        let (sender, receiver) = std::sync::mpsc::channel();
        let _subscription = host.subscribe(move |update| {
            sender.send(update.clone()).unwrap();
        });
        let outcome = host.dispatch_blocking(
            Invocation {
                command: bump,
                params: vec![],
            },
            context(),
        );
        assert_eq!(outcome.result, Ok(Value::Int(1)));
        let update = receiver.recv().unwrap();
        assert_eq!(
            update.changes,
            vec![(1, Value::Record(vec![Value::Int(1)]))]
        );

        let add = host.resolve("counter.add").unwrap();
        let outcome = host.dispatch_blocking(
            Invocation {
                command: add,
                params: vec![Value::Int(2)],
            },
            context(),
        );
        assert_eq!(outcome.result, Ok(Value::Unit));
        assert_eq!(
            host.snapshot().1,
            vec![Value::Int(7), Value::Record(vec![Value::Int(1)])]
        );
    }

    #[test]
    fn a_module_cannot_reuse_the_shell_namespace() {
        struct Shadow;
        impl EngineProtocol for Shadow {
            fn descriptor(&self) -> &'static ProtocolDescriptor {
                descriptor()
            }
            fn invoke(&mut self, _: &Invocation) -> Result<Value, CommandError> {
                Ok(Value::Unit)
            }
            fn snapshot(&self) -> Vec<Value> {
                vec![Value::Int(0)]
            }
        }
        let shadow = ModuleEngine {
            descriptor: descriptor(),
            start: Box::new(|| Box::new(Shadow)),
            optional: false,
            enabled: true,
            depends: Vec::new(),
        };
        assert!(Composite::new(Box::new(Counter(0)), vec![shadow]).is_err());
    }

    /// A shell remembering which optional modules run.
    struct Remembering {
        saved: Option<Vec<String>>,
        changes: std::sync::Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl EngineProtocol for Remembering {
        fn descriptor(&self) -> &'static ProtocolDescriptor {
            descriptor()
        }
        fn invoke(&mut self, _: &Invocation) -> Result<Value, CommandError> {
            Ok(Value::Unit)
        }
        fn snapshot(&self) -> Vec<Value> {
            vec![Value::Int(0)]
        }
        fn initial_modules(&self) -> Option<Vec<String>> {
            self.saved.clone()
        }
        fn modules_changed(&mut self, enabled: &[String]) {
            self.changes.lock().unwrap().push(enabled.to_vec());
        }
    }

    #[test]
    fn optional_modules_switch_off_and_on() {
        let changes = std::sync::Arc::new(Mutex::new(Vec::new()));
        let shell = Remembering {
            saved: None,
            changes: changes.clone(),
        };
        let engine = Composite::new(Box::new(shell), vec![tally(true, true)]).unwrap();
        let host = Host::start(engine);
        let bump = host.resolve("tally.bump").unwrap();
        let switch = host.resolve("modules.set_enabled").unwrap();
        let call = |command: usize, params: Vec<Value>| {
            host.dispatch_blocking(Invocation { command, params }, context())
                .result
        };
        assert_eq!(call(bump, vec![]), Ok(Value::Int(1)));
        // Off: the commands answer `module_disabled`, the state is idle.
        assert_eq!(
            call(
                switch,
                vec![Value::Text("tally".into()), Value::Bool(false)]
            ),
            Ok(Value::List(vec![Value::Text("tally".into())]))
        );
        assert_eq!(call(bump, vec![]).unwrap_err().code, "module_disabled");
        assert_eq!(
            host.snapshot().1,
            vec![
                Value::Int(0),
                Value::Record(vec![Value::Int(0)]),
                Value::Record(vec![Value::Bool(false)])
            ]
        );
        // On again: a new engine, starting over.
        call(switch, vec![Value::Text("tally".into()), Value::Bool(true)]).unwrap();
        assert_eq!(call(bump, vec![]), Ok(Value::Int(1)));
        // Switching to the current setting changes nothing.
        assert_eq!(
            call(switch, vec![Value::Text("tally".into()), Value::Bool(true)]),
            Ok(Value::List(vec![]))
        );
        assert_eq!(
            call(switch, vec![Value::Text("other".into()), Value::Bool(true)])
                .unwrap_err()
                .code,
            "unknown_module"
        );
        assert_eq!(
            *changes.lock().unwrap(),
            vec![Vec::<String>::new(), vec!["tally".to_string()]]
        );
    }

    #[test]
    fn saved_choices_win_over_defaults() {
        let shell = Remembering {
            saved: Some(vec![]),
            changes: Default::default(),
        };
        let engine = Composite::new(Box::new(shell), vec![tally(true, true)]).unwrap();
        assert_eq!(
            engine.snapshot()[2],
            Value::Record(vec![Value::Bool(false)])
        );
    }
}
