//! Content written directly in a window, not in a component: the page
//! defines a styled control's style as it does for one in a component, and
//! places what is smaller than the window at its top leading corner.

use bern_backend_web::WebBrowser;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

const STAGE_PROTOCOL: &str = r#"
#[bern::protocol(namespace = "stage", version = 1)]
pub mod stage {
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

const STAGE_UI: &str = r#"
use bern::prelude::*;

style! {
    Button.stage {
        background: accent,
        corner: 10,
    }
}

#[component]
pub fn app(app: stage::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Stage" }, initial: open) {
                Button(id: mini, label: tr! { "en" => "Add" }, variant: stage) => app.add()
            }
        }
    }
}
"#;

/// Content smaller than the window, written directly in it: a column, a
/// component's centered column and a stack, through a condition.
const PLACE_UI: &str = r#"
use bern::prelude::*;

#[component]
pub fn app(app: stage::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Stage" }, initial: open) {
                if app.count > 1 {
                    Stack(align: bottom) {
                        Button(id: layer, label: tr! { "en" => "Layer" }) => app.add()
                    }
                } else if app.count > 0 {
                    centered(app: app)
                } else {
                    Column(gap: 12, padding: 24) {
                        Column(align: trailing) {
                            Text(content: tr! { "en" => "Inner" })
                        }
                        Button(id: add, label: tr! { "en" => "Add" }) => app.add()
                    }
                }
            }
        }
    }
}

#[component]
pub fn centered(app: stage::Client) -> impl View {
    ui! {
        Column(gap: 12, align: center) {
            Button(id: centered, label: tr! { "en" => "Centered" }) => app.add()
        }
    }
}
"#;

/// A project whose window holds `ui` directly, compiled from sources
/// written under `dir`.
fn stage_project(dir: &Path, ui: &str) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("stage-protocol", STAGE_PROTOCOL),
        ("stage-ui", ui),
        ("stage-core", ""),
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
id = "stage"
bundle_id = "dev.bern.stage"
version = "0.1.0"
name = { "en" = "Stage" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "stage-protocol"
source = "stage-protocol/src/lib.rs"

[engine]
package = "stage-core"
type = "stage_core::Engine"

[ui]
package = "stage-ui"
sources = ["stage-ui/src/lib.rs"]
root = "app"

[targets.web_app]
ui = "web"
platform = "browser"
web_mode = "app"
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

/// A variant of a control written directly in a window has its entry in
/// `S`, which the button refers to.
#[test]
fn a_style_variant_in_a_window_is_defined() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/web-stage-project"), STAGE_UI);
    let target = config.targets[0].clone();
    let problems = WebBrowser.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/web-stage"),
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
    };
    let script = WebBrowser
        .generate(&compiled.app, &context)
        .unwrap()
        .into_iter()
        .find(|file| file.path.as_os_str() == "app.js")
        .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
        .expect("the page's `app.js`");
    let defined = script
        .find("const S = {\n  buttonStage: { ")
        .unwrap_or_else(|| panic!("`buttonStage` is not defined:\n{script}"));
    let used = script
        .find("B.Button(p, ctx, { id: \"mini\", ")
        .unwrap_or_else(|| panic!("no button `mini`:\n{script}"));
    assert!(defined < used, "{script}");
    assert!(
        script[used..].contains("style: S.buttonStage }"),
        "{script}"
    );
}

/// Content smaller than the window starts at its top leading corner, as on
/// the other targets: the window's content, also through a condition or a
/// component, goes into the window's own element, a column there spans its
/// width with its items at its alignment, a button keeps its own width, and
/// a stack without a size fills the window.
#[test]
fn content_in_a_window_starts_at_its_top_leading_corner() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/web-place-project"), PLACE_UI);
    let target = config.targets[0].clone();
    let problems = WebBrowser.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/web-place"),
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
    };
    let files = WebBrowser.generate(&compiled.app, &context).unwrap();
    let text = |name: &str| {
        files
            .iter()
            .find(|file| file.path.as_os_str() == name)
            .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
            .unwrap_or_else(|| panic!("no `{name}`"))
    };
    let script = text("app.js");
    for expected in [
        "B.When(p, ctx, ",
        "B.Layers(p, ctx, { align: \"bottom\" }, ",
        "c_centered(p, B.child(ctx, \"centered\"), ",
    ] {
        assert!(script.contains(expected), "missing `{expected}`:\n{script}");
    }
    // The window's content is a column whose children stretch across it:
    // a column there is as wide as the window. A column places its items
    // by its alignment, leading by default, so a button keeps its width.
    let style = text("bern.css");
    let content = &style[style.find("\n.bern-window-content {\n").unwrap()..];
    let content = &content[..content.find('}').unwrap()];
    assert!(content.contains("flex-direction: column;"), "{content}");
    assert!(!content.contains("align-items"), "{content}");
    assert!(style.contains(".bern-window-content > .bern-layers,\n.bern-modal-content > .bern-layers,\n.bern-layers:has(.bern-split) {\n  width: 100%;\n  height: 100%;"));
    assert!(style.contains(".bern-vstack.bern-align-leading {\n  align-items: flex-start;\n}"));
    let runtime = text("bern.js");
    assert!(
        runtime.contains("bern-align-${align ?? (axis === \"column\" ? \"leading\" : \"center\")}")
    );
}

#[test]
fn browser_extension_is_loaded_built_and_packaged_with_the_page() {
    use bern_target_api::{BuildStep, Signing};
    let root = workspace();
    let project = root.join("target/bern-test/web-extension-project");
    let (_, compiled) = stage_project(&project, STAGE_UI);
    let manifest = project.join("Bern.toml");
    let config_text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, format!("{config_text}\nweb_extension = {{ path = \"web\", build = [\"python3\", \"build.py\"] }}\n")).unwrap();
    std::fs::create_dir_all(project.join("web")).unwrap();
    std::fs::write(
        project.join("web/index.js"),
        "export default () => ({imports:{}});",
    )
    .unwrap();
    let config = bern_compiler::config::load(&manifest).unwrap();
    let output = root.join("target/bern-test/web-extension-output");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: config.targets[0].clone(),
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
    let files = WebBrowser.generate(&compiled.app, &context).unwrap();
    let text = |name: &str| {
        String::from_utf8(
            files
                .iter()
                .find(|f| f.path == Path::new(name))
                .unwrap()
                .contents
                .clone(),
        )
        .unwrap()
    };
    assert!(text("app.js").starts_with("import './extension-loader.js?v="));
    assert!(text("extension-loader.js").contains("installWebExtension(setup)"));
    let plan = WebBrowser.plan(&compiled.app, &context).unwrap();
    let BuildStep::Run {
        program, args, cwd, ..
    } = &plan.steps[0]
    else {
        panic!("extension builds first")
    };
    assert_eq!(program, "python3");
    assert_eq!(
        args,
        &[
            "build.py".to_string(),
            output.join("extension").display().to_string()
        ]
    );
    assert_eq!(cwd, &project.join("web"));
    std::fs::create_dir_all(output.join("extension/lib")).unwrap();
    std::fs::write(output.join("extension/lib/worker.wasm"), [0, 97, 115, 109]).unwrap();
    let package = WebBrowser
        .package(&compiled.app, &context, &Signing::default())
        .unwrap();
    assert!(format!("{:?}", package.steps).contains("extension/lib/worker.wasm"));
    std::fs::remove_file(project.join("web/index.js")).unwrap();
    assert!(WebBrowser.generate(&compiled.app, &context).is_err());
}
