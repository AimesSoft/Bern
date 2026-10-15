//! Generate the showcase and type-check the Swift output with the installed
//! toolchain, so generator mistakes fail here rather than in `bern build`.

use bern_backend_swiftui::{MINIMUM_IOS, MINIMUM_MACOS, SwiftUiIos, SwiftUiMacos};
use bern_ir::{Application, ResourceDef, SourceLocation};
use bern_target_api::{BuildContext, BuildPlan, BuildStep, Framework, Target};
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

#[test]
fn resources_must_fit_the_bundle_layout() {
    let (config, compiled) = showcase();
    let mut app: Application = compiled.app;
    let declared = SourceLocation {
        file: "Bern.toml".into(),
        line: 40,
        column: 10,
    };
    for name in [
        "en.lproj/Localizable.strings",
        "notes.lproj",
        "v1.2/clip.mp4",
        "media/clip.v2.mp4",
    ] {
        app.resources.push(ResourceDef {
            name: name.into(),
            declared: declared.clone(),
        });
    }
    let macos = config.targets.iter().find(|t| t.name == "macos").unwrap();
    let problems = SwiftUiMacos.check(&app, macos);
    let messages: Vec<&str> = problems.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(problems.len(), 3, "{messages:?}");
    assert!(messages[0].contains("`en.lproj` is where the macOS bundle keeps localizations"));
    assert!(messages[1].contains("`notes.lproj`"));
    assert!(messages[2].contains("directory `v1.2` contains `.`"));
    assert!(
        problems
            .iter()
            .all(|d| d.location == Some(declared.clone()))
    );
}

#[test]
fn conditional_progress_rails_keep_the_posters_overlay_layout() {
    use bern_ir::{Element, Expr, ExprKind, IfNode, Node, Prop};
    use bern_protocol::{Type, Value};
    let (config, mut compiled) = showcase();
    let element = |kind: &str, props, children| Node::Element(Element {
        kind: kind.into(), id: None, path: format!("test.{kind}"), props,
        template: None, children, action: None,
        source: SourceLocation { file: "poster.rs".into(), line: 1, column: 1 },
    });
    let image = element("Image", [
        ("name".into(), Prop::Resource { name: "images/bern.png".into() }),
        ("aspect".into(), Prop::Number { value: 2.0 / 3.0 }),
    ].into(), vec![]);
    let bar = element("Canvas", [
        ("height".into(), Prop::Number { value: 4.0 }),
    ].into(), vec![]);
    let rail = element("Rail", Default::default(), vec![bar]);
    let conditional = Node::If(IfNode {
        branches: vec![(Expr { ty: Type::Bool, kind: ExprKind::Literal { value: Value::Bool(true) } }, vec![rail])],
        otherwise: vec![],
    });
    // The same image and optional progress bar must still use the sizing
    // contract of overlay layers, even though the Rail is inside an if.
    compiled.app.components[0].body = element("Stack", Default::default(), vec![image, conditional]);
    let output = workspace().join("target/bern-test/conditional-rail");
    let context = context(&config, &compiled, "macos", &output);
    let files = SwiftUiMacos.generate(&compiled.app, &context).unwrap();
    let generated = files.iter().find(|f| f.path == Path::new("Sources/BernGenerated.swift")).unwrap();
    let source = String::from_utf8_lossy(&generated.contents);
    assert!(source.contains("BernLayers(alignment: .center) {"), "{source}");
    assert!(source.contains(".bernFillsLayers()"), "{source}");
}

#[test]
fn showcase_generates_swift_that_type_checks() {
    let root = workspace();
    let (config, compiled) = showcase();
    let names: Vec<&str> = compiled
        .app
        .resources
        .iter()
        .map(|resource| resource.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "images/bern.png",
            "images/hdr-gainmap.jpg",
            "images/hdr-pq.png",
            "images/landscape.jpg",
            "media/aurora.mp4",
            "media/broken.mp4",
            "media/tides.mp4"
        ]
    );
    let output = root.join("target/bern-test/swiftui");
    let _ = std::fs::remove_dir_all(&output);
    let context = context(&config, &compiled, "macos", &output);
    let problems = SwiftUiMacos.check(&compiled.app, &context.target);
    assert!(problems.is_empty(), "{problems:?}");
    let files = SwiftUiMacos.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    // The engine learns the declared names; the plan ships them into the
    // bundle next to the generated localizations, before signing.
    let glue = std::fs::read_to_string(output.join("glue/src/lib.rs")).unwrap();
    assert!(
        glue.contains(
            r#"resources: vec!["images/bern.png".into(), "images/hdr-gainmap.jpg".into(), "images/hdr-pq.png".into(), "images/landscape.jpg".into(), "media/aurora.mp4".into(), "media/broken.mp4".into(), "media/tides.mp4".into()]"#
        ),
        "{glue}"
    );
    let manifest = std::fs::read_to_string(output.join("build.json")).unwrap();
    assert!(manifest.contains(r#""media/tides.mp4""#), "{manifest}");
    // Without `[app].icon` the bundle carries the Bern logo, which the
    // resource mirror keeps.
    let icns = std::fs::read(output.join("Showcase.app/Contents/Resources/AppIcon.icns")).unwrap();
    assert_eq!(&icns[..4], b"icns");
    assert_eq!(
        u32::from_be_bytes([icns[4], icns[5], icns[6], icns[7]]) as usize,
        icns.len()
    );
    let plist = std::fs::read_to_string(output.join("Showcase.app/Contents/Info.plist")).unwrap();
    assert!(
        plist.contains("<key>CFBundleIconFile</key>\n    <string>AppIcon</string>"),
        "{plist}"
    );
    let plan = SwiftUiMacos.plan(&compiled.app, &context).unwrap();
    let mirror = plan
        .steps
        .iter()
        .position(|step| matches!(step, BuildStep::Mirror { .. }))
        .expect("a mirror step");
    let sign = plan
        .steps
        .iter()
        .position(|step| matches!(step, BuildStep::Run { program, .. } if program == "codesign"))
        .unwrap();
    assert!(mirror < sign);
    let BuildStep::Mirror { dir, files, keep } = &plan.steps[mirror] else {
        unreachable!()
    };
    assert_eq!(dir, &output.join("Showcase.app/Contents/Resources"));
    assert!(files.contains(&(
        config.dir.join("media/aurora.mp4"),
        PathBuf::from("media/aurora.mp4")
    )));
    assert!(keep.contains(&PathBuf::from("en.lproj/InfoPlist.strings")));
    assert!(keep.contains(&PathBuf::from("zh-Hans.lproj/Localizable.strings")));

    // Debug symbols never ship in the bundle: the dSYM swiftc writes next to
    // the executable moves beside the bundle before signing, and release
    // builds strip what the dSYM keeps from the executable.
    let symbols = output.join("Showcase.app.dSYM");
    // Programs run, with the tool xcrun runs.
    let programs = |plan: &BuildPlan| -> Vec<String> {
        plan.steps
            .iter()
            .filter_map(|step| match step {
                BuildStep::Run { program, args, .. } if program == "xcrun" => {
                    Some(format!("xcrun {}", args[0]))
                }
                BuildStep::Run { program, .. } => Some(program.clone()),
                _ => None,
            })
            .collect()
    };
    let moved = plan.steps.iter().position(|step| {
        matches!(step, BuildStep::Run { program, args, .. }
            if program == "mv"
                && args[0] == output.join("Showcase.app/Contents/MacOS/Showcase.dSYM").display().to_string()
                && args[1] == symbols.display().to_string())
    });
    assert!(moved.is_some_and(|moved| moved < sign), "{:?}", plan.steps);
    assert!(
        plan.steps
            .iter()
            .any(|step| matches!(step, BuildStep::RemoveIfExists { path } if path == &symbols))
    );
    assert!(plan.products.contains(&symbols));
    assert!(!programs(&plan).contains(&"xcrun strip".to_string()));
    let release = SwiftUiMacos
        .plan(
            &compiled.app,
            &BuildContext {
                release: true,
                ..context.clone()
            },
        )
        .unwrap();
    assert_eq!(
        programs(&release),
        ["cargo", "xcrun swiftc", "mv", "xcrun strip", "codesign"]
    );
    let swiftc = release
        .steps
        .iter()
        .find_map(|step| match step {
            BuildStep::Run { args, .. } if args[0] == "swiftc" => Some(args),
            _ => None,
        })
        .unwrap();
    assert!(
        swiftc
            .windows(2)
            .any(|pair| pair == ["-Xlinker", "-dead_strip"])
    );
    assert!(swiftc.contains(&"-g".to_string()));

    // The showcase declares no style overrides, so it keeps the system look.
    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for forbidden in [
        "Color(",
        ".background(",
        "glassEffect",
        "WKWebView",
        "JSONDecoder",
    ] {
        assert!(
            !generated.contains(forbidden),
            "generated Swift without style overrides uses `{forbidden}`"
        );
    }
    if !Command::new("xcrun")
        .arg("--find")
        .arg("swiftc")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("swiftc not available; skipping type check");
        return;
    }
    let sources = output.join("Sources");
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
        ])
        .arg("-target")
        .arg(format!("arm64-apple-macos{MINIMUM_MACOS}"))
        .arg("-module-cache-path")
        .arg(root.join("target/bern-swift-module-cache"))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernStyle.swift"))
        .arg(sources.join("BernAppKit.swift"))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// The build context of target `name` of the showcase, writing to `output`.
fn context(
    config: &bern_compiler::Config,
    compiled: &bern_compiler::Compiled,
    name: &str,
    output: &Path,
) -> BuildContext {
    let root = workspace();
    let target = config
        .targets
        .iter()
        .find(|target| target.name == name)
        .unwrap_or_else(|| panic!("the showcase declares target `{name}`"))
        .clone();
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.to_path_buf(),
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
    }
}

#[test]
fn ios_resources_must_not_take_the_bundles_own_names() {
    let (config, compiled) = showcase();
    let mut app: Application = compiled.app;
    let declared = SourceLocation {
        file: "Bern.toml".into(),
        line: 40,
        column: 10,
    };
    for name in [
        "Info.plist",
        "Assets.car",
        "Showcase",
        "AppIcon60x60@2x.png",
        "docs/Info.plist",
    ] {
        app.resources.push(ResourceDef {
            name: name.into(),
            declared: declared.clone(),
        });
    }
    let target = config.targets.iter().find(|t| t.name == "ios").unwrap();
    let problems = SwiftUiIos.check(&app, target);
    let messages: Vec<&str> = problems.iter().map(|d| d.message.as_str()).collect();
    // A file of the same name in a directory is no conflict.
    assert_eq!(problems.len(), 4, "{messages:?}");
    assert!(
        messages
            .iter()
            .all(|m| m.contains("is a file of the iOS bundle itself")),
        "{messages:?}"
    );
}

#[test]
fn showcase_generates_ios_swift_that_type_checks() {
    let root = workspace();
    let (config, compiled) = showcase();
    let target = config.targets.iter().find(|t| t.name == "ios").unwrap();
    let problems = SwiftUiIos.check(&compiled.app, target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/swiftui-ios");
    let _ = std::fs::remove_dir_all(&output);
    let context = context(&config, &compiled, "ios", &output);
    let files = SwiftUiIos.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    // Windows are virtual; the channel listens only where `bern run` names
    // a directory the Mac reaches.
    let glue = std::fs::read_to_string(output.join("glue/src/lib.rs")).unwrap();
    // Settings is always virtual; the main window is native where the app
    // says it can open several windows (iPad).
    assert!(
        glue.contains(r#"id: "settings".into(), sessions: vec![1], presentation: bern_ffi::runtime::Presentation::Virtual"#),
        "{glue}"
    );
    assert!(
        glue.contains("presentation: if NATIVE_WINDOWS.load("),
        "{glue}"
    );
    assert!(
        glue.contains("pub extern \"C\" fn bern_app_native_windows(native: u8)"),
        "{glue}"
    );
    assert!(
        glue.contains(r#"ipc: ::std::env::var_os("BERN_IPC_DIR").is_some()"#),
        "{glue}"
    );
    // A flat bundle: Info.plist and the localizations at its top, a launch
    // screen so the app fills the screen, several scenes for iPad windows.
    let plist = std::fs::read_to_string(output.join("Showcase.app/Info.plist")).unwrap();
    for expected in [
        "<key>UILaunchScreen</key>",
        "<key>UIApplicationSupportsMultipleScenes</key>\n        <true/>",
        &format!("<key>MinimumOSVersion</key>\n    <string>{MINIMUM_IOS}</string>"),
        "<key>CFBundleIconName</key>\n            <string>AppIcon</string>",
    ] {
        assert!(plist.contains(expected), "{expected}\n{plist}");
    }
    assert!(
        output
            .join("Showcase.app/zh-Hans.lproj/Localizable.strings")
            .exists()
    );
    let icon =
        std::fs::read(output.join("Assets.xcassets/AppIcon.appiconset/AppIcon.png")).unwrap();
    assert_eq!(bern_target_api::icon::png_size(&icon), Some((1024, 1024)));

    let plan = SwiftUiIos.plan(&compiled.app, &context).unwrap();
    let programs: Vec<String> = plan
        .steps
        .iter()
        .filter_map(|step| match step {
            BuildStep::Run { program, args, .. } if program == "xcrun" => Some(format!(
                "xcrun {}",
                args.iter()
                    .find(|a| !a.starts_with('-') && *a != "iphonesimulator")
                    .unwrap()
            )),
            BuildStep::Run { program, .. } => Some(program.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        programs,
        [
            "cargo",
            "cargo",
            "xcrun actool",
            "xcrun swiftc",
            "mv",
            "codesign"
        ]
    );
    // The engine for the simulator, the command line tools for the Mac.
    let targets: Vec<&String> = plan
        .steps
        .iter()
        .filter_map(|step| match step {
            BuildStep::Run { program, args, .. } if program == "cargo" => args
                .windows(2)
                .find(|pair| pair[0] == "--target")
                .map(|pair| &pair[1]),
            _ => None,
        })
        .collect();
    assert!(targets[0].ends_with("-apple-ios-sim") || targets[0] == "x86_64-apple-ios");
    assert!(targets[1].ends_with("-apple-darwin"));
    assert!(matches!(
        &plan.run,
        Some(bern_target_api::Run::Simulator { bundle_id, .. }) if bundle_id == "dev.bern.showcase"
    ));

    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for expected in [
        "import UIKit",
        r#"id: "main", for: String.self) { _ in"#,
        r#"} defaultValue: { "main" }"#,
        ".defaultSize(width: 920.0, height: 640.0)",
        ".windowResizability(.contentMinSize)",
        ".bernMinimumSize(width: 720.0, height: 480.0)",
        "CommandGroup(replacing: .appSettings)",
        "BernVirtualHost(initial: [])",
        r#"BernSheetChrome(kind: "settings", title: "#,
        ".navigationTitle(bernWindow.title)",
    ] {
        assert!(
            generated.contains(expected),
            "generated Swift lacks `{expected}`"
        );
    }
    for forbidden in ["AppKit", "NSApplication", "Color(", ".background("] {
        assert!(
            !generated.contains(forbidden),
            "generated iOS Swift without style overrides uses `{forbidden}`"
        );
    }
    let Some(sdk) = Command::new("xcrun")
        .args(["--sdk", "iphonesimulator", "--show-sdk-path"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    else {
        eprintln!("no iOS simulator SDK; skipping type check");
        return;
    };
    let architecture = if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    };
    let sources = output.join("Sources");
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
            "-sdk",
        ])
        .arg(sdk)
        .arg("-target")
        .arg(format!("{architecture}-apple-ios{MINIMUM_IOS}-simulator"))
        .arg("-module-cache-path")
        .arg(root.join("target/bern-swift-module-cache"))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernStyle.swift"))
        .arg(sources.join("BernUIKit.swift"))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated iOS code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

const DYNAMIC_PROTOCOL: &str = r#"
#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Dest {
        Home,
        Feed,
        Extra,
    }

    #[value]
    pub struct Entry {
        pub name: String,
        pub dest: Dest,
    }

    #[state]
    pub struct State {
        pub entries: Vec<Entry>,
        pub levels: Vec<i64>,
        pub mode: Dest,
        pub picked: Option<Dest>,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}
"#;

const DYNAMIC_UI: &str = r#"
use bern::prelude::*;

#[bern::session]
pub struct Draft {
    pub dest: demo::Dest,
    pub tab: demo::Dest,
    pub more: bool,
}

#[component]
pub fn app(app: demo::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Main" }, initial: open) {
                page(app: app)
            }
        }
    }
}

#[component]
pub fn page(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! {
        SplitView {
            Sidebar(id: nav, selection: draft.dest, fallback: Dest::Home) {
                SidebarItem(value: Dest::Home, label: tr! { "en" => "Home" })
                if draft.more {
                    SidebarItem(value: Dest::Extra, label: tr! { "en" => "Extra" })
                }
                Section(title: tr! { "en" => "Entries" }) {
                    ForEach(items: app.entries, key: name) |entry| {
                        SidebarItem(value: entry.dest, label: entry.name)
                    }
                }
                match app.mode {
                    Dest::Feed => { SidebarItem(value: Dest::Feed, label: tr! { "en" => "Feed" }) }
                    _ => {}
                }
            }
            Detail {
                Tabs(id: tabs, selection: draft.tab) {
                    Tab(value: Dest::Home, label: tr! { "en" => "Home" }) {
                        Column {
                            match app.mode {
                                Dest::Home => { Text(content: tr! { "en" => "Home content" }, appear: fade) }
                                Dest::Feed => { Text(content: tr! { "en" => "Feed content" }, appear: fade) }
                                Dest::Extra => { Text(content: tr! { "en" => "Extra content" }, appear: fade) }
                            }
                            Switch(id: more, label: tr! { "en" => "More" }, value: draft.more)
                            Canvas(height: 40) {
                                Series(values: app.levels, kind: area)
                            }
                            Text(content: tr! { "en" => "Marked" }, mark: accent)
                            Toolbar {
                                if draft.more {
                                    Button(id: add, label: tr! { "en" => "Add" }) => app.add(1)
                                }
                                ForEach(items: app.entries, key: name) |entry| {
                                    Button(id: open, label: entry.name) => app.add(2)
                                }
                            }
                        }
                    }
                    if let Some(picked) = app.picked {
                        Tab(value: picked, label: tr! { "en" => "Picked" }) {
                            Spacer
                        }
                    }
                }
            }
        }
    }
}
"#;

/// Items of a sidebar, tabs and a toolbar in conditions, a `match` and
/// lists, with a canvas and a marked text: the Swift type-checks.
#[test]
fn dynamic_navigation_items_type_check() {
    check_dynamic_navigation(false);
    check_dynamic_navigation(true);
}

fn check_dynamic_navigation(styled: bool) {
    let root = workspace();
    let dir = root.join(if styled {
        "target/bern-test/dynamic-styled-project"
    } else {
        "target/bern-test/dynamic-project"
    });
    let _ = std::fs::remove_dir_all(&dir);
    for (package, source) in [
        ("demo-protocol", DYNAMIC_PROTOCOL),
        ("demo-ui", DYNAMIC_UI),
        ("demo-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    if styled {
        std::fs::write(
            dir.join("demo-ui/src/lib.rs"),
            format!("{DYNAMIC_UI}\nstyle! {{ SidebarItem {{ hover: background(fill.subtle) + scale(1.03, snappy), selected: fill(fill.strong) }} }}"),
        )
        .unwrap();
    }
    std::fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "demo"
bundle_id = "dev.bern.demo"
version = "0.1.0"
name = { "en" = "Demo" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "demo-protocol"
source = "demo-protocol/src/lib.rs"

[engine]
package = "demo-core"
type = "demo_core::Engine"

[ui]
package = "demo-ui"
sources = ["demo-ui/src/lib.rs"]
root = "app"

[targets.macos]
platform = "macos"
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
    let output = root.join(if styled {
        "target/bern-test/swiftui-dynamic-styled"
    } else {
        "target/bern-test/swiftui-dynamic"
    });
    let _ = std::fs::remove_dir_all(&output);
    let mut context = context(&config, &compiled, "macos", &output);
    context.project_dir = dir.clone();
    let problems = SwiftUiMacos.check(&compiled.app, &context.target);
    assert!(problems.is_empty(), "{problems:?}");
    for file in SwiftUiMacos.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    assert_eq!(generated.contains(".bernSidebarNavigation("), styled);
    // A transition on a match arm must start when the discriminator changes,
    // including when no button/engine command supplies an animation transaction.
    assert!(
        generated.lines().any(|line| line.contains(".animation(")
            && line.contains("value: bernEngine.mode)")),
        "{generated}"
    );
    if styled {
        // Keyboard navigation must see conditional, repeated and matched rows,
        // not just the destinations that existed at initial construction.
        let navigation = generated
            .lines()
            .find(|line| line.contains(".bernSidebarNavigation("))
            .unwrap();
        assert!(
            navigation.contains("values: { () -> [Dest] in"),
            "{navigation}"
        );
        assert!(navigation.contains(".extra"), "{navigation}");
        assert!(navigation.contains(".entries"), "{navigation}");
        assert!(navigation.contains(".feed"), "{navigation}");
    }
    // The selection falls back when its item goes away.
    assert!(
        generated.contains(".onChange(of: { () -> [Dest] in"),
        "{generated}"
    );
    if !Command::new("xcrun")
        .arg("--find")
        .arg("swiftc")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("swiftc not available; skipping type check");
        return;
    }
    let sources = output.join("Sources");
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
        ])
        .arg("-target")
        .arg(format!("arm64-apple-macos{MINIMUM_MACOS}"))
        .arg("-module-cache-path")
        .arg(root.join("target/bern-swift-module-cache"))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernStyle.swift"))
        .arg(sources.join("BernAppKit.swift"))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}


const PALETTE_UI: &str = r#"
use bern::prelude::*;
use demo_protocol::demo;
#[bern::value] pub enum Page { #[label(tr! { "en" => "Home" })] Home }
#[bern::session] pub struct Draft { pub page: Page, pub query: String, pub file: MediaSource }
#[component]
pub fn app(app: demo::Client) -> impl View {
 ui! {
  WindowSet(handle: Windows, primary: main) {
   Window(id: main, initial: open, title: tr! { "en" => "Palette" }) { screen(app: app) }
  }
 }
}
#[component]
pub fn screen(app: demo::Client, #[session] draft: Draft) -> impl View {
 ui! {
   SplitView {
    Sidebar(id: pages, selection: draft.page) {
     SidebarItem(value: Page::Home, label: tr! { "en" => "Home" })
    }
    Detail {
     Toolbar {
      Spacer
      Search(id: search, value: draft.query, style: palette, prompt: tr! { "en" => "Search" }, panel_prompt: tr! { "en" => "Search everything" }, shortcut: "cmd+k", width: 640, height: 480) {
       Column(gap: 0) {
        SearchItem(id: action, label: tr! { "en" => "Action" }, icon: "play", detail: tr! { "en" => "Episode 2" }, hint: tr! { "en" => "Return" }) => app.add(1)
        SearchItem(id: poster, label: tr! { "en" => "Poster" }, navigation: false) { Text(content: tr! { "en" => "Poster" }) } => app.add(2)
        SearchItem(id: file, label: tr! { "en" => "File" }, pick: draft.file, files: "video") => app.add(3)
       }
      }
     }
     Text(content: tr! { "en" => "Home" })
    }
   }
 }
}
"#;

#[test]
fn search_palette_generates_native_controls_and_type_checks() {
    let root = workspace();
    let dir = root.join("target/bern-test/search-palette-project");
    let _ = std::fs::remove_dir_all(&dir);
    for (package, source) in [
        ("demo-protocol", DYNAMIC_PROTOCOL),
        ("demo-ui", PALETTE_UI),
        ("demo-core", ""),
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
id = "demo"
bundle_id = "dev.bern.demo"
version = "0.1.0"
name = { "en" = "Demo" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "demo-protocol"
source = "demo-protocol/src/lib.rs"

[engine]
package = "demo-core"
type = "demo_core::Engine"

[ui]
package = "demo-ui"
sources = ["demo-ui/src/lib.rs"]
root = "app"

[targets.macos]
platform = "macos"
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
    let output = root.join("target/bern-test/swiftui-search-palette");
    let _ = std::fs::remove_dir_all(&output);
    let mut context = context(&config, &compiled, "macos", &output);
    context.project_dir = dir.clone();
    let problems = SwiftUiMacos.check(&compiled.app, &context.target);
    assert!(problems.is_empty(), "{problems:?}");
    for file in SwiftUiMacos.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    assert!(generated.contains("BernSearchPaletteHost {"));
    assert!(generated.contains("BernSearchPalette(text:"));
    assert!(generated.contains("BernSearchItem(target:"));
    assert!(generated.contains("BernPaletteDetail()"));
    assert!(!generated.contains(".modifier(BernSearchable("));
    let errors = SwiftUiIos.check(&compiled.app, &context.target);
    assert!(errors.iter().any(|e| e.message.contains("macOS Toolbar")));
    if !Command::new("xcrun")
        .arg("--find")
        .arg("swiftc")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("swiftc not available; skipping type check");
        return;
    }
    let sources = output.join("Sources");
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
        ])
        .arg("-target")
        .arg(format!("arm64-apple-macos{MINIMUM_MACOS}"))
        .arg("-module-cache-path")
        .arg(root.join("target/bern-swift-module-cache"))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernStyle.swift"))
        .arg(sources.join("BernAppKit.swift"))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
