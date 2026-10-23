//! Exercise native layout: a video surface must reach the black bars of a
//! bounded player, including when standard controls take some of its height.
#[cfg(target_os = "macos")]
#[test]
fn video_surface_fills_the_container_independently_of_picture_aspect() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/video-sizing");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernMedia.swift");
    let start = runtime.find("struct BernVideoSizing:").unwrap();
    let end = runtime[start..].find("struct BernVideo:").unwrap() + start;
    let tests = r#"
@MainActor final class Surface { var view: NSView? }
struct Probe: NSViewRepresentable {
    let surface: Surface
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        surface.view = view
        return view
    }
    func updateNSView(_ view: NSView, context: Context) {}
}
@main struct VideoSizingTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        for fill in [false, true] {
            for size in [CGSize(width: 844, height: 390), CGSize(width: 1280, height: 800),
                         CGSize(width: 390, height: 844), CGSize(width: 300, height: 100)] {
                for controls in [false, true] {
                    let surface = Surface()
                    let bar: CGFloat = controls ? 46 : 0
                    let host = NSHostingView(rootView: VStack(spacing: 6) {
                        ZStack { Probe(surface: surface) }
                            .modifier(BernVideoSizing(fill: fill))
                            .frame(maxWidth: .infinity)
                        if controls { Color.clear.frame(height: 40) }
                    }.frame(width: size.width, height: size.height))
                    host.sizingOptions = []
                    let window = NSWindow(contentRect: CGRect(origin: CGPoint(x: -10000, y: -10000), size: size),
                                          styleMask: [.borderless], backing: .buffered, defer: false)
                    window.isReleasedWhenClosed = false
                    window.contentView = host
                    window.orderBack(nil)
                    host.layoutSubtreeIfNeeded()
                    RunLoop.current.run(until: Date().addingTimeInterval(0.03))
                    host.layoutSubtreeIfNeeded()
                    let actual = surface.view!.bounds.size
                    let height = size.height - bar
                    let expected = fill ? CGSize(width: size.width, height: height)
                        : CGSize(width: min(size.width, height * 16 / 9), height: min(height, size.width * 9 / 16))
                    precondition(abs(actual.width - expected.width) < 1 && abs(actual.height - expected.height) < 1,
                        "fill=\(fill) controls=\(controls) container=\(size): \(actual) != \(expected)")
                    window.orderOut(nil)
                }
            }
        }
        print("16 native video layout cases passed")
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
    let binary = root.join("video-sizing-test");
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
        "{}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
}
