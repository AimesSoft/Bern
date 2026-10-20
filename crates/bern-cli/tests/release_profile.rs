//! Every crate a target generates builds its releases the same way: one
//! code generation unit with link-time optimization, and programs without
//! a symbol table.

use bern_target_api::{BuildContext, Framework, Target};
use std::path::Path;

#[test]
fn every_generated_crate_optimizes_its_releases_across_crates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config = bern_compiler::config::load(&root.join("examples/gallery/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    let targets: Vec<(&str, Box<dyn Target>)> = vec![
        ("macos", Box::new(bern_backend_swiftui::SwiftUiMacos)),
        ("ios", Box::new(bern_backend_swiftui::SwiftUiIos)),
        ("android", Box::new(bern_backend_compose::ComposeAndroid)),
        ("linux", Box::new(bern_backend_gtk::GtkLinux)),
        ("windows", Box::new(bern_backend_winui3::WinUi3Windows)),
        ("web_app", Box::new(bern_backend_web::WebBrowser)),
        ("web_ui", Box::new(bern_backend_web::WebBrowser)),
    ];
    let mut manifests = Vec::new();
    for (name, target) in targets {
        let context = BuildContext {
            project_dir: config.dir.clone(),
            workspace_dir: root.clone(),
            output_dir: root.join("target/bern-test/release-profile").join(name),
            target: config
                .targets
                .iter()
                .find(|target| target.name == name)
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
            release: true,
        };
        for file in target.generate(&compiled.app, &context).unwrap() {
            if !file.path.ends_with("Cargo.toml") {
                continue;
            }
            let shown = format!("{name}: {}", file.path.display());
            let manifest: toml::Table =
                toml::from_str(std::str::from_utf8(&file.contents).unwrap()).unwrap();
            let release = &manifest["profile"]["release"];
            assert_eq!(release["lto"].as_bool(), Some(true), "{shown}");
            assert_eq!(release["codegen-units"].as_integer(), Some(1), "{shown}");
            assert_eq!(release["strip"].as_bool(), Some(true), "{shown}");
            manifests.push(shown);
        }
    }
    // Android's command line and MCP server, and the WebUI's engine host,
    // are crates of their own beside the engine's.
    assert_eq!(
        manifests,
        [
            "macos: glue/Cargo.toml",
            "ios: glue/Cargo.toml",
            "android: glue/Cargo.toml",
            "android: tools/Cargo.toml",
            "linux: app/Cargo.toml",
            "windows: glue/Cargo.toml",
            "web_app: glue/page/Cargo.toml",
            "web_ui: glue/page/Cargo.toml",
            "web_ui: glue/host/Cargo.toml",
        ]
    );
}
