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
        #[content]
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
    pub file: MediaSource,
    pub size: f64,
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
fn tab_accessory_stands_directly_in_its_tabs() {
    let tabs = |accessory: &str| {
        format!(
            r#"Tabs(selection: draft.mode) {{
                Tab(value: Mode::Fast, label: tr! {{ "en" => "Fast", "ja" => "速い" }}) {{ Spacer }}
                {accessory}
            }}"#
        )
    };
    compile_ui(&wrap(&tabs(
        "TabAccessory(shown: app.count > 0) { Spacer }",
    )))
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
    assert!(
        errors(&tabs("if app.count > 0 { TabAccessory { Spacer } }"))
            .contains("directly in its `Tabs`")
    );
    assert!(
        errors(&tabs("TabAccessory { Spacer } TabAccessory { Spacer }"))
            .contains("keeps one `TabAccessory`")
    );
}

#[test]
fn an_icon_only_tab_has_an_icon() {
    let tab = |props: &str| {
        format!(
            r#"Tabs(selection: draft.mode) {{
                Tab(value: Mode::Fast, label: tr! {{ "en" => "Fast", "ja" => "速い" }}{props}) {{ Spacer }}
            }}"#
        )
    };
    let compiled =
        compile_ui(&wrap(&tab(r#", icon: "hare", display: icon"#))).unwrap_or_else(|errors| {
            panic!(
                "{}",
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    let mut displays = Vec::new();
    compiled.app.walk(&mut |node, _| {
        if let ir::Node::Element(element) = node
            && element.kind == "Tab"
        {
            displays.push(element.keyword("display"));
        }
    });
    assert_eq!(displays, [Some("icon")]);
    assert!(errors(&tab(", display: icon")).contains("needs an `icon`"));
    assert!(
        errors(&tab(r#", icon: "hare", display: title"#))
            .contains("expected one of: title_and_icon, icon")
    );
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
        // Swift's `default` and Kotlin's `else` come last.
        (
            r#"match draft.mode { _ => { Spacer } Mode::Fast => { Spacer } }"#,
            "`_` must be the last arm of a `match`",
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

const CONTENT_PROTOCOL: &str = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Notice {
        #[label(tr! { "en" => "Saved {title}", "ja" => "{title} を保存しました" })]
        Saved {
            #[content]
            title: String,
        },
        #[label(tr! { "en" => "{n, plural, one {# file} other {# files}} due {day, date, MMM d}", "ja" => "{n} 件、{day, date, M月d日}まで" })]
        Due { n: i64, day: CalendarDate },
    }

    #[state]
    pub struct State {
        pub count: i64,
        #[content]
        pub title: String,
        pub status: String,
        pub due: CalendarDate,
        pub notice: Notice,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}
"#;

fn content_ui(body: &str) -> String {
    format!(
        r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {{
    pub name: String,
}}

#[component]
pub fn app(app: demo::Client) -> impl View {{
    ui! {{
        WindowSet(handle: AppWindows, primary: main) {{
            Window(id: main, title: tr! {{ "en" => "Main", "ja" => "メイン" }}, initial: open) {{
                page(app: app)
            }}
        }}
    }}
}}

#[component]
pub fn heading(#[content] title: String, note: String) -> impl View {{
    ui! {{ Text(content: title) }}
}}

#[component]
pub fn page(app: demo::Client, #[session] draft: Draft) -> impl View {{
    ui! {{
        {body}
    }}
}}
"#
    )
}

#[test]
fn shown_text_is_declared_content_and_arguments_fit_their_format() {
    let valid = r#"Column {
        Text(content: app.title)
        Text(content: draft.name)
        Text(content: app.notice)
        heading(title: app.title, note: app.status)
        Text(content: tr! { "en" => "{n, plural, =0 {no files} one {# file} other {# files}}", "ja" => "{n} 件"; n: app.count })
        Text(content: tr! { "en" => "Due {day, date, EEE, MMM d}", "ja" => "{day, date, M月d日}まで"; day: app.due })
        Text(content: tr! { "en" => "{t, duration, m:ss} left", "ja" => "残り {t, duration, m:ss}"; t: app.count })
        Text(content: tr! { "en" => "Saved {title}", "ja" => "{title} を保存"; title: app.title })
    }"#;
    compile_project(CONTENT_PROTOCOL, &content_ui(valid)).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let cases = [
        (
            r#"Text(content: app.status)"#,
            "error[B105]: `app.status` is shown as it is",
        ),
        (
            r#"Text(content: tr! { "en" => "Now {s}", "ja" => "今 {s}"; s: app.status })"#,
            "error[B105]: `app.status`",
        ),
        (
            r#"Text(content: tr! { "en" => "Now {s}", "ja" => "今 {s}"; s: "busy" })"#,
            "error[B105]: fixed text belongs in the translations",
        ),
        (
            r#"heading(title: app.status, note: app.status)"#,
            "error[B105]: `title` of `heading`",
        ),
        (
            r#"Text(content: tr! { "en" => "{n, plural, other {# files}}", "ja" => "{n} 件"; n: app.count })"#,
            "error[B104]: the `en` text needs the plural branch `one` of `n`",
        ),
        (
            r#"Text(content: tr! { "en" => "{n, plural, one {# x} other {# xs}}", "ja" => "{n}"; n: app.title })"#,
            "error[B104]: `{n, plural, …}` needs a number, found text",
        ),
        (
            r#"Text(content: tr! { "en" => "{d, date, MMM d}", "ja" => "{d, date, M月d日}"; d: app.count })"#,
            "error[B104]: `{d, date, …}` needs a `CalendarDate`, found int",
        ),
        (
            r#"Text(content: tr! { "en" => "{d}", "ja" => "{d}"; d: app.due })"#,
            "error[B104]: `{d}` is a date",
        ),
        (
            r#"Text(content: app.due)"#,
            "a date is written in the reader's language with `tr!`",
        ),
    ];
    for (body, expected) in cases {
        let output = match compile_project(CONTENT_PROTOCOL, &content_ui(body)) {
            Ok(_) => panic!("expected `{body}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
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
                features: &[],
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
    fn targets_without_idle_control_warn_that_idleness_stays_on() {
        let compiled = compile_project(
            MEDIA_PROTOCOL,
            &media_project(
                r#", idle_after: 3, idle_enabled: draft.name == """#,
                "",
                "Spacer",
            ),
        )
        .unwrap_or_else(|errors| panic!("{errors:?}"));
        let messages: Vec<String> = Outline
            .check(
                &compiled.app,
                &TargetConfig {
                    name: "outline".into(),
                    platform: "test".into(),
                    ui: None,
                    web_mode: None,
                    channels: vec![],
                    frontends: vec![],
                },
            )
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(
            messages
                .iter()
                .any(|m| m.contains("ignores `idle_enabled` of window `main`")),
            "{messages:?}"
        );
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

    #[test]
    fn targets_name_the_features_they_lack() {
        let app = with_engine_pictures(
            r#"Column {
                Canvas(height: 80, label: tr! { "en" => "Header", "ja" => "ヘッダー" }) {
                    Shape(kind: rect, fill_from: app.poster, fade: down)
                }
                Image(source: app.cover, width: 100)
                Button(id: open, label: tr! { "en" => "Open", "ja" => "開く" }, pick: draft.file) => app.add(1)
                ScrollView(snap: start) {
                    Text(content: app.title, scroll_scale: 1.1)
                }
                Column(stagger: 0.03) { Spacer(appear: fade) }
                Rail { Spacer(at: 0.5) }
                PopoverButton(id: more, label: tr! { "en" => "More", "ja" => "その他" }) { Spacer }
            }"#,
        )
        .unwrap_or_else(|errors| panic!("{errors:?}"))
        .app;
        let problems = check_common(&Outline, &app);
        for expected in [
            "target `outline-test` cannot show pictures from a `MediaSource`",
            "target `outline-test` cannot fill a shape from a picture or fade it",
            "target `outline-test` cannot ask for a file with `pick:`",
            "target `outline-test` cannot snap a scroll view to its items",
            "target `outline-test` does not follow scrolling with `scroll_scale`: items keep their look",
            "target `outline-test` does not stagger: children appear together",
            "target `outline-test` cannot place elements on a `Rail`",
            "target `outline-test` cannot build a `PopoverButton`",
        ] {
            assert!(
                problems.iter().any(|d| d.message.contains(expected)),
                "{expected}: {problems:?}"
            );
        }
    }

    #[test]
    fn elements_placed_directly_in_a_window_are_checked() {
        let hi = r#"Text(content: tr! { "en" => "Hi", "ja" => "やあ" })"#;
        let source = wrap(hi).replace(
            "page(app: app, windows: windows)",
            &format!(
                r#"Column(stagger: 0.03) {{
                    page(app: app, windows: windows)
                    Rail {{ Spacer(at: 0.5) }}
                    PopoverButton(id: more, label: tr! {{ "en" => "More", "ja" => "その他" }}) {{ {hi} }}
                    Button(id: grow, label: tr! {{ "en" => "Grow", "ja" => "拡大" }}) {{ {hi} }} => app.add(1)
                }}"#
            ),
        );
        let app = compile_ui(&source)
            .unwrap_or_else(|errors| panic!("{errors:?}"))
            .app;
        let problems = check_common(&Outline, &app);
        for expected in [
            "target `outline-test` does not support `Column`",
            "target `outline-test` does not support `Rail`",
            "target `outline-test` cannot place elements on a `Rail`",
            "target `outline-test` cannot build a `PopoverButton`",
            "target `outline-test` does not stagger: children appear together",
            "target `outline-test` cannot build a `Button` that shows content instead of its label",
        ] {
            assert!(
                problems.iter().any(|d| d.message.contains(expected)),
                "{expected}: {problems:?}"
            );
        }
        // Each points at the element in the window.
        let column = problems
            .iter()
            .find(|d| d.message.contains("does not support `Column`"))
            .unwrap();
        assert!(column.location.is_some(), "{column:?}");
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
        #[content]
        pub title: String,
    }

    #[state]
    pub struct State {
        pub count: i64,
        #[content]
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
    fn optional_sizes_need_their_breakpoints_and_borrow_layouts() {
        let medium = format!(
            "Adaptive(id: choice) {{ Layout(size: compact) {{ {ADD} }} Layout(size: medium) {{ {ADD} }} Layout(size: expanded) {{ {ADD} }} }}"
        );
        let output = failure("", &medium, LAYOUTS);
        assert!(
            output.contains("`Layout(size: medium)` needs `medium_below`"),
            "{output}"
        );
        let four = "\n[layouts]\ncompact_below = 600\nmedium_below = 1024\nexpanded_below = 1440\n";
        compile_project_with(PROTOCOL, &project("", &medium), four)
            .unwrap_or_else(|errors| panic!("{errors:?}"));
        // A size without a layout of its own shows the next larger one,
        // else the next smaller.
        use ir::LayoutSize::{Compact, Expanded, Large, Medium};
        assert_eq!(Medium.shown(&[Compact, Expanded]), Some(Expanded));
        assert_eq!(Large.shown(&[Compact, Expanded]), Some(Expanded));
        assert_eq!(Medium.shown(&[Compact, Medium, Expanded]), Some(Medium));
        let config = ir::LayoutConfig {
            compact_below: 600.0,
            medium_below: Some(1024.0),
            expanded_below: Some(1440.0),
        };
        assert_eq!(
            [599.0, 600.0, 1023.0, 1024.0, 1439.0, 1440.0].map(|width| config.size(width)),
            [Compact, Medium, Medium, Expanded, Expanded, Large]
        );
        // The window's size reads the same breakpoints.
        let padding = format!(
            "Column(padding: if window.large {{ 48 }} else if window.medium {{ 24 }} else {{ 16 }}) {{ {ADD} }}"
        );
        compile_project_with(PROTOCOL, &project("", &padding), four)
            .unwrap_or_else(|errors| panic!("{errors:?}"));
        let output = failure("", &padding, "");
        assert!(output.contains("error[B402]: `window.large`"), "{output}");
        let output = failure("", &padding, LAYOUTS);
        assert!(output.contains("needs `expanded_below`"), "{output}");
        let disordered = "\n[layouts]\ncompact_below = 600\nmedium_below = 500\n";
        let output = failure("", &padding, disordered);
        assert!(
            output.contains("[layouts].medium_below must be wider than compact_below"),
            "{output}"
        );
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

    const RESET: &str =
        r#"Button(id: reset, label: tr! { "en" => "Reset", "ja" => "リセット" }) => app.reset()"#;

    /// A second contract, `OtherFeature`, for `child` to be scoped by.
    const OTHER: &str = r#"
#[bern::features]
pub enum OtherFeature {
    #[commands(reset)]
    Reset,
}
"#;

    /// The contract project with `extra` declarations and one more
    /// component, `child`, declared with `attribute` and placing `body`.
    fn with_child(
        page: &str,
        attribute: &str,
        body: &str,
        extra: &str,
    ) -> Result<Compiled, Vec<Diagnostic>> {
        let source = format!(
            "{}{extra}\n{attribute}\npub fn child(app: demo::Client) -> impl View {{\n    ui! {{ {body} }}\n}}\n",
            project("", page)
        );
        compile_project_with(PROTOCOL, &source, LAYOUTS)
    }

    /// The diagnostics of a project that must fail, as text.
    fn text(result: Result<Compiled, Vec<Diagnostic>>) -> String {
        match result {
            Ok(_) => panic!("expected the project to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// The entries that reach a feature in one size.
    fn entries(coverage: &ir::Coverage) -> &[String] {
        match coverage {
            ir::Coverage::Covered { entries } | ir::Coverage::Conditional { entries } => entries,
            ir::Coverage::Missing => &[],
        }
    }

    #[test]
    fn a_feature_lowers_its_commands_sizes_and_reasons() {
        let compiled = compiled(
            r#"#[commands(reset, media.play)]
    #[guaranteed("shown whenever the page is")]
    #[exempt(linux, "no media keys")]"#,
            ADD,
        );
        let contract = &compiled.app.contracts[0];
        assert_eq!(contract.name, "PageFeature");
        let feature = &contract.features[0];
        assert_eq!(feature.name, "Add");
        assert_eq!(feature.doc, "Add one.");
        // Protocol commands by catalog id, media commands by their own.
        assert_eq!(feature.commands, ["demo.add", "demo.reset", "media.play"]);
        // Without `#[layouts]`, a feature is due in every required size.
        assert_eq!(
            feature.layouts,
            [ir::LayoutSize::Compact, ir::LayoutSize::Expanded]
        );
        assert_eq!(
            feature.guaranteed.as_deref(),
            Some("shown whenever the page is")
        );
        assert_eq!(feature.exemptions.len(), 1);
        assert_eq!(feature.exemptions[0].platform, "linux");
        assert_eq!(feature.exemptions[0].reason, "no media keys");
    }

    #[test]
    fn unknown_feature_commands_and_sizes_are_rejected() {
        for (features, expected) in [
            (
                "#[commands(launch)]",
                "`launch` is neither a command of the protocol nor a standard media command (`media.play`, …)",
            ),
            ("#[commands(media.fly)]", "`media.fly` is neither a command"),
            (
                "#[layouts(huge)]",
                "unknown layout size `huge`; sizes are compact and expanded",
            ),
            // Contracts cover the required sizes only.
            ("#[layouts(medium)]", "sizes are compact and expanded"),
        ] {
            let output = failure(features, ADD, LAYOUTS);
            assert!(output.contains(expected), "{features}: {output}");
            assert!(output.contains("--> demo-ui/src/lib.rs:"), "{output}");
        }
    }

    #[test]
    fn a_feature_for_one_size_is_proven_in_that_size_only() {
        let compiled = compiled("#[layouts(expanded)]", &layouts("Spacer", ADD));
        let feature = &compiled.app.contracts[0].features[0];
        assert_eq!(feature.layouts, [ir::LayoutSize::Expanded]);
        assert_eq!(feature.coverage.len(), 1);
        assert_eq!(feature.coverage[0].size, ir::LayoutSize::Expanded);
        assert!(matches!(
            feature.coverage[0].coverage,
            ir::Coverage::Covered { .. }
        ));
        assert!(report(&compiled, "macos").is_empty());
    }

    #[test]
    fn a_feature_in_some_branches_only_stays_conditional() {
        let compiled = compiled("", &format!("if draft.on {{ {ADD} }} else {{ Spacer }}"));
        for size in &compiled.app.contracts[0].features[0].coverage {
            assert_eq!(
                size.coverage,
                ir::Coverage::Conditional {
                    entries: vec!["page.add".into()]
                },
                "{size:?}"
            );
        }
        assert!(report(&compiled, "macos").contains("error[B203]"));
    }

    #[test]
    fn components_a_scope_places_belong_to_it() {
        let compiled = with_child("child(app: app)", "#[component]", ADD, "")
            .unwrap_or_else(|errors| panic!("{errors:?}"));
        for size in &compiled.app.contracts[0].features[0].coverage {
            assert_eq!(
                size.coverage,
                ir::Coverage::Covered {
                    entries: vec!["child.add".into()]
                },
                "{size:?}"
            );
        }
        let output = text(with_child(
            "child(app: app)",
            "#[component]",
            &format!("Column {{ {ADD} {RESET} }}"),
            "",
        ));
        assert!(
            output.contains(
                "error[B202]: `child.reset` runs `demo.reset` inside the scope of contract `PageFeature`"
            ),
            "{output}"
        );
    }

    #[test]
    fn a_component_scoped_by_another_contract_answers_to_that_one() {
        let reset = RESET.replace(") =>", ", feature: OtherFeature::Reset) =>");
        let compiled = with_child(
            &format!("Column {{ {ADD} child(app: app) }}"),
            "#[component(features = OtherFeature)]",
            &reset,
            OTHER,
        )
        .unwrap_or_else(|errors| panic!("{errors:?}"));
        let coverage = |contract: &str| {
            let contract = compiled
                .app
                .contracts
                .iter()
                .find(|candidate| candidate.name == contract)
                .unwrap();
            assert_eq!(contract.features[0].coverage.len(), 2);
            entries(&contract.features[0].coverage[0].coverage).to_vec()
        };
        // The child's entries prove its own contract, not the page's.
        assert_eq!(coverage("PageFeature"), ["page.add"]);
        assert_eq!(coverage("OtherFeature"), ["child.reset"]);

        // A feature of the child's contract on the page is B204.
        let page = format!(
            "Column {{ {ADD} {} child(app: app) }}",
            reset.replace("id: reset", "id: again")
        );
        let output = text(with_child(
            &page,
            "#[component(features = OtherFeature)]",
            &reset,
            OTHER,
        ));
        assert!(
            output.contains(
                "error[B204]: `OtherFeature::Reset` belongs to contract `OtherFeature`, but `page.again` is in the scope of `PageFeature`"
            ),
            "{output}"
        );
    }

    #[test]
    fn an_entry_shared_by_two_scopes_is_reported_once() {
        let panel = r#"
#[component(features = OtherFeature)]
pub fn panel(app: demo::Client) -> impl View {
    ui! { child(app: app) }
}
"#;
        let output = text(with_child(
            &format!("Column {{ {ADD} child(app: app) }}"),
            "#[component]",
            RESET,
            &format!("{OTHER}{panel}"),
        ));
        assert_eq!(
            output.matches("`child.reset` runs `demo.reset`").count(),
            1,
            "{output}"
        );
    }

    #[test]
    fn a_scope_that_places_itself_is_reported_not_searched_forever() {
        // The page places itself, and so does a component it places.
        for (page, child) in [
            (format!("Column {{ {ADD} page(app: app) }}"), "Spacer"),
            (
                "child(app: app)".to_string(),
                &*format!("Column {{ {ADD} child(app: app) }}"),
            ),
        ] {
            let output = text(with_child(&page, "#[component]", child, ""));
            assert!(
                output.contains("components reference each other recursively"),
                "{output}"
            );
        }
    }

    #[test]
    fn a_feature_entry_runs_a_business_command_b204() {
        let flip = r#"Button(id: flip, label: tr! { "en" => "Flip", "ja" => "反転" }, feature: PageFeature::Add) => draft.on.toggle()"#;
        let output = failure("", &format!("Column {{ {ADD} {flip} }}"), LAYOUTS);
        assert!(
            output.contains(
                "error[B204]: `page.flip` names `PageFeature::Add` but runs no business command"
            ),
            "{output}"
        );
    }

    #[test]
    fn features_in_a_window_are_outside_every_scope_b204() {
        let source = project("", ADD).replace(
            "other(app: app)",
            &format!("other(app: app)\n{}", ADD.replace("id: add", "id: stray")),
        );
        let output = text(compile_project_with(PROTOCOL, &source, LAYOUTS));
        assert!(
            output.contains(
                "error[B204]: `PageFeature::Add` is used outside the scope of contract `PageFeature`"
            ),
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
fn animated_svg_resources_and_controls_are_checked() {
    let compile_svg = |body: &str| {
        let dir = write_project(
            PROTOCOL,
            &wrap(body),
            "\n[resources]\npaths = [\"motion.svg\"]\n",
        );
        std::fs::write(
            dir.join("motion.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#,
        )
        .unwrap();
        let result = config::load(&dir.join("Bern.toml")).and_then(|c| compile(&c));
        let _ = std::fs::remove_dir_all(dir);
        result
    };
    assert!(compile_svg(r#"Svg(source: "motion.svg", width: 104, height: 104, playing: true, speed: 2, restart: app.count, interactive: false)"#).is_ok());
    for (body, expected) in [
        (
            r#"Svg(source: "motion.png", width: 104, height: 104)"#,
            "Svg requires a .svg resource",
        ),
        (
            r#"Svg(source: "missing.svg", width: 104, height: 104)"#,
            "is not declared",
        ),
        (
            r#"Svg(source: "motion.svg", width: 0, height: 104)"#,
            "must be above zero",
        ),
        (
            r#"Svg(source: "motion.svg", width: 104, height: 104, playing: 3)"#,
            "bool",
        ),
        (
            r#"Svg(source: app.title, width: 104, height: 104)"#,
            "declared .svg resource",
        ),
    ] {
        let errors = compile_svg(body).err().unwrap();
        assert!(
            errors.iter().any(|e| e.message.contains(expected)),
            "{errors:?}"
        );
    }
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
            "`source` needs a MediaSource or Option<MediaSource>, found text",
        ),
        (
            r#"Image(source: 3, width: 10)"#,
            "expected the name of a declared resource",
        ),
        (
            r#"Image(source: "pics/a.png")"#,
            "`Image` needs `width:`, `height:` or `aspect:`",
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
fn pictures_placed_directly_in_a_window_are_declared() {
    let source = wrap("Spacer").replace(
        "page(app: app, windows: windows)",
        r#"Column {
                    page(app: app, windows: windows)
                    Image(source: "missing.png", width: 10)
                }"#,
    );
    let Err(errors) = compile_ui(&source) else {
        panic!("an undeclared picture in a window compiled");
    };
    let error = errors
        .iter()
        .find(|e| {
            e.message
                == "resource `missing.png` is not declared; list it under [resources] in Bern.toml"
        })
        .unwrap_or_else(|| panic!("{errors:?}"));
    assert!(error.location.is_some(), "{error:?}");
}

#[test]
fn sessions_read_directly_in_a_window_belong_to_it() {
    let source = wrap("Spacer")
        .replace(
            "pub fn app(app: demo::Client, windows: AppWindows)",
            "pub fn app(app: demo::Client, windows: AppWindows, #[session] draft: Draft)",
        )
        .replacen(
            "about_page()",
            r#"Column {
                    about_page()
                    if draft.on {
                        Switch(id: on, label: tr! { "en" => "On", "ja" => "オン" }, value: draft.on)
                    }
                }"#,
            1,
        );
    let app = compile_ui(&source)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
        .app;
    assert_eq!(app.window("about").unwrap().sessions, ["Draft"]);
    assert_eq!(app.window("main").unwrap().sessions, ["Draft"]);
}

#[test]
fn window_expressions_read_only_sessions_the_window_owns() {
    for props in [
        ", immersive: draft.on",
        ", idle_after: 3, idle_enabled: draft.on",
        ", landscape: draft.on",
    ] {
        let source = wrap("Spacer")
            .replace(
                "pub fn app(app: demo::Client, windows: AppWindows)",
                "pub fn app(app: demo::Client, windows: AppWindows, #[session] draft: Draft)",
            )
            .replace(r#""情報" })"#, &format!(r#""情報" }}{props})"#));
        assert!(source.contains(props));
        let Err(errors) = compile_ui(&source) else {
            panic!("`{props}` read a session the window does not own");
        };
        let error = errors
            .iter()
            .find(|e| e.message.contains("reads `Draft`, which this window does not own"))
            .unwrap_or_else(|| panic!("{errors:?}"));
        assert!(error.message.starts_with("window `about`"), "{error:?}");
        assert!(error.location.is_some(), "{error:?}");
    }
}

#[test]
fn windows_cannot_place_the_root() {
    let source = wrap("Spacer").replacen("about_page()", "app(app: app, windows: windows)", 1);
    let Err(errors) = compile_ui(&source) else {
        panic!("a window that places the root compiled");
    };
    assert!(
        errors
            .iter()
            .any(|e| e.message == "the root component cannot be placed by a component or a window"),
        "{errors:?}"
    );
}

/// Compile `body` with pictures the engine gives and the resource
/// `pics/a.png`.
fn with_engine_pictures(body: &str) -> Result<Compiled, Vec<Diagnostic>> {
    let protocol = PROTOCOL.replace(
        "pub mode: Mode,",
        "pub mode: Mode,\n        pub poster: Option<MediaSource>,\n        pub cover: MediaSource,",
    );
    let dir = write_project(
        &protocol,
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
fn pictures_come_from_the_engine_and_color_shapes() {
    let compiled = with_engine_pictures(
        r#"Column {
            Image(source: app.poster, height: 150, label: tr! { "en" => "Poster", "ja" => "ポスター" })
            Image(source: app.cover, width: 100, fit: cover)
            Canvas(height: 180, label: tr! { "en" => "Header", "ja" => "ヘッダー" }) {
                Shape(kind: rect, fill_from: app.poster, fade: down)
                Shape(kind: rect, fill_from: "pics/a.png", opacity: 0.5)
            }
        }"#,
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let page = compiled.app.component("page").unwrap();
    let mut seen = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node {
            match element.kind.as_str() {
                "Image" => seen.push(element.expr("source").unwrap().ty.display()),
                "Shape" => seen.push(format!(
                    "{:?} {:?} {:?}",
                    element.expr("fill_from").map(|expr| expr.ty.display()),
                    element.resource("fill_from"),
                    element.keyword("fade")
                )),
                _ => {}
            }
        }
    });
    assert_eq!(
        seen,
        [
            "MediaSource?",
            "MediaSource",
            r#"Some("MediaSource?") None Some("down")"#,
            r#"None Some("pics/a.png") None"#,
        ]
    );

    for (body, expected) in [
        (
            "Image(source: app.count, width: 10)",
            "`source` needs a MediaSource or Option<MediaSource>, found int",
        ),
        (
            "Canvas { Shape(kind: rect, fill_from: app.title) }",
            "found text",
        ),
        (
            "Canvas { Shape(kind: rect, fill_from: \"pics/b.png\") }",
            "resource `pics/b.png` is not declared",
        ),
        (
            "Canvas { Shape(kind: rect, fill_from: app.poster, fade: sideways) }",
            "expected one of: up, down, left, right",
        ),
    ] {
        let message = match with_engine_pictures(body) {
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

    // `pages.notes.show()` in the module sets the shell's page selection,
    // and the summary reads the session that holds it.
    let summary = compiled
        .app
        .components
        .iter()
        .find(|component| component.name == "notes_summary")
        .unwrap();
    let mut shows = None;
    summary.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node
            && let Some(ir::Action::Session {
                field,
                op: ir::SessionOp::Set { value },
            }) = &element.action
        {
            shows = Some((field.ty.clone(), value.kind.clone()));
        }
    });
    let (ty, value) = shows.expect("the link sets a session field");
    assert_eq!(
        ty,
        Type::Enum {
            name: "MainPage".into()
        }
    );
    assert!(
        matches!(value, ir::ExprKind::Variant { variant: 2, .. }),
        "{value:?}"
    );
    assert!(
        summary
            .params
            .iter()
            .any(|param| matches!(&param.kind, ir::ParamKind::Session { .. }))
    );
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
fn sidebar_footers_and_toolbar_searches_are_checked() {
    let window = |footer: &str, toolbar: &str| {
        wrap(&format!(
            r#"SplitView {{
                Sidebar(selection: draft.mode) {{
                    SidebarItem(value: Mode::Fast, label: tr! {{ "en" => "Fast", "ja" => "速い" }})
                    {footer}
                }}
                Detail {{
                    Toolbar {{ {toolbar} }}
                    Spacer
                }}
            }}"#
        ))
    };
    let footer = "SidebarFooter { Text(content: app.title) }";
    let search = "Search(id: find, value: draft.name) { Text(content: app.title) }";
    compile_ui(&window(footer, search)).unwrap_or_else(|errors| panic!("{errors:?}"));
    for (footer, toolbar, expected) in [
        (
            format!("{footer} {footer}"),
            String::new(),
            "at most one `SidebarFooter`",
        ),
        (
            String::new(),
            format!("{search} {}", search.replace("find", "again")),
            "at most one `Search`",
        ),
        (
            String::new(),
            format!("if draft.on {{ {search} }}"),
            "stands directly in its `Toolbar`",
        ),
    ] {
        let message = match compile_ui(&window(&footer, &toolbar)) {
            Ok(_) => panic!("expected `{footer}` / `{toolbar}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        assert!(message.contains(expected), "{footer} {toolbar}: {message}");
    }
    let outside = errors(footer);
    assert!(outside.contains("SidebarFooter"), "{outside}");
}

#[test]
fn session_actions_panels_and_notices_are_checked() {
    let compiled = compile_ui(&wrap(
        r#"Column {
            Button(id: open, label: tr! { "en" => "Open", "ja" => "開く" }) => draft.on.set(true)
            Button(id: flip, label: tr! { "en" => "Flip", "ja" => "反転" }) => draft.on.toggle()
            Button(id: fast, label: tr! { "en" => "Fast", "ja" => "速い" }) => draft.mode.set(Mode::Fast)
            Button(id: large, label: tr! { "en" => "Large", "ja" => "大" }) => draft.size.set(18)
            Button(id: key, label: tr! { "en" => "A", "ja" => "A" }) => draft.name.push_str("A")
            Button(id: back, label: tr! { "en" => "Delete", "ja" => "削除" }) => draft.name.pop()
            Switch(id: panel, label: tr! { "en" => "Panel", "ja" => "パネル" }, value: draft.on, style: button, icon: "sidebar.right")
            Toast(content: app.title, key: app.count, duration: 2)
            Inspector(presented: draft.on, width: 300) {
                Text(content: app.title)
            }
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let mut ops = Vec::new();
    for component in &compiled.app.components {
        component.body.walk(&mut |node| {
            if let ir::Node::Element(element) = node
                && let Some(ir::Action::Session { op, .. }) = &element.action
            {
                ops.push(match op {
                    ir::SessionOp::Toggle => "toggle",
                    ir::SessionOp::PushStr { .. } => "push_str",
                    ir::SessionOp::Pop => "pop",
                    ir::SessionOp::Set { .. } => "set",
                });
                // A whole number set on a decimal field is that decimal.
                if let ir::SessionOp::Set { value } = op
                    && value.ty == Type::Float
                {
                    assert_eq!(
                        value.kind,
                        ir::ExprKind::Literal {
                            value: Value::Float(18.0)
                        }
                    );
                }
            }
        });
    }
    assert_eq!(ops, ["set", "toggle", "set", "set", "push_str", "pop"]);
    compile_ui(&wrap(
        r#"Toolbar {
            SplitButton(id: danmaku, label: tr! { "en" => "Comments", "ja" => "コメント" }, icon: "text.bubble", value: draft.on) {
                Switch(id: merge, label: tr! { "en" => "Merge", "ja" => "まとめる" }, value: draft.on)
            }
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));

    let label = r#"label: tr! { "en" => "Go", "ja" => "行く" }"#;
    for (body, expected) in [
        (
            format!("Button(id: go, {label}) => draft.name.toggle()"),
            "flips a `bool`",
        ),
        (
            format!("Button(id: go, {label}) => draft.on.set(\"yes\")"),
            "the new value",
        ),
        (
            format!("Button(id: go, {label}) => draft.on.clear()"),
            "support `set(value)`",
        ),
        (
            format!("Button(id: go, {label}) => draft.on.push_str(\"x\")"),
            "edit a `String`",
        ),
        (
            format!("Button(id: go, {label}) => draft.name.push_str(3)"),
            "the text to add",
        ),
        (
            "Column { if draft.on { Inspector(presented: draft.on) { Spacer } } }".to_string(),
            "stands directly in the container",
        ),
        (
            "Column { Inspector(presented: draft.on) { Spacer } Inspector(presented: draft.on) { Spacer } }"
                .to_string(),
            "at most one `Inspector`",
        ),
    ] {
        let message = errors(&body);
        assert!(message.contains(expected), "{body}: {message}");
    }
}

#[test]
fn files_are_picked_and_dropped() {
    let compiled = compile_ui(&wrap(
        r#"Column {
            Toolbar {
                IconButton(id: open, icon: "folder", label: tr! { "en" => "Open", "ja" => "開く" }, pick: draft.file, files: "Video, .SRT", shortcut: "cmd+o") => app.add(1)
                Divider
                Menu(id: more, label: tr! { "en" => "More", "ja" => "その他" }) {
                    MenuItem(id: folder, label: tr! { "en" => "Folder", "ja" => "フォルダ" }, pick: draft.file, files: "folder") => app.add(2)
                }
            }
            Drop(id: drop, label: tr! { "en" => "Drop", "ja" => "ドロップ" }, value: draft.file, files: "video", over: draft.on, opens: true) {
                Text(content: app.title, nudge: draft.on)
            } => app.add(3)
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let page = compiled.app.component("page").unwrap();
    let mut seen = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node
            && let Some(files) = element.keyword("files")
        {
            let field = element.field("pick").or(element.field("value")).unwrap();
            seen.push(format!("{} {files} {}", element.kind, field.ty.display()));
        }
    });
    assert_eq!(
        seen,
        [
            "IconButton video .srt MediaSource",
            "MenuItem folder MediaSource",
            "Drop video MediaSource",
        ]
    );
    let mut flags = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node {
            if let Some(opens) = element.keyword("opens") {
                flags.push(format!("opens {opens}"));
            }
            if let Some(nudge) = element.expr("nudge") {
                flags.push(format!("nudge {}", nudge.ty.display()));
            }
        }
    });
    assert_eq!(flags, ["opens true", "nudge bool"]);

    let open =
        r#"IconButton(id: open, icon: "folder", label: tr! { "en" => "Open", "ja" => "開く" }"#;
    for (body, expected) in [
        (format!(r#"{open}, pick: draft.file, files: "movies") => app.add(1)"#), "neither a kind"),
        (format!(r#"{open}, pick: draft.file, files: "any video") => app.add(1)"#), "`any` already takes every file"),
        (format!(r#"{open}, pick: draft.file, files: "video .mp4 video") => app.add(1)"#), "written twice"),
        (format!(r#"{open}, files: "video") => app.add(1)"#), "add `pick: draft.file`"),
        (format!(r#"{open}, pick: draft.name) => app.add(1)"#), "a `MediaSource` or `Option<MediaSource>` field"),
        (
            r#"Drop(id: d, label: tr! { "en" => "D", "ja" => "D" }, value: draft.file) { Spacer }"#.to_string(),
            "UI action must bind",
        ),
        (
            r#"Drop(id: d, label: tr! { "en" => "D", "ja" => "D" }, value: draft.file) { Spacer Spacer } => app.add(1)"#.to_string(),
            "takes exactly one child",
        ),
        (format!(r#"Toolbar {{ Divider {open}) => app.add(1) }}"#), "put it between items"),
        (format!(r#"Toolbar {{ {open}) => app.add(1) Divider Divider {open}) => app.add(1) }}"#), "put it between items"),
        (
            format!(r#"Toolbar {{ {open}) => app.add(1) if draft.on {{ Divider }} }}"#),
            "stands directly in it",
        ),
        (
            r#"Drop(id: d, label: tr! { "en" => "D", "ja" => "D" }, value: draft.file, opens: yes) { Spacer } => app.add(1)"#.to_string(),
            "expected `true` or `false`",
        ),
        ("Text(content: app.title, nudge: app.title)".to_string(), "bool"),
    ] {
        let message = errors(&body);
        assert!(message.contains(expected), "{body}: {message}");
    }
}

#[test]
fn scrolling_and_gestures_link_to_values() {
    let compiled = compile_ui(&wrap(
        r#"Column {
            ScrollView(axis: horizontal, snap: center, position: draft.name) {
                Row(gap: 30) {
                    Text(content: app.title, scroll_scale: 1.12, scroll_opacity: 0.6)
                }
            }
            Pinch(id: zoom, label: tr! { "en" => "Zoom", "ja" => "拡大" }, value: draft.size, min: 100, max: 320) {
                Spacer
            } => app.add(1)
            Swipe(id: level, label: tr! { "en" => "Level", "ja" => "レベル" }, value: draft.size, min: 0, max: 1, axis: vertical, distance: 200) {
                Spacer
            }
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let page = compiled.app.component("page").unwrap();
    let mut seen = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node {
            match element.kind.as_str() {
                "ScrollView" => seen.push(format!(
                    "{:?} {}",
                    element.keyword("snap"),
                    element.field("position").unwrap().ty.display()
                )),
                "Text" => seen.push(format!(
                    "{:?} {:?}",
                    element.number("scroll_scale"),
                    element.number("scroll_opacity")
                )),
                "Pinch" | "Swipe" => seen.push(format!(
                    "{} {:?} {:?} {}",
                    element.kind,
                    element.number("min"),
                    element.number("distance"),
                    element.action.is_some()
                )),
                _ => {}
            }
        }
    });
    assert_eq!(
        seen,
        [
            r#"Some("center") text"#,
            "Some(1.12) Some(0.6)",
            "Pinch Some(100.0) None true",
            "Swipe Some(0.0) Some(200.0) false",
        ]
    );

    let pinch = r#"Pinch(id: z, label: tr! { "en" => "Z", "ja" => "Z" }, value: draft.size"#;
    for (body, expected) in [
        ("Text(content: app.title, scroll_scale: 0)".to_string(), "`scroll_scale` must be above zero"),
        ("Text(content: app.title, scroll_opacity: 1.5)".to_string(), "an opacity from 0 to 1"),
        ("ScrollView(position: draft.on) { Spacer }".to_string(), "an `i64` or `String` field"),
        ("ScrollView(snap: middle) { Spacer }".to_string(), "expected one of: start, center"),
        ("ScrollView(arrival: nudge) { Spacer }".to_string(), "add `position: draft.x`"),
        (format!("{pinch}, min: 3, max: 3) {{ Spacer }}"), "`Pinch` needs `min` below `max`"),
        (format!("{pinch}, min: 1, max: 3)"), "takes exactly one child"),
        (format!("{pinch}, min: 1, max: 3, distance: 4) {{ Spacer }}"), "distance"),
        (
            r#"Swipe(id: s, label: tr! { "en" => "S", "ja" => "S" }, value: draft.size, min: 0, max: 1, distance: 0) { Spacer }"#.to_string(),
            "`distance` must be above zero",
        ),
    ] {
        let message = errors(&body);
        assert!(message.contains(expected), "{body}: {message}");
    }
}

#[test]
fn rails_stagger_and_popover_buttons() {
    let compiled = compile_ui(&wrap(
        r#"Column(gap: 8, stagger: 0.03) {
            Text(content: app.title, appear: fade)
            Rail(anchor: start) {
                Text(content: app.title, at: draft.size, animate: smooth)
                Canvas(from: 0, to: draft.size) { Shape(kind: rect, fill: fill.subtle) }
                if draft.on {
                    Spacer(at: 1)
                }
            }
            Grid(min_width: 120, stagger: 0.05) {
                Text(content: app.title, appear: fade)
            }
            PopoverButton(id: volume, label: tr! { "en" => "Volume", "ja" => "音量" }, icon: "speaker.wave.3.fill", presented: draft.on) {
                Slider(id: level, label: tr! { "en" => "Level", "ja" => "レベル" }, value: draft.size, min: 0, max: 1)
            }
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let page = compiled.app.component("page").unwrap();
    let mut seen = Vec::new();
    page.body.walk(&mut |node| {
        if let ir::Node::Element(element) = node {
            if let Some(step) = element.number("stagger") {
                seen.push(format!("{} staggers {step}", element.kind));
            }
            for name in ["at", "from", "to"] {
                if element.prop(name).is_some() {
                    seen.push(format!("{} {name}", element.kind));
                }
            }
            if element.kind == "PopoverButton" {
                seen.push(format!("popover {}", element.field("presented").is_some()));
            }
        }
    });
    assert_eq!(
        seen,
        [
            "Column staggers 0.03",
            "Text at",
            "Canvas from",
            "Canvas to",
            "Spacer at",
            "Grid staggers 0.05",
            "popover true",
        ]
    );
    for (body, expected) in [
        (
            "Text(content: app.title, at: 0.5)",
            "place an element on the `Rail` it stands in",
        ),
        (
            "Rail { Spacer }",
            "needs `at:` (a fraction of the rail) or `from:` and `to:`",
        ),
        ("Rail { Spacer(at: 0.1, from: 0, to: 1) }", "not both"),
        ("Rail { Spacer(from: 0) }", "`from:` and `to:` go together"),
        ("Rail { Spacer(at: draft.on) }", "`at` must be a number"),
        (
            "Text(content: app.title, stagger: 0.1)",
            "spaces out the children of a container",
        ),
        (
            "Column(stagger: 2) { Spacer }",
            "a delay from 0 to 1 second",
        ),
        (
            "Rail(axis: diagonal) { Spacer(at: 0) }",
            "expected one of: horizontal, vertical",
        ),
    ] {
        let message = errors(body);
        assert!(message.contains(expected), "{body}: {message}");
    }
}

#[test]
fn lengths_follow_state_and_ifs_choose_values() {
    let compiled = compile_ui(&wrap(
        r#"Column(padding: if window.idle { 0 } else { 24 }, gap: if draft.on { 8 } else { 2.5 }, width: 320, animate: hero, appearance: dark) {
            Grid(min_width: if draft.on { 120 } else { 240 }) { Spacer }
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let mut column = None;
    for component in &compiled.app.components {
        component.body.walk(&mut |node| {
            if let ir::Node::Element(element) = node
                && element.kind == "Column"
            {
                column = Some(element.clone());
            }
        });
    }
    let column = column.expect("column");
    assert!(
        matches!(column.prop("padding"), Some(ir::Prop::Expr { expr }) if expr.ty == Type::Int)
    );
    // A whole number beside a float makes a float.
    assert!(matches!(column.prop("gap"), Some(ir::Prop::Expr { expr }) if expr.ty == Type::Float));
    assert!(matches!(column.prop("width"), Some(ir::Prop::Number { value }) if *value == 320.0));
    assert_eq!(column.keyword("animate"), Some("hero"));
    assert_eq!(column.keyword("appearance"), Some("dark"));

    for (body, expected) in [
        (
            "Column(padding: if app.title { 0 } else { 1 }) { Spacer }",
            "condition is a `bool`",
        ),
        (
            "Column(padding: if draft.on { 0 } else { app.title }) { Spacer }",
            "same type",
        ),
        ("Column(padding: draft.name) { Spacer }", "is a length"),
        (
            "Column(padding: if draft.on { 0 }) { Spacer }",
            "needs an `else`",
        ),
        ("Column(animate: wobble) { Spacer }", "wobble"),
    ] {
        let message = errors(body);
        assert!(message.contains(expected), "{body}: {message}");
    }
}

#[test]
fn shortcuts_are_normalized_and_checked() {
    assert_eq!(
        ir::spec::parse_shortcut("Shift+CMD+P").as_deref(),
        Ok("cmd+shift+p")
    );
    assert_eq!(ir::spec::parse_shortcut("space").as_deref(), Ok("space"));
    assert_eq!(ir::spec::parse_shortcut("cmd+,").as_deref(), Ok("cmd+,"));
    let compiled = compile_ui(&wrap(
        r#"Column {
            Button(id: go, label: tr! { "en" => "Go", "ja" => "行く" }, shortcut: "cmd+k") => app.add(1)
            Switch(id: on, label: tr! { "en" => "On", "ja" => "オン" }, value: draft.on, style: button, shortcut: "escape")
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let mut shortcuts = Vec::new();
    for component in &compiled.app.components {
        component.body.walk(&mut |node| {
            if let ir::Node::Element(element) = node
                && let Some(shortcut) = element.keyword("shortcut")
            {
                shortcuts.push(shortcut.to_string());
            }
        });
    }
    assert_eq!(shortcuts, ["cmd+k", "escape"]);
    for (shortcut, expected) in [
        ("hyper+k", "is not a modifier"),
        ("cmd+enter", "is not a key"),
        ("cmd+cmd+k", "written twice"),
    ] {
        let message = errors(&format!(
            r#"Button(id: go, label: tr! {{ "en" => "Go", "ja" => "行く" }}, shortcut: "{shortcut}") => app.add(1)"#
        ));
        assert!(message.contains(expected), "{shortcut}: {message}");
    }
}

#[test]
fn accents_and_appearance_come_from_enums() {
    let source = |props: &str, accents: &str| {
        format!(
            "{}\nstyle! {{ accents {{ {accents} }} }}\n",
            wrap("Spacer").replace(
                "WindowSet(handle: AppWindows, primary: main)",
                &format!("WindowSet(handle: AppWindows, primary: main{props})"),
            )
        )
    };
    let compiled = compile_ui(&source(
        ", accent: app.mode",
        r##"fast: "#FF2E55", slow: color(light: "#2A51D4", dark: "#6E8BFF")"##,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert!(compiled.app.windows.accent.is_some());
    assert_eq!(compiled.app.style.accents.len(), 2);
    assert!(compiled.app.style.accent("Fast").is_some());

    for (props, accents, expected) in [
        (
            ", accent: app.mode",
            r##"fast: "#FF2E55""##,
            "accent `Slow` is not declared",
        ),
        (
            ", accent: app.title",
            r##"fast: "#FF2E55""##,
            "is an enum whose variants",
        ),
        (
            ", appearance: app.mode",
            "",
            "an enum of `System`, `Light` and `Dark`",
        ),
        ("", "fast: accent", "an accent is"),
    ] {
        let message = match compile_ui(&source(props, accents)) {
            Ok(_) => panic!("expected `{props}` / `{accents}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        assert!(message.contains(expected), "{props} {accents}: {message}");
    }
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

/// Corners `style!` names, with Windows' own values.
const CORNERS: &str = "corner { card: 22, poster: 10 }
    #[target(windows)] corner { card: 8, poster: 6 }
    Button.stage { corner: card }";

/// Compile `body` with [`CORNERS`] in a project that has a picture.
fn with_corners(body: &str) -> Result<Compiled, Vec<Diagnostic>> {
    let dir = write_project(
        PROTOCOL,
        &format!("{}\nstyle! {{ {CORNERS} }}\n", wrap(body)),
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
fn named_corners_take_each_platform_radius() {
    let compiled = with_corners(
        r#"Column {
            Button(id: stage, label: app.title, variant: stage) {
                Image(source: "pics/a.png", width: 528, height: 297, corner: card)
            } => app.add(1)
            Image(source: "pics/a.png", width: 100, height: 150, corner: if draft.on { poster } else { 0 })
            Canvas(height: 40, label: tr! { "en" => "Frame", "ja" => "枠" }) {
                Shape(kind: rounded, corner: card, fill: fill.subtle)
            }
        }"#,
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let style = &compiled.app.style;
    assert_eq!(style.corner("card", "macos"), Some(22.0));
    assert_eq!(style.corner("card", "windows"), Some(8.0));
    assert_eq!(style.corner("poster", "linux"), Some(10.0));
    // The rule takes the radius everywhere, then Windows' own after it.
    use ir::style::StyleEffect;
    let stage: Vec<_> = style
        .rules
        .iter()
        .filter(|rule| rule.variant.as_deref() == Some("stage"))
        .collect();
    assert_eq!(stage.len(), 2);
    assert_eq!(stage[0].base, [StyleEffect::Corner { radius: 22.0 }]);
    assert_eq!(stage[1].targets, ["windows"]);
    assert_eq!(stage[1].base, [StyleEffect::Corner { radius: 8.0 }]);
    let corners = |platform: &str| {
        let app = compiled.app.on_platform(platform);
        let mut found = Vec::new();
        app.walk(&mut |node, _| {
            if let ir::Node::Element(element) = node
                && let Some(corner) = element.prop("corner")
            {
                found.push(match corner {
                    ir::Prop::Number { value } => format!("{} {value}", element.kind),
                    ir::Prop::Expr { expr } => match &expr.kind {
                        ir::ExprKind::If { then, .. } => {
                            format!("{} if {:?}", element.kind, then.kind)
                        }
                        other => format!("{} {other:?}", element.kind),
                    },
                    other => format!("{} {other:?}", element.kind),
                });
            }
        });
        found
    };
    assert_eq!(
        corners("macos"),
        [
            "Image 22",
            "Image if Literal { value: Float(10.0) }",
            "Shape 22"
        ]
    );
    assert_eq!(
        corners("windows"),
        [
            "Image 8",
            "Image if Literal { value: Float(6.0) }",
            "Shape 8"
        ]
    );

    for (body, expected) in [
        (
            r#"Image(source: "pics/a.png", width: 10, height: 10, corner: tile)"#,
            "corner `tile` is not declared; `style!` names card, poster",
        ),
        (
            r#"Image(source: "pics/a.png", width: 10, height: 10, corner: if draft.on { tile } else { 0 })"#,
            "corner `tile` is not declared",
        ),
    ] {
        let message = match with_corners(body) {
            Ok(_) => panic!("expected `{body}` to fail"),
            Err(errors) => errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        assert!(message.contains(expected), "{body}: {message}");
    }
    for (body, expected) in [
        (
            "#[target(windows)] corner { card: 8 }",
            "needs a radius for every platform first",
        ),
        (
            "corner { card: 22 } corner { card: 20 }",
            "already has a radius for every platform",
        ),
        ("corner { card: 22, card: 20 }", "is named twice"),
        (
            "corner { card: 22 } #[target(amiga)] corner { card: 8 }",
            "is not a platform",
        ),
        ("corner { card: -2 }", "0 or more"),
        (
            "Button.stage { corner: card }",
            "corner `card` is not declared",
        ),
    ] {
        assert_style_error(body, expected);
    }
}

#[test]
fn buttons_show_content_and_reveal_on_hover() {
    let compiled = compile_ui(&wrap(
        r#"Stack(align: top_trailing) {
            Button(id: card, label: tr! { "en" => "Open", "ja" => "開く" }) {
                Column {
                    Text(content: app.title)
                    Spacer
                }
            } => app.add(1)
            IconButton(id: star, icon: "star", label: tr! { "en" => "Star", "ja" => "星" }, reveal: hover) => app.add(2)
        }"#,
    ))
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let (mut content, mut revealed) = (None, None);
    for component in &compiled.app.components {
        component.body.walk(&mut |node| {
            if let ir::Node::Element(element) = node {
                if element.id.as_deref() == Some("card") {
                    content = Some(element.children.len());
                }
                if element.keyword("reveal") == Some("hover") {
                    revealed = Some(element.kind.clone());
                }
            }
        });
    }
    assert_eq!(content, Some(1));
    assert_eq!(revealed.as_deref(), Some("IconButton"));

    let label = r#"label: tr! { "en" => "Open", "ja" => "開く" }"#;
    for (body, expected) in [
        (
            format!(
                "Button(id: card, {label}) {{ Text(content: app.title) Spacer }} => app.add(1)"
            ),
            "shows one child",
        ),
        (
            format!("Button(id: card, {label}, icon: \"star\") {{ Spacer }} => app.add(1)"),
            "draws no label of its own",
        ),
        (
            format!(
                "Button(id: card, {label}) {{ Column {{ Button(id: inner, {label}) => app.add(2) }} }} => app.add(1)"
            ),
            "cannot hold `Button`",
        ),
        (
            "Text(content: app.title, reveal: always)".to_string(),
            "hover",
        ),
    ] {
        let message = errors(&body);
        assert!(message.contains(expected), "{body}: {message}");
    }
}

/// Parse the body of a `style!` invocation.
fn style_item(body: &str) -> bern_syntax::StyleItem {
    let item: syn::ItemMacro = syn::parse_str(&format!("style! {{ {body} }}")).unwrap();
    bern_syntax::parse_style(item.mac.tokens).unwrap()
}

/// Lower the application's one `style!`, declared in `ui.rs`.
fn lower_style(body: &str) -> (ir::style::StyleSheet, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let sheet = style::lower(&[("ui.rs".into(), style_item(body))], &mut diagnostics);
    (sheet, diagnostics)
}

/// Lowering `body` reports a diagnostic containing `expected`.
fn assert_style_error(body: &str, expected: &str) {
    let (_, diagnostics) = lower_style(body);
    assert!(
        diagnostics.iter().any(|d| d.message.contains(expected)),
        "{body}: {diagnostics:?}"
    );
}

#[test]
fn style_rules_are_lowered_and_checked() {
    let (sheet, diagnostics) = lower_style(
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

    let (sheet, diagnostics) = lower_style(
        "Button.card { shadow: 4, hover: shadow(12, y: 5, opacity: 0.3) + scale(1.03, snappy) }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let card = &sheet.rules[0];
    use ir::style::StyleEffect;
    assert_eq!(
        card.base[0],
        StyleEffect::Shadow {
            radius: 4.0,
            y: 4.0 / 3.0,
            opacity: 0.2
        }
    );
    assert_eq!(
        card.states[0].effects[0],
        StyleEffect::Shadow {
            radius: 12.0,
            y: 5.0,
            opacity: 0.3
        }
    );

    // A border at rest takes a width as the states' does.
    let (sheet, diagnostics) = lower_style(
        "Button.day { border: border(fill.subtle, width: 0.5), hover: border(accent) }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(matches!(
        &sheet.rules[0].base[0],
        StyleEffect::Border { width, .. } if *width == 0.5
    ));
    assert!(matches!(
        &sheet.rules[0].states[0].effects[0],
        StyleEffect::Border { width, .. } if *width == 1.0
    ));

    for (body, expected) in [
        ("Button { hover: shadow(-2) }", "radius of 0 or more"),
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
        ("Button { margin: 4 }", "is not a style property"),
        (
            "Button { width: 4 }",
            "`Button` writes no `width`; a style sets it on Row, Column, Stack",
        ),
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
        // Effects that miss their color, and lengths and opacities out
        // of range.
        (
            "Button { hover: background() }",
            "`background` takes a color, such as `background(accent)`",
        ),
        (
            "Button { hover: foreground() }",
            "`foreground` takes a color",
        ),
        ("Button { hover: border() }", "`border` takes a color"),
        ("SidebarItem { selected: fill() }", "`fill` takes a color"),
        (
            "SidebarItem { selected: indicator(edge: top) }",
            "`indicator` takes a color",
        ),
        (
            "Button { hover: border(accent, width: -1) }",
            "a border's width is 0 or more",
        ),
        (
            "SidebarItem { selected: indicator(accent, width: -3) }",
            "an indicator's width is 0 or more",
        ),
        (
            "Button { hover: opacity(1.5) }",
            "an opacity is from 0 to 1",
        ),
        (
            "Button { hover: opacity(-0.5) }",
            "an opacity is from 0 to 1",
        ),
        ("Button { corner: -4 }", "`corner` is 0 or more"),
        ("Button { padding: -2 }", "`padding` is 0 or more"),
    ] {
        assert_style_error(body, expected);
    }
}

#[test]
fn an_application_lowers_at_most_one_style() {
    let mut diagnostics = Vec::new();
    let sheet = style::lower(&[], &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(sheet.motion.len(), ir::style::BUILT_IN_MOTION.len());
    assert_eq!(sheet.transitions, ir::style::built_in_transitions());
    assert!(sheet.rules.is_empty() && sheet.patterns.is_empty());
    assert!(sheet.accents.is_empty() && sheet.text_weights.is_empty());

    // Only the first is lowered; each further one is reported where it
    // is declared.
    let sheet = style::lower(
        &[
            ("a.rs".into(), style_item("Button { corner: 4 }")),
            ("b.rs".into(), style_item("IconButton { corner: 8 }")),
            ("c.rs".into(), style_item("Stack { padding: 2 }")),
        ],
        &mut diagnostics,
    );
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].component, "Button");
    let files: Vec<&str> = diagnostics
        .iter()
        .map(|d| {
            assert!(d.message.contains("at most one `style!`"), "{d:?}");
            d.location.as_ref().unwrap().file.as_str()
        })
        .collect();
    assert_eq!(files, ["b.rs", "c.rs"]);
}

#[test]
fn motion_tokens_are_springs_that_replace_built_ins_by_name() {
    let (sheet, diagnostics) = lower_style(
        "motion { snappy: spring(period: 0.3, damping: 0.7), wobble: spring(period: 0.6, damping: 0.3) }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(sheet.motion.len(), ir::style::BUILT_IN_MOTION.len() + 1);
    let snappy = sheet.token("snappy").unwrap();
    assert_eq!((snappy.period, snappy.damping), (0.3, 0.7));
    assert_eq!(sheet.token("wobble").unwrap().period, 0.6);
    assert!(sheet.token("smooth").is_some());

    for (body, expected) in [
        ("motion { wobble: 0.5 }", "a motion token is `spring("),
        (
            "motion { wobble: bounce(period: 0.5, damping: 0.5) }",
            "a motion token is `spring(",
        ),
        (
            "motion { wobble: spring(period: 0.5) }",
            "a spring needs `period:` and `damping:`",
        ),
        (
            "motion { wobble: spring(period: 0, damping: 0.5) }",
            "a spring's period and damping are positive",
        ),
        (
            "motion { wobble: spring(period: 0.5, damping: -1) }",
            "a spring's period and damping are positive",
        ),
    ] {
        let (sheet, diagnostics) = lower_style(body);
        assert!(
            diagnostics.iter().any(|d| d.message.contains(expected)),
            "{body}: {diagnostics:?}"
        );
        assert!(sheet.token("wobble").is_none(), "{body}");
    }
}

#[test]
fn transitions_name_a_kind_and_a_motion_token() {
    use ir::style::{Edge, TransitionKind};
    let (sheet, diagnostics) = lower_style(
        "motion { wobble: spring(period: 0.6, damping: 0.3) }
         transition {
             appear: fade(wobble),
             rise: slide(edge: top, distance: 24, snappy),
             drop: slide(),
             unfold: grow(edge: leading),
             open: grow(),
             zoom: pop(from: 0.5),
             pop: pop(0.6, bouncy),
         }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let kind = |name: &str| {
        let transition = sheet.transition(name).unwrap();
        (transition.kind.clone(), transition.motion.as_str())
    };
    assert_eq!(kind("appear"), (TransitionKind::Fade, "wobble"));
    assert_eq!(
        kind("rise"),
        (
            TransitionKind::Slide {
                edge: Edge::Top,
                distance: 24.0
            },
            "snappy"
        )
    );
    // Without arguments: from the bottom, 16 points, the smooth token.
    assert_eq!(
        kind("drop"),
        (
            TransitionKind::Slide {
                edge: Edge::Bottom,
                distance: 16.0
            },
            "smooth"
        )
    );
    assert_eq!(
        kind("unfold"),
        (
            TransitionKind::Grow {
                edge: Edge::Leading
            },
            "smooth"
        )
    );
    assert_eq!(
        kind("open"),
        (TransitionKind::Grow { edge: Edge::Bottom }, "smooth")
    );
    assert_eq!(kind("zoom"), (TransitionKind::Pop { from: 0.5 }, "smooth"));
    // A built-in transition is replaced, not declared twice.
    assert_eq!(kind("pop"), (TransitionKind::Pop { from: 0.6 }, "bouncy"));
    assert_eq!(sheet.transitions.len(), 3 + 6);

    for (body, expected) in [
        ("transition { a: fade }", "a transition is `fade(token)`"),
        (
            "transition { a: spin(smooth) }",
            "`spin` is not a transition; use fade, pop, grow or slide",
        ),
        (
            "transition { a: slide(edge: left) }",
            "`left` is not an edge; use leading, trailing, top or bottom",
        ),
        (
            "transition { a: fade(wobbly) }",
            "`wobbly` is not a motion token; declare it under `motion { … }`",
        ),
    ] {
        assert_style_error(body, expected);
    }
    let (sheet, _) = lower_style("transition { a: spin(smooth) }");
    assert!(sheet.transition("a").is_none());
}

#[test]
fn patterns_weave_colored_bands_and_gaps() {
    use ir::style::{Band, ColorRef};
    let (sheet, diagnostics) = lower_style(
        r##"pattern {
            tartan: check(band(accent, 4), gap(2), band(color(light: "#102030", dark: "#405060"), 1, 0.5)),
            plain: check(band(accent, 1)),
            plain: check(band(accent, 2), twill: true),
        }"##,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let tartan = sheet.pattern("tartan").unwrap();
    assert_eq!(
        tartan.bands,
        [
            // A band is opaque unless it says otherwise.
            Band {
                color: Some(ColorRef::Role {
                    role: "accent".into()
                }),
                width: 4.0,
                opacity: 1.0
            },
            Band {
                color: None,
                width: 2.0,
                opacity: 0.0
            },
            Band {
                color: Some(ColorRef::Fixed {
                    light: ir::Srgb::parse("#102030").unwrap(),
                    dark: ir::Srgb::parse("#405060").unwrap()
                }),
                width: 1.0,
                opacity: 0.5
            },
        ]
    );
    assert!(!tartan.twill);
    // A later pattern of the same name replaces the earlier one.
    assert_eq!(sheet.patterns.len(), 2);
    let plain = sheet.pattern("plain").unwrap();
    assert_eq!(plain.bands[0].width, 2.0);
    assert!(plain.twill);

    for (body, expected) in [
        (
            "pattern { a: check(band(accent)) }",
            "a band is `band(color, width, opacity)`",
        ),
        (
            "pattern { a: check(band(accent, wide)) }",
            "a band's width is a number",
        ),
        (
            "pattern { a: check(band(accent, 2, half)) }",
            "a band's opacity is a number",
        ),
        (
            "pattern { a: check(band(accent, 2), gap(wide)) }",
            "a gap's width is a number",
        ),
        (
            "pattern { a: check(band(accent, 2), stripe(1)) }",
            "a pattern holds `band(…)`, `gap(…)` and `twill: true`",
        ),
        ("pattern { a: plaid }", "a pattern is `check(band(color"),
        (
            "pattern { a: check(band(accent, 0)) }",
            "a pattern needs a colored band wider than zero",
        ),
        (
            "pattern { a: check(band(accent, 2), gap()) }",
            "a gap is `gap(width)`",
        ),
        (
            "pattern { a: check(band(accent, 2), band(accent, -2)) }",
            "a band's width is 0 or more",
        ),
        (
            "pattern { a: check(band(accent, 2), gap(-1)) }",
            "a gap's width is 0 or more",
        ),
        (
            "pattern { a: check(band(accent, 2, 1.5)) }",
            "a band's opacity is from 0 to 1",
        ),
    ] {
        assert_style_error(body, expected);
    }
}

#[test]
fn accents_are_colors_for_both_modes() {
    use ir::style::ColorRef;
    let red = ir::Srgb::parse("#FF0000").unwrap();
    let (sheet, diagnostics) = lower_style(
        r##"accents {
            red: "#FF0000",
            blue: color(light: "#0000FF", dark: "#8080FF"),
            green: "#00FF00",
            green: "#008800",
            pink: "#F48BA6",
        }"##,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // One hex color that reads in both modes serves both as chosen.
    assert_eq!(
        sheet.accent("red").unwrap().color,
        ColorRef::Fixed {
            light: red,
            dark: red
        }
    );
    assert_eq!(
        sheet.accent("blue").unwrap().color,
        ColorRef::Fixed {
            light: ir::Srgb::parse("#0000FF").unwrap(),
            dark: ir::Srgb::parse("#8080FF").unwrap()
        }
    );
    // A later accent of the same name replaces the earlier one.
    assert_eq!(sheet.accents.len(), 4);
    let green = ir::Srgb::parse("#008800").unwrap();
    assert_eq!(
        sheet.accent("green").unwrap().color,
        ColorRef::Fixed {
            light: green,
            dark: green
        }
    );
    // A pale one darkens on light surfaces until it and the text on it
    // read, and stays as chosen on dark ones.
    let pink = ir::Srgb::parse("#F48BA6").unwrap();
    let ColorRef::Fixed { light, dark } = sheet.accent("pink").unwrap().color.clone() else {
        panic!("an accent is a fixed color");
    };
    assert_eq!(dark, pink);
    assert_ne!(light, pink);

    for (body, expected) in [
        (
            r##"accents { red: "#F00" }"##,
            "`#F00` is not a `#RRGGBB` color",
        ),
        (
            r##"accents { red: color(light: "#FF0000", dark: "red") }"##,
            "`red` is not a `#RRGGBB` color",
        ),
        (
            r##"accents { red: color(light: "#FF0000", dark: 3) }"##,
            "a color is written `\"#RRGGBB\"`",
        ),
        (
            r##"accents { red: color(light: "#FF0000", night: "#000000") }"##,
            "write `color(light: \"#RRGGBB\", dark: \"#RRGGBB\")`",
        ),
        (
            r##"accents { red: color(light: "#FF0000") }"##,
            "a fixed color gives both modes",
        ),
    ] {
        assert_style_error(body, expected);
        assert!(lower_style(body).0.accents.is_empty(), "{body}");
    }
}

#[test]
fn text_styles_take_a_weight_and_a_size() {
    use ir::style::TextWeight;
    let (sheet, diagnostics) = lower_style(
        "text {
            large_title: heavy,
            title: font(size: 30, weight: semibold),
            body: font(size: 15, tv: 18),
            caption: bold,
            caption: light,
        }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let weight = |style: &str, weight: &str, size: Option<f64>, tv_size: Option<f64>| TextWeight {
        style: style.into(),
        weight: weight.into(),
        size,
        tv_size,
    };
    assert_eq!(
        sheet.text_weights,
        [
            weight("large_title", "heavy", None, None),
            weight("title", "semibold", Some(30.0), None),
            // A size alone keeps the regular weight; `tv` is the size on a
            // television.
            weight("body", "regular", Some(15.0), Some(18.0)),
            // A later entry for a style replaces the earlier one.
            weight("caption", "light", None, None),
        ]
    );

    for (body, expected) in [
        ("text { poster: heavy }", "`poster` is not a text style"),
        (
            "text { control: font(size: 14) }",
            "`control` is the platform's control font",
        ),
        ("text { title: chunky }", "`chunky` is not a weight"),
        (
            "text { title: font(weight: chunky) }",
            "`chunky` is not a weight",
        ),
        (
            "text { title: font(size: big) }",
            "`font` takes `size: points`, `tv: points` (the size on a television) and `weight: name`",
        ),
        (
            "text { title: 700 }",
            "a text style takes a weight, such as `heavy`",
        ),
        (
            "text { title: font(size: -3) }",
            "a text size is more than 0 points",
        ),
        (
            "text { title: font(size: 0, weight: bold) }",
            "a text size is more than 0 points",
        ),
    ] {
        assert_style_error(body, expected);
    }
}

#[test]
fn style_rules_take_base_properties_states_and_platforms() {
    use ir::style::{ColorRef, ControlSize, Edge, StyleEffect, StyleState};
    let role = |role: &str| ColorRef::Role { role: role.into() };
    let (sheet, diagnostics) = lower_style(
        r#"#[target(ios, android)]
        #[exempt(browser, "no hover on touch")]
        Button.pill {
            corner: 12,
            padding: 6,
            size: small,
            foreground: text.primary,
            border: control.border,
            hover: offset(y: -2, snappy) + opacity(0.8) + border(accent, width: 2),
            pressed: scale(x: 0.97, y: 0.9, press),
            focus: system,
            disabled: none,
        }
        SidebarItem { selected: fill(accent) + indicator(accent) + indicator(accent, edge: bottom) }
        IconButton.flat { size: extra_large }"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let pill = &sheet.rules[0];
    assert_eq!(pill.variant.as_deref(), Some("pill"));
    assert_eq!(pill.targets, ["ios", "android"]);
    assert_eq!(pill.exempt, ["browser"]);
    assert_eq!(
        pill.base,
        [
            StyleEffect::Corner { radius: 12.0 },
            StyleEffect::Padding { length: 6.0 },
            StyleEffect::Size {
                size: ControlSize::Small
            },
            StyleEffect::Foreground {
                color: role("text.primary")
            },
            // A border is one point wide unless it says otherwise.
            StyleEffect::Border {
                color: role("control.border"),
                width: 1.0
            },
        ]
    );
    let states: Vec<(StyleState, &[StyleEffect])> = pill
        .states
        .iter()
        .map(|state| (state.state, state.effects.as_slice()))
        .collect();
    assert_eq!(
        states,
        [
            (
                StyleState::Hover,
                &[
                    StyleEffect::Offset {
                        x: 0.0,
                        y: -2.0,
                        motion: "snappy".into()
                    },
                    // Without a token, effects move with `smooth`.
                    StyleEffect::Opacity {
                        value: 0.8,
                        motion: "smooth".into()
                    },
                    StyleEffect::Border {
                        color: role("accent"),
                        width: 2.0
                    },
                ][..]
            ),
            (
                StyleState::Pressed,
                &[StyleEffect::Scale {
                    x: 0.97,
                    y: 0.9,
                    motion: "press".into()
                }][..]
            ),
            (StyleState::Focus, &[StyleEffect::System][..]),
            (StyleState::Disabled, &[StyleEffect::Nothing][..]),
        ]
    );
    // An indicator marks the leading edge, three points wide, unless it
    // says otherwise.
    assert_eq!(
        sheet.rules[1].states[0].effects,
        [
            StyleEffect::Fill {
                color: role("accent")
            },
            StyleEffect::Indicator {
                color: role("accent"),
                edge: Edge::Leading,
                width: 3.0
            },
            StyleEffect::Indicator {
                color: role("accent"),
                edge: Edge::Bottom,
                width: 3.0
            },
        ]
    );
    assert_eq!(
        sheet.rules[2].base,
        [StyleEffect::Size {
            size: ControlSize::ExtraLarge
        }]
    );
    assert_eq!(
        style::variants(&sheet),
        [
            ("Button".to_string(), "pill".to_string()),
            ("IconButton".to_string(), "flat".to_string())
        ]
    );

    for (body, expected) in [
        (
            "Stack { hover: scale(1.1) }",
            "`Stack` has no `hover` state",
        ),
        ("Button { corner: round }", "corner `round` is not declared"),
        ("Button { corner: \"8\" }", "`corner` is a number"),
        ("Button { padding: wide }", "`padding` is a number"),
        (
            "Button { size: huge }",
            "`size` is small, regular, large or extra_large",
        ),
        ("Button { settle: wobbly }", "`settle` names a motion token"),
        (
            "#[exempt(amiga, \"old\")] Button { corner: 4 }",
            "`amiga` is not a platform",
        ),
        (
            "Button { hover: glow(accent) }",
            "`glow` is not a style effect",
        ),
        ("Button { hover: 3 }", "expected an effect such as"),
        (
            "Button { hover: scale(snappy) }",
            "write `scale(1.03, token)` or `scale(x: 0.97, y: 0.9, token)`",
        ),
        (
            "Button { hover: opacity(snappy) }",
            "write `opacity(0.6, token)`",
        ),
        (
            "Button { hover: shadow(opacity: 0.5) }",
            "write `shadow(12)` or `shadow(12, y: 4, opacity: 0.2)`",
        ),
        (
            "Button { hover: shadow(8, opacity: 1.5) }",
            "an opacity from 0 to 1",
        ),
        (
            "Button { hover: indicator(accent) }",
            "`indicator` does not apply in the `hover` state",
        ),
        (
            "SidebarItem { selected: indicator(accent, edge: left) }",
            "`left` is not an edge",
        ),
        (
            "Button { background: rgb(1, 2, 3) }",
            "a color is a role such as `accent` or `color(light: …, dark: …)`",
        ),
    ] {
        assert_style_error(body, expected);
    }
    // An effect that does not apply is left out; the others stay.
    let (sheet, _) = lower_style("Button { hover: indicator(accent) + scale(1.1) }");
    assert_eq!(sheet.rules[0].states[0].effects.len(), 1);
    assert_eq!(sheet.rules[0].states[0].effects[0].name(), "scale");
}

#[test]
fn controlled_settings_reject_ambiguous_sources_and_non_enum_selections() {
    let label = r#"tr! { "en" => "Preference", "ja" => "設定" }"#;
    for (body, message) in [
        (
            format!("Switch(id: flag, label: {label}, checked: true)"),
            "needs an action",
        ),
        (
            format!(
                "Switch(id: flag, label: {label}, checked: true, value: draft.on) => app.add(1)"
            ),
            "exactly one",
        ),
        (
            format!("Picker(id: mode, label: {label}, selected: app.count) => app.add(1)"),
            "enum",
        ),
    ] {
        assert!(errors(&body).contains(message), "{}", errors(&body));
    }
    compile_ui(&wrap(&format!("Column {{ Switch(id: flag, label: {label}, checked: true) => app.add(1) Picker(id: mode, label: {label}, selected: app.mode) => app.add(1) }}"))).unwrap();
}

#[test]
fn chip_pickers_take_no_button_size() {
    let label = r#"tr! { "en" => "Mode", "ja" => "モード" }"#;
    for prop in ["shape: capsule", "height: 30"] {
        let message = errors(&format!(
            "Picker(id: mode, label: {label}, value: draft.mode, style: chips, {prop})"
        ));
        assert!(
            message.contains("`style: chips` shows a chip per option"),
            "{message}"
        );
    }
    compile_ui(&wrap(&format!(
        "Flow(gap: 10) {{ Spacer Picker(id: mode, label: {label}, value: draft.mode, style: chips, label_hidden: true) }}"
    )))
    .unwrap();
}

#[test]
fn player_scene_and_controlled_slider_validate_sources() {
    let label = r#"tr! { "en" => "Position", "ja" => "位置" }"#;
    for (body, message) in [
        (
            format!("Slider(id: position, label: {label}, current: app.count, min: 0, max: 100)"),
            "needs an action",
        ),
        (
            format!(
                "Slider(id: position, label: {label}, current: app.count, value: draft.size, min: 0, max: 100) => app.add(app.count)"
            ),
            "exactly one",
        ),
    ] {
        assert!(errors(&body).contains(message), "{}", errors(&body));
    }
    let body = format!(
        r#"ScaleBox(width: 1280, height: 800) {{
        Positioned(x: -68, y: 14, width: 54, height: 16) {{
            Text(content: app.title, style: caption, font_size: if draft.on {{ 20 }} else {{ 30 }})
        }}
        Slider(id: position, label: {label}, current: app.count, min: 0, max: 100) => app.add(app.count)
    }}"#
    );
    compile_ui(&wrap(&body)).unwrap();
}

#[test]
fn slide_and_grow_take_their_edge_and_distance_by_name_or_by_position() {
    use ir::style::{Edge, TransitionKind};
    let (sheet, diagnostics) = lower_style(
        "transition {
             page: slide(bottom, 12, smooth),
             panel: slide(trailing, 24, hero),
             named: slide(edge: trailing, distance: 24, hero),
             mixed: slide(top, distance: 30, bouncy),
             drop: slide(top, jelly),
             lift: slide(leading),
             near: slide(8, snappy),
             unfold: grow(trailing, jelly),
             rise: grow(top),
         }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let kind = |name: &str| {
        let transition = sheet.transition(name).unwrap();
        (transition.kind.clone(), transition.motion.as_str())
    };
    let slide = |edge, distance| TransitionKind::Slide { edge, distance };
    assert_eq!(kind("page"), (slide(Edge::Bottom, 12.0), "smooth"));
    assert_eq!(kind("panel"), (slide(Edge::Trailing, 24.0), "hero"));
    assert_eq!(kind("named"), kind("panel"));
    assert_eq!(kind("mixed"), (slide(Edge::Top, 30.0), "bouncy"));
    assert_eq!(kind("drop"), (slide(Edge::Top, 16.0), "jelly"));
    // A name alone is the edge when it is one, otherwise the token.
    assert_eq!(kind("lift"), (slide(Edge::Leading, 16.0), "smooth"));
    assert_eq!(kind("near"), (slide(Edge::Bottom, 8.0), "snappy"));
    assert_eq!(
        kind("unfold"),
        (
            TransitionKind::Grow {
                edge: Edge::Trailing
            },
            "jelly"
        )
    );
    assert_eq!(
        kind("rise"),
        (TransitionKind::Grow { edge: Edge::Top }, "smooth")
    );

    let not_an_edge = "is not an edge; use leading, trailing, top or bottom";
    for (body, expected) in [
        ("transition { a: slide(right, 24, hero) }", not_an_edge),
        ("transition { a: slide(left, smooth) }", not_an_edge),
        ("transition { a: grow(right, jelly) }", not_an_edge),
        ("transition { a: grow(edge: left) }", not_an_edge),
        (
            "transition { a: slide(right) }",
            "`right` is neither an edge nor a motion token",
        ),
        (
            "transition { a: slide(top, 24, wobbly) }",
            "`wobbly` is not a motion token",
        ),
        (
            "transition { a: slide(bottom, -12, smooth) }",
            "a slide's distance is 0 or more",
        ),
        (
            "transition { a: slide(bottom, 12, 24, smooth) }",
            "write `slide(bottom, 16, token)`",
        ),
        (
            "transition { a: slide(edge: top, bottom, smooth) }",
            "write `slide(bottom, 16, token)`",
        ),
        (
            "transition { a: slide(top, 24, hero, jelly) }",
            "write `slide(bottom, 16, token)`",
        ),
        (
            "transition { a: grow(bottom, 12, smooth) }",
            "write `grow(bottom, token)`",
        ),
    ] {
        assert_style_error(body, expected);
    }
}

#[test]
fn styles_set_the_lengths_elements_write_for_their_platforms() {
    use ir::style::{Metric, StyleEffect};
    let (sheet, diagnostics) = lower_style(
        "Row.brand { gap: 5, padding_x: 12 }
         #[target(browser)] Row.brand { gap: 10, padding_x: 0, width: fit }
         Form.settings { gap: 10, row_height: 44, padding_y: 6 }
         IconButton.transport { font_size: 20, height: 14 }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let metric = |metric, value| StyleEffect::Metric { metric, value };
    assert_eq!(
        sheet.rules[1].base,
        vec![
            metric(Metric::Gap, Some(10.0)),
            metric(Metric::PaddingX, Some(0.0)),
            metric(Metric::Width, None),
        ]
    );
    assert!(
        sheet.rules[2]
            .base
            .contains(&metric(Metric::RowHeight, Some(44.0)))
    );
    assert_eq!(
        sheet.rules[3].base,
        vec![
            metric(Metric::FontSize, Some(20.0)),
            metric(Metric::Height, Some(14.0)),
        ]
    );
    for (body, expected) in [
        (
            "Button.x { gap: 4 }",
            "`Button` writes no `gap`; a style sets it on Row, Column, Form",
        ),
        (
            "Row.x { font_size: 13 }",
            "`Row` writes no `font_size`; a style sets it on IconButton",
        ),
        ("Row.x { width: wide }", "`width` is a length or `fit`"),
        ("Row.x { gap: -1 }", "`gap` is 0 or more"),
        ("Row.x { gap: fit }", "`gap` is a number"),
        (
            "IconButton.x { font_size: 0 }",
            "`font_size` is more than 0",
        ),
        ("Row.x { hover: scale(1.1) }", "`Row` has no `hover` state"),
    ] {
        assert_style_error(body, expected);
    }
}
