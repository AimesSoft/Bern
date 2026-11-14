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
fn animated_svg_generates_for_every_presenter_without_webview() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config =
        bern_compiler::config::load(&root.join("examples/svg-animation/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    assert!(
        bern_target_api::svg::svg_used(&compiled.app)
            && !bern_target_api::svg::webpage_used(&compiled.app),
        "root-window SVGs must register the native SVG player without WebPage"
    );
    type TargetCase = (
        &'static str,
        Box<dyn Target>,
        &'static str,
        &'static str,
        &'static str,
    );
    let targets: Vec<TargetCase> = vec![
        (
            "macos",
            Box::new(bern_backend_swiftui::SwiftUiMacos),
            "Sources/BernRuntime.swift",
            "bern_svg_render",
            "WKWebView",
        ),
        (
            "ios",
            Box::new(bern_backend_swiftui::SwiftUiIos),
            "Sources/BernRuntime.swift",
            "bern_svg_render",
            "WKWebView",
        ),
        (
            "web_app",
            Box::new(bern_backend_web::WebBrowser),
            "features.js",
            "el(\"canvas\", { class: \"bern-svg\" })",
            "allow-scripts",
        ),
        (
            "android",
            Box::new(bern_backend_compose::ComposeAndroid),
            "project/app/src/main/kotlin/dev/bern/runtime/BernSvg.kt",
            "BernNative.svgRender",
            "WebView(",
        ),
        (
            "linux",
            Box::new(bern_backend_gtk::GtkLinux),
            "app/Cargo.toml",
            "bern-gtk",
            "\"webpage\"",
        ),
        (
            "windows",
            Box::new(bern_backend_winui3::WinUi3Windows),
            "app/runtime/bern_pch.h",
            "#define BERN_WITH_SVG 1",
            "#define BERN_WITH_WEBPAGE 1",
        ),
    ];
    for (name, target, file, marker, forbidden) in targets {
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
        let bsvg = files
            .iter()
            .find(|f| f.path == Path::new("__bern_anim_svg/animation.svg.bsvg"))
            .unwrap_or_else(|| panic!("{name}: missing compiled BSVG"));
        assert!(
            bsvg.contents.starts_with(bern_svg::MAGIC),
            "{name}: invalid BSVG magic"
        );

        let generated = files.iter().find(|f| f.path == *file).unwrap();
        let source = String::from_utf8_lossy(&generated.contents);
        assert!(
            source.contains(marker),
            "{name}: presenter missing `{marker}`"
        );
        assert!(
            !source.contains(forbidden),
            "{name}: must not pull in WebView (`{forbidden}`)"
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
                    .is_some_and(|e| !matches!(e.to_str(), Some("png" | "icns" | "bsvg")))
            })
            .map(|f| String::from_utf8_lossy(&f.contents))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("bern-script ="), "{name}: script dependency");
        assert!(
            !text.contains("/*bern:script-"),
            "{name}: unexpanded script template"
        );
        if name == "web_app" {
            assert!(
                !text.contains("features = [\"boa\"]"),
                "{name}: browser uses Worker"
            );
            assert!(text.contains("new Worker(url)"));
        } else {
            assert!(
                text.contains("features = [\"boa\"]"),
                "{name}: native targets must use Boa"
            );
            assert!(!text.contains("bernScriptDispatch"));
            assert!(!text.contains("\"script\""));
        }
        if name != "web_app" {
            context.target.webview = Some(false);
            let plain = target.generate(&compiled.app, &context).unwrap();
            assert!(
                plain
                    .iter()
                    .any(|f| f.path == Path::new("__bern_anim_svg/animation.svg.bsvg")),
                "{name}: native SVG player works even with `webview = false`"
            );
            let text = plain
                .iter()
                .filter(|f| {
                    f.path
                        .extension()
                        .is_some_and(|e| !matches!(e.to_str(), Some("png" | "icns" | "bsvg")))
                })
                .map(|f| String::from_utf8_lossy(&f.contents))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("features = [\"boa\"]"));
            assert!(!text.contains("bernScriptDispatch"));
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
fn tvos_and_gpui_play_animated_svg_natively_without_webkit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config =
        bern_compiler::config::load(&root.join("examples/svg-animation/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();

    // tvOS
    let tv_context = BuildContext {
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
    let tv_target = bern_backend_swiftui::SwiftUiTvos;
    let errors = tv_target.check(&compiled.app, &tv_context.target);
    assert!(!errors.iter().any(|e| e.is_error()), "{errors:?}");
    let files = tv_target.generate(&compiled.app, &tv_context).unwrap();
    let runtime = files
        .iter()
        .find(|f| f.path == Path::new("Sources/BernRuntime.swift"))
        .unwrap();
    let rt_text = String::from_utf8_lossy(&runtime.contents);
    assert!(!rt_text.contains("WebKit"));
    assert!(rt_text.contains("bern_svg_render"));
    assert!(
        files
            .iter()
            .any(|f| f.path == Path::new("__bern_anim_svg/animation.svg.bsvg"))
    );
    let plan = tv_target.plan(&compiled.app, &tv_context).unwrap();
    assert!(plan.steps.iter().any(|s| matches!(s, bern_target_api::BuildStep::Mirror { files, .. } if files.iter().any(|(from, to)| from.starts_with(&tv_context.output_dir) && to == Path::new("__bern_anim_svg/animation.svg.bsvg")))));

    // GPUI
    let gpui_context = BuildContext {
        output_dir: root.join("target/svg-generation-test/gpui"),
        target: bern_target_api::TargetConfig {
            ui: Some("gpui".into()),
            platform: "macos".into(),
            ..Default::default()
        },
        ..tv_context
    };
    let gpui_target = bern_backend_gpui::Gpui::MACOS;
    let gpui_errors = gpui_target.check(&compiled.app, &gpui_context.target);
    assert!(!gpui_errors.iter().any(|e| e.is_error()), "{gpui_errors:?}");
    let gpui_files = gpui_target.generate(&compiled.app, &gpui_context).unwrap();
    assert!(
        gpui_files
            .iter()
            .any(|f| f.path == Path::new("__bern_anim_svg/animation.svg.bsvg"))
    );
    let gpui_ui = gpui_files
        .iter()
        .find(|f| f.path == Path::new("app/src/ui.rs"))
        .unwrap();
    assert!(String::from_utf8_lossy(&gpui_ui.contents).contains("b::svg(c, b::SvgProps"));
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
    assert!(
        compiled
            .app
            .resources
            .iter()
            .all(|resource| resource.picture.is_none())
    );
    for platform in platforms {
        let app = bern_target_api::svg::prepare(&compiled.app, &target(platform));
        assert!(bern_target_api::svg::svg_used(&app), "{platform}");
    }
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

#[test]
fn decode_max_side_bounds_blurred_backdrops_and_transitive_grids() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config =
        bern_compiler::config::load(&root.join("examples/svg-animation/Bern.toml")).unwrap();
    let mut app = bern_compiler::compile(&config).unwrap().app;

    let cond = |lo: i64, hi: i64| bern_ir::Prop::Expr {
        expr: bern_ir::Expr {
            kind: bern_ir::ExprKind::If {
                cond: Box::new(bern_ir::Expr {
                    kind: bern_ir::ExprKind::Literal {
                        value: bern_protocol::Value::Bool(true),
                    },
                    ty: bern_protocol::Type::Bool,
                }),
                then: Box::new(bern_ir::Expr {
                    kind: bern_ir::ExprKind::Literal {
                        value: bern_protocol::Value::Int(lo),
                    },
                    ty: bern_protocol::Type::Int,
                }),
                otherwise: Box::new(bern_ir::Expr {
                    kind: bern_ir::ExprKind::Literal {
                        value: bern_protocol::Value::Int(hi),
                    },
                    ty: bern_protocol::Type::Int,
                }),
            },
            ty: bern_protocol::Type::Int,
        },
    };
    let elem = |kind: &str, props: Vec<(&str, bern_ir::Prop)>| bern_ir::Element {
        kind: kind.into(),
        id: None,
        path: kind.into(),
        props: props.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        template: None,
        children: Vec::new(),
        action: None,
        source: bern_ir::SourceLocation::default(),
    };

    app.components = vec![bern_ir::ComponentDef {
        name: "card".into(),
        module: Vec::new(),
        params: Vec::new(),
        body: bern_ir::Node::Element(elem(
            "Image",
            vec![("aspect", bern_ir::Prop::Number { value: 1.0 })],
        )),
        source: bern_ir::SourceLocation::default(),
    }];

    let mut grid_a = elem(
        "Grid",
        vec![("min_width", cond(96, 130)), ("gap", cond(14, 22))],
    );
    grid_a
        .children
        .push(bern_ir::Node::Component(bern_ir::ComponentRef {
            component: "card".into(),
            args: Vec::new(),
            source: bern_ir::SourceLocation::default(),
        }));
    let mut grid_b = elem(
        "Grid",
        vec![
            ("min_width", bern_ir::Prop::Number { value: 150.0 }),
            ("gap", bern_ir::Prop::Number { value: 22.0 }),
        ],
    );
    grid_b
        .children
        .push(bern_ir::Node::Component(bern_ir::ComponentRef {
            component: "card".into(),
            args: Vec::new(),
            source: bern_ir::SourceLocation::default(),
        }));
    let backdrop = elem("Image", vec![("blur", cond(50, 90))]);

    let mut column = elem("Column", Vec::new());
    column.children.push(bern_ir::Node::If(bern_ir::IfNode {
        branches: vec![(
            bern_ir::Expr {
                kind: bern_ir::ExprKind::Literal {
                    value: bern_protocol::Value::Bool(false),
                },
                ty: bern_protocol::Type::Bool,
            },
            vec![bern_ir::Node::Element(grid_a)],
        )],
        otherwise: vec![bern_ir::Node::Element(grid_b)],
    }));
    column.children.push(bern_ir::Node::Element(backdrop));
    app.windows.windows[0].content = bern_ir::Node::Element(column);

    let bern_ir::Node::Element(card_el) = &app.components[0].body else {
        panic!();
    };
    assert_eq!(
        bern_target_api::svg::decode_max_side(&app, card_el),
        Some(322.0)
    );

    let bern_ir::Node::Element(root_el) = &app.windows.windows[0].content else {
        panic!();
    };
    let bern_ir::Node::Element(backdrop_el) = &root_el.children[1] else {
        panic!();
    };
    assert_eq!(
        bern_target_api::svg::decode_max_side(&app, backdrop_el),
        Some(32.0)
    );
}
