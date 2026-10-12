//! An application engine made of its shell and its modules.
//!
//! Every part is the engine its own protocol macro adapted, with its own
//! catalog. [`Composite`] presents them as one engine: one catalog from
//! [`bern_protocol::compose`], commands routed to the part that declares
//! them, and one state snapshot in which each module's state is a record.
//!
//! An optional module runs only while it is switched on: switching it off
//! drops its engine, and switching it on creates a new one. While it is
//! off its commands answer `module_disabled` and its state reads as the
//! default values.

use crate::outbox::{self, Pending};
use crate::{EngineProtocol, QueryAnswer};
use std::collections::{BTreeMap, VecDeque};

/// How many commands one command may set off through calls and events
/// before the rest are dropped as a loop.
const FOLLOW_UP_LIMIT: usize = 10_000;
use bern_protocol::{
    CommandError, DescriptorError, Invocation, ModulePart, ProtocolDescriptor, Type, Value,
    command_offsets, compose, module_state_type,
};

/// How the application runs one of its modules.
pub struct ModuleEngine {
    /// The module's catalog, so an optional module that is off never needs
    /// an engine.
    pub descriptor: &'static ProtocolDescriptor,
    /// Creates the module's engine; called again each time an optional
    /// module is switched on.
    pub start: Box<dyn Fn() -> Box<dyn EngineProtocol> + Send>,
    /// Whether the module can be switched off.
    pub optional: bool,
    /// Whether an optional module runs when the application saved no
    /// choice.
    pub enabled: bool,
    /// Ids of the modules this one depends on.
    pub depends: Vec<String>,
}

struct Module {
    id: String,
    descriptor: &'static ProtocolDescriptor,
    entry: ModuleEngine,
    engine: Option<Box<dyn EngineProtocol>>,
    /// Its state while it is switched off.
    idle: Value,
}

pub struct Composite {
    descriptor: &'static ProtocolDescriptor,
    shell: Box<dyn EngineProtocol>,
    modules: Vec<Module>,
    /// Where each part's commands start: the shell, then every module.
    offsets: Vec<usize>,
    /// Index of `modules.set_enabled`, when some module is optional.
    switch: Option<usize>,
    /// `namespace.event` to the commands answering it, in catalog order.
    subscribers: BTreeMap<String, Vec<usize>>,
}

impl Composite {
    /// Compose `shell` and `modules`, in the order the application lists
    /// its modules. Fails when the catalogs cannot form one application.
    pub fn new(
        shell: Box<dyn EngineProtocol>,
        modules: Vec<ModuleEngine>,
    ) -> Result<Self, DescriptorError> {
        let module_parts: Vec<ModulePart<'_>> = modules
            .iter()
            .map(|entry| ModulePart {
                descriptor: entry.descriptor,
                optional: entry.optional,
            })
            .collect();
        let composed = compose(shell.descriptor(), &module_parts)?;
        let offsets = command_offsets(shell.descriptor(), &module_parts);
        let switch = modules
            .iter()
            .any(|entry| entry.optional)
            .then(|| composed.commands.len() - 1);
        // One composite exists per engine instance, usually one per process.
        let descriptor: &'static ProtocolDescriptor = Box::leak(Box::new(composed));
        let saved = shell.initial_modules();
        let mut parts = Vec::new();
        for entry in modules {
            let id = entry.descriptor.namespace.clone();
            let idle = Value::default_for(
                &Type::Record {
                    name: module_state_type(&id),
                },
                &descriptor.types,
            )
            .map_err(|error| DescriptorError(error.message))?;
            let running = !entry.optional
                || saved
                    .as_ref()
                    .map_or(entry.enabled, |saved| saved.contains(&id));
            parts.push(Module {
                id,
                descriptor: entry.descriptor,
                engine: running.then(|| (entry.start)()),
                entry,
                idle,
            });
        }
        let mut subscribers: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, command) in descriptor.commands.iter().enumerate() {
            if let Some(event) = &command.on {
                subscribers.entry(event.clone()).or_default().push(index);
            }
        }
        let mut composite = Self {
            descriptor,
            shell,
            modules: parts,
            offsets,
            switch,
            subscribers,
        };
        // A running module's dependencies run too.
        let running: Vec<String> = composite
            .modules
            .iter()
            .filter(|module| module.engine.is_some())
            .map(|module| module.id.clone())
            .collect();
        for id in running {
            composite.switch_on(&id, &mut Vec::new());
        }
        Ok(composite)
    }

    /// The part declaring `command` (0 for the shell, then modules from 1)
    /// and the command's index in its catalog.
    fn route(&self, command: usize) -> Option<(usize, usize)> {
        let part = self.offsets.iter().rposition(|offset| *offset <= command)?;
        let local = command - self.offsets[part];
        let count = match part {
            0 => self.shell.descriptor().commands.len(),
            module => self.modules[module - 1].descriptor.commands.len(),
        };
        (local < count).then_some((part, local))
    }

    /// The states a part may read while it handles a command: the shell
    /// reads every running module, a module the running modules it
    /// depends on.
    fn readable(&self, part: usize) -> BTreeMap<String, Vec<Value>> {
        let allowed = |module: &Module| match part {
            0 => true,
            part => self.modules[part - 1].entry.depends.contains(&module.id),
        };
        self.modules
            .iter()
            .filter(|module| allowed(module))
            .filter_map(|module| {
                let engine = module.engine.as_ref()?;
                Some((module.id.clone(), engine.snapshot()))
            })
            .collect()
    }

    /// Run one command and collect what it sent.
    fn run(
        &mut self,
        command: usize,
        params: &[Value],
    ) -> (Result<Value, CommandError>, Vec<Pending>) {
        if Some(command) == self.switch {
            let (Some(Value::Text(id)), Some(Value::Bool(enabled))) =
                (params.first(), params.get(1))
            else {
                return (
                    Err(CommandError::new(
                        "invalid_params",
                        "expected an id and a bool",
                    )),
                    Vec::new(),
                );
            };
            let id = id.clone();
            return (self.set_enabled(&id, *enabled), Vec::new());
        }
        let Some((part, local)) = self.route(command) else {
            return (Err(unknown()), Vec::new());
        };
        let local = Invocation {
            command: local,
            params: params.to_vec(),
        };
        outbox::begin(self.readable(part));
        let result = if part == 0 {
            self.shell.invoke(&local)
        } else {
            let module = &mut self.modules[part - 1];
            match &mut module.engine {
                Some(engine) => engine.invoke(&local),
                None => Err(disabled(&module.id)),
            }
        };
        (result, outbox::take())
    }

    /// The commands a pending call or event runs, with their parameters.
    fn resolve(&self, pending: Pending) -> Vec<(usize, Vec<Value>)> {
        match pending {
            Pending::Call {
                namespace,
                command,
                params,
            } => {
                let part = if namespace == self.shell.descriptor().namespace {
                    Some(0)
                } else {
                    self.position(&namespace).map(|index| index + 1)
                };
                part.map(|part| vec![(self.offsets[part] + command, params)])
                    .unwrap_or_default()
            }
            Pending::Event {
                namespace,
                event,
                params,
            } => self
                .subscribers
                .get(&format!("{namespace}.{event}"))
                .map(|commands| {
                    commands
                        .iter()
                        // An event never reaches a module that is off.
                        .filter(|command| match self.route(**command) {
                            Some((0, _)) => true,
                            Some((part, _)) => self.modules[part - 1].engine.is_some(),
                            None => false,
                        })
                        .map(|command| (*command, params.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn position(&self, id: &str) -> Option<usize> {
        self.modules.iter().position(|module| module.id == id)
    }

    /// Run `id` and every module it depends on, adding those it started to
    /// `changed`.
    fn switch_on(&mut self, id: &str, changed: &mut Vec<String>) {
        let Some(index) = self.position(id) else {
            return;
        };
        for dependency in self.modules[index].entry.depends.clone() {
            self.switch_on(&dependency, changed);
        }
        let module = &mut self.modules[index];
        if module.engine.is_none() {
            module.engine = Some((module.entry.start)());
            changed.push(module.id.clone());
        }
    }

    /// Stop `id` and every module depending on it, adding those it stopped
    /// to `changed`.
    fn switch_off(&mut self, id: &str, changed: &mut Vec<String>) {
        let dependents: Vec<String> = self
            .modules
            .iter()
            .filter(|module| {
                module
                    .entry
                    .depends
                    .iter()
                    .any(|dependency| dependency == id)
            })
            .map(|module| module.id.clone())
            .collect();
        for dependent in dependents {
            self.switch_off(&dependent, changed);
        }
        if let Some(index) = self.position(id) {
            let module = &mut self.modules[index];
            if module.entry.optional && module.engine.take().is_some() {
                changed.push(module.id.clone());
            }
        }
    }

    fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<Value, CommandError> {
        let Some(index) = self.position(id) else {
            return Err(CommandError::new(
                "unknown_module",
                format!("`{id}` is not a module of this application"),
            ));
        };
        if !self.modules[index].entry.optional {
            return Err(CommandError::new(
                "not_optional",
                format!("module `{id}` always runs"),
            ));
        }
        let mut changed = Vec::new();
        if enabled {
            self.switch_on(id, &mut changed);
        } else {
            self.switch_off(id, &mut changed);
        }
        // Report in the application's module order.
        let changed: Vec<String> = self
            .modules
            .iter()
            .map(|module| module.id.clone())
            .filter(|id| changed.contains(id))
            .collect();
        if !changed.is_empty() {
            let running: Vec<String> = self
                .modules
                .iter()
                .filter(|module| module.entry.optional && module.engine.is_some())
                .map(|module| module.id.clone())
                .collect();
            self.shell.modules_changed(&running);
        }
        Ok(Value::List(changed.into_iter().map(Value::Text).collect()))
    }
}

fn unknown() -> CommandError {
    CommandError::new("unknown_command", "unknown command index")
}

fn disabled(id: &str) -> CommandError {
    CommandError::new(
        "module_disabled",
        format!("module `{id}` is switched off; switch it on with `modules.set_enabled`"),
    )
}

impl EngineProtocol for Composite {
    fn descriptor(&self) -> &'static ProtocolDescriptor {
        self.descriptor
    }

    fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
        let (result, sent) = self.run(invocation.command, &invocation.params);
        let result = Ok(result?);
        // What the command sent runs now, in order; what that sends runs
        // after it.
        let mut queue: VecDeque<Pending> = sent.into();
        let mut steps = 0;
        while let Some(pending) = queue.pop_front() {
            for (command, params) in self.resolve(pending) {
                steps += 1;
                if steps > FOLLOW_UP_LIMIT {
                    eprintln!(
                        "bern: `{}` set off more than {FOLLOW_UP_LIMIT} commands through calls and events; the rest are dropped",
                        self.descriptor.commands[invocation.command].id
                    );
                    return result;
                }
                let (outcome, more) = self.run(command, &params);
                if let Err(error) = outcome {
                    eprintln!(
                        "bern: `{}`, set off by `{}`, failed: {}: {}",
                        self.descriptor.commands[command].id,
                        self.descriptor.commands[invocation.command].id,
                        error.code,
                        error.message
                    );
                }
                queue.extend(more);
            }
        }
        result
    }

    fn query(&self, invocation: &Invocation, answer: QueryAnswer) {
        let Some((part, command)) = self.route(invocation.command) else {
            return answer.send(Err(unknown()));
        };
        let local = Invocation {
            command,
            params: invocation.params.clone(),
        };
        if part == 0 {
            return self.shell.query(&local, answer);
        }
        let module = &self.modules[part - 1];
        match &module.engine {
            Some(engine) => engine.query(&local, answer),
            None => answer.send(Err(disabled(&module.id))),
        }
    }

    fn snapshot(&self) -> Vec<Value> {
        let mut state = self.shell.snapshot();
        for module in &self.modules {
            state.push(match &module.engine {
                Some(engine) => Value::Record(engine.snapshot()),
                None => module.idle.clone(),
            });
        }
        if self.switch.is_some() {
            state.push(Value::Record(
                self.modules
                    .iter()
                    .filter(|module| module.entry.optional)
                    .map(|module| Value::Bool(module.engine.is_some()))
                    .collect(),
            ));
        }
        state
    }
}
