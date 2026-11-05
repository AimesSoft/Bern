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
    // Windows programs carry the icon, which GPUI gives every window, and
    // the version, from a resource script the build script compiles.
    let windows_files = Gpui::WINDOWS.generate(&compiled.app, &context).unwrap();
    let windows_file = |name: &str| {
        let file = windows_files
            .iter()
            .find(|file| file.path == Path::new(name))
            .unwrap_or_else(|| panic!("no {name}"));
        String::from_utf8_lossy(&file.contents).into_owned()
    };
    let script = windows_file("app/app.rc");
    for expected in ["1 ICON \"app.ico\"", "VALUE \"FileDescription\", \"Hello\""] {
        assert!(script.contains(expected), "{script}");
    }
    assert!(windows_file("app/build.rs").contains("embed_resource::compile(\"app.rc\""));
    let cargo = windows_file("app/Cargo.toml");
    assert!(
        cargo.contains("[build-dependencies]\nembed-resource"),
        "{cargo}"
    );
    assert!(cargo.contains("features = [\"windows\"]"), "{cargo}");
    assert!(!windows_file("app/app.ico").is_empty());
    // Linux takes neither.
    let linux_files = Gpui::LINUX.generate(&compiled.app, &context).unwrap();
    assert!(
        !linux_files
            .iter()
            .any(|file| file.path == Path::new("app/build.rs"))
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

/// A macOS icon declared as an Icon Composer document goes beside the
/// bundle for `actool`, which compiles it into the bundle; the Info.plist
/// names the compiled icon.
#[test]
fn an_icon_composer_document_is_compiled_into_the_bundle() {
    let root = workspace();
    let (config, mut compiled) = compile("hello");
    let art = root.join("target/bern-test/gpui-icon-document");
    let _ = std::fs::remove_dir_all(&art);
    let json = r#"{"fill":"automatic","groups":[{"layers":[{"image-name":"Shape.svg","name":"Shape","glass":true}]}]}"#;
    for (path, text) in [
        ("Hello.icon/icon.json", json),
        ("Hello.icon/Assets/Shape.svg", "<svg/>"),
    ] {
        let path = art.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    compiled
        .app
        .app
        .platform_icons
        .insert("macos".into(), art.join("Hello.icon").display().to_string());
    let output = root.join("target/bern-test/gpui-icon-document-out");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: gpui_macos(),
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
    let has = |name: &str| files.iter().any(|file| file.path == Path::new(name));
    assert!(has("AppIcon.icon/icon.json") && has("AppIcon.icon/Assets/Shape.svg"));
    assert!(!has("Hello.app/Contents/Resources/AppIcon.icns"));
    let plist = files
        .iter()
        .find(|file| file.path == Path::new("Hello.app/Contents/Info.plist"))
        .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
        .unwrap();
    assert!(
        plist.contains("<key>CFBundleIconName</key>\n    <string>AppIcon</string>"),
        "{plist}"
    );
    let plan = Gpui::MACOS.plan(&compiled.app, &context).unwrap();
    let actool = plan
        .steps
        .iter()
        .find_map(|step| match step {
            bern_target_api::BuildStep::Run { program, args, .. }
                if program == "xcrun" && args.first().is_some_and(|arg| arg == "actool") =>
            {
                Some(args.join(" "))
            }
            _ => None,
        })
        .expect("a step runs actool");
    assert!(actool.contains("--app-icon AppIcon"), "{actool}");
    assert!(
        actool.contains(&format!(
            "--compile {}",
            output.join("Hello.app/Contents/Resources").display()
        )),
        "{actool}"
    );
    let _ = std::fs::remove_dir_all(&art);
}

/// The gallery's `MenuBar` becomes the menus the runtime merges into the
/// system's: by role, with submenus, checked items, keys and file panels.
#[test]
fn the_menu_bar_is_generated_for_macos() {
    let root = workspace();
    let (config, compiled) = compile("gallery");
    let problems = Gpui::MACOS.check(&compiled.app, &gpui_macos());
    assert!(
        !problems
            .iter()
            .any(|problem| problem.message.contains("MenuBar")),
        "{problems:?}"
    );
    let output = root.join("target/bern-test/gpui-menu-bar");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output,
        target: gpui_macos(),
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
    let ui = files
        .iter()
        .find(|file| file.path == Path::new("app/src/ui.rs"))
        .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
        .unwrap();
    for expected in [
        "fn menu_bar(c: &b::Ctx) -> Vec<b::BarMenu>",
        "menu_bar: Some(menu_bar)",
        "Some(\"file\")",
        "Some(\"about\")",
        "b::bar_submenu(",
        "b::bar_item(c, \"menu_open\"",
        "b::bar_item(c, \"menu_crimson\"",
        "v.push(b::BarEntry::Divider);",
    ] {
        assert!(ui.contains(expected), "the gallery lacks `{expected}`");
    }
    // A checked item reads its state; a picking one opens the panel first.
    let crimson = &ui[ui.find("b::bar_item(c, \"menu_crimson\"").unwrap()..];
    assert!(!crimson[..crimson.find("));").unwrap()].contains(", false, None,"));
    let open = &ui[ui.find("b::bar_item(c, \"menu_open\"").unwrap()..];
    assert!(open[..open.find("));").unwrap()].contains("Some(("));
}

#[test]
fn motion_props_move_the_elements_they_are_on() {
    let root = workspace();
    let (config, compiled) = compile("gallery");
    let problems = Gpui::MACOS.check(&compiled.app, &gpui_macos());
    for prop in [
        "appear",
        "disappear",
        "stagger",
        "nudge",
        "animate",
        "scroll_scale",
        "scroll_opacity",
    ] {
        assert!(
            !problems
                .iter()
                .any(|problem| problem.message.contains(&format!("`{prop}`"))),
            "`{prop}` is shown without motion: {problems:?}"
        );
    }
    let output = root.join("target/bern-test/gpui-motion");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output,
        target: gpui_macos(),
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
    let ui = files
        .iter()
        .find(|file| file.path == Path::new("app/src/ui.rs"))
        .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
        .unwrap();
    for expected in [
        // Tokens and transitions of `style!`, built-in ones too.
        "const M_SMOOTH: b::Curve = (0.45, 1.0);",
        "static X_FADE: b::Transition = b::Transition { kind: b::TransitionKind::Fade, motion: M_SMOOTH };",
        // An element that appears and leaves, and the condition that keeps
        // it while it leaves.
        "appear: Some(&X_FADE), disappear: Some(&X_FADE)",
        "b::leaving(c, ",
        "b::leave(c, ",
        // The rows of a staggering list take turns.
        "b::staggered(c, ",
        // A nudge, and lengths that spring to their values.
        "nudge: Some(",
        "b::eased(c, ",
        // The film strip's cells follow scrolling, and the cell a changed
        // position scrolls to nudges.
        "scroll: Some((1.12, 0.6, (0.5, 0.5)))",
        ".within_scroll(",
        "arrival: true",
    ] {
        assert!(ui.contains(expected), "the gallery lacks `{expected}`");
    }
}

/// The generated interface of the hello world with `body` as its window's
/// content.
fn hello_with(name: &str, body: &str) -> String {
    let root = workspace();
    let dir = root.join("target/bern-test").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let copy = dir.join("source");
    for package in ["hello-core", "hello-protocol", "hello-ui"] {
        let from = root.join("examples/hello").join(package);
        for file in ["Cargo.toml", "src/lib.rs"] {
            let to = copy.join(package).join(file);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(from.join(file), to).unwrap();
        }
    }
    std::fs::copy(
        root.join("examples/hello/Bern.toml"),
        copy.join("Bern.toml"),
    )
    .unwrap();
    std::fs::write(
        copy.join("hello-ui/src/lib.rs"),
        format!(
            r#"use bern::prelude::*;
use hello_protocol::hello;

#[component]
pub fn app(app: hello::Client, windows: AppWindows) -> impl View {{
    ui! {{
        WindowSet(handle: AppWindows, primary: main) {{
            Window(id: main, title: tr! {{ "zh-Hans" => "你好", "en" => "Hello" }}, initial: open, width: 480, height: 320) {{
                {body}
            }}
        }}
    }}
}}
"#
        ),
    )
    .unwrap();
    let config = bern_compiler::config::load(&copy.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: dir.join("out"),
        target: gpui_macos(),
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
    files
        .iter()
        .find(|file| file.path == Path::new("app/src/ui.rs"))
        .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
        .unwrap()
}

/// A stack that fills only through spacers inside it starts from its own
/// length beside a spacer, so that the spacers give way to what it holds.
#[test]
fn a_spaced_stack_keeps_its_length_beside_a_spacer() {
    let ui = hello_with(
        "gpui-spaced",
        r#"Column {
                    Row {
                        Spacer
                        Column {
                            Row { Spacer Text(content: tr! { "zh-Hans" => "一", "en" => "One" }) Spacer }
                        }
                        Spacer
                    }
                    Row {
                        Column {
                            Row { Spacer Text(content: tr! { "zh-Hans" => "二", "en" => "Two" }) }
                        }
                        Column {
                            Row { Spacer Text(content: tr! { "zh-Hans" => "三", "en" => "Three" }) }
                        }
                    }
                }"#,
    );
    assert_eq!(ui.matches("keep: true").count(), 1, "{ui}");
}

/// A stack of controls in a row too narrow for them keeps its width, while
/// a stack of text gives it up and wraps.
#[test]
fn a_stack_of_controls_does_not_shrink_in_a_row() {
    let ui = hello_with(
        "gpui-firm",
        r#"Row {
                    Stack {
                        IconButton(id: down, icon: "arrow.down.circle", label: tr! { "zh-Hans" => "下载", "en" => "Download" }) => app.wave()
                    }
                    Stack {
                        Text(content: tr! { "zh-Hans" => "一", "en" => "One" })
                    }
                }"#,
    );
    assert_eq!(ui.matches("keep: true").count(), 1, "{ui}");
}
