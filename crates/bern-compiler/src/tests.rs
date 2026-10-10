use super::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn showcase() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/showcase/Bern.toml")
}

#[test]
fn showcase_compiles_to_ir() {
    let config = config::load(&showcase()).unwrap();
    let compiled = match compile(&config) {
        Ok(compiled) => compiled,
        Err(errors) => panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    };
    let app = &compiled.app;
    assert_eq!(compiled.protocol_module, "showcase_protocol::showcase");
    assert_eq!(app.windows.primary, "main");
    let main = app.window("main").unwrap();
    assert_eq!(main.initial, ir::WindowInitial::Open);
    assert_eq!(main.sessions, vec!["MainDraft"]);
    let settings = app.window("settings").unwrap();
    assert_eq!(settings.role, ir::WindowRole::Settings);
    assert_eq!(settings.sessions, vec!["SettingsDraft"]);
    assert!(app.component("main_window").is_some());
    let widget = app.component("item_count").expect("library component");
    assert_eq!(widget.module, vec!["showcase_widgets"]);
    let library = &compiled.libraries[0];
    assert_eq!(library.package, "showcase-widgets");
    assert_eq!(library.version, "0.1.0");
    assert_eq!(library.descriptor_version, ir::LIBRARY_DESCRIPTOR_VERSION);
    assert!(
        app.component("app").is_none(),
        "the root becomes the window catalog"
    );
    let draft = app.session("MainDraft").unwrap();
    let volume = &draft.fields[draft.field_index("volume").unwrap()];
    assert_eq!(volume.initial, Value::Float(50.0));
    // Every text has a unique key and complete translations.
    let mut keys = BTreeSet::new();
    for text in &app.texts {
        assert!(keys.insert(text.key.clone()), "duplicate key {}", text.key);
        assert_eq!(text.translations.len(), 2, "{}", text.key);
    }
    assert!(keys.contains("Page.Text"));
    assert!(keys.contains("app.name"));
    // Declared resources, with the manifest entry that declared each.
    let names: Vec<&str> = app.resources.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        ["media/aurora.mp4", "media/broken.mp4", "media/tides.mp4"]
    );
    let declared = &app.resources[0].declared;
    assert!(declared.file.ends_with("Bern.toml"), "{}", declared.file);
    assert!(declared.line > 1 && declared.column > 1, "{declared:?}");
    // The IR is serializable and versioned.
    let json = serde_json::to_string(app).unwrap();
    let back: ir::Application = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, app);
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Mode {
        #[label(tr! { "en" => "Fast", "ja" => "速い" })]
        Fast,
        #[label(tr! { "en" => "Slow", "ja" => "遅い" })]
        Slow,
    }

    #[state]
    pub struct State {
        pub count: i64,
        pub title: String,
        pub mode: Mode,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}
"#;

/// Write a throwaway project whose UI is `ui` and compile it.
fn compile_ui(ui: &str) -> Result<Compiled, Vec<Diagnostic>> {
    compile_project(PROTOCOL, ui)
}

fn compile_project(protocol: &str, ui: &str) -> Result<Compiled, Vec<Diagnostic>> {
    let dir = std::env::temp_dir().join(format!(
        "bern-compiler-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for (package, source) in [
        ("demo-protocol", protocol),
        ("demo-ui", ui),
        ("demo-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    std::fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "demo"
bundle_id = "dev.bern.demo"
version = "0.1.0"
name = { "en" = "Demo", "ja" = "デモ" }

[languages]
supported = ["en", "ja"]
default = "en"

[protocol]
package = "demo-protocol"
source = "demo-protocol/src/lib.rs"

[engine]
package = "demo-core"
type = "demo_core::Engine"

[ui]
package = "demo-ui"
sources = ["demo-ui/src/lib.rs"]
root = "app"

[targets.macos]
platform = "macos"
"#,
    )
    .unwrap();
    let config = config::load(&dir.join("Bern.toml"))?;
    let result = compile(&config);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn wrap(body: &str) -> String {
    format!(
        r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {{
    pub name: String,
    pub on: bool,
    pub mode: demo::Mode,
}}

#[component]
pub fn app(app: demo::Client, windows: AppWindows) -> impl View {{
    ui! {{
        WindowSet(handle: AppWindows, primary: main) {{
            Window(id: main, title: tr! {{ "en" => "Main", "ja" => "メイン" }}, initial: open) {{
                page(app: app, windows: windows)
            }}
            Window(id: about, title: tr! {{ "en" => "About", "ja" => "情報" }}) {{
                about_page()
            }}
        }}
    }}
}}

#[component]
pub fn about_page() -> impl View {{
    ui! {{ Spacer }}
}}

#[component]
pub fn page(app: demo::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {{
    ui! {{
        {body}
    }}
}}
"#
    )
}

fn errors(body: &str) -> String {
    match compile_ui(&wrap(body)) {
        Ok(_) => panic!("expected `{body}` to fail"),
        Err(errors) => errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[test]
fn valid_declarations_compile() {
    compile_ui(&wrap(
        r#"Column {
            Text(content: tr! { "en" => "Count {n}", "ja" => "数 {n}"; n: app.count })
            TextInput(id: name, label: tr! { "en" => "Name", "ja" => "名前" }, value: draft.name)
            Button(id: add, label: tr! { "en" => "Add", "ja" => "追加" }) => app.add(1)
            Button(id: about, label: tr! { "en" => "About", "ja" => "情報" }) => windows.about.open()
            match draft.mode {
                Mode::Fast => { Text(content: app.mode) }
                Mode::Slow => { Spacer }
            }
        }"#,
    ))
    .unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
        )
    });
}

#[test]
fn diagnostics_cover_the_documented_failures() {
    let cases = [
        (
            r#"Text(content: tr! { "en" => "Hello" })"#,
            "error[B102]: Missing translation\nMissing language: ja",
        ),
        (r#"Text(content: "Hello")"#, "error[B103]"),
        (
            r#"Button(id: go, label: tr! { "en" => "Go", "ja" => "行く" })"#,
            "error[B301]",
        ),
        (
            r#"Button(id: go, label: tr! { "en" => "Go", "ja" => "行く" }) => app.launch()"#,
            "has no command `launch`",
        ),
        (
            r#"TextInput(id: t, label: tr! { "en" => "T", "ja" => "T" }, value: app.title)"#,
            "controls only write to session state",
        ),
        (
            r#"Button(id: go, label: tr! { "en" => "Go", "ja" => "行く" }) => app.add(draft.name)"#,
            "argument `amount` must be int, found text",
        ),
        (
            r#"match draft.mode { Mode::Fast => { Spacer } }"#,
            "non-exhaustive match; missing Mode::Slow",
        ),
        (
            r#"Button(id: go, label: tr! { "en" => "Go", "ja" => "行く" }) => windows.help.open()"#,
            "window `help` is not declared",
        ),
        (
            r#"Text(content: draft.on)"#,
            "bool has no text presentation",
        ),
        (
            r#"Text(content: tr! { "en" => "{a}", "ja" => "{b}"; a: app.count })"#,
            "same placeholders",
        ),
        (
            r#"Text(content: tr! { "en" => "Hi", "ja" => "やあ", "fr" => "Salut" })"#,
            "language tag `fr` is not in [languages].supported",
        ),
        (r#"page(app: app, windows: windows)"#, "recursively"),
        (r#"missing_page()"#, "unknown component `missing_page`"),
        (
            r#"Column { Switch(id: a, label: tr! { "en" => "A", "ja" => "A" }, value: draft.on) Switch(id: a, label: tr! { "en" => "B", "ja" => "B" }, value: draft.on) }"#,
            "control id `a` is used twice",
        ),
    ];
    for (body, expected) in cases {
        let output = errors(body);
        assert!(
            output.contains(expected),
            "`{body}` should report `{expected}`, got:\n{output}"
        );
    }
}

#[test]
fn diagnostics_point_at_the_declaration() {
    let output = errors(r#"Text(content: "Hello")"#);
    assert!(output.contains("--> demo-ui/src/lib.rs:"), "{output}");
}

#[test]
fn protocol_labels_follow_project_languages() {
    let protocol = PROTOCOL.replace(r#", "ja" => "速い""#, "");
    let output = match compile_project(&protocol, &wrap("Spacer")) {
        Ok(_) => panic!("a protocol label without `ja` must fail"),
        Err(errors) => errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    };
    assert!(output.contains("error[B102]"), "{output}");
    assert!(output.contains("demo-protocol/src/lib.rs"), "{output}");
}

/// A target registered from outside the compiler consumes the same IR.
mod independent_target {
    use super::*;
    use bern_target_api::{
        BuildContext, BuildPlan, ColorSource, GeneratedFile, Registry, Target, TargetConfig,
        TargetDescriptor, WindowCapabilities, check_common,
    };

    struct Outline;

    impl Target for Outline {
        fn descriptor(&self) -> TargetDescriptor {
            TargetDescriptor {
                id: "outline-test",
                ui: "outline",
                platforms: &["test"],
                version: "0",
                ir_versions: IR_VERSION..=IR_VERSION,
                color_source: ColorSource::BernTheme,
                components: &["Form", "Section", "Text"],
                windows: WindowCapabilities {
                    native: false,
                    virtual_windows: true,
                    floating: false,
                },
                channels: &[],
                media: None,
            }
        }
        fn check(&self, app: &ir::Application, _: &TargetConfig) -> Vec<Diagnostic> {
            check_common(self, app)
        }
        fn generate(
            &self,
            app: &ir::Application,
            _: &BuildContext,
        ) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
            let outline: Vec<String> = app.components.iter().map(|c| c.name.clone()).collect();
            Ok(vec![GeneratedFile::text("outline.txt", outline.join("\n"))])
        }
        fn plan(
            &self,
            _: &ir::Application,
            _: &BuildContext,
        ) -> Result<BuildPlan, Vec<Diagnostic>> {
            Ok(BuildPlan::default())
        }
    }

    #[test]
    fn new_targets_register_without_compiler_changes() {
        let config = config::load(&showcase()).unwrap();
        let app = compile(&config).ok().unwrap().app;
        let mut registry = Registry::new();
        registry.register(Box::new(Outline));
        let target = registry
            .resolve(&TargetConfig {
                name: "outline".into(),
                platform: "test".into(),
                ui: Some("outline".into()),
                web_mode: None,
                channels: vec![],
            })
            .unwrap();
        // The outline target lacks most components and reports each one
        // with its source location instead of generating partial UI.
        let problems = target.check(&app, &TargetConfig::default());
        assert!(
            problems
                .iter()
                .any(|d| d.message.contains("does not support `SplitView`"))
        );
        // Video needs a media backend the target does not have.
        assert!(
            problems
                .iter()
                .any(|d| d.message.contains("has no media backend"))
        );
        assert!(problems.iter().all(|d| d.location.is_some()));
        // A floating window needs a target that can draw one.
        let mut floating = app.clone();
        floating.windows.windows[1].presentation = ir::Presentation::Floating;
        let problems = target.check(&floating, &TargetConfig::default());
        assert!(problems.iter().any(|d| {
            d.message
                .contains("window `settings` requests floating presentation")
        }));
    }
}

/// A library that lacks a language the application requires is rejected
/// with the same B102 rule, naming the library and its version.
#[test]
fn libraries_must_cover_the_application_languages() {
    let dir = std::env::temp_dir().join(format!(
        "bern-library-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let widgets = dir.join("widgets");
    std::fs::create_dir_all(widgets.join("src")).unwrap();
    std::fs::write(
        widgets.join("Cargo.toml"),
        "[package]\nname = \"widgets\"\nversion = \"2.3.0\"\n",
    )
    .unwrap();
    std::fs::write(
        widgets.join("Bern.toml"),
        "[library]\npackage = \"widgets\"\nsources = [\"src/lib.rs\"]\n\n[languages]\nsupported = [\"en\"]\ndefault = \"en\"\n",
    )
    .unwrap();
    std::fs::write(
        widgets.join("src/lib.rs"),
        r#"
        #[component]
        pub fn badge(count: i64) -> impl View {
            ui! { Text(content: tr! { "en" => "{n} new"; n: count }) }
        }
        "#,
    )
    .unwrap();
    let library = config::load_library(&widgets.join("Bern.toml")).unwrap();
    let described = compile_library(&library).unwrap();
    assert_eq!(described.version, "2.3.0");
    assert_eq!(described.components[0].module, vec!["widgets"]);
    // The application requires `en` and `ja`.
    let app = config::load(&showcase()).unwrap();
    let mut app = app;
    app.languages = vec!["en".into(), "ja".into()];
    let problems = check_library(&described, &app);
    let output = problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        output.contains("error[B102]") && output.contains("widgets 2.3.0") && output.contains("ja"),
        "{output}"
    );
    // A library cannot hold engine clients or session state.
    std::fs::write(
        widgets.join("src/lib.rs"),
        r#"
        #[component]
        pub fn panel(app: demo::Client) -> impl View { ui! { Spacer } }
        "#,
    )
    .unwrap();
    let errors =
        compile_library(&config::load_library(&widgets.join("Bern.toml")).unwrap()).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("value parameters only"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const MEDIA_PROTOCOL: &str = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub count: i64,
        pub clip: Option<MediaSource>,
        pub saved: String,
    }

    #[commands]
    pub trait Commands {
        fn save(&mut self, name: String) -> Result<(), CommandError>;
    }
}
"#;

/// A project with application players, a media page and close checks.
fn media_project(window_props: &str, window_action: &str, body: &str) -> String {
    format!(
        r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {{
    pub name: String,
}}

#[bern::session]
pub struct Other {{
    pub note: String,
}}

#[bern::players]
pub struct Players {{
    /// The application's preview player.
    pub preview: Player,
}}

#[component]
pub fn app(
    app: demo::Client,
    windows: AppWindows,
    players: Players,
    #[session] draft: Draft,
    #[session] other: Other,
) -> impl View {{
    ui! {{
        WindowSet(handle: AppWindows, primary: main) {{
            Window(id: main, title: tr! {{ "en" => "Main", "ja" => "メイン" }}, initial: open{window_props}) {{
                page(app: app, players: players)
            }}{window_action}
            Window(id: notes, title: tr! {{ "en" => "Notes", "ja" => "メモ" }}) {{
                notes_page()
            }}
        }}
    }}
}}

#[component]
pub fn notes_page(#[session] other: Other) -> impl View {{
    ui! {{ TextInput(id: note, label: tr! {{ "en" => "Note", "ja" => "メモ" }}, value: other.note) }}
}}

#[component]
pub fn page(app: demo::Client, players: Players, #[session] draft: Draft) -> impl View {{
    ui! {{
        {body}
    }}
}}
"#
    )
}

fn media_errors(window_props: &str, window_action: &str, body: &str) -> String {
    match compile_project(
        MEDIA_PROTOCOL,
        &media_project(window_props, window_action, body),
    ) {
        Ok(_) => panic!("expected `{window_props}` / `{body}` to fail"),
        Err(errors) => errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

const CLOSE_CHECK: &str = r#", confirm_close: draft.name != app.saved, close_message: tr! { "en" => "Save the name?", "ja" => "名前を保存しますか？" }"#;

#[test]
fn video_players_and_close_checks_lower_to_ir() {
    let compiled = compile_project(
        MEDIA_PROTOCOL,
        &media_project(
            CLOSE_CHECK,
            " => app.save(draft.name)",
            r#"Column {
                Video(id: clip, source: app.clip, fit: cover)
                Video(id: preview, player: players.preview, controls: none, label: tr! { "en" => "Preview", "ja" => "プレビュー" })
                Button(id: play, label: tr! { "en" => "Play", "ja" => "再生" }) => players.preview.play()
                Button(id: seek, label: tr! { "en" => "Start", "ja" => "最初" }) => players.preview.seek(0)
                if let Some(clip) = app.clip {
                    Button(id: open, label: tr! { "en" => "Open", "ja" => "開く" }) => players.preview.open(clip)
                }
                TextInput(id: name, label: tr! { "en" => "Name", "ja" => "名前" }, value: draft.name)
            }"#,
        ),
    )
    .unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
        )
    });
    let app = &compiled.app;
    assert_eq!(
        app.players,
        vec![ir::PlayerDef {
            id: "preview".into(),
            doc: "The application's preview player.".into()
        }]
    );
    assert!(app.uses_media());
    assert!(app.types.record_def(bern_media::MEDIA_SOURCE).is_some());
    let main = app.window("main").unwrap();
    let close = main.close.as_ref().expect("close check");
    assert_eq!(close.when.sessions(), vec!["Draft".to_string()]);
    assert!(matches!(close.save, Some(ir::Action::Engine { .. })));
    assert!(app.window("notes").unwrap().close.is_none());
    let page = app.component("page").unwrap();
    let mut actions = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node
            && let Some(ir::Action::Media { player, op, .. }) = &element.action
        {
            actions.push((player.clone(), *op));
        }
    });
    assert_eq!(
        actions,
        vec![
            ("preview".to_string(), ir::MediaOp::Play),
            ("preview".to_string(), ir::MediaOp::Seek),
            ("preview".to_string(), ir::MediaOp::Open),
        ]
    );
}

#[test]
fn video_and_close_diagnostics() {
    let video = [
        (r#"Video(id: v)"#, "exactly one of `source:`"),
        (
            r#"Video(id: v, source: app.clip, player: players.preview)"#,
            "exactly one of `source:`",
        ),
        (
            r#"Video(id: v, source: app.count)"#,
            "`source` needs a MediaSource or Option<MediaSource>, found int",
        ),
        (
            r#"Video(id: v, player: players.extra)"#,
            "player `extra` is not declared",
        ),
        (
            r#"Button(id: b, label: tr! { "en" => "B", "ja" => "B" }) => players.preview.rewind()"#,
            "players support open, play, pause, seek, set_volume, set_muted",
        ),
        (
            r#"Button(id: b, label: tr! { "en" => "B", "ja" => "B" }) => players.preview.seek(draft.name)"#,
            "argument `position_ms` must be int, found text",
        ),
        (
            r#"Text(content: players.preview)"#,
            "players are used by `Video(player: …)`",
        ),
    ];
    for (body, expected) in video {
        let output = media_errors("", "", body);
        assert!(
            output.contains(expected),
            "`{body}` should report `{expected}`, got:\n{output}"
        );
    }
    let close = [
        (
            r#", confirm_close: draft.name != """#,
            "",
            "declared together",
        ),
        ("", " => app.save(draft.name)", "needs `confirm_close:`"),
        (
            r#", confirm_close: other.note != "", close_message: tr! { "en" => "Discard?", "ja" => "破棄しますか？" }"#,
            "",
            "reads `Other`, which this window does not own",
        ),
        (
            CLOSE_CHECK,
            " => windows.notes.open()",
            "must be an engine command",
        ),
    ];
    for (props, action, expected) in close {
        let output = media_errors(props, action, "Spacer");
        assert!(
            output.contains(expected),
            "`{props}{action}` should report `{expected}`, got:\n{output}"
        );
    }
    let declared = MEDIA_PROTOCOL.replace(
        "#[state]",
        "#[value]\n    pub struct MediaSource { pub url: String }\n\n    #[state]",
    );
    let output = match compile_project(&declared, &media_project("", "", "Spacer")) {
        Ok(_) => panic!("declaring MediaSource must fail"),
        Err(errors) => errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    };
    assert!(
        output.contains("`MediaSource` is a framework type"),
        "{output}"
    );
}

const QUERY_PROTOCOL: &str = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Mode {
        #[label(tr! { "en" => "Fast", "ja" => "速い" })]
        Fast,
        #[label(tr! { "en" => "Slow", "ja" => "遅い" })]
        Slow,
    }

    #[value]
    pub struct Hit {
        pub id: i64,
        pub title: String,
    }

    #[state]
    pub struct State {
        pub count: i64,
        pub title: String,
        pub mode: Mode,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }

    #[queries]
    pub trait Queries {
        #[reads(count)]
        fn find(&self, text: String, answer: Answer<Vec<Hit>>);
        fn first(&self, text: String, answer: Answer<Option<Hit>>);
        fn total(&self, answer: Answer<i64>);
    }
}
"#;

fn query_errors(body: &str) -> String {
    match compile_project(QUERY_PROTOCOL, &wrap(body)) {
        Ok(_) => panic!("expected `{body}` to fail"),
        Err(errors) => errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[test]
fn query_bindings_lower_with_typed_templates() {
    let compiled = compile_project(
        QUERY_PROTOCOL,
        &wrap(
            r#"Column {
            Query(
                id: hits,
                query: app.find(draft.name),
                loading: tr! { "en" => "Searching…", "ja" => "検索中…" },
                empty: tr! { "en" => "No hits for {text}", "ja" => "{text} は見つかりません"; text: draft.name },
                icon: "magnifyingglass"
            ) |hits| {
                ForEach(items: hits, key: id) |hit| { Text(content: hit.title) }
            }
            Query(id: best, query: app.first(draft.name), empty: tr! { "en" => "None", "ja" => "なし" }) |hit| {
                Text(content: hit.title)
            }
            Query(id: total, query: app.total()) |count| { Text(content: count) }
        }"#,
        ),
    )
    .unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
        )
    });
    let app = &compiled.app;
    let find = app.protocol.command("demo.find").unwrap().1;
    assert!(find.is_query());
    assert_eq!(find.reads, vec!["count"]);
    let page = app.component("page").unwrap();
    let mut queries = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node
            && element.kind == "Query"
        {
            queries.push(element.clone());
        }
    });
    assert_eq!(queries.len(), 3);
    let hits = &queries[0];
    assert_eq!(hits.id.as_deref(), Some("hits"));
    let binding = hits.query("query").unwrap();
    assert_eq!(binding.query, 1);
    assert_eq!(binding.args.len(), 1);
    let hit = Type::Record { name: "Hit".into() };
    assert_eq!(binding.result, Type::list(hit.clone()));
    assert_eq!(hits.template.as_ref().unwrap().ty, Type::list(hit.clone()));
    assert!(hits.text("empty").is_some() && hits.text("loading").is_some());
    // An optional result shows the value inside it.
    assert_eq!(queries[1].template.as_ref().unwrap().ty, hit);
    assert_eq!(queries[2].template.as_ref().unwrap().ty, Type::Int);
}

#[test]
fn query_diagnostics() {
    let cases = [
        (
            r#"Button(id: go, label: tr! { "en" => "Go", "ja" => "行く" }) => app.find(draft.name)"#,
            "`find` is a query",
        ),
        (
            r#"Query(id: q, query: app.add(1)) |x| { Spacer }"#,
            "`add` is a command",
        ),
        (
            r#"Query(id: q, query: app.lookup(1)) |x| { Spacer }"#,
            "has no query `lookup`",
        ),
        (
            r#"Query(id: q, query: app.total(), empty: tr! { "en" => "None", "ja" => "なし" }) |n| { Spacer }"#,
            "`empty:` needs a query whose result can be empty",
        ),
        (
            r#"Query(id: q, query: app.find(draft.on)) |x| { Spacer }"#,
            "argument `text` must be text, found bool",
        ),
        (
            r#"Query(id: q, query: app.find(draft.name)) |hits| { Text(content: hits) }"#,
            "has no text presentation",
        ),
        (
            r#"Query(id: q, query: windows.main.find(draft.name)) |x| { Spacer }"#,
            "not the typed engine client",
        ),
        (r#"Query(id: q, query: app.total())"#, "needs a template"),
        (
            r#"Text(content: app.find(draft.name))"#,
            "calls are not allowed",
        ),
    ];
    for (body, expected) in cases {
        let output = query_errors(body);
        assert!(
            output.contains(expected),
            "`{body}` should report `{expected}`, got:\n{output}"
        );
    }
}
