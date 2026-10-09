//! Business engine of the control showcase.
//!
//! The engine depends only on the protocol; it never sees a window, a
//! control or a transport, and its tests run without any UI.

use bern::CommandError;
use showcase_protocol::showcase::{self, Action, Item, ItemKind, State};

pub struct ShowcaseEngine {
    state: State,
    next_id: i64,
}

impl Default for ShowcaseEngine {
    fn default() -> Self {
        let kinds = [
            (ItemKind::Documents, 12),
            (ItemKind::Photos, 248),
            (ItemKind::Music, 37),
            (ItemKind::Movies, 5),
            (ItemKind::Downloads, 19),
        ];
        let items = kinds
            .into_iter()
            .enumerate()
            .map(|(index, (kind, count))| Item {
                id: index as i64 + 1,
                kind,
                count,
            })
            .collect::<Vec<_>>();
        Self {
            next_id: items.len() as i64 + 1,
            state: State {
                last_action: None,
                activations: 0,
                volume: 50.0,
                items,
            },
        }
    }
}

impl showcase::Commands for ShowcaseEngine {
    fn record(&mut self, action: Action) -> Result<(), CommandError> {
        self.state.last_action = Some(action);
        self.state.activations = self.state.activations.saturating_add(1);
        Ok(())
    }

    fn set_volume(&mut self, value: f64) -> Result<(), CommandError> {
        if !(0.0..=100.0).contains(&value) {
            return Err(CommandError::new(
                "out_of_range",
                "volume must be between 0 and 100",
            ));
        }
        self.state.volume = value;
        Ok(())
    }

    fn add_item(&mut self, kind: ItemKind) -> Result<i64, CommandError> {
        let id = self.next_id;
        self.next_id += 1;
        self.state.items.push(Item { id, kind, count: 0 });
        Ok(id)
    }

    fn remove_item(&mut self, id: i64) -> Result<(), CommandError> {
        let before = self.state.items.len();
        self.state.items.retain(|item| item.id != id);
        if self.state.items.len() == before {
            return Err(CommandError::new(
                "not_found",
                format!("no item with id {id}"),
            ));
        }
        Ok(())
    }
}

impl showcase::Engine for ShowcaseEngine {
    fn state(&self) -> &State {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern::bind::BernValue;
    use bern::{CallContext, Entry, Host, Invocation, Value};

    fn call(host: &Host, command: &str, params: Vec<Value>) -> Result<Value, CommandError> {
        let index = host.resolve(command).expect("registered command");
        host.dispatch_blocking(
            Invocation {
                command: index,
                params,
            },
            CallContext {
                entry: Entry::Test,
                request_id: "test".into(),
            },
        )
        .result
    }

    #[test]
    fn engine_runs_headless_through_the_shared_host() {
        let host = Host::start(showcase::Adapter(ShowcaseEngine::default()));
        assert_eq!(host.descriptor().namespace, "showcase");
        call(&host, "showcase.record", vec![Action::Delete.to_value()]).unwrap();
        let (revision, state) = host.snapshot();
        assert_eq!(revision, 1);
        let last = host.descriptor().state_index("last_action").unwrap();
        assert_eq!(state[last], Some(Action::Delete).to_value());

        let id = call(&host, "showcase.add_item", vec![ItemKind::Music.to_value()]).unwrap();
        assert_eq!(id, Value::Int(6));
        call(&host, "showcase.remove_item", vec![Value::Int(6)]).unwrap();
        let error = call(&host, "showcase.remove_item", vec![Value::Int(6)]).unwrap_err();
        assert_eq!(error.code, "not_found");
    }

    #[test]
    fn invalid_values_are_rejected_by_the_engine_and_the_host() {
        let host = Host::start(showcase::Adapter(ShowcaseEngine::default()));
        let error = call(&host, "showcase.set_volume", vec![Value::Float(140.0)]).unwrap_err();
        assert_eq!(error.code, "out_of_range");
        let error = call(
            &host,
            "showcase.set_volume",
            vec![Value::Text("loud".into())],
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_params");
        assert_eq!(host.snapshot().0, 0);
    }

    #[test]
    fn descriptor_carries_labels_policies_and_external_names() {
        let descriptor = showcase::descriptor();
        descriptor.validate().unwrap();
        let (_, set_volume) = descriptor.command("showcase.set_volume").unwrap();
        assert_eq!(
            set_volume.policy.retry,
            bern::__private::protocol::Retry::Idempotent
        );
        assert_eq!(set_volume.errors, vec!["out_of_range"]);
        assert_eq!(set_volume.external.cli, vec!["showcase", "set-volume"]);
        assert_eq!(set_volume.external.mcp, "showcase_set_volume");
        let action = descriptor.types.enum_def("Action").unwrap();
        let label = action.variants[0].label.as_ref().unwrap();
        assert_eq!(label.get("en"), Some("Primary button"));
        assert_eq!(label.key, "Action.Primary");
    }
}
