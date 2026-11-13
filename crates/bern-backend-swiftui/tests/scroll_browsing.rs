//! Scroll browsing observes native user interaction on older systems, from
//! inside the scroll content rather than a sibling that may be another scroll.

use bern_backend_swiftui::{SwiftUiIos, SwiftUiMacos, SwiftUiTvos};
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Worktrees use the main checkout's target, even without an environment
/// override. No generated fixture or Swift module cache belongs in a worktree.
fn target_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    let root = workspace();
    let common = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(common.status.success());
    let common = root
        .join(String::from_utf8(common.stdout).unwrap().trim())
        .canonicalize()
        .unwrap();
    common.parent().unwrap().join("target")
}

fn browsing_runtime() -> &'static str {
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("// MARK: - Scroll browsing").unwrap();
    let end = start
        + style[start..]
            .find("\nprivate struct BernKeyedNudge")
            .unwrap();
    &style[start..end]
}

fn project(dir: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let protocol = r#"
#[bern::protocol(namespace = "scrolling", version = 1)]
pub mod scrolling {
    use bern::prelude::*;
    #[value]
    pub struct Item {
        pub id: i64,
        #[content]
        pub title: String,
    }
    #[state]
    pub struct State { pub items: Vec<Item> }
    #[commands]
    pub trait Commands { fn close(&mut self) -> Result<(), CommandError>; }
}
"#;
    let ui = r#"
use bern::prelude::*;
#[bern::session]
pub struct Draft {
    pub outer: bool,
    pub inner: bool,
    pub horizontal: bool,
    pub anchored: bool,
    pub positioned: bool,
    pub selected: i64,
}
#[component]
pub fn app(app: scrolling::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Scrolling" }, initial: open) {
                Column {
                    ScrollView(browsing: draft.outer) {
                        Column {
                            Text(content: tr! { "en" => "Outer" })
                            ScrollView(browsing: draft.inner) {
                                Text(content: tr! { "en" => "Inner" })
                            }
                        }
                    }
                    ScrollView(axis: horizontal, browsing: draft.horizontal) {
                        Text(content: tr! { "en" => "First" })
                        Text(content: tr! { "en" => "Second" })
                    }
                    ScrollView(open_center: 0.4, open_at: 0.3, browsing: draft.anchored) {
                        Column { Text(content: tr! { "en" => "Anchored" }) }
                    }
                    ScrollView(position: draft.selected, snap: center, browsing: draft.positioned) {
                        Column {
                            ForEach(items: app.items, key: id) |item| { Text(content: item.title) }
                        }
                    }
                    ScrollView { Text(content: tr! { "en" => "Unobserved" }) }
                }
            }
        }
    }
}
"#;
    for (package, source) in [
        ("scrolling-protocol", protocol),
        ("scrolling-ui", ui),
        ("scrolling-core", ""),
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
id = "scrolling"
bundle_id = "dev.bern.scrolling"
version = "0.1.0"
name = { "en" = "Scrolling" }
[languages]
supported = ["en"]
default = "en"
[protocol]
package = "scrolling-protocol"
source = "scrolling-protocol/src/lib.rs"
[engine]
package = "scrolling-core"
type = "scrolling_core::Engine"
[ui]
package = "scrolling-ui"
sources = ["scrolling-ui/src/lib.rs"]
root = "app"
[targets.ios]
platform = "ios"
frontends = ["swiftui", "swiftui-16"]
[targets.macos]
platform = "macos"
[targets.tvos]
platform = "tvos"
"#,
    )
    .unwrap();
    let config = bern_compiler::config::load(&dir.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    (config, compiled)
}

#[test]
fn browsing_probes_are_inside_content_for_nested_and_multi_root_scrolls() {
    let root = workspace();
    let output = target_dir().join("bern-test/scroll-browsing-generated");
    let (config, compiled) = project(&output.join("project"));
    for (platform, backend) in [
        ("ios", &SwiftUiIos as &dyn Target),
        ("macos", &SwiftUiMacos as &dyn Target),
        ("tvos", &SwiftUiTvos as &dyn Target),
    ] {
        let target = config.targets.iter().find(|t| t.name == platform).unwrap();
        let context = BuildContext {
            project_dir: config.dir.clone(),
            workspace_dir: root.clone(),
            output_dir: output.join(platform),
            target: target.clone(),
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
        let app = compiled.app.on_platform(platform);
        let files = backend.generate(&app, &context).unwrap();
        let generated: Vec<_> = files
            .iter()
            .filter(|file| {
                file.path
                    .file_name()
                    .is_some_and(|n| n == "BernGenerated.swift")
            })
            .collect();
        assert_eq!(generated.len(), if platform == "ios" { 2 } else { 1 });
        for file in generated {
            let swift = std::str::from_utf8(&file.contents).unwrap();
            assert_eq!(
                swift.matches(".bernScrollBrowsingContent()").count(),
                5,
                "{swift}"
            );
            assert_eq!(swift.matches(".bernScrollBrowsing(").count(), 5, "{swift}");
            let mut open_scrolls = Vec::new();
            let mut depth = 0;
            for line in swift.lines() {
                if line.trim_start().starts_with("SwiftUI.ScrollView(") {
                    open_scrolls.push(depth);
                }
                // These fixtures have no braces in their text content.
                if line.contains(".bernScrollBrowsingContent()") {
                    assert!(
                        open_scrolls.last().is_some_and(|outer| depth > *outer),
                        "{line}"
                    );
                }
                depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
                while open_scrolls.last().is_some_and(|outer| depth <= *outer) {
                    open_scrolls.pop();
                }
            }
            assert!(swift.contains("BernOpenCenter(at:"), "{swift}");
            assert!(swift.contains(".bernScrollPosition("), "{swift}");
        }
    }
}

#[test]
fn native_browsing_observes_user_events_without_gestures_offsets_or_idle_timers() {
    let source = browsing_runtime();
    assert!(source.contains("content.onScrollPhaseChange"));
    assert!(source.contains("pan?.addTarget(self, action: #selector(panChanged(_:)))"));
    assert!(source.contains("pan?.removeTarget(self, action: #selector(panChanged(_:)))"));
    assert!(source.contains("case .began, .changed: begin()"));
    assert!(
        source.contains(
            "scrollView.isDragging || scrollView.isTracking || scrollView.isDecelerating"
        )
    );
    assert!(source.contains("link.add(to: .main, forMode: .common)"));
    assert!(source.contains("displayLink?.invalidate()"));
    assert!(source.contains("weak var observer: BernNativeScrollBrowsingObserver?"));
    assert!(source.contains(".frame(width: 0, height: 0)"));
    assert!(source.contains("isUserInteractionEnabled = false"));
    assert!(source.contains("enclosingScrollView"));
    assert!(source.contains("sequence(first: superview, next: { $0?.superview })"));
    assert!(source.contains("NSScrollView.willStartLiveScrollNotification"));
    assert!(source.contains("NSScrollView.didLiveScrollNotification"));
    assert!(source.contains("NSScrollView.didEndLiveScrollNotification"));
    assert!(source.contains("dismantleUIView"));
    assert!(source.contains("dismantleNSView"));
    assert!(!source.contains("DragGesture"));
    assert!(!source.contains("addGestureRecognizer"));
    assert!(!source.contains("contentOffset"));
    assert!(!source.contains("asyncAfter"));
    assert!(!source.contains("Timer("));
    assert!(!source.contains(".subviews"));
}

#[cfg(target_os = "macos")]
fn swift_output(name: &str) -> (PathBuf, PathBuf) {
    let target = target_dir();
    let output = target.join("bern-test").join(name);
    let cache = target.join("bern-swift-module-cache");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    (output, cache.canonicalize().unwrap())
}

#[test]
#[cfg(target_os = "macos")]
fn older_uikit_browsing_compiles_with_swift_six_isolation() {
    let (output, cache) = swift_output("scroll-browsing-uikit");
    let file = output.join("runtime.swift");
    std::fs::write(
        &file,
        format!("import SwiftUI\nimport UIKit\n{}", browsing_runtime()),
    )
    .unwrap();
    let architecture = if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    };
    for (sdk, platform) in [
        ("iphonesimulator", "ios16.0"),
        ("appletvsimulator", "tvos17.0"),
    ] {
        if !Command::new("xcrun")
            .args(["--sdk", sdk, "--show-sdk-path"])
            .output()
            .is_ok_and(|result| result.status.success())
        {
            eprintln!("no {sdk} SDK; skipping the browsing compile check");
            continue;
        }
        let result = Command::new("xcrun")
            .args([
                "--sdk",
                sdk,
                "swiftc",
                "-swift-version",
                "6",
                "-parse-as-library",
                "-wmo",
                "-emit-sil",
                "-o",
                "/dev/null",
                "-target",
            ])
            .arg(format!("{architecture}-apple-{platform}-simulator"))
            .arg("-module-cache-path")
            .arg(&cache)
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{sdk}:\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
#[cfg(target_os = "macos")]
fn browsing_publication_and_appkit_observers_have_balanced_lifetimes() {
    let (output, cache) = swift_output("scroll-browsing-appkit");
    let source = format!(
        "import SwiftUI\nimport AppKit\n{}\n{}",
        browsing_runtime(),
        r#"
@main struct ScrollBrowsingTest {
    @MainActor static func drain() async {
        for _ in 0..<4 {
            await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
                DispatchQueue.main.async { continuation.resume() }
            }
        }
    }

    @MainActor static func main() async {
        var browsing = false
        var writes: [Bool] = []
        var inLifecycle = false
        let binding = Binding(get: { browsing }, set: {
            precondition(!inLifecycle, "layout/dismantle may not publish a binding")
            browsing = $0
            writes.append($0)
        })
        let activity = BernScrollBrowsingActivity()
        activity.bind(binding)
        let first = activity.acquire()
        inLifecycle = true
        activity.report(first, true)
        precondition(!browsing, "publication must be deferred")
        inLifecycle = false
        await drain()
        precondition(browsing && writes == [true])

        let second = activity.acquire()
        activity.report(second, true)
        activity.release(first)
        activity.report(first, false)
        await drain()
        precondition(browsing && writes == [true], "an old source cannot clear a newer source")
        inLifecycle = true
        activity.release(second)
        precondition(browsing)
        inLifecycle = false
        await drain()
        precondition(!browsing && writes == [true, false])

        // A queued publication reads the latest binding, not a captured one.
        var replacement = false
        let third = activity.acquire()
        activity.report(third, true)
        activity.bind(Binding(get: { replacement }, set: { replacement = $0 }))
        await drain()
        precondition(replacement && !browsing)
        activity.release(third)
        activity.report(third, true)
        await drain()
        precondition(!replacement, "late native callbacks after release are ignored")
        activity.bind(binding)

        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        scroll.documentView = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 500))
        let key = ObjectIdentifier(scroll)
        let owner = UUID(), other = UUID()
        activity.attach(to: scroll, owner: owner)
        activity.attach(to: scroll, owner: other)
        let center = NotificationCenter.default
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: scroll)
        await drain()
        precondition(browsing)
        activity.detach(from: key, owner: owner)
        await drain()
        precondition(browsing, "another content probe still owns this native observer")
        center.post(name: NSScrollView.didLiveScrollNotification, object: scroll)
        await drain()
        precondition(browsing)
        center.post(name: NSScrollView.didEndLiveScrollNotification, object: scroll)
        await drain()
        precondition(!browsing)
        scroll.contentView.scroll(to: NSPoint(x: 0, y: 150))
        scroll.reflectScrolledClipView(scroll.contentView)
        await drain()
        precondition(!browsing, "programmatic offsets must not count as browsing")

        // Legacy wheel ticks need not have a start/end pair. Each unbracketed
        // user event is observable, but cannot leave browsing stuck forever.
        writes.removeAll()
        center.post(name: NSScrollView.didLiveScrollNotification, object: scroll)
        await drain()
        precondition(writes == [true, false] && !browsing)
        center.post(name: NSScrollView.didLiveScrollNotification, object: scroll)
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: scroll)
        await drain()
        precondition(browsing, "a pending wheel release cannot end a new live gesture")

        // A last-owner release queued during layout cannot undo a rebind.
        inLifecycle = true
        activity.detach(from: key, owner: other)
        let fresh = UUID()
        activity.attach(to: scroll, owner: fresh)
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: scroll)
        activity.detach(from: key, owner: other)
        inLifecycle = false
        await drain()
        precondition(browsing)
        center.post(name: NSApplication.didResignActiveNotification, object: nil)
        await drain()
        precondition(!browsing)
        activity.detach(from: key, owner: fresh)
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: scroll)
        await drain()
        precondition(!browsing, "detached observers must not receive native events")

        // Find the nearest enclosing scroll, never an outer or sibling one.
        let outer = NSScrollView(), inner = NSScrollView(), sibling = NSScrollView()
        let outerContent = NSView(), innerContent = NSView()
        outer.documentView = outerContent
        outerContent.addSubview(inner)
        outerContent.addSubview(sibling)
        inner.documentView = innerContent
        let probe = BernScrollBrowsingProbeView()
        innerContent.addSubview(probe)
        precondition(probe.enclosingScrollView === inner)
        let attachment = BernScrollBrowsingAttachment()
        attachment.update(BernScrollBrowsingContext(activity: activity, binding: binding))
        attachment.connect(to: probe.enclosingScrollView)
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: outer)
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: sibling)
        await drain()
        precondition(!browsing)
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: inner)
        await drain()
        precondition(browsing)
        inLifecycle = true
        attachment.dismantle()
        precondition(browsing)
        inLifecycle = false
        attachment.connect(to: inner)
        center.post(name: NSScrollView.willStartLiveScrollNotification, object: inner)
        await drain()
        precondition(!browsing, "dismantling is final, even if layout runs again")

        // The final idle publication must survive destruction of the session.
        var departing: BernScrollBrowsingActivity? = BernScrollBrowsingActivity()
        departing!.bind(binding)
        let last = departing!.acquire()
        departing!.report(last, true)
        await drain()
        precondition(browsing)
        departing!.release(last)
        departing = nil
        await drain()
        precondition(!browsing)
    }
}
"#,
    );
    let file = output.join("main.swift");
    let executable = output.join("scroll-browsing");
    std::fs::write(&file, source).unwrap();
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-swift-version",
            "6",
            "-parse-as-library",
            "-module-cache-path",
        ])
        .arg(cache)
        .arg(file)
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
