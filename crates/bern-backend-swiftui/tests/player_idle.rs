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
    let style = include_str!("../templates/BernStyle.swift");
    let shortcuts_start = style.find("enum BernShortcuts {").unwrap();
    let shortcuts_end =
        style[shortcuts_start..].find("\nextension View {").unwrap() + shortcuts_start;
    let stubs = r#"
enum BernText { static func string(_ key: String) -> String { key } }
@MainActor final class Platform { var nsWindow: NSWindow? }
@MainActor final class BernWindowContext {
    final class QuietShortcut {
        var shortcut: KeyboardShortcut?
        var enabled: Bool
        init(shortcut: KeyboardShortcut?, enabled: Bool) {
            self.shortcut = shortcut
            self.enabled = enabled
        }
    }
    let platform = Platform()
    var idle = false
    var popoverOwner: UUID?
    var idleAreas: Set<UUID> = []
    var idleAreasHovered: [UUID: Double] = [:]
    var idleAreasChanged: (() -> Void)?
    var quietShortcuts: [QuietShortcut] = []
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
        // Quiet shortcuts (bare and modified) neither wake an idle window nor reset the countdown.
        let rightEntry = BernWindowContext.QuietShortcut(shortcut: BernShortcuts.keyboard("right"), enabled: true)
        let cmdLeftEntry = BernWindowContext.QuietShortcut(shortcut: BernShortcuts.keyboard("cmd+left"), enabled: true)
        context.quietShortcuts = [rightEntry, cmdLeftEntry]
        let rightKey = NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: [.numericPad, .function],
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 124
        )!
        monitor.handle(event: rightKey, after: 0.02)
        precondition(context.idle, "a quiet bare shortcut must not wake an idle window")
        let cmdLeftKey = NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: [.command, .numericPad, .function],
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 123
        )!
        monitor.handle(event: cmdLeftKey, after: 0.02)
        precondition(context.idle, "a quiet modified shortcut must not wake an idle window")
        // Disabling the quiet shortcut or pressing an ordinary key wakes the window.
        rightEntry.enabled = false
        monitor.handle(event: rightKey, after: 0.02)
        precondition(!context.idle, "a disabled quiet shortcut wakes the window")
        RunLoop.current.run(until: Date().addingTimeInterval(0.06))
        precondition(context.idle, "window returns to idle after waiting")
        let plainKey = NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: [],
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: " ", charactersIgnoringModifiers: " ", isARepeat: false, keyCode: 49
        )!
        monitor.handle(event: plainKey, after: 0.02)
        precondition(!context.idle, "an ordinary key wakes an idle window")
        RunLoop.current.run(until: Date().addingTimeInterval(0.06))
        precondition(context.idle, "window returns to idle again")
        context.quietShortcuts = []
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
            "import AppKit\nimport SwiftUI\n{stubs}\n{}\n{}",
            &style[shortcuts_start..shortcuts_end],
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

#[cfg(target_os = "macos")]
#[test]
fn pointer_hides_and_restores_with_immersive_idle() {
    let appkit = include_str!("../templates/BernAppKit.swift");
    assert!(appkit.contains("private var hidePointer: Bool { immersive && context.idle }"));
    assert!(appkit.contains(".pointerVisibility(hidden ? .hidden : .automatic)"));
    assert!(appkit.contains("NSCursor.setHiddenUntilMouseMoves(hidden)"));
    assert!(appkit.contains("lastMouseLocation"));

    let uikit = include_str!("../templates/BernUIKit.swift");
    assert!(uikit.contains("BernPointerHider"));
    assert!(uikit.contains("UIPointerStyle.hidden()"));
}
