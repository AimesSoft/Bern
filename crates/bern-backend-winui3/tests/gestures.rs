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
    bern_backend_winui3::Generator::new(&app).interface()
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
    assert!(plain.contains("B::Button(p, c,"), "{plain}");
    assert!(
        !plain.contains("B::ContentButton(p, c,"),
        "an action child is not visual content"
    );
    assert!(
        plain.contains(
            ".doubleTap = [](Ctx const& c) { c.Write(Field{0, 2}, R::Not(c.Peek(Field{0, 2}))); }"
        ),
        "{plain}"
    );
    assert!(!plain.contains("unsupported DoubleTap"));
    button
        .children
        .push(Node::Element(element("Spacer", vec![])));
    let content = generate(button.clone());
    assert!(content.contains("B::ContentButton(p, c,"));
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
        ".continuous = true",
        ".active = Field{0, 4}",
        ".onCommit = [](Ctx const& c) { c.Write(Field{0, 3}, R::Not(c.Peek(Field{0, 3}))); }",
        "B::ContentButton(p, c,",
    ] {
        assert!(code.contains(expected), "missing {expected}: {code}");
    }
    assert!(!code.contains("unsupported OnCommit"));
    assert_eq!(code.matches("B::Swipe(p, c,").count(), 1);
    swipe.props.remove("continuous");
    swipe.props.remove("active");
    swipe.children.pop();
    let old = generate(swipe);
    assert!(!old.contains(".continuous = true"));
}

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
        code.contains("B::ScreenBrightness(c, Field{0, 4});"),
        "{code}"
    );
    assert!(code.contains("c.WindowFlag(\"touch\")"), "{code}");
    assert!(!code.contains("unsupported ScreenBrightness"));
    let runtime = include_str!("../templates/cpp/bern_ui_features.inl");
    let brightness = runtime
        .split_once("inline void ScreenBrightness(")
        .unwrap()
        .1
        .split_once("struct GestureProps")
        .unwrap()
        .0;
    assert!(brightness.contains("Value::Bool(false)"));
    assert!(!brightness.contains("props.value"));
}

#[cfg(not(windows))]
#[test]
fn portable_gesture_runtime_executes_tap_drag_and_cancel_sequences() {
    let runtime = include_str!("../templates/cpp/bern_ui_widgets.inl");
    let state = runtime
        .split_once("// BEGIN portable gesture state")
        .unwrap()
        .1
        .split_once("// END portable gesture state")
        .unwrap()
        .0;
    let tests = r#"
int main() {
    for (bool continuous : {false, true}) {
        bool active = true;
        double draft = 0.2, committed = 0;
        int actions = 0;
        FinishSwipe(continuous,
            [&] { draft = 0.8; return true; },
            [&] { assert(active && draft == 0.8); ++actions; },
            [&] { assert(active); committed = draft; },
            [&] { active = false; draft = 0.2; }); // synchronous Watch writes old engine state
        assert(committed == 0.8 && actions == (continuous ? 0 : 1));
        assert(!active && draft == 0.2);
        active = true;
        FinishSwipe(continuous, [] { return false; }, [] { assert(false); }, [] { assert(false); }, [&] { active = false; });
        assert(!active); // cancel only cleans up
    }
    for (int failure = 0; failure < 3; ++failure) {
        bool cleaned = false;
        try {
            FinishSwipe(false,
                [&] { if (failure == 0) throw 0; return true; },
                [&] { if (failure == 1) throw 1; },
                [&] { if (failure == 2) throw 2; },
                [&] { cleaned = true; });
            assert(false);
        } catch (int result) { assert(result == failure); }
        assert(cleaned);
    }
    ButtonTaps taps;
    assert(!taps.Press(10, 0, 0, 400, 5, 5));
    assert(taps.Click() == TapResult::Pending);
    auto old = taps.generation;
    assert(!taps.Press(200, 1, 1, 400, 5, 5));
    assert(taps.Expire(old) == TapResult::None);
    assert(taps.Click() == TapResult::Double);
    assert(taps.Expire(taps.generation) == TapResult::None);
    taps.Press(700, 0, 0, 400, 5, 5);
    assert(taps.Click() == TapResult::Pending);
    assert(taps.Expire(taps.generation) == TapResult::Single);
    assert(taps.Expire(taps.generation) == TapResult::None);
    taps.Press(900, 0, 0, 400, 5, 5);
    taps.Click();
    assert(taps.Press(950, 20, 0, 400, 5, 5)); // separate target position
    assert(taps.Move(40, 0, 8, 8));
    assert(taps.Click() == TapResult::None);
    assert(taps.Expire(taps.generation) == TapResult::None);
    taps.Press(1100, 0, 0, 400, 5, 5);
    taps.Click();
    taps.Cancel(); // unmount, capture lost, or keyboard activation
    assert(taps.Expire(taps.generation) == TapResult::None);

    SwipeDrag drag;
    drag.Begin(1, 0, 0, 0.5);
    assert(!drag.Move(4, 2, false, 8, 300, 0, 1));
    assert(!drag.End()); // click, not drag
    drag.Begin(1, 0, 0, 0.5);
    assert(!drag.Move(9, 20, false, 8, 300, 0, 1));
    assert(!drag.Move(50, 20, false, 8, 300, 0, 1));
    assert(!drag.End()); // cannot steal a perpendicular scroll later
    for (bool continuous : {false, true}) {
        int actions = 0, commits = 0;
        drag.Begin(1, 0, 0, 0.5);
        for (double x : {30., 60., 600., 700.}) {
            auto value = drag.Move(x, 3, false, 8, 300, 0, 1);
            if (value && continuous) ++actions;
        }
        assert(drag.moving && drag.last == 1.0);
        if (drag.End()) { if (!continuous) ++actions; ++commits; }
        assert(actions == (continuous ? 3 : 1) && commits == 1);
        drag.Begin(2, 0, 0, 0.2);
        auto value = drag.Move(0, -30, true, 8, 300, 0, 1);
        assert(value && std::abs(*value - 0.3) < 1e-10); // external rebase
        drag.Cancel();
        if (drag.End()) { ++actions; ++commits; }
        assert(commits == 1 && !drag.down && !drag.moving);
    }
}
"#;
    let source = format!(
        "#include <algorithm>\n#include <cassert>\n#include <cmath>\n#include <cstdint>\n#include <optional>\n{state}\n{tests}"
    );
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let dir = target
        .join("bern-test")
        .join(format!("winui-gestures-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cpp = dir.join("gestures.cpp");
    let binary = dir.join("gestures");
    std::fs::write(&cpp, source).unwrap();
    let built = std::process::Command::new("c++")
        .arg("-std=c++20")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(&cpp)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = std::process::Command::new(&binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
