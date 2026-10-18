use super::*;
use quote::quote;

#[test]
fn media_source_used_only_by_an_event_is_registered() {
    let items = parse_source_file(
        r#"
        #[bern::protocol(namespace = "downloads", version = 1)]
        pub mod downloads {
            #[state]
            pub struct State { pub count: i64 }
            #[commands]
            pub trait Commands { fn play(&mut self) -> Result<(), CommandError>; }
            #[events]
            pub trait Events { fn playing(&self, source: MediaSource); }
        }
    "#,
    )
    .unwrap();
    let descriptor = lower::protocol_descriptor(&items.protocols[0].1).unwrap();
    assert!(descriptor.types.record_def("MediaSource").is_some());
    assert_eq!(
        descriptor.events[0].params[0].ty,
        bern_protocol::Type::Record {
            name: "MediaSource".into()
        }
    );
}

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
        (quote! { Spacer(min: 1e999) }, "out of range for f64"),
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

const QUERY_PROTOCOL: &str = r#"
    #[bern::protocol(namespace = "library", version = 1)]
    pub mod library {
        use bern::prelude::*;

        #[value]
        pub struct Item { pub id: i64, pub title: String }

        #[state]
        pub struct State { pub items: Vec<Item> }

        #[commands]
        pub trait Commands {
            fn add(&mut self, title: String) -> Result<i64, CommandError>;
        }

        #[queries]
        pub trait Queries {
            /// Items whose title contains `text`
            #[reads(items)]
            #[errors("invalid_search")]
            fn search(&self, text: String, answer: Answer<Vec<Item>>);
        }
    }
"#;

#[test]
fn queries_lower_after_commands_as_read_only_overlapping_operations() {
    let items = parse_source_file(QUERY_PROTOCOL).unwrap();
    let protocol = &items.protocols[0].1;
    let queries = protocol.queries.as_ref().unwrap();
    assert_eq!(queries.methods[0].name, "search");
    assert_eq!(queries.methods[0].reads[0], "items");
    let descriptor = lower::protocol_descriptor(protocol).unwrap();
    assert_eq!(descriptor.commands.len(), 2);
    let (index, search) = descriptor.command("library.search").unwrap();
    assert_eq!(index, 1);
    assert!(search.is_query());
    assert_eq!(search.doc, "Items whose title contains `text`");
    assert_eq!(search.params[0].name, "text");
    assert_eq!(
        search.result,
        bern_protocol::Type::list(bern_protocol::Type::Record {
            name: "Item".into()
        })
    );
    assert_eq!(search.reads, vec!["items"]);
    assert_eq!(search.errors, vec!["invalid_search"]);
    assert_eq!(
        search.policy.concurrency,
        bern_protocol::Concurrency::Overlapping
    );
    assert!(search.policy.cancellable);
    assert_eq!(search.external.cli, vec!["library", "search"]);
    assert_eq!(search.external.mcp, "library_search");
}

#[test]
fn query_declarations_must_read_and_answer_through_answer() {
    for (from, to, needle) in [
        (
            "fn search(&self",
            "fn search(&mut self",
            "a query takes `&self`",
        ),
        (
            ", answer: Answer<Vec<Item>>);",
            ") -> Vec<Item>;",
            "answers through its last parameter",
        ),
        (
            "answer: Answer<Vec<Item>>);",
            "answer: Answer<Vec<Item>>) -> bool;",
            "returns nothing",
        ),
        (
            "#[reads(items)]",
            "#[reads(count)]",
            "`count` is not a field",
        ),
        (
            "#[reads(items)]",
            "#[idempotent]",
            "not supported on queries",
        ),
    ] {
        let source = QUERY_PROTOCOL.replace(from, to);
        let result = parse_source_file(&source)
            .and_then(|items| lower::protocol_descriptor(&items.protocols[0].1).map(|_| ()));
        let error = result.unwrap_err().to_string();
        assert!(error.contains(needle), "{to}: {error}");
    }
}

#[test]
fn only_the_query_property_holds_a_call() {
    let body = parse_ui(quote! {
        Query(id: results, query: app.search(draft.text, 3)) |items| { Spacer }
    })
    .unwrap();
    let Node::Element(element) = &body.nodes[0] else {
        panic!()
    };
    let Expr::Query(call) = &element.prop("query").unwrap().value else {
        panic!()
    };
    assert_eq!(call.receiver_path(), "app");
    assert_eq!(call.method, "search");
    assert_eq!(call.args.len(), 2);
    // A component argument named `query` is still a plain binding.
    let body = parse_ui(quote! { page(query: draft.query) }).unwrap();
    let Node::Component(call) = &body.nodes[0] else {
        panic!()
    };
    assert!(matches!(call.args[0].value, Expr::Path(_)));
    for (tokens, needle) in [
        (
            quote! { Text(content: app.search(draft.text)) },
            "calls are not allowed",
        ),
        (
            quote! { Query(id: results, query: search(draft.text)) |items| { Spacer } },
            "called on a typed client",
        ),
    ] {
        let error = parse_ui(tokens).unwrap_err().to_string();
        assert!(error.contains(needle), "{error}");
    }
}
