//! One Rust focus region declaration reaches every native/web presenter.
use bern_target_api::{BuildContext, Framework, Target};
use std::path::Path;

#[test]
fn focus_regions_generate_for_every_presenter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let dir = root.join("target/bern-test/focus-region-project");
    let protocol = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;
    #[state] pub struct State { pub panel: bool }
    #[commands] pub trait Commands {
        fn set_panel(&mut self, open: bool) -> Result<(), CommandError>;
    }
}"#;
    let ui = r#"
use bern::prelude::*;
#[bern::session] pub struct Draft { pub text: String }
#[component] pub fn app(app: demo::Client, windows: AppWindows) -> impl View {
    ui! { WindowSet(handle: AppWindows, primary: main) {
        Window(id: main, title: tr! { "en" => "Test" }, initial: open) { page(app: app) }
    } }
}
#[component] pub fn page(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! { FocusRegion { Row {
        TextInput(id: text, label: tr! { "en" => "Message" }, value: draft.text)
        Button(id: send, label: tr! { "en" => "Send" }) => app.set_panel(true)
    } } }
}
"#;
    for (package, source) in [
        ("focus-region-protocol", protocol),
        ("focus-region-ui", ui),
        ("focus-region-core", ""),
    ] {
        let package_dir = dir.join(package);
        std::fs::create_dir_all(package_dir.join("src")).unwrap();
        std::fs::write(
            package_dir.join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(package_dir.join("src/lib.rs"), source).unwrap();
    }
    std::fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "focus_region_test"
bundle_id = "dev.bern.focusregiontest"
name = { "en" = "Popover test" }
version = "0.1.0"
[languages]
supported = ["en"]
default = "en"
[protocol]
package = "focus-region-protocol"
source = "focus-region-protocol/src/lib.rs"
[engine]
package = "focus-region-core"
type = "focus_region_core::Engine"
[ui]
package = "focus-region-ui"
sources = ["focus-region-ui/src/lib.rs"]
root = "app"
[targets.macos]
platform = "macos"
[targets.ios]
platform = "ios"
[targets.tvos]
platform = "tvos"
[targets.android]
platform = "android"
[targets.linux]
platform = "linux"
[targets.windows]
platform = "windows"
[targets.web_app]
ui = "web"
platform = "browser"
web_mode = "app"
"#,
    )
    .unwrap();
    let config = bern_compiler::config::load(&dir.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    let targets: Vec<(&str, Box<dyn Target>, &str)> = vec![
        (
            "macos",
            Box::new(bern_backend_swiftui::SwiftUiMacos),
            ".modifier(BernFocusRegion())",
        ),
        (
            "ios",
            Box::new(bern_backend_swiftui::SwiftUiIos),
            ".modifier(BernFocusRegion())",
        ),
        (
            "tvos",
            Box::new(bern_backend_swiftui::SwiftUiTvos),
            ".modifier(BernFocusRegion())",
        ),
        (
            "android",
            Box::new(bern_backend_compose::ComposeAndroid),
            "BernFocusRegion {",
        ),
        (
            "linux",
            Box::new(bern_backend_gtk::GtkLinux),
            "b::FocusRegion(p, c,",
        ),
        (
            "windows",
            Box::new(bern_backend_winui3::WinUi3Windows),
            "B::FocusRegion(p, c,",
        ),
        (
            "web_app",
            Box::new(bern_backend_web::WebBrowser),
            "B.FocusRegion(p, ctx,",
        ),
    ];
    for (name, target, commit) in targets {
        let context = BuildContext {
            project_dir: config.dir.clone(),
            workspace_dir: root.clone(),
            output_dir: root
                .join("target/bern-test/focus-region-targets")
                .join(name),
            target: config
                .targets
                .iter()
                .find(|t| t.name == name)
                .unwrap()
                .clone(),
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
        let errors = target.check(&compiled.app, &context.target);
        assert!(!errors.iter().any(|e| e.is_error()), "{name}: {errors:?}");
        let files = target.generate(&compiled.app, &context).unwrap();
        assert!(
            files
                .iter()
                .any(|f| String::from_utf8_lossy(&f.contents).contains(commit)),
            "{name}: missing generated focus region {commit}"
        );
        for file in files {
            let path = context.output_dir.join(file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file.contents).unwrap();
        }
    }
}
