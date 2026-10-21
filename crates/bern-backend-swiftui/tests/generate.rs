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
    let element = |kind: &str, props, children| {
        Node::Element(Element {
            kind: kind.into(),
            id: None,
            path: format!("test.{kind}"),
            props,
            template: None,
            children,
            action: None,
            source: SourceLocation {
                file: "poster.rs".into(),
                line: 1,
                column: 1,
            },
        })
    };
    let image = element(
        "Image",
        [
            (
                "name".into(),
                Prop::Resource {
                    name: "images/bern.png".into(),
                },
            ),
            ("aspect".into(), Prop::Number { value: 2.0 / 3.0 }),
        ]
        .into(),
        vec![],
    );
    let bar = element(
        "Canvas",
        [("height".into(), Prop::Number { value: 4.0 })].into(),
        vec![],
    );
    let rail = element("Rail", Default::default(), vec![bar]);
    let conditional = Node::If(IfNode {
        branches: vec![(
            Expr {
                ty: Type::Bool,
                kind: ExprKind::Literal {
                    value: Value::Bool(true),
                },
            },
            vec![rail],
        )],
        otherwise: vec![],
    });
    // The same image and optional progress bar must still use the sizing
    // contract of overlay layers, even though the Rail is inside an if.
    compiled.app.components[0].body =
        element("Stack", Default::default(), vec![image, conditional]);
    let output = workspace().join("target/bern-test/conditional-rail");
    let context = context(&config, &compiled, "macos", &output);
    let files = SwiftUiMacos.generate(&compiled.app, &context).unwrap();
    let generated = files
        .iter()
        .find(|f| f.path == Path::new("Sources/BernGenerated.swift"))
        .unwrap();
    let source = String::from_utf8_lossy(&generated.contents);
    assert!(
        source.contains("BernLayers(alignment: .center) {"),
        "{source}"
    );
    assert!(source.contains(".bernFillsLayers()"), "{source}");
}

#[test]
fn rows_give_items_their_widths_and_sized_stacks_start_them() {
    use bern_ir::{Element, Expr, ExprKind, IfNode, Node, Prop};
    use bern_protocol::{Type, Value};
    let (config, mut compiled) = showcase();
    let element = |kind: &str, props, children| {
        Node::Element(Element {
            kind: kind.into(),
            id: None,
            path: format!("test.{kind}"),
            props,
            template: None,
            children,
            action: None,
            source: SourceLocation {
                file: "row.rs".into(),
                line: 1,
                column: 1,
            },
        })
    };
    let text = || {
        element(
            "Text",
            [(
                "content".into(),
                Prop::Text {
                    text: bern_ir::Text::Dynamic {
                        expr: Expr {
                            ty: Type::Text,
                            kind: ExprKind::Literal {
                                value: Value::Text("Tag".into()),
                            },
                        },
                    },
                },
            )]
            .into(),
            vec![],
        )
    };
    let spacer = || element("Spacer", Default::default(), vec![]);
    // A spacer of the row inside a condition, and one of a column inside it.
    let conditional = Node::If(IfNode {
        branches: vec![(
            Expr {
                ty: Type::Bool,
                kind: ExprKind::Literal {
                    value: Value::Bool(true),
                },
            },
            vec![spacer()],
        )],
        otherwise: vec![],
    });
    let column = element(
        "Column",
        [("width".into(), Prop::Number { value: 58.0 })].into(),
        vec![text(), spacer()],
    );
    let divider = element("Divider", Default::default(), vec![]);
    compiled.app.components[0].body = element(
        "Row",
        [
            ("gap".into(), Prop::Number { value: 10.0 }),
            ("width".into(), Prop::Number { value: 224.0 }),
        ]
        .into(),
        vec![text(), conditional, divider, column],
    );
    let output = workspace().join("target/bern-test/row-widths");
    let context = context(&config, &compiled, "macos", &output);
    let files = SwiftUiMacos.generate(&compiled.app, &context).unwrap();
    let generated = files
        .iter()
        .find(|f| f.path == Path::new("Sources/BernGenerated.swift"))
        .unwrap();
    let source = String::from_utf8_lossy(&generated.contents);
    assert!(
        source.contains("BernRow(alignment: .center, spacing: 10.0) {"),
        "{source}"
    );
    let start = source
        .find("BernRow(alignment: .center, spacing: 10.0) {")
        .unwrap();
    let row = &source[start..start + source[start..].find(".frame(width: 224.0").unwrap()];
    // Only the row's own spacer grows along it alone.
    assert_eq!(
        row.matches(".layoutValue(key: BernRowSpacer.self, value: true)")
            .count(),
        1,
        "{row}"
    );
    assert!(row.contains("SwiftUI.HStack(spacing: 0) {"), "{row}");
    // Spacers of the row and of the column shrink to nothing, as on the
    // other targets: the system's least is a gap of 8 points on macOS and
    // 60 on tvOS, which squeezed a narrow row's texts.
    assert_eq!(
        row.matches("SwiftUI.Spacer(minLength: 0)\n").count(),
        2,
        "{row}"
    );
    assert!(!source.contains("SwiftUI.Spacer()"), "{source}");
    assert!(
        source.contains(".frame(width: 58.0, height: nil, alignment: .topLeading)"),
        "{source}"
    );
    assert!(
        source.contains(".frame(width: 224.0, height: nil, alignment: .leading)"),
        "{source}"
    );
}

/// The showcase's tool strip: a segmented picker, a spacer and chips in a
/// flow; the chips are items of the flow, the spacer takes the rest of its
/// line. Labelled chips elsewhere wrap in a flow of their own.
#[test]
fn chip_pickers_are_items_of_their_flow() {
    let (config, compiled) = showcase();
    let output = workspace().join("target/bern-test/chips");
    let context = context(&config, &compiled, "macos", &output);
    let files = SwiftUiMacos.generate(&compiled.app, &context).unwrap();
    let generated = files
        .iter()
        .find(|f| f.path == Path::new("Sources/BernGenerated.swift"))
        .unwrap();
    let source = String::from_utf8_lossy(&generated.contents);
    let start = source
        .find("BernFlow(gap: 10.0) {")
        .expect("the tool strip");
    let strip = &source[start..];
    let segmented = strip.find(".pickerStyle(.segmented)").unwrap();
    let spacer = strip
        .find(".layoutValue(key: BernRowSpacer.self, value: true)")
        .unwrap();
    let chips = strip.find("BernChips(selection:").unwrap();
    assert!(segmented < spacer && spacer < chips, "{strip}");
    assert!(
        strip[chips..].contains("flat: true, target: \"controls_page.strip_filter\""),
        "{strip}"
    );
    assert!(
        source.contains("flat: false, target: \"controls_page.density_chips\""),
        "{source}"
    );
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
    // The engine, then the list of native libraries it links.
    assert_eq!(
        programs(&release),
        ["cargo", "cargo", "xcrun swiftc", "mv", "xcrun strip", "codesign"]
    );
    let listing = release
        .steps
        .iter()
        .filter_map(|step| match step {
            BuildStep::Run { program, args, .. } if program == "cargo" => Some(args),
            _ => None,
        })
        .nth(1)
        .unwrap();
    assert_eq!(listing[..2], ["rustc", "--lib"]);
    assert!(listing.contains(&"--release".to_string()));
    let native_libs = listing.last().unwrap().strip_prefix("native-static-libs=").unwrap();
    assert!(native_libs.ends_with("libbern_app.native-libs"));
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
    // swiftc links what the engine's static library needs.
    assert!(swiftc.contains(&format!("@{native_libs}")));

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
    typecheck_ios(&root, &output);
}

/// Type-check generated iOS Swift against the simulator SDK, when it is
/// installed.
fn typecheck_ios(root: &Path, output: &Path) {
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
        #[content]
        pub name: String,
        pub dest: Dest,
    }

    #[value]
    pub enum Size {
        #[label(tr! { "en" => "small" })]
        Small,
        #[label(tr! { "en" => "large" })]
        Large,
    }

    #[value]
    pub enum Notice {
        #[label(tr! { "en" => "Saved {name} on {day, date, EEE, MMM d}" })]
        Saved {
            #[content]
            name: String,
            day: CalendarDate,
        },
        #[label(tr! { "en" => "{n, plural, one {# {size} entry} other {# {size} entries}}" })]
        Counted { n: i64, size: Size, weight: f64 },
        #[label(tr! { "en" => "Idle" })]
        Idle,
    }

    #[state]
    pub struct State {
        pub entries: Vec<Entry>,
        pub levels: Vec<i64>,
        pub mode: Dest,
        pub picked: Option<Dest>,
        pub notice: Notice,
        pub due: CalendarDate,
        pub seconds: f64,
        pub count: i64,
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
                                Shape(kind: rounded, x: 1, y: 1, width: 60, height: 38, corner: 8, stroke: control.border, stroke_width: 1.5, dash: 4.5, dash_gap: 3)
                            }
                            Stack(align: top_leading, under_bars: true) {
                                Button(id: resume, label: tr! { "en" => "Resume" }, shape: capsule, height: 32) {
                                    Row(gap: 6) {
                                        Text(content: tr! { "en" => "Resume" })
                                    }
                                } => app.add(1)
                            }
                            Text(content: tr! { "en" => "Marked" }, mark: accent)
                            Text(content: app.notice)
                            Text(content: tr! { "en" => "{n, plural, =0 {none} one {# level} other {# levels}} by {day, date, MMMM d, y}, {t, duration, h:mm:ss} left"; n: app.count, day: app.due, t: app.seconds })
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
        generated
            .lines()
            .any(|line| line.contains(".animation(") && line.contains("value: bernEngine.mode)")),
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
            navigation.contains("values: { () -> [App_Dest] in"),
            "{navigation}"
        );
        assert!(navigation.contains(".extra"), "{navigation}");
        assert!(navigation.contains(".entries"), "{navigation}");
        assert!(navigation.contains(".feed"), "{navigation}");
    }
    // The selection falls back when its item goes away.
    assert!(
        generated.contains(".onChange(of: { () -> [App_Dest] in"),
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
  Stack(enabled: draft.query == "") {
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
     ScrollView(axis: horizontal, indicators: hidden, appear: fade) { Text(content: tr! { "en" => "Shelf" }) }
     Form(appear: fade) { Section { Text(content: tr! { "en" => "Row" }) } }
    }
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

[targets.ios]
platform = "ios"
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
    // A disabled split view keeps its toolbar safe area on macOS.
    assert!(generated.contains(".disabled(!("));
    assert!(!generated.contains(".allowsHitTesting("));
    assert!(generated.contains(".scrollIndicators(.never)"));
    // Scroll views enter without moving themselves: a scroll view's
    // content enters on appear inside it, and a form fades below the bar.
    let shelf = generated
        .split("SwiftUI.ScrollView(.horizontal) {")
        .nth(1)
        .unwrap();
    let shelf = &shelf[..shelf.find(".scrollIndicators").unwrap()];
    assert!(
        shelf.contains("SwiftUI.Group {") && shelf.contains("appear: BernAppear.fade, delay: 0.0)"),
        "{shelf}"
    );
    assert!(generated.contains(".bernScrollEntrance(motion:"));
    // iOS shows the same palette: the system text field, a tap outside to
    // close.
    let ios_output = root.join("target/bern-test/swiftui-search-palette-ios");
    let _ = std::fs::remove_dir_all(&ios_output);
    let mut ios = crate::context(&config, &compiled, "ios", &ios_output);
    ios.project_dir = dir.clone();
    let problems = SwiftUiIos.check(&compiled.app, &ios.target);
    assert!(problems.is_empty(), "{problems:?}");
    for file in SwiftUiIos.generate(&compiled.app, &ios).unwrap() {
        let path = ios_output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ios_generated =
        std::fs::read_to_string(ios_output.join("Sources/BernGenerated.swift")).unwrap();
    assert!(ios_generated.contains("BernSearchPaletteHost {"));
    assert!(ios_generated.contains("BernSearchPalette(text:"));
    typecheck_ios(&root, &ios_output);
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
fn tab_accessory_is_the_tab_views_bottom_accessory() {
    let (config, mut compiled) = showcase();
    add_tab_accessory(&mut compiled.app);
    for (platform, files) in [
        ("ios", {
            let output = workspace().join("target/bern-test/tab-accessory-ios");
            let context = context(&config, &compiled, "ios", &output);
            SwiftUiIos.generate(&compiled.app, &context).unwrap()
        }),
        ("macos", {
            let output = workspace().join("target/bern-test/tab-accessory-macos");
            let context = context(&config, &compiled, "macos", &output);
            SwiftUiMacos.generate(&compiled.app, &context).unwrap()
        }),
    ] {
        let source = files
            .iter()
            .filter(|f| f.path.extension().is_some_and(|e| e == "swift"))
            .map(|f| String::from_utf8_lossy(&f.contents).into_owned())
            .collect::<String>();
        for expected in [
            "let bernAccessory = SwiftUI.AnyView(SwiftUI.Group {",
            "let bernAccessoryShown: Bool = true",
            ".bernTabAccessoryInset(bernAccessoryShown, bernAccessory)",
            ".bernTabAccessory(bernAccessoryShown, bernAccessory)",
        ] {
            assert!(source.contains(expected), "{platform}: {expected}");
        }
    }
}

#[test]
fn declared_apple_icons_are_used_as_they_are() {
    let root = workspace();
    let (config, mut compiled) = showcase();
    let art = root.join("target/bern-test/swiftui-icons");
    let _ = std::fs::remove_dir_all(&art);
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend_from_slice(&1024u32.to_be_bytes());
    png.extend_from_slice(&1024u32.to_be_bytes());
    let mut icns = b"icns".to_vec();
    icns.extend_from_slice(&(16 + png.len() as u32).to_be_bytes());
    icns.extend_from_slice(b"ic10");
    icns.extend_from_slice(&(8 + png.len() as u32).to_be_bytes());
    icns.extend_from_slice(&png);
    let contents = r#"{"images":[{"filename":"Icon.png","idiom":"universal","platform":"ios","size":"1024x1024"},{"appearances":[{"appearance":"luminosity","value":"tinted"}],"filename":"Icon-tinted.png","idiom":"universal","platform":"ios","size":"1024x1024"}]}"#;
    for (path, bytes) in [
        ("App.icns", icns.clone()),
        (
            "AppIcon.appiconset/Contents.json",
            contents.as_bytes().to_vec(),
        ),
        ("AppIcon.appiconset/Icon.png", png.clone()),
        ("AppIcon.appiconset/Icon-tinted.png", png.clone()),
    ] {
        let path = art.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    // Absolute paths stay themselves when joined to the project directory.
    let icons = &mut compiled.app.app.platform_icons;
    icons.insert("macos".into(), art.join("App.icns").display().to_string());
    icons.insert(
        "ios".into(),
        art.join("AppIcon.appiconset").display().to_string(),
    );
    let output = root.join("target/bern-test/swiftui-icons-out");
    let file = |files: &[bern_target_api::GeneratedFile], path: &str| {
        files
            .iter()
            .find(|file| file.path == Path::new(path))
            .map(|file| file.contents.clone())
            .unwrap_or_else(|| panic!("no generated {path}"))
    };
    let mac = SwiftUiMacos
        .generate(
            &compiled.app,
            &context(&config, &compiled, "macos", &output),
        )
        .unwrap();
    assert_eq!(
        file(&mac, "Showcase.app/Contents/Resources/AppIcon.icns"),
        icns
    );
    let ios = SwiftUiIos
        .generate(&compiled.app, &context(&config, &compiled, "ios", &output))
        .unwrap();
    let set = "Assets.xcassets/AppIcon.appiconset";
    assert_eq!(
        file(&ios, &format!("{set}/Contents.json")),
        contents.as_bytes()
    );
    assert_eq!(file(&ios, &format!("{set}/Icon-tinted.png")), png);
    // The set has its own sizes: nothing scales it.
    let plan = SwiftUiIos
        .plan(&compiled.app, &context(&config, &compiled, "ios", &output))
        .unwrap();
    assert!(!plan.steps.iter().any(|step| matches!(
        step,
        BuildStep::Run { program, .. } if program == "sips"
    )));
    let _ = std::fs::remove_dir_all(&art);
}

/// A scene's background bleeds to the window's edges; the traffic lights
/// keep their room on macOS only.
#[test]
fn scene_layers_bleed_and_window_buttons_keep_the_traffic_lights_room() {
    use bern_ir::{Element, Expr, ExprKind, Node, Prop};
    use bern_protocol::{Type, Value};
    let (config, mut compiled) = showcase();
    let element = |kind: &str, props: Vec<(&str, Prop)>, children| {
        Node::Element(Element {
            kind: kind.into(),
            id: None,
            path: format!("test.{kind}"),
            props: props
                .into_iter()
                .map(|(name, prop)| (name.into(), prop))
                .collect(),
            template: None,
            children,
            action: None,
            source: SourceLocation {
                file: "scene.rs".into(),
                line: 1,
                column: 1,
            },
        })
    };
    let number = |value| Prop::Number { value };
    let place = |x, y, width, height| {
        vec![
            ("x", number(x)),
            ("y", number(y)),
            ("width", number(width)),
            ("height", number(height)),
        ]
    };
    let mut background = place(0.0, 0.0, 1280.0, 800.0);
    background.push((
        "bleed",
        Prop::Expr {
            expr: Expr {
                ty: Type::Bool,
                kind: ExprKind::Literal {
                    value: Value::Bool(true),
                },
            },
        },
    ));
    compiled.app.components[0].body = element(
        "ScaleBox",
        vec![("width", number(1280.0)), ("height", number(800.0))],
        vec![
            element("Positioned", background, vec![]),
            element(
                "Positioned",
                place(14.0, 0.0, 1246.0, 64.0),
                vec![element(
                    "Row",
                    vec![],
                    vec![
                        element("WindowButtons", vec![], vec![]),
                        element("Spacer", vec![], vec![]),
                    ],
                )],
            ),
        ],
    );
    let output = workspace().join("target/bern-test/scene-bleed");
    let context = context(&config, &compiled, "macos", &output);
    let files = SwiftUiMacos.generate(&compiled.app, &context).unwrap();
    let generated = files
        .iter()
        .find(|f| f.path == Path::new("Sources/BernGenerated.swift"))
        .unwrap();
    let source = String::from_utf8_lossy(&generated.contents);
    assert!(
        source.contains(
            ".modifier(BernBleedingLayer(x: 0.0, y: 0.0, width: 1280.0, height: 800.0, bleed: true))"
        ),
        "{source}"
    );
    // A layer that does not bleed keeps its own space for its layers.
    assert!(
        source.contains(".environment(\\.bernSceneRoom, nil)"),
        "{source}"
    );
    assert!(
        source.contains("#if os(macOS)\n")
            && source.contains("SwiftUI.Color.clear.frame(width: 84, height: 1)"),
        "{source}"
    );
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

/// A drop area that opens videos, written directly in the window.
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
                    Text(content: tr! { "en" => "Clip" })
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

[targets.macos]
platform = "macos"

[targets.ios]
platform = "ios"
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
/// component, has its style in `BernStyles` as one in a component has, on
/// macOS and iOS; the Swift type-checks.
#[test]
fn a_style_variant_in_a_window_is_defined() {
    let root = workspace();
    let (config, compiled) = stage_project(
        &root.join("target/bern-test/swiftui-stage-project"),
        STAGE_UI,
    );
    for (name, target) in [
        ("macos", &SwiftUiMacos as &dyn Target),
        ("ios", &SwiftUiIos),
    ] {
        let output = root.join(format!("target/bern-test/swiftui-stage-{name}"));
        let _ = std::fs::remove_dir_all(&output);
        let context = context(&config, &compiled, name, &output);
        let problems = target.check(&compiled.app, &context.target);
        assert!(problems.is_empty(), "{name}: {problems:?}");
        for file in target.generate(&compiled.app, &context).unwrap() {
            let path = output.join(&file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, &file.contents).unwrap();
        }
        let generated =
            std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
        let defined = generated
            .find("enum BernStyles {\n    static let buttonStage = BernStyle(")
            .unwrap_or_else(|| panic!("{name}: `buttonStage` is not defined:\n{generated}"));
        let used = generated
            .find(".buttonStyle(BernButtonStyle(style: BernStyles.buttonStage))")
            .unwrap_or_else(|| panic!("{name}: the button does not use its style:\n{generated}"));
        assert!(defined < used, "{generated}");
    }
    typecheck_ios(&root, &root.join("target/bern-test/swiftui-stage-ios"));
    if !Command::new("xcrun")
        .arg("--find")
        .arg("swiftc")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("swiftc not available; skipping type check");
        return;
    }
    let sources = root.join("target/bern-test/swiftui-stage-macos/Sources");
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

/// A drop area that opens files, written directly in a window rather than
/// in a component, still has the Info.plist list its kinds as document
/// types, on macOS and iOS.
#[test]
fn a_drop_in_a_window_lists_its_document_types() {
    let root = workspace();
    let (config, compiled) =
        stage_project(&root.join("target/bern-test/swiftui-drop-project"), DROP_UI);
    for (name, target, own) in [
        (
            "macos",
            &SwiftUiMacos as &dyn Target,
            "<key>CFBundleTypeExtensions</key>",
        ),
        (
            "ios",
            &SwiftUiIos,
            "<string>dev.bern.stage.file.mkv</string>",
        ),
    ] {
        let output = root.join(format!("target/bern-test/swiftui-drop-{name}"));
        let context = context(&config, &compiled, name, &output);
        let problems = target.check(&compiled.app, &context.target);
        assert!(problems.is_empty(), "{name}: {problems:?}");
        let files = target.generate(&compiled.app, &context).unwrap();
        let plist = files
            .iter()
            .find(|file| file.path.ends_with("Info.plist"))
            .unwrap_or_else(|| panic!("{name}: no Info.plist"));
        let plist = String::from_utf8_lossy(&plist.contents);
        for expected in [
            "<key>CFBundleDocumentTypes</key>",
            "<string>Video</string>",
            "<string>public.movie</string>",
            own,
        ] {
            assert!(
                plist.contains(expected),
                "{name}: missing `{expected}`:\n{plist}"
            );
        }
    }
}

/// Content smaller than the window starts at its top leading corner, as on
/// the other targets: a column written directly in the window spans its
/// width, its alignment placing its items across it, also through a
/// component; a stack without a size fills the window. A column inside
/// them keeps its own width. The Swift type-checks.
#[test]
fn content_in_a_window_starts_at_its_top_leading_corner() {
    let root = workspace();
    let (config, compiled) = stage_project(
        &root.join("target/bern-test/swiftui-place-project"),
        PLACE_UI,
    );
    for (name, target) in [
        ("macos", &SwiftUiMacos as &dyn Target),
        ("ios", &SwiftUiIos),
    ] {
        let output = root.join(format!("target/bern-test/swiftui-place-{name}"));
        let _ = std::fs::remove_dir_all(&output);
        let context = context(&config, &compiled, name, &output);
        let problems = target.check(&compiled.app, &context.target);
        assert!(problems.is_empty(), "{name}: {problems:?}");
        for file in target.generate(&compiled.app, &context).unwrap() {
            let path = output.join(&file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, &file.contents).unwrap();
        }
        let generated =
            std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
        let style = std::fs::read_to_string(output.join("Sources/BernStyle.swift")).unwrap();
        // The window places its content at its top leading corner.
        assert!(
            style.contains(
                ".frame(width: bounds.size.width, height: bounds.size.height, alignment: .topLeading)"
            ),
            "{name}"
        );
        let window = &generated[generated.find("struct BernWindow0").unwrap()..];
        assert_eq!(
            window
                .matches(".frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)")
                .count(),
            1,
            "{name}: the stack fills the window:\n{window}"
        );
        let lines: Vec<&str> = window.lines().map(str::trim).collect();
        let call = lines
            .iter()
            .position(|line| *line == "AppView_centered()")
            .unwrap();
        assert_eq!(
            lines[call + 1],
            ".frame(maxWidth: .infinity, alignment: .center)",
            "{name}: the component's column spans the window:\n{window}"
        );
        assert_eq!(
            window
                .matches(".frame(maxWidth: .infinity, alignment: .leading)")
                .count(),
            1,
            "{name}: the column spans the window:\n{window}"
        );
        // The inner column and the component's own body keep their width.
        assert!(
            !window.contains(".frame(maxWidth: .infinity, alignment: .trailing)"),
            "{name}:\n{window}"
        );
        let component = &generated[generated.find("struct AppView_centered").unwrap()..];
        let component = &component[..component
            .find("struct BernWindow0")
            .unwrap_or(component.len())];
        assert!(
            !component.contains("maxWidth: .infinity"),
            "{name}:\n{component}"
        );
    }
    typecheck_ios(&root, &root.join("target/bern-test/swiftui-place-ios"));
    if !Command::new("xcrun")
        .arg("--find")
        .arg("swiftc")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("swiftc not available; skipping type check");
        return;
    }
    let sources = root.join("target/bern-test/swiftui-place-macos/Sources");
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
