//! What SwiftUI gained in iOS 17 that the generator writes for a scroll
//! view and a control: a scroll view that opens at a point of what it
//! scrolls, and the largest size of a control. An iOS app builds both front
//! ends by default, and each compiles for its own earliest system.

use bern_backend_swiftui::SwiftUiIos;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

mod support;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "lines", version = 1)]
pub mod lines {
    use bern::prelude::*;

    #[value]
    pub struct Line {
        pub id: i64,
        #[content]
        pub title: String,
    }

    #[state]
    pub struct State {
        pub lines: Vec<Line>,
    }

    #[commands]
    pub trait Commands {
        fn wave(&mut self) -> Result<(), CommandError>;
    }
}
"#;

const UI: &str = r#"
use bern::prelude::*;

style! {
    Button.big { size: extra_large }
}

#[component]
pub fn app(app: lines::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Lines" }, initial: open) {
                Column(gap: 8) {
                    ScrollView(axis: horizontal, open_at: 0.5) {
                        Row(gap: 8) {
                            ForEach(items: app.lines, key: id) |line| {
                                Text(content: line.title)
                            }
                        }
                    }
                    Button(id: wave, label: tr! { "en" => "Wave" }, variant: big) => app.wave()
                    ScrollView(open_at: 0.3) {
                        Column(gap: 0) {
                            ForEach(items: app.lines, key: id) |line| {
                                Text(content: line.title)
                            }
                        }
                    }
                }
            }
        }
    }
}
"#;

fn project(dir: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let _ = std::fs::remove_dir_all(dir);
    for (package, source) in [
        ("lines-protocol", PROTOCOL),
        ("lines-ui", UI),
        ("lines-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    // No `frontends`: the front end of iOS 16 and that of iOS 17 and later.
    std::fs::write(
        dir.join("Bern.toml"),
        r#"
[app]
id = "lines"
bundle_id = "dev.bern.lines"
version = "0.1.0"
name = { "en" = "Lines" }

[languages]
supported = ["en"]
default = "en"

[protocol]
package = "lines-protocol"
source = "lines-protocol/src/lib.rs"

[engine]
package = "lines-core"
type = "lines_core::Engine"

[ui]
package = "lines-ui"
sources = ["lines-ui/src/lib.rs"]
root = "app"

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

#[test]
fn ios_16_opens_scroll_views_at_their_anchor_and_has_the_largest_size_it_can() {
    let root = workspace();
    let test_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
        .join("bern-test");
    let (config, compiled) = project(&test_root.join("ios16-front-end-project"));
    let target = config
        .targets
        .iter()
        .find(|target| target.name == "ios")
        .unwrap()
        .clone();
    assert_eq!(target.frontends, ["swiftui", "swiftui-16"]);
    let app = compiled.app.on_platform(&target.platform);
    let output = test_root.join("ios16-front-end");
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
    let problems = SwiftUiIos.check(&app, &context.target);
    assert!(
        problems.iter().all(|problem| !problem.is_error()),
        "{problems:?}"
    );
    for file in SwiftUiIos.generate(&app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let read = |path: &str| std::fs::read_to_string(output.join(path)).unwrap();

    // From iOS 17 the system opens the scroll view where it is anchored.
    let current = read("Sources17/BernGenerated.swift");
    for expected in [
        ".defaultScrollAnchor(UnitPoint(x: 0.5, y: 0))",
        ".defaultScrollAnchor(UnitPoint(x: 0, y: 0.3))",
        ".bernControlSize(.extraLarge)",
        "BernStyle(base: BernLook(), size: .extraLarge)",
    ] {
        assert!(current.contains(expected), "lacks `{expected}`:\n{current}");
    }
    assert!(!current.contains("BernOpenAt16"), "{current}");

    // On iOS 16 what each scroll view scrolls opens it there.
    let older = read("Sources/BernGenerated.swift");
    for expected in [
        "SwiftUI.ScrollView(.horizontal) { BernOpenAt16(anchor: UnitPoint(x: 0.5, y: 0)) {",
        "SwiftUI.ScrollView(.vertical) { BernOpenAt16(anchor: UnitPoint(x: 0, y: 0.3)) {",
        // A style still draws its control at the largest size, and the
        // platform's control takes the largest it has.
        "BernStyle(base: BernLook(), size: .extraLarge)",
        ".bernControlSize(.extraLarge)",
    ] {
        assert!(older.contains(expected), "lacks `{expected}`:\n{older}");
    }
    assert!(!older.contains("defaultScrollAnchor"), "{older}");
    let style = read("Sources/BernStyle.swift");
    assert!(
        style.contains("case .extraLarge: return .large\n"),
        "{style}"
    );

    // A touch changes only the glow's subscriber. Broadcasting its location
    // through the window environment resets native segments before their action.
    assert!(!style.contains("bernPointerLocation16"));
    assert!(style.contains("@ObservedObject var pointer: BernPointer"));
    assert!(style.contains("BernGlowTracking(pointer: pointer, radius: radius, opacity: opacity)"));
    let tracking = style
        .split("private struct BernGlowTracking:")
        .nth(1)
        .unwrap();
    assert!(
        tracking
            .split("private struct BernGlowLight:")
            .next()
            .unwrap()
            .contains("pointer.location")
    );
    let newer_style = read("Sources17/BernStyle.swift");
    assert!(newer_style.contains("@Bindable var pointer: BernPointer"));
    assert!(!read("Sources/BernCompat16.swift").contains("BernPointerLocation16Key"));

    support::check_ios(&root, &output);
}
