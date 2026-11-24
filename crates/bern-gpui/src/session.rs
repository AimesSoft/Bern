//! Session commands of external entries: `ui.inspect`, `ui.activate`,
//! `ui.edit`, `ui.submit`, `ui.set_bool`, `ui.set_number`, `ui.select`,
//! `ui.back`, `ui.focus`, `ui.set_file`, `ui.language`, `ui.set_language`,
//! and all window management commands (`window.open`, `window.close`,
//! `window.activate`, `window.confirm_close`, `window.minimize`,
//! `window.restore`, `window.maximize`, `window.move`, `window.resize`,
//! `window.sync_overlay`, `window.screenshot`).
//!
//! GPUI builds elements while a window draws, and a window hidden behind
//! others does not draw; the controls are therefore listed by building
//! each window's content once more when an entry asks, so what an agent
//! reads is the current state whether the window is visible or not.

use crate::host::host;
use crate::protocol::{Reader, SessionCommand, Value, Writer, encode, session_catalog};
use crate::{Failure, windows};
use gpui::App;
use std::rc::Rc;

/// Choose an option by its key; false when none has it.
pub(crate) type Select = Rc<dyn Fn(&str) -> bool>;

/// What a control does when an external entry asks.
#[derive(Clone, Default)]
pub(crate) struct Actions {
    pub(crate) activate: Option<crate::ctx::Press>,
    pub(crate) edit: Option<Rc<dyn Fn(String)>>,
    pub(crate) submit: Option<Rc<dyn Fn()>>,
    pub(crate) set_number: Option<Rc<dyn Fn(f64)>>,
    pub(crate) set_bool: Option<Rc<dyn Fn(bool)>>,
    /// Choose an option by its label or the name of its value.
    pub(crate) select: Option<Select>,
    /// Pop one level of navigation; false when already at root.
    pub(crate) back: Option<Rc<dyn Fn() -> bool>>,
    /// Give keyboard focus to this control.
    pub(crate) focus: Option<crate::ctx::Press>,
    /// Deliver a validated `file://` or `http(s)://` URL to a file picker or drop area.
    pub(crate) set_file: Option<Rc<dyn Fn(String)>>,
    /// Accepted file kinds for `set_file` (default `"any"`).
    pub(crate) file_kinds: Option<&'static str>,
}

/// Public entry point for generated `NativeView` wrappers to register a
/// declared Bern control entry in the CLI registry.
#[derive(Clone, Default)]
pub struct NativeControlActions {
    pub activate: Option<Rc<dyn Fn()>>,
    pub edit: Option<Rc<dyn Fn(String)>>,
    pub submit: Option<Rc<dyn Fn()>>,
    pub set_number: Option<Rc<dyn Fn(f64)>>,
    pub set_bool: Option<Rc<dyn Fn(bool)>>,
}

/// Register a declared `NativeView` control entry synchronously during render
/// so `ui.inspect` and external commands (`ui.activate`, `ui.set_number`, etc.)
/// can inspect and drive it.
pub fn register_native(
    c: &crate::Ctx,
    id: &str,
    role: &'static str,
    label: String,
    value: Option<String>,
    enabled: bool,
    actions: NativeControlActions,
) {
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role,
        label,
        value,
        enabled: enabled && c.enabled(),
        actions: Actions {
            activate: actions
                .activate
                .map(|f| Rc::new(move |_cx: &mut gpui::App| f()) as crate::ctx::Press),
            edit: actions.edit,
            submit: actions.submit,
            set_number: actions.set_number,
            set_bool: actions.set_bool,
            ..Actions::default()
        },
    });
}

/// A control as external entries see it.
pub(crate) struct Control {
    /// `component.id`, with row keys inside lists.
    pub(crate) target: String,
    /// The window instance, `main#1`.
    pub(crate) window: Rc<str>,
    pub(crate) role: &'static str,
    pub(crate) label: String,
    pub(crate) value: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) actions: Actions,
}

fn send(request: u64, bytes: &[u8]) {
    // SAFETY: the handle is live while the application runs; the bytes are
    // valid for this call.
    unsafe { bern_ffi::bern_session_complete(host().app(), request, bytes.as_ptr(), bytes.len()) };
}

fn complete(request: u64, index: u32, result: Value) {
    let (types, specs) = session_catalog();
    let mut writer = Writer::new();
    writer.u8(0);
    writer.u32(index);
    // Requests name commands by their position in `SessionCommand::ALL`;
    // the catalog lists specs in its own order.
    let spec = SessionCommand::ALL
        .get(index as usize)
        .and_then(|command| specs.iter().find(|spec| spec.id == command.id()));
    if let Some(spec) = spec {
        encode(&result, &spec.result, &types, &mut writer);
    }
    send(request, &writer.bytes);
}

fn fail(request: u64, failure: &Failure) {
    let mut writer = Writer::new();
    writer.u8(1);
    writer.text(&failure.code);
    writer.text(&failure.message);
    send(request, &writer.bytes);
}

fn inspect(controls: Vec<Control>) -> Value {
    let optional = |text: Option<String>| match text {
        Some(text) => Value::some(Value::Text(text)),
        None => Value::none(),
    };
    Value::List(
        controls
            .into_iter()
            .map(|control| {
                let blocked = windows::is_blocked(&control.window);
                let reason = if blocked {
                    Some("blocked_by_modal".to_string())
                } else if !control.enabled {
                    Some("disabled".to_string())
                } else {
                    None
                };
                Value::Record(vec![
                    Value::Text(control.target),
                    Value::Text(control.window.to_string()),
                    Value::Text(control.role.to_string()),
                    Value::Text(control.label),
                    Value::Bool(control.enabled && !blocked),
                    optional(control.value),
                    optional(reason),
                ])
            })
            .collect(),
    )
}

fn find(controls: Vec<Control>, target: &str) -> Result<Control, Failure> {
    let mut matches = controls
        .into_iter()
        .filter(|control| control.target == target);
    match (matches.next(), matches.next()) {
        (None, _) => Err(Failure::new(
            "target_not_found",
            format!("no mounted control `{target}`; see ui.inspect"),
        )),
        (Some(_), Some(_)) => Err(Failure::new(
            "target_ambiguous",
            format!("`{target}` is mounted in several places"),
        )),
        (Some(control), None) if windows::is_blocked(&control.window) => Err(Failure::new(
            "target_blocked",
            format!("`{target}` is blocked by a modal dialog"),
        )),
        (Some(control), None) if !control.enabled => Err(Failure::new(
            "target_disabled",
            format!("`{target}` is disabled"),
        )),
        (Some(control), None) => Ok(control),
    }
}

fn unsupported(control: &Control, action: &str) -> Failure {
    Failure::new(
        "unsupported_action",
        format!(
            "`{}` ({}) does not support {action}",
            control.target, control.role
        ),
    )
}

fn run(
    request: u64,
    index: u32,
    reader: &mut Reader<'_>,
    cx: &mut App,
) -> Result<Option<Value>, Failure> {
    let malformed = |error: crate::protocol::CodecError| Failure::new("invalid_request", error.0);
    match SessionCommand::ALL.get(index as usize) {
        Some(SessionCommand::Inspect) => Ok(Some(inspect(windows::controls(cx)))),
        Some(SessionCommand::Activate) => {
            let target = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let activate = control
                .actions
                .activate
                .clone()
                .ok_or_else(|| unsupported(&control, "activate"))?;
            activate(cx);
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::Edit) => {
            let target = reader.text().map_err(malformed)?;
            let value = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let edit = control
                .actions
                .edit
                .clone()
                .ok_or_else(|| unsupported(&control, "edit"))?;
            edit(value);
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::Submit) => {
            let target = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let submit = control
                .actions
                .submit
                .clone()
                .ok_or_else(|| unsupported(&control, "submit"))?;
            submit();
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::SetBool) => {
            let target = reader.text().map_err(malformed)?;
            let value = reader.u8().map_err(malformed)? != 0;
            let control = find(windows::controls(cx), &target)?;
            let set = control
                .actions
                .set_bool
                .clone()
                .ok_or_else(|| unsupported(&control, "set_bool"))?;
            set(value);
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::SetNumber) => {
            let target = reader.text().map_err(malformed)?;
            let value = reader.f64().map_err(malformed)?;
            if !value.is_finite() {
                return Err(Failure::new("invalid_value", "expected a finite number"));
            }
            let control = find(windows::controls(cx), &target)?;
            let set = control
                .actions
                .set_number
                .clone()
                .ok_or_else(|| unsupported(&control, "set_number"))?;
            set(value);
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::Select) => {
            let target = reader.text().map_err(malformed)?;
            let option = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let select = control
                .actions
                .select
                .clone()
                .ok_or_else(|| unsupported(&control, "select"))?;
            if select(&option) {
                Ok(Some(Value::Unit))
            } else {
                Err(Failure::new(
                    "invalid_option",
                    format!("`{}` has no option `{option}`", control.target),
                ))
            }
        }
        Some(SessionCommand::Back) => {
            let target = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let back = control
                .actions
                .back
                .clone()
                .ok_or_else(|| unsupported(&control, "back"))?;
            if back() {
                Ok(Some(Value::Unit))
            } else {
                Err(Failure::new("target_unavailable", "nothing to go back to"))
            }
        }
        Some(SessionCommand::Focus) => {
            let target = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let focus = control
                .actions
                .focus
                .clone()
                .ok_or_else(|| unsupported(&control, "focus"))?;
            focus(cx);
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::SetFile) => {
            let target = reader.text().map_err(malformed)?;
            let url = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let give = control
                .actions
                .set_file
                .clone()
                .ok_or_else(|| unsupported(&control, "set_file"))?;
            let scheme = url
                .split_once("://")
                .map(|(scheme, rest)| (scheme.to_ascii_lowercase(), rest))
                .filter(|(_, rest)| !rest.is_empty());
            if !matches!(
                scheme.as_ref().map(|(scheme, _)| scheme.as_str()),
                Some("file" | "http" | "https")
            ) {
                return Err(Failure::new(
                    "invalid_url",
                    format!("`{url}` is not a file:// or http(s) URL"),
                ));
            }
            let kinds = control.actions.file_kinds.unwrap_or("any");
            if !crate::files::accepts(&url, kinds) {
                return Err(Failure::new(
                    "unsupported_file",
                    format!("`{}` takes {kinds}, not `{url}`", control.target),
                ));
            }
            give(url);
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::Language) => Ok(Some(Value::Text(crate::text::language()))),
        Some(SessionCommand::SetLanguage) => {
            let tag = reader.text().map_err(malformed)?;
            if crate::text::set_language(&tag) {
                Ok(Some(Value::Unit))
            } else {
                Err(Failure::new(
                    "unsupported_language",
                    format!(
                        "`{tag}` is not one of {}",
                        crate::text::supported().join(", ")
                    ),
                ))
            }
        }
        Some(SessionCommand::WindowOpen) => {
            let kind_id = reader.text().map_err(malformed)?;
            let id = windows::open_kind(&kind_id, cx)?;
            Ok(Some(Value::Text(id)))
        }
        Some(SessionCommand::WindowClose) => {
            let window_id = reader.text().map_err(malformed)?;
            let result = windows::request_close_instance(&window_id, cx)?;
            Ok(Some(result.to_value()))
        }
        Some(SessionCommand::WindowActivate) => {
            let window_id = reader.text().map_err(malformed)?;
            windows::activate_instance(&window_id, cx)?;
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::WindowConfirmClose) => {
            let window_id = reader.text().map_err(malformed)?;
            let confirmation = reader.text().map_err(malformed)?;
            let decision = reader.u32().map_err(malformed)?;
            if decision > 2 {
                return Err(Failure::new("invalid_params", "unknown close decision"));
            }
            windows::confirm_close_instance(
                &window_id,
                &confirmation,
                decision,
                cx,
                Box::new(move |outcome| match outcome {
                    Ok(result) => complete(request, index, result.to_value()),
                    Err(failure) => fail(request, &failure),
                }),
            )?;
            Ok(None)
        }
        Some(SessionCommand::WindowList) => Err(Failure::new(
            "unknown_command",
            "window.list is answered by the runtime",
        )),
        Some(SessionCommand::WindowMinimize) => {
            let window_id = reader.text().map_err(malformed)?;
            windows::minimize_instance(&window_id, cx)?;
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::WindowRestore) => {
            let window_id = reader.text().map_err(malformed)?;
            windows::restore_instance(&window_id, cx)?;
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::WindowMaximize) => {
            let window_id = reader.text().map_err(malformed)?;
            windows::maximize_instance(&window_id, cx)?;
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::WindowMove) => {
            let window_id = reader.text().map_err(malformed)?;
            let x = reader.f64().map_err(malformed)?;
            let y = reader.f64().map_err(malformed)?;
            windows::move_instance(&window_id, x, y, cx)?;
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::WindowResize) => {
            let window_id = reader.text().map_err(malformed)?;
            let width = reader.f64().map_err(malformed)?;
            let height = reader.f64().map_err(malformed)?;
            windows::resize_instance(&window_id, width, height, cx)?;
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::WindowSyncOverlay) => {
            let overlay_id = reader.text().map_err(malformed)?;
            windows::sync_overlay(&overlay_id, cx);
            Ok(Some(Value::Unit))
        }
        Some(SessionCommand::WindowScreenshot) => {
            let target_id = if reader.u8().map_err(malformed)? != 0 {
                Some(reader.text().map_err(malformed)?)
            } else {
                None
            };
            if reader.u8().map_err(malformed)? != 0 {
                let _ = reader.text().map_err(malformed)?;
            }
            let result = windows::screenshot_instance(target_id.as_deref(), cx)?;
            Ok(Some(result))
        }
        Some(command) => Err(Failure::new(
            "ui_unavailable",
            format!("the GPUI target does not run `{}` yet", command.id()),
        )),
        None => Err(Failure::new("invalid_request", "unknown session command")),
    }
}

/// Handle one session request from the session runtime.
pub(crate) fn handle(request: u64, bytes: &[u8], cx: &mut App) {
    let mut reader = Reader::new(bytes);
    let index = match reader.u32() {
        Ok(index) => index,
        Err(error) => return fail(request, &Failure::new("invalid_request", error.0)),
    };
    match run(request, index, &mut reader, cx) {
        Ok(Some(value)) => complete(request, index, value),
        Ok(None) => {}
        Err(failure) => fail(request, &failure),
    }
}
