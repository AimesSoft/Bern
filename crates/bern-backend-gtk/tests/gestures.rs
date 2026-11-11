//! Narrow generator and portable gesture-runtime regressions. No native window.
use bern_ir::{
    Action, Element, FieldRef, Node, Prop, SessionDef, SessionFieldDef, SessionOp, SourceLocation,
};
use bern_protocol::{Type, Value};
use std::path::Path;

fn element(kind: &str, children: Vec<Node>) -> Element {
    Element {
        kind: kind.into(),
        id: Some(format!("gesture_{}", kind.to_lowercase())),
        path: format!("test.{kind}"),
        props: Default::default(),
        template: None,
        children,
        action: None,
        source: SourceLocation {
            file: "gestures.rs".into(),
            line: 1,
            column: 1,
        },
    }
}
fn field(index: usize, ty: Type) -> FieldRef {
    FieldRef {
        session: "gesture".into(),
        field: index,
        ty,
    }
}
fn action_node(kind: &str, index: usize) -> Node {
    let mut node = element(kind, vec![]);
    node.action = Some(Action::Session {
        field: field(index, Type::Bool),
        op: SessionOp::Toggle,
    });
    Node::Element(node)
}
fn generate(body: Element) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = bern_compiler::config::load(&root.join("examples/hello/Bern.toml")).unwrap();
    let mut app = bern_compiler::compile(&config).unwrap().app;
    app.sessions.push(SessionDef {
        name: "gesture".into(),
        fields: (0..5)
            .map(|i| SessionFieldDef {
                name: format!("field{i}"),
                ty: if i == 0 { Type::Float } else { Type::Bool },
                initial: if i == 0 {
                    Value::Float(0.5)
                } else {
                    Value::Bool(false)
                },
            })
            .collect(),
    });
    app.windows.windows[0].content = Node::Element(body);
    app.windows.windows[0].sessions.push("gesture".into());
    bern_backend_gtk::Generator::new(&app).generate(0, "test")
}

#[test]
fn action_children_are_extracted_and_never_rendered() {
    let mut button = element(
        "Button",
        vec![
            action_node("DoubleTap", 2),
            Node::Element(element("ContextMenu", vec![])),
        ],
    );
    button.action = Some(Action::Session {
        field: field(1, Type::Bool),
        op: SessionOp::Toggle,
    });
    let plain = generate(button.clone());
    assert!(plain.contains("b::Button(p, c,"), "{plain}");
    assert!(
        !plain.contains("b::ContentButton(p, c,"),
        "an action child is not visual content"
    );
    assert!(
        plain.contains(
            "double_tap: Some(b::act(|c| b::session_toggle(c, b::Field { session: 0, field: 2 }))"
        ),
        "{plain}"
    );
    assert!(!plain.contains("unsupported DoubleTap"));
    button
        .children
        .push(Node::Element(element("Spacer", vec![])));
    let content = generate(button.clone());
    assert!(content.contains("b::ContentButton(p, c,"));
    let mut swipe = element(
        "Swipe",
        vec![Node::Element(button), action_node("OnCommit", 3)],
    );
    swipe.props.insert(
        "value".into(),
        Prop::Field {
            field: field(0, Type::Float),
        },
    );
    swipe.props.insert(
        "active".into(),
        Prop::Field {
            field: field(4, Type::Bool),
        },
    );
    swipe.props.insert(
        "continuous".into(),
        Prop::Keyword {
            value: "true".into(),
        },
    );
    swipe.action = Some(Action::Session {
        field: field(1, Type::Bool),
        op: SessionOp::Toggle,
    });
    let code = generate(swipe.clone());
    for expected in [
        "continuous: true",
        "active: Some(b::Field { session: 0, field: 4 })",
        "on_commit: Some(b::act(|c| b::session_toggle(c, b::Field { session: 0, field: 3 }))",
        "b::ContentButton(p, c,",
    ] {
        assert!(code.contains(expected), "missing {expected}: {code}");
    }
    assert!(!code.contains("unsupported OnCommit"));
    assert_eq!(code.matches("b::Swipe(p, c,").count(), 1);
    swipe.props.remove("continuous");
    swipe.props.remove("active");
    swipe.children.pop();
    let old = generate(swipe);
    assert!(!old.contains("continuous: true"));
}

// Compile and execute the exact GTK-independent state used by the adapter.
#[path = "../../bern-gtk/src/gesture_state.rs"]
mod gesture_state;

#[test]
fn host_capabilities_are_non_visual_and_do_not_write_brightness_value() {
    let mut brightness = element("ScreenBrightness", vec![]);
    brightness.props.insert(
        "value".into(),
        Prop::Field {
            field: field(0, Type::Float),
        },
    );
    brightness.props.insert(
        "available".into(),
        Prop::Field {
            field: field(4, Type::Bool),
        },
    );
    let mut button = element("Button", vec![]);
    button.props.insert(
        "enabled".into(),
        Prop::Expr {
            expr: bern_ir::Expr {
                ty: Type::Bool,
                kind: bern_ir::ExprKind::Binding {
                    binding: bern_ir::Binding::Window {
                        field: "touch".into(),
                    },
                },
            },
        },
    );
    let code = generate(element(
        "Column",
        vec![Node::Element(brightness), Node::Element(button)],
    ));
    assert!(
        code.contains("b::ScreenBrightness(c, b::Field { session: 0, field: 4 });"),
        "{code}"
    );
    assert!(code.contains("c.window(\"touch\")"), "{code}");
    assert!(!code.contains("unsupported ScreenBrightness"));
    let runtime = include_str!("../../bern-gtk/src/features.rs");
    let brightness = runtime
        .split_once("pub fn ScreenBrightness(")
        .unwrap()
        .1
        .split_once("// MARK: - Gestures")
        .unwrap()
        .0;
    assert!(brightness.contains("Value::Bool(false)"));
    assert!(!brightness.contains("props.value"));
}
