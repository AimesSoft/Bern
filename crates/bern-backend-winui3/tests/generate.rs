//! Generate the showcase for `winui3 + windows` and, on Windows with the
//! Visual Studio C++ tools installed, compile the generated sources for both
//! front ends, so generator mistakes fail here rather than in `bern build`.

use bern_backend_winui3::{WinUi3Windows, product_name};
use bern_ir::Application;
use bern_target_api::{BuildContext, BuildStep, Framework, Registry, Target, TargetConfig};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    // Without the verbatim prefix Windows gives canonical paths.
    match root.to_string_lossy().strip_prefix(r"\\?\") {
        Some(plain) => PathBuf::from(plain),
        None => root,
    }
}

fn showcase() -> (bern_compiler::Config, bern_compiler::Compiled) {
    let root = workspace();
    let config = bern_compiler::config::load(&root.join("examples/showcase/Bern.toml")).unwrap();
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

fn windows_target(config: &bern_compiler::Config) -> TargetConfig {
    config
        .targets
        .iter()
        .find(|target| target.platform == "windows")
        .expect("the showcase declares a Windows target")
        .clone()
}

#[test]
fn windows_resolves_to_winui3_by_default() {
    let mut registry = Registry::new();
    registry.register(Box::new(WinUi3Windows));
    let config = TargetConfig {
        name: "windows".into(),
        platform: "windows".into(),
        ..TargetConfig::default()
    };
    assert_eq!(
        registry.resolve(&config).unwrap().descriptor().id,
        "winui3-windows"
    );
}

#[test]
fn languages_without_runtime_texts_are_rejected() {
    let (config, compiled) = showcase();
    let mut app: Application = compiled.app;
    // Use a reserved private-use language so adding translations cannot
    // turn this unsupported-language case into a supported one.
    app.languages.supported.push("qaa".into());
    let problems = WinUi3Windows.check(&app, &windows_target(&config));
    assert!(
        problems
            .iter()
            .any(|problem| problem.message.contains("no `qaa` translation")),
        "{problems:?}"
    );
}

#[test]
fn showcase_generates_a_winui3_application() {
    let root = workspace();
    let (config, compiled) = showcase();
    let target_config = windows_target(&config);
    let problems = WinUi3Windows.check(&compiled.app, &target_config);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(product_name(&compiled.app), "Showcase");
    let output = root.join("target/bern-test/winui3");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: target_config,
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }

    // The engine library learns the declared resource names and the
    // channel, and exports the start function.
    let lib = std::fs::read_to_string(output.join("glue/src/lib.rs")).unwrap();
    assert!(
        lib.contains(
            r#"resources: vec!["images/bern.png".into(), "images/hdr-gainmap.jpg".into(), "images/hdr-pq.png".into(), "images/landscape.jpg".into(), "media/aurora.mp4".into(), "media/broken.mp4".into(), "media/tides.mp4".into()]"#
        ),
        "{lib}"
    );
    assert!(lib.contains("ipc: true"), "{lib}");
    assert!(
        lib.contains("pub unsafe extern \"C\" fn bern_app_start("),
        "{lib}"
    );
    let cargo = std::fs::read_to_string(output.join("glue/Cargo.toml")).unwrap();
    assert!(cargo.contains(r#"crate-type = ["staticlib"]"#), "{cargo}");
    let cargo_config = std::fs::read_to_string(output.join("glue/.cargo/config.toml")).unwrap();
    assert!(cargo_config.contains("+crt-static"), "{cargo_config}");
    assert!(
        !cargo.contains(r"\\?\") && !cargo.contains("//?/"),
        "{cargo}"
    );
    let manifest = std::fs::read_to_string(output.join("build.json")).unwrap();
    for expected in [
        r#""ui": "winui3""#,
        r#""backend": "windows-mediaengine""#,
        r#""media/tides.mp4""#,
    ] {
        assert!(manifest.contains(expected), "{manifest}");
    }
    for expected in [r#""ui_bridge": "cpp""#, r#""xaml-islands""#] {
        assert!(manifest.contains(expected), "{manifest}");
    }
    let script = std::fs::read_to_string(output.join("app/build.bat")).unwrap();
    for expected in [
        "vcvarsall.bat",
        "cppwinrt.exe",
        "microsoft.windowsappsdk.winui 2.3.9",
        "Microsoft.WindowsAppRuntime.Bootstrap.dll",
        "set \"EXE=showcase\"",
        "bern_app.lib",
        // The precompiled header is made once per profile and reused.
        "set \"OBJ=%APP%\\obj\\debug\"",
        "/Yc\"bern_pch.h\"",
        "/Yu\"bern_pch.h\"",
        "pch.key",
    ] {
        assert!(script.contains(expected), "missing `{expected}`");
    }
    assert!(
        script.contains("\r\n") && !script.contains("\r\r"),
        "CRLF lines"
    );
    // Without `[app].icon` the executable carries the Bern logo.
    let ico = std::fs::read(output.join("app/app.ico")).unwrap();
    assert_eq!(&ico[..4], &[0, 0, 1, 0]);
    let resource = std::fs::read_to_string(output.join("app/app.rc")).unwrap();
    assert!(resource.contains("1 ICON \"app.ico\""), "{resource}");
    assert!(
        resource.contains("VALUE \"ProductName\", \"Bern 控件展示\""),
        "{resource}"
    );
    let identity = std::fs::read_to_string(output.join("app/app_id.h")).unwrap();
    assert!(
        identity.contains("#define BERN_BUNDLE_ID \"dev.bern.showcase\""),
        "{identity}"
    );
    let manifest = std::fs::read_to_string(output.join("app/app.manifest")).unwrap();
    assert!(manifest.contains("maxversiontested"), "{manifest}");
    for runtime in [
        "runtime/bern_base.h",
        "runtime/bern_main.cpp",
        "runtime/bern_pch.h",
        "runtime/bern_winui.cpp",
        "runtime/bern_islands.cpp",
        "runtime/bern_ui_windows.inl",
    ] {
        assert!(output.join("app").join(runtime).is_file(), "{runtime}");
    }

    // Every component is realized; without style overrides, colors come
    // from the system theme.
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    assert!(!ui.contains("unsupported"), "{ui}");
    // The tool strip: in the flow, a spacer takes the rest of its line and
    // the chips are items (the runtime adds them to the flow's parent).
    let strip = &ui[ui.find("B::Flow(p, c, {.gap = B::Len(10.0)}").unwrap()..];
    let spacer = strip.find("B::Spacer(p);").unwrap();
    let chips = strip.find("{.id = \"strip_filter\"").unwrap();
    assert!(spacer < chips, "{strip}");
    assert!(
        strip[chips..].contains(".style = \"chips\", .labelHidden = true"),
        "{strip}"
    );
    for forbidden in ["ColorHelper", "Colors::", "SolidColorBrush"] {
        assert!(
            !ui.contains(forbidden),
            "generated code without style overrides uses `{forbidden}`"
        );
    }
    for expected in [
        "B::SplitView(p, c, {.id = \"pages\", .selection = Field{0, 0}",
        "B::Navigation(p, c, \"library\"",
        "B::Query(p, c, {.id = \"library_results\"",
        "B::Adaptive(p, c, \"clip_choice\",",
        "B::Tabs(p, c, {.id = \"tabs\"",
        "B::Video(p, c, {.id = \"preview\", .player = \"preview\"",
        "static CloseCheck close_settings(Ctx const& c) {",
        "static std::vector<WindowDef> Windows() {",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`");
    }
    let catalog = std::fs::read_to_string(output.join("app/app_catalog.cpp")).unwrap();
    for expected in [
        "Catalog BernCatalog() {",
        "c.bundleId = \"dev.bern.showcase\";",
        "L\"Bern 控件展示\"",
    ] {
        assert!(catalog.contains(expected), "missing `{expected}`");
    }

    // The plan builds the engine library, then the application, ships the
    // resources next to the executable and copies the entries.
    let plan = WinUi3Windows.plan(&compiled.app, &context).unwrap();
    let programs: Vec<&str> = plan
        .steps
        .iter()
        .filter_map(|step| match step {
            BuildStep::Run { program, .. } => Some(program.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(programs, ["cargo", "cmd"]);
    let mirror = plan
        .steps
        .iter()
        .find_map(|step| match step {
            BuildStep::Mirror { dir, files, .. } => Some((dir, files)),
            _ => None,
        })
        .expect("a mirror step");
    assert_eq!(mirror.0, &output.join("Showcase/resources"));
    assert!(mirror.1.contains(&(
        config.dir.join("media/aurora.mp4"),
        PathBuf::from("media/aurora.mp4")
    )));
    assert_eq!(
        plan.products,
        [
            output.join("Showcase/showcase.exe"),
            output.join("bin/showcase.exe"),
            output.join("bin/showcase-mcp.exe"),
            output.join("skills/showcase/SKILL.md"),
        ]
    );
    // The agent skill spells the programs' paths as Windows does.
    let skill = plan
        .steps
        .iter()
        .find_map(|step| match step {
            BuildStep::Write { path, contents } if path.ends_with("skills/showcase/SKILL.md") => {
                Some(String::from_utf8(contents.clone()).unwrap())
            }
            _ => None,
        })
        .expect("the plan writes the skill");
    assert!(
        skill.contains("| `showcase` | `..\\..\\bin\\showcase.exe` |"),
        "{skill}"
    );
    assert!(skill.contains("at `%LOCALAPPDATA%\\Programs\\Showcase\\bin\\showcase.exe`"));

    compile(&output);
}

/// A target may build one front end alone: the system XAML without the
/// bootstrapper, or WinUI 3 that offers the runtime's installer.
#[test]
fn a_target_can_build_one_front_end() {
    let root = workspace();
    let (config, compiled) = showcase();
    let generate = |frontends: &[&str]| {
        let target = TargetConfig {
            frontends: frontends.iter().map(|name| name.to_string()).collect(),
            ..windows_target(&config)
        };
        let context = BuildContext {
            project_dir: config.dir.clone(),
            workspace_dir: root.clone(),
            output_dir: root.join("target/bern-test/winui3-frontends"),
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
        let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
        let text = |name: &str| {
            files
                .iter()
                .find(|file| file.path == Path::new(name))
                .map(|file| String::from_utf8(file.contents.clone()).unwrap())
                .unwrap()
        };
        (
            text("app/build.bat"),
            text("build.json"),
            text("app/app_id.h"),
        )
    };

    let (script, manifest, _) = generate(&["xaml-islands"]);
    assert!(script.contains("/DBERN_WITH_WINUI=0 /DBERN_WITH_ISLANDS=1"));
    assert!(!script.contains("bern_winui.cpp") && script.contains("bern_islands.cpp"));
    assert!(!script.contains("Bootstrap.lib"), "no bootstrapper to link");
    assert!(script.contains(r#"del /q "%OUT%\Microsoft.WindowsAppRuntime.Bootstrap.dll""#));
    assert!(
        manifest.contains(
            r#""frontends": [
    "xaml-islands"
  ]"#
        ),
        "{manifest}"
    );

    let (script, manifest, identity) = generate(&["winui3"]);
    assert!(script.contains("/DBERN_WITH_WINUI=1 /DBERN_WITH_ISLANDS=0"));
    assert!(script.contains("bern_winui.cpp") && !script.contains("bern_islands.cpp"));
    assert!(script.contains("Microsoft.WindowsAppRuntime.Bootstrap.lib"));
    assert!(manifest.contains(r#""winui3""#) && !manifest.contains(r#""xaml-islands""#));
    assert!(
        identity.contains(r#"#define BERN_RUNTIME_INSTALLER L"https://aka.ms/windowsappsdk/2.5/latest/windowsappruntimeinstall-"#),
        "{identity}"
    );

    // Each choice has its own precompiled header.
    let (both, _, _) = generate(&[]);
    let key = |script: &str| {
        script
            .lines()
            .find(|line| line.starts_with("set \"KEY="))
            .unwrap()
            .to_string()
    };
    assert_ne!(key(&both), key(&script));
}

/// Compile the generated sources on Windows where the Visual Studio C++
/// tools are installed. Linking needs the engine library, which `bern
/// build` makes; compiling both front ends checks the generator.
#[cfg(windows)]
fn compile(output: &Path) {
    use std::process::Command;
    let vswhere = Path::new(&std::env::var("ProgramFiles(x86)").unwrap_or_default())
        .join("Microsoft Visual Studio/Installer/vswhere.exe");
    if !vswhere.is_file() {
        eprintln!("the Visual Studio C++ tools are not installed; skipping the compile check");
        return;
    }
    let result = Command::new("cmd")
        .args(["/d", "/c"])
        .arg(output.join("app/build.bat"))
        .arg("compile")
        .current_dir(output.join("app"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "the C++ compiler rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[cfg(not(windows))]
fn compile(_: &Path) {
    eprintln!("not on Windows; skipping the compile check");
}

/// Compile an example, check it for Windows and generate its sources into
/// `target/bern-test/winui3-<name>`.
fn generate_example(name: &str) -> (Application, PathBuf) {
    let root = workspace();
    let config =
        bern_compiler::config::load(&root.join(format!("examples/{name}/Bern.toml"))).unwrap();
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
    let target_config = windows_target(&config);
    let problems: Vec<_> = WinUi3Windows
        .check(&compiled.app, &target_config)
        .into_iter()
        .filter(|problem| problem.severity != bern_target_api::Severity::Note)
        .collect();
    assert!(problems.is_empty(), "{name}: {problems:#?}");
    let output = root.join(format!("target/bern-test/winui3-{name}"));
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: target_config,
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    (compiled.app, output)
}

/// The planner: sidebar items that come and go, styled buttons and sidebar
/// items, a drawing with a pattern and series, a marked text, shared
/// elements, transitions and an immersive window that rests.
#[test]
fn planner_generates_without_warnings() {
    let (_, output) = generate_example("planner");
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    assert!(!ui.contains("unsupported"), "{ui}");
    for expected in [
        "static B::StyleDef const S_sidebarItem = [] {",
        "static B::StyleDef const S_buttonHero = [] {",
        "static B::PatternDef const P_gingham{",
        "static B::Transition const X_pop_in{",
        "B::SidebarItem(p, c, {",
        ".style = &S_sidebarItem",
        "B::Canvas(p, c, {",
        "B::PatternOp{.pattern = &P_gingham",
        "B::SeriesOp{.values = R::Numbers(",
        // A dashed outline.
        ".dash = 4.5, .dashGap = 3.0",
        ".mark = \"accent\"",
        ".hero = \"title\"",
        ".appear = &X_pop_in, .disappear = &X_fade",
        "c.WindowFlag(\"idle\")",
        "w.idleAfter = 3.0;",
        "w.immersive = [](Ctx const& c)",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`");
    }
    compile(&output);
}

/// The gallery: every v0.29 feature and the four of v0.30.
/// The animated SVG example: its runtime is generated only for
/// applications declaring `Svg`, so no other test compiles it.
#[test]
fn svg_animation_generates_and_compiles() {
    let (_, output) = generate_example("svg-animation");
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    assert!(ui.contains("B::Svg(p, c, {"), "{ui}");
    let svg = std::fs::read_to_string(output.join("app/runtime/bern_ui_svg.inl")).unwrap();
    assert!(svg.contains("namespace B {"));
    compile(&output);
}

#[test]
fn gallery_generates_without_warnings() {
    let (_, output) = generate_example("gallery");
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    assert!(!ui.contains("unsupported"), "{ui}");
    for expected in [
        "B::Toast(p, c, {",
        "B::Inspector(p, c, {.presented = Field{",
        "B::SplitButton(p, c, {",
        // The caption's split button shows engine state and sets it.
        "B::SplitButton(p, c, {.id = \"caption\"",
        ".current = ",
        "B::PopoverButton(p, c, {",
        "B::Drop(p, c, {",
        "B::Pinch(p, c, {",
        "B::Swipe(p, c, {",
        "B::Rail(p, c, {",
        "B::Place(p, c, {.at = B::Len(Get(",
        "B::ContentButton(p, c, {",
        "B::Shortcut(c, \"cmd+o\", \"open_file\", \"activate\");",
        "B::Shortcut(c, \"cmd+k\", \"find\", \"focus\");",
        ".pick = Field{",
        ".files = \"video\"",
        "B::ScrollView(p, c, {.axis = \"horizontal\", .snap = \"center\", .position = Field{",
        ".scrollScale = 1.12, .scrollOpacity = 0.6",
        ".hideIndicators = true",
        ".stagger = 0.04",
        "c.Write(Field{",
        "B::Adaptive(p, c, \"film_controls\", std::array<Build, 4>{",
        "w.dialog = true;",
        "w.accent = ThemeAccent;",
        "w.appearance = ThemeAppearance;",
        ".reveal = true",
        ".appearance = \"dark\"",
        ".nudge = [](Ctx const& c)",
        "R::Url(c.Engine(",
        "B::KeyShortcut(p, c, {.keys = \"escape\"",
        "B::DefaultFocus(c, ",
        "c.WindowFlag(\"tv\")",
        "R::Append(c.Peek(",
        "R::DropLast(c.Peek(",
        ".suggest = true",
        // The about window's first tab shows only its symbol.
        ".icon = \"info.circle\", .iconOnly = true, .build = ",
        // The details' keys follow state, and the user chooses them.
        "B::Shortcut(c, [](Ctx const& c) { return ",
        "B::ShortcutInput(p, c, {.id = \"details_key\"",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`");
    }
    let catalog = std::fs::read_to_string(output.join("app/app_catalog.cpp")).unwrap();
    for expected in [
        "c.mediumBelow = 1024.0;",
        "c.fileKinds.push_back({\"video\", {\"mp4\"",
        "wchar_t const* IconGlyph(",
    ] {
        assert!(catalog.contains(expected), "missing `{expected}`");
    }
    compile(&output);
}

/// The showcase with a `TabAccessory` in its first `Tabs`, shown while a
/// literal holds.
fn add_tab_accessory(app: &mut bern_ir::Application) {
    use bern_ir::{Element, Expr, ExprKind, Node, Prop, SourceLocation};
    fn into(node: &mut Node) -> bool {
        match node {
            Node::Element(element) if element.kind == "Tabs" => {
                element.children.push(Node::Element(Element {
                    kind: "TabAccessory".into(),
                    id: None,
                    path: "test.accessory".into(),
                    props: [(
                        "shown".into(),
                        Prop::Expr {
                            expr: Expr {
                                ty: bern_protocol::Type::Bool,
                                kind: ExprKind::Literal {
                                    value: bern_protocol::Value::Bool(true),
                                },
                            },
                        },
                    )]
                    .into(),
                    template: None,
                    children: vec![Node::Element(Element {
                        kind: "Spacer".into(),
                        id: None,
                        path: "test.accessory.0".into(),
                        props: Default::default(),
                        template: None,
                        children: vec![],
                        action: None,
                        source: SourceLocation {
                            file: "tabs.rs".into(),
                            line: 1,
                            column: 1,
                        },
                    })],
                    action: None,
                    source: SourceLocation {
                        file: "tabs.rs".into(),
                        line: 1,
                        column: 1,
                    },
                }));
                true
            }
            Node::Element(element) => element.children.iter_mut().any(into),
            Node::If(node) => {
                node.branches
                    .iter_mut()
                    .any(|(_, body)| body.iter_mut().any(into))
                    || node.otherwise.iter_mut().any(into)
            }
            _ => false,
        }
    }
    assert!(
        app.components
            .iter_mut()
            .any(|component| into(&mut component.body))
    );
}

#[test]
fn tab_accessory_is_a_card_under_the_pages() {
    let (config, mut compiled) = showcase();
    add_tab_accessory(&mut compiled.app);
    let root = workspace();
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/winui3-tab-accessory"),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    let source = files
        .iter()
        .map(|f| String::from_utf8_lossy(&f.contents).into_owned())
        .collect::<String>();
    assert!(
        source.contains("B::TabAccessory{.shown = [](Ctx const& c) { return "),
        "{source}"
    );
}

fn node(
    kind: &str,
    props: Vec<(&str, bern_ir::Prop)>,
    children: Vec<bern_ir::Node>,
) -> bern_ir::Node {
    bern_ir::Node::Element(bern_ir::Element {
        kind: kind.into(),
        id: matches!(kind, "Menu" | "SearchItem" | "Button")
            .then(|| format!("test_{}", kind.to_lowercase())),
        path: format!("test.{kind}"),
        props: props
            .into_iter()
            .map(|(name, prop)| (name.to_string(), prop))
            .collect(),
        template: None,
        children,
        action: None,
        source: bern_ir::SourceLocation {
            file: "features.rs".into(),
            line: 1,
            column: 1,
        },
    })
}

fn number(value: f64) -> bern_ir::Prop {
    bern_ir::Prop::Number { value }
}

fn flag(value: bool) -> bern_ir::Prop {
    bern_ir::Prop::Expr {
        expr: bern_ir::Expr {
            ty: bern_protocol::Type::Bool,
            kind: bern_ir::ExprKind::Literal {
                value: bern_protocol::Value::Bool(value),
            },
        },
    }
}

fn word(value: &str) -> bern_ir::Prop {
    bern_ir::Prop::Keyword {
        value: value.into(),
    }
}

fn words(value: &str) -> bern_ir::Prop {
    bern_ir::Prop::Text {
        text: bern_ir::Text::Dynamic {
            expr: bern_ir::Expr {
                ty: bern_protocol::Type::Text,
                kind: bern_ir::ExprKind::Literal {
                    value: bern_protocol::Value::Text(value.into()),
                },
            },
        },
    }
}

fn icon(name: &str) -> bern_ir::Prop {
    bern_ir::Prop::Icon { name: name.into() }
}

/// The showcase with a scene and a layer that bleeds out of it, an
/// accordion, wrapping rows, a grid of cells, a modal, a form's layout,
/// icon-only labels and menus, capsule
/// controls, a search item, a layer under the window's bars and a capsule
/// button around its content at the end of its first component.
fn add_layout_features(app: &mut bern_ir::Application) {
    let text = || {
        node(
            "Text",
            vec![
                ("content", words("Text")),
                ("lines", number(2.0)),
                ("font_size", number(12.0)),
                ("line_height", number(16.0)),
            ],
            vec![],
        )
    };
    let features = node(
        "Column",
        vec![("padding_x", number(12.0)), ("padding_y", number(4.0))],
        vec![
            node(
                "ScaleBox",
                vec![("width", number(400.0)), ("height", number(300.0))],
                vec![
                    node(
                        "Positioned",
                        vec![
                            ("x", number(0.0)),
                            ("y", number(0.0)),
                            ("width", number(400.0)),
                            ("height", number(300.0)),
                            ("bleed", flag(true)),
                        ],
                        vec![],
                    ),
                    node(
                        "Positioned",
                        vec![
                            ("x", number(10.0)),
                            ("y", number(20.0)),
                            ("width", number(100.0)),
                            ("height", number(40.0)),
                            ("animate", word("smooth")),
                        ],
                        vec![node(
                            "Marquee",
                            vec![("duration", number(8.0)), ("phase", number(0.2))],
                            vec![text()],
                        )],
                    ),
                ],
            ),
            node(
                "Accordion",
                vec![
                    ("count", number(3.0)),
                    ("selected", number(1.0)),
                    ("collapsed_width", number(40.0)),
                    ("height", number(120.0)),
                    ("corner", number(12.0)),
                    ("animate", word("bouncy")),
                ],
                vec![text(), text(), text()],
            ),
            node(
                "Flow",
                vec![("gap", number(4.0))],
                vec![node(
                    "Label",
                    vec![
                        ("title", words("Star")),
                        ("icon", icon("star")),
                        ("display", word("icon")),
                    ],
                    vec![],
                )],
            ),
            node(
                "Grid",
                vec![
                    ("min_width", number(80.0)),
                    ("row_gap", number(6.0)),
                    ("cell_ratio", number(0.667)),
                    ("cell_extra", number(20.0)),
                ],
                vec![
                    node(
                        "Image",
                        vec![
                            (
                                "source",
                                bern_ir::Prop::Resource {
                                    name: "images/bern.png".into(),
                                },
                            ),
                            ("corner", number(8.0)),
                            ("rendering", word("template")),
                            ("aspect", number(1.5)),
                        ],
                        vec![],
                    ),
                    // A picture lowered from an SVG with colors per
                    // appearance: one for the dark one beside it.
                    node(
                        "Image",
                        vec![
                            (
                                "source",
                                bern_ir::Prop::Resource {
                                    name: "images/bern.png".into(),
                                },
                            ),
                            (
                                "dark_source",
                                bern_ir::Prop::Resource {
                                    name: "images/bern-dark.png".into(),
                                },
                            ),
                            ("width", number(24.0)),
                            ("height", number(24.0)),
                        ],
                        vec![],
                    ),
                    // Flying, it is drawn larger than its size on the way.
                    node(
                        "Image",
                        vec![
                            (
                                "source",
                                bern_ir::Prop::Resource {
                                    name: "images/bern.png".into(),
                                },
                            ),
                            ("width", number(48.0)),
                            ("height", number(27.0)),
                            ("hero", word("still")),
                        ],
                        vec![],
                    ),
                ],
            ),
            node(
                "Row",
                vec![("align", word("baseline"))],
                vec![
                    text(),
                    node("Spacer", vec![], vec![]),
                    node(
                        "Menu",
                        vec![
                            ("label", words("More")),
                            ("icon", icon("ellipsis")),
                            ("style", word("icon")),
                            ("shape", word("capsule")),
                            ("height", number(30.0)),
                        ],
                        vec![],
                    ),
                    node(
                        "ControlGroup",
                        vec![("height", number(32.0))],
                        vec![node(
                            "SearchItem",
                            vec![
                                ("label", words("Open")),
                                ("icon", icon("folder")),
                                ("detail", words("File")),
                                ("hint", words("⌘O")),
                            ],
                            vec![],
                        )],
                    ),
                ],
            ),
            node(
                "Form",
                vec![("gap", number(12.0)), ("row_height", number(44.0))],
                vec![node(
                    "Section",
                    vec![("style", word("plain")), ("padding_x", number(8.0))],
                    vec![text()],
                )],
            ),
            node(
                "Modal",
                vec![
                    ("width", number(300.0)),
                    ("height", number(200.0)),
                    ("cover", word("compact")),
                ],
                vec![text()],
            ),
            // As tall as its content.
            node("Modal", vec![("width", number(280.0))], vec![text()]),
            // A scene under the window's bars, and a capsule button around
            // its content.
            node("Stack", vec![("under_bars", word("true"))], vec![text()]),
            node(
                "Button",
                vec![
                    ("label", words("Continue")),
                    ("shape", word("capsule")),
                    ("height", number(32.0)),
                ],
                vec![node("Row", vec![], vec![text()])],
            ),
        ],
    );
    let component = app
        .components
        .iter_mut()
        .find(|component| {
            matches!(&component.body, bern_ir::Node::Element(body) if body.kind == "Form")
        })
        .expect("a component whose body is a form");
    let bern_ir::Node::Element(body) = &mut component.body else {
        unreachable!()
    };
    body.children.push(features);
}

/// The layout features of the scaled scenes, accordions, flows, cell
/// grids, modals and form layouts generate the runtime's calls; on Windows
/// with the C++ tools both front ends compile them.
#[test]
fn layout_features_generate_for_winui3() {
    let (config, mut compiled) = showcase();
    add_layout_features(&mut compiled.app);
    let problems = WinUi3Windows.check(&compiled.app, &windows_target(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let root = workspace();
    let output = root.join("target/bern-test/winui3-layout-features");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    assert!(!ui.contains("unsupported"), "{ui}");
    for expected in [
        "B::Stack(p, c, {.axis = \"column\", .paddingX = B::Len(12.0), .paddingY = B::Len(4.0)}, ",
        "B::ScaleBox(p, c, {.width = B::Len(400.0), .height = B::Len(300.0)}, ",
        // The background bleeds past the scene's edges; the other layer
        // stays in the scene.
        "B::Positioned(p, c, {.x = B::Len(0.0), .y = B::Len(0.0), .width = B::Len(400.0), .height = B::Len(300.0), .bleed = [](Ctx const& c) { return Value::Bool(true); }}, ",
        "B::Positioned(p, c, {.x = B::Len(10.0), .y = B::Len(20.0), .width = B::Len(100.0), .height = B::Len(40.0), .animate = &M_smooth}, ",
        "B::Marquee(p, c, {.duration = B::Len(8.0), .phase = 0.2}, ",
        ".lines = 2.0, .fontSize = B::Len(12.0), .lineHeight = B::Len(16.0)}",
        "B::Accordion(p, c, {.count = B::Len(3.0), .selected = B::Len(1.0), .collapsedWidth = B::Len(40.0), .height = B::Len(120.0), .corner = B::Len(12.0), .animate = &M_bouncy}, ",
        "B::Flow(p, c, {.gap = B::Len(4.0)}, ",
        ".iconOnly = true}",
        ".rowGap = B::Len(6.0), .cellRatio = 0.667, .cellExtra = B::Len(20.0)}",
        ".corner = B::Len(8.0), .templated = true, .aspect = 1.5}",
        "B::Image(p, c, {.source = \"images/bern.png\", .darkSource = \"images/bern-dark.png\", .width = B::Len(24.0), .height = B::Len(24.0)});",
        "B::Image(p, c, {.source = \"images/bern.png\", .width = B::Len(48.0), .height = B::Len(27.0), .flies = true});",
        ".align = \"baseline\"",
        "B::MenuLook{.iconOnly = true, .shape = B::ControlShape{.capsule = true, .height = B::Len(30.0)}}",
        "B::ControlGroup(p, c, {.height = B::Len(32.0)}, ",
        "B::SearchItem(p, c, {.id = \"test_searchitem\"",
        "B::Form(p, c, nullptr, {.gap = 12.0, .rowHeight = 44.0}, ",
        "B::Section(p, c, nullptr, nullptr, {.plain = true, .paddingX = 8.0}, ",
        "B::Modal(p, c, {.width = B::Len(300.0), .height = B::Len(200.0), .cover = true}, ",
        "B::Modal(p, c, {.width = B::Len(280.0)}, ",
        ".underBars = true}",
        "B::ContentButton(p, c, {.id = \"test_button\"",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`");
    }
    compile(&output);
}

/// The showcase with a button around a picture whose progress runs along
/// its foot, in a column wider than the picture, and a vertical rail beside
/// a bar.
fn add_rails_in_stacks(app: &mut bern_ir::Application) {
    let bar = |size: &str, length: f64| {
        node(
            "Canvas",
            vec![
                ("from", number(0.0)),
                ("to", number(0.86)),
                (size, number(length)),
            ],
            vec![node(
                "Shape",
                vec![("kind", word("rect")), ("fill", word("accent"))],
                vec![],
            )],
        )
    };
    let column = node(
        "Column",
        vec![("width", number(296.0))],
        vec![
            node(
                "Button",
                vec![("label", words("Playing"))],
                vec![node(
                    "Stack",
                    vec![("align", word("bottom_leading"))],
                    vec![
                        node(
                            "Image",
                            vec![
                                (
                                    "source",
                                    bern_ir::Prop::Resource {
                                        name: "images/bern.png".into(),
                                    },
                                ),
                                ("width", number(208.0)),
                                ("height", number(117.0)),
                                ("corner", number(8.0)),
                            ],
                            vec![],
                        ),
                        node("Rail", vec![], vec![bar("height", 3.0)]),
                    ],
                )],
            ),
            node(
                "Stack",
                vec![("align", word("top_leading"))],
                vec![
                    node(
                        "Canvas",
                        vec![("width", number(40.0)), ("height", number(120.0))],
                        vec![node("Shape", vec![("kind", word("rect"))], vec![])],
                    ),
                    node(
                        "Rail",
                        vec![("axis", word("vertical"))],
                        vec![bar("width", 3.0)],
                    ),
                ],
            ),
        ],
    );
    let component = app
        .components
        .iter_mut()
        .find(|component| {
            matches!(&component.body, bern_ir::Node::Element(body) if body.kind == "Form")
        })
        .expect("a component whose body is a form");
    let bern_ir::Node::Element(body) = &mut component.body else {
        unreachable!()
    };
    body.children.push(column);
}

/// A rail among a stack's layers is their backdrop: the stack takes the
/// picture's size, not the column's width, and the rail along the
/// picture's foot (or beside the bar) is as long as it. The rail asks for
/// no length of its own where it is given one; a vertical rail fills only
/// along its axis, as a horizontal one does.
#[test]
fn a_rail_in_a_stack_takes_the_size_of_its_other_layers() {
    let (config, mut compiled) = showcase();
    add_rails_in_stacks(&mut compiled.app);
    let problems = WinUi3Windows.check(&compiled.app, &windows_target(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let root = workspace();
    let output = root.join("target/bern-test/winui3-rails-in-stacks");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    assert!(!ui.contains("unsupported"), "{ui}");
    let order = [
        "B::Stack(p, c, {.axis = \"column\", .width = B::Len(296.0)}, ",
        "B::ContentButton(p, c, {.id = \"test_button\"",
        "B::Layers(p, c, {.align = \"bottom_leading\"}, ",
        ".width = B::Len(208.0), .height = B::Len(117.0)",
        "B::Rail(p, c, {}, ",
        "B::Place(p, c, {.from = B::Len(0.0), .to = B::Len(0.86)}, ",
        "B::Canvas(p, c, {.height = B::Len(3.0)}, ",
        "B::Layers(p, c, {.align = \"top_leading\"}, ",
        "B::Canvas(p, c, {.width = B::Len(40.0), .height = B::Len(120.0)}, ",
        "B::Rail(p, c, {.axis = \"vertical\"}, ",
        "B::Canvas(p, c, {.width = B::Len(3.0)}, ",
    ];
    let mut at = 0;
    for expected in order {
        let found = ui[at..]
            .find(expected)
            .unwrap_or_else(|| panic!("missing `{expected}` after the earlier parts"));
        at += found + expected.len();
    }
    // The runtime the calls go to: a rail is a backdrop among layers, asks
    // for no length where one is given, and a vertical one fills along its
    // axis only.
    let core = std::fs::read_to_string(output.join("app/runtime/bern_ui_core.inl")).unwrap();
    let motion = std::fs::read_to_string(output.join("app/runtime/bern_ui_motion.inl")).unwrap();
    for (runtime, expected) in [
        (
            &core,
            "return role.starts_with(L\"bern-rail\") || role == L\"bern-canvas\";",
        ),
        (
            &core,
            "role == L\"bern-rail-h\" || role == L\"bern-rail-wh\")",
        ),
        (&motion, "float main = std::isinf(length) ? longest : 0;"),
        (
            &motion,
            "impl->vertical ? L\"bern-rail-h\" : L\"bern-rail-w\"",
        ),
    ] {
        assert!(runtime.contains(expected), "missing `{expected}`");
    }
    compile(&output);
}

const FORMATS_PROTOCOL: &str = r#"
#[bern::protocol(namespace = "formats", version = 1)]
pub mod formats {
    use bern::prelude::*;

    #[value]
    pub enum Size {
        #[label(tr! { "en" => "small", "zh-Hans" => "小" })]
        Small,
        #[label(tr! { "en" => "large", "zh-Hans" => "大" })]
        Large,
    }

    #[value]
    pub enum Notice {
        #[label(tr! { "en" => "No notice", "zh-Hans" => "没有通知" })]
        Nothing,
        #[label(tr! { "en" => "Saved {name} on {day, date, EEE, MMM d}", "zh-Hans" => "已保存 {name}，{day, date, M月d日 EEE}" })]
        Saved {
            #[content]
            name: String,
            day: CalendarDate,
        },
        #[label(tr! { "en" => "{n, plural, one {# {size} file} other {# {size} files}}", "zh-Hans" => "{n} 个{size}文件" })]
        Files { n: i64, size: Size },
    }

    #[value]
    pub struct Entry {
        pub id: i64,
        #[content]
        pub name: String,
    }

    #[state]
    pub struct State {
        pub count: i64,
        pub seconds: f64,
        pub today: CalendarDate,
        pub notice: Notice,
        pub entries: Vec<Entry>,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self) -> Result<(), CommandError>;
    }
}
"#;

const FORMATS_UI: &str = r#"
use bern::prelude::*;

#[component]
pub fn app(app: formats::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Formats", "zh-Hans" => "格式" }, initial: open) {
                Column {
                    Text(content: tr! { "en" => "{n, plural, =0 {no files} one {# file} other {# files}}", "zh-Hans" => "{n} 个文件"; n: app.count })
                    Text(content: tr! { "en" => "Today is {d, date, EEEE, MMMM d, y}", "zh-Hans" => "今天是 {d, date, y年M月d日 EEEE}"; d: app.today })
                    Text(content: tr! { "en" => "{t, duration, m:ss} used", "zh-Hans" => "已用 {t, duration, m:ss}"; t: app.seconds })
                    Text(content: tr! { "en" => "Braces {{stay}}", "zh-Hans" => "括号 {{保留}}" })
                    Text(content: app.notice)
                }
            }
            StatusItem(icon: "star", label: tr! { "en" => "Formats: {n}", "zh-Hans" => "格式：{n}"; n: app.count }, login_item: true) {
                CommandItem(id: add, label: tr! { "en" => "Add", "zh-Hans" => "添加" }, checked: app.count > 2) => app.add()
                Divider
                CommandMenu(label: tr! { "en" => "More", "zh-Hans" => "更多" }) {
                    if app.count > 0 {
                        CommandItem(id: add_more, label: tr! { "en" => "Add more", "zh-Hans" => "再添加" }, enabled: app.count < 9) => app.add()
                    } else {
                        Divider
                    }
                }
                ForEach(items: app.entries, key: id) |entry| {
                    CommandItem(id: entry, label: tr! { "en" => "Open {name}", "zh-Hans" => "打开 {name}"; name: entry.name }) => windows.main.open()
                }
                match app.notice {
                    Notice::Saved { name, day } => {
                        CommandItem(id: saved, label: tr! { "en" => "Saved {name}", "zh-Hans" => "已保存 {name}"; name: name }) => app.add()
                    }
                    _ => { Divider }
                }
            }
        }
    }
}
"#;

/// A project with plural branches, dates, durations and labels that show
/// their variant's fields, compiled from sources written under `dir`.
fn formats_project(dir: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("formats-protocol", FORMATS_PROTOCOL),
        ("formats-ui", FORMATS_UI),
        ("formats-core", ""),
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
id = "formats"
bundle_id = "dev.bern.formats"
version = "0.1.0"
name = { "en" = "Formats", "zh-Hans" = "格式" }

[languages]
supported = ["en", "zh-Hans"]
default = "en"

[protocol]
package = "formats-protocol"
source = "formats-protocol/src/lib.rs"

[engine]
package = "formats-core"
type = "formats_core::Engine"

[ui]
package = "formats-ui"
sources = ["formats-ui/src/lib.rs"]
root = "app"

[targets.android]
platform = "android"

[targets.windows]
platform = "windows"
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

/// Texts with plural branches, dates and durations pass typed arguments;
/// the catalog says which fields a label shows; the string tables keep
/// Bern's format for texts with arguments and carry the date names.
#[test]
fn text_formats_generate_for_winui3() {
    let root = workspace();
    let (config, compiled) = formats_project(&root.join("target/bern-test/winui3-formats-project"));
    let problems = WinUi3Windows.check(&compiled.app, &windows_target(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let output = root.join("target/bern-test/winui3-formats");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    for expected in [
        "{\"n\", bern::Text::ArgOf(",
        "{\"d\", bern::Text::DateArg(",
        "{\"t\", bern::Text::ArgOf(",
        // The status item: its menu is built when it opens, branches and
        // rows as C++ control flow.
        "static void StatusMenu(Ctx const& c, StatusArea::Menu& m) {",
        "m.Sub(bern::Text::Tr(",
        "for (auto const& row : R::AsList(",
        "}(c.WithLocal(\"entry\", [row] { return row; }));",
        "auto tag = R::Tag(value);",
        "[value] { return R::Payload(value, 0); }",
        "item.menu = StatusMenu;",
        "item.loginItem = true;",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`:\n{ui}");
    }
    let catalog = std::fs::read_to_string(output.join("app/app_catalog.cpp")).unwrap();
    for expected in [
        "{{\"name\", 0}, {\"day\", 1}}}",
        "{{\"n\", 0}, {\"size\", 1}}}",
        "{n, plural, =0 {no files} one {# file} other {# files}}",
        "Braces {stay}",
        "bern.date.weekday_long.1",
        "Monday",
    ] {
        assert!(catalog.contains(expected), "missing `{expected}`");
    }
    compile(&output);
}

#[test]
fn a_declared_ico_is_used_as_it_is() {
    let root = workspace();
    let (config, mut compiled) = showcase();
    let ico = root.join("target/bern-test/winui3-icon.ico");
    std::fs::create_dir_all(ico.parent().unwrap()).unwrap();
    // One 16-pixel bitmap image, as icon editors write the small sizes.
    let mut bytes = vec![0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0];
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&22u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 40]);
    std::fs::write(&ico, &bytes).unwrap();
    // An absolute path stays itself when joined to the project directory.
    compiled
        .app
        .app
        .platform_icons
        .insert("windows".into(), ico.display().to_string());
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/winui3-icon"),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    let app_ico = files
        .iter()
        .find(|file| file.path == Path::new("app/app.ico"))
        .expect("the icon");
    assert_eq!(app_ico.contents, bytes);
    let _ = std::fs::remove_file(&ico);
}

/// Puts a copy of the showcase's name field in a row after it, its label
/// hidden.
fn add_field_in_row(app: &mut bern_ir::Application) {
    use bern_ir::{Element, Node, Prop};
    fn into(node: &mut Node) -> bool {
        match node {
            Node::Element(element) => {
                let field = element.children.iter().find_map(|child| match child {
                    Node::Element(child)
                        if child.kind == "TextInput" && child.id.as_deref() == Some("name") =>
                    {
                        Some(child.clone())
                    }
                    _ => None,
                });
                let Some(mut beside) = field else {
                    return element.children.iter_mut().any(into);
                };
                beside.id = Some("beside_name".into());
                beside.path = format!("{}.beside", beside.path);
                beside.props.insert(
                    "label_hidden".into(),
                    Prop::Keyword {
                        value: "true".into(),
                    },
                );
                element.children.push(Node::Element(Element {
                    kind: "Row".into(),
                    id: None,
                    path: format!("{}.row", element.path),
                    props: Default::default(),
                    template: None,
                    children: vec![Node::Element(beside)],
                    action: None,
                    source: element.source.clone(),
                }));
                true
            }
            Node::If(node) => {
                node.branches
                    .iter_mut()
                    .any(|(_, body)| body.iter_mut().any(into))
                    || node.otherwise.iter_mut().any(into)
            }
            _ => false,
        }
    }
    assert!(
        app.components
            .iter_mut()
            .any(|component| into(&mut component.body))
    );
}

#[test]
fn a_text_input_can_hide_its_label() {
    let (config, mut compiled) = showcase();
    add_field_in_row(&mut compiled.app);
    let root = workspace();
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/winui3-text-input"),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    let ui = files
        .iter()
        .find(|file| file.path.ends_with("app_ui.inl"))
        .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
        .unwrap();
    let field = |id: &str| {
        ui.lines()
            .find(|line| line.contains(&format!("B::TextInput(p, c, {{.id = \"{id}\"")))
            .unwrap_or_else(|| panic!("`{id}` in {ui}"))
            .to_string()
    };
    assert!(field("beside_name").contains(".labelHidden = true, .field = "));
    assert!(!field("name").contains(".labelHidden"));
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

/// A column without `align` holding a centered column, a styled stack that
/// a condition may give a divider, and what fills across a column.
const FILL_UI: &str = r#"
use bern::prelude::*;

style! {
    Stack.card { background: fill.subtle, corner: 8 }
}

#[component]
pub fn app(app: stage::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Stage" }, initial: open) {
                Column(gap: 12, padding: 24) {
                    Column(align: center) {
                        Text(content: tr! { "en" => "A longer title" })
                        Button(id: short, label: tr! { "en" => "Short" }) => app.add()
                    }
                    Stack(variant: card) {
                        Column(padding: 12) {
                            Text(content: tr! { "en" => "A card" })
                            if app.count > 0 {
                                Divider
                            }
                        }
                    }
                    Divider
                    Progress(value: app.count, total: 10, label: tr! { "en" => "Progress" })
                    Grid(min_width: 80) {
                        Text(content: tr! { "en" => "Cell" })
                    }
                    List {
                        Text(content: tr! { "en" => "Row" })
                    }
                    Row {
                        Text(content: tr! { "en" => "Left" })
                        Spacer
                    }
                    Stack {
                        Canvas(label: tr! { "en" => "Backdrop" }) {
                            Shape(kind: rect, fill: fill.subtle)
                        }
                        Text(content: tr! { "en" => "On a backdrop" })
                    }
                    Button(id: wide, label: tr! { "en" => "Wide" }, width: fill) => app.add()
                }
            }
        }
    }
}
"#;

/// A window that rests after three seconds and holds landscape, over a
/// picture where a resting mouse lets it rest after one.
const IDLE_UI: &str = r#"
use bern::prelude::*;

#[component]
pub fn app(app: stage::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Stage" }, initial: open, idle_after: 3, landscape: true) {
                Button(id: picture, label: tr! { "en" => "Picture" }, idle_area: 1) {
                    Spacer
                } => app.add()
            }
        }
    }
}
"#;

/// A project whose only styled control is written directly in its window,
/// compiled from sources written under `dir`.
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

[targets.windows]
platform = "windows"
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

/// A variant of a control written directly in a window, not in a
/// component, has its style defined as one in a component has.
#[test]
fn a_style_variant_in_a_window_is_defined() {
    let root = workspace();
    let (config, compiled) = stage_project(
        &root.join("target/bern-test/winui3-stage-project"),
        STAGE_UI,
    );
    let problems = WinUi3Windows.check(&compiled.app, &windows_target(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let output = root.join("target/bern-test/winui3-stage");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: windows_target(&config),
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
    for file in WinUi3Windows.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    let definition = "static B::StyleDef const S_buttonStage = ";
    let defined = ui
        .find(definition)
        .unwrap_or_else(|| panic!("missing `{definition}`:\n{ui}"));
    let used = ui
        .find("&S_buttonStage")
        .unwrap_or_else(|| panic!("the button does not use its style:\n{ui}"));
    assert!(defined < used, "{ui}");
    compile(&output);
}

/// Content smaller than the window starts at its top leading corner, as on
/// the other targets: a column spans the window's width and its buttons
/// keep their own (a plain button was stretched across it), and the
/// window's content, also through a condition or a
/// component, goes into the window's own parent, where a stack without a
/// size fills the window.
#[test]
fn content_in_a_window_starts_at_its_top_leading_corner() {
    let root = workspace();
    let (config, compiled) = stage_project(
        &root.join("target/bern-test/winui3-place-project"),
        PLACE_UI,
    );
    let problems = WinUi3Windows.check(&compiled.app, &windows_target(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let output = root.join("target/bern-test/winui3-place");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let text = |name: &str| {
        files
            .iter()
            .find(|file| file.path.ends_with(name))
            .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
            .unwrap_or_else(|| panic!("no `{name}`"))
    };
    assert!(text("bern_ui_windows.inl").contains("def.render(Parent::Window(content), ctx);"));
    let widgets = text("bern_ui_widgets.inl");
    assert!(
        widgets.contains(
            "} else if (p.window && !props.width.Given() && !props.height.Given()) {\n        // Written directly in a window, a stack without a size fills it"
        ),
        "a stack in the window fills it"
    );
    // A column without `align` is a leading one; the window stretches its
    // content and leaves a button at its leading edge: before it is on
    // screen a button's alignment is still `Stretch`, so it is marked.
    assert!(widgets.contains(": xaml::HorizontalAlignment::Left;\n        children(Parent::Aligned(panel, h, std::nullopt), ctx);"));
    assert!(widgets.contains("        SetRole(button, L\"bern-fill-width\");\n    } else {\n        KeepWidth(button);\n    }"));
    let core = text("bern_ui_core.inl");
    for expected in [
        "bool stretching = crossH ? *crossH == xaml::HorizontalAlignment::Stretch : window;",
        "if (stretching && !across && RoleOf(Unwrapped(element)) == L\"bern-own-width\") framework.HorizontalAlignment(xaml::HorizontalAlignment::Left);",
    ] {
        assert!(core.contains(expected), "missing `{expected}`");
    }
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    for expected in ["B::When(p, c, ", "B::Layers(p, c, {.align = \"bottom\"}, "] {
        assert!(ui.contains(expected), "missing `{expected}`:\n{ui}");
    }
    compile(&output);
}

#[test]
fn a_column_without_align_is_a_leading_one() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/winui3-fill-project"), FILL_UI);
    let problems = WinUi3Windows.check(&compiled.app, &windows_target(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let output = root.join("target/bern-test/winui3-fill");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: windows_target(&config),
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
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let text = |name: &str| {
        files
            .iter()
            .find(|file| file.path.ends_with(name))
            .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
            .unwrap_or_else(|| panic!("no `{name}`"))
    };
    // Without `align` a column leaves its children at its leading edge,
    // with their own width; what fills across fills it, and content a
    // condition adds later makes its containers fill in turn.
    let widgets = text("bern_ui_widgets.inl");
    for expected in [
        ": xaml::HorizontalAlignment::Left;\n        children(Parent::Aligned(panel, h, std::nullopt), ctx);",
        // A stack with a backdrop fills across a column, not a row.
        "            if (!p.IsHorizontal()) {\n                auto role = std::wstring(RoleOf(layers));",
        "SetRole(layers, h ? L\"bern-layers-wh\" : L\"bern-layers-w\");",
        // A progress bar spans the column.
        "        auto panel = Stacked(label, bar);\n        SetRole(panel, L\"bern-fill-width\");",
        // A divider, a grid and a filling button already fill.
        "        line.Height(1);\n        SetRole(line, L\"bern-fill-width\");",
        "    SetRole(panel, L\"bern-fill-width\");",
        "        SetRole(button, L\"bern-fill-width\");",
    ] {
        assert!(widgets.contains(expected), "missing `{expected}`");
    }
    let structure = text("bern_ui_structure.inl");
    for expected in [
        "    SetRole(rows, L\"bern-fill-width\");\n    children(Parent::OfList(rows), ctx);",
        "    MarkFullWidth(holder);",
        "        SetRole(column, L\"bern-fill-width\");",
        "            SetRole(line, L\"bern-fill-width\");\n            line.ColumnSpacing(8);",
    ] {
        assert!(structure.contains(expected), "missing `{expected}`");
    }
    let core = text("bern_ui_core.inl");
    for expected in [
        "if (role == L\"bern-column\" && FullWidth(inner)) return true;",
        "else if (crossH && (across || !keepsH)) framework.HorizontalAlignment(across ? xaml::HorizontalAlignment::Stretch : *crossH);",
        "        if (panel && (across || down)) Refill(panel);",
        "        } else if (role == L\"bern-column\") {\n            if (across) element.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);",
    ] {
        assert!(core.contains(expected), "missing `{expected}`");
    }
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    for expected in [
        "B::Stack(p, c, {.axis = \"column\", .gap = B::Len(12.0), .padding = B::Len(24.0)}, ",
        "B::Stack(p, c, {.axis = \"column\", .align = \"center\"}, ",
        "B::Divider(p, c);",
        "B::List(p, c, ",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`:\n{ui}");
    }
    compile(&output);
}

/// The picture is an idle area of its window; neither it nor `landscape`,
/// which a desktop window has nothing to do with, warns.
#[test]
fn an_idle_area_is_watched_by_its_window() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/winui3-idle-project"), IDLE_UI);
    let problems = WinUi3Windows.check(&compiled.app, &windows_target(&config));
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/winui3-idle");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: windows_target(&config),
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
    for file in WinUi3Windows.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/app_ui.inl")).unwrap();
    for expected in [
        "B::Decorate(p, c, {.idleArea = 1.0}, ",
        "w.idleAfter = 3.0;",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`:\n{ui}");
    }
    compile(&output);
}
