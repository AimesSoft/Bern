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
                },
                channels: &[],
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
        assert!(problems.iter().all(|d| d.location.is_some()));
    }
}
