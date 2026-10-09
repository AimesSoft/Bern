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
        };
        let descriptor = ProtocolDescriptor {
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
