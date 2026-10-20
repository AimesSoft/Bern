//! Generate the examples for Android and check the Kotlin, the Gradle
//! project and the glue. Compiling the Kotlin needs Gradle and its
//! downloads; `bern build --target android` does that.

use bern_backend_compose::ComposeAndroid;
use bern_target_api::{BuildContext, Framework, Target, TargetConfig};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn compile(example: &str) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let root = workspace();
    let config =
        bern_compiler::config::load(&root.join("examples").join(example).join("Bern.toml"))
            .unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    (config, compiled)
}

fn android(config: &bern_compiler::Config) -> TargetConfig {
    config
        .targets
        .iter()
        .find(|target| target.platform == "android")
        .expect("an android target")
        .clone()
}

fn context(config: &bern_compiler::Config, compiled: &bern_compiler::Compiled) -> BuildContext {
    let root = workspace();
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/compose"),
        target: android(config),
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    }
}

fn file<'a>(files: &'a [bern_target_api::GeneratedFile], suffix: &str) -> &'a str {
    let file = files
        .iter()
        .find(|file| file.path.to_string_lossy().ends_with(suffix))
        .unwrap_or_else(|| panic!("no generated `{suffix}`"));
    std::str::from_utf8(&file.contents).unwrap()
}

#[test]
fn hello_generates_a_gradle_project_and_the_jni_glue() {
    let (config, compiled) = compile("hello");
    let problems = ComposeAndroid.check(&compiled.app, &android(&config));
    assert!(problems.is_empty(), "{problems:?}");
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    let kotlin = file(&files, "dev/bern/hello/BernGenerated.kt");
    assert!(kotlin.contains("package dev.bern.hello"));
    assert!(kotlin.contains("class MainActivity : ComponentActivity()"));
    assert!(kotlin.contains("object BernEngine {"));
    assert!(kotlin.contains("fun wave(bernWindow: BernWindowContext?"));
    let manifest = file(&files, "AndroidManifest.xml");
    assert!(manifest.contains("android:name=\".MainActivity\""));
    let build = file(&files, "app/build.gradle.kts");
    assert!(build.contains("applicationId = \"dev.bern.hello\""));
    assert!(build.contains("minSdk = 26"));
    let glue = file(&files, "glue/src/lib.rs");
    assert!(glue.contains("fn Java_dev_bern_runtime_BernNative_start("));
    assert!(file(&files, "glue/Cargo.toml").contains("crate-type = [\"cdylib\"]"));
    // The runtime's strings in every language, with Java specifiers.
    let texts = file(&files, "assets/bern/texts/zh-Hans.json");
    assert!(
        texts.contains("\"bern.window.open\": \"打开“%s”\""),
        "{texts}"
    );
    assert!(file(&files, "res/values-b+zh+Hans/strings.xml").contains(">你好<"));
    // The control channel brings the command line for this computer.
    assert!(file(&files, "tools/src/cli.rs").contains("bern_adapter_cli::run("));
}

#[test]
fn planner_generates_styles_drawing_and_dynamic_items() {
    let (config, compiled) = compile("planner");
    let problems = ComposeAndroid.check(&compiled.app, &android(&config));
    assert!(
        problems
            .iter()
            .all(|problem| problem.severity == bern_target_api::Severity::Warning),
        "{problems:?}"
    );
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    let kotlin = file(&files, "BernGenerated.kt");
    assert!(kotlin.contains("object BernStyles {"), "{kotlin}");
    assert!(kotlin.contains("val buttonHero = BernStyle("));
    assert!(kotlin.contains("BernStyledSidebarItem("));
    assert!(kotlin.contains("object BernPatterns {"));
    assert!(kotlin.contains("BernDraw.series(this, bernPalette, bernValues0"));
    // A dashed outline.
    assert!(kotlin.contains(", dash = 4.5, dashGap = 3.0)"), "{kotlin}");
    assert!(kotlin.contains("BernMarkedText("));
    // The archive item comes and goes; the selection falls back.
    assert!(kotlin.contains("LaunchedEffect(bernVisible)"));
    assert!(kotlin.contains("if (bernPlannerDraft.page !in bernVisible)"));
    // Tags pop in when added.
    assert!(kotlin.contains("BernAppear(BernTransition.Pop("));
    // The title grows from one style to the other; the window goes
    // immersive once idle.
    assert!(kotlin.contains("BernHero(\"title\", BernMotion("));
    assert!(kotlin.contains("AnimatedContent("));
    assert!(kotlin.contains("BernWindowRoot(context, idleAfter = 3.0, immersive = "));
}

#[test]
fn showcase_generates_video_queries_and_windows() {
    let (config, compiled) = compile("showcase");
    let problems = ComposeAndroid.check(&compiled.app, &android(&config));
    assert!(problems.is_empty(), "{problems:?}");
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    let kotlin = file(&files, "BernGenerated.kt");
    assert!(
        kotlin.contains("BernVideo(\"media_page.featured\""),
        "{kotlin}"
    );
    assert!(kotlin.contains("BernQueryView("));
    assert!(kotlin.contains("BernTabs(content = {"));
    assert!(kotlin.contains("BernToolbarPicker("));
    assert!(kotlin.contains("\"settings\" -> BernWindow"));
    let build = file(&files, "app/build.gradle.kts");
    assert!(build.contains("androidx.media3:media3-exoplayer"));
}

#[test]
fn gallery_generates_stagger_rails_popovers_and_four_sizes() {
    let (config, compiled) = compile("gallery");
    let problems = ComposeAndroid.check(&compiled.app, &android(&config));
    assert!(
        problems
            .iter()
            .all(|problem| problem.severity == bern_target_api::Severity::Warning),
        "{problems:?}"
    );
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    let kotlin = file(&files, "BernGenerated.kt");
    // The wall's cards take turns rising; the card is a component, whose
    // root takes its turn on the clock of the repetition it stands in.
    assert!(
        kotlin.contains("BernStaggered(rememberBernStagger(0.04)) {"),
        "{kotlin}"
    );
    assert!(kotlin.contains(", staggered = true) {"));
    // The film's timeline: ticks at fractions, names from their start, the
    // level's fill as large as the stack it stands in.
    assert!(kotlin.contains("BernRail(false, BernRailAnchor.Center) {"));
    assert!(kotlin.contains("BernRail(false, BernRailAnchor.Start) {"));
    assert!(
        kotlin.contains("BernRail(false, BernRailAnchor.Center, Modifier.matchParentSize()) {")
    );
    assert!(kotlin.contains("BernPlace((swatch.at).toDouble(), null, null, null) {"));
    assert!(
        kotlin.contains(
            "BernPlace(null, (0L).toDouble(), (bernGalleryDraft.level).toDouble(), null) {"
        )
    );
    assert!(
        kotlin.contains("BernPlace((bernGalleryDraft.level).toDouble(), null, null, BernMotion(")
    );
    // The frame's volume in a popover.
    assert!(kotlin.contains("BernPopoverButton(\"frame.volume\""));
    // Four sizes: the wall's padding reads the window's, and the film's
    // controls choose among three layouts.
    assert!(kotlin.contains("bernWindow.large"));
    assert!(kotlin.contains("BernHost.layouts = BernLayouts(600.0, 1024.0, 1440.0)"));
    assert!(kotlin.contains("medium = {"));
    assert!(kotlin.contains("large = null,"));
    // A television's layout and margins, the dialog's first focus, and
    // Escape (a television's Back) closing the details while they show.
    assert!(kotlin.contains("tv = {"));
    assert!(kotlin.contains("bernWindow.tv"));
    assert!(kotlin.contains("bernDefaultFocus()"));
    assert!(
        kotlin.contains("BernShortcut(\"escape\", { true }) {"),
        "{kotlin}"
    );
    // Keys that type, a search that suggests, a television's caption.
    assert!(kotlin.contains("bernGalleryDraft.query = bernGalleryDraft.query + \"check\""));
    assert!(kotlin.contains("offsetByCodePoints"));
    assert!(
        kotlin
            .contains("\"caption\" to Pair(\"regular\", if (BernHost.television) 14.0 else null)")
    );
    // The remote's arrows move the focus before a shortcut takes them.
    assert!(kotlin.contains("if (BernHost.television && BernShortcuts.isArrow(event))"));
}

/// The showcase with a `TabAccessory` in its first `Tabs`, shown while a
/// literal holds.
fn add_tab_accessory(app: &mut bern_ir::Application) {
    use bern_ir::{Element, Expr, ExprKind, Node, Prop, SourceLocation};
    fn into(node: &mut Node) -> bool {
        match node {
            Node::Element(element) if element.kind == "Tabs" => {
                element.children.push(Node::Element(Element {
                    kind: "TabAccessory".into(),
                    id: None,
                    path: "test.accessory".into(),
                    props: [(
                        "shown".into(),
                        Prop::Expr {
                            expr: Expr {
                                ty: bern_protocol::Type::Bool,
                                kind: ExprKind::Literal {
                                    value: bern_protocol::Value::Bool(true),
                                },
                            },
                        },
                    )]
                    .into(),
                    template: None,
                    children: vec![Node::Element(Element {
                        kind: "Spacer".into(),
                        id: None,
                        path: "test.accessory.0".into(),
                        props: Default::default(),
                        template: None,
                        children: vec![],
                        action: None,
                        source: SourceLocation {
                            file: "tabs.rs".into(),
                            line: 1,
                            column: 1,
                        },
                    })],
                    action: None,
                    source: SourceLocation {
                        file: "tabs.rs".into(),
                        line: 1,
                        column: 1,
                    },
                }));
                true
            }
            Node::Element(element) => element.children.iter_mut().any(into),
            Node::If(node) => {
                node.branches
                    .iter_mut()
                    .any(|(_, body)| body.iter_mut().any(into))
                    || node.otherwise.iter_mut().any(into)
            }
            _ => false,
        }
    }
    assert!(
        app.components
            .iter_mut()
            .any(|component| into(&mut component.body))
    );
}

#[test]
fn tab_accessory_sits_above_the_navigation_bar() {
    let (config, mut compiled) = compile("showcase");
    add_tab_accessory(&mut compiled.app);
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    let source = files
        .iter()
        .filter(|f| f.path.extension().is_some_and(|e| e == "kt"))
        .map(|f| String::from_utf8_lossy(&f.contents).into_owned())
        .collect::<String>();
    assert!(source.contains("}, accessory = {"), "{source}");
    assert!(source.contains("if (true) BernTabAccessory {"), "{source}");
}

fn node(
    kind: &str,
    props: Vec<(&str, bern_ir::Prop)>,
    children: Vec<bern_ir::Node>,
) -> bern_ir::Node {
    bern_ir::Node::Element(bern_ir::Element {
        kind: kind.into(),
        id: matches!(kind, "Menu" | "SearchItem" | "Button")
            .then(|| format!("test_{}", kind.to_lowercase())),
        path: format!("test.{kind}"),
        props: props
            .into_iter()
            .map(|(name, prop)| (name.to_string(), prop))
            .collect(),
        template: None,
        children,
        action: None,
        source: bern_ir::SourceLocation {
            file: "features.rs".into(),
            line: 1,
            column: 1,
        },
    })
}

fn number(value: f64) -> bern_ir::Prop {
    bern_ir::Prop::Number { value }
}

fn word(value: &str) -> bern_ir::Prop {
    bern_ir::Prop::Keyword {
        value: value.into(),
    }
}

fn words(value: &str) -> bern_ir::Prop {
    bern_ir::Prop::Text {
        text: bern_ir::Text::Dynamic {
            expr: bern_ir::Expr {
                ty: bern_protocol::Type::Text,
                kind: bern_ir::ExprKind::Literal {
                    value: bern_protocol::Value::Text(value.into()),
                },
            },
        },
    }
}

fn icon(name: &str) -> bern_ir::Prop {
    bern_ir::Prop::Icon { name: name.into() }
}

/// The showcase with a scene, an accordion, wrapping rows, a grid of
/// cells, a modal, a form's layout, icon-only labels and menus, capsule
/// controls and a search item at the end of its first component.
fn add_layout_features(app: &mut bern_ir::Application) {
    let text = || {
        node(
            "Text",
            vec![
                ("content", words("Text")),
                ("lines", number(2.0)),
                ("font_size", number(12.0)),
                ("line_height", number(16.0)),
            ],
            vec![],
        )
    };
    let features = node(
        "Column",
        vec![("padding_x", number(12.0)), ("padding_y", number(4.0))],
        vec![
            node(
                "ScaleBox",
                vec![("width", number(400.0)), ("height", number(300.0))],
                vec![node(
                    "Positioned",
                    vec![
                        ("x", number(10.0)),
                        ("y", number(20.0)),
                        ("width", number(100.0)),
                        ("height", number(40.0)),
                        ("animate", word("smooth")),
                    ],
                    vec![node(
                        "Marquee",
                        vec![("duration", number(8.0)), ("phase", number(0.2))],
                        vec![text()],
                    )],
                )],
            ),
            node(
                "Accordion",
                vec![
                    ("count", number(3.0)),
                    ("selected", number(1.0)),
                    ("collapsed_width", number(40.0)),
                    ("height", number(120.0)),
                    ("corner", number(12.0)),
                    ("animate", word("bouncy")),
                ],
                vec![text(), text(), text()],
            ),
            node(
                "Flow",
                vec![("gap", number(4.0))],
                vec![node(
                    "Label",
                    vec![
                        ("title", words("Star")),
                        ("icon", icon("star")),
                        ("display", word("icon")),
                    ],
                    vec![],
                )],
            ),
            node(
                "Grid",
                vec![
                    ("min_width", number(80.0)),
                    ("row_gap", number(6.0)),
                    ("cell_ratio", number(0.667)),
                    ("cell_extra", number(20.0)),
                ],
                vec![node(
                    "Image",
                    vec![
                        (
                            "source",
                            bern_ir::Prop::Resource {
                                name: "images/bern.png".into(),
                            },
                        ),
                        ("corner", number(8.0)),
                        ("rendering", word("template")),
                        ("aspect", number(1.5)),
                    ],
                    vec![],
                )],
            ),
            node(
                "Row",
                vec![("align", word("baseline"))],
                vec![
                    text(),
                    node("Spacer", vec![], vec![]),
                    node(
                        "Menu",
                        vec![
                            ("label", words("More")),
                            ("icon", icon("ellipsis")),
                            ("style", word("icon")),
                            ("shape", word("capsule")),
                            ("height", number(30.0)),
                        ],
                        vec![],
                    ),
                    node(
                        "ControlGroup",
                        vec![("height", number(32.0))],
                        vec![node(
                            "SearchItem",
                            vec![
                                ("label", words("Open")),
                                ("icon", icon("folder")),
                                ("detail", words("File")),
                                ("hint", words("⌘O")),
                            ],
                            vec![],
                        )],
                    ),
                ],
            ),
            node(
                "Form",
                vec![("gap", number(12.0)), ("row_height", number(44.0))],
                vec![node(
                    "Section",
                    vec![("style", word("plain")), ("padding_x", number(8.0))],
                    vec![text()],
                )],
            ),
            node(
                "Modal",
                vec![
                    ("width", number(300.0)),
                    ("height", number(200.0)),
                    ("cover", word("compact")),
                ],
                vec![text()],
            ),
        ],
    );
    let component = app
        .components
        .iter_mut()
        .find(|component| {
            matches!(&component.body, bern_ir::Node::Element(body) if body.kind == "Form")
        })
        .expect("a component whose body is a form");
    let bern_ir::Node::Element(body) = &mut component.body else {
        unreachable!()
    };
    body.children.push(features);
}

/// The layout features of the scaled scenes, accordions, flows, cell
/// grids, modals and form layouts generate the runtime's composables; the
/// Kotlin compiles in `bern build --target android`.
#[test]
fn layout_features_generate_for_compose() {
    let (config, mut compiled) = compile("showcase");
    add_layout_features(&mut compiled.app);
    let problems = ComposeAndroid.check(&compiled.app, &android(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let kotlin =
        bern_backend_compose::Generator::new(&compiled.app).generate("dev.bern.showcase", 0);
    assert!(!kotlin.contains("unsupported"), "{kotlin}");
    let dump = std::env::var("BERN_DUMP").ok();
    if let Some(path) = dump {
        std::fs::write(path, &kotlin).unwrap();
    }
    for expected in [
        "Column(Modifier.padding(horizontal = 12f.dp, vertical = 4f.dp)",
        "BernScaleBox(400f.dp, 300f.dp) {",
        "BernPositioned(bernLength(10f.dp, BernMotion(",
        "BernMarquee(8.0, 0.2, false, (1).toFloat()) {",
        "BernText(BernText.text(\"Text\"), bernTextStyle(\"body\", size = 12f, lineHeight = 16f), Color.Unspecified, lines = 2)",
        "BernAccordion(3, 1, bernLength(40f.dp, BernMotion(0.5, 0.62)), 8.dp,",
        "BernFlow(4f.dp) {",
        "bernTextStyle(\"body\"), iconOnly = true)",
        "BernGrid(minWidth = 80f.dp, gap = 8.dp, rowGap = 6f.dp, cellRatio = 0.667f, cellExtra = 20f.dp) {",
        ", corner = 8f.dp, template = true, aspect = 1.5f)",
        "Box(Modifier.alignByBaseline()) {",
        "iconOnly = true, shape = BernShape(true, 30f.dp)) { dismiss ->",
        "BernControlGroup(null, 32f.dp) {",
        "BernSearchItem(\"text_page.test_searchitem\", BernText.text(\"Open\")",
        "BernForm(gap = 12f.dp, rows = BernRows(44f.dp, null, null), plainFirst = true) {",
        "BernSection(null, null, plain = true, paddingX = 8f.dp) {",
        "BernModal(300f.dp, 200f.dp, true, null, null) {",
    ] {
        assert!(kotlin.contains(expected), "missing `{expected}`");
    }
}

const FORMATS_PROTOCOL: &str = r#"
#[bern::protocol(namespace = "formats", version = 1)]
pub mod formats {
    use bern::prelude::*;

    #[value]
    pub enum Size {
        #[label(tr! { "en" => "small", "zh-Hans" => "小" })]
        Small,
        #[label(tr! { "en" => "large", "zh-Hans" => "大" })]
        Large,
    }

    #[value]
    pub enum Notice {
        #[label(tr! { "en" => "No notice", "zh-Hans" => "没有通知" })]
        Nothing,
        #[label(tr! { "en" => "Saved {name} on {day, date, EEE, MMM d}", "zh-Hans" => "已保存 {name}，{day, date, M月d日 EEE}" })]
        Saved {
            #[content]
            name: String,
            day: CalendarDate,
        },
        #[label(tr! { "en" => "{n, plural, one {# {size} file} other {# {size} files}}", "zh-Hans" => "{n} 个{size}文件" })]
        Files { n: i64, size: Size },
    }

    #[state]
    pub struct State {
        pub count: i64,
        pub seconds: f64,
        pub today: CalendarDate,
        pub notice: Notice,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self) -> Result<(), CommandError>;
    }
}
"#;

const FORMATS_UI: &str = r#"
use bern::prelude::*;

#[component]
pub fn app(app: formats::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Formats", "zh-Hans" => "格式" }, initial: open) {
                Column {
                    Text(content: tr! { "en" => "{n, plural, =0 {no files} one {# file} other {# files}}", "zh-Hans" => "{n} 个文件"; n: app.count })
                    Text(content: tr! { "en" => "Today is {d, date, EEEE, MMMM d, y}", "zh-Hans" => "今天是 {d, date, y年M月d日 EEEE}"; d: app.today })
                    Text(content: tr! { "en" => "{t, duration, m:ss} used", "zh-Hans" => "已用 {t, duration, m:ss}"; t: app.seconds })
                    Text(content: tr! { "en" => "Braces {{stay}}", "zh-Hans" => "括号 {{保留}}" })
                    Text(content: app.notice)
                }
            }
        }
    }
}
"#;

/// A project with plural branches, dates, durations and labels that show
/// their variant's fields, compiled from sources written under `dir`.
fn formats_project(dir: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("formats-protocol", FORMATS_PROTOCOL),
        ("formats-ui", FORMATS_UI),
        ("formats-core", ""),
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
id = "formats"
bundle_id = "dev.bern.formats"
version = "0.1.0"
name = { "en" = "Formats", "zh-Hans" = "格式" }

[languages]
supported = ["en", "zh-Hans"]
default = "en"

[protocol]
package = "formats-protocol"
source = "formats-protocol/src/lib.rs"

[engine]
package = "formats-core"
type = "formats_core::Engine"

[ui]
package = "formats-ui"
sources = ["formats-ui/src/lib.rs"]
root = "app"

[targets.android]
platform = "android"

[targets.windows]
platform = "windows"
"#,
    )
    .unwrap();
    let config = bern_compiler::config::load(&dir.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    (config, compiled)
}

/// Texts with plural branches, dates and durations pass named, typed
/// arguments; labels fill in their variant's fields; the string tables
/// keep Bern's format for texts with arguments and carry the date names.
#[test]
fn text_formats_generate_for_compose() {
    let root = workspace();
    let (config, compiled) =
        formats_project(&root.join("target/bern-test/compose-formats-project"));
    let problems = ComposeAndroid.check(&compiled.app, &android(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    let kotlin = file(&files, "dev/bern/formats/BernGenerated.kt");
    for expected in [
        "is Saved -> BernText.format(\"Notice.Saved\", mapOf(\"name\" to BernText.arg(name), \"day\" to (day).bernArg))",
        "is Files -> BernText.format(\"Notice.Files\", mapOf(\"n\" to BernText.arg(n), \"size\" to BernText.arg(size)))",
        "is Nothing -> BernText.string(\"Notice.Nothing\")",
        "val bernArg: BernArg get() = BernArg.Date(year, month, day)",
        "mapOf(\"n\" to BernText.arg(BernEngine.count))",
        "mapOf(\"d\" to (BernEngine.today).bernArg)",
        "mapOf(\"t\" to BernText.arg(BernEngine.seconds))",
    ] {
        assert!(kotlin.contains(expected), "missing `{expected}`");
    }
    let en = file(&files, "assets/bern/texts/en.json");
    assert!(
        en.contains(r#""{n, plural, =0 {no files} one {# file} other {# files}}""#),
        "{en}"
    );
    assert!(en.contains(r#""Braces {stay}""#), "{en}");
    assert!(
        en.contains(r#""bern.date.weekday_long.1": "Monday""#),
        "{en}"
    );
    let zh = file(&files, "assets/bern/texts/zh-Hans.json");
    assert!(zh.contains(r#""bern.date.weekday.1": "周一""#), "{zh}");
    assert!(zh.contains(r#""今天是 {d, date, y年M月d日 EEEE}""#), "{zh}");
    let runtime = file(&files, "dev/bern/runtime/BernRuntime.kt");
    assert!(runtime.contains("class BernTextParser"));
}

#[test]
fn a_declared_res_folder_is_the_launcher_icon() {
    let (config, mut compiled) = compile("hello");
    let res = workspace().join("target/bern-test/compose-icon-res");
    let _ = std::fs::remove_dir_all(&res);
    for (path, contents) in [
        ("mipmap-anydpi-v26/ic_launcher.xml", "<adaptive-icon/>"),
        (
            "mipmap-anydpi-v26/ic_launcher_round.xml",
            "<adaptive-icon/>",
        ),
        ("mipmap-hdpi/ic_launcher_foreground.png", "png"),
        ("values/ic_launcher_background.xml", "<resources/>"),
    ] {
        let path = res.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    // An absolute path stays itself when joined to the project directory.
    compiled
        .app
        .app
        .platform_icons
        .insert("android".into(), res.display().to_string());
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    assert_eq!(
        file(&files, "main/res/mipmap-anydpi-v26/ic_launcher.xml"),
        "<adaptive-icon/>"
    );
    assert_eq!(
        file(&files, "main/res/values/ic_launcher_background.xml"),
        "<resources/>"
    );
    let manifest = file(&files, "AndroidManifest.xml");
    assert!(manifest.contains("android:icon=\"@mipmap/ic_launcher\""));
    assert!(
        manifest.contains("android:roundIcon=\"@mipmap/ic_launcher_round\""),
        "{manifest}"
    );
    // The folder replaces the PNG launcher icon.
    assert!(
        !files
            .iter()
            .any(|file| file.path.ends_with("mipmap-xxxhdpi/ic_launcher.png"))
    );
    let _ = std::fs::remove_dir_all(&res);
}

/// `app` with its first icon replaced by `name`.
fn with_icon(app: &bern_ir::Application, name: &str) -> bern_ir::Application {
    fn replace(value: &mut serde_json::Value, name: &str) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                if map.get("kind").and_then(|kind| kind.as_str()) == Some("icon") {
                    map.insert("name".into(), name.into());
                    return true;
                }
                map.values_mut().any(|child| replace(child, name))
            }
            serde_json::Value::Array(items) => items.iter_mut().any(|child| replace(child, name)),
            _ => false,
        }
    }
    let mut value = serde_json::to_value(app).unwrap();
    assert!(replace(&mut value, name), "the application has no icon");
    serde_json::from_value(value).unwrap()
}

/// `BernIcons.get` has a branch for every name of the Bern icon set, each
/// a Material icon; a name the table lacks fails the check at the element
/// instead of showing the neutral dot when the application runs.
#[test]
fn icons_come_from_the_table_and_others_fail_the_check() {
    let (config, compiled) = compile("gallery");
    let files = ComposeAndroid
        .generate(&compiled.app, &context(&config, &compiled))
        .unwrap();
    let views = file(&files, "dev/bern/runtime/BernViews.kt");
    assert!(!views.contains("/*bern:icon-branches*/"));
    for name in bern_target_api::icon_set::NAMES {
        assert!(views.contains(&format!("        \"{name}\" -> ")), "{name}");
    }
    for branch in [
        "\"film.stack\" -> videoLibrary",
        "\"server.rack\" -> dns",
        "\"sparkles\" -> autoAwesome",
        "\"puzzlepiece.extension\" -> extension",
        "\"arrow.triangle.2.circlepath\" -> sync",
        "\"wifi\" -> wifi",
        "\"keyboard\" -> keyboard",
        "\"minus\" -> remove",
        "\"arrow.left\" -> Icons.AutoMirrored.Filled.ArrowBack",
    ] {
        assert!(views.contains(branch), "{branch}");
    }
    assert!(views.contains("private val videoLibrary by lazy"));
    for name in ["film.stack", "server.rack", "no.such.symbol"] {
        let problems = ComposeAndroid.check(&with_icon(&compiled.app, name), &android(&config));
        let rejected = problems.iter().any(|problem| {
            problem.is_error()
                && problem
                    .message
                    .starts_with(&format!("the Material icon set has no `{name}`"))
        });
        assert_eq!(rejected, name == "no.such.symbol", "{name}: {problems:?}");
    }
}
