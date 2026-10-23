//! Engine-controlled popovers must write the new value, including dismissal,
//! while existing session-bound and unbound popovers keep their behavior.
use bern_target_api::{BuildContext, Framework, Target};
use std::path::Path;

#[test]
fn popovers_generate_state_bindings_for_every_presenter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let dir = root.join("target/bern-test/popover-project");
    let protocol = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;
    #[state] pub struct State { pub panel: bool }
    #[commands] pub trait Commands {
        fn set_panel(&mut self, open: bool) -> Result<(), CommandError>;
    }
}"#;
    let ui = r##"
use bern::prelude::*;
style! {
    PopoverButton { foreground: text.primary }
    #[target(ios, tvos, android, linux, windows, browser)]
    PopoverButton { background: color(light: "#F2F2F2", dark: "#1E1E1E") }
}
#[bern::session] pub struct Draft { pub open: bool }
#[component] pub fn app(app: demo::Client, windows: AppWindows) -> impl View {
    ui! { WindowSet(handle: AppWindows, primary: main) {
        Window(id: main, title: tr! { "en" => "Test" }, initial: open) { page(app: app) }
    } }
}
#[component] pub fn page(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! { Column {
        PopoverButton(panel_appearance: window, id: controlled, label: tr! { "en" => "Controlled" }, open: app.panel) {
            Text(content: tr! { "en" => "Settings" })
        } => app.set_panel(app.panel)
        PopoverButton(panel_appearance: surface, id: adaptive, label: tr! { "en" => "Adaptive" }) { Spacer }
        PopoverButton(id: draft, label: tr! { "en" => "Draft" }, presented: draft.open) { Spacer }
        PopoverButton(id: local, label: tr! { "en" => "Local" }) { Spacer }
    } }
}"##;
    for (package, source) in [
        ("popover-protocol", protocol),
        ("popover-ui", ui),
        ("popover-core", ""),
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
id = "popover_test"
bundle_id = "dev.bern.popover-test"
name = { "en" = "Popover test" }
version = "0.1.0"
[languages]
supported = ["en"]
default = "en"
[protocol]
package = "popover-protocol"
source = "popover-protocol/src/lib.rs"
[engine]
package = "popover-core"
type = "popover_core::Engine"
[ui]
package = "popover-ui"
sources = ["popover-ui/src/lib.rs"]
root = "app"
[targets.macos]
platform = "macos"
[targets.ios]
platform = "ios"
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
            "bernEngine.setPanel(newValue, from: bernWindow)",
        ),
        (
            "ios",
            Box::new(bern_backend_swiftui::SwiftUiIos),
            "bernEngine.setPanel(newValue, from: bernWindow)",
        ),
        (
            "android",
            Box::new(bern_backend_compose::ComposeAndroid),
            "BernEngine.setPanel(it, bernWindow)",
        ),
        (
            "linux",
            Box::new(bern_backend_gtk::GtkLinux),
            "b::dispatch(c, 0, vec![v.clone()], None)",
        ),
        (
            "windows",
            Box::new(bern_backend_winui3::WinUi3Windows),
            "Dispatch(c, 0, Values{v})",
        ),
        (
            "web_app",
            Box::new(bern_backend_web::WebBrowser),
            "C.set_panel(ctx, newValue",
        ),
    ];
    for (name, target, commit) in targets {
        let context = BuildContext {
            project_dir: config.dir.clone(),
            workspace_dir: root.clone(),
            output_dir: root.join("target/bern-test/popover-targets").join(name),
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
            "{name}: missing new-value commit {commit}"
        );
        let all = files
            .iter()
            .map(|file| String::from_utf8_lossy(&file.contents))
            .collect::<Vec<_>>()
            .join("\n");
        let marker = match name {
            "macos" | "ios" => "windowTheme: true, style: BernStyles.popoverButton",
            "android" => "windowTheme = true, style = BernStyles.popoverButton",
            "linux" => "window_theme: true, style: Some(&STYLES[",
            "windows" => ".windowTheme = true, .style = &S_popoverButton",
            _ => "windowTheme: true, style: S.popoverButton",
        };
        assert!(
            all.contains(marker),
            "{name}: popup theme and colors must reach its presenter: {marker}"
        );
        if matches!(name, "macos" | "ios") {
            let line = all
                .lines()
                .find(|line| line.contains("BernPopoverButton(") && line.contains("page.adaptive"))
                .expect("adaptive popover call");
            let theme = if name == "macos" {
                "windowTheme: false"
            } else {
                "windowTheme: true"
            };
            assert!(
                line.contains(theme),
                "native glass must retain the presenting area's contrast: {line}"
            );
        }
        for file in files {
            let path = context.output_dir.join(file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file.contents).unwrap();
        }
    }
}
