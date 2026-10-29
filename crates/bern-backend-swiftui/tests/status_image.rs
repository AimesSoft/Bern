//! `StatusItem(image:)` on macOS: the build draws the SVG's shape into the
//! bundle at one and two pixels per point, and the menu bar shows it as a
//! template image. The Swift is type-checked against the SDK.

use bern_backend_swiftui::{MINIMUM_IOS, MINIMUM_MACOS, MINIMUM_TVOS, SwiftUiMacos};
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "tray", version = 1)]
pub mod tray {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub on: bool,
    }

    #[commands]
    pub trait Commands {
        fn toggle(&mut self) -> Result<(), CommandError>;
    }
}
"#;

const UI: &str = r#"
use bern::prelude::*;

#[component]
pub fn app(app: tray::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Tray" }, initial: open) {
                Text(content: tr! { "en" => "Tray" })
            }
            StatusItem(image: "art/tray.svg", label: tr! { "en" => "Tray" }) {
                CommandItem(id: toggle, label: tr! { "en" => "On" }, checked: app.on) => app.toggle()
            }
        }
    }
}
"#;

fn project(dir: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("tray-protocol", PROTOCOL),
        ("tray-ui", UI),
        ("tray-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    std::fs::create_dir_all(dir.join("art")).unwrap();
    std::fs::write(
        dir.join("art/tray.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7" fill="#08f"/></svg>"##,
    )
    .unwrap();
    std::fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "tray"
bundle_id = "dev.bern.tray"
version = "0.1.0"
name = { "en" = "Tray" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "tray-protocol"
source = "tray-protocol/src/lib.rs"

[engine]
package = "tray-core"
type = "tray_core::Engine"

[ui]
package = "tray-ui"
sources = ["tray-ui/src/lib.rs"]
root = "app"

[resources]
paths = ["art/tray.svg"]

[targets.macos]
platform = "macos"
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

fn context(
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
        .unwrap()
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

#[test]
fn the_menu_bar_shows_the_status_items_own_picture_as_a_template() {
    let root = workspace();
    let (config, compiled) = project(&root.join("target/bern-test/status-image-project"));
    let app = compiled.app.on_platform("macos");
    let output = root.join("target/bern-test/status-image-macos");
    let _ = std::fs::remove_dir_all(&output);
    let context = context(&config, &compiled, "macos", &output);
    let problems = SwiftUiMacos.check(&app, &context.target);
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let files = SwiftUiMacos.generate(&app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    // Black on clear at 18 and 36 pixels, whatever the SVG's colour.
    for (name, side) in [("BernStatusIcon.png", 18), ("BernStatusIcon@2x.png", 36)] {
        let file = files
            .iter()
            .find(|file| {
                file.path
                    .ends_with(Path::new("Contents/Resources").join(name))
            })
            .unwrap_or_else(|| panic!("no {name}"));
        assert_eq!(
            bern_target_api::icon::png_size(&file.contents),
            Some((side, side))
        );
    }
    // The build keeps them when it puts the declared resources in place.
    let plan = SwiftUiMacos.plan(&app, &context).unwrap();
    let keep = plan
        .steps
        .iter()
        .find_map(|step| match step {
            bern_target_api::BuildStep::Mirror { keep, .. } => Some(keep),
            _ => None,
        })
        .expect("a step that puts the resources in place");
    for name in ["BernStatusIcon.png", "BernStatusIcon@2x.png"] {
        assert!(keep.contains(&PathBuf::from(name)), "{keep:?}");
    }
    let swift = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    assert!(
        swift.contains("SwiftUI.Image(nsImage: BernStatusArea.picture).accessibilityLabel("),
        "{swift}"
    );
    typecheck("macos");
}

/// Type-check the generated Swift of target `name` against its SDK, when
/// the SDK is installed.
fn typecheck(name: &str) {
    let root = workspace();
    let architecture = if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    };
    let (sdk, target, platform) = match name {
        "macos" => (
            "macosx",
            format!("{architecture}-apple-macos{MINIMUM_MACOS}"),
            "BernAppKit.swift",
        ),
        "ios" => (
            "iphonesimulator",
            format!("{architecture}-apple-ios{MINIMUM_IOS}-simulator"),
            "BernUIKit.swift",
        ),
        _ => (
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
    let sources = root.join(format!("target/bern-test/status-image-{name}/Sources"));
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
        "swiftc rejected the generated {name} code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
