//! Outside dismissal must consume the press before the underlying player sees it.
#[cfg(target_os = "macos")]
#[test]
fn popover_dismissal_consumes_only_the_outside_click() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/popover-dismissal");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernAppKit.swift");
    let start = runtime.find("struct BernPopoverOutsidePress:").unwrap();
    let end = runtime[start..].find("/// `idle_area`:").unwrap() + start;
    let tests = r#"
@main struct PopoverDismissalTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        func makeWindow() -> NSWindow {
            let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 500, height: 300),
                styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.orderBack(nil)
            return window
        }
        let window = makeWindow()
        let popover = makeWindow()
        let other = makeWindow()
        let observer = BernPopoverOutsidePress.Observer(frame: NSRect(x: 420, y: 250, width: 40, height: 30))
        window.contentView!.addSubview(observer)
        var dismissals = 0
        observer.dismiss = { dismissals += 1 }
        func event(_ type: NSEvent.EventType, in owner: NSWindow) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: NSPoint(x: 100, y: 100), modifierFlags: [],
                timestamp: 0, windowNumber: owner.windowNumber, context: nil,
                eventNumber: 0, clickCount: 1, pressure: 1)!
        }
        let clicks: [(NSEvent.EventType, NSEvent.EventType)] = [(.leftMouseDown, .leftMouseUp), (.rightMouseDown, .rightMouseUp)]
        for (down, up) in clicks {
            observer.active = true
            let before = dismissals
            precondition(!observer.consume(event(down, in: popover)), "popover controls receive input")
            precondition(!observer.consume(event(down, in: other)), "unrelated windows receive input")
            precondition(dismissals == before, "only an outside press in the owning window dismisses")
            precondition(observer.consume(event(down, in: window)), "dismissal press never reaches the video")
            precondition(dismissals == before + 1, "dismiss exactly once")
            observer.active = false // SwiftUI has already removed the presentation.
            precondition(observer.consume(event(up, in: window)), "release cannot activate a control underneath")
            precondition(!observer.consume(event(down, in: window)), "a new click works after dismissal")
            precondition(!observer.consume(event(up, in: window)), "a new release works after dismissal")
        }
        func key(_ code: UInt16, in owner: NSWindow) -> NSEvent {
            NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                windowNumber: owner.windowNumber, context: nil, characters: "", charactersIgnoringModifiers: "",
                isARepeat: false, keyCode: code)!
        }
        window.addChildWindow(popover, ordered: .above)
        for owner in [window, popover] {
            observer.active = true
            let before = dismissals
            precondition(!observer.consume(key(53, in: other)), "Escape in another window passes")
            precondition(!observer.consume(key(49, in: owner)), "other keys pass")
            precondition(observer.consume(key(53, in: owner)), "Escape only closes the popover")
            precondition(dismissals == before + 1, "Escape dismisses once")
            precondition(!observer.consume(key(53, in: owner)), "next Escape reaches the player")
        }
        observer.stop()
        for item in [window, popover, other] { item.orderOut(nil) }
    }
}
"#;
    let source = root.join("main.swift");
    fs::write(
        &source,
        format!(
            "import AppKit\nimport SwiftUI\n{}\n{tests}",
            &runtime[start..end]
        ),
    )
    .unwrap();
    let binary = root.join("popover-dismissal-test");
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
