//! The watchOS target: `examples/wearos` generates and type-checks against
//! the watchOS simulator SDK, `Adaptive` requires an explicit
//! `Layout(size: watch)` (`B401`), `#[bern::features]` proves `watch`
//! coverage, `window.watch` prunes non-watch branches before checking, and
//! `package()` assembles a Watch-Only `.ipa` containerplan.

use bern_backend_swiftui::{MINIMUM_WATCHOS, SwiftUiWatchos};
use bern_target_api::{
    BuildContext, BuildStep, Framework, GeneratedFile, Run, Signing, SimulatedSystem, Target,
};
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "support/cache.rs"]
mod cache;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn compile(manifest: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let config = bern_compiler::config::load(manifest).unwrap();
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

#[test]
fn watch_example_generates_and_typechecks_for_the_apple_watch_simulator() {
    let root = workspace();
    let (config, compiled) = compile(&root.join("examples/wearos/Bern.toml"));
    let target = config.targets.iter().find(|t| t.name == "watchos").unwrap();
    let problems = SwiftUiWatchos.check(&compiled.app, target);
    assert!(problems.is_empty(), "{problems:?}");

    let output = root.join("target/bern-test/swiftui-watchos");
    let context = build_context(&config, &compiled, "watchos", &output);
    write(
        &output,
        &SwiftUiWatchos.generate(&compiled.app, &context).unwrap(),
    );

    let plist = std::fs::read_to_string(output.join("WearDemo.app/Info.plist")).unwrap();
    for expected in [
        "<key>UIDeviceFamily</key>\n    <array>\n        <integer>4</integer>",
        "<string>WatchSimulator</string>",
        &format!("<key>MinimumOSVersion</key>\n    <string>{MINIMUM_WATCHOS}</string>"),
        "<key>WKApplication</key>\n    <true/>",
        "<key>WKWatchOnly</key>\n    <true/>",
        "<key>CFBundleIconName</key>\n            <string>AppIcon</string>",
    ] {
        assert!(plist.contains(expected), "{expected}\n{plist}");
    }

    let plan = SwiftUiWatchos.plan(&compiled.app, &context).unwrap();
    let actool = plan
        .steps
        .iter()
        .find_map(|step| match step {
            BuildStep::Run { program, args, .. }
                if program == "xcrun" && args.iter().any(|a| a == "actool") =>
            {
                Some(args.join(" "))
            }
            _ => None,
        })
        .unwrap();
    for expected in [
        "--platform watchsimulator",
        "--target-device watch",
        "--app-icon AppIcon",
    ] {
        assert!(actool.contains(expected), "{expected}\n{actool}");
    }
    assert!(matches!(
        &plan.run,
        Some(Run::Simulator { bundle_id, system: SimulatedSystem::Watchos, .. })
            if bundle_id == "dev.bern.wear.demo"
    ));

    let package = SwiftUiWatchos
        .package(&compiled.app, &context, &Signing::default())
        .unwrap();
    assert!(
        package
            .products
            .iter()
            .any(|p| p.file_name().unwrap() == "WearDemo-0.1.0-watchos.ipa")
    );

    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for expected in [
        "import WatchKit",
        "BernVirtualHost(initial: [])",
        "BernSheetChrome(kind: \"settings\"",
        "BernWatchPicker(",
        "BernWatchMenu(",
    ] {
        assert!(
            generated.contains(expected),
            "generated watchOS Swift lacks `{expected}`"
        );
    }
    for forbidden in [
        "AppKit",
        "NSApplication",
        ".keyboardShortcut(",
        "SwiftUI.Menu {",
    ] {
        assert!(
            !generated.contains(forbidden),
            "generated watchOS Swift uses `{forbidden}`"
        );
    }
    typecheck(&root, &output);
}

#[test]
fn watchos_requires_watch_layout_prunes_phone_branches_and_proves_features() {
    let root = workspace();
    let dir = root.join("target/bern-test/watchos-project");
    let _ = std::fs::remove_dir_all(&dir);
    let protocol = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;
    #[state]
    pub struct State {
        pub count: i64,
    }
    #[commands]
    pub trait Commands {
        fn add(&mut self) -> Result<(), CommandError>;
    }
}
"#;
    let missing_watch = r#"
use bern::prelude::*;
use demo_protocol::demo;

#[component]
pub fn app(app: demo::Client) -> impl View {
    ui! {
        WindowSet(handle: Windows, primary: main) {
            Window(id: main, initial: open, title: tr! { "en" => "Watch" }) {
                Adaptive(id: shell) {
                    Layout(size: compact) { Button(id: add_c, label: tr! { "en" => "Add" }) => app.add() }
                    Layout(size: expanded) { Button(id: add_e, label: tr! { "en" => "Add" }) => app.add() }
                }
            }
        }
    }
}
"#;
    for (package, source) in [
        ("demo-protocol", protocol),
        ("demo-ui", missing_watch),
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
    let manifest = dir.join("Bern.toml");
    std::fs::write(
        &manifest,
        r#"
[app]
id = "watch_check"
bundle_id = "dev.bern.watch.check"
version = "0.1.0"
name = { "en" = "Watch" }

[languages]
supported = ["en"]
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

[layouts]
compact_below = 600

[targets.watchos]
platform = "watchos"
"#,
    )
    .unwrap();

    // Missing `Layout(size: watch)` fails with B401.
    let (config, compiled) = compile(&manifest);
    let target = config.targets.iter().find(|t| t.name == "watchos").unwrap();
    let problems = SwiftUiWatchos.check(&compiled.app, target);
    assert!(
        problems
            .iter()
            .any(|d| d.code == Some("B401") && d.message.contains("explicit `Layout(size: watch)`")),
        "{problems:?}"
    );

    // Feature contract checks `watch` coverage too, and phone-only `pick:` is
    // pruned on watchOS.
    let with_features = r#"
use bern::prelude::*;
use demo_protocol::demo;

#[bern::features]
pub enum WatchFeature {
    #[commands(add)]
    Add,
}

#[bern::session]
pub struct Draft {
    pub picked: MediaSource,
    pub rows: Vec<i64>,
    pub inspect: bool,
}

#[bern::value]
pub struct RowItem {
    pub id: i64,
    pub title: String,
}

#[component]
pub fn app(app: demo::Client) -> impl View {
    ui! {
        WindowSet(handle: Windows, primary: main) {
            Window(id: main, initial: open, title: tr! { "en" => "Watch" }) {
                screen(app: app)
            }
        }
    }
}

#[component(features = WatchFeature)]
pub fn screen(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Adaptive(id: shell) {
            Layout(size: compact) {
                Button(id: pick_c, label: tr! { "en" => "Pick" }, pick: draft.picked, feature: WatchFeature::Add) => app.add()
            }
            Layout(size: expanded) {
                Button(id: pick_e, label: tr! { "en" => "Pick" }, pick: draft.picked, feature: WatchFeature::Add) => app.add()
            }
            Layout(size: watch) {
                Spacer
            }
        }
    }
}
"#;
    std::fs::write(dir.join("demo-ui/src/lib.rs"), with_features).unwrap();
    let (_, missing_feature) = compile(&manifest);
    let report = bern_target_api::feature_report(&missing_feature.app, "watchos");
    assert!(
        report.diagnostics.iter().any(|d| d.code == Some("B201")),
        "{report:?}"
    );
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].2, bern_ir::LayoutSize::Watch);

    let covered = with_features.replace(
        "Spacer",
        r#"Column(gap: 6) {
                    if window.watch {
                        Button(id: add_w, label: tr! { "en" => "Add" }, feature: WatchFeature::Add) {
                            Text(content: tr! { "en" => "Add" })
                            ContextMenu {
                                MenuItem(id: ctx_add, label: tr! { "en" => "Again" }, feature: WatchFeature::Add) => app.add()
                            }
                        } => app.add()
                    } else {
                        Button(id: pick_other, label: tr! { "en" => "Pick" }, pick: draft.picked, feature: WatchFeature::Add) => app.add()
                    }
                    IconButton(id: plain_round, icon: "play.fill", height: 32, prominence: plain, label: tr! { "en" => "Play" }, feature: WatchFeature::Add) => app.add()
                    PopoverButton(id: pop_list, label: tr! { "en" => "More" }, icon: "list.bullet", prominence: plain) {
                        Text(content: tr! { "en" => "Item" })
                    }
                    Form {
                        Text(content: tr! { "en" => "Heading" })
                    }
                    Modal(width: 220) {
                        Text(content: tr! { "en" => "Dialog" })
                    }
                    Inspector(presented: draft.inspect) {
                        Text(content: tr! { "en" => "Info" })
                    }
                }"#,
    );
    std::fs::write(dir.join("demo-ui/src/lib.rs"), covered).unwrap();
    let (config, compiled) = compile(&manifest);
    assert!(
        !bern_target_api::feature_report(&compiled.app, "watchos")
            .diagnostics
            .iter()
            .any(|d| d.is_error())
    );
    let target = config.targets.iter().find(|t| t.name == "watchos").unwrap();
    let problems = SwiftUiWatchos.check(&compiled.app, target);
    assert!(problems.is_empty(), "{problems:?}");

    let output = root.join("target/bern-test/swiftui-watchos-features");
    let mut context = build_context(&config, &compiled, "watchos", &output);
    context.project_dir = dir;
    write(
        &output,
        &SwiftUiWatchos.generate(&compiled.app, &context).unwrap(),
    );
    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    assert!(!generated.contains("pick_c") && !generated.contains("pick_other"));
    assert!(!generated.contains(".contextMenu {"));
    assert!(generated.contains("BernWatchContextMenu("));
    assert!(
        generated.contains(".bernCircleControl(diameter: 32.0, prominent: false, plain: true)")
    );
    assert!(generated.contains(".listRowBackground(SwiftUI.Color.clear)"));
    assert!(generated.contains(".sheet(isPresented:"));
    typecheck(&root, &output);
}

fn typecheck(root: &Path, output: &Path) {
    let architecture = if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    };
    let Some(sdk) = Command::new("xcrun")
        .args(["--sdk", "watchsimulator", "--show-sdk-path"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    else {
        eprintln!("no watchsimulator SDK; skipping type check");
        return;
    };
    let sources = output.join("Sources");
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
            "-sdk",
        ])
        .arg(sdk)
        .arg("-target")
        .arg(format!(
            "{architecture}-apple-watchos{MINIMUM_WATCHOS}-simulator"
        ))
        .arg("-module-cache-path")
        .arg(cache::module_cache(root))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernStyle.swift"))
        .arg(sources.join("BernWatchKit.swift"))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated watchOS code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
