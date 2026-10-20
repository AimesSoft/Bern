//! A control registers for external entries while it is mounted, and only
//! then: one that appears and goes again at once, as a phone's modal does
//! when the window's width arrives after its first layout, leaves nothing
//! behind. Exercises the actual SwiftUI modifier in an off-screen window;
//! no application engine or screenshot permissions are needed.
#[cfg(target_os = "macos")]
#[test]
fn controls_that_come_and_go_at_once_leave_no_registration() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/control-registry");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernRuntime.swift");
    let start = runtime.find("// MARK: - Controls").unwrap();
    let end = runtime
        .find("/// Chooses one layout by the available width.")
        .unwrap();
    let prelude = r#"
import AppKit
import SwiftUI
@MainActor @Observable final class BernWindowContext {
    var id = "main#1"
    var width: CGFloat = 0
    /// Compact once the width is known, as the runtime's breakpoints are.
    var compact: Bool { width > 0 && width < 600 }
}
@MainActor final class BernHost {
    static let shared = BernHost()
    let controls = BernControls()
    func isBlocked(_ window: String) -> Bool { false }
}
"#;
    let tests = r#"
@MainActor @Observable final class Flow {
    var shown = true
    var page = false
}

func buttons(_ prefix: String) -> some View {
    HStack {
        Text("Skip").modifier(BernControl(
            target: "\(prefix).skip", role: "button", label: "Skip", enabled: true,
            actions: BernControlActions()))
        Text("Start").modifier(BernControl(
            target: "\(prefix).start", role: "button", label: "Start", enabled: true,
            actions: BernControlActions()))
    }
}

/// Like an `Adaptive`: it shows the wide layout until it has measured the
/// room it is given.
struct Measured: View {
    @State private var width: CGFloat?
    var body: some View {
        Group {
            if (width ?? .infinity) < 600 { buttons("phone") } else { buttons("desk") }
        }
        .frame(minWidth: 0, maxWidth: .infinity)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
    }
}

/// Like a modal that covers the window where it is compact: until the
/// window's width is known its controls mount in the floating panel, then
/// again in the cover.
struct Root: View {
    let window: BernWindowContext
    let flow: Flow
    var body: some View {
        ZStack {
            if flow.shown {
                if window.compact {
                    buttons("footer").padding(8)
                } else {
                    buttons("footer")
                }
            }
            if flow.page { Measured() }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { window.width = $0 }
        .environment(window)
    }
}

@main struct ControlRegistryTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 390, height: 600),
            styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let context = BernWindowContext()
        let flow = Flow()
        window.contentView = NSHostingView(rootView: Root(window: context, flow: flow))
        window.orderBack(nil)
        func spin(_ seconds: Double) { RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds)) }
        func targets() -> [String] { BernHost.shared.controls.all.map(\.target) }
        spin(0.5)
        precondition(context.compact, "the window's width arrives after the first layout")
        precondition(targets() == ["footer.skip", "footer.start"],
            "each mounted control is listed once, not also its first mount: \(targets())")
        do {
            _ = try BernHost.shared.controls.find("footer.skip")
        } catch {
            preconditionFailure("the control is found: \(error)")
        }
        flow.shown = false
        spin(0.5)
        precondition(targets().isEmpty, "controls that went are no longer listed: \(targets())")
        flow.shown = true
        spin(0.5)
        precondition(targets() == ["footer.skip", "footer.start"], "controls that come back are listed again: \(targets())")
        // A layout that measures itself once it is placed in a window
        // already on screen: only the layout it settles on is listed.
        flow.shown = false
        flow.page = true
        spin(0.5)
        precondition(targets() == ["phone.skip", "phone.start"],
            "the layout shown before the width was measured is not listed: \(targets())")
        window.close()
        print("control registry passed")
    }
}
"#;
    let source = root.join("ControlRegistry.swift");
    let binary = root.join("control-registry-test");
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

/// macOS 27 starts a view's task as the view appears, so the window above
/// cannot show what iOS 26 does: a task that starts later registers a
/// control that has already gone. The registration stays out of tasks.
#[test]
fn controls_register_as_they_appear_not_in_a_task() {
    let runtime = include_str!("../templates/BernRuntime.swift");
    let start = runtime
        .find("private struct BernControl: ViewModifier")
        .unwrap();
    let end = start + runtime[start..].find("\n}\n").unwrap();
    let control = &runtime[start..end];
    assert!(control.contains(".onAppear"), "{control}");
    assert!(control.contains(".onDisappear"), "{control}");
    assert!(!control.contains(".task"), "{control}");
}
