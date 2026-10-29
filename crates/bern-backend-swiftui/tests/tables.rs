//! `Table`, `TextInput(current:)`, `paste:` and `Watch`: the system table
//! on macOS and iOS and a list of row buttons on tvOS, a field that keeps
//! the user's text while it is edited, buttons that read the clipboard,
//! which tvOS does not have, and the choice told to the engine; with them
//! a text in a status color and the login item's switch, which only macOS
//! shows. The Swift is type-checked against each system's SDK.

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
#[bern::protocol(namespace = "rows", version = 1)]
pub mod rows {
    use bern::prelude::*;

    #[value]
    pub struct Server {
        pub id: i64,
        #[content]
        pub name: String,
        pub delay: i64,
        pub active: bool,
    }

    #[state]
    pub struct State {
        pub servers: Vec<Server>,
        pub by_name: bool,
        pub descending: bool,
        pub title: String,
    }

    #[commands]
    pub trait Commands {
        fn sort_by_name(&mut self) -> Result<(), CommandError>;
        fn activate(&mut self, ids: Vec<i64>) -> Result<(), CommandError>;
        fn remove(&mut self, ids: Vec<i64>) -> Result<(), CommandError>;
        fn rename(&mut self, title: String) -> Result<(), CommandError>;
        fn import(&mut self, links: String) -> Result<(), CommandError>;
    }
}
"#;

/// The application, with the clipboard's buttons where the platform has a
/// clipboard.
fn ui(paste: bool) -> String {
    let pasting = if paste {
        r#"Button(id: import, label: tr! { "en" => "Import" }, paste: draft.links) => app.import(draft.links)"#
    } else {
        ""
    };
    let menu_paste = if paste {
        r#"MenuItem(id: import_item, label: tr! { "en" => "Import" }, paste: draft.links) => app.import(draft.links)"#
    } else {
        ""
    };
    format!(
        r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {{
    pub chosen: Vec<i64>,
    pub links: String,
}}

#[component]
pub fn app(app: rows::Client, windows: AppWindows) -> impl View {{
    ui! {{
        WindowSet(handle: AppWindows, primary: main) {{
            Window(id: main, title: tr! {{ "en" => "Rows" }}, initial: open) {{
                page(app: app)
            }}
        }}
    }}
}}

#[component]
pub fn page(app: rows::Client, #[session] draft: Draft) -> impl View {{
    ui! {{
        Column {{
            Row {{
                TextInput(id: title, label: tr! {{ "en" => "Title" }}, label_hidden: true, current: app.title) => app.rename(app.title)
                {pasting}
            }}
            Table(id: servers, label: tr! {{ "en" => "Servers" }}, items: app.servers, key: id, selection: draft.chosen) |server| {{
                TableColumn(title: tr! {{ "en" => "Name" }}, width: 200, sorted: app.by_name, descending: app.descending) {{
                    Row(gap: 6) {{
                        if server.active {{
                            Label(title: tr! {{ "en" => "Active" }}, icon: "checkmark", display: icon)
                        }}
                        Text(content: server.name, lines: 1)
                    }}
                }} => app.sort_by_name()
                TableColumn(title: tr! {{ "en" => "Delay" }}) {{
                    Text(content: tr! {{ "en" => "{{n}} ms"; n: server.delay }})
                }}
                TableMenu {{
                    MenuItem(id: use_server, label: tr! {{ "en" => "Use" }}) => app.activate(draft.chosen)
                    {menu_paste}
                    Divider
                    MenuItem(id: remove, label: tr! {{ "en" => "Remove" }}, role: destructive) => app.remove(draft.chosen)
                }}
            }} => app.activate(draft.chosen)
            Watch(value: draft.chosen) => app.activate(draft.chosen)
            Text(content: tr! {{ "en" => "Fast" }}, color: status.success)
            Switch(id: at_login, label: tr! {{ "en" => "Open at Login" }}, login_item: true)
        }}
    }}
}}
"#
    )
}

fn project(dir: &Path, paste: bool) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("rows-protocol", PROTOCOL.to_string()),
        ("rows-ui", ui(paste)),
        ("rows-core", String::new()),
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
id = "rows"
bundle_id = "dev.bern.rows"
version = "0.1.0"
name = { "en" = "Rows" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "rows-protocol"
source = "rows-protocol/src/lib.rs"

[engine]
package = "rows-core"
type = "rows_core::Engine"

[ui]
package = "rows-ui"
sources = ["rows-ui/src/lib.rs"]
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

/// Generate for target `name` into `target/bern-test/tables-{name}`; the
/// generated Swift.
fn generate(name: &str, target: &dyn Target, paste: bool) -> String {
    let root = workspace();
    let (config, compiled) = project(
        &root.join(format!("target/bern-test/tables-project-{name}")),
        paste,
    );
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
    let output = root.join(format!("target/bern-test/tables-{name}"));
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
fn macos_and_ios_show_the_system_table() {
    for (name, target) in [
        ("macos", &SwiftUiMacos as &dyn Target),
        ("ios", &SwiftUiIos),
    ] {
        let swift = generate(name, target, true);
        for expected in [
            "SwiftUI.Table(of: BernTableRow<",
            // The heading's order shows the column state says; a click
            // runs the column's action.
            ", sortOrder: SwiftUI.Binding(get: { if bernEngine.byName {",
            "SwiftUI.TableColumn(SwiftUI.Text(verbatim: ",
            ".width(min: 40, ideal: 200.0)",
            ".contextMenu(forSelectionType: Int64.self, menu: { ids in",
            ", primaryAction: { ids in ",
            "role: \"table\"",
            // The field keeps the user's text; the buttons read the
            // clipboard.
            "BernHeldText(",
            "BernClipboard.paste { text in ",
            // The engine hears of each new choice.
            "BernWatch(value: session_Draft.chosen) {",
            ".foregroundStyle(BernColor.role(\"status.success\").color)",
        ] {
            assert!(
                swift.contains(expected),
                "{name} lacks `{expected}`:\n{swift}"
            );
        }
        // Only macOS opens applications at login.
        assert_eq!(
            swift.contains("BernLoginItemToggle(label: "),
            name == "macos",
            "{name}:\n{swift}"
        );
        typecheck(name);
    }
}

#[test]
fn tvos_lists_row_buttons_and_has_no_clipboard() {
    // The clipboard's buttons are refused: tvOS has none.
    let root = workspace();
    let (config, compiled) = project(
        &root.join("target/bern-test/tables-project-tvos-paste"),
        true,
    );
    let tvos = config
        .targets
        .iter()
        .find(|target| target.name == "tvos")
        .unwrap();
    let app = compiled.app.on_platform(&tvos.platform).on_television();
    let problems = SwiftUiTvos.check(&app, tvos);
    assert!(
        problems
            .iter()
            .any(|problem| problem.is_error() && format!("{problem:?}").contains("paste")),
        "{problems:?}"
    );
    let swift = generate("tvos", &SwiftUiTvos, false);
    assert!(!swift.contains("SwiftUI.Table("), "{swift}");
    assert!(swift.contains("SwiftUI.List {"), "{swift}");
    assert!(swift.contains("BernHeldText("), "{swift}");
    assert!(swift.contains("BernWatch(value: "), "{swift}");
    typecheck("tvos");
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
    let sources = root.join(format!("target/bern-test/tables-{name}/Sources"));
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
