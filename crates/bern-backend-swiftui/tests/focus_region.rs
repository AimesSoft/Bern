//! A real AppKit editor and companion buttons exercise the shared region guard.
#[cfg(target_os = "macos")]
#[test]
fn focus_region_consumes_dismissal_but_keeps_inside_actions() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/focus-region");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernAppKit.swift");
    let start = runtime.find("private struct BernFocusRegionKey:").unwrap();
    let end = runtime[start..].find("/// A message composer").unwrap() + start;
    let tests = r#"
@main struct Test {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 500, height: 300),
            styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let field = NSTextField(frame: NSRect(x: 20, y: 20, width: 180, height: 28))
        field.stringValue = "保留未发送的弹幕"
        let second = NSTextField(frame: NSRect(x: 350, y: 20, width: 100, height: 28))
        let observer = BernFocusRegionPress.Observer(frame: NSRect(x: 10, y: 10, width: 300, height: 50))
        window.contentView!.addSubview(field)
        window.contentView!.addSubview(second)
        window.contentView!.addSubview(observer)
        window.orderBack(nil)
        func event(_ type: NSEvent.EventType, _ point: NSPoint, number: Int? = nil) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: 0,
                windowNumber: number ?? window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        }
        let outside = NSPoint(x: 400, y: 150), inside = NSPoint(x: 100, y: 30), send = NSPoint(x: 270, y: 30)
        for playing in [false, true] {
            var state = playing
            window.makeFirstResponder(field)
            precondition(!observer.consume(event(.leftMouseDown, inside)), "input stays editable")
            precondition(!observer.consume(event(.leftMouseDown, send)), "Send is inside the region")
            precondition(field.currentEditor() != nil, "inside clicks retain editing")
            precondition(!observer.consume(event(.leftMouseDown, outside, number: -1)), "other windows pass")
            if !observer.consume(event(.leftMouseDown, outside)) { state.toggle() }
            precondition(state == playing, "dismissing press must not toggle playback")
            precondition(field.currentEditor() == nil, "outside press blurs immediately")
            precondition(observer.consume(event(.leftMouseDragged, outside)), "dismissal cannot become a drag")
            precondition(observer.consume(event(.leftMouseUp, outside)), "release is consumed after focus is gone")
            precondition(field.stringValue == "保留未发送的弹幕", "draft survives")
            if !observer.consume(event(.leftMouseDown, outside)) { state.toggle() }
            precondition(state != playing, "next click works")
            precondition(!observer.consume(event(.leftMouseUp, outside)), "next release works")
        }
        window.makeFirstResponder(second)
        precondition(!observer.consume(event(.leftMouseDown, outside)), "ordinary fields keep their behavior")
        precondition(second.currentEditor() != nil, "other fields keep focus")
        window.makeFirstResponder(field)
        observer.isHidden = true
        precondition(!observer.consume(event(.leftMouseDown, outside)), "hidden regions never intercept")
        observer.stop()
        window.orderOut(nil)
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
    let binary = root.join("focus-region-test");
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
