//! A modal without a height is as tall as its content, a scroll view in it
//! as tall as what it scrolls, and no taller than the window leaves it; one
//! with a height keeps it. Exercises the runtime's `BernModalStage` in
//! off-screen windows; no application engine or screenshot permissions are
//! needed.
#[cfg(target_os = "macos")]
#[test]
fn modals_without_a_height_fit_their_content() {
    use std::{fs, process::Command};
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/modal-fit");
    fs::create_dir_all(&root).unwrap();
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("private struct BernModalStage").unwrap();
    let end = start + style[start..].find("/// Marks a `Spacer`").unwrap();
    let stage = style[start..end].replacen("private struct", "struct", 1);
    let tests = r#"
import AppKit
import SwiftUI

@MainActor final class Measured {
    static var card = CGSize.zero
}

/// A dialog's column: a title, a scroll view of `rows` rows and a button.
struct Dialog: View {
    let rows: Int
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Title").font(.title3)
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(0..<rows, id: \.self) { Text("Row \($0)") }
                }
            }
            HStack { Spacer(minLength: 0); Button("Done") {} }
        }
        .padding(24)
    }
}

@MainActor func spin(_ seconds: Double) { RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds)) }

/// The card's size for a modal `width` wide and `height` tall (none: as its
/// content) in a window of `window`.
@MainActor func card(rows: Int, width: CGFloat, height: CGFloat?, window: CGSize) -> CGSize {
    Measured.card = .zero
    let shown = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: window.width, height: window.height),
        styleMask: [.borderless], backing: .buffered, defer: false)
    shown.isReleasedWhenClosed = false
    shown.contentView = NSHostingView(rootView: BernModalStage(width: width, height: height) {
        Color.black.opacity(0.24)
        Dialog(rows: rows)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .onGeometryChange(for: CGSize.self) { $0.size } action: { Measured.card = $0 }
    }.frame(width: window.width, height: window.height))
    shown.orderBack(nil)
    spin(0.3)
    shown.close()
    return Measured.card
}

/// How tall the dialog's column is at `width` with nothing holding it.
@MainActor func natural(rows: Int, width: CGFloat) -> CGFloat {
    NSHostingView(rootView: Dialog(rows: rows).frame(width: width)).fittingSize.height
}

func near(_ a: CGFloat, _ b: CGFloat) -> Bool { abs(a - b) < 1 }

@main struct ModalFitTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        let window = CGSize(width: 400, height: 700)
        // A short dialog: as tall as its content.
        let short = card(rows: 4, width: 320, height: nil, window: window)
        let wanted = natural(rows: 4, width: 320)
        precondition(near(short.width, 320), "the card is as wide as declared: \(short)")
        precondition(wanted < 400 && near(short.height, wanted), "the card is as tall as its content, \(wanted): \(short)")
        // A long one: as tall as the window leaves it, 16 points from each
        // edge and below the toolbar's room on macOS; its rows scroll.
        let long = card(rows: 80, width: 320, height: nil, window: window)
        precondition(near(long.height, 700 - 52 - 32), "the card stops at the window's edges: \(long)")
        // A declared height stays, whatever the content.
        let declared = card(rows: 4, width: 320, height: 500, window: window)
        precondition(near(declared.height, 500), "the card is as tall as declared: \(declared)")
        // A narrow window: the card keeps 16 points from its sides.
        let narrow = card(rows: 4, width: 320, height: nil, window: CGSize(width: 300, height: 700))
        precondition(near(narrow.width, 268), "the card fits the window's width: \(narrow)")
        precondition(near(narrow.height, natural(rows: 4, width: 268)), "its height follows its width: \(narrow)")
        print("modal fit passed")
    }
}
"#;
    let source = root.join("ModalFit.swift");
    let binary = root.join("modal-fit-test");
    fs::write(&source, format!("{tests}\n{stage}")).unwrap();
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
