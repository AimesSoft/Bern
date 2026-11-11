//! Compile exclusive taps, continuous swipes and scoped brightness with native SDKs.
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
#[bern::session]
pub struct Draft { pub amount: f64, pub active: bool, pub available: bool }
#[component]
pub fn app(app: lines::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Gestures" }, initial: open) {
                Column {
                    ScreenBrightness(value: draft.amount, available: draft.available)
                    Swipe(id: swipe, label: tr! { "en" => "Level" }, value: draft.amount, min: 0, max: 1, continuous: true, active: draft.active) {
                        Button(id: surface, label: tr! { "en" => "Surface" }) {
                            Canvas(width: 200, height: 200)
                            DoubleTap => app.wave()
                        } => draft.active.set(false)
                        OnCommit => app.wave()
                    } => app.wave()
                    if window.touch { Text(content: tr! { "en" => "Touch" }) }
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
fn touch_gestures_compile_for_both_ios_frontends() {
    let root = workspace();
    let build_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let (config, compiled) = project(&build_root.join("bern-test/touch-gestures-project"));
    let target = config
        .targets
        .iter()
        .find(|target| target.name == "ios")
        .unwrap()
        .clone();
    assert_eq!(target.frontends, ["swiftui", "swiftui-16"]);
    let app = compiled.app.on_platform(&target.platform);
    let output = build_root.join("bern-test/touch-gestures");
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

    for path in [
        "Sources/BernGenerated.swift",
        "Sources17/BernGenerated.swift",
    ] {
        let source = read(path);
        assert!(source.contains(".bernDoubleTap(single:"), "{source}");
        assert!(
            source.contains("continuous: true, active: SwiftUI.Binding"),
            "{source}"
        );
        assert!(source.contains("BernScreenBrightness(value:"), "{source}");
        assert!(!source.contains("unsupported DoubleTap"));
        assert!(!source.contains("unsupported OnCommit"));
    }
    support::check_ios(&root, &output);
}

#[test]
#[cfg(target_os = "macos")]
fn swipe_commit_and_scoped_brightness_preserve_observer_order_and_ownership() {
    use std::process::Command;
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("@MainActor\nfunc bernCommitSwipe(").unwrap();
    let end = start + style[start..].find("\n/// One step").unwrap();
    let output = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target"))
        .join("bern-test/swipe-commit");
    std::fs::create_dir_all(&output).unwrap();
    let brightness = style
        .find("@MainActor\nfinal class BernBrightnessOverrides")
        .unwrap();
    let source = format!(
        "import Foundation\n{}\n{}\n{}",
        &style[start..end],
        &style[brightness..],
        r#"
@main struct SwipeCommitTest {
    enum Failure: Error { case expected }
    @MainActor static func main() {
        var value = 0.2
        var active = true
        var seen: [Double] = []
        var order: [String] = []
        bernCommitSwipe(update: {
            value = 0.8
            order.append("update")
        }, change: {
            precondition(active)
            seen.append(value)
            order.append("change")
        }, commit: {
            precondition(active)
            seen.append(value)
            order.append("commit")
        }, clear: {
            active = false
            value = 0.2 // Synchronous idle Watch restores the engine snapshot.
            order.append("clear")
        })
        precondition(seen == [0.8, 0.8] && !active && value == 0.2)
        precondition(order == ["update", "change", "commit", "clear"])
        for failure in 0..<3 {
            var cleared = 0
            var visited: [Int] = []
            do {
                try bernCommitSwipe(update: {
                    visited.append(0)
                    if failure == 0 { throw Failure.expected }
                }, change: {
                    visited.append(1)
                    if failure == 1 { throw Failure.expected }
                }, commit: {
                    visited.append(2)
                    throw Failure.expected
                }, clear: { cleared += 1 })
                preconditionFailure("must propagate the error")
            } catch Failure.expected {} catch { preconditionFailure("unexpected error") }
            precondition(cleared == 1 && visited == Array(0...failure))
        }
        let overrides = BernBrightnessOverrides()
        let screen = NSObject()
        let key = ObjectIdentifier(screen)
        let first = UUID(), second = UUID()
        var brightness = 0.4
        var writes = 0
        let write = { (value: Double) in brightness = value; writes += 1 }
        precondition(overrides.acquire(screen: key, owner: first, read: { brightness }, write: write) == 0.4)
        precondition(writes == 0, "activation reads without setting brightness")
        overrides.update(screen: key, owner: first, value: 0.6)
        precondition(overrides.acquire(screen: key, owner: second, read: { brightness }, write: write) == 0.6)
        overrides.update(screen: key, owner: second, value: 0.8)
        let before = writes
        overrides.release(screen: key, owner: first)
        precondition(brightness == 0.8 && writes == before, "old owner must not restore through the active override")
        overrides.release(screen: key, owner: second)
        precondition(brightness == 0.4, "last owner restores the original screen setting")
        brightness = 0.3
        precondition(overrides.acquire(screen: key, owner: first, read: { brightness }, write: write) == 0.3)
        overrides.update(screen: key, owner: first, value: 0.5)
        _ = overrides.acquire(screen: key, owner: second, read: { brightness }, write: write)
        overrides.update(screen: key, owner: second, value: 0.9)
        overrides.update(screen: key, owner: first, value: 0.7)
        precondition(brightness == 0.9, "a covered owner must not override the top owner")
        overrides.release(screen: key, owner: second)
        precondition(brightness == 0.7, "releasing the top owner restores the preceding override")
        overrides.update(screen: key, owner: first, value: .nan)
        precondition(brightness == 0.7)
        overrides.update(screen: key, owner: first, value: 2)
        precondition(brightness == 1)
        overrides.release(screen: key, owner: first)
        precondition(brightness == 0.3, "reactivation uses a fresh baseline")
    }
}
"#
    );
    let file = output.join("main.swift");
    let executable = output.join("swipe-commit");
    std::fs::write(&file, source).unwrap();
    let result = Command::new("xcrun")
        .args(["swiftc", "-swift-version", "6", "-parse-as-library"])
        .arg(&file)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(executable).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
