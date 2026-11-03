//! `glow` and `blur` drawn by the actual SwiftUI runtime, off every
//! screen: the light lies where the window's pointer is, adds to the view,
//! stays inside its corner and goes when the pointer leaves; a blur stays
//! inside the view's bounds. Where the pointer comes from (hover, fingers)
//! needs a real window and is not covered here.
#[cfg(target_os = "macos")]
#[test]
fn a_glow_follows_the_pointer_and_a_blur_stays_inside() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/glow");
    fs::create_dir_all(&root).unwrap();
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("// MARK: - Glow").unwrap();
    let end = style
        .find("private struct BernSpin: ViewModifier {")
        .unwrap();
    let runtime = &style[start..end];
    let prelude = r#"
import AppKit
import SwiftUI
extension View {
    func bernGlow(_ radius: Double, opacity: Double, corner: Double) -> some View {
        modifier(BernGlow(radius: radius, opacity: opacity, corner: corner))
    }
    func bernBlur(_ radius: Double, corner: Double) -> some View {
        blur(radius: radius).clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
    }
}
"#;
    let tests = r#"
struct Glowing: View {
    let pointer: BernPointer
    var body: some View {
        ZStack {
            Color(white: 0.3).frame(width: 80, height: 80).bernGlow(1, opacity: 0.4, corner: 20)
            // Laid over the glowing view, as a title over a cover.
            Color(white: 0.3).frame(width: 20, height: 20)
        }
        .frame(width: 100, height: 100)
        .background(Color.black)
        .environment(\.bernPointer, pointer)
    }
}
struct Blurred: View {
    var body: some View {
        HStack(spacing: 0) { Color.black; Color.white }
            .frame(width: 40, height: 40)
            .bernBlur(6, corner: 0)
            .padding(10)
            .background(Color.red)
    }
}
@MainActor func shot<V: View>(_ view: V) -> NSBitmapImageRep {
    let renderer = ImageRenderer(content: view)
    renderer.scale = 1
    return NSBitmapImageRep(cgImage: renderer.cgImage!)
}
func level(_ rep: NSBitmapImageRep, _ x: Int, _ y: Int) -> Double {
    let c = rep.colorAt(x: x, y: y)!.usingColorSpace(.sRGB)!
    return Double(c.redComponent + c.greenComponent + c.blueComponent) / 3
}
func same(_ rep: NSBitmapImageRep, _ a: (Int, Int), _ b: (Int, Int)) -> Bool {
    rep.colorAt(x: a.0, y: a.1) == rep.colorAt(x: b.0, y: b.1)
}
@main struct GlowTest {
    @MainActor static func main() {
        let pointer = BernPointer()
        let dark = shot(Glowing(pointer: pointer))
        pointer.location = CGPoint(x: 50, y: 50)
        let lit = shot(Glowing(pointer: pointer))
        let base = level(dark, 30, 50)
        precondition(level(lit, 30, 50) > base + 0.1, "lit near the pointer: \(level(lit, 30, 50)) over \(base)")
        precondition(level(lit, 30, 50) > level(lit, 14, 50), "brighter at the centre than towards the radius")
        precondition(abs(level(lit, 50, 50) - base) < 0.01, "what lies on top stays as it is")
        precondition(abs(level(lit, 11, 11) - base) < 0.01, "the corner stays dark")
        pointer.location = CGPoint(x: 95, y: 50)
        let away = shot(Glowing(pointer: pointer))
        precondition(abs(level(away, 30, 50) - base) < 0.01, "dark when the pointer is outside")

        let blurred = shot(Blurred())
        let edge = level(blurred, 30, 30)
        precondition(edge > 0.2 && edge < 0.8, "black and white blur into grey: \(edge)")
        precondition(same(blurred, (9, 30), (2, 30)) && same(blurred, (50, 30), (58, 30)), "the blur does not spread past the bounds")
        print("glow passed")
    }
}
"#;
    let source = root.join("Glow.swift");
    let binary = root.join("glow-test");
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
