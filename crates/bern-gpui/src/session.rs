//! Session commands of external entries: `ui.inspect` lists the controls of
//! the open windows, and `ui.activate`, `ui.edit`, `ui.submit`,
//! `ui.set_bool` and `ui.select` run a control's actions through the same
//! closures pointer and keyboard input run; `ui.language` and
//! `ui.set_language` read and switch the interface language. The others
//! are refused at once rather than left to time out.
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
    pub(crate) set_bool: Option<Rc<dyn Fn(bool)>>,
    /// Choose an option by its label or the name of its value.
    pub(crate) select: Option<Select>,
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
                let reason = (!control.enabled).then(|| "disabled".to_string());
                Value::Record(vec![
                    Value::Text(control.target),
                    Value::Text(control.window.to_string()),
                    Value::Text(control.role.to_string()),
                    Value::Text(control.label),
                    Value::Bool(control.enabled),
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

fn run(index: u32, reader: &mut Reader<'_>, cx: &mut App) -> Result<Value, Failure> {
    let malformed = |error: crate::protocol::CodecError| Failure::new("invalid_request", error.0);
    match SessionCommand::ALL.get(index as usize) {
        Some(SessionCommand::Inspect) => Ok(inspect(windows::controls(cx))),
        Some(SessionCommand::Activate) => {
            let target = reader.text().map_err(malformed)?;
            let control = find(windows::controls(cx), &target)?;
            let activate = control
                .actions
                .activate
                .clone()
                .ok_or_else(|| unsupported(&control, "activate"))?;
            activate(cx);
            Ok(Value::Unit)
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
            Ok(Value::Unit)
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
            Ok(Value::Unit)
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
            Ok(Value::Unit)
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
                Ok(Value::Unit)
            } else {
                Err(Failure::new(
                    "invalid_option",
                    format!("`{}` has no option `{option}`", control.target),
                ))
            }
        }
        Some(SessionCommand::Language) => Ok(Value::Text(crate::text::language())),
        Some(SessionCommand::SetLanguage) => {
            let tag = reader.text().map_err(malformed)?;
            if crate::text::set_language(&tag) {
                Ok(Value::Unit)
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
    match run(index, &mut reader, cx) {
        Ok(value) => complete(request, index, value),
        Err(failure) => fail(request, &failure),
    }
}
