//! Pointer events on any visual element, a slider's hover position and
//! declarations reading player state. No native window.
use bern_ir::{
    Action, Binding, Element, Expr, ExprKind, FieldRef, IfNode, Node, Prop, SessionDef,
    SessionFieldDef, SessionOp, SourceLocation, Text,
};
use bern_protocol::{Type, Value};
use bern_target_api::Target;
use std::path::Path;

fn element(kind: &str, children: Vec<Node>) -> Element {
    Element {
        kind: kind.into(),
        id: Some(format!("pointer_{}", kind.to_lowercase())),
        path: format!("test.{kind}"),
        props: Default::default(),
        template: None,
        children,
        action: None,
        source: SourceLocation {
            file: "pointer_events.rs".into(),
            line: 1,
            column: 1,
        },
    }
}

fn field(index: usize, ty: Type) -> FieldRef {
    FieldRef {
        session: "pointer".into(),
        field: index,
        ty,
    }
}

fn toggle(index: usize) -> Action {
    Action::Session {
        field: field(index, Type::Bool),
        op: SessionOp::Toggle,
    }
}

fn event(kind: &str, index: usize) -> Node {
    let mut node = element(kind, vec![]);
    node.action = Some(toggle(index));
    Node::Element(node)
}

/// A `ContextMenu` with one item, toggling `index`.
fn menu(index: usize) -> Node {
    let mut item = element("MenuItem", vec![]);
    item.id = Some("replay".into());
    item.action = Some(toggle(index));
    Node::Element(element("ContextMenu", vec![Node::Element(item)]))
}

/// All four pointer events, toggling fields 1 to 4.
fn events() -> Vec<Node> {
    vec![
        event("DoubleTap", 1),
        menu(2),
        event("WheelUp", 3),
        event("WheelDown", 4),
    ]
}

fn toggled(index: usize) -> String {
    format!(
        "[](Ctx const& c) {{ c.Write(Field{{0, {index}}}, R::Not(c.Peek(Field{{0, {index}}}))); }}"
    )
}

fn player(field: &str) -> Expr {
    Expr {
        ty: bern_ir::player_field_type(field).unwrap(),
        kind: ExprKind::Binding {
            binding: Binding::Player {
                player: "preview".into(),
                field: field.into(),
            },
        },
    }
}

fn generate(body: Node) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = bern_compiler::config::load(&root.join("examples/hello/Bern.toml")).unwrap();
    let mut app = bern_compiler::compile(&config).unwrap().app;
    assert!(app.sessions.is_empty(), "the session below is session 0");
    let mut fields: Vec<SessionFieldDef> = (0..5)
        .map(|i| SessionFieldDef {
            name: format!("field{i}"),
            ty: if i == 0 { Type::Float } else { Type::Bool },
            initial: if i == 0 {
                Value::Float(0.5)
            } else {
                Value::Bool(false)
            },
        })
        .collect();
    fields.push(SessionFieldDef {
        name: "hover".into(),
        ty: Type::optional(Type::Float),
        initial: Value::Optional(None),
    });
    app.sessions.push(SessionDef {
        name: "pointer".into(),
        fields,
    });
    app.windows.windows[0].content = body;
    app.windows.windows[0].sessions.push("pointer".into());
    bern_backend_winui3::Generator::new(&app).interface()
}

/// The lines from the first containing `start` up to the one containing
/// `end`, both included.
fn between<'a>(code: &'a str, start: &str, end: &str) -> &'a str {
    let from = code
        .find(start)
        .unwrap_or_else(|| panic!("no `{start}` in {code}"));
    let to = from
        + code[from..]
            .find(end)
            .unwrap_or_else(|| panic!("no `{end}` in {code}"));
    &code[from..to + end.len()]
}

fn assert_events_attach(code: &str, inner: &str) {
    let expected = format!(
        "B::PointerEvents(p, c, {{.doubleTap = {}, .menu = std::vector<std::optional<B::MenuItemDef>>{{B::MenuItemDef{{.id = \"replay\", .action = {}}}}}, .wheelUp = {}, .wheelDown = {}}}, [](Parent const& p, Ctx const& c) {{",
        toggled(1),
        toggled(2),
        toggled(3),
        toggled(4)
    );
    assert!(code.contains(&expected), "missing {expected}: {code}");
    // The element itself stands directly in the builder: no wrapper.
    let attached = between(code, "B::PointerEvents(p, c, ", inner);
    assert_eq!(attached.lines().count(), 2, "{attached}");
    assert!(!code.contains("unsupported"), "{code}");
    assert!(!code.contains(".own = true"), "{code}");
    assert!(!code.contains("B::MenuItem("), "{code}");
}

#[test]
fn a_video_takes_double_taps_menus_and_wheel_turns() {
    let mut video = element("Video", events());
    video.props.insert(
        "player".into(),
        Prop::Player {
            id: "preview".into(),
        },
    );
    let code = generate(Node::Element(video.clone()));
    assert_events_attach(&code, "B::Video(p, c, {");
    assert_eq!(code.matches("B::Video(p, c, {").count(), 1);
    // Its own props are those it has without events.
    video.children.clear();
    let plain = generate(Node::Element(video));
    let line = |code: &str| {
        code.lines()
            .find(|line| line.contains("B::Video(p, c, {"))
            .unwrap()
            .trim()
            .to_string()
    };
    assert_eq!(line(&code), line(&plain));
    assert!(!plain.contains("B::PointerEvents("));
}

#[test]
fn a_stack_takes_pointer_events_beside_its_content() {
    let mut text = element("Text", vec![]);
    text.props.insert(
        "content".into(),
        Prop::Text {
            text: Text::Dynamic {
                expr: Expr {
                    ty: Type::Text,
                    kind: ExprKind::Literal {
                        value: Value::Text("over".into()),
                    },
                },
            },
        },
    );
    let mut children = vec![Node::Element(text)];
    children.extend(events());
    let mut stack = element("Stack", children);
    stack.props.insert(
        "under_bars".into(),
        Prop::Keyword {
            value: "true".into(),
        },
    );
    let code = generate(Node::Element(stack.clone()));
    assert_events_attach(&code, "B::Layers(p, c, {");
    assert_eq!(code.matches("B::Text(p, c, {").count(), 1);
    // Events among the children do not change the stack: the same props,
    // and no menu item counts as a control under the bars.
    let mut plain = stack;
    plain.children.truncate(1);
    let plain = generate(Node::Element(plain));
    let layers = |code: &str| {
        code.lines()
            .find(|line| line.contains("B::Layers(p, c, {"))
            .unwrap()
            .trim()
            .to_string()
    };
    assert_eq!(layers(&code), layers(&plain));
    assert!(!layers(&code).contains(".covers"), "{code}");
}

#[test]
fn a_button_keeps_its_double_tap_and_menu_and_takes_the_wheel() {
    let mut button = element("Button", vec![event("WheelUp", 3)]);
    button.action = Some(toggle(1));
    let code = generate(Node::Element(button.clone()));
    assert!(
        code.contains(&format!(
            "B::PointerEvents(p, c, {{.wheelUp = {}}}, [](Parent const& p, Ctx const& c) {{",
            toggled(3)
        )),
        "{code}"
    );
    // A wheel is not content: still a plain button.
    assert!(code.contains("B::Button(p, c, {"), "{code}");
    assert!(!code.contains("B::ContentButton("), "{code}");
    assert!(!code.contains("unsupported"), "{code}");

    button.children = events();
    let code = generate(Node::Element(button));
    let attach = code
        .lines()
        .find(|line| line.contains("B::PointerEvents(p, c, "))
        .unwrap();
    assert!(!attach.contains(".doubleTap"), "{attach}");
    assert!(!attach.contains(".menu"), "{attach}");
    assert!(
        attach.contains(&format!(".wheelUp = {}", toggled(3))),
        "{attach}"
    );
    assert!(
        attach.contains(&format!(".wheelDown = {}", toggled(4))),
        "{attach}"
    );
    let button = code
        .lines()
        .find(|line| line.contains("B::Button(p, c, {"))
        .unwrap();
    assert!(
        button.contains(&format!(".doubleTap = {}", toggled(1))),
        "{button}"
    );
    assert!(
        button.contains(
            ".menu = std::vector<std::optional<B::MenuItemDef>>{B::MenuItemDef{.id = \"replay\""
        ),
        "{button}"
    );
}

#[test]
fn a_slider_reports_where_the_pointer_hovers() {
    let mut slider = element("Slider", vec![event("WheelUp", 3), event("WheelDown", 4)]);
    slider.props.insert(
        "value".into(),
        Prop::Field {
            field: field(0, Type::Float),
        },
    );
    slider.props.insert(
        "hover".into(),
        Prop::Field {
            field: field(5, Type::optional(Type::Float)),
        },
    );
    slider
        .props
        .insert("min".into(), Prop::Number { value: 0.0 });
    slider
        .props
        .insert("max".into(), Prop::Number { value: 1.0 });
    let code = generate(Node::Element(slider));
    let line = code
        .lines()
        .find(|line| line.contains("B::Slider(p, c, {"))
        .unwrap();
    assert!(line.contains(".field = Field{0, 0}"), "{line}");
    assert!(line.ends_with(".hover = Field{0, 5}});"), "{line}");
    // The native slider takes the wheel too.
    assert!(
        code.contains(&format!(
            "B::PointerEvents(p, c, {{.wheelUp = {}, .wheelDown = {}, .own = true}}, ",
            toggled(3),
            toggled(4)
        )),
        "{code}"
    );
    let runtime = include_str!("../templates/cpp/bern_ui_widgets.inl");
    let slider = runtime
        .split_once("inline void Slider(")
        .unwrap()
        .1
        .split_once("struct PickOption")
        .unwrap()
        .0;
    assert!(slider.contains("Value::Some(Value::Float(*at)) : Value::None()"));
    assert!(slider.contains("PointerCanceledEvent()"));
}

#[test]
fn declarations_read_the_player_state_this_front_end_received() {
    let mut text = element("Text", vec![]);
    text.props.insert(
        "content".into(),
        Prop::Text {
            text: Text::Dynamic {
                expr: player("position_ms"),
            },
        },
    );
    let mut progress = element("Progress", vec![]);
    progress.props.insert(
        "value".into(),
        Prop::Expr {
            expr: player("progress"),
        },
    );
    let body = Node::Element(element(
        "Column",
        vec![
            Node::If(IfNode {
                branches: vec![(player("playing"), vec![Node::Element(text)])],
                otherwise: vec![],
            }),
            Node::Element(progress),
        ],
    ));
    let code = generate(body);
    for expected in [
        "if (R::Truthy(c.Player(\"preview\", \"playing\"))) return 0;",
        "R::Plain(c.Player(\"preview\", \"position_ms\"))",
        "return c.Player(\"preview\", \"progress\");",
    ] {
        assert!(code.contains(expected), "missing {expected}: {code}");
    }
    let core = include_str!("../templates/cpp/bern_ui_core.inl");
    let read = core
        .split_once("inline Value Ctx::Player(")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    for field in bern_ir::PLAYER_FIELDS {
        assert!(read.contains(&format!("field == \"{field}\"")), "{field}");
    }
    // The state is the hub's, read reactively.
    assert!(read.contains("Media::State("));
}

#[test]
fn the_target_lists_the_pointer_and_player_features() {
    let features = bern_backend_winui3::WinUi3Windows.descriptor().features;
    for feature in [
        bern_target_api::feature::POINTER_EVENTS,
        bern_target_api::feature::WHEEL,
        bern_target_api::feature::SLIDER_HOVER,
        bern_target_api::feature::PLAYER_STATE,
    ] {
        assert!(features.contains(&feature), "{feature}");
    }
}
