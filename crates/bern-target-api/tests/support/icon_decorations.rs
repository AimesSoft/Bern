//! One small IR fixture shared by the native and web generator regressions.

use bern_ir::{Element, Expr, ExprKind, Node, Prop, SourceLocation, Text};
use bern_protocol::{Type, Value};
use bern_target_api::{BuildContext, Framework, Target, TargetConfig};
use std::path::Path;

pub fn generated(target: &dyn Target, platform: &str, ui: Option<&str>) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = bern_compiler::config::load(&root.join("examples/hello/Bern.toml")).unwrap();
    let mut compiled = bern_compiler::compile(&config).unwrap();
    let literal = |ty, value| Expr {
        ty,
        kind: ExprKind::Literal { value },
    };
    let mut button = Element {
        kind: "IconButton".into(),
        id: Some("decorated_icon".into()),
        path: "test.decorated_icon".into(),
        props: [
            (
                "icon".into(),
                Prop::Icon {
                    name: "arrow.down.circle".into(),
                },
            ),
            ("font_size".into(), Prop::Number { value: 18.0 }),
            (
                "label".into(),
                Prop::Text {
                    text: Text::Dynamic {
                        expr: literal(Type::Text, Value::Text("Transfer".into())),
                    },
                },
            ),
            (
                "icon_progress".into(),
                Prop::Expr {
                    expr: literal(Type::Float, Value::Float(0.5)),
                },
            ),
            (
                "badge_icon".into(),
                Prop::Icon {
                    name: "checkmark".into(),
                },
            ),
            (
                "badge_visible".into(),
                Prop::Expr {
                    expr: literal(Type::Bool, Value::Bool(false)),
                },
            ),
        ]
        .into(),
        template: None,
        children: Vec::new(),
        action: None,
        source: SourceLocation {
            file: "icon_decorations.rs".into(),
            line: 1,
            column: 1,
        },
    };
    let decorated = Node::Element(button.clone());
    let mut integer = button.clone();
    integer.id = Some("integer_icon".into());
    integer.path = "test.integer_icon".into();
    integer.props.insert(
        "icon_progress".into(),
        Prop::Expr {
            expr: literal(Type::Int, Value::Int(1)),
        },
    );
    integer.props.remove("badge_visible");
    button.id = Some("plain_icon".into());
    button.path = "test.plain_icon".into();
    for prop in ["icon_progress", "badge_icon", "badge_visible"] {
        button.props.remove(prop);
    }
    // The root WindowSet is extracted into the window catalog; the example
    // need not contain any reusable components after compilation.
    compiled.app.windows.windows[0].content = Node::Element(Element {
        kind: "Row".into(),
        id: None,
        path: "test.icons".into(),
        props: Default::default(),
        template: None,
        children: vec![decorated, Node::Element(integer), Node::Element(button)],
        action: None,
        source: SourceLocation {
            file: "icon_decorations.rs".into(),
            line: 1,
            column: 1,
        },
    });
    let target_config = TargetConfig {
        name: format!("icon_{platform}"),
        platform: platform.into(),
        ui: ui.map(str::to_string),
        web_mode: (platform == "browser").then(|| "app".into()),
        ..TargetConfig::default()
    };
    let errors = target.check(&compiled.app, &target_config);
    assert!(errors.iter().all(|error| !error.is_error()), "{errors:?}");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root
            .join("target/bern-test/icon-decorations")
            .join(target.descriptor().id),
        target: target_config,
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    };
    target
        .generate(&compiled.app, &context)
        .unwrap()
        .iter()
        .filter_map(|file| std::str::from_utf8(&file.contents).ok())
        .filter(|source| source.contains("decorated_icon") && source.contains("plain_icon"))
        .collect::<Vec<_>>()
        .join("\n")
}
