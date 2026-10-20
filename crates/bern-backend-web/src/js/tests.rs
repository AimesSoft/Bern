use super::*;
use bern_ir::{
    Action, AppInfo, Binding, Element, Expr, ExprKind, FieldRef, IfNode, Languages, SessionDef,
    SessionFieldDef, SourceLocation, Text, Theme, ThemeMode, WindowCatalog,
};
use bern_protocol::{
    CommandPolicy, CommandSpec, EnumDef, ExternalNames, LocalizedText, OperationKind, ParamSpec,
    ProtocolDescriptor, Scope, StateField, Type, VariantDef,
};
use std::collections::BTreeMap;

fn app() -> Application {
    Application {
        ir_version: bern_ir::IR_VERSION,
        app: AppInfo {
            id: "test".into(),
            bundle_id: "test.app".into(),
            name: LocalizedText::default(),
            version: "1".into(),
            icon: None,
            platform_icons: BTreeMap::new(),
            publisher: None,
        },
        languages: Languages {
            supported: vec!["en".into()],
            default: "en".into(),
            strict: true,
        },
        theme: Theme {
            mode: ThemeMode::System,
            brand_seed: None,
        },
        protocol: ProtocolDescriptor {
            descriptor_version: 1,
            namespace: "test".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![],
            commands: vec![],
            events: vec![],
            modules: vec![],
        },
        types: TypeTable::default(),
        sessions: vec![],
        windows: WindowCatalog {
            handle: "Windows".into(),
            primary: "main".into(),
            windows: vec![],
            accent: None,
            appearance: None,
        },
        players: vec![],
        components: vec![],
        texts: vec![],
        resources: vec![],
        layouts: None,
        contracts: vec![],
        style: Default::default(),
    }
}

fn element(kind: &str) -> Element {
    Element {
        kind: kind.into(),
        id: Some(kind.to_lowercase()),
        path: kind.into(),
        props: BTreeMap::new(),
        template: None,
        children: vec![],
        action: None,
        source: SourceLocation::default(),
    }
}

fn literal_expr(value: Value, ty: Type) -> Expr {
    Expr {
        ty,
        kind: ExprKind::Literal { value },
    }
}

fn dynamic_text(expr: Expr) -> Prop {
    Prop::Text {
        text: Text::Dynamic { expr },
    }
}

fn keyword(value: &str) -> Prop {
    Prop::Keyword {
        value: value.into(),
    }
}

fn command(name: &str, params: &[Type]) -> CommandSpec {
    CommandSpec {
        id: format!("test.{name}"),
        name: name.into(),
        doc: String::new(),
        kind: OperationKind::Command,
        scope: Scope::Engine,
        params: params
            .iter()
            .enumerate()
            .map(|(index, ty)| ParamSpec {
                name: format!("p{index}"),
                ty: ty.clone(),
                doc: String::new(),
            })
            .collect(),
        result: Type::Unit,
        errors: vec![],
        policy: CommandPolicy::default(),
        external: ExternalNames::derive("test", name),
        reads: vec![],
        on: None,
    }
}

#[test]
fn controlled_switch_commits_new_value_without_rewriting_string_arguments() {
    let mut app = app();
    app.protocol.state.push(StateField {
        name: "enabled".into(),
        ty: Type::Bool,
        doc: String::new(),
        content: false,
    });
    app.protocol
        .commands
        .push(command("set_enabled", &[Type::Text, Type::Bool]));
    let checked = Expr {
        ty: Type::Bool,
        kind: ExprKind::Binding {
            binding: Binding::Engine {
                field: 0,
                path: vec![],
            },
        },
    };
    let mut switch = element("Switch");
    switch.props.insert(
        "checked".into(),
        Prop::Expr {
            expr: checked.clone(),
        },
    );
    switch.action = Some(Action::Engine {
        command: 0,
        args: vec![
            literal_expr(Value::Text("E.enabled.get()".into()), Type::Text),
            checked,
        ],
    });
    let generated = Generator::new(&app)
        .control_field(&switch, "checked")
        .unwrap();
    assert!(generated.contains("get: () => E.enabled.get()"));
    assert!(generated.contains("peek: () => E.enabled.get()"));
    assert!(
        generated.contains("C.set_enabled(ctx, \"E.enabled.get()\", newValue)"),
        "{generated}"
    );
}

#[test]
fn explicit_picker_options_keep_live_labels_and_controlled_values() {
    let mut app = app();
    let ty = Type::Enum { name: "Tab".into() };
    app.types.enums.push(EnumDef {
        name: "Tab".into(),
        variants: ["Active", "Completed"]
            .into_iter()
            .map(|name| VariantDef {
                name: name.into(),
                fields: vec![],
                label: None,
            })
            .collect(),
    });
    app.protocol.state.push(StateField {
        name: "tab".into(),
        ty: ty.clone(),
        doc: String::new(),
        content: false,
    });
    app.protocol.state.push(StateField {
        name: "count".into(),
        ty: Type::Int,
        doc: String::new(),
        content: false,
    });
    app.protocol
        .commands
        .push(command("set_tab", std::slice::from_ref(&ty)));
    let selected = Expr {
        ty: ty.clone(),
        kind: ExprKind::Binding {
            binding: Binding::Engine {
                field: 0,
                path: vec![],
            },
        },
    };
    let mut picker = element("Picker");
    picker.props.insert(
        "selected".into(),
        Prop::Expr {
            expr: selected.clone(),
        },
    );
    picker.props.insert("label_hidden".into(), keyword("true"));
    picker.action = Some(Action::Engine {
        command: 0,
        args: vec![selected],
    });
    let mut option = element("PickerOption");
    option.props.insert(
        "value".into(),
        Prop::Expr {
            expr: Expr {
                ty,
                kind: ExprKind::Variant {
                    variant: 1,
                    fields: vec![],
                },
            },
        },
    );
    option.props.insert(
        "label".into(),
        dynamic_text(Expr {
            ty: Type::Int,
            kind: ExprKind::Binding {
                binding: Binding::Engine {
                    field: 1,
                    path: vec![],
                },
            },
        }),
    );
    picker.children.push(Node::Element(option));
    let mut out = Out::default();
    Generator::new(&app).element(&mut out, &picker);
    let generated = out.finish();
    assert!(
        generated.contains("C.set_tab(ctx, newValue)"),
        "{generated}"
    );
    assert!(
        generated.contains(
            "options: [{ value: 1, name: \"Completed\", label: () => B.int(E.count.get()) }]"
        ),
        "{generated}"
    );
    assert!(generated.contains("labelHidden: true"));
    assert!(!generated.contains("L.Tab"));
}

#[test]
fn sidebar_search_preserves_results_shortcut_and_header() {
    let mut app = app();
    app.sessions.push(SessionDef {
        name: "draft".into(),
        fields: vec![
            SessionFieldDef {
                name: "query".into(),
                ty: Type::Text,
                initial: Value::Text(String::new()),
            },
            SessionFieldDef {
                name: "page".into(),
                ty: Type::Int,
                initial: Value::Int(0),
            },
        ],
    });
    let mut sidebar = element("Sidebar");
    sidebar.props.insert(
        "selection".into(),
        Prop::Field {
            field: FieldRef {
                session: "draft".into(),
                field: 1,
                ty: Type::Int,
            },
        },
    );
    sidebar
        .props
        .insert("width".into(), Prop::Number { value: 240.0 });
    let mut search = element("Search");
    search.props.insert(
        "value".into(),
        Prop::Field {
            field: FieldRef {
                session: "draft".into(),
                field: 0,
                ty: Type::Text,
            },
        },
    );
    search.props.insert("shortcut".into(), keyword("cmd+k"));
    let mut result = element("Text");
    result.props.insert(
        "content".into(),
        dynamic_text(literal_expr(Value::Text("Results".into()), Type::Text)),
    );
    search.children.push(Node::Element(result));
    let mut header = element("SidebarHeader");
    header.children.push(Node::Element(element("Divider")));
    sidebar
        .children
        .extend([Node::Element(search), Node::Element(header)]);
    let mut detail = element("Detail");
    detail.props.insert(
        "titled".into(),
        Prop::Expr {
            expr: literal_expr(Value::Bool(false), Type::Bool),
        },
    );
    let mut split = element("SplitView");
    split
        .children
        .extend([Node::Element(sidebar), Node::Element(detail)]);
    let mut out = Out::default();
    Generator::new(&app).element(&mut out, &split);
    let generated = out.finish();
    assert!(
        generated.contains("content: () => \"Results\""),
        "{generated}"
    );
    assert!(generated.contains("F.shortcut(\"cmd+k\""));
    assert!(
        generated.contains("header: (p, ctx) => {\n    B.Divider(p);"),
        "{generated}"
    );
    assert!(generated.contains("titled: () => false"));
    assert!(generated.contains("width: 240"));
}

#[test]
fn session_control_runs_action_after_updating_its_draft() {
    let mut app = app();
    app.sessions.push(SessionDef {
        name: "draft".into(),
        fields: vec![SessionFieldDef {
            name: "enabled".into(),
            ty: Type::Bool,
            initial: Value::Bool(false),
        }],
    });
    app.protocol.commands.push(command("save", &[Type::Bool]));
    let mut switch = element("Switch");
    switch.props.insert(
        "value".into(),
        Prop::Field {
            field: FieldRef {
                session: "draft".into(),
                field: 0,
                ty: Type::Bool,
            },
        },
    );
    switch.action = Some(Action::Engine {
        command: 0,
        args: vec![Expr {
            ty: Type::Bool,
            kind: ExprKind::Binding {
                binding: Binding::Session {
                    session: "draft".into(),
                    field: 0,
                    path: vec![],
                },
            },
        }],
    });
    let generated = Generator::new(&app)
        .control_field(&switch, "checked")
        .unwrap();
    assert!(
        generated
            .contains("ctx.s.draft.enabled.set(newValue); C.save(ctx, ctx.s.draft.enabled.get());"),
        "{generated}"
    );
}

#[test]
fn chip_pickers_stand_in_their_flow_beside_a_spacer() {
    let mut app = app();
    let ty = Type::Enum {
        name: "Category".into(),
    };
    app.types.enums.push(EnumDef {
        name: "Category".into(),
        variants: ["All", "Tools"]
            .into_iter()
            .map(|name| VariantDef {
                name: name.into(),
                fields: vec![],
                label: None,
            })
            .collect(),
    });
    app.protocol.state.push(StateField {
        name: "category".into(),
        ty: ty.clone(),
        doc: String::new(),
        content: false,
    });
    app.protocol
        .commands
        .push(command("set_category", std::slice::from_ref(&ty)));
    let selected = Expr {
        ty,
        kind: ExprKind::Binding {
            binding: Binding::Engine {
                field: 0,
                path: vec![],
            },
        },
    };
    let mut picker = element("Picker");
    picker.props.insert(
        "selected".into(),
        Prop::Expr {
            expr: selected.clone(),
        },
    );
    picker.props.insert("style".into(), keyword("chips"));
    picker.props.insert("label_hidden".into(), keyword("true"));
    picker.action = Some(Action::Engine {
        command: 0,
        args: vec![selected],
    });
    let mut flow = element("Flow");
    flow.children = vec![Node::Element(element("Spacer")), Node::Element(picker)];
    let mut out = Out::default();
    Generator::new(&app).element(&mut out, &flow);
    let generated = out.finish();
    let start = generated.find("B.Flow(").expect("a flow");
    let spacer = generated.find("B.Spacer(").expect("a spacer");
    let chips = generated.find("B.Picker(").expect("a picker");
    assert!(start < spacer && spacer < chips, "{generated}");
    assert!(
        generated[chips..].contains("style: \"chips\""),
        "{generated}"
    );
    assert!(
        generated[chips..].contains("labelHidden: true"),
        "{generated}"
    );
}

#[test]
fn scene_layers_bleed_and_window_buttons_take_no_room() {
    let app = app();
    let number = |value: f64| Prop::Number { value };
    let mut background = element("Positioned");
    for (name, value) in [("x", 0.0), ("y", 0.0), ("width", 1280.0), ("height", 800.0)] {
        background.props.insert(name.into(), number(value));
    }
    background.props.insert(
        "bleed".into(),
        Prop::Expr {
            expr: literal_expr(Value::Bool(true), Type::Bool),
        },
    );
    let mut bar = element("Positioned");
    for (name, value) in [("x", 14.0), ("y", 0.0), ("width", 1246.0), ("height", 64.0)] {
        bar.props.insert(name.into(), number(value));
    }
    let mut row = element("Row");
    row.children = vec![
        Node::Element(element("WindowButtons")),
        Node::Element(element("Spacer")),
    ];
    bar.children = vec![Node::Element(row)];
    let mut scene = element("ScaleBox");
    scene.props.insert("width".into(), number(1280.0));
    scene.props.insert("height".into(), number(800.0));
    scene.children = vec![Node::Element(background), Node::Element(bar)];
    let mut out = Out::default();
    Generator::new(&app).element(&mut out, &scene);
    let generated = out.finish();
    // Each layer places itself in the scene; the background bleeds.
    assert!(
        generated.contains("F.bindLayer(p, ctx, { x: 0, y: 0, width: 1280, height: 800, bleed: () => true }, null);"),
        "{generated}"
    );
    assert!(
        generated.contains("F.bindLayer(p, ctx, { x: 14, y: 0, width: 1246, height: 64 }, null);"),
        "{generated}"
    );
    // A page has no window buttons: nothing stands for them.
    assert!(!generated.contains("WindowButtons"), "{generated}");
    assert!(!generated.contains("unsupported"), "{generated}");
}

#[test]
fn tabs_under_conditions_build_in_the_tab_bar() {
    let mut app = app();
    app.sessions.push(SessionDef {
        name: "draft".into(),
        fields: vec![SessionFieldDef {
            name: "page".into(),
            ty: Type::Int,
            initial: Value::Int(0),
        }],
    });
    let tab = |value: i64, label: &str| {
        let mut tab = element("Tab");
        tab.id = None;
        tab.props.insert(
            "value".into(),
            Prop::Expr {
                expr: literal_expr(Value::Int(value), Type::Int),
            },
        );
        tab.props.insert(
            "label".into(),
            dynamic_text(literal_expr(Value::Text(label.into()), Type::Text)),
        );
        let mut text = element("Text");
        text.id = None;
        text.props.insert(
            "content".into(),
            dynamic_text(literal_expr(
                Value::Text(format!("{label} page")),
                Type::Text,
            )),
        );
        tab.children.push(Node::Element(text));
        Node::Element(tab)
    };
    let mut accessory = element("TabAccessory");
    accessory.id = None;
    accessory.children.push(Node::Element(element("Divider")));
    let mut tabs = element("Tabs");
    tabs.props.insert(
        "selection".into(),
        Prop::Field {
            field: FieldRef {
                session: "draft".into(),
                field: 0,
                ty: Type::Int,
            },
        },
    );
    tabs.props.insert(
        "fallback".into(),
        Prop::Expr {
            expr: literal_expr(Value::Int(2), Type::Int),
        },
    );
    tabs.children.extend([
        Node::If(IfNode {
            branches: vec![(
                literal_expr(Value::Bool(true), Type::Bool),
                vec![tab(0, "Home")],
            )],
            otherwise: vec![],
        }),
        tab(2, "Search"),
        Node::Element(accessory),
    ]);
    let mut out = Out::default();
    Generator::new(&app).element(&mut out, &tabs);
    let generated = out.finish();
    // The tabs build into the bar, the one under the condition too, each
    // with its page; the accessory builds once, beside them.
    assert!(generated.contains("fallback: 2n,"), "{generated}");
    assert!(generated.contains("}, (p, ctx) => {"), "{generated}");
    assert!(
        generated.contains("B.When(p, ctx, () => (true) ? 0 : -1, ["),
        "{generated}"
    );
    assert!(
        generated.contains("B.Tab(p, ctx, { value: 0n, name: \"\", label: () => \"Home\""),
        "{generated}"
    );
    assert!(
        generated.contains("content: () => \"Home page\""),
        "{generated}"
    );
    assert!(
        generated.contains("B.Tab(p, ctx, { value: 2n, name: \"\", label: () => \"Search\""),
        "{generated}"
    );
    assert_eq!(generated.matches("B.Divider(p);").count(), 1, "{generated}");
    assert!(!generated.contains("unsupported"), "{generated}");
}
