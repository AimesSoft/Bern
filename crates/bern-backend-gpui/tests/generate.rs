//! Generate the hello world for `gpui + macos` and check that what the
//! target cannot show yet fails the check where it is declared. Compiling
//! the generated application needs GPUI, which is not part of the
//! workspace; `bern build --target <gpui target>` does that.

use bern_backend_gpui::{Gpui, product_name};
use bern_target_api::{BuildContext, Framework, Registry, Target, TargetConfig};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn compile(example: &str) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let config =
        bern_compiler::config::load(&workspace().join("examples").join(example).join("Bern.toml"))
            .unwrap();
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

fn gpui_macos() -> TargetConfig {
    TargetConfig {
        name: "gpui_macos".into(),
        platform: "macos".into(),
        ui: Some("gpui".into()),
        channels: vec!["ipc".into()],
        ..TargetConfig::default()
    }
}

#[test]
fn gpui_is_chosen_only_by_name() {
    let mut registry = Registry::new();
    registry.register(Box::new(Gpui::MACOS));
    registry.register(Box::new(Gpui::WINDOWS));
    registry.register(Box::new(Gpui::LINUX));
    for (platform, id) in [
        ("macos", "gpui-macos"),
        ("windows", "gpui-windows"),
        ("linux", "gpui-linux"),
    ] {
        let config = TargetConfig {
            name: platform.into(),
            platform: platform.into(),
            ui: Some("gpui".into()),
            ..TargetConfig::default()
        };
        assert_eq!(registry.resolve(&config).unwrap().descriptor().id, id);
        // Without `ui`, the platform's default technology is asked for,
        // which this registry does not have: never GPUI in its place.
        let default = TargetConfig { ui: None, ..config };
        assert!(registry.resolve(&default).is_err());
    }
}

#[test]
fn hello_generates_a_gpui_application() {
    let root = workspace();
    let (config, compiled) = compile("hello");
    let target = gpui_macos();
    let problems = Gpui::MACOS.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(product_name(&compiled.app), "Hello");
    let output = root.join("target/bern-test/gpui");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
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
    let files = Gpui::MACOS.generate(&compiled.app, &context).unwrap();
    let file = |name: &str| {
        let file = files
            .iter()
            .find(|file| file.path == Path::new(name))
            .unwrap_or_else(|| panic!("no {name}"));
        String::from_utf8(file.contents.clone()).unwrap()
    };
    let cargo = file("app/Cargo.toml");
    assert!(cargo.contains("bern-gpui = { path = "), "{cargo}");
    let main = file("app/src/main.rs");
    assert!(main.contains("bern_gpui::run("), "{main}");
    assert!(main.contains("ipc: true"), "{main}");
    let ui = file("app/src/ui.rs");
    // The window's one line of text, in the title style, from the strings
    // of both languages.
    assert!(ui.contains("fn window_main(c: &b::Ctx) -> b::El"), "{ui}");
    assert!(ui.contains("b::TextStyle::Title"), "{ui}");
    assert!(ui.contains("你好，世界！"), "{ui}");
    assert!(ui.contains("Hello, World!"), "{ui}");
    assert!(
        ui.contains("open: true, width: Some(480.0), height: Some(320.0)"),
        "{ui}"
    );
    let manifest = file("build.json");
    for expected in [r#""ui": "gpui""#, r#""color_source": "bern_theme""#] {
        assert!(manifest.contains(expected), "{manifest}");
    }

    // On macOS an app bundle: its Info.plist and icon, the program in
    // `Contents/MacOS`, the command line in `Contents/Helpers` and linked
    // in `bin/`.
    let plist = file("Hello.app/Contents/Info.plist");
    for expected in [
        "<key>CFBundleExecutable</key>\n    <string>hello</string>",
        "<key>CFBundleIconFile</key>\n    <string>AppIcon</string>",
    ] {
        assert!(plist.contains(expected), "{plist}");
    }
    assert!(
        files
            .iter()
            .any(|file| file.path == Path::new("Hello.app/Contents/Resources/AppIcon.icns"))
    );
    let plan = Gpui::MACOS.plan(&compiled.app, &context).unwrap();
    assert!(
        plan.products
            .iter()
            .any(|product| product == &output.join("Hello.app")),
        "{:?}",
        plan.products
    );
    assert!(
        matches!(&plan.run, Some(bern_target_api::Run::Program(program)) if program == &output.join("Hello.app/Contents/MacOS/hello")),
        "{:?}",
        plan.run
    );
    assert!(plan.steps.iter().any(|step| matches!(step,
        bern_target_api::BuildStep::Copy { to, .. } if to == &output.join("Hello.app/Contents/Helpers/hello"))));
    // Windows programs carry their extension.
    let windows = Gpui::WINDOWS.plan(&compiled.app, &context).unwrap();
    assert!(
        windows
            .products
            .iter()
            .any(|product| product == &output.join("bin/hello-mcp.exe")),
        "{:?}",
        windows.products
    );
}

#[test]
fn what_the_target_does_not_show_fails_where_it_is_declared() {
    let (_, compiled) = compile("showcase");
    let problems = Gpui::MACOS.check(&compiled.app, &gpui_macos());
    // What a component it lists cannot show yet, where it is declared, and
    // window properties beyond the size and the title.
    for expected in [
        "does not show a `Video` with the standard control bar",
        "does not show `Window(id: settings, role:)`",
    ] {
        assert!(
            problems
                .iter()
                .any(|problem| problem.message.contains(expected)),
            "{expected}: {problems:?}"
        );
    }
    assert!(
        problems
            .iter()
            .filter(|problem| problem.message.contains("`Video`"))
            .all(|problem| problem.location.is_some()),
        "{problems:?}"
    );
}
