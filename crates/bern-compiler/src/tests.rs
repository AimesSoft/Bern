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
        [
            "images/bern.png",
            "images/hdr-gainmap.jpg",
            "images/hdr-pq.png",
            "images/landscape.jpg",
            "media/aurora.mp4",
            "media/broken.mp4",
            "media/tides.mp4"
        ]
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
    compile_project_with(protocol, ui, "")
}

/// Like `compile_project`, with extra `Bern.toml` tables.
fn compile_project_with(
    protocol: &str,
    ui: &str,
    manifest: &str,
) -> Result<Compiled, Vec<Diagnostic>> {
    let dir = write_project(protocol, ui, manifest);
    let config = config::load(&dir.join("Bern.toml"));
    let result = config.and_then(|config| compile(&config));
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// A throwaway project in a new directory, with extra `Bern.toml` tables.
fn write_project(protocol: &str, ui: &str, manifest: &str) -> std::path::PathBuf {
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
"#
        .to_string()
            + manifest,
    )
    .unwrap();
    dir
}

#[test]
fn windows_builds_both_front_ends_unless_told_otherwise() {
    let load = |table: &str| {
        let dir = write_project(
            PROTOCOL,
            "",
            &format!("\n[targets.windows]\nplatform = \"windows\"\n{table}"),
        );
        let result = config::load(&dir.join("Bern.toml"));
        let _ = std::fs::remove_dir_all(&dir);
        result
    };
    let frontends = |config: &config::Config| {
        let windows = config
            .targets
            .iter()
            .find(|target| target.name == "windows")
            .unwrap();
        windows.frontends.clone()
    };
    assert_eq!(frontends(&load("").unwrap()), ["winui3", "xaml-islands"]);
    assert_eq!(
        frontends(&load("frontends = [\"winui3\"]").unwrap()),
        ["winui3"]
    );
    assert_eq!(
        frontends(&load("frontends = [\"xaml-islands\"]").unwrap()),
        ["xaml-islands"]
    );
    let macos = load("").unwrap();
    assert!(
        macos
            .targets
            .iter()
            .find(|target| target.name == "macos")
            .unwrap()
            .frontends
            .is_empty()
    );

    let message = |table: &str| load(table).unwrap_err()[0].message.clone();
    assert!(message("frontends = []").contains("lists none"));
    assert!(message("frontends = [\"uwp\"]").contains("unknown front end `uwp`"));
    assert!(message("frontends = [\"winui3\", \"winui3\"]").contains("twice"));
    let dir = write_project(PROTOCOL, "", "frontends = [\"winui3\"]\n");
    let error = config::load(&dir.join("Bern.toml")).unwrap_err();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        error[0].message.contains("Windows targets only"),
        "{error:?}"
    );
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
            "error[B104]: every translation must use the same placeholders",
        ),
        (
            r#"Text(content: tr! { "en" => "{n}", "ja" => "{n}" })"#,
            "error[B104]: placeholder `{n}` has no argument",
        ),
        (
            r#"Text(content: tr! { "en" => "Hi", "ja" => "やあ"; n: app.count })"#,
            "error[B104]: argument `n` is not used",
        ),
        (
            r#"Text(content: tr! { "en" => "Hi", "ja" => "やあ", "fr" => "Salut" })"#,
            "error[B101]: language tag `fr` is not in [languages].supported",
        ),
        (
            r#"Text(content: tr! { "en" => "Hi", "en" => "Hey", "ja" => "やあ" })"#,
            "error[B101]: language `en` is translated twice",
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
                styles: &[],
                motion: &[],
                window_chrome: false,
                text_marks: false,
                dynamic_items: false,
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
                frontends: vec![],
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

/// Phase four: adaptive layouts and feature contracts.
mod contracts {
    use super::*;

    const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub count: i64,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
        fn reset(&mut self) -> Result<(), CommandError>;
    }
}
"#;

    const LAYOUTS: &str = "\n[layouts]\ncompact_below = 600\n";

    const ADD: &str = r#"Button(id: add, label: tr! { "en" => "Add", "ja" => "追加" }, feature: PageFeature::Add) => app.add(1)"#;

    fn project(features: &str, body: &str) -> String {
        format!(
            r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {{
    pub on: bool,
}}

#[bern::features]
pub enum PageFeature {{
    /// Add one.
    #[commands(add)]
    {features}
    Add,
}}

#[component]
pub fn app(app: demo::Client, windows: AppWindows) -> impl View {{
    ui! {{
        WindowSet(handle: AppWindows, primary: main) {{
            Window(id: main, title: tr! {{ "en" => "Main", "ja" => "メイン" }}, initial: open) {{
                Column {{
                    page(app: app)
                    other(app: app)
                }}
            }}
        }}
    }}
}}

#[component(features = PageFeature)]
pub fn page(app: demo::Client, #[session] draft: Draft) -> impl View {{
    ui! {{
        {body}
    }}
}}

#[component]
pub fn other(app: demo::Client) -> impl View {{
    ui! {{ Spacer }}
}}
"#
        )
    }

    fn compiled(features: &str, body: &str) -> Compiled {
        compile_project_with(PROTOCOL, &project(features, body), LAYOUTS).unwrap_or_else(|errors| {
            panic!(
                "{}",
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
    }

    fn failure(features: &str, body: &str, manifest: &str) -> String {
        match compile_project_with(PROTOCOL, &project(features, body), manifest) {
            Ok(_) => panic!("expected `{body}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// The report on `platform`, as text.
    fn report(compiled: &Compiled, platform: &str) -> String {
        bern_target_api::feature_report(&compiled.app, platform)
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn layouts(compact: &str, expanded: &str) -> String {
        format!(
            "Adaptive(id: choice) {{ Layout(size: expanded) {{ {expanded} }} Layout(size: compact) {{ {compact} }} }}"
        )
    }

    #[test]
    fn a_feature_reached_in_every_size_is_covered() {
        let menu = format!(
            r#"Menu(id: more, label: tr! {{ "en" => "More", "ja" => "その他" }}) {{ {} }}"#,
            ADD.replace("Button", "MenuItem")
        );
        let compiled = compiled("", &layouts(&menu, ADD));
        let contract = &compiled.app.contracts[0];
        assert_eq!(contract.component, "page");
        let feature = &contract.features[0];
        assert_eq!(feature.commands, ["demo.add"]);
        for size in &feature.coverage {
            assert_eq!(
                size.coverage,
                ir::Coverage::Covered {
                    entries: vec!["page.add".into()]
                },
                "{size:?}"
            );
        }
        assert!(report(&compiled, "macos").is_empty());
    }

    #[test]
    fn a_feature_missing_in_one_size_is_b201() {
        let compiled = compiled("", &layouts("Spacer", ADD));
        let output = report(&compiled, "macos");
        assert!(
            output.contains("error[B201]: Feature missing in adaptive layout\nContract: PageFeature\nFeature: Add\ncompact: missing"),
            "{output}"
        );
        assert!(!output.contains("expanded: missing"), "{output}");
    }

    #[test]
    fn a_business_entry_without_a_feature_is_b202() {
        let body = format!(
            r#"Column {{ {ADD} Button(id: reset, label: tr! {{ "en" => "Reset", "ja" => "リセット" }}) => app.reset() }}"#
        );
        let output = failure("", &body, LAYOUTS);
        assert!(output.contains("error[B202]"), "{output}");
        assert!(
            output.contains("`page.reset` runs `demo.reset`"),
            "{output}"
        );
    }

    #[test]
    fn a_conditional_entry_is_unproven_b203_or_guaranteed_b206() {
        let body = format!("if draft.on {{ {ADD} }}");
        let output = report(&compiled("", &body), "macos");
        assert!(output.contains("error[B203]"), "{output}");
        let output = report(
            &compiled(r#"#[guaranteed("shown whenever the draft is on")]"#, &body),
            "macos",
        );
        assert!(output.contains("warning[B206]"), "{output}");
        assert!(!output.contains("error"), "{output}");
    }

    #[test]
    fn every_branch_reaching_a_feature_is_proven() {
        let body = format!(
            "if draft.on {{ {ADD} }} else {{ {} }}",
            ADD.replace("id: add", "id: add_too")
        );
        let compiled = compiled("", &body);
        assert!(report(&compiled, "macos").is_empty());
        assert!(matches!(
            compiled.app.contracts[0].features[0].coverage[0].coverage,
            ir::Coverage::Covered { .. }
        ));
    }

    #[test]
    fn wrong_commands_and_stray_features_are_b204() {
        let body = r#"Button(id: reset, label: tr! { "en" => "Reset", "ja" => "リセット" }, feature: PageFeature::Add) => app.reset()"#;
        let output = failure("", body, LAYOUTS);
        assert!(output.contains("error[B204]"), "{output}");
        assert!(
            output.contains("the feature is implemented by demo.add"),
            "{output}"
        );
        let stray = project("", ADD).replace(
            "ui! { Spacer }",
            r#"ui! { Button(id: x, label: tr! { "en" => "X", "ja" => "X" }, feature: PageFeature::Add) => app.add(1) }"#,
        );
        let output = match compile_project_with(PROTOCOL, &stray, LAYOUTS) {
            Ok(_) => panic!("a feature outside its scope must fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        assert!(output.contains("error[B204]"), "{output}");
        assert!(
            output.contains("outside the scope of contract `PageFeature`"),
            "{output}"
        );
    }

    #[test]
    fn an_exemption_applies_to_its_platform_only_b205() {
        let compiled = compiled(
            r#"#[exempt(macos, "no such control on this platform")]"#,
            "Spacer",
        );
        let output = report(&compiled, "macos");
        assert!(output.contains("note[B205]"), "{output}");
        assert!(!output.contains("error"), "{output}");
        assert!(report(&compiled, "browser").contains("error[B201]"));
    }

    #[test]
    fn adaptive_needs_every_size_b401_and_breakpoints_b402() {
        let only_expanded = format!("Adaptive {{ Layout(size: expanded) {{ {ADD} }} }}");
        let output = failure("", &only_expanded, LAYOUTS);
        assert!(
            output.contains("error[B401]: `Adaptive` needs exactly one `Layout(size: compact)`"),
            "{output}"
        );
        let output = failure("", &layouts(ADD, ADD), "");
        assert!(output.contains("error[B402]"), "{output}");
    }

    #[test]
    fn the_same_control_id_names_one_control_across_layouts() {
        // Duplicates within one layout are still errors.
        let twice = layouts(&format!("Column {{ {ADD} {ADD} }}"), ADD);
        let output = failure("", &twice, LAYOUTS);
        assert!(
            output.contains("control id `add` is used twice"),
            "{output}"
        );
    }
}

/// Compile `body` in a project that declares the resource `pics/a.png`.
fn with_picture(body: &str) -> Result<Compiled, Vec<Diagnostic>> {
    let dir = write_project(
        PROTOCOL,
        &wrap(body),
        "\n[resources]\npaths = [\"pics/a.png\"]\n",
    );
    std::fs::create_dir_all(dir.join("pics")).unwrap();
    std::fs::write(dir.join("pics/a.png"), b"\x89PNG").unwrap();
    let config = config::load(&dir.join("Bern.toml"));
    let result = config.and_then(|config| compile(&config));
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn images_progress_grids_and_stacks_lower_to_ir() {
    let compiled = with_picture(
        r#"Column {
            Image(source: "pics/a.png", label: tr! { "en" => "A", "ja" => "エー" }, width: 64)
            Progress(value: app.count, total: 10, label: tr! { "en" => "Done", "ja" => "完了" })
            Progress
            Grid(min_width: 100, gap: 4) {
                Stack(align: bottom_trailing) {
                    Image(source: "pics/a.png", width: 32, height: 32, fit: cover)
                    Spacer
                }
            }
        }"#,
    )
    .unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let page = compiled.app.component("page").unwrap();
    let mut kinds = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node {
            kinds.push(element.kind.clone());
            match element.kind.as_str() {
                "Image" => assert_eq!(element.resource("source"), Some("pics/a.png")),
                "Progress" if element.text("label").is_some() => {
                    assert_eq!(element.expr("value").unwrap().ty, Type::Int);
                    assert_eq!(element.number("total"), Some(10.0));
                }
                "Grid" => assert_eq!(element.number("min_width"), Some(100.0)),
                "Stack" => assert_eq!(element.keyword("align"), Some("bottom_trailing")),
                _ => {}
            }
        }
    });
    assert_eq!(
        kinds,
        [
            "Column", "Image", "Progress", "Progress", "Grid", "Stack", "Image", "Spacer"
        ]
    );
}

#[test]
fn image_and_progress_diagnostics() {
    let cases = [
        (
            r#"Image(source: "pics/b.png", width: 10)"#,
            "resource `pics/b.png` is not declared",
        ),
        (
            r#"Image(source: "pics/a.gif", width: 10)"#,
            "`pics/a.gif` is not a PNG or JPEG image",
        ),
        (
            r#"Image(source: "../a.png", width: 10)"#,
            "hidden files, `.` and `..` are not resources",
        ),
        (
            r#"Image(source: app.title, width: 10)"#,
            "expected the name of a declared resource",
        ),
        (
            r#"Image(source: "pics/a.png")"#,
            "`Image` needs `width:`, `height:` or both",
        ),
        (
            r#"Image(source: "pics/a.png", width: 0)"#,
            "`width` must be above zero",
        ),
        (
            r#"Progress(value: app.title)"#,
            "`value` must be a number, found",
        ),
        (r#"Progress(total: 5)"#, "`total:` needs a `value:`"),
        (
            r#"Grid(min_width: -4) { Spacer }"#,
            "`min_width` must be above zero",
        ),
        (
            r#"Stack(align: middle) { Spacer }"#,
            "expected one of: top_leading",
        ),
    ];
    for (body, expected) in cases {
        let message = match with_picture(body) {
            Ok(_) => panic!("expected `{body}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        assert!(message.contains(expected), "{body}\n{message}");
    }
}

#[test]
fn modules_compose_into_one_catalog_and_bind_by_namespace() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/planner/Bern.toml");
    let config = config::load(&manifest).unwrap();
    let ids: Vec<_> = config
        .modules
        .iter()
        .map(|module| module.id.as_str())
        .collect();
    assert_eq!(ids, ["notes", "tags"]);
    assert_eq!(config.modules[1].depends, ["notes"]);
    // `sources = "ui/src"` collects the directory.
    let files: Vec<_> = config.modules[0]
        .ui_sources
        .iter()
        .map(|file| file.display.as_str())
        .collect();
    assert_eq!(
        files,
        [
            "modules/notes/ui/src/lib.rs",
            "modules/notes/ui/src/list.rs",
            "modules/notes/ui/src/page.rs"
        ]
    );

    let compiled = compile(&config).unwrap();
    let protocol = &compiled.app.protocol;
    assert_eq!(protocol.modules, ["notes", "tags"]);
    let fields: Vec<_> = protocol
        .state
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    // Both modules are optional, so the switches follow them.
    assert_eq!(fields, ["title", "activity", "notes", "tags", "modules"]);
    assert!(config.modules.iter().all(|module| module.optional));
    assert!(compiled.app.types.record_def("NotesState").is_some());
    assert_eq!(
        compiled.module_protocols,
        ["notes_protocol::notes", "tags_protocol::tags"]
    );
    let list = compiled
        .app
        .components
        .iter()
        .find(|component| component.name == "notes_list")
        .unwrap();
    assert_eq!(
        list.params[0].kind,
        ir::ParamKind::Client {
            namespace: "notes".into()
        }
    );
    // `notes.count` reads the `count` field inside the `notes` state field.
    let notes = protocol.state_index("notes").unwrap();
    let mut found = false;
    list.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node {
            for prop in element.props.values() {
                if format!("{prop:?}").contains(&format!("field: {notes}, path: [1]")) {
                    found = true;
                }
            }
        }
    });
    assert!(found, "a binding reads notes.count");

    // The notes page joins the `main` page slot's enum, and the summary slot
    // and page content call the contributed components.
    let pages = compiled.app.types.enum_def("MainPage").unwrap();
    let variants: Vec<_> = pages.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(variants, ["Overview", "Archive", "Notes"]);
    assert_eq!(
        pages.variants[2].label.as_ref().unwrap().translations["en"],
        "Notes"
    );
    let mut calls = Vec::new();
    for component in &compiled.app.components {
        let mut refs = Vec::new();
        component_refs(&component.body, &mut refs);
        calls.extend(
            refs.into_iter()
                .map(|called| (component.name.clone(), called)),
        );
    }
    assert!(calls.contains(&("overview".into(), "notes_summary".into())));
    assert!(calls.contains(&("main_window".into(), "notes_page".into())));
}

#[test]
fn module_manifests_are_checked() {
    let planner = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/planner");
    let text = std::fs::read_to_string(planner.join("Bern.toml")).unwrap();
    // Tags listed before the notes module it depends on.
    let swapped = text.replace(
        "[[modules]]\npath = \"modules/notes\"\n\n[[modules]]\npath = \"modules/tags\"",
        "[[modules]]\npath = \"modules/tags\"\n\n[[modules]]\npath = \"modules/notes\"",
    );
    assert_ne!(swapped, text);
    let manifest = planner.join(format!("Bern-swapped-{}.toml", std::process::id()));
    std::fs::write(&manifest, swapped).unwrap();
    let errors = config::load(&manifest).unwrap_err();
    let _ = std::fs::remove_file(&manifest);
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("lists after it")),
        "{errors:?}"
    );
}

#[test]
fn navigation_items_may_come_and_go() {
    let sidebar = |props: &str, items: &str| {
        wrap(&format!(
            r#"SplitView {{
                Sidebar(selection: draft.mode{props}) {{
                    SidebarItem(value: Mode::Fast, label: tr! {{ "en" => "Fast", "ja" => "速い" }})
                    {items}
                }}
                Detail {{ Spacer }}
            }}"#
        ))
    };
    compile_ui(&sidebar(
        ", fallback: Mode::Fast",
        r#"if draft.on {
            SidebarItem(value: Mode::Slow, label: tr! { "en" => "Slow", "ja" => "遅い" })
        }
        match app.mode {
            Mode::Slow => { SidebarItem(value: Mode::Slow, label: tr! { "en" => "Slow", "ja" => "遅い" }) }
            _ => {}
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    for (props, items, expected) in [
        (
            "",
            "if draft.on { Text(content: app.title) }",
            "cannot contain `Text`",
        ),
        (", fallback: 3", "", "this value"),
    ] {
        let message = match compile_ui(&sidebar(props, items)) {
            Ok(_) => panic!("expected `{items}` with `{props}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        assert!(message.contains(expected), "{props} {items}: {message}");
    }
}

#[test]
fn canvas_and_text_marks_are_checked() {
    let with_style = |body: &str| {
        format!(
            "{}\nstyle! {{ pattern {{ check1: check(band(accent, 2, 0.3), gap(4), twill: true) }} }}\n",
            wrap(body)
        )
    };
    let compiled = compile_ui(&with_style(
        r#"Column {
            Text(content: app.title, mark: accent)
            Canvas(height: 40, label: tr! { "en" => "Chart", "ja" => "図" }) {
                Shape(kind: rounded, corner: 6, fill: fill.subtle)
                Pattern(name: check1, opacity: 0.5)
            }
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let pattern = compiled.app.style.pattern("check1").unwrap();
    assert_eq!(pattern.bands.len(), 2);
    assert!(pattern.bands[1].color.is_none());
    assert!(pattern.twill);

    for (body, expected) in [
        (
            "Text(content: app.title, mark: brand)",
            "is not a color role",
        ),
        ("Canvas { Pattern(name: plaid) }", "is not a pattern"),
        (
            "Canvas { Series(values: app.count) }",
            "is a list of numbers",
        ),
        ("Shape(kind: rect)", "Canvas"),
        ("Canvas { Text(content: app.title) }", "cannot contain"),
    ] {
        let message = match compile_ui(&with_style(body)) {
            Ok(_) => panic!("expected `{body}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        assert!(message.contains(expected), "{body}: {message}");
    }
}

#[test]
fn style_rules_are_lowered_and_checked() {
    let source = |body: &str| {
        let item: syn::ItemMacro = syn::parse_str(&format!("style! {{ {body} }}")).unwrap();
        bern_syntax::parse_style(item.mac.tokens).unwrap()
    };
    let lower = |body: &str| {
        let mut diagnostics = Vec::new();
        let sheet = style::lower(&[("ui.rs".into(), source(body))], &mut diagnostics);
        (sheet, diagnostics)
    };
    let (sheet, diagnostics) = lower(
        "motion { jelly: spring(period: 0.5, damping: 0.4) }
         SidebarItem { corner: 8, hover: background(fill.subtle) + scale(1.03, snappy), selected: indicator(accent, edge: leading, width: 4), settle: jelly }
         #[target(macos)] Button.hero { background: color(light: \"#FF2E55\", dark: \"#FF5577\"), size: large }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(sheet.token("jelly").unwrap().period, 0.5);
    assert_eq!(sheet.rules.len(), 2);
    let item = &sheet.rules[0];
    assert_eq!(item.states.len(), 2);
    assert_eq!(item.states[0].effects.len(), 2);
    assert_eq!(item.settle.as_deref(), Some("jelly"));
    assert_eq!(sheet.rules[1].targets, ["macos"]);
    assert_eq!(
        style::variants(&sheet),
        [("Button".to_string(), "hero".to_string())]
    );

    for (body, expected) in [
        ("Slider { corner: 4 }", "does not take style overrides"),
        ("Button { background: brand }", "is not a color role"),
        (
            "Button { hover: scale(1.1, wobble) }",
            "is not a motion token",
        ),
        (
            "Button { selected: fill(accent) }",
            "has no `selected` state",
        ),
        (
            "SidebarItem { hover: fill(accent) }",
            "does not apply in the `hover` state",
        ),
        (
            "Button { background: color(light: \"#FFFFFF\") }",
            "gives both modes",
        ),
        ("#[target(amiga)] Button { corner: 4 }", "is not a platform"),
        ("Button { width: 4 }", "is not a style property"),
        (
            "pattern { plain: check(gap(4)) }",
            "needs a colored band wider than zero",
        ),
        (
            "pattern { plain: stripes(band(accent, 2)) }",
            "a pattern is `check(",
        ),
        (
            "pattern { plain: check(band(brand, 2)) }",
            "is not a color role",
        ),
    ] {
        let (_, diagnostics) = lower(body);
        assert!(
            diagnostics.iter().any(|d| d.message.contains(expected)),
            "{body}: {diagnostics:?}"
        );
    }
}
