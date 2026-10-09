use bern_core::{App, CommandError, FieldSpec, Node, Scope, ValueKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) struct Session {
    id: String,
    revision: u64,
    fields: BTreeMap<String, FieldSpec>,
    targets: BTreeMap<String, Target>,
}

#[derive(Clone, Serialize)]
struct Target {
    id: String,
    field: String,
    kind: ValueKind,
    command: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditParams {
    value: String,
}

impl Session {
    pub(crate) fn from_json(ui_json: &str, session_id: &str) -> Result<Self, CommandError> {
        let app: App = serde_json::from_str(ui_json)
            .map_err(|_| CommandError::new("invalid_ui", "无法读取界面描述"))?;
        app.validate()?;
        if session_id.is_empty()
            || session_id.len() > 128
            || !session_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(CommandError::new("invalid_session", "会话标识无效"));
        }
        let mut targets = BTreeMap::new();
        collect_targets(&app.root, &mut targets);
        Ok(Self {
            id: session_id.into(),
            revision: 0,
            fields: app
                .fields
                .into_iter()
                .filter(|field| field.scope == Scope::Session)
                .map(|field| (field.id.clone(), field))
                .collect(),
            targets,
        })
    }

    pub(crate) fn dispatch_json(
        &mut self,
        command: &str,
        target: &str,
        payload_json: &str,
    ) -> String {
        let result = self.dispatch(command, target, payload_json);
        json!({
            "ok": result.is_ok(),
            "session_id": self.id,
            "revision": self.revision.to_string(),
            "command": command,
            "target": target,
            "error": result.err(),
        })
        .to_string()
    }

    fn dispatch(
        &mut self,
        command: &str,
        target_id: &str,
        payload_json: &str,
    ) -> Result<(), CommandError> {
        if !matches!(command, "session.edit" | "session.toggle") {
            return Err(CommandError::new("unknown_command", "未登记的会话操作"));
        }
        let target = self
            .targets
            .get(target_id)
            .ok_or_else(|| CommandError::new("target_not_found", "目标控件不存在"))?;
        if target.command != command {
            return Err(CommandError::new(
                "unsupported_action",
                "该控件不支持此操作",
            ));
        }
        // Only interactive session fields enter the target catalog. Engine
        // fields cannot be modified through this local session boundary.
        let field = self
            .fields
            .get_mut(&target.field)
            .ok_or_else(|| CommandError::new("target_not_found", "会话字段不存在"))?;
        let value = match command {
            "session.edit" => {
                let params: EditParams = serde_json::from_str(payload_json)
                    .map_err(|_| CommandError::new("invalid_params", "请输入有效的文字"))?;
                Value::String(params.value)
            }
            "session.toggle" => {
                let params: Value = serde_json::from_str(payload_json)
                    .map_err(|_| CommandError::new("invalid_params", "选择操作参数无效"))?;
                if !params.as_object().is_some_and(|object| object.is_empty()) {
                    return Err(CommandError::new(
                        "invalid_params",
                        "选择操作不接受额外参数",
                    ));
                }
                Value::Bool(
                    !field
                        .initial
                        .as_bool()
                        .ok_or_else(|| CommandError::new("invalid_state", "选择状态无效"))?,
                )
            }
            _ => unreachable!(),
        };
        if !field.kind.accepts(&value) {
            return Err(CommandError::new(
                "invalid_params",
                "内容过长或字段类型不匹配",
            ));
        }
        if field.initial != value {
            let next_revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| CommandError::new("revision_exhausted", "会话需要重新打开"))?;
            field.initial = value;
            self.revision = next_revision;
        }
        Ok(())
    }

    pub(crate) fn snapshot_json(&self) -> String {
        let values: BTreeMap<_, _> = self
            .fields
            .iter()
            .map(|(id, field)| (id, &field.initial))
            .collect();
        json!({
            "session_id": self.id,
            "revision": self.revision.to_string(),
            "values": values,
        })
        .to_string()
    }

    pub(crate) fn catalog_json(&self) -> String {
        json!({
            "schema_version": 1,
            "session_id": self.id,
            "commands": [
                {
                    "id": "session.edit",
                    "label": "编辑文字",
                    "scope": "session",
                    "target_kind": "text_input",
                    "params": {"type": "object", "properties": {"value": {"type": "string", "max_utf8_bytes": 4096}}, "required": ["value"], "additionalProperties": false},
                },
                {
                    "id": "session.toggle",
                    "label": "切换选择",
                    "scope": "session",
                    "target_kind": "checkbox",
                    "params": {"type": "object", "properties": {}, "additionalProperties": false},
                },
            ],
            "targets": self.targets.values().collect::<Vec<_>>(),
        }).to_string()
    }
}

fn collect_targets(node: &Node, targets: &mut BTreeMap<String, Target>) {
    match node {
        Node::Column { children, .. } | Node::Row { children, .. } => {
            for child in children {
                collect_targets(child, targets);
            }
        }
        Node::TextInput { id, field, .. } | Node::Checkbox { id, field, .. } => {
            targets.insert(
                id.clone(),
                Target {
                    id: id.clone(),
                    field: field.id.clone(),
                    kind: field.kind,
                    command: if matches!(node, Node::TextInput { .. }) {
                        "session.edit"
                    } else {
                        "session.toggle"
                    },
                },
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_core::{Command, Field};

    fn fixture() -> String {
        let draft = Field::<String>::session("draft");
        let checked = Field::<bool>::session("checked");
        let count = Field::<i32>::engine("count");
        let app = App::new(
            "测试",
            Node::column(
                "root",
                12,
                24,
                vec![
                    Node::text_input("editor", "草稿", draft),
                    Node::text("echo", draft),
                    Node::checkbox("choice", "选择", checked),
                    Node::text("number", count),
                    Node::button("increment", "增加", Command::new("counter.increment")),
                ],
            ),
            vec![
                draft.describe("初始".into()),
                checked.describe(false),
                count.describe(0),
            ],
            vec![Command::new("counter.increment").describe("增加")],
        );
        serde_json::to_string(&app).unwrap()
    }

    fn result(session: &mut Session, command: &str, target: &str, payload: Value) -> Value {
        serde_json::from_str(&session.dispatch_json(command, target, &payload.to_string())).unwrap()
    }

    #[test]
    fn commands_change_only_their_declared_session_targets() {
        let mut session = Session::from_json(&fixture(), "tab-a").unwrap();
        let edit = result(
            &mut session,
            "session.edit",
            "editor",
            json!({"value": "中文输入"}),
        );
        assert_eq!(edit["ok"], true);
        assert_eq!(edit["revision"], "1");
        assert_eq!(
            result(&mut session, "session.toggle", "choice", json!({}))["revision"],
            "2"
        );
        let snapshot: Value = serde_json::from_str(&session.snapshot_json()).unwrap();
        assert_eq!(
            snapshot["values"],
            json!({"draft": "中文输入", "checked": true})
        );
        assert!(snapshot["values"].get("count").is_none());
    }

    #[test]
    fn invalid_input_and_unregistered_targets_leave_state_unchanged() {
        let mut session = Session::from_json(&fixture(), "tab-a").unwrap();
        let before = session.snapshot_json();
        for (command, target, payload) in [
            ("session.edit", "editor", json!({"value": 12})),
            (
                "session.edit",
                "editor",
                json!({"value": "你".repeat(1366)}),
            ),
            (
                "session.edit",
                "editor",
                json!({"value": "x", "extra": true}),
            ),
            ("session.edit", "count", json!({"value": "x"})),
            ("session.edit", "choice", json!({"value": "x"})),
            ("session.toggle", "choice", json!({"value": true})),
            ("counter.increment", "increment", json!({})),
        ] {
            assert_eq!(result(&mut session, command, target, payload)["ok"], false);
            assert_eq!(session.snapshot_json(), before);
        }
    }

    #[test]
    fn sessions_are_independent_and_catalog_exposes_only_bound_targets() {
        let mut first = Session::from_json(&fixture(), "tab-a").unwrap();
        let second = Session::from_json(&fixture(), "tab-b").unwrap();
        result(
            &mut first,
            "session.edit",
            "editor",
            json!({"value": "只在此页"}),
        );
        let snapshot: Value = serde_json::from_str(&second.snapshot_json()).unwrap();
        assert_eq!(snapshot["values"]["draft"], "初始");
        assert_eq!(snapshot["revision"], "0");
        let catalog: Value = serde_json::from_str(&first.catalog_json()).unwrap();
        assert_eq!(catalog["targets"].as_array().unwrap().len(), 2);
        assert!(
            catalog["targets"]
                .as_array()
                .unwrap()
                .iter()
                .all(|target| target["field"] != "count")
        );
    }

    #[test]
    fn no_op_edit_does_not_advance_revision_and_toggle_is_an_action() {
        let mut session = Session::from_json(&fixture(), "tab-a").unwrap();
        assert_eq!(
            result(
                &mut session,
                "session.edit",
                "editor",
                json!({"value": "初始"})
            )["revision"],
            "0"
        );
        result(&mut session, "session.toggle", "choice", json!({}));
        result(&mut session, "session.toggle", "choice", json!({}));
        let snapshot: Value = serde_json::from_str(&session.snapshot_json()).unwrap();
        assert_eq!(snapshot["values"]["checked"], false);
        assert_eq!(snapshot["revision"], "2");
    }

    #[test]
    fn malformed_descriptions_and_missing_session_identity_are_rejected() {
        assert!(Session::from_json("{}", "tab-a").is_err());
        assert!(Session::from_json(&fixture(), "").is_err());
        let mut app: App = serde_json::from_str(&fixture()).unwrap();
        app.fields[0].scope = Scope::Engine;
        assert!(Session::from_json(&serde_json::to_string(&app).unwrap(), "tab-a").is_err());
    }
}
