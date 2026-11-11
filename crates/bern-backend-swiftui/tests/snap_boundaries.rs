//! Check snap geometry with the actual Swift runtime types.
#[test]
#[cfg(target_os = "macos")]
fn snap_ends_follow_the_last_item_and_clear_after_removal() {
    use std::{path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("final class BernSnapItems:").unwrap();
    let end = start
        + style[start..]
            .find("\nprivate struct BernSnapItemsKey")
            .unwrap();
    let output = root.join("../../target/bern-test/snap-boundaries");
    std::fs::create_dir_all(&output).unwrap();
    let source = format!(
        "import SwiftUI\n{}\n{}",
        &style[start..end],
        r#"
@main struct SnapBoundaryTest {
    @MainActor static func main() {
        let items = BernSnapItems()
        let first = UUID(), middle = UUID(), last = UUID()
        items.report(first, CGRect(x: 0, y: 0, width: 220, height: 120))
        items.report(middle, CGRect(x: 250, y: 150, width: 300, height: 200))
        items.report(last, CGRect(x: 580, y: 380, width: 160, height: 80))
        precondition(items.largest.ends == CGSize(width: 160, height: 80))
        precondition(items.rest(580, length: 600, axis: .horizontal, center: false) == 580)
        precondition(items.rest(380, length: 600, axis: .vertical, center: false) == 380)
        // Scrolling changes every origin equally, not the trailing size.
        items.report(first, CGRect(x: -580, y: -380, width: 220, height: 120))
        items.report(middle, CGRect(x: -330, y: -230, width: 300, height: 200))
        items.report(last, CGRect(x: 0, y: 0, width: 160, height: 80))
        precondition(items.largest.ends == CGSize(width: 160, height: 80))
        items.remove(last)
        precondition(items.largest.ends == CGSize(width: 300, height: 200))
        items.remove(middle)
        precondition(items.largest.ends == CGSize(width: 220, height: 120))
        items.remove(first)
        precondition(items.largest.ends == .zero)
        precondition(items.rest(0, length: 600, axis: .horizontal, center: false) == nil)
    }
}
"#,
    );
    let file = output.join("main.swift");
    let executable = output.join("snap-boundaries");
    std::fs::write(&file, source).unwrap();
    let result = Command::new("xcrun")
        .args(["swiftc", "-swift-version", "6", "-parse-as-library"])
        .arg(&file)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(executable).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
