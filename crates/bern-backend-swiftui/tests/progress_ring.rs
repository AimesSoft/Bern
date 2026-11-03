//! `Progress(style: ring)` drawn by the actual SwiftUI runtime, off every
//! screen: a quarter lies from the top to the right, in the accent, over
//! nothing else; at nothing yet, a dot sits at the top; a whole ring goes
//! all the way round. The ring takes the width it is given.
#[cfg(target_os = "macos")]
#[test]
fn a_progress_ring_draws_its_arc_from_the_top_clockwise() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/progress-ring");
    fs::create_dir_all(&root).unwrap();
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("// MARK: - Progress ring").unwrap();
    let end = style.find("// MARK: - Whole-window pages").unwrap();
    let runtime = &style[start..end];
    let prelude = r#"
import AppKit
import SwiftUI
private struct BernAccentKey: EnvironmentKey {
    static let defaultValue: Color? = Color(red: 1, green: 0, blue: 0)
}
extension EnvironmentValues {
    var bernAccent: Color? {
        get { self[BernAccentKey.self] }
        set { self[BernAccentKey.self] = newValue }
    }
}
"#;
    let tests = r#"
@MainActor func shot(_ value: Double) -> NSBitmapImageRep {
    let ring = BernProgressRing(value: value, total: 1, width: 40, lineWidth: 4, label: nil)
        .transaction { $0.disablesAnimations = true }
        .background(Color.white)
    let renderer = ImageRenderer(content: ring)
    renderer.scale = 1
    return NSBitmapImageRep(cgImage: renderer.cgImage!)
}
/// Whether the pixel is the accent: red, not white.
func red(_ rep: NSBitmapImageRep, _ x: Int, _ y: Int) -> Bool {
    let c = rep.colorAt(x: x, y: y)!.usingColorSpace(.sRGB)!
    return c.redComponent > 0.8 && c.greenComponent < 0.4
}
@main struct RingTest {
    @MainActor static func main() {
        let quarter = shot(0.25)
        precondition(quarter.pixelsWide == 40 && quarter.pixelsHigh == 40, "the width it is given")
        precondition(red(quarter, 30, 5), "the arc runs from the top towards the right")
        precondition(red(quarter, 37, 20), "and reaches the right at a quarter")
        precondition(!red(quarter, 20, 37), "nothing at the bottom")
        precondition(!red(quarter, 2, 20), "nothing on the left")
        precondition(!red(quarter, 20, 20), "nothing in the middle")

        let none = shot(0)
        precondition(red(none, 20, 2), "a dot at the top, so it is seen to have started")
        precondition(!red(none, 37, 20), "and no more")

        let whole = shot(1)
        for (x, y) in [(20, 2), (37, 20), (20, 37), (2, 20)] {
            precondition(red(whole, x, y), "a whole ring goes all the way round: \(x), \(y)")
        }
        print("ring passed")
    }
}
"#;
    let source = root.join("Ring.swift");
    let binary = root.join("ring-test");
    fs::write(&source, format!("{prelude}\n{runtime}\n{tests}")).unwrap();
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
