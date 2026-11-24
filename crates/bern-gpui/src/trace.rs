//! UI-thread performance tracing into `bern_ffi::trace`.
//!
//! When tracing is off (`bern_trace_mask() == 0`), each point is a single
//! atomic load and does no work.

use bern_ffi::trace::{bern_trace_mask, bern_trace_now, bern_trace_ui, kind};

#[inline]
pub(crate) fn enabled() -> bool {
    bern_trace_mask() != 0
}

#[inline]
pub(crate) fn now() -> u64 {
    bern_trace_now()
}

#[inline]
pub(crate) fn record(event_kind: u16, key: u64, a: u64, b: u64, t0: u64, t1: u64, name: &str) {
    if !enabled() {
        return;
    }
    // SAFETY: `name` points to `name.len()` valid UTF-8 bytes for this call.
    unsafe {
        bern_trace_ui(
            u32::from(event_kind),
            key,
            a,
            b,
            t0,
            t1,
            name.as_ptr(),
            name.len(),
        );
    }
}

pub(crate) fn frame(window_id: &str, t0: u64, t1: u64) {
    record(kind::FRAME, 0, 0, 0, t0, t1, window_id);
}

pub(crate) fn dispatch(token: u64, command: usize, t0: u64, t1: u64) {
    record(kind::DISPATCH, token, command as u64, 0, t0, t1, "");
}

pub(crate) fn receive_state(revision: u64, bytes: usize, t0: u64, t1: u64) {
    record(kind::RECEIVE_STATE, revision, bytes as u64, 0, t0, t1, "");
}

pub(crate) fn receive_reply(token: u64, t0: u64, t1: u64) {
    record(kind::RECEIVE_REPLY, token, 0, 0, t0, t1, "");
}

pub(crate) fn receive_query(subscription: u64, t0: u64, t1: u64) {
    record(kind::RECEIVE_QUERY, subscription, 0, 0, t0, t1, "");
}
