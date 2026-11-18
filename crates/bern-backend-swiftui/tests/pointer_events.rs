//! Pointer events, the slider's `hover` and player state bindings: the same
//! declarations generate for macOS, iOS, tvOS and watchOS and the Swift
//! compiles against each SDK. Wheels and hover are of macOS and iOS only;
//! on the TV and the watch the elements show without them.
use bern_backend_swiftui::{
    MINIMUM_MACOS, MINIMUM_TVOS, MINIMUM_WATCHOS, SwiftUiIos, SwiftUiMacos, SwiftUiTvos,
    SwiftUiWatchos,
};
use bern_ir::Application;
use bern_target_api::{BuildContext, Framework, GeneratedFile, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

mod support;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "points", version = 1)]
pub mod points {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub title: String,
    }

    #[commands]
    pub trait Commands {
        fn wave(&mut self) -> Result<(), CommandError>;
    }
}
"#;

const UI: &str = r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft { pub hover: Option<f64> }

/// Media players owned by the application.
#[bern::players]
pub struct Players { pub preview: Player }

#[component]
pub fn app(app: points::Client, windows: AppWindows, players: Players, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Pointer" }, initial: open) {
                Column {
                    Video(id: preview, player: players.preview, controls: none, label: tr! { "en" => "Preview" }) {
                        DoubleTap => players.preview.pause()
                        ContextMenu {
                            MenuItem(id: replay, label: tr! { "en" => "Replay" }) => players.preview.seek(0)
                        }
                        WheelUp => players.preview.set_volume(1.0)
                        WheelDown => players.preview.set_volume(0.0)
                    }
                    Stack {
                        Text(content: tr! { "en" => "Area" })
                        DoubleTap => app.wave()
                        ContextMenu {
                            MenuItem(id: again, label: tr! { "en" => "Again" }) => app.wave()
                        }
                        WheelUp => app.wave()
                        WheelDown => app.wave()
                    }
                    Button(id: nudge, label: tr! { "en" => "Nudge" }) {
                        WheelUp => app.wave()
                        WheelDown => app.wave()
                    } => app.wave()
                    Slider(id: seek, label: tr! { "en" => "Position" }, current: players.preview.progress, hover: draft.hover, min: 0, max: 1) => players.preview.play()
                    if players.preview.playing {
                        Text(content: tr! { "en" => "{at} of {length}"; at: players.preview.position_ms, length: players.preview.duration })
                    }
                }
            }
        }
    }
}
"#;

fn project(dir: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("points-protocol", PROTOCOL),
        ("points-ui", UI),
        ("points-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    // No `frontends`: the front end of iOS 16 and that of iOS 17 and later.
    std::fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "points"
bundle_id = "dev.bern.points"
version = "0.1.0"
name = { "en" = "Points" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "points-protocol"
source = "points-protocol/src/lib.rs"

[engine]
package = "points-core"
type = "points_core::Engine"

[ui]
package = "points-ui"
sources = ["points-ui/src/lib.rs"]
root = "app"

[targets.macos]
platform = "macos"

[targets.ios]
platform = "ios"

[targets.tvos]
platform = "tvos"

[targets.watchos]
platform = "watchos"
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

/// The build context of target `name`, writing to `output`.
fn build_context(
    config: &bern_compiler::Config,
    compiled: &bern_compiler::Compiled,
    name: &str,
    output: &Path,
) -> BuildContext {
    let root = workspace();
    let target = config
        .targets
        .iter()
        .find(|target| target.name == name)
        .unwrap_or_else(|| panic!("the project declares target `{name}`"))
        .clone();
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.to_path_buf(),
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
    }
}

fn write(output: &Path, files: &[GeneratedFile]) {
    let _ = std::fs::remove_dir_all(output);
    for file in files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
}

/// The generated `Sources/BernGenerated.swift`.
fn generated(output: &Path) -> String {
    std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap()
}

/// What every front end makes of the declarations.
fn assert_pointer_events(source: &str, wheel: bool) {
    // A double tap of the video and of the stack, single presses passing.
    assert!(
        source.matches("BernPointerEvents(doubleTap: {").count() >= 2,
        "{source}"
    );
    // A button keeps its own taps and takes only the wheel where supported.
    if wheel {
        assert!(
            source.contains("BernPointerEvents(doubleTap: nil, wheelUp:"),
            "{source}"
        );
        assert!(source.contains("shaped: false"), "{source}");
    } else {
        assert!(
            !source.contains("BernPointerEvents(doubleTap: nil"),
            "{source}"
        );
    }
    // The menu of an element is the system's menu.
    assert!(source.matches(".contextMenu {").count() >= 2, "{source}");
    // Player state read in a condition, in a text and in the slider.
    assert!(
        source.matches("BernPlayerReading(").count() >= 4,
        "{source}"
    );
    assert!(
        source.contains(r#"BernPlayerReading(BernMedia.shared.players["preview"]).playing"#),
        "{source}"
    );
    // The events are not views among the content.
    for kind in ["DoubleTap", "WheelUp", "WheelDown"] {
        assert!(!source.contains(&format!("unsupported {kind}")), "{source}");
    }
}

/// Compile the generated Swift as far as the compiler's own checks go,
/// when the SDK is installed.
fn typecheck(root: &Path, output: &Path, sdk: &str, triple: String, files: &[&str]) {
    let Some(sdk_path) = Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    else {
        eprintln!("no {sdk} SDK; skipping the check");
        return;
    };
    let sources = output.join("Sources");
    let mut command = Command::new("xcrun");
    command
        .args([
            "--sdk",
            sdk,
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
            "-sdk",
        ])
        .arg(&sdk_path)
        .arg("-target")
        .arg(&triple)
        .arg("-module-cache-path")
        .arg(support::cache::module_cache(root))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"));
    for file in files {
        command.arg(sources.join(file));
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated code for {triple}:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn architecture() -> &'static str {
    if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    }
}

#[test]
fn pointer_events_compile_for_both_ios_frontends() {
    let root = workspace();
    let build_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let (config, compiled) = project(&build_root.join("bern-test/pointer-events-project"));
    let context = build_context(
        &config,
        &compiled,
        "ios",
        &build_root.join("bern-test/pointer-events-ios"),
    );
    assert_eq!(context.target.frontends, ["swiftui", "swiftui-16"]);
    let app = compiled.app.on_platform(&context.target.platform);
    let problems = SwiftUiIos.check(&app, &context.target);
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    write(
        &context.output_dir,
        &SwiftUiIos.generate(&app, &context).unwrap(),
    );
    assert_pointer_events(&generated(&context.output_dir), true);
    assert_pointer_events(
        &std::fs::read_to_string(context.output_dir.join("Sources17/BernGenerated.swift")).unwrap(),
        true,
    );
    for source in [
        generated(&context.output_dir),
        std::fs::read_to_string(context.output_dir.join("Sources17/BernGenerated.swift")).unwrap(),
    ] {
        assert!(
            source.contains(".bernSliderHover(SwiftUI.Binding("),
            "{source}"
        );
        assert!(source.contains("wheelUp: {"), "{source}");
        assert!(source.contains("wheelDown: {"), "{source}");
    }
    support::check_ios(&root, &context.output_dir);
}

#[test]
fn pointer_events_type_check_for_macos() {
    let root = workspace();
    let build_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let (config, compiled) = project(&build_root.join("bern-test/pointer-events-project"));
    let context = build_context(
        &config,
        &compiled,
        "macos",
        &build_root.join("bern-test/pointer-events-macos"),
    );
    let app = compiled.app.on_platform(&context.target.platform);
    let problems = SwiftUiMacos.check(&app, &context.target);
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    write(
        &context.output_dir,
        &SwiftUiMacos.generate(&app, &context).unwrap(),
    );
    let source = generated(&context.output_dir);
    assert_pointer_events(&source, true);
    assert!(
        source.contains(".bernSliderHover(SwiftUI.Binding("),
        "{source}"
    );
    assert!(source.contains("wheelUp: {"), "{source}");
    assert!(source.contains("wheelDown: {"), "{source}");
    typecheck(
        &root,
        &context.output_dir,
        "macosx",
        format!("{}-apple-macos{MINIMUM_MACOS}", architecture()),
        &[
            "BernRuntime.swift",
            "BernMedia.swift",
            "BernStyle.swift",
            "BernAppKit.swift",
            "BernGenerated.swift",
        ],
    );
}

#[test]
fn pointer_events_type_check_for_tvos() {
    let root = workspace();
    let build_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let (config, compiled) = project(&build_root.join("bern-test/pointer-events-project"));
    let context = build_context(
        &config,
        &compiled,
        "tvos",
        &build_root.join("bern-test/pointer-events-tvos"),
    );
    let app: Application = compiled.app.on_platform(&context.target.platform);
    let problems = SwiftUiTvos.check(&app, &context.target);
    // The wheel and the slider's hover are only warned of: the elements
    // show without them.
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    write(
        &context.output_dir,
        &SwiftUiTvos.generate(&app, &context).unwrap(),
    );
    let source = generated(&context.output_dir);
    assert_pointer_events(&source, false);
    assert!(source.contains("wheelUp: nil"), "{source}");
    assert!(source.contains("wheelDown: nil"), "{source}");
    assert!(!source.contains(".bernSliderHover("), "{source}");
    typecheck(
        &root,
        &context.output_dir,
        "appletvsimulator",
        format!("{}-apple-tvos{MINIMUM_TVOS}-simulator", architecture()),
        &[
            "BernRuntime.swift",
            "BernMedia.swift",
            "BernStyle.swift",
            "BernUIKit.swift",
            "BernGenerated.swift",
        ],
    );
}

#[test]
fn pointer_events_type_check_for_watchos() {
    let root = workspace();
    let build_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let (config, compiled) = project(&build_root.join("bern-test/pointer-events-project"));
    let context = build_context(
        &config,
        &compiled,
        "watchos",
        &build_root.join("bern-test/pointer-events-watchos"),
    );
    let app: Application = compiled.app.on_platform(&context.target.platform);
    let problems = SwiftUiWatchos.check(&app, &context.target);
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    write(
        &context.output_dir,
        &SwiftUiWatchos.generate(&app, &context).unwrap(),
    );
    let source = generated(&context.output_dir);
    assert!(source.contains("BernWatchContextMenu("), "{source}");
    assert!(source.contains("wheelUp: nil"), "{source}");
    assert!(!source.contains(".bernSliderHover("), "{source}");
    assert!(
        source.contains("BernPointerEvents(doubleTap: {"),
        "{source}"
    );
    assert!(
        source.matches("BernPlayerReading(").count() >= 4,
        "{source}"
    );
    typecheck(
        &root,
        &context.output_dir,
        "watchsimulator",
        format!(
            "{}-apple-watchos{MINIMUM_WATCHOS}-simulator",
            architecture()
        ),
        &[
            "BernRuntime.swift",
            "BernMedia.swift",
            "BernStyle.swift",
            "BernWatchKit.swift",
            "BernGenerated.swift",
        ],
    );
}
