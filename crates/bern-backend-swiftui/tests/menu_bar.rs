//! The menu bar, the status item, shortcuts that follow state and
//! `ShortcutInput`: macOS puts the `MenuBar` in its menu bar and the
//! `StatusItem` at its right, iOS and tvOS show neither; shortcuts read
//! their keys from state on all three. The Swift is type-checked against
//! each system's SDK.

use bern_backend_swiftui::{
    MINIMUM_IOS, MINIMUM_MACOS, MINIMUM_TVOS, SwiftUiIos, SwiftUiMacos, SwiftUiTvos,
};
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
#[bern::protocol(namespace = "keys", version = 1)]
pub mod keys {
    use bern::media::MediaSource;
    use bern::prelude::*;

    #[value]
    pub struct Shortcuts {
        pub open: String,
        pub back: String,
        pub play: String,
    }

    #[state]
    pub struct State {
        pub keys: Shortcuts,
        pub playing: bool,
        pub rate: i64,
    }

    #[commands]
    pub trait Commands {
        fn open(&mut self, source: MediaSource) -> Result<(), CommandError>;
        fn back(&mut self) -> Result<(), CommandError>;
        fn toggle(&mut self) -> Result<(), CommandError>;
        fn set_rate(&mut self, rate: i64) -> Result<(), CommandError>;
        fn set_open_key(&mut self, keys: String) -> Result<(), CommandError>;
    }
}
"#;

const UI: &str = r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {
    pub file: MediaSource,
}

#[component]
pub fn app(app: keys::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Keys" }, initial: open) {
                page(app: app)
            }
            Window(id: settings, title: tr! { "en" => "Settings" }) {
                Form {
                    Section {
                        ShortcutInput(id: open_key, label: tr! { "en" => "Open" }, current: app.keys.open) => app.set_open_key(app.keys.open)
                        ShortcutInput(id: back_key, label: tr! { "en" => "Back" }, label_hidden: true, current: app.keys.back, prompt: tr! { "en" => "Type…" }, empty: tr! { "en" => "Off" }, enabled: !app.playing) => app.set_open_key(app.keys.back)
                    }
                }
            }
            MenuBar {
                CommandMenu(role: file) {
                    CommandItem(id: open_file, label: tr! { "en" => "Open…" }, shortcut: app.keys.open, pick: draft.file, files: "video") => app.open(draft.file)
                }
                CommandMenu(label: tr! { "en" => "Playback" }) {
                    CommandItem(id: play, label: tr! { "en" => "Play" }, shortcut: app.keys.play, enabled: app.playing) => app.toggle()
                    Divider
                    CommandMenu(label: tr! { "en" => "Rate" }) {
                        CommandItem(id: rate_one, label: tr! { "en" => "1×" }, checked: app.rate == 1) => app.set_rate(1)
                        CommandItem(id: rate_two, label: tr! { "en" => "2×" }, checked: app.rate == 2, shortcut: "cmd+2") => app.set_rate(2)
                    }
                    if app.playing {
                        CommandItem(id: stop, label: tr! { "en" => "Stop" }) => app.toggle()
                    }
                }
                CommandMenu(role: app) {
                    CommandItem(id: preferences, label: tr! { "en" => "Settings…" }, shortcut: "cmd+,") => windows.settings.open()
                }
                CommandMenu(role: about) {
                    CommandItem(id: about, label: tr! { "en" => "About Keys" }) => windows.settings.open()
                }
                CommandMenu(role: help) {
                    CommandItem(id: keys_help, label: tr! { "en" => "Keys Help" }, shortcut: "cmd+?") => app.back()
                }
            }
            StatusItem(icon: "play", label: tr! { "en" => "Keys" }, login_item: true) {
                CommandItem(id: status_play, label: tr! { "en" => "Playing" }, checked: app.playing) => app.toggle()
                Divider
                CommandMenu(label: tr! { "en" => "Rate" }) {
                    CommandItem(id: status_rate, label: tr! { "en" => "2×" }, checked: app.rate == 2) => app.set_rate(2)
                }
                if app.playing {
                    CommandItem(id: status_back, label: tr! { "en" => "Back" }) => app.back()
                }
            }
        }
    }
}

#[component]
pub fn page(app: keys::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Column {
            Button(id: open, label: tr! { "en" => "Open" }, shortcut: app.keys.open) => app.open(draft.file)
            Shortcut(key: app.keys.back, enabled: app.playing) => app.back()
            Shortcut(key: "space") => app.toggle()
            if window.menu_bar {
                Text(content: tr! { "en" => "In the menu bar" })
            }
        }
    }
}
"#;

fn project(dir: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("keys-protocol", PROTOCOL),
        ("keys-ui", UI),
        ("keys-core", ""),
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
id = "keys"
bundle_id = "dev.bern.keys"
version = "0.1.0"
name = { "en" = "Keys" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "keys-protocol"
source = "keys-protocol/src/lib.rs"

[engine]
package = "keys-core"
type = "keys_core::Engine"

[ui]
package = "keys-ui"
sources = ["keys-ui/src/lib.rs"]
root = "app"

[targets.macos]
platform = "macos"

[targets.ios]
platform = "ios"

[targets.tvos]
platform = "tvos"
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

/// Generate for target `name` into `target/bern-test/menu-bar-{name}`,
/// the application as that platform shows it; the generated Swift.
fn generate(name: &str, target: &dyn Target) -> String {
    let root = workspace();
    let (config, compiled) =
        project(&root.join(format!("target/bern-test/menu-bar-project-{name}")));
    let platform = &config
        .targets
        .iter()
        .find(|target| target.name == name)
        .unwrap()
        .platform;
    let app = compiled.app.on_platform(platform);
    let app = if name == "tvos" {
        app.on_television()
    } else {
        app
    };
    let output = root.join(format!("target/bern-test/menu-bar-{name}"));
    let _ = std::fs::remove_dir_all(&output);
    let context = context(&config, &compiled, name, &output);
    let problems = target.check(&app, &context.target);
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{name}: {problems:?}"
    );
    for file in target.generate(&app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap()
}

#[test]
fn the_menu_bar_is_the_macos_commands_and_shortcuts_follow_state() {
    let swift = generate("macos", &SwiftUiMacos);
    for expected in [
        "struct BernMenuBar: Commands {",
        ".commands { BernMenuBar() }",
        // The file menu's items stand where new documents would.
        "SwiftUI.CommandGroup(replacing: .newItem) { BernMenu0() }",
        "SwiftUI.CommandGroup(after: .appSettings) { BernMenu2() }",
        // The About item and the help item run the application's own.
        "SwiftUI.CommandGroup(replacing: .appInfo) { BernMenu3() }",
        "SwiftUI.CommandGroup(replacing: .help) { BernMenu4() }",
        "BernMenuItems.register(\"menu_bar.about\"",
        "SwiftUI.CommandMenu(SwiftUI.Text(verbatim: BernText.string(\"app.0.2.1.label\"))) { BernMenu1() }",
        // The items are views, which follow the state they read.
        "struct BernMenu1: View {",
        // A submenu, a checked item and a literal shortcut.
        "SwiftUI.Menu {",
        "SwiftUI.Toggle(isOn: SwiftUI.Binding(get: { (bernEngine.rate) == (Int64(1)) }",
        ".keyboardShortcut(SwiftUI.KeyboardShortcut(SwiftUI.KeyEquivalent(Character(\"2\")), modifiers: [.command]))",
        // The item's file goes to the primary window's draft.
        "BernFiles.pick(\"video\"",
        "return BernHost.shared.windows[\"main\"] ?? closed",
        "BernMenuItems.register(\"menu_bar.open_file\"",
        // Shortcuts that follow state, in the window and in the menu.
        ".keyboardShortcut(BernShortcuts.keyboard(bernEngine.keys.`open`))",
        "BernKeyShortcut(shortcut: BernShortcuts.keyboard(bernEngine.keys.back), enabled: bernEngine.playing)",
        "BernShortcutInput(current: bernEngine.keys.`open`",
        "prompt: BernText.string(\"bern.shortcut.prompt\"), empty: BernText.string(\"bern.shortcut.none\")",
        // `window.menu_bar` holds on macOS: its branch stays, unconditional.
        "SwiftUI.Text(verbatim: BernText.string(\"page.0.3.if0.0.content\"))",
        // The status item: a symbol the menu bar draws as a template, a
        // menu that opens the primary window, the application's items,
        // “Open at Login” and Quit; without windows the application stays
        // in the menu bar.
        "SwiftUI.MenuBarExtra {",
        "SwiftUI.Image(systemName: \"play\").accessibilityLabel(",
        "struct BernStatusMenu: View {",
        "SwiftUI.Button(BernText.format(\"bern.window.open\", [BernText.string(",
        "BernMenuItems.register(\"status_item.status_play\"",
        "{ BernStatusArea.open(\"",
        "BernLoginItemToggle()",
        "SwiftUI.Button(BernText.string(\"bern.app.quit\")) { NSApplication.shared.terminate(nil) }",
        "BernStatusArea.shown = true",
    ] {
        assert!(
            swift.contains(expected),
            "macOS lacks `{expected}`:\n{swift}"
        );
    }
    typecheck("macos");
}

#[test]
fn ios_and_tvos_show_no_menu_bar_or_status_item() {
    for (name, target) in [("ios", &SwiftUiIos as &dyn Target), ("tvos", &SwiftUiTvos)] {
        let swift = generate(name, target);
        assert!(!swift.contains("BernMenuBar"), "{name}:\n{swift}");
        assert!(!swift.contains("MenuBarExtra"), "{name}:\n{swift}");
        assert!(!swift.contains("BernStatusMenu"), "{name}:\n{swift}");
        assert!(
            !swift.contains("page.0.3.if0.0.content"),
            "{name}:\n{swift}"
        );
        let expected = if name == "tvos" {
            "BernKeyShortcut(remote: BernShortcuts.remote(bernEngine.keys.back), enabled: bernEngine.playing)"
        } else {
            "BernKeyShortcut(shortcut: BernShortcuts.keyboard(bernEngine.keys.back), enabled: bernEngine.playing)"
        };
        assert!(
            swift.contains(expected),
            "{name} lacks `{expected}`:\n{swift}"
        );
        typecheck(name);
    }
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
    let sources = root.join(format!("target/bern-test/menu-bar-{name}/Sources"));
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
