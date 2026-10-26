//! The bounded preview must not instantiate hidden rows, including while resizing.
#[cfg(target_os = "macos")]
#[test]
fn preview_mounts_only_complete_rows_and_reserves_overflow() {
    use std::{fs, path::Path, process::Command};
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("enum BernPreviewGeometry").unwrap();
    let end = style.find("struct BernAccordionWeights:").unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/preview-list");
    fs::create_dir_all(&dir).unwrap();
    let source = format!(
        "import AppKit\nimport SwiftUI\n{}\n{}",
        &style[start..end],
        r#"
@MainActor @Observable final class Probe {
    var total = 1000
    var active: Set<Int> = []
    var created: Set<Int> = []
}
struct Page: View {
    let probe: Probe
    var body: some View {
        Button(action: {}) {
            BernPreviewList(items: Array(0..<probe.total), id: \.self,
                            itemHeight: 97, gap: 10, moreHeight: 22) { index in
                Color.gray.frame(width: 52, height: 97)
                    .onAppear { probe.active.insert(index); probe.created.insert(index) }
                    .onDisappear { probe.active.remove(index) }
            }
        }.buttonStyle(.plain)
    }
}
@main struct Test {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.prohibited)
        let probe = Probe()
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 86, height: 650),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: Page(probe: probe))
        func settle() {
            window.contentView?.layoutSubtreeIfNeeded()
            RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        }
        for (height, count): (CGFloat, Int) in [(650, 5), (900, 8), (300, 2), (22, 0), (450, 4)] {
            window.setContentSize(NSSize(width: 86, height: height))
            settle()
            precondition(probe.active == Set(0..<count), "height=\(height) mounted=\(probe.active)")
            precondition(probe.created.allSatisfy { $0 < 8 }, "hidden rows must never be mounted")
        }
        probe.total = 3
        window.setContentSize(NSSize(width: 86, height: 311))
        settle()
        precondition(probe.active == Set(0..<3), "all rows fit without an overflow badge")
        probe.total = 0
        settle()
        precondition(probe.active.isEmpty)
        for total in [0, 1, 3, 1000] {
            for height in stride(from: CGFloat(0), through: 2000, by: 1) {
                let count = BernPreviewGeometry.visible(total: total, height: height, item: 97, gap: 10, more: 22)
                precondition(count <= total)
                if count > 0 {
                    let used = count < total ? CGFloat(count) * 107 + 22 : CGFloat(count) * 107 - 10
                    precondition(used <= height)
                }
            }
        }
    }
}
"#
    );
    let file = dir.join("main.swift");
    let binary = dir.join("preview-list");
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
