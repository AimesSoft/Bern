//! The engine and the media hub a frontend talks to.
//!
//! In-process builds hold the engine host and the media hub themselves.
//! The WebUI build reaches those of a separate engine host through its
//! control channel. The C ABI only uses these interfaces, so every build
//! runs the same session runtime, close protocol and query subscriptions.

use bern_command::{CallContext, Host};
use bern_media::{
    MediaCapabilities, MediaCommand, MediaExecutor, MediaHub, MediaUpdate, Observation,
};
use bern_protocol::{
    Cancellation, CommandError, CommandOutcome, Invocation, ProtocolDescriptor, StateUpdate, Value,
};
use std::sync::Arc;

/// Keeps a listener registered until dropped.
pub type Guard = Box<dyn Send + Sync>;

pub type CommandReply = Box<dyn FnOnce(CommandOutcome) + Send>;
pub type MediaReply = Box<dyn FnOnce(Result<Value, CommandError>) + Send>;

/// The engine instance behind a frontend.
pub trait EngineLink: Send + Sync + 'static {
    fn descriptor(&self) -> &'static ProtocolDescriptor;
    fn engine_id(&self) -> String;
    /// Current revision and full state.
    fn snapshot(&self) -> (u64, Vec<Value>);
    /// Run a command or query; `reply` gets its outcome, now or later.
    fn dispatch(
        &self,
        invocation: Invocation,
        context: CallContext,
        cancellation: Cancellation,
        reply: CommandReply,
    );
    /// Receive every state update until the guard is dropped.
    fn subscribe(&self, listener: Box<dyn Fn(&StateUpdate) + Send + Sync>) -> Guard;
    fn stop(&self);
    /// The frontend's open windows changed. A remote engine host lists
    /// them for external entries; an in-process host already sees them.
    fn windows_changed(&self, windows: Vec<String>) {
        let _ = windows;
    }
    /// Whether the engine runs in this process, so its jobs are traced
    /// here.
    fn local(&self) -> bool {
        true
    }
}

impl EngineLink for Host {
    fn descriptor(&self) -> &'static ProtocolDescriptor {
        Host::descriptor(self)
    }

    fn engine_id(&self) -> String {
        Host::engine_id(self).to_string()
    }

    fn snapshot(&self) -> (u64, Vec<Value>) {
        Host::snapshot(self)
    }

    fn dispatch(
        &self,
        invocation: Invocation,
        context: CallContext,
        cancellation: Cancellation,
        reply: CommandReply,
    ) {
        self.dispatch_cancellable(invocation, context, cancellation, reply);
    }

    fn subscribe(&self, listener: Box<dyn Fn(&StateUpdate) + Send + Sync>) -> Guard {
        Box::new(Host::subscribe(self, move |update| listener(update)))
    }

    fn stop(&self) {
        Host::stop(self);
    }
}

/// The media hub merging every player's state.
pub trait MediaLink: Send + Sync + 'static {
    fn snapshot(&self) -> (u64, Vec<Value>);
    fn subscribe(&self, listener: Box<dyn Fn(&MediaUpdate) + Send + Sync>) -> Guard;
    fn dispatch(&self, command: MediaCommand, params: Vec<Value>, reply: MediaReply);
    fn create(&self, player: &str, owner: &str) -> Result<(), CommandError>;
    fn bind(&self, player: &str, url: Option<String>) -> Result<u64, CommandError>;
    fn release(&self, player: &str);
    fn release_window(&self, window: &str);
    fn attach_view(&self, token: &str, player: &str, window: &str);
    fn detach_view(&self, token: &str);
    fn attach_executor(&self, executor: Arc<dyn MediaExecutor>, capabilities: MediaCapabilities);
    fn detach_executor(&self);
    fn complete(&self, request: u64, result: Result<(), CommandError>);
    fn observe(&self, player: &str, version: u64, observation: Observation);
}

impl MediaLink for MediaHub {
    fn snapshot(&self) -> (u64, Vec<Value>) {
        MediaHub::snapshot(self)
    }

    fn subscribe(&self, listener: Box<dyn Fn(&MediaUpdate) + Send + Sync>) -> Guard {
        Box::new(MediaHub::subscribe(self, move |update| listener(update)))
    }

    fn dispatch(&self, command: MediaCommand, params: Vec<Value>, reply: MediaReply) {
        MediaHub::dispatch(self, command, params, reply);
    }

    fn create(&self, player: &str, owner: &str) -> Result<(), CommandError> {
        MediaHub::create(self, player, owner)
    }

    fn bind(&self, player: &str, url: Option<String>) -> Result<u64, CommandError> {
        MediaHub::bind(self, player, url)
    }

    fn release(&self, player: &str) {
        MediaHub::release(self, player);
    }

    fn release_window(&self, window: &str) {
        MediaHub::release_window(self, window);
    }

    fn attach_view(&self, token: &str, player: &str, window: &str) {
        MediaHub::attach_view(self, token, player, window);
    }

    fn detach_view(&self, token: &str) {
        MediaHub::detach_view(self, token);
    }

    fn attach_executor(&self, executor: Arc<dyn MediaExecutor>, capabilities: MediaCapabilities) {
        MediaHub::attach_executor(self, executor, capabilities);
    }

    fn detach_executor(&self) {
        MediaHub::detach_executor(self);
    }

    fn complete(&self, request: u64, result: Result<(), CommandError>) {
        MediaHub::complete(self, request, result);
    }

    fn observe(&self, player: &str, version: u64, observation: Observation) {
        MediaHub::observe(self, player, version, observation);
    }
}
