//! The `framework-names` example names its value types like types of
//! Compose, Kotlin and Android (`Modifier`, `Color`, `Text`, `List`,
//! `Bundle`, `Role`…) and the variants of one enum like Kotlin's types
//! (`String`, `List`, `Boolean`). Its components and sessions are named
//! so that `View` appended gave the runtime's `BernQueryView` and `bern`
//! prepended a view's own `bernWindow`. Generated Kotlin spells the types
//! and sessions `App_…`, components `AppView_…` and the variables of
//! sessions `session_…`, and writes Kotlin's own types in full inside a
//! sealed class, where its variant classes would hide them.
//!
//! With `BERN_TEST_KOTLIN=1`, Gradle compiles the generated Kotlin (about
//! a minute with its caches warm); `bern build --target android` does the
//! same.

use bern_backend_compose::{ComposeAndroid, toolchain};
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Generate the example into `target/bern-test/compose-names`.
fn generate() -> PathBuf {
    let root = workspace();
    let config =
        bern_compiler::config::load(&root.join("examples/framework-names/Bern.toml")).unwrap();
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
    let target = config
        .targets
        .iter()
        .find(|target| target.platform == "android")
        .unwrap()
        .clone();
    let problems = ComposeAndroid.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/compose-names");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target,
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
    for file in ComposeAndroid.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    output
}

#[test]
fn value_types_named_like_framework_types_compile() {
    let output = generate();
    let kotlin = std::fs::read_to_string(
        output.join("project/app/src/main/kotlin/dev/bern/names/BernGenerated.kt"),
    )
    .unwrap();
    for expected in [
        "enum class App_Layout : BernLabeled {",
        "enum class App_Modifier {",
        "enum class App_Bundle {",
        "data class App_Text(val id: Long, val content: String, val size: Double) {",
        "data class App_List(val texts: List<App_Text>) {",
        "fun decode(reader: BernReader): App_Text = App_Text(",
        // Variant classes named like Kotlin's types, and fields that need
        // those types.
        "sealed class App_Content {",
        "data class Text(val text: App_Text) : App_Content()",
        "data class List(val items: kotlin.collections.List<kotlin.String>) : App_Content()",
        "data class String(val value: kotlin.String) : App_Content()",
        "data class Boolean(val on: kotlin.Boolean) : App_Content()",
        "data class Long(val value: kotlin.Long) : App_Content()",
        "data class Double(val value: kotlin.Double) : App_Content()",
        "data object Empty : App_Content()",
        "val bernName: kotlin.String",
        "enum class App_Shape : BernLabeled {",
        "var layout: App_Layout by mutableStateOf(App_Layout.Column)",
        "BernEngine.paint(App_Color.Green, bernWindow)",
        r#"BernEngine.show(App_Content.String(value = "words"), bernWindow)"#,
        "is App_Content.Text -> {",
        "App_Layout.Column -> {",
        "text: App_Text",
        // Errors name the declared type.
        r#"else -> throw BernDecodeError("invalid Layout")"#,
        // Components and sessions.
        "fun AppView_page() {",
        "fun AppView_empty() {",
        "fun AppView_bern_query() {",
        "fun AppView_caption(text: App_Text) {",
        "AppView_empty()",
        "class App_Draft : BernSessionState(0) {",
        "class App_URL : BernSessionState(1) {",
        "class App_Window : BernSessionState(2) {",
        "val session_Draft = context.session(App_Draft::class.java)",
        "val session_Window = bernWindow.session(App_Window::class.java)",
        "val session_Engine = bernWindow.session(App_Engine::class.java)",
        "if (session_Window.pinned) {",
    ] {
        assert!(
            kotlin.contains(expected),
            "BernGenerated.kt lacks `{expected}`"
        );
    }
    for bare in [
        "class Layout",
        "class Modifier",
        "class Color",
        "class Text(",
        "class List(",
        "class URLSession",
        "class WindowSession",
        "fun EmptyView(",
        "fun BernQueryView(",
    ] {
        assert!(
            !kotlin.contains(&format!("\n{bare}")),
            "BernGenerated.kt declares `{bare}`"
        );
    }

    if std::env::var_os("BERN_TEST_KOTLIN").is_none() {
        eprintln!("BERN_TEST_KOTLIN is not set; skipping the Kotlin compile");
        return;
    }
    let (Some(gradle), Some(sdk)) = (toolchain::gradle(), toolchain::sdk()) else {
        panic!("BERN_TEST_KOTLIN needs Gradle and the Android SDK");
    };
    let mut command = Command::new(gradle);
    command
        .arg("--project-dir")
        .arg(output.join("project"))
        .args(["--console=plain", "--quiet", ":app:compileDebugKotlin"])
        .env("ANDROID_HOME", sdk);
    if let Some(java_home) = toolchain::java_home() {
        command.env("JAVA_HOME", java_home);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "kotlinc rejected the generated code:\n{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
