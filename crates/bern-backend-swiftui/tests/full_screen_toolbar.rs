//! A detached toolbar keeps its controls; only its decoration is transparent.
#[cfg(target_os = "macos")]
#[test]
fn toolbar_decoration_follows_the_button_and_restores_per_window() {
    use std::{fs, process::Command};

    let template = include_str!("../templates/BernAppKit.swift");
    let implementation = template
        .split_once("// MARK: - Full-screen toolbar backdrop")
        .unwrap()
        .1
        .split_once("private struct BernUnifiedBarAppearance")
        .unwrap()
        .0;
    let test = r#"
@MainActor final class Decoration: NSView {}
@MainActor final class FixtureWindow: NSWindow {
    var closeControl: NSButton?
    override func standardWindowButton(_ kind: NSWindow.ButtonType) -> NSButton? {
        if kind == .closeButton, let closeControl { return closeControl }
        return super.standardWindowButton(kind)
    }
}
@main enum BackdropTest {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.prohibited)
        func window() -> FixtureWindow {
            let w = FixtureWindow(contentRect: NSRect(x: -9000, y: -9000, width: 400, height: 300),
                styleMask: [.titled, .closable], backing: .buffered, defer: false)
            w.isReleasedWhenClosed = false
            w.toolbar = NSToolbar(identifier: UUID().uuidString)
            w.toolbar?.isVisible = true
            return w
        }
        func notify(_ name: Notification.Name, _ object: AnyObject) {
            NotificationCenter.default.post(name: name, object: object)
        }
        let main = window(), other = window(), host = window()
        let toolbar = main.toolbar
        let group = NSView(), button = NSButton(), otherButton = NSButton()
        main.closeControl = button; other.closeControl = otherButton
        main.contentView!.addSubview(group); group.addSubview(button)
        other.contentView!.addSubview(otherButton)
        let backdrop = Decoration(), unrelated = Decoration(), foreign = Decoration()
        group.addSubview(backdrop); button.addSubview(unrelated); other.contentView!.addSubview(foreign)
        backdrop.alphaValue = 0.37; backdrop.isHidden = true
        let original = backdrop.alphaValue
        let first = BernFullScreenToolbarBackdrop(backdropClass: Decoration.self)
        let second = BernFullScreenToolbarBackdrop(backdropClass: Decoration.self)
        first.follow(main); second.follow(other)
        precondition(backdrop.alphaValue == original)
        notify(NSWindow.willEnterFullScreenNotification, main)
        precondition(backdrop.isHidden && backdrop.alphaValue == 0, "arm before AppKit unhides it")
        let added = Decoration(); added.alphaValue = 0.75
        group.removeFromSuperview(); host.contentView!.addSubview(group); group.addSubview(added)
        precondition(host.parent == nil && button.window === host)
        backdrop.isHidden = false
        notify(NSApplication.willUpdateNotification, app)
        precondition(backdrop.alphaValue == 0 && added.alphaValue == 0 && !backdrop.isHidden)
        precondition(foreign.alphaValue == 1 && unrelated.alphaValue == 1 && button.alphaValue == 1 && button.isEnabled)
        notify(NSWindow.willExitFullScreenNotification, main)
        group.removeFromSuperview(); main.contentView!.addSubview(group)
        precondition(backdrop.alphaValue == 0, "keep transparent on the way back")
        notify(NSWindow.didExitFullScreenNotification, main)
        precondition(backdrop.alphaValue == original && added.alphaValue == 0.75 && !backdrop.isHidden)
        precondition(main.toolbar === toolbar && toolbar?.isVisible == true)
        notify(NSWindow.willEnterFullScreenNotification, other)
        precondition(foreign.alphaValue == 0 && backdrop.alphaValue == original)
        second.follow(nil) // switching to immersive or detaching the window
        precondition(foreign.alphaValue == 1)
        notify(NSWindow.willEnterFullScreenNotification, main)
        notify(NSWindow.willCloseNotification, main)
        precondition(backdrop.alphaValue == original)
        notify(NSWindow.willEnterFullScreenNotification, main)
        precondition(backdrop.alphaValue == original, "a closed window must no longer be observed")
    }
}
"#;
    let dir = std::env::temp_dir().join(format!("bern-toolbar-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("main.swift");
    fs::write(
        &source,
        format!("import AppKit\nimport QuartzCore\n{implementation}\n{test}"),
    )
    .unwrap();
    let binary = dir.join("test");
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
    fs::remove_dir_all(dir).unwrap();
}
