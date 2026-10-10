//! Media functions of the C ABI.
//!
//! The frontend hosts the media executor (for example AVFoundation): it
//! attaches itself to the hub, performs the requests it receives through
//! `on_media_request` and reports what it observes. Its views create,
//! bind and release the players of their declarations here, and its
//! controls dispatch the same media commands every other entry uses.

use super::{BernApp, BernBuffer, BernCallbacks, error_bytes, result_bytes, slice, with_app};
use bern_media::{
    ExecRequest, MediaCapabilities, MediaCommand, MediaExecutor, Observation, media_catalog,
};
use bern_protocol::{CommandError, Reader, Value, Writer, decode, encode};

/// Sends hub requests to the frontend's executor.
struct FrontendExecutor {
    callbacks: BernCallbacks,
}

impl MediaExecutor for FrontendExecutor {
    fn execute(&self, request: ExecRequest) {
        self.callbacks.media_request(&request.encode());
    }
}

/// Media state for the frontend: revision, count, then `MediaPlayer` records.
pub(crate) fn state_bytes(revision: u64, players: &[Value]) -> Vec<u8> {
    let (types, _) = media_catalog();
    let player = bern_protocol::Type::Record {
        name: "MediaPlayer".into(),
    };
    let mut writer = Writer::new();
    writer.u64(revision);
    writer.u32(players.len() as u32);
    for value in players {
        encode(value, &player, &types, &mut writer);
    }
    writer.bytes
}

fn ok_bytes(write: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.u8(0);
    write(&mut writer);
    writer.bytes
}

fn status_bytes(result: Result<(), CommandError>) -> Vec<u8> {
    match result {
        Ok(()) => ok_bytes(|_| {}),
        Err(error) => error_bytes(&error),
    }
}

fn malformed(error: impl std::fmt::Display) -> CommandError {
    CommandError::invalid_params(format!("malformed media call: {error}"))
}

/// The frontend's executor is ready. Layout: backend text, backend version
/// text, background playback u8, multiple views u8.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_executor_attached(
    app: *mut BernApp,
    bytes: *const u8,
    len: usize,
) -> BernBuffer {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    with_app(app, BernBuffer::empty(), |app| {
        let mut reader = Reader::new(bytes);
        let result = (|| -> Result<(), CommandError> {
            let capabilities = MediaCapabilities {
                backend: reader.text().map_err(malformed)?,
                backend_version: reader.text().map_err(malformed)?,
                background_playback: reader.u8().map_err(malformed)? != 0,
                multi_view: reader.u8().map_err(malformed)? != 0,
            };
            app.media.attach_executor(
                std::sync::Arc::new(FrontendExecutor {
                    callbacks: app.callbacks,
                }),
                capabilities,
            );
            Ok(())
        })();
        BernBuffer::from_vec(status_bytes(result))
    })
}

/// The frontend's executor stopped; players become unavailable.
///
/// # Safety
/// `app` must be null or a live handle from `bern_app_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_executor_detached(app: *mut BernApp) {
    with_app(app, (), |app| app.media.detach_executor())
}

/// Current media state, in the layout of `on_media_state`.
///
/// # Safety
/// `app` must be null or a live handle from `bern_app_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_snapshot(app: *mut BernApp) -> BernBuffer {
    with_app(app, BernBuffer::empty(), |app| {
        let (revision, players) = app.media.snapshot();
        BernBuffer::from_vec(state_bytes(revision, &players))
    })
}

/// Dispatch media command `command` (index in the media catalog) with
/// encoded parameters. The outcome arrives through `on_reply` with `token`:
/// `0, revision, result` or `1, revision, code, message`.
///
/// # Safety
/// `params` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_dispatch(
    app: *mut BernApp,
    command: u32,
    params: *const u8,
    len: usize,
    token: u64,
) {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(params, len) }.to_vec();
    with_app(app, (), |app| {
        let callbacks = app.callbacks;
        let media = app.media.clone();
        let (types, specs) = media_catalog();
        let Some(command) = MediaCommand::from_index(command) else {
            let error = CommandError::new("unknown_command", "unknown media command index");
            callbacks.reply(
                token,
                &result_bytes(
                    &Err(error),
                    media.snapshot().0,
                    &bern_protocol::Type::Unit,
                    &types,
                ),
            );
            return;
        };
        let spec = specs[command.index() as usize].clone();
        let mut reader = Reader::new(&bytes);
        let decoded = (|| {
            let params = spec
                .params
                .iter()
                .map(|param| decode(&param.ty, &types, &mut reader))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| CommandError::invalid_params(error.0))?;
            if !reader.is_finished() {
                return Err(CommandError::invalid_params("trailing parameter bytes"));
            }
            Ok(params)
        })();
        match decoded {
            Err(error) => callbacks.reply(
                token,
                &result_bytes(&Err(error), media.snapshot().0, &spec.result, &types),
            ),
            Ok(params) => {
                let hub = media.clone();
                media.dispatch(
                    command,
                    params,
                    Box::new(move |result| {
                        callbacks.reply(
                            token,
                            &result_bytes(&result, hub.snapshot().0, &spec.result, &types),
                        );
                    }),
                );
            }
        }
    })
}

/// Create the player of a `Video(source: …)` declaration for one component
/// instance. Layout: player text, owner text. Returns `0` or `1, code, message`.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_create(
    app: *mut BernApp,
    bytes: *const u8,
    len: usize,
) -> BernBuffer {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    with_app(app, BernBuffer::empty(), |app| {
        let mut reader = Reader::new(bytes);
        let result = (|| {
            let player = reader.text().map_err(malformed)?;
            let owner = reader.text().map_err(malformed)?;
            app.media.create(&player, &owner)
        })();
        BernBuffer::from_vec(status_bytes(result))
    })
}

/// The declaration's source binding has a value. Layout: player text,
/// present u8, url text when present. Returns `0, source version` or
/// `1, code, message`. The same value never reloads.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_bind(
    app: *mut BernApp,
    bytes: *const u8,
    len: usize,
) -> BernBuffer {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    with_app(app, BernBuffer::empty(), |app| {
        let mut reader = Reader::new(bytes);
        let result = (|| {
            let player = reader.text().map_err(malformed)?;
            let url = match reader.u8().map_err(malformed)? {
                0 => None,
                _ => Some(reader.text().map_err(malformed)?),
            };
            app.media.bind(&player, url)
        })();
        BernBuffer::from_vec(match result {
            Ok(version) => ok_bytes(|writer| writer.u64(version)),
            Err(error) => error_bytes(&error),
        })
    })
}

/// The owner of a declaration's player ended. Layout: player text.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_release(app: *mut BernApp, bytes: *const u8, len: usize) {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    with_app(app, (), |app| {
        if let Ok(player) = Reader::new(bytes).text() {
            app.media.release(&player);
        }
    })
}

/// A video view attached or detached. Layout: token text, attached u8,
/// then player text and window instance text when attached.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_view(app: *mut BernApp, bytes: *const u8, len: usize) {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    with_app(app, (), |app| {
        let mut reader = Reader::new(bytes);
        let (Ok(token), Ok(attached)) = (reader.text(), reader.u8()) else {
            return;
        };
        if attached == 0 {
            app.media.detach_view(&token);
        } else if let (Ok(player), Ok(window)) = (reader.text(), reader.text()) {
            app.media.attach_view(&token, &player, &window);
        }
    })
}

/// The executor finished a request. Layout: request u64, then `0` or
/// `1, code, message`.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_complete(app: *mut BernApp, bytes: *const u8, len: usize) {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    with_app(app, (), |app| {
        let mut reader = Reader::new(bytes);
        let Ok(request) = reader.u64() else { return };
        let result = match reader.u8() {
            Ok(0) => Ok(()),
            _ => Err(super::read_error(&mut reader)),
        };
        app.media.complete(request, result);
    })
}

/// The executor observed a player's state, in the layout of
/// `Observation::encode`. Reports for an older source are ignored.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_media_event(app: *mut BernApp, bytes: *const u8, len: usize) {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    with_app(app, (), |app| {
        if let Ok((player, version, observation)) = Observation::decode(bytes) {
            app.media.observe(&player, version, observation);
        }
    })
}
