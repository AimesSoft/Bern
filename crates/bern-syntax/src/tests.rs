use super::*;
use quote::quote;

#[test]
fn parses_elements_templates_conditions_and_actions() {
    let body = parse_ui(quote! {
        Column(gap: 12) {
            Text(content: tr! { "zh-Hans" => "音量 {value}", "en" => "Volume {value}"; value: app.volume })
            List(items: app.items, key: id) |item| {
                Label(title: item.kind, icon: "star")
            }
            if draft.page == Page::Text && !draft.compact {
                Divider
            } else {
                Spacer
            }
            if let Some(action) = app.last_action {
                Text(content: action)
            }
            match draft.page {
                Page::Text => { text_page(app: app) }
                Route::Item { id } => { item_detail(id: id) }
                _ => { Spacer }
            }
            Button(id: save, label: tr! { "en" => "Save" }) => app.record(Action::Primary)
            Slider(id: volume, value: draft.volume, min: 0.0, max: 100.0) => app.set_volume(draft.volume)
        }
    })
    .unwrap();
    assert_eq!(body.nodes.len(), 1);
    let Node::Element(column) = &body.nodes[0] else {
        panic!()
    };
    let children = column.children.as_ref().unwrap();
    assert_eq!(children.len(), 7);
    let Node::Element(list) = &children[1] else {
        panic!()
    };
    assert_eq!(list.template.as_ref().unwrap(), "item");
    let Node::Element(button) = &children[5] else {
        panic!()
    };
    let action = button.action.as_ref().unwrap();
    assert_eq!(action.receiver_path(), "app");
    assert_eq!(action.method, "record");
    assert!(matches!(action.args[0], Expr::Variant(_)));
    let Node::Element(text) = &children[0] else {
        panic!()
    };
    let Expr::Tr(tr) = &text.prop("content").unwrap().value else {
        panic!()
    };
    assert_eq!(tr.entries.len(), 2);
    assert_eq!(tr.args[0].name, "value");
}

#[test]
fn rejects_callbacks_calls_assignment_and_loops() {
    for (tokens, needle) in [
        (
            quote! { Button(label: tr!{"en" => "A"}) => |x| save(x) },
            "arbitrary callbacks",
        ),
        (
            quote! { Text(content: format(app.name)) },
            "calls are not allowed",
        ),
        (
            quote! { Text(content: app.name()) },
            "calls are not allowed",
        ),
        (
            quote! { Button(label: tr!{"en" => "A"}) => app.count = 1 },
            "direct assignment",
        ),
        (
            quote! { for item in app.items { Text(content: item) } },
            "`for` loops",
        ),
        (quote! { Text(content: format!("x")) }, "only `tr!`"),
        (
            quote! { Button(label: tr!{"en" => "A"}) => save() },
            "typed client",
        ),
        (
            quote! { Text(content: app.a == app.b == app.c) },
            "cannot be chained",
        ),
    ] {
        let error = parse_ui(tokens).unwrap_err().to_string();
        assert!(error.contains(needle), "{error} should mention {needle}");
    }
}

#[test]
fn operator_precedence_matches_rust() {
    let body = parse_ui(quote! { Text(content: a || b && c == d) }).unwrap();
    let Node::Element(element) = &body.nodes[0] else {
        panic!()
    };
    let Expr::Binary { op, rhs, .. } = &element.props[0].value else {
        panic!()
    };
    assert_eq!(*op, BinaryOp::Or);
    let Expr::Binary { op, .. } = &**rhs else {
        panic!()
    };
    assert_eq!(*op, BinaryOp::And);
}

#[test]
fn collects_protocol_session_values_and_components() {
    let source = r#"
        use bern::prelude::*;

        #[bern::protocol(namespace = "showcase", version = 1)]
        pub mod showcase {
            use bern::prelude::*;

            #[value]
            pub enum Action {
                #[label(tr! { "zh-Hans" => "主要", "en" => "Primary" })]
                Primary,
                Secondary,
            }

            #[state]
            pub struct State {
                /// Most recent action
                pub last_action: Option<Action>,
                pub volume: f64,
            }

            #[commands]
            pub trait Commands {
                /// Record an action
                #[errors("busy")]
                fn record(&mut self, action: Action) -> Result<(), CommandError>;
                #[idempotent]
                fn set_volume(&mut self, value: f64) -> Result<(), CommandError>;
            }
        }

        #[bern::session]
        pub struct Draft {
            #[initial(50.0)]
            pub volume: f64,
            pub name: String,
        }

        mod pages {
            #[component]
            pub fn page(app: showcase::Client, #[session] draft: Draft) -> impl View {
                ui! {
                    Text(content: draft.name)
                }
            }
        }
    "#;
    let items = parse_source_file(source).unwrap();
    assert_eq!(items.protocols.len(), 1);
    let protocol = &items.protocols[0].1;
    assert_eq!(protocol.args.namespace, "showcase");
    assert_eq!(protocol.state.fields.len(), 2);
    assert_eq!(protocol.state.fields[0].doc, "Most recent action");
    assert_eq!(protocol.commands.methods.len(), 2);
    assert_eq!(protocol.commands.methods[0].errors, vec!["busy"]);
    assert!(protocol.commands.methods[1].idempotent);
    let ValueKind::Enum(variants) = &protocol.values[0].kind else {
        panic!()
    };
    assert!(variants[0].label.is_some());
    assert_eq!(items.sessions.len(), 1);
    assert!(items.sessions[0].1.fields[0].initial.is_some());
    assert_eq!(items.components.len(), 1);
    assert_eq!(items.components[0].0, vec!["pages"]);
    assert_eq!(items.components[0].1.params[1].kind, ParamKind::Session);
}

#[test]
fn rejects_unsupported_types_and_conditional_items() {
    let error = parse_source_file(
        r#"
        #[bern::session]
        pub struct Draft { pub count: i32 }
    "#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("use `i64`"), "{error}");
    let error = parse_source_file(
        r#"
        #[cfg(feature = "x")]
        #[bern::value]
        pub enum Mode { A }
    "#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("conditional compilation"), "{error}");
    let error = parse_source_file(
        r#"
        #[component]
        fn page() -> impl View { let x = 1; ui! { Spacer } }
    "#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("exactly one `ui!"), "{error}");
}

#[test]
fn names_convert_between_cases() {
    assert_eq!(pascal_case("main_window"), "MainWindow");
    assert_eq!(camel_case("last_action"), "lastAction");
}
