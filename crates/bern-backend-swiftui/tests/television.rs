//! The tvOS target: the showcase builds for the Apple TV simulator, and an
//! application with a television layout, shortcuts and a default focus
//! keeps on tvOS only what a television shows. The Swift is type-checked
//! against the tvOS simulator SDK, and the same application against macOS.

use bern_backend_swiftui::{MINIMUM_MACOS, MINIMUM_TVOS, SwiftUiMacos, SwiftUiTvos};
use bern_target_api::{
    BuildContext, BuildStep, Framework, GeneratedFile, Run, SimulatedSystem, Target, brand_assets,
};
use std::path::{Path, PathBuf};
use std::process::Command;

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

#[test]
fn showcase_builds_for_the_apple_tv_simulator() {
    let root = workspace();
    let (config, compiled) = compile(&root.join("examples/showcase/Bern.toml"));
    let target = config.targets.iter().find(|t| t.name == "tvos").unwrap();
    let problems = SwiftUiTvos.check(&compiled.app, target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/swiftui-tvos");
    let context = build_context(&config, &compiled, "tvos", &output);
    write(
        &output,
        &SwiftUiTvos.generate(&compiled.app, &context).unwrap(),
    );

    // A television bundle for the simulator: one scene, the brand assets
    // as the icon and the Top Shelf.
    let plist = std::fs::read_to_string(output.join("Showcase.app/Info.plist")).unwrap();
    for expected in [
        "<key>UIDeviceFamily</key>\n    <array>\n        <integer>3</integer>",
        "<string>AppleTVSimulator</string>",
        &format!("<key>MinimumOSVersion</key>\n    <string>{MINIMUM_TVOS}</string>"),
        "<key>CFBundlePrimaryIcon</key>\n        <string>App Icon</string>",
        "<key>TVTopShelfPrimaryImage</key>\n        <string>Top Shelf Image</string>",
        "<key>UIApplicationSupportsMultipleScenes</key>\n        <false/>",
    ] {
        assert!(plist.contains(expected), "{expected}\n{plist}");
    }
    // The showcase declares no tvOS icon: its icon is drawn into brand
    // assets that pass the same check as declared ones.
    let set = output.join(format!(
        "Assets.xcassets/{}.brandassets",
        brand_assets::NAME
    ));
    let listed = brand_assets::check(&set).unwrap();
    assert!(listed.len() >= 10, "{listed:?}");
    let front = std::fs::read(
        set.join("App Icon.imagestack/Front.imagestacklayer/Content.imageset/front.png"),
    )
    .unwrap_or_else(|_| {
        panic!(
            "no front layer of the small icon among {:?}",
            listed
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
        )
    });
    assert_eq!(bern_target_api::icon::png_size(&front), Some((400, 240)));

    let plan = SwiftUiTvos.plan(&compiled.app, &context).unwrap();
    let programs: Vec<String> = plan
        .steps
        .iter()
        .filter_map(|step| match step {
            BuildStep::Run { program, args, .. } if program == "xcrun" => Some(format!(
                "xcrun {}",
                args.iter()
                    .find(|a| !a.starts_with('-') && *a != "appletvsimulator")
                    .unwrap()
            )),
            BuildStep::Run { program, .. } => Some(program.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        programs,
        [
            "cargo",
            "cargo",
            "xcrun actool",
            "xcrun swiftc",
            "mv",
            "codesign"
        ]
    );
    // The engine for the Apple TV simulator, the command line tools for the
    // Mac; the icon compiled for a television.
    let mut targets = plan.steps.iter().filter_map(|step| match step {
        BuildStep::Run { program, args, .. } if program == "cargo" => args
            .windows(2)
            .find(|pair| pair[0] == "--target")
            .map(|pair| pair[1].clone()),
        _ => None,
    });
    let engine = targets.next().unwrap();
    assert!(
        engine == "aarch64-apple-tvos-sim" || engine == "x86_64-apple-tvos",
        "{engine}"
    );
    assert!(targets.next().unwrap().ends_with("-apple-darwin"));
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
        "--platform appletvsimulator",
        "--target-device tv",
        "--app-icon App Icon & Top Shelf Image",
    ] {
        assert!(actool.contains(expected), "{expected}\n{actool}");
    }
    assert!(matches!(
        &plan.run,
        Some(Run::Simulator { bundle_id, system: SimulatedSystem::Tvos, .. })
            if bundle_id == "dev.bern.showcase"
    ));

    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for expected in ["import UIKit", "BernVirtualHost(initial: [])"] {
        assert!(
            generated.contains(expected),
            "generated Swift lacks `{expected}`"
        );
    }
    for forbidden in ["AppKit", "NSApplication", ".keyboardShortcut("] {
        assert!(
            !generated.contains(forbidden),
            "generated tvOS Swift uses `{forbidden}`"
        );
    }
    typecheck(&root, &output, Apple::Tvos);
}

const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Mode {
        #[label(tr! { "en" => "Light" })]
        Light,
        #[label(tr! { "en" => "Dark" })]
        Dark,
    }

    #[state]
    pub struct State {
        pub count: i64,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}
"#;

/// A phone and desktop layout that picks files, which a television cannot,
/// and a television layout that the remote drives: Play/Pause, the
/// clickpad's left edge and Back stand for keys, and the focus starts on
/// the first button.
const UI: &str = r#"
use bern::prelude::*;
use demo_protocol::demo;

style! {
    text {
        body: font(size: 13, tv: 16, weight: regular),
    }
}

#[bern::session]
pub struct Draft {
    pub file: MediaSource,
    pub details: bool,
    pub query: String,
    pub level: f64,
    pub on: bool,
    pub mode: demo::Mode,
}

#[component]
pub fn app(app: demo::Client) -> impl View {
    ui! {
        WindowSet(handle: Windows, primary: main) {
            Window(id: main, initial: open, title: tr! { "en" => "Remote" }) {
                screen(app: app)
            }
        }
    }
}

#[component]
pub fn screen(app: demo::Client) -> impl View {
    ui! {
        Adaptive(id: shell) {
            Layout(size: compact) { phone(app: app) }
            Layout(size: expanded) { phone(app: app) }
            Layout(size: tv) { television(app: app) }
        }
    }
}

#[component]
pub fn phone(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Column {
            Button(id: open_file, label: tr! { "en" => "Open" }, pick: draft.file, files: "video") => app.add(1)
        }
    }
}

#[component]
pub fn television(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Stack {
            if window.tv {
                Column(gap: 12) {
                    Button(id: play, label: tr! { "en" => "Play" }, default_focus: true) => app.add(1)
                    IconButton(id: more, icon: "ellipsis", label: tr! { "en" => "More" }) => draft.details.set(true)
                }
            } else {
                Button(id: open_elsewhere, label: tr! { "en" => "Open" }, pick: draft.file, files: "video") => app.add(2)
            }
            Shortcut(key: "space") => app.add(3)
            Shortcut(key: "left", enabled: app.count > 0) => app.add(-10)
            if draft.details {
                Shortcut(key: "escape") => draft.details.set(false)
            }
            // A scene at the design's size: a keyboard of initials, a
            // search that suggests, a form's slider, switch, segments and
            // menu.
            ScaleBox(width: 1280, height: 720) {
                Column(gap: 8) {
                    Text(content: tr! { "en" => "Body" })
                    Button(id: key_a, label: tr! { "en" => "A" }) => draft.query.push_str("A")
                    Button(id: key_delete, label: tr! { "en" => "Delete" }) => draft.query.pop()
                    Search(id: find, value: draft.query, prompt: tr! { "en" => "Search" }, height: 52, suggest: true) {
                        Text(content: tr! { "en" => "Suggestion" })
                    }
                    Form {
                        Section(title: tr! { "en" => "Section" }) {
                            Slider(id: level, label: tr! { "en" => "Level" }, value: draft.level, min: 0, max: 1)
                            Switch(id: on, label: tr! { "en" => "On" }, value: draft.on, style: switch)
                            Picker(id: mode_segments, label: tr! { "en" => "Mode" }, value: draft.mode, style: segmented, label_hidden: true)
                            Picker(id: mode_menu, label: tr! { "en" => "Mode" }, value: draft.mode, style: menu)
                        }
                    }
                }
            }
        }
    }
}
"#;

fn remote_project() -> PathBuf {
    let dir = workspace().join("target/bern-test/remote-project");
    let _ = std::fs::remove_dir_all(&dir);
    for (package, source) in [
        ("demo-protocol", PROTOCOL),
        ("demo-ui", UI),
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
id = "remote"
bundle_id = "dev.bern.remote"
version = "0.1.0"
name = { "en" = "Remote" }

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
medium_below = 1024
expanded_below = 1440

[targets.macos]
platform = "macos"

[targets.tvos]
platform = "tvos"
"#,
    )
    .unwrap();
    dir.join("Bern.toml")
}

#[test]
fn a_television_keeps_its_layout_and_the_remote_stands_for_keys() {
    let root = workspace();
    let manifest = remote_project();
    let (config, compiled) = compile(&manifest);

    // What the phone and desktop pick is left out on a television, so the
    // check passes although tvOS cannot pick files.
    let tvos = config.targets.iter().find(|t| t.name == "tvos").unwrap();
    let problems = SwiftUiTvos.check(&compiled.app, tvos);
    assert!(problems.is_empty(), "{problems:?}");
    let television = compiled.app.on_television();
    let names: Vec<&str> = television
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    assert!(!names.contains(&"phone"), "{names:?}");
    assert!(names.contains(&"television"), "{names:?}");

    let output = root.join("target/bern-test/swiftui-tvos-remote");
    let mut context = build_context(&config, &compiled, "tvos", &output);
    context.project_dir = manifest.parent().unwrap().to_path_buf();
    write(
        &output,
        &SwiftUiTvos.generate(&compiled.app, &context).unwrap(),
    );
    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for expected in [
        // The television's text sizes, keys that type, a search that
        // suggests, and tvOS's drawn switch, segments and labelled menu.
        "let bernSceneBodySize: CGFloat = 16.0",
        ".font(.system(size: 16.0, weight: .regular))",
        "session_Draft.query += \"A\"",
        "_ = session_Draft.query.popLast()",
        "suggest: true",
        ".toggleStyle(BernFocusSwitchStyle())",
        "BernSegments(selection:",
        "SwiftUI.Spacer(minLength: 12)",
        "BernKeyShortcut(remote: .playPause, enabled: true)",
        "BernKeyShortcut(remote: .left, enabled: (bernEngine.count) > (Int64(0)))",
        "BernKeyShortcut(remote: .back, enabled: true)",
        ".bernDefaultFocus()",
    ] {
        assert!(
            generated.contains(expected),
            "generated tvOS Swift lacks `{expected}`"
        );
    }
    for forbidden in ["open_file", "open_elsewhere", "fileImporter"] {
        assert!(
            !generated.contains(forbidden),
            "generated tvOS Swift keeps `{forbidden}`, which a television never shows"
        );
    }
    typecheck(&root, &output, Apple::Tvos);

    // On the Mac the same shortcuts are keys, the television layout stays
    // in the `Adaptive` for a window that is never a television's.
    let macos = config.targets.iter().find(|t| t.name == "macos").unwrap();
    let problems = SwiftUiMacos.check(&compiled.app, macos);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/swiftui-macos-remote");
    let mut context = build_context(&config, &compiled, "macos", &output);
    context.project_dir = manifest.parent().unwrap().to_path_buf();
    write(
        &output,
        &SwiftUiMacos.generate(&compiled.app, &context).unwrap(),
    );
    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for forbidden in ["BernSegments", "BernFocusSwitchStyle", "bernSceneBodySize"] {
        assert!(
            !generated.contains(forbidden),
            "generated macOS Swift uses tvOS's `{forbidden}`"
        );
    }
    for expected in [
        // A desktop's text size.
        ".font(.system(size: 13.0, weight: .regular))",
        "BernKeyShortcut(shortcut: SwiftUI.KeyboardShortcut(.space, modifiers: []), enabled: true)",
        "BernKeyShortcut(shortcut: SwiftUI.KeyboardShortcut(.escape, modifiers: []), enabled: true)",
        ".bernDefaultFocus()",
        "open_file",
    ] {
        assert!(
            generated.contains(expected),
            "generated macOS Swift lacks `{expected}`"
        );
    }
    typecheck(&root, &output, Apple::Macos);
}

enum Apple {
    Macos,
    Tvos,
}

/// Type-check generated Swift against the SDK of `system`, when it is
/// installed.
fn typecheck(root: &Path, output: &Path, system: Apple) {
    let architecture = if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    };
    let (sdk, target, platform) = match system {
        Apple::Macos => (
            "macosx",
            format!("{architecture}-apple-macos{MINIMUM_MACOS}"),
            "BernAppKit.swift",
        ),
        Apple::Tvos => (
            "appletvsimulator",
            format!("{architecture}-apple-tvos{MINIMUM_TVOS}-simulator"),
            "BernUIKit.swift",
        ),
    };
    let Some(sdk) = Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    else {
        eprintln!("no {sdk} SDK; skipping type check");
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
        .arg(target)
        .arg("-module-cache-path")
        .arg(root.join("target/bern-swift-module-cache"))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernStyle.swift"))
        .arg(sources.join(platform))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
