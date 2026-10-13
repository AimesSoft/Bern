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
    assert!(kotlin.contains("BernMarkedText("));
    // The archive item comes and goes; the selection falls back.
    assert!(kotlin.contains("LaunchedEffect(bernVisible)"));
    assert!(kotlin.contains("if (bernPlannerDraft.page !in bernVisible)"));
    // Tags pop in when added.
    assert!(kotlin.contains("BernAppear(bernEnter(BernTransition.Pop("));
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
