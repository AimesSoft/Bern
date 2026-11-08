//! Root identity resets opt in without changing existing navigation stacks.
use bern_backend_swiftui::{Generator, Platform};
use bern_ir::{
    Element, Expr, ExprKind, FieldRef, Node, Prop, SessionDef, SessionFieldDef, SourceLocation,
};
use bern_protocol::{Type, Value};
use std::path::Path;

#[test]
fn navigation_reset_observes_root_identity_with_initial_delivery() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = bern_compiler::config::load(&root.join("examples/showcase/Bern.toml")).unwrap();
    let mut compiled =
        bern_compiler::compile(&config).unwrap_or_else(|_| panic!("showcase compiles"));
    compiled.app.sessions.push(SessionDef {
        name: "ResetDraft".into(),
        fields: vec![SessionFieldDef {
            name: "routes".into(),
            ty: Type::list(Type::Text),
            initial: Value::List(vec![]),
        }],
    });
    let navigation = |reset: Option<Expr>| {
        let mut props = [(
            "path".into(),
            Prop::Field {
                field: FieldRef {
                    session: "ResetDraft".into(),
                    field: 0,
                    ty: Type::list(Type::Text),
                },
            },
        )]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
        if let Some(expr) = reset {
            props.insert("reset".into(), Prop::Expr { expr });
        }
        Node::Element(Element {
            kind: "Navigation".into(),
            id: None,
            path: "test.navigation".into(),
            props,
            template: None,
            children: vec![],
            action: None,
            source: SourceLocation {
                file: "navigation.rs".into(),
                line: 1,
                column: 1,
            },
        })
    };
    for platform in [Platform::Ios, Platform::Macos, Platform::Tvos] {
        compiled.app.windows.windows[0].content = navigation(None);
        let original = Generator::new(&compiled.app, platform).generate(0);
        assert!(!original.contains("routes.removeAll()"));
        compiled.app.windows.windows[0].content = navigation(Some(Expr {
            ty: Type::Int,
            kind: ExprKind::Literal {
                value: Value::Int(7),
            },
        }));
        let reset = Generator::new(&compiled.app, platform).generate(0);
        assert!(
            reset.contains(".onChange(of: Int64(7), initial: true) {"),
            "{reset}"
        );
        assert!(reset.contains("routes.removeAll()"), "{reset}");
    }
}
