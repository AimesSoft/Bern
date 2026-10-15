//! Exercise the scroll tracker without launching an application window.
#[test]
#[cfg(target_os = "macos")]
fn scroll_tracking_batches_geometry_and_ignores_unrelated_keys() {
    use std::{path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("@MainActor\n@Observable\nfinal class BernScrollItems").unwrap();
    let end = style[start..].find("\nprivate struct BernScrollItemsKey").unwrap() + start;
    let output = root.join("../../target/bern-test/scroll-tracking");
    std::fs::create_dir_all(&output).unwrap();
    let source = format!(
        "import SwiftUI\nstruct BernMotion {{}}\n{}\n{}",
        &style[start..end],
        r#"
@main struct ScrollTrackingTest {
    @MainActor static func main() async throws {
        let items = BernScrollItems(keyType: Int64.self)
        items.viewport = CGSize(width: 100, height: 100)
        let first = AnyHashable(Int64(1)), second = AnyHashable(Int64(2))
        let near = CGRect(x: 40, y: 40, width: 20, height: 20)
        let far = CGRect(x: 40, y: 150, width: 20, height: 20)
        items.report(first, near)
        items.report(second, far)
        try await Task.sleep(for: .milliseconds(10))
        precondition(items.nearest == first)

        // An alphabet index next to the cards is not a numeric scroll target.
        items.report(AnyHashable("A"), near)
        items.report(first, far)
        items.report(second, near)
        precondition(items.nearest == first, "do not publish a half-measured layout")
        try await Task.sleep(for: .milliseconds(10))
        precondition(items.nearest == second)
        items.remove(second)
        try await Task.sleep(for: .milliseconds(10))
        precondition(items.nearest == first)

        // Initial positioning stays quiet, deliberate navigation nudges,
        // and taking over the scroll cancels a pending arrival.
        items.expect(first, nudge: false)
        try await Task.sleep(for: .milliseconds(180))
        precondition(items.stops == 1 && items.arrivals == 0)
        items.expect(first)
        items.report(first, near)
        try await Task.sleep(for: .milliseconds(180))
        precondition(items.arrived == first && items.arrivals == 1)
        items.expect(second)
        items.cancelArrival()
        try await Task.sleep(for: .milliseconds(180))
        precondition(items.stops == 2 && items.arrivals == 1)
        items.remove(first)
        try await Task.sleep(for: .milliseconds(10))
        precondition(items.nearest == nil)
    }
}
"#,
    );
    let file = output.join("main.swift");
    let executable = output.join("scroll-tracking");
    std::fs::write(&file, source).unwrap();
    let result = Command::new("xcrun")
        .args(["swiftc", "-swift-version", "6", "-parse-as-library"])
        .arg(&file).arg("-o").arg(&executable).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let result = Command::new(executable).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}
