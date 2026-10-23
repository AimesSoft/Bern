//! Exercise native focus styling across window updates without removing focus.
#[cfg(target_os = "macos")]
#[test]
fn focus_rings_stay_hidden_for_new_controls_and_windows_while_editing_works() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/focus-appearance");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernAppKit.swift");
    let start = runtime.find("// MARK: - Focus appearance").unwrap();
    let end = runtime
        .find("/// Reports the NSWindow that hosts a window root.")
        .unwrap();
    let tests = r#"
@main struct FocusAppearanceTest {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.prohibited)
        BernFocusAppearance.install()
        BernFocusAppearance.install()

        func window() -> NSWindow {
            let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 500, height: 300),
                styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.orderBack(nil)
            return window
        }
        func update() {
            NotificationCenter.default.post(name: NSApplication.willUpdateNotification, object: app)
        }
        func assertNoRing(_ control: NSControl) {
            precondition(control.focusRingType == .none, "native view must not draw a ring")
            precondition(control.cell?.focusRingType == NSFocusRingType.none, "native cell must not draw a ring")
        }

        let main = window()
        let field = NSTextField(frame: NSRect(x: 20, y: 20, width: 200, height: 28))
        field.stringValue = "draft"
        field.focusRingType = .exterior
        main.contentView!.addSubview(field)
        precondition(main.makeFirstResponder(field))
        let editor = field.currentEditor()!
        update()
        assertNoRing(field)
        precondition(main.firstResponder === editor, "styling must preserve the field editor")
        editor.selectedRange = NSRange(location: 5, length: 0)
        editor.insertText(" edited")
        precondition(editor.string == "draft edited", "focused field must still accept text")

        // Controls that mount later, including in separate popover/sheet windows.
        let panel = window()
        let button = NSButton(title: "Open", target: nil, action: nil)
        let slider = NSSlider(value: 0.5, minValue: 0, maxValue: 1, target: nil, action: nil)
        for control in [button, slider] as [NSControl] {
            control.focusRingType = .exterior
            panel.contentView!.addSubview(control)
        }
        update()
        assertNoRing(button)
        assertNoRing(slider)
        precondition(button.isEnabled && slider.isEnabled, "styling must not disable controls")

        // A native control may be restyled while SwiftUI updates its content.
        field.focusRingType = .exterior
        update()
        assertNoRing(field)
        precondition(main.firstResponder === editor, "updates must not resign keyboard focus")
        panel.orderOut(nil)
        main.orderOut(nil)
    }
}
"#;
    let source = root.join("main.swift");
    fs::write(
        &source,
        format!("import AppKit\n{}\n{tests}", &runtime[start..end]),
    )
    .unwrap();
    let binary = root.join("focus-appearance-test");
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
