//! Exercise the production accordion in an actual SwiftUI window: headers and
//! footers keep their space while the week grows and shrinks with the window.
#[cfg(target_os = "macos")]
#[test]
fn accordion_uses_remaining_height_and_keeps_explicit_heights() {
    use std::{fs, path::Path, process::Command};
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("struct BernAccordionWeights:").unwrap();
    let end = style.find("// MARK: - Grids").unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/accordion-height");
    fs::create_dir_all(&dir).unwrap();
    let source = format!(
        "{}\n{}\n{}",
        r#"
import AppKit
import SwiftUI
struct BernMotion { var animation: Animation { .linear(duration: 0) } }
@propertyWrapper struct BernReducedMotion: DynamicProperty { var wrappedValue: Bool { true } }
let bernForcedReduceMotion = true
"#,
        &style[start..end],
        r#"
@MainActor @Observable final class Measurements {
    var fixedHeight: CGFloat? = nil
    var selected = 0
    var cells: [Int: CGSize] = [:]
}
struct Page: View {
    let measurements: Measurements
    var body: some View {
        VStack(spacing: 0) {
            Text("Heading").frame(height: 64)
            BernAccordion(count: 7, selected: measurements.selected, collapsedWidth: 86,
                          gap: 8, height: measurements.fixedHeight, motion: nil) {
                ForEach(0..<7) { index in
                    BernAccordionCell(corner: 12) {
                        Color.gray.onGeometryChange(for: CGSize.self) { $0.size } action: {
                            measurements.cells[index] = $0
                        }
                    }
                }
            }
            Color.clear.frame(height: 24)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}
@main struct HeightTest {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.prohibited)
        let measurements = Measurements()
        let host = NSHostingView(rootView: Page(measurements: measurements))
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 1100, height: 700),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = host
        func settle() {
            window.contentView?.layoutSubtreeIfNeeded()
            RunLoop.main.run(until: Date().addingTimeInterval(0.15))
        }
        for height: CGFloat in [700, 1020, 400, 800] {
            window.setContentSize(NSSize(width: 1100, height: height))
            settle()
            precondition(measurements.cells.count == 7)
            for size in measurements.cells.values {
                precondition(abs(size.height - (height - 88)) < 1,
                             "window=\(height), cell=\(size.height), expected=\(height - 88)")
            }
            precondition(abs(measurements.cells.values.reduce(0) { $0 + $1.width } + 48 - 1100) < 1)
        }
        measurements.selected = 4
        settle()
        precondition(abs(measurements.cells[4]!.width - 536) < 1)
        measurements.fixedHeight = 320
        settle()
        precondition(measurements.cells.values.allSatisfy { abs($0.height - 320) < 1 })
        window.setContentSize(NSSize(width: 1100, height: 1000))
        settle()
        precondition(measurements.cells.values.allSatisfy { abs($0.height - 320) < 1 })
    }
}
"#
    );
    let file = dir.join("main.swift");
    let binary = dir.join("accordion-height");
    fs::write(&file, source).unwrap();
    let result = Command::new("xcrun")
        .args(["swiftc", "-swift-version", "6", "-parse-as-library"])
        .arg(&file)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
