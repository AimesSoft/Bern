//! Wear OS packages use Wear controls and explicit watch layouts; other
//! targets must not compile any watch-only component or expression.
use bern_backend_compose::{ComposeAndroid, ComposeWearos};
use bern_ir::{Binding, ExprKind, Node};
use bern_target_api::{
    AndroidOptions, BuildContext, EjectEngine, EjectOptions, Framework, GeneratedFile, Target,
    TargetConfig,
};
use std::path::Path;

fn sample() -> (bern_compiler::Config, bern_compiler::Compiled) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config = bern_compiler::config::load(&root.join("examples/wearos/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    (config, compiled)
}

fn context(
    config: &bern_compiler::Config,
    compiled: &bern_compiler::Compiled,
    platform: &str,
) -> BuildContext {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/wearos-generate"),
        target: config
            .targets
            .iter()
            .find(|t| t.platform == platform)
            .unwrap()
            .clone(),
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: vec![],
        framework: Framework::Path(root.join("crates")),
        release: false,
    }
}

fn text<'a>(files: &'a [GeneratedFile], path: &str) -> &'a str {
    std::str::from_utf8(
        &files
            .iter()
            .find(|f| f.path.to_string_lossy().ends_with(path))
            .unwrap_or_else(|| panic!("missing {path}"))
            .contents,
    )
    .unwrap()
}

#[test]
fn watch_build_uses_its_own_manifest_controls_and_package_identity() {
    let (config, compiled) = sample();
    let ctx = context(&config, &compiled, "wearos");
    let errors = ComposeWearos.check(&compiled.app, &ctx.target);
    assert!(!errors.iter().any(|e| e.is_error()), "{errors:?}");
    let files = ComposeWearos.generate(&compiled.app, &ctx).unwrap();
    let manifest = text(&files, "AndroidManifest.xml");
    assert!(manifest.contains("android.hardware.type.watch"));
    assert!(manifest.contains("com.google.android.wearable.standalone"));
    assert!(!manifest.contains("LEANBACK_LAUNCHER"));
    let build = text(&files, "app/build.gradle.kts");
    assert!(build.contains("androidx.wear.compose:compose-material3:1.7.1"));
    assert!(!build.contains("androidx.compose.material3:material3"));
    assert!(build.contains("minSdk = 26"));
    assert!(build.contains("abiFilters += \"arm64-v8a\""));
    assert!(!build.contains("splits {"));
    let metadata: serde_json::Value = serde_json::from_str(text(&files, "build.json")).unwrap();
    assert_eq!(metadata["target"], "compose-wearos");
    assert_eq!(metadata["platform"], "wearos");
    assert_eq!(metadata["minimum_sdk"], 26);
    assert_eq!(metadata["abis"], serde_json::json!(["arm64-v8a"]));
    for f in &files {
        if f.path.extension().is_some_and(|ext| ext == "kt") {
            let source = String::from_utf8_lossy(&f.contents);
            assert!(
                !source.contains("androidx.compose.material3."),
                "phone Material in {}",
                f.path.display()
            );
        }
    }
    let kotlin = text(&files, "BernGenerated.kt");
    assert!(kotlin.contains("watch = {"));
    assert!(kotlin.contains("compact = null"));
    assert!(kotlin.contains("BernLazyList"));
}

#[test]
fn a_phone_retains_phone_controls_and_no_watch_branch() {
    let (config, compiled) = sample();
    let ctx = context(&config, &compiled, "android");
    let files = ComposeAndroid.generate(&compiled.app, &ctx).unwrap();
    assert!(!text(&files, "AndroidManifest.xml").contains("android.hardware.type.watch"));
    assert!(text(&files, "app/build.gradle.kts").contains("androidx.compose.material3:material3"));
    assert!(!text(&files, "BernGenerated.kt").contains("watch = {"));
    assert!(!text(&files, "BernGenerated.kt").contains("BernHost.watch = true"));
}

#[test]
fn missing_watch_layout_fails_instead_of_falling_back_to_phone() {
    let (config, mut compiled) = sample();
    for window in &mut compiled.app.windows.windows {
        window.content.elements_mut(&mut |e| {
            if e.kind == "Adaptive" {
                e.children.retain(|node| !matches!(node, Node::Element(layout) if layout.keyword("size") == Some("watch")));
            }
        });
    }
    let ctx = context(&config, &compiled, "wearos");
    assert!(
        ComposeWearos
            .check(&compiled.app, &ctx.target)
            .iter()
            .any(|e| e.message.contains("explicit `Layout(size: watch)`"))
    );
}

#[test]
fn feature_contracts_require_watch_entries_not_phone_coverage() {
    let (mut config, _) = sample();
    let root = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let dir = root.join(format!("bern-test/wear-contracts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lib.rs");
    config.ui_sources[0].path = path.clone();
    let ui = r#"
use bern::prelude::*;
use wear_demo_protocol::watch_demo;
#[bern::features]
pub enum CountFeature { #[commands(add)] Add }
#[component]
pub fn app(app: watch_demo::Client, windows: AppWindows) -> impl View {
    ui! { WindowSet(handle: AppWindows, primary: main) {
        Window(id: main, title: tr! { "en" => "Count", "zh-Hans" => "计数" }, initial: open) { count(app: app) }
    } }
}
#[component(features = CountFeature)]
pub fn count(app: watch_demo::Client) -> impl View {
    ui! { Adaptive {
        Layout(size: compact) { Button(id: add, label: tr! { "en" => "Add", "zh-Hans" => "加一" }, feature: CountFeature::Add) => app.add() }
        Layout(size: expanded) { Button(id: add, label: tr! { "en" => "Add", "zh-Hans" => "加一" }, feature: CountFeature::Add) => app.add() }
        Layout(size: watch) { Spacer }
    } }
}
"#;
    std::fs::write(&path, ui).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    let phone = bern_target_api::feature_report(&compiled.app, "android");
    assert!(!phone.diagnostics.iter().any(|d| d.is_error()));
    let watch = bern_target_api::feature_report(&compiled.app, "wearos");
    assert!(
        watch.diagnostics.iter().any(|d| d.code == Some("B201")),
        "{watch:?}"
    );
    assert_eq!(watch.rows.len(), 1);
    assert_eq!(watch.rows[0].2, bern_ir::LayoutSize::Watch);
    let fixed = ui.replace("Layout(size: watch) { Spacer }", "Layout(size: watch) { Button(id: add, label: tr! { \"en\" => \"Add\", \"zh-Hans\" => \"加一\" }, feature: CountFeature::Add) => app.add() }");
    std::fs::write(&path, fixed).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    assert!(
        !bern_target_api::feature_report(&compiled.app, "wearos")
            .diagnostics
            .iter()
            .any(|d| d.is_error())
    );
}

#[test]
fn watch_dimensions_are_not_compile_time_constants() {
    let (_, mut compiled) = sample();
    let window = &mut compiled.app.windows.windows[0];
    let field = |name: &str| bern_ir::Expr {
        kind: ExprKind::Binding {
            binding: Binding::Window { field: name.into() },
        },
        ty: bern_protocol::Type::Bool,
    };
    window.content = Node::If(bern_ir::IfNode {
        branches: vec![(field("short"), vec![window.content.clone()])],
        otherwise: vec![],
    });
    let lowered = compiled.app.on_watch();
    assert!(
        matches!(&lowered.windows.windows[0].content, Node::If(branch) if matches!(&branch.branches[0].0.kind, ExprKind::Binding { binding: Binding::Window { field } } if field == "short"))
    );
}

#[test]
fn every_other_backend_discards_watch_only_content_before_checking() {
    let (_, compiled) = sample();
    for platform in [
        "macos", "ios", "tvos", "linux", "windows", "browser", "android",
    ] {
        let config = TargetConfig {
            name: platform.into(),
            platform: platform.into(),
            ..Default::default()
        };
        let app = bern_target_api::svg::prepare(&compiled.app, &config);
        app.walk(&mut |node, _| {
            assert!(!matches!(node, Node::Element(e) if e.kind == "Layout" && e.keyword("size") == Some("watch")), "{platform}");
        });
    }
}

#[test]
fn watch_release_defaults_to_arm64_and_32bit_and_honors_min_sdk_and_abi_overrides() {
    let (config, compiled) = sample();

    // Default release omits `abis`: ships both arm64-v8a and armeabi-v7a at minSdk 26.
    let mut release = context(&config, &compiled, "wearos");
    release.release = true;
    let files = ComposeWearos.generate(&compiled.app, &release).unwrap();
    let build = text(&files, "app/build.gradle.kts");
    assert!(build.contains("minSdk = 26"), "{build}");
    assert!(
        build.contains("include(\"arm64-v8a\", \"armeabi-v7a\")"),
        "{build}"
    );
    assert!(!build.contains("abiFilters"), "{build}");
    let metadata: serde_json::Value = serde_json::from_str(text(&files, "build.json")).unwrap();
    assert_eq!(metadata["minimum_sdk"], 26);
    assert_eq!(
        metadata["abis"],
        serde_json::json!(["arm64-v8a", "armeabi-v7a"])
    );

    // Explicit `android.min_sdk` and single ABI override (for example Google Play at API 30
    // or a 32-bit-only build).
    let mut play = release.clone();
    play.target.abis = vec!["arm64-v8a".into()];
    play.target.android = Some(AndroidOptions {
        min_sdk: Some(30),
        ..Default::default()
    });
    let files = ComposeWearos.generate(&compiled.app, &play).unwrap();
    let build = text(&files, "app/build.gradle.kts");
    assert!(build.contains("minSdk = 30"), "{build}");
    assert!(build.contains("abiFilters += \"arm64-v8a\""), "{build}");
    assert!(!build.contains("splits {"), "{build}");
    let metadata: serde_json::Value = serde_json::from_str(text(&files, "build.json")).unwrap();
    assert_eq!(metadata["minimum_sdk"], 30);
    assert_eq!(metadata["abis"], serde_json::json!(["arm64-v8a"]));

    let mut armv7_debug = context(&config, &compiled, "wearos");
    armv7_debug.target.abis = vec!["armeabi-v7a".into()];
    let files = ComposeWearos.generate(&compiled.app, &armv7_debug).unwrap();
    let build = text(&files, "app/build.gradle.kts");
    assert!(build.contains("abiFilters += \"armeabi-v7a\""), "{build}");
    let metadata: serde_json::Value = serde_json::from_str(text(&files, "build.json")).unwrap();
    assert_eq!(metadata["abis"], serde_json::json!(["armeabi-v7a"]));

    // Source eject carries both the configured `min_sdk` and the release ABIs.
    let mut eject_ctx = release.clone();
    eject_ctx.target.android = Some(AndroidOptions {
        min_sdk: Some(30),
        ..Default::default()
    });
    let ejected = ComposeWearos
        .eject(
            &compiled.app,
            &eject_ctx,
            &EjectOptions {
                engine: EjectEngine::Source,
                with_cli: false,
                release: true,
                staging_dir: eject_ctx.output_dir.join("staging"),
            },
        )
        .unwrap();
    let ejected_build = text(&ejected.files, "app/build.gradle.kts");
    assert!(ejected_build.contains("minSdk = 30"), "{ejected_build}");
    assert!(
        ejected_build.contains(
            "Triple(\"armeabi-v7a\", \"armv7-linux-androideabi\", \"armv7a-linux-androideabi\")"
        ),
        "{ejected_build}"
    );
    assert!(ejected_build.contains("(30 downTo 21)"), "{ejected_build}");

    // Phone Android target also honors `android.min_sdk` and keeps arm64-v8a default.
    let mut phone = context(&config, &compiled, "android");
    phone.release = true;
    phone.target.android = Some(AndroidOptions {
        min_sdk: Some(29),
        ..Default::default()
    });
    let files = ComposeAndroid.generate(&compiled.app, &phone).unwrap();
    let build = text(&files, "app/build.gradle.kts");
    assert!(build.contains("minSdk = 29"), "{build}");
    assert!(build.contains("abiFilters += \"arm64-v8a\""), "{build}");
    let metadata: serde_json::Value = serde_json::from_str(text(&files, "build.json")).unwrap();
    assert_eq!(metadata["minimum_sdk"], 29);
    assert_eq!(metadata["abis"], serde_json::json!(["arm64-v8a"]));
}
