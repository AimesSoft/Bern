//! Exercise the actual AppKit coordinator across separate native containers.
//! No application engine or screenshot permissions are needed.
#[cfg(target_os = "macos")]
#[test]
fn image_heroes_cross_columns_reverse_and_respect_reduced_motion() {
    use std::{fs, process::Command};
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/image-hero");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernAppKit.swift")
        .split("// MARK: - Window-level image heroes")
        .nth(1)
        .unwrap();
    let prelude = r#"
import AppKit
import SwiftUI
struct BernMotion { let period: Double; let damping: Double }
let bernMotionScale: Double = 1
let bernForcedReduceMotion = false
@propertyWrapper struct BernReducedMotion: DynamicProperty { var wrappedValue: Bool { false } }
enum BernImageFit { case contain, cover, fill }
enum BernImage {
    struct Picture { let image: CGImage; let orientation: SwiftUI.Image.Orientation; let ratio: CGFloat }
}
"#;
    let tests = r#"
@main struct HeroTest {
    @MainActor static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 800, height: 600),
            styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.orderBack(nil)
        let root = window.contentView!
        let left = NSView(frame: NSRect(x: 0, y: 0, width: 220, height: 600))
        let right = NSView(frame: NSRect(x: 220, y: 0, width: 580, height: 600))
        root.addSubview(left); root.addSubview(right)
        let bitmap = CGContext(data: nil, width: 16, height: 9, bitsPerComponent: 8, bytesPerRow: 64,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        bitmap.setFillColor(NSColor.red.cgColor); bitmap.fill(CGRect(x: 0, y: 0, width: 16, height: 9))
        let picture = BernImage.Picture(image: bitmap.makeImage()!, orientation: .up, ratio: 16.0 / 9.0)
        func anchor(_ parent: NSView, _ frame: CGRect, key: String = "frame", reduced: Bool = false) -> BernPictureHeroAnchor {
            let view = BernPictureHeroAnchor(frame: frame)
            view.key = key; view.picture = picture; view.reduced = reduced
            parent.addSubview(view)
            return view
        }
        func spin(_ seconds: Double) { RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds)) }
        func ghosts() -> [BernHeroFlightView] {
            (root.superview?.subviews.compactMap { $0 as? BernHeroOverlay }.flatMap { $0.subviews } ?? [])
                .compactMap { $0 as? BernHeroFlightView }
        }
        var sourceHidden = false, destinationHidden = false
        let source = anchor(left, CGRect(x: 10, y: 30, width: 200, height: 112.5))
        source.visibility = { sourceHidden = $0 }
        spin(0.05)
        precondition(ghosts().isEmpty, "initial mount must not fly")
        let destination = anchor(right, CGRect(x: 20, y: 160, width: 540, height: 303.75))
        destination.visibility = { destinationHidden = $0 }
        spin(0.08)
        precondition(ghosts().count == 1 && sourceHidden && destinationHidden, "one flight above both columns")
        let halfway = ghosts()[0].frame
        precondition(halfway.width > 200 && halfway.width < 540, "must interpolate geometry")
        destination.removeFromSuperview()
        let restored = anchor(left, CGRect(x: 10, y: 30, width: 200, height: 112.5))
        spin(0.04)
        precondition(ghosts().count == 1, "reverse replaces the running flight")
        precondition(ghosts()[0].frame.width < halfway.width + 50, "reverse starts near the current pixels")
        spin(1.3)
        precondition(ghosts().isEmpty && !sourceHidden && !destinationHidden, "settling restores originals and removes overlays")
        restored.removeFromSuperview()
        let reduced = anchor(right, CGRect(x: 20, y: 160, width: 540, height: 303.75), reduced: true)
        spin(0.08)
        precondition(ghosts().isEmpty && !sourceHidden, "reduced motion skips geometry animation")
        reduced.removeFromSuperview()
        let closing = anchor(right, CGRect(x: 20, y: 160, width: 540, height: 303.75))
        spin(0.08)
        closing.removeFromSuperview()
        spin(0.08)
        precondition(ghosts().isEmpty && !sourceHidden, "removing the destination cancels and cleans up")
        // Other elements fly what the window draws where they are.
        let tile = NSView(frame: CGRect(x: 0, y: 0, width: 220, height: 600))
        tile.wantsLayer = true; tile.layer?.backgroundColor = NSColor.blue.cgColor
        left.addSubview(tile, positioned: .below, relativeTo: nil)
        func viewAnchor(_ parent: NSView, _ frame: CGRect) -> BernPictureHeroAnchor {
            let view = BernPictureHeroAnchor(frame: frame)
            view.key = "window"; view.capturesView = true
            parent.addSubview(view)
            return view
        }
        var previewHidden = false, shellHidden = false
        let preview = viewAnchor(left, CGRect(x: 10, y: 30, width: 150, height: 94))
        preview.visibility = { previewHidden = $0 }
        spin(0.05)
        precondition(ghosts().isEmpty, "a view's first mount must not fly")
        let shell = viewAnchor(right, CGRect(x: 0, y: 0, width: 580, height: 600))
        shell.visibility = { shellHidden = $0 }
        spin(0.08)
        precondition(ghosts().count == 1 && previewHidden && shellHidden, "a view flies as a picture of the window")
        precondition(ghosts()[0].frame.width > 150 && ghosts()[0].frame.width < 580, "the picture grows")
        spin(1.3)
        precondition(ghosts().isEmpty && !previewHidden && !shellHidden, "the view flight settles")
        window.close()
        print("image hero lifecycle passed")
    }
}
"#;
    let source = root.join("ImageHero.swift");
    let binary = root.join("image-hero-test");
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
