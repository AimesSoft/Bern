//! A native fullscreen helper is not an open player popover.
#[cfg(target_os = "macos")]
#[test]
fn idle_ignores_fullscreen_helpers_but_keeps_popovers_awake() {
    use std::{fs, process::Command};
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/player-idle");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernAppKit.swift");
    let start = runtime
        .find("@MainActor\nfinal class BernEventMonitor")
        .unwrap();
    let end = runtime[start..].find("/// `submit: clear`").unwrap() + start;
    let stubs = r#"
@MainActor final class Platform { var nsWindow: NSWindow? }
@MainActor final class BernWindowContext {
    let platform = Platform()
    var idle = false
    var popoverOwner: UUID?
    var idleAreas: Set<UUID> = []
    var idleAreasHovered: [UUID: Double] = [:]
    var idleAreasChanged: (() -> Void)?
}
@main struct Test {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        let context = BernWindowContext()
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        let helper = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 20, height: 20), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        helper.isReleasedWhenClosed = false
        window.orderBack(nil)
        window.addChildWindow(helper, ordered: .above)
        helper.orderBack(nil)
        context.platform.nsWindow = window
        let id = UUID()
        context.idleAreas = [id]
        context.idleAreasHovered = [id: 0.02]
        let monitor = BernEventMonitor()
        monitor.start(context: context, after: 0.02)
        // Updates while awake must not keep extending the input deadline.
        for _ in 0..<8 {
            context.idleAreasChanged?()
            RunLoop.current.run(until: Date().addingTimeInterval(0.005))
        }
        precondition(context.idle, "redraws and native helper windows must not prevent idle")
        // Repainting a label may rebuild its native tracking area. Neither
        // leaving, re-entering nor re-registering it is user activity.
        context.idleAreasHovered = [:]
        context.idleAreasChanged?()
        precondition(context.idle, "hover loss on redraw must not wake an idle window")
        context.idleAreas.remove(id)
        context.idleAreasChanged?()
        context.idleAreas.insert(id)
        context.idleAreasHovered = [id: 0.02]
        context.idleAreasChanged?()
        RunLoop.current.run(until: Date().addingTimeInterval(0.06))
        precondition(context.idle, "redraw must not create a hide/show cycle")
        // Actual input still wakes the window. Popovers keep it awake.
        context.popoverOwner = UUID()
        monitor.activity(after: 0.02)
        precondition(!context.idle, "actual input must wake an idle window")
        RunLoop.current.run(until: Date().addingTimeInterval(0.10))
        precondition(!context.idle, "a real popover keeps controls awake")
        context.popoverOwner = nil
        context.idleAreasChanged?()
        RunLoop.current.run(until: Date().addingTimeInterval(0.10))
        precondition(context.idle, "dismissing a popover restores idle")
        context.idleAreasHovered = [:]
        context.idleAreasChanged?()
        monitor.activity(after: 0.02)
        RunLoop.current.run(until: Date().addingTimeInterval(0.06))
        precondition(!context.idle, "real input over controls keeps them awake")
        monitor.stop()
        helper.orderOut(nil)
        window.orderOut(nil)
    }
}
"#;
    let source = root.join("main.swift");
    fs::write(
        &source,
        format!(
            "import AppKit\nimport SwiftUI\n{stubs}\n{}",
            &runtime[start..end]
        ),
    )
    .unwrap();
    let binary = root.join("idle-test");
    let compiled = Command::new("xcrun")
        .args(["swiftc", "-swift-version", "6", "-parse-as-library"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let ran = Command::new(binary).output().unwrap();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
}
