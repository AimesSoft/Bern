//! Generate the showcase for `gtk + linux` and, where GTK 4 and
//! libadwaita are installed, type-check the generated application, so
//! generator mistakes fail here rather than in `bern build`.

use bern_backend_gtk::{GtkLinux, product_name};
use bern_ir::Application;
use bern_target_api::{BuildContext, BuildStep, Framework, Registry, Target, TargetConfig};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
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

fn linux_target(config: &bern_compiler::Config) -> TargetConfig {
    config
        .targets
        .iter()
        .find(|target| target.platform == "linux")
        .expect("the showcase declares a Linux target")
        .clone()
}

#[test]
fn linux_resolves_to_gtk_by_default() {
    let mut registry = Registry::new();
    registry.register(Box::new(GtkLinux));
    let config = TargetConfig {
        name: "linux".into(),
        platform: "linux".into(),
        ..TargetConfig::default()
    };
    assert_eq!(
        registry.resolve(&config).unwrap().descriptor().id,
        "gtk-linux"
    );
    let explicit = TargetConfig {
        ui: Some("flutter".into()),
        ..config
    };
    let error = registry.resolve(&explicit).err().unwrap();
    assert!(
        error.message.contains("flutter + linux"),
        "{}",
        error.message
    );
}

#[test]
fn languages_without_runtime_texts_are_rejected() {
    let (config, compiled) = showcase();
    let mut app: Application = compiled.app;
    app.languages.supported.push("ko".into());
    let problems = GtkLinux.check(&app, &linux_target(&config));
    assert!(
        problems
            .iter()
            .any(|problem| problem.message.contains("no `ko` translation")),
        "{problems:?}"
    );
}

#[test]
fn showcase_generates_a_gtk_application() {
    let root = workspace();
    let (config, compiled) = showcase();
    let target_config = linux_target(&config);
    let problems = GtkLinux.check(&compiled.app, &target_config);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(product_name(&compiled.app), "Showcase");
    let output = root.join("target/bern-test/gtk");
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
    let files = GtkLinux.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }

    // The engine learns the declared resource names and the channel.
    let main = std::fs::read_to_string(output.join("app/src/main.rs")).unwrap();
    assert!(
        main.contains(
            r#"resources: vec!["images/bern.png".into(), "images/hdr-gainmap.jpg".into(), "images/hdr-pq.png".into(), "images/landscape.jpg".into(), "media/aurora.mp4".into(), "media/broken.mp4".into(), "media/tides.mp4".into()]"#
        ),
        "{main}"
    );
    assert!(main.contains("ipc: true"), "{main}");
    let manifest = std::fs::read_to_string(output.join("build.json")).unwrap();
    for expected in [
        r#""ui": "gtk""#,
        r#""backend": "gtk-mediafile""#,
        r#""media/tides.mp4""#,
    ] {
        assert!(manifest.contains(expected), "{manifest}");
    }
    let desktop =
        std::fs::read_to_string(output.join("Showcase/dev.bern.showcase.desktop")).unwrap();
    assert!(desktop.contains("Name[zh_CN]=Bern 控件展示"), "{desktop}");
    // Without `[app].icon` the Bern logo sits beside the executable, named
    // after the application, and the desktop entry shows it.
    let icon = output.join("Showcase/icons/dev.bern.showcase.png");
    let png = std::fs::read(&icon).unwrap();
    assert_eq!(bern_target_api::icon::png_size(&png), Some((512, 512)));
    // Desktop entries write paths with `/`, also when generated on Windows.
    let written = icon.display().to_string().replace('\\', "/");
    assert!(desktop.contains(&format!("Icon={written}\n")), "{desktop}");

    // Every component is realized; without style overrides, colors come
    // from the system style.
    let ui = std::fs::read_to_string(output.join("app/src/ui.rs")).unwrap();
    assert!(!ui.contains("unsupported"), "{ui}");
    // The tool strip: in the flow, a spacer takes the rest of its line and
    // the chips are items (the runtime adds them to the flow).
    let strip = &ui[ui
        .find("b::Flow(p, c, b::FlowProps { gap: Some(b::Num::Fixed(10.0)) }")
        .unwrap()..];
    let spacer = strip.find("b::Spacer(p);").unwrap();
    let chips = strip.find("id: \"strip_filter\"").unwrap();
    assert!(spacer < chips, "{strip}");
    assert!(
        strip[chips..].contains("style: Some(\"chips\"), label_hidden: true"),
        "{strip}"
    );
    for forbidden in ["CssProvider", "rgb(", "@define-color", "gdk::RGBA"] {
        assert!(
            !ui.contains(forbidden),
            "generated code without style overrides uses `{forbidden}`"
        );
    }
    for expected in [
        "b::SplitView(p, c, b::SplitSpec {",
        "b::Navigation(p, c, Some(\"library\")",
        "b::Query(p, c, b::QueryProps { id: Some(\"library_results\")",
        "b::Adaptive(p, c, Some(\"clip_choice\")",
        "b::Tabs(p, c, Some(\"tabs\")",
        "b::Video(p, c, b::VideoProps { id: \"preview\", source: None, player: Some(\"preview\")",
        "fn close_settings(c: &b::Ctx) -> b::CloseCheck",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`");
    }

    // The plan builds the application, ships the resources next to the
    // executable and copies the entries.
    let plan = GtkLinux.plan(&compiled.app, &context).unwrap();
    let BuildStep::Run { program, .. } = &plan.steps[0] else {
        panic!("the plan starts with cargo");
    };
    assert_eq!(program, "cargo");
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
            output.join("Showcase/showcase"),
            output.join("bin/showcase"),
            output.join("bin/showcase-mcp"),
        ]
    );

    // Type-check the generated application where GTK is installed.
    let gtk = Command::new("pkg-config")
        .args(["--exists", "gtk4 >= 4.14", "libadwaita-1 >= 1.5"])
        .status()
        .is_ok_and(|status| status.success());
    if !gtk {
        eprintln!("GTK 4.14 and libadwaita 1.5 not installed; skipping the type check");
        return;
    }
    let result = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(output.join("app/Cargo.toml"))
        .arg("--target-dir")
        .arg(root.join("target/bern-glue-linux"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "rustc rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn example(name: &str) -> (bern_compiler::Config, Application) {
    let root = workspace();
    let config =
        bern_compiler::config::load(&root.join("examples").join(name).join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    (config, compiled.app)
}

fn linux() -> TargetConfig {
    TargetConfig {
        name: "linux".into(),
        platform: "linux".into(),
        ui: None,
        web_mode: None,
        channels: Vec::new(),
        frontends: Vec::new(),
    }
}

/// The planner and the gallery use styles, drawing, motion, the media
/// application components and the v0.29 and v0.30 features: GTK builds all
/// of them, and draws them with generated tables.
#[test]
fn planner_and_gallery_build_for_gtk() {
    for name in ["planner", "gallery"] {
        let (_, app) = example(name);
        let problems = GtkLinux.check(&app, &linux());
        assert!(
            problems.iter().all(|problem| !problem.is_error()),
            "{name}: {problems:?}"
        );
        assert!(problems.is_empty(), "{name} warns on GTK: {problems:?}");
        let ui = bern_backend_gtk::Generator::new(&app)
            .generate(0, "bern_gtk::protocol::ProtocolDescriptor::default()");
        assert!(!ui.contains("unsupported"), "{name}: {ui}");
        for expected in [
            "static STYLES: &[b::StyleDef] = &[",
            "const M_JELLY: b::Curve",
            "b::Canvas(p, c,",
        ] {
            assert!(ui.contains(expected), "{name} lacks `{expected}`");
        }
    }
    let (_, planner) = example("planner");
    let ui = bern_backend_gtk::Generator::new(&planner).generate(0, "x");
    // A dashed outline.
    for expected in [
        "dash: Some(4.5), dash_gap: Some(3.0)",
        "dash: None, dash_gap: None",
    ] {
        assert!(ui.contains(expected), "planner lacks `{expected}`");
    }
    let (_, gallery) = example("gallery");
    let ui = bern_backend_gtk::Generator::new(&gallery).generate(0, "x");
    for expected in [
        // Cards: a style with a corner and a shadow, shown on a button that
        // shows its content.
        "b::StyleDef { name: \"buttonCard\"",
        "b::ContentButton(p, c, b::ContentButtonProps { id: \"open_swatch\"",
        "reveal: true",
        "appearance: Some(\"dark\")",
        "b::Inspector(p, c, b::InspectorProps",
        "b::Rail(p, c, b::RailProps { vertical: false, anchor: Some(\"start\"), overhang: None }",
        "b::Place(p, c, b::PlaceProps { at: Some(b::Num::Bound(",
        "b::ScrollView(p, c, b::ScrollProps { axis: Some(\"horizontal\"), snap: Some(\"center\")",
        "b::PopoverButton(p, c,",
        "b::SplitButton(p, c,",
        "b::Drop(p, c,",
        "b::Pinch(p, c,",
        "b::Swipe(p, c,",
        "b::Toast(p, c,",
        // Cards take turns appearing.
        "\"swatch\", Some(0.04), ",
        "b::shortcut(c, \"cmd+k\", b::act(|c| b::focus_target(c, \"find\")));",
        "b::session_set(c, b::Field { session: 0, field: 2 }, V::Bool(false))",
        "c.window(\"idle\")",
        "c.window(\"large\")",
        "dialog: true",
        "idle_after: Some(3.0)",
        "theme: Some(b::ThemeDef {",
        "b::Source::Url(b::url(&c.engine(",
        // Escape closes the details; the dialog's focus starts on its
        // button; the margins ask whether the window is a television's
        // (never, on a desktop).
        "b::KeyShortcut(p, c, b::KeyShortcutProps { keys: \"escape\", enabled: None }",
        "b::default_focus(c, ",
        "c.window(\"tv\")",
        // Keys that type into the search, which suggests.
        "b::session_push_str(c, ",
        "b::session_pop(c, ",
        "suggest: true",
    ] {
        assert!(ui.contains(expected), "the gallery lacks `{expected}`");
    }
}

/// Style effects GTK cannot show fail the check at the rule, unless the
/// rule exempts Linux.
#[test]
fn style_effects_gtk_cannot_show_fail_the_check() {
    let (_, mut app) = example("planner");
    let rule = app
        .style
        .rules
        .iter_mut()
        .find(|rule| rule.component == "SidebarItem")
        .expect("the planner styles its sidebar items");
    // Sidebar rows take no shadow on any target.
    rule.states[0]
        .effects
        .push(bern_ir::style::StyleEffect::Shadow {
            radius: 4.0,
            y: 1.0,
            opacity: 0.2,
        });
    let errors = GtkLinux.check(&app, &linux());
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("cannot show `shadow` on `SidebarItem`")),
        "{errors:?}"
    );
    for rule in &mut app.style.rules {
        rule.exempt.push("linux".into());
    }
    let errors = GtkLinux.check(&app, &linux());
    assert!(
        !errors
            .iter()
            .any(|error| error.message.contains("cannot show")),
        "{errors:?}"
    );
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
fn tab_accessory_slides_up_under_the_pages() {
    let (config, mut compiled) = showcase();
    add_tab_accessory(&mut compiled.app);
    let root = workspace();
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/gtk-tab-accessory"),
        target: linux_target(&config),
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
    let files = GtkLinux.generate(&compiled.app, &context).unwrap();
    let source = files
        .iter()
        .filter(|f| f.path.extension().is_some_and(|e| e == "rs"))
        .map(|f| String::from_utf8_lossy(&f.contents).into_owned())
        .collect::<String>();
    assert!(
        source.contains("Some(b::TabAccessory { shown: Some(b::get(|c| "),
        "{source}"
    );
    // Type-check it against the runtime where GTK is installed.
    let gtk = Command::new("pkg-config")
        .args(["--exists", "gtk4 >= 4.14", "libadwaita-1 >= 1.5"])
        .status()
        .is_ok_and(|status| status.success());
    if !gtk {
        eprintln!("GTK 4.14 and libadwaita 1.5 not installed; skipping the type check");
        return;
    }
    let output = &context.output_dir;
    let _ = std::fs::remove_dir_all(output);
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let result = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(output.join("app/Cargo.toml"))
        .arg("--target-dir")
        .arg(root.join("target/bern-glue-linux"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "rustc rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// An element of a test declaration.
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

/// The showcase with a scene, an accordion, wrapping rows, a grid of
/// cells, a modal, a form's layout, icon-only labels and menus, capsule
/// controls and a search item at the end of its first component.
fn add_layout_features(app: &mut Application) {
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
                vec![node(
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
                )],
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
                vec![node(
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
                )],
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
/// grids, modals and form layouts generate the runtime's elements, and
/// the application type-checks against the runtime where GTK is installed.
#[test]
fn layout_features_generate_for_gtk() {
    let (config, mut compiled) = showcase();
    add_layout_features(&mut compiled.app);
    let problems = GtkLinux.check(&compiled.app, &linux_target(&config));
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    let ui = bern_backend_gtk::Generator::new(&compiled.app).generate(0, "x");
    assert!(!ui.contains("unsupported"), "{ui}");
    for expected in [
        "b::Stack(p, c, b::StackProps { axis: \"column\", gap: None, padding: b::Padding { all: None, x: Some(b::Num::Fixed(12.0)), y: Some(b::Num::Fixed(4.0)) }",
        "b::ScaleBox(p, c, b::ScaleBoxProps { width: Some(b::Num::Fixed(400.0)), height: Some(b::Num::Fixed(300.0)) }",
        "b::Positioned(p, c, b::PositionedProps { x: Some(b::Num::Fixed(10.0)), y: Some(b::Num::Fixed(20.0)), width: Some(b::Num::Fixed(100.0)), height: Some(b::Num::Fixed(40.0)), animate: Some(M_SMOOTH) }",
        "b::Marquee(p, c, b::MarqueeProps { duration: Some(b::Num::Fixed(8.0)), phase: 0.2, paused: None, opacity: None }",
        "lines: Some(2.0), font_size: Some(b::Num::Fixed(12.0)), line_height: Some(b::Num::Fixed(16.0))",
        "b::Accordion(p, c, b::AccordionProps { count: Some(b::Num::Fixed(3.0)), selected: Some(b::Num::Fixed(1.0))",
        "b::Flow(p, c, b::FlowProps { gap: Some(b::Num::Fixed(4.0)) }",
        "icon_only: true",
        "cells: Some((0.667, b::Num::Fixed(20.0)))",
        "corner: Some(b::Num::Fixed(8.0)), template: true, aspect: Some(1.5)",
        "align: Some(\"baseline\")",
        "icon_only: true, shape: b::Shape { capsule: true, height: Some(b::Num::Fixed(30.0)) }",
        "b::ControlGroup(p, c, b::ControlGroupProps { label: None, height: Some(b::Num::Fixed(32.0)) }",
        "b::SearchItem(p, c, b::SearchItemProps { id: \"test_searchitem\"",
        "b::Form(p, c, b::FormProps { title: None, gap: Some(12.0), row_height: Some(44.0)",
        "plain: true, padding_x: Some(8.0)",
        "b::Modal(p, c, b::ModalProps { width: Some(b::Num::Fixed(300.0)), height: Some(b::Num::Fixed(200.0)), cover: true, decor: None }",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`");
    }
    // Type-check it against the runtime where GTK is installed.
    let gtk = Command::new("pkg-config")
        .args(["--exists", "gtk4 >= 4.14", "libadwaita-1 >= 1.5"])
        .status()
        .is_ok_and(|status| status.success());
    if !gtk {
        eprintln!("GTK 4.14 and libadwaita 1.5 not installed; skipping the type check");
        return;
    }
    let root = workspace();
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/gtk-layout-features"),
        target: linux_target(&config),
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
    let files = GtkLinux.generate(&compiled.app, &context).unwrap();
    let output = &context.output_dir;
    let _ = std::fs::remove_dir_all(output);
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let result = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(output.join("app/Cargo.toml"))
        .arg("--target-dir")
        .arg(root.join("target/bern-glue-linux"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "rustc rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// The first bytes of a PNG of `size` × `size` pixels.
fn png(size: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&size.to_be_bytes());
    bytes.extend_from_slice(&size.to_be_bytes());
    bytes
}

#[test]
fn a_declared_icon_theme_ships_under_the_application_name() {
    let root = workspace();
    let (config, mut compiled) = showcase();
    let theme = root.join("target/bern-test/gtk-icon-theme");
    let _ = std::fs::remove_dir_all(&theme);
    for (path, contents) in [
        ("48x48/apps/art.png", png(48)),
        ("256x256/apps/art.png", png(256)),
        ("scalable/apps/art.svg", b"<svg/>".to_vec()),
        ("symbolic/apps/art-symbolic.svg", b"<svg/>".to_vec()),
    ] {
        let path = theme.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    // An absolute path stays itself when joined to the project directory.
    compiled
        .app
        .app
        .platform_icons
        .insert("linux".into(), theme.display().to_string());
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/gtk-icon"),
        target: linux_target(&config),
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
    let files = GtkLinux.generate(&compiled.app, &context).unwrap();
    let file = |path: &str| {
        files
            .iter()
            .find(|file| file.path == Path::new("Showcase").join(path))
            .map(|file| file.contents.clone())
            .unwrap_or_else(|| panic!("no generated {path}"))
    };
    // The largest PNG beside the executable; every icon in the theme,
    // named after the application.
    assert_eq!(file("icons/dev.bern.showcase.png"), png(256));
    assert_eq!(
        file("icons/hicolor/48x48/apps/dev.bern.showcase.png"),
        png(48)
    );
    assert_eq!(
        file("icons/hicolor/scalable/apps/dev.bern.showcase.svg"),
        b"<svg/>"
    );
    assert_eq!(
        file("icons/hicolor/symbolic/apps/dev.bern.showcase-symbolic.svg"),
        b"<svg/>"
    );
    let _ = std::fs::remove_dir_all(&theme);
}

/// `app` with its first icon replaced by `name`.
fn with_icon(app: &Application, name: &str) -> Application {
    fn replace(value: &mut serde_json::Value, name: &str) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                if map.get("kind").and_then(|kind| kind.as_str()) == Some("icon") {
                    map.insert("name".into(), name.into());
                    return true;
                }
                map.values_mut().any(|child| replace(child, name))
            }
            serde_json::Value::Array(items) => items.iter_mut().any(|child| replace(child, name)),
            _ => false,
        }
    }
    let mut value = serde_json::to_value(app).unwrap();
    assert!(replace(&mut value, name), "the application has no icon");
    serde_json::from_value(value).unwrap()
}

/// The runtime draws every name of the Bern icon set from the theme; a
/// name its table lacks fails the check at the element instead of showing
/// `image-missing` when the application runs.
#[test]
fn icons_the_table_lacks_fail_the_check() {
    for name in bern_target_api::icon_set::NAMES {
        assert!(bern_backend_gtk::icons::has(name), "{name}");
    }
    let (_, gallery) = example("gallery");
    for name in [
        "film.stack",
        "server.rack",
        "sparkles",
        "puzzlepiece.extension",
        "arrow.triangle.2.circlepath",
        "wifi",
        "no.such.symbol",
    ] {
        let problems = GtkLinux.check(&with_icon(&gallery, name), &linux());
        let rejected = problems.iter().any(|problem| {
            problem.is_error()
                && problem
                    .message
                    .starts_with(&format!("the GTK icon set has no `{name}`"))
        });
        assert_eq!(rejected, name == "no.such.symbol", "{name}: {problems:?}");
    }
    let ui = bern_backend_gtk::Generator::new(&with_icon(&gallery, "film.stack"))
        .generate(0, "bern_gtk::protocol::ProtocolDescriptor::default()");
    assert!(ui.contains("\"film.stack\""));
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

/// A drop area that opens videos, around a grid that asks to `reflow`,
/// written directly in the window.
const DROP_UI: &str = r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {
    pub file: MediaSource,
}

#[component]
pub fn app(app: stage::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Stage" }, initial: open) {
                Drop(id: clip, label: tr! { "en" => "Drop a Video" }, value: draft.file, files: "video", opens: true) {
                    Grid(min_width: 120, reflow: smooth) {
                        Text(content: tr! { "en" => "Clip" })
                    }
                } => app.add()
            }
        }
    }
}
"#;

/// A project of the interface `ui` over a small protocol, compiled from
/// sources written under `dir`.
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

[targets.linux]
platform = "linux"
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

/// The context building a stage project into `output`.
fn stage_context(
    config: &bern_compiler::Config,
    compiled: &bern_compiler::Compiled,
    output: &Path,
) -> BuildContext {
    let root = workspace();
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.to_path_buf(),
        target: linux_target(config),
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    }
}

/// A variant of a control written directly in a window, not in a
/// component, has its style in the table as one in a component has.
#[test]
fn a_style_variant_in_a_window_is_defined() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/gtk-stage-project"), STAGE_UI);
    let target = linux_target(&config);
    let problems = GtkLinux.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/gtk-stage");
    let _ = std::fs::remove_dir_all(&output);
    let context = stage_context(&config, &compiled, &output);
    for file in GtkLinux.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/src/ui.rs")).unwrap();
    for expected in [
        "static STYLES: &[b::StyleDef] = &[\n    b::StyleDef { name: \"buttonStage\", ",
        "b::Button(p, c, b::ButtonProps { id: \"mini\", ",
        "style: Some(&STYLES[0])",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`:\n{ui}");
    }
}

/// A drop area that opens files, written directly in a window rather than
/// in a component, still has the desktop entry offer the application for
/// its kinds.
#[test]
fn a_drop_in_a_window_registers_its_kinds() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/gtk-drop-project"), DROP_UI);
    let output = root.join("target/bern-test/gtk-drop");
    let _ = std::fs::remove_dir_all(&output);
    let context = stage_context(&config, &compiled, &output);
    for file in GtkLinux.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let entry = std::fs::read_to_string(output.join("Stage/dev.bern.stage.desktop")).unwrap();
    for expected in ["/stage %U\n", "MimeType=video/mp4;video/x-matroska;"] {
        assert!(entry.contains(expected), "missing `{expected}`:\n{entry}");
    }
}

/// `reflow` GTK cannot show warns in a window's own content as it does in
/// a component.
#[test]
fn reflow_in_a_window_is_checked() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/gtk-reflow-project"), DROP_UI);
    let problems = GtkLinux.check(&compiled.app, &linux_target(&config));
    assert!(
        problems.iter().any(|problem| problem
            .message
            .ends_with("the items of this `Grid` move at once")),
        "{problems:?}"
    );
}

/// Content smaller than the window starts at its top leading corner, as on
/// the other targets. The runtime does it: the window's content, also
/// through a condition or a component, goes into the window's own parent,
/// which makes a stack without a size fill the window and leaves a button
/// at the leading edge of a column that stretches the rest (the runtime's
/// own tests check that).
#[test]
fn content_in_a_window_starts_at_its_top_leading_corner() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/gtk-place-project"), PLACE_UI);
    let problems = GtkLinux.check(&compiled.app, &linux_target(&config));
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/gtk-place");
    let _ = std::fs::remove_dir_all(&output);
    let context = stage_context(&config, &compiled, &output);
    for file in GtkLinux.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/src/ui.rs")).unwrap();
    for expected in [
        "fn w_main(p: &b::Parent, c: &b::Ctx) {\n    b::When(p, c, ",
        "b::Layers(p, c, b::LayersProps { align: Some(\"bottom\"), width: None, height: None, ",
        "c_centered(p, &c.child(\"centered\", vec![]));",
        "b::StackProps { axis: \"column\", gap: Some(b::Num::Fixed(12.0)), padding: b::Padding { all: Some(b::Num::Fixed(24.0)), x: None, y: None }, width: None, height: None, align: None, ",
        "b::ButtonProps { id: \"add\", ",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`:\n{ui}");
    }
}
