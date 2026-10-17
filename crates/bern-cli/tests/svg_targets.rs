use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

#[test]
fn animated_svg_generates_for_every_presenter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config =
        bern_compiler::config::load(&root.join("examples/svg-animation/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    assert!(
        bern_target_api::svg::used(&compiled.app),
        "root-window SVGs must load the runtime too"
    );
    let targets: Vec<(&str, Box<dyn Target>, &str, &str)> = vec![
        (
            "macos",
            Box::new(bern_backend_swiftui::SwiftUiMacos),
            "Sources/BernRuntime.swift",
            "WKWebView",
        ),
        (
            "ios",
            Box::new(bern_backend_swiftui::SwiftUiIos),
            "Sources/BernRuntime.swift",
            "UIViewRepresentable",
        ),
        (
            "web_app",
            Box::new(bern_backend_web::WebBrowser),
            "features.js",
            "sandbox: \"allow-scripts\"",
        ),
        (
            "android",
            Box::new(bern_backend_compose::ComposeAndroid),
            "project/app/src/main/kotlin/dev/bern/runtime/BernSvg.kt",
            "WebView(context)",
        ),
        (
            "linux",
            Box::new(bern_backend_gtk::GtkLinux),
            "app/Cargo.toml",
            "features = [\"svg\"]",
        ),
        (
            "windows",
            Box::new(bern_backend_winui3::WinUi3Windows),
            "app/runtime/bern_ui_svg.inl",
            "CreateCoreWebView2EnvironmentWithOptions",
        ),
    ];
    for (name, target, file, marker) in targets {
        let context = BuildContext {
            project_dir: config.dir.clone(),
            workspace_dir: root.clone(),
            output_dir: root.join("target/svg-generation-test").join(name),
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
        let generated = files.iter().find(|f| f.path == *file).unwrap();
        let source = String::from_utf8_lossy(&generated.contents);
        assert!(source.contains(marker), "{name}: presenter missing");
        assert!(
            !source.contains("/*bern:svg-"),
            "{name}: unresolved runtime template"
        );
        if name == "android" {
            assert!(source.contains("assets.open(\"bern/resources/$source\")"));
            let plan = target.plan(&compiled.app, &context).unwrap();
            assert!(
                plan.steps.iter().any(|step| matches!(step,
                    bern_target_api::BuildStep::Mirror { dir, keep, .. }
                    if dir.ends_with("app/src/main/kotlin")
                        && keep.contains(&PathBuf::from("dev/bern/runtime/BernSvg.kt"))
                )),
                "Android's source cleanup must preserve the SVG presenter"
            );
        }
        // Preserve files for native toolchain checking on a corresponding host.
        for file in files {
            let path = context.output_dir.join(file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file.contents).unwrap();
        }
    }
}
