//! Session commands of external entries, run through the same closures
//! the user's input runs, in the order of the framework session catalog.

use crate::controls::{self, Entry};
use crate::host::{host, with_bytes};
use crate::protocol::{Reader, SessionCommand, Value, Writer, encode, session_catalog};
use crate::reactive::untrack;
use crate::text;
use crate::windows::{self, Win};
use crate::{Failure, snapshot};
use adw::prelude::*;
use std::rc::Rc;

fn send(request: u64, bytes: &[u8]) {
    with_bytes(bytes, |ptr, len| {
        // SAFETY: the handle is live while the application runs.
        unsafe { bern_ffi::bern_session_complete(host().app(), request, ptr, len) }
    });
    snapshot::schedule();
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

fn find(target: &str) -> Result<Entry, Failure> {
    let matches: Vec<Entry> = controls::entries()
        .into_iter()
        .filter(|entry| entry.target == target)
        .collect();
    let entry = match matches.len() {
        0 => {
            return Err(Failure::new(
                "target_not_found",
                format!("no mounted control `{target}`; see ui.inspect"),
            ));
        }
        1 => matches.into_iter().next().expect("one match"),
        _ => {
            return Err(Failure::new(
                "target_ambiguous",
                format!("`{target}` is mounted in several places"),
            ));
        }
    };
    if let Some(win) = entry.win.upgrade()
        && win.is_blocked()
    {
        return Err(Failure::new(
            "target_blocked",
            format!(
                "`{target}` is blocked by the close confirmation of `{}`; decide it with window.confirm_close",
                win.id
            ),
        ));
    }
    if let Some(enabled) = &entry.enabled
        && !untrack(|| enabled())
    {
        return Err(Failure::new(
            "target_unavailable",
            format!("`{target}` is disabled"),
        ));
    }
    Ok(entry)
}

fn unsupported(entry: &Entry, action: &str) -> Failure {
    Failure::new(
        "target_unavailable",
        format!(
            "`{}` ({}) does not support {action}",
            entry.target, entry.role
        ),
    )
}

fn malformed(error: crate::protocol::CodecError) -> Failure {
    Failure::new("invalid_request", error.0)
}

fn inspect() -> Value {
    let rows = controls::entries()
        .into_iter()
        .map(|entry| {
            let win = entry.win.upgrade();
            let enabled = entry
                .enabled
                .as_ref()
                .map(|enabled| untrack(|| enabled()))
                .unwrap_or(true);
            let label = entry
                .label
                .as_ref()
                .map(|label| untrack(|| label()))
                .unwrap_or_default();
            let value = entry
                .actions
                .value
                .as_ref()
                .and_then(|value| untrack(|| value()));
            let blocked = win.as_ref().is_some_and(|win| win.is_blocked());
            let reason = if blocked {
                Some("blocked_by_modal")
            } else if !enabled {
                Some("disabled")
            } else {
                None
            };
            let optional = |text: Option<String>| match text {
                Some(text) => Value::some(Value::Text(text)),
                None => Value::none(),
            };
            Value::Record(vec![
                Value::Text(entry.target),
                Value::Text(win.map(|win| win.id.clone()).unwrap_or_default()),
                Value::Text(entry.role.to_string()),
                Value::Text(label),
                Value::Bool(enabled),
                optional(value),
                optional(reason.map(String::from)),
            ])
        })
        .collect();
    Value::List(rows)
}

/// Handle one session request from the session runtime.
pub(crate) fn handle(request: u64, bytes: &[u8]) {
    let mut reader = Reader::new(bytes);
    let index = match reader.u32() {
        Ok(index) => index,
        Err(error) => return fail(request, &malformed(error)),
    };
    if let Err(failure) = run(request, index, &mut reader) {
        fail(request, &failure);
    }
}

fn presentation(
    request: u64,
    index: u32,
    win: Rc<Win>,
    perform: impl FnOnce(&Rc<Win>, crate::Done),
) {
    perform(
        &win,
        Box::new(move |result| match result {
            Ok(()) => complete(request, index, Value::Unit),
            Err(failure) => fail(request, &failure),
        }),
    );
}

fn run(request: u64, index: u32, reader: &mut Reader<'_>) -> Result<(), Failure> {
    let ok = |result: Value| complete(request, index, result);
    match index {
        0 => ok(inspect()),
        1 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let activate = entry
                .actions
                .activate
                .clone()
                .ok_or_else(|| unsupported(&entry, "activate"))?;
            activate();
            ok(Value::Unit);
        }
        2 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let value = reader.text().map_err(malformed)?;
            let edit = entry
                .actions
                .edit
                .clone()
                .ok_or_else(|| unsupported(&entry, "edit"))?;
            edit(value);
            ok(Value::Unit);
        }
        3 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let submit = entry
                .actions
                .submit
                .clone()
                .ok_or_else(|| unsupported(&entry, "submit"))?;
            submit();
            ok(Value::Unit);
        }
        4 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let value = reader.u8().map_err(malformed)? != 0;
            let set = entry
                .actions
                .set_bool
                .clone()
                .ok_or_else(|| unsupported(&entry, "set_bool"))?;
            set(value);
            ok(Value::Unit);
        }
        5 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let value = reader.f64().map_err(malformed)?;
            let set = entry
                .actions
                .set_number
                .clone()
                .ok_or_else(|| unsupported(&entry, "set_number"))?;
            set(value);
            ok(Value::Unit);
        }
        6 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let option = reader.text().map_err(malformed)?;
            let select = entry
                .actions
                .select
                .clone()
                .ok_or_else(|| unsupported(&entry, "select"))?;
            if !select(&option) {
                return Err(Failure::new(
                    "invalid_params",
                    format!("`{}` has no option `{option}`", entry.target),
                ));
            }
            ok(Value::Unit);
        }
        7 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let back = entry
                .actions
                .back
                .clone()
                .ok_or_else(|| unsupported(&entry, "back"))?;
            if !back() {
                return Err(Failure::new("target_unavailable", "nothing to go back to"));
            }
            ok(Value::Unit);
        }
        8 => {
            let kind = reader.text().map_err(malformed)?;
            let id = windows::open(&kind)?;
            ok(Value::Text(id));
        }
        9 => {
            let win = windows::named(&reader.text().map_err(malformed)?)?;
            let result = windows::request_close(&win)?;
            ok(result.to_value());
        }
        10 => {
            let win = windows::named(&reader.text().map_err(malformed)?)?;
            win.window.present();
            ok(Value::Unit);
        }
        11 => {
            let win = windows::named(&reader.text().map_err(malformed)?)?;
            let confirmation = reader.text().map_err(malformed)?;
            let decision = reader.u32().map_err(malformed)?;
            if decision > 2 {
                return Err(Failure::new("invalid_params", "unknown close decision"));
            }
            windows::confirm_close(
                &win,
                &confirmation,
                decision,
                Box::new(move |result| match result {
                    Ok(result) => complete(request, index, result.to_value()),
                    Err(failure) => fail(request, &failure),
                }),
            );
        }
        12 => {
            return Err(Failure::new(
                "unknown_command",
                "window.list is answered by the runtime",
            ));
        }
        13 => {
            let win = windows::named(&reader.text().map_err(malformed)?)?;
            presentation(request, index, win, windows::minimize);
        }
        14 => {
            let win = windows::named(&reader.text().map_err(malformed)?)?;
            presentation(request, index, win, windows::restore);
        }
        15 => {
            let win = windows::named(&reader.text().map_err(malformed)?)?;
            presentation(request, index, win, windows::maximize);
        }
        16 => {
            windows::named(&reader.text().map_err(malformed)?)?;
            return Err(Failure::new(
                "target_unavailable",
                "GTK 4 cannot place windows; the window manager positions them",
            ));
        }
        17 => {
            let win = windows::named(&reader.text().map_err(malformed)?)?;
            let width = reader.f64().map_err(malformed)?;
            let height = reader.f64().map_err(malformed)?;
            presentation(request, index, win, move |win, done| {
                windows::resize(win, width, height, done)
            });
        }
        18 => ok(Value::Text(text::with(|language| language.current.peek()))),
        19 => {
            let language = reader.text().map_err(malformed)?;
            if !text::set_language(&language) {
                let supported = text::with(|language| language.supported.join(", "));
                return Err(Failure::new(
                    "unsupported_language",
                    format!("`{language}` is not supported; supported: {supported}"),
                ));
            }
            ok(Value::Unit);
        }
        20 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let focus = entry
                .actions
                .focus
                .clone()
                .ok_or_else(|| unsupported(&entry, "focus"))?;
            focus();
            ok(Value::Unit);
        }
        21 => {
            let entry = find(&reader.text().map_err(malformed)?)?;
            let url = reader.text().map_err(malformed)?;
            let give = entry
                .actions
                .set_file
                .clone()
                .ok_or_else(|| unsupported(&entry, "set_file"))?;
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
            let kinds = entry.actions.file_kinds.unwrap_or("any");
            if !crate::features::accepts(&url, kinds) {
                return Err(Failure::new(
                    "unsupported_file",
                    format!("`{}` takes {kinds}, not `{url}`", entry.target),
                ));
            }
            give(url);
            ok(Value::Unit);
        }
        other => {
            return Err(Failure::new(
                "unknown_command",
                format!("unknown session command {other}"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::mount;
    use crate::controls::tests::entry;
    use crate::protocol::Type;

    #[test]
    fn targets_are_found_once_and_enabled() {
        let mut send = entry("main/send", "button");
        send.enabled = Some(Rc::new(|| false));
        for control in [
            entry("main/save", "button"),
            send,
            entry("main/row", "row"),
            entry("main/row", "row"),
        ] {
            mount(control);
        }
        assert_eq!(
            find("main/save").ok().map(|entry| entry.role),
            Some("button")
        );
        let code = |target| find(target).err().map(|failure| failure.code);
        assert_eq!(code("main/missing").as_deref(), Some("target_not_found"));
        assert_eq!(code("main/row").as_deref(), Some("target_ambiguous"));
        assert_eq!(code("main/send").as_deref(), Some("target_unavailable"));
    }

    #[test]
    fn inspection_lists_controls_as_the_catalog_describes_them() {
        let mut save = entry("main/save", "button");
        save.label = Some(Rc::new(|| "Save".to_string()));
        let mut volume = entry("main/volume", "slider");
        volume.enabled = Some(Rc::new(|| false));
        volume.actions.value = Some(Rc::new(|| Some("0.5".to_string())));
        mount(save);
        mount(volume);
        let (types, _) = session_catalog();
        let rows = inspect();
        rows.check(
            &Type::list(Type::Record {
                name: "ControlInfo".into(),
            }),
            &types,
        )
        .expect("ControlInfo rows");
        let plain = |text: &str| Value::Text(text.into());
        assert_eq!(
            rows,
            Value::List(vec![
                Value::Record(vec![
                    plain("main/save"),
                    plain(""),
                    plain("button"),
                    plain("Save"),
                    Value::Bool(true),
                    Value::none(),
                    Value::none(),
                ]),
                Value::Record(vec![
                    plain("main/volume"),
                    plain(""),
                    plain("slider"),
                    plain(""),
                    Value::Bool(false),
                    Value::some(plain("0.5")),
                    Value::some(plain("disabled")),
                ]),
            ])
        );
    }
}
