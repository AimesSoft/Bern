//! Exercise the AppKit outside-press handler in an off-screen window.
#[cfg(target_os = "macos")]
#[test]
fn composer_outside_press_preserves_draft_and_other_fields() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/composer-focus");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernAppKit.swift");
    let start = runtime.find("private struct BernFocusRegionKey:").unwrap();
    let end = runtime[start..].find("/// `idle_area`:").unwrap() + start;
    let tests = r#"
@main struct ComposerFocusTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 500, height: 300),
            styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let field = NSTextField(frame: NSRect(x: 20, y: 20, width: 200, height: 28))
        field.stringValue = "保留未发送的弹幕"
        let second = NSTextField(frame: NSRect(x: 260, y: 20, width: 200, height: 28))
        let observer = BernComposerOutsidePress.Observer(frame: field.frame)
        window.contentView!.addSubview(field)
        window.contentView!.addSubview(second)
        window.contentView!.addSubview(observer)
        window.orderBack(nil)
        var dismissed = 0
        observer.active = true
        observer.dismiss = { dismissed += 1; observer.active = false }
        func event(_ point: NSPoint, number: Int? = nil) -> NSEvent {
            NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [],
                timestamp: 0, windowNumber: number ?? window.windowNumber, context: nil,
                eventNumber: 0, clickCount: 1, pressure: 1)!
        }
        window.makeFirstResponder(field)
        observer.handle(event(NSPoint(x: 100, y: 30)))
        precondition(dismissed == 0 && field.currentEditor() != nil, "inside click keeps editing")
        observer.handle(event(NSPoint(x: 300, y: 100), number: -1))
        precondition(dismissed == 0, "another window does not steal focus")
        observer.handle(event(NSPoint(x: 300, y: 100)))
        precondition(dismissed == 1 && field.currentEditor() == nil, "outside press ends editing")
        precondition(field.stringValue == "保留未发送的弹幕", "blur preserves draft")
        window.makeFirstResponder(second)
        observer.handle(event(NSPoint(x: 300, y: 30)))
        precondition(dismissed == 1 && second.currentEditor() != nil, "inactive composer leaves other field alone")
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
    let binary = root.join("composer-focus-test");
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
