use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

/// Point the Android target at an empty SDK, NDK and Gradle below `dir`.
/// Planning only names the tools' paths, so the build plan is the same
/// with or without Android Studio on the computer running the tests.
fn fake_android_tools(dir: &Path) {
    let sdk = dir.join("sdk");
    let ndk = sdk.join("ndk/29.0.0");
    std::fs::create_dir_all(&ndk).unwrap();
    // SAFETY: this file's only test sets these before planning, and no
    // other thread reads the environment meanwhile.
    unsafe {
        std::env::set_var("ANDROID_HOME", &sdk);
        std::env::set_var("ANDROID_NDK_HOME", &ndk);
        std::env::set_var("BERN_GRADLE", dir.join("gradle"));
    }
}

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
        let mut context = BuildContext {
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
            fake_android_tools(&root.join("target/svg-generation-test/android-tools"));
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
        // The same target selects its script adapter at build time.
        context.target.javascript = true;
        let scripted = target.generate(&compiled.app, &context).unwrap();
        let text = scripted
            .iter()
            .filter(|f| {
                f.path
                    .extension()
                    .is_some_and(|e| e != "png" && e != "icns")
            })
            .map(|f| String::from_utf8_lossy(&f.contents))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("bern-script ="), "{name}: script dependency");
        assert!(
            !text.contains("features = [\"boa\"]"),
            "{name}: must not link Boa"
        );
        assert!(
            !text.contains("/*bern:script-"),
            "{name}: unexpanded script template"
        );
        match name {
            "linux" => assert!(text.contains("\"script\"")),
            "web_app" => assert!(text.contains("new Worker(url)")),
            _ => assert!(text.contains("bernScriptDispatch")),
        }
        if name != "web_app" {
            context.target.webview = Some(false);
            let plain = target.generate(&compiled.app, &context).unwrap();
            let png = plain
                .iter()
                .find(|f| f.path.to_string_lossy().starts_with("__bern_static_svg/"))
                .unwrap();
            assert_eq!(&png.contents[..8], b"\x89PNG\r\n\x1a\n");
            let text = plain
                .iter()
                .filter(|f| {
                    f.path
                        .extension()
                        .is_some_and(|e| e != "png" && e != "icns")
                })
                .map(|f| String::from_utf8_lossy(&f.contents))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("features = [\"boa\"]"));
            assert!(!text.contains("bernScriptDispatch"));
            assert!(!text.contains("class BernSvg"));
            context.target.webview = None;
        }
        let files = scripted;
        // Preserve files for native toolchain checking on a corresponding host.
        for file in files {
            let path = context.output_dir.join(file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file.contents).unwrap();
        }
    }
}

#[test]
fn tvos_compiles_svg_to_png_and_selects_boa() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config =
        bern_compiler::config::load(&root.join("examples/svg-animation/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/svg-generation-test/tvos"),
        target: bern_target_api::TargetConfig {
            platform: "tvos".into(),
            javascript: true,
            ..Default::default()
        },
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
    let target = bern_backend_swiftui::SwiftUiTvos;
    let errors = target.check(&compiled.app, &context.target);
    assert!(!errors.iter().any(|e| e.is_error()), "{errors:?}");
    let files = target.generate(&compiled.app, &context).unwrap();
    let runtime = files
        .iter()
        .find(|f| f.path == Path::new("Sources/BernRuntime.swift"))
        .unwrap();
    assert!(!String::from_utf8_lossy(&runtime.contents).contains("WebKit"));
    assert!(
        files
            .iter()
            .any(|f| f.path == Path::new("__bern_static_svg/animation.svg.png"))
    );
    let plan = target.plan(&compiled.app, &context).unwrap();
    assert!(plan.steps.iter().any(|s| matches!(s, bern_target_api::BuildStep::Mirror { files, .. } if files.iter().any(|(from, to)| from.starts_with(&context.output_dir) && to == Path::new("__bern_static_svg/animation.svg.png")))));
    for file in files {
        let path = context.output_dir.join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.contents).unwrap();
    }
}
