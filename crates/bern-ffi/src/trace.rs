//! The performance trace at the boundary: the C functions frontends record
//! the UI thread's side with, and the session commands `trace.start`,
//! `trace.stop` and `trace.report`, which the runtime answers itself.
//!
//! Frontends keep their own copy of the category mask, read once at start
//! with [`bern_trace_mask`], and check it before each point; recording
//! goes through [`bern_trace_ui`] with timestamps from [`bern_trace_now`]
//! or the platform clock it reads (see `bern_command::trace::now_ns`).

use super::slice;
use bern_command::trace;
use bern_protocol::{CommandError, SessionCommand, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

pub use bern_command::trace::*;

/// Run a trace session command.
pub(crate) fn run(command: SessionCommand, params: &[Value]) -> Result<Value, CommandError> {
    let flag = |index: usize| match params.get(index) {
        Some(Value::Optional(Some(value))) => matches!(**value, Value::Bool(true)),
        _ => false,
    };
    match command {
        SessionCommand::TraceStart => {
            let Some(Value::Text(rules)) = params.first() else {
                return Err(CommandError::invalid_params("`rules` must be text"));
            };
            trace::start(rules)
                .map(|started| started.to_value())
                .map_err(|message| CommandError::new("invalid_rules", message))
        }
        SessionCommand::TraceStop => Ok(trace::stop(flag(0)).to_value()),
        SessionCommand::TraceReport => {
            let top = match params.first() {
                Some(Value::Optional(Some(value))) => match **value {
                    Value::Int(top) => top.clamp(0, 1000) as usize,
                    _ => 10,
                },
                _ => 10,
            };
            Ok(trace::report(top, flag(1)).to_value())
        }
        _ => Err(CommandError::unknown_command(command.id())),
    }
}

fn utf8<'a>(bytes: *const u8, len: usize) -> Option<&'a str> {
    // SAFETY: the callers' contracts guarantee `len` readable bytes.
    let bytes = unsafe { slice(bytes, len) };
    std::str::from_utf8(bytes).ok().filter(|text| !text.is_empty())
}

/// The trace clock in nanoseconds: the platform's monotonic clock.
#[unsafe(no_mangle)]
pub extern "C" fn bern_trace_now() -> u64 {
    trace::now_ns()
}

/// The categories being recorded (`bern_command::trace` bits); 0 while
/// tracing is off.
#[unsafe(no_mangle)]
pub extern "C" fn bern_trace_mask() -> u32 {
    trace::mask()
}

/// Record the UI thread's side: `kind` is one of the UI kinds of
/// `bern_command::trace::kind`, `t0` and `t1` come from the trace clock,
/// `name` (UTF-8, may be empty) names the control or the work.
///
/// # Safety
/// `name` must point to `name_len` readable bytes or be null with
/// `name_len == 0`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn bern_trace_ui(
    kind: u32,
    key: u64,
    a: u64,
    b: u64,
    t0: u64,
    t1: u64,
    name: *const u8,
    name_len: usize,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if trace::mask() == 0 {
            return;
        }
        trace::ui(kind as u16, key, a, b, t0, t1, utf8(name, name_len));
    }));
}

/// A line to print as it is, such as a motion value
/// (`motion.values`).
///
/// # Safety
/// `line` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_trace_text(line: *const u8, len: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(line) = utf8(line, len) {
            trace::text(line);
        }
    }));
}

/// Rules and a directory for timelines, for hosts without environment
/// variables. Call before `bern_app_start`; later calls start tracing at
/// once.
///
/// # Safety
/// Each pointer must point to its length of readable bytes or be null
/// with length 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_trace_configure(
    rules: *const u8,
    rules_len: usize,
    dir: *const u8,
    dir_len: usize,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        trace::configure(utf8(rules, rules_len), utf8(dir, dir_len));
    }));
}

/// Read what was recorded; a web page calls it from a timer, as it has no
/// collector thread.
#[unsafe(no_mangle)]
pub extern "C" fn bern_trace_pump() {
    let _ = catch_unwind(trace::pump);
}
