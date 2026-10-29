//! A macOS settings window takes its content's height: its content is
//! not inside the geometry reader other windows lay their content out in,
//! which has no height of its own and left the window empty.

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
#[bern::protocol(namespace = "prefs", version = 1)]
pub mod prefs {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub on: bool,
    }

    #[commands]
    pub trait Commands {
        fn set_on(&mut self, on: bool) -> Result<(), CommandError>;
    }
}
"#;

const UI: &str = r#"
use bern::prelude::*;

#[component]
pub fn app(app: prefs::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Main" }, initial: open) {
                Text(content: tr! { "en" => "Main" })
            }
            Window(id: settings, title: tr! { "en" => "Settings" }, role: settings, width: 560) {
                Form {
                    Section(title: tr! { "en" => "General" }) {
                        Switch(id: on, label: tr! { "en" => "On" }, checked: app.on) => app.set_on(app.on)
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
        ("prefs-protocol", PROTOCOL),
        ("prefs-ui", UI),
        ("prefs-core", ""),
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
id = "prefs"
bundle_id = "dev.bern.prefs"
version = "0.1.0"
name = { "en" = "Prefs" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "prefs-protocol"
source = "prefs-protocol/src/lib.rs"

[engine]
package = "prefs-core"
type = "prefs_core::Engine"

[ui]
package = "prefs-ui"
sources = ["prefs-ui/src/lib.rs"]
root = "app"

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
fn a_macos_settings_window_takes_its_contents_height() {
    let root = workspace();
    let (config, compiled) = project(&root.join("target/bern-test/settings-window-project"));
    let app = compiled.app.on_platform("macos");
    let output = root.join("target/bern-test/settings-window-macos");
    let _ = std::fs::remove_dir_all(&output);
    let context = context(&config, &compiled, "macos", &output);
    for file in SwiftUiMacos.generate(&app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let swift = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    let settings = &swift[swift
        .find("/// Root of the `settings` window.")
        .expect("the settings root")..];
    let settings = &settings[..settings.find("\n}\n").unwrap()];
    assert!(
        settings.contains("BernSearchPaletteHost(fits: true) {"),
        "{settings}"
    );
    assert!(
        settings.contains(".fixedSize(horizontal: false, vertical: true)"),
        "{settings}"
    );
    // Other windows fill themselves, their content at the top leading corner.
    let main = &swift[swift
        .find("/// Root of the `main` window.")
        .expect("the main root")..];
    let main = &main[..main.find("\n}\n").unwrap()];
    assert!(main.contains("BernSearchPaletteHost {"), "{main}");
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
    let sources = root.join(format!("target/bern-test/settings-window-{name}/Sources"));
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
