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
            "class: \"bern-svg\"",
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

#[test]
fn svgs_that_neither_move_nor_run_are_pictures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config =
        bern_compiler::config::load(&root.join("examples/svg-animation/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    let target = |platform: &str| bern_target_api::TargetConfig {
        platform: platform.into(),
        ..Default::default()
    };
    let platforms = ["windows", "macos", "android", "linux", "browser"];
    // The example's document animates: it stays a document.
    assert!(
        compiled
            .app
            .resources
            .iter()
            .all(|resource| resource.picture.is_none())
    );
    for platform in platforms {
        let app = bern_target_api::svg::prepare(&compiled.app, &target(platform));
        assert!(bern_target_api::svg::used(&app), "{platform}");
    }
    // A still one, one in the text's color, or one with colors per
    // appearance is a picture of it everywhere: no document engine.
    for (picture, rendering, dark) in [
        (bern_ir::SvgPicture::Still, None, None),
        (bern_ir::SvgPicture::Tinted, Some("template"), None),
        (
            bern_ir::SvgPicture::Themed,
            None,
            Some("__bern_static_svg/animation.svg.dark.png"),
        ),
    ] {
        let mut still = compiled.app.clone();
        for resource in &mut still.resources {
            resource.picture = Some(picture);
        }
        for platform in platforms {
            let app = bern_target_api::svg::prepare(&still, &target(platform));
            assert!(!bern_target_api::svg::used(&app), "{platform}");
            for name in ["__bern_static_svg/animation.svg.png"]
                .into_iter()
                .chain(dark)
            {
                assert!(
                    app.resources.iter().any(|resource| resource.name == name),
                    "{platform}: {name}"
                );
            }
            let mut images = Vec::new();
            let mut look = |node: &bern_ir::Node| {
                if let bern_ir::Node::Element(e) = node
                    && e.kind == "Image"
                {
                    images.push((
                        e.keyword("rendering").map(str::to_string),
                        e.resource("dark_source").map(str::to_string),
                    ));
                }
            };
            for window in &app.windows.windows {
                window.content.walk(&mut look);
            }
            for component in &app.components {
                component.body.walk(&mut look);
            }
            assert!(!images.is_empty(), "{platform}");
            assert!(
                images
                    .iter()
                    .all(|(r, d)| r.as_deref() == rendering && d.as_deref() == dark),
                "{platform}: {images:?}"
            );
        }
    }
}

#[test]
fn svgs_with_colors_per_appearance_are_drawn_in_each() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config =
        bern_compiler::config::load(&root.join("examples/svg-animation/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    // The example's resource, as a document black in the light appearance
    // and white in the dark one.
    let dir = root.join("target/svg-generation-test/themed");
    std::fs::create_dir_all(&dir).unwrap();
    let svg = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4">"#,
        r#"<style>rect{fill:#000}@media (prefers-color-scheme: dark){rect{fill:#fff}}</style>"#,
        r#"<rect width="4" height="4"/></svg>"#,
    );
    std::fs::write(dir.join("animation.svg"), svg).unwrap();
    assert_eq!(bern_svg::picture(svg), Some(bern_svg::Picture::Themed));
    let mut themed = compiled.app.clone();
    for resource in &mut themed.resources {
        resource.picture = Some(bern_ir::SvgPicture::Themed);
    }
    let context = BuildContext {
        project_dir: dir.clone(),
        workspace_dir: root.clone(),
        output_dir: dir.join("out"),
        target: bern_target_api::TargetConfig {
            platform: "macos".into(),
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
    let app = bern_target_api::svg::prepare(&themed, &context.target);
    let files = bern_target_api::svg::static_files(&app, &context).unwrap();
    let first_pixel = |name: &str| {
        let file = files
            .iter()
            .find(|file| file.path == Path::new(name))
            .unwrap_or_else(|| panic!("{name}"));
        let mut reader = png::Decoder::new(std::io::Cursor::new(&file.contents))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut pixels).unwrap();
        pixels[..4].to_vec()
    };
    assert_eq!(
        first_pixel("__bern_static_svg/animation.svg.png"),
        [0, 0, 0, 255]
    );
    assert_eq!(
        first_pixel("__bern_static_svg/animation.svg.dark.png"),
        [255, 255, 255, 255]
    );
}
