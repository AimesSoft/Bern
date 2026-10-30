//! `spin` drawn by the actual SwiftUI runtime in a window off every
//! screen: it turns while its condition holds, stops where it is, and
//! goes on from there. No application engine or screenshot permissions
//! are needed.
#[cfg(target_os = "macos")]
#[test]
fn a_spinning_view_turns_stops_where_it_is_and_goes_on() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/spin");
    fs::create_dir_all(&root).unwrap();
    let style = include_str!("../templates/BernStyle.swift");
    let start = style
        .find("private struct BernSpin: ViewModifier {")
        .unwrap();
    let end = style
        .find("private struct BernNudge: ViewModifier {")
        .unwrap();
    let runtime = &style[start..end];
    let prelude = r#"
import AppKit
import SwiftUI
let bernForcedReduceMotion = false
@propertyWrapper struct BernReducedMotion: DynamicProperty { var wrappedValue: Bool { false } }
extension View {
    func bernSpin(_ on: Bool, period: Double) -> some View { modifier(BernSpin(on: on, period: period)) }
}
"#;
    let tests = r#"
final class Model: ObservableObject { @Published var on = true }
struct Probe: View {
    @ObservedObject var model: Model
    var body: some View {
        Rectangle().fill(Color.red).frame(width: 40, height: 10)
            .bernSpin(model.on, period: 2)
            .frame(width: 60, height: 60)
            .background(Color.white)
    }
}
@main struct SpinTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        let model = Model()
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 60, height: 60),
            styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let host = NSHostingView(rootView: Probe(model: model))
        host.frame = NSRect(x: 0, y: 0, width: 60, height: 60)
        window.contentView = host
        window.orderBack(nil)
        func wait(_ seconds: Double) { RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds)) }
        func shot() -> [UInt8] {
            let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds)!
            host.cacheDisplay(in: host.bounds, to: rep)
            return Array(UnsafeBufferPointer(start: rep.bitmapData!, count: rep.bytesPerRow * rep.pixelsHigh))
        }
        func changed(_ a: [UInt8], _ b: [UInt8]) -> Int { zip(a, b).filter { $0 != $1 }.count }
        wait(0.3)
        let first = shot(); wait(0.4)
        precondition(changed(first, shot()) > 500, "it turns")
        model.on = false
        wait(0.2)
        let stopped = shot(); wait(0.4)
        precondition(changed(stopped, shot()) == 0, "it stops")
        precondition(changed(stopped, first) > 500, "where it is, not back at the start")
        model.on = true
        wait(0.2)
        let again = shot(); wait(0.3)
        precondition(changed(again, shot()) > 500, "it goes on")
        window.close()
        print("spin passed")
    }
}
"#;
    let source = root.join("Spin.swift");
    let binary = root.join("spin-test");
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
