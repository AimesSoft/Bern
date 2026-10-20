//! A layout chosen by width is the right one from its first build: on a
//! phone, a subtree mounted as the app starts or inserted later builds only
//! the phone's layout, and a modal that covers a compact window never
//! starts as a floating panel; a wide window never builds the narrow
//! layout first. Exercises the actual `BernAdaptive` and window widths in
//! off-screen windows; no application engine or screenshot permissions are
//! needed.
#[cfg(target_os = "macos")]
#[test]
fn layouts_chosen_by_width_are_right_from_the_first_build() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/adaptive-first-layout");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernRuntime.swift");
    let start = runtime.find("// MARK: - Controls").unwrap();
    let end = runtime.find("// MARK: - Host").unwrap();
    let prelude = r#"
import AppKit
import SwiftUI
@MainActor @Observable final class BernWindowContext {
    var id = "main#1"
    var width: CGFloat = 0
    var height: CGFloat = 0
    /// What the platform says the window gets before its first layout.
    @ObservationIgnored let unmeasuredSize: CGSize
    init(unmeasured: CGSize) { unmeasuredSize = unmeasured }
    /// As the runtime's: the layout width by the app's breakpoints.
    var compact: Bool { bernAppBreakpoints.size(of: layoutWidth) == "compact" }
}
@MainActor final class BernHost {
    static let shared = BernHost()
    let controls = BernControls()
    func isBlocked(_ window: String) -> Bool { false }
}
let bernAppBreakpoints = BernBreakpoints(compact: 600, medium: nil, expanded: nil)
"#;
    let tests = r#"
/// How often each layout was built, by name. Layouts build on the main
/// thread only.
enum Builds {
    nonisolated(unsafe) static var counts: [String: Int] = [:]
    static func built(_ name: String) { counts[name, default: 0] += 1 }
}

@MainActor @Observable final class Flow {
    var started = false
    var modal = false
    var page = false
}

func layouts(_ name: String) -> some View {
    BernAdaptive(breakpoints: bernAppBreakpoints, declared: ["compact", "expanded"], target: name) { size in
        let _ = Builds.built("\(name).\(size)")
        switch size {
        case "compact": Text("phone")
        default: Text("desk")
        }
    }
}

struct Root: View {
    let window: BernWindowContext
    let flow: Flow
    var body: some View {
        ZStack {
            if flow.started { layouts("home") }
            // A modal that covers the window where it is compact.
            if flow.modal {
                if window.compact {
                    let _ = Builds.built("modal.cover")
                    Color.clear
                } else {
                    let _ = Builds.built("modal.panel")
                    Color.clear.frame(width: 300, height: 200)
                }
            }
            if flow.page { layouts("page") }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onGeometryChange(for: CGSize.self) { $0.size } action: {
            window.width = $0.width
            window.height = $0.height
        }
        .environment(window)
    }
}

@MainActor func spin(_ seconds: Double) { RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds)) }

/// Starts the app in an off-screen window `width` wide whose platform says
/// `unmeasured` before the first layout, then inserts a page.
@MainActor func run(_ name: String, width: CGFloat, unmeasured: CGSize, compact: Bool) {
    Builds.counts = [:]
    let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: width, height: 700),
        styleMask: [.titled], backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false
    let context = BernWindowContext(unmeasured: unmeasured)
    let flow = Flow()
    flow.started = true
    flow.modal = true
    window.contentView = NSHostingView(rootView: Root(window: context, flow: flow))
    window.orderBack(nil)
    spin(0.5)
    precondition(context.width == width, "\(name): the window is measured: \(context.width)")
    let shown = compact ? "compact" : "expanded"
    let other = compact ? "expanded" : "compact"
    func size(_ target: String) -> String? {
        try? BernHost.shared.controls.find(target).actions.value?()
    }
    precondition(size("home") == shown, "\(name): the layout started with is \(shown): \(String(describing: size("home")))")
    precondition(Builds.counts["home.\(shown)", default: 0] > 0, "\(name): \(Builds.counts)")
    precondition(Builds.counts["home.\(other)"] == nil,
        "\(name): the layout started with never builds the \(other) one: \(Builds.counts)")
    precondition(Builds.counts[compact ? "modal.cover" : "modal.panel", default: 0] > 0, "\(name): \(Builds.counts)")
    precondition(Builds.counts[compact ? "modal.panel" : "modal.cover"] == nil,
        "\(name): the modal is \(compact ? "a cover" : "a panel") from its first build: \(Builds.counts)")
    // A layout inserted into the window after it is measured.
    flow.modal = false
    flow.page = true
    spin(0.5)
    precondition(size("page") == shown, "\(name): the layout inserted is \(shown): \(String(describing: size("page")))")
    precondition(Builds.counts["page.\(shown)", default: 0] > 0, "\(name): \(Builds.counts)")
    precondition(Builds.counts["page.\(other)"] == nil,
        "\(name): the layout inserted never builds the \(other) one: \(Builds.counts)")
    flow.started = false
    flow.page = false
    spin(0.2)
    window.close()
}

@main struct AdaptiveFirstLayoutTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        // A phone: its scene is the window's width.
        run("phone", width: 390, unmeasured: CGSize(width: 390, height: 844), compact: true)
        // A window of its own on a tablet: the screen is wider than it.
        run("tablet window", width: 1000, unmeasured: CGSize(width: 1366, height: 1024), compact: false)
        // A Mac window: nothing is known before its first layout.
        run("desktop", width: 1000, unmeasured: CGSize(width: CGFloat.infinity, height: .infinity), compact: false)
        print("adaptive first layout passed")
    }
}
"#;
    let source = root.join("AdaptiveFirstLayout.swift");
    let binary = root.join("adaptive-first-layout-test");
    fs::write(
        &source,
        format!("{prelude}\n{}\n{tests}", &runtime[start..end]),
    )
    .unwrap();
    let result = Command::new("xcrun")
        .args(["swiftc", "-parse-as-library"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(&binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
