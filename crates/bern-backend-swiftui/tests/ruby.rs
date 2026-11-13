//! Ruby keeps source scalar indices and uses the same Core Text layout on
//! both iOS front ends, tvOS and macOS. These tests are only run on explicit request.

use bern_backend_swiftui::{MINIMUM_TVOS, SwiftUiIos, SwiftUiMacos, SwiftUiTvos};
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

mod support;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn test_dir(name: &str) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target"))
        .join("bern-test")
        .join(name)
}

fn generate(platform: &str, backend: &dyn Target) -> PathBuf {
    let root = workspace();
    let dir = test_dir(&format!("ruby-{platform}"));
    let frontends = if platform == "ios" {
        "frontends = [\"swiftui\", \"swiftui-16\"]"
    } else {
        ""
    };
    for (package, source) in [
        (
            "ruby-protocol",
            r#"
#[bern::protocol(namespace = "ruby", version = 1)]
pub mod ruby {
    use bern::prelude::*;
    #[state]
    pub struct State {
        #[content]
        pub text: String,
        pub annotations: String,
        pub shown: bool,
        pub lit: f64,
        #[content]
        pub path: String,
        pub clock: f64,
        pub since: f64,
    }
    #[commands]
    pub trait Commands {
        fn refresh(&mut self) -> Result<(), CommandError>;
    }
}
"#,
        ),
        (
            "ruby-ui",
            r#"
use bern::prelude::*;
style! { text { body: font(size: 24, weight: medium), } }
#[component]
pub fn app(app: ruby::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Ruby" }, initial: open) {
                Column {
                    Text(content: app.text)
                    Text(content: app.text, ruby: app.annotations, style: large_title)
                    Text(content: app.text, ruby: "")
                    Text(content: app.text, ruby: "[]")
                    Text(content: app.text, ruby: if app.shown { app.annotations } else { "" })
                    Text(content: app.text, ruby: app.annotations, font_size: 30,
                         bold: true, color: accent, line_height: 52, lines: 2)
                    Text(content: app.text, ruby: app.annotations, lit: app.lit,
                         lit_path: app.path, lit_clock: app.clock, lit_since: app.since)
                    Text(content: app.text, ruby: app.annotations, lit: app.lit)
                }
            }
        }
    }
}
"#,
        ),
        ("ruby-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(package).join("src")).unwrap();
        std::fs::write(
            dir.join(package).join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(package).join("src/lib.rs"), source).unwrap();
    }
    std::fs::write(
        dir.join("Bern.toml"),
        format!(
            r#"
[app]
id = "ruby"
bundle_id = "dev.bern.ruby"
version = "0.1.0"
name = {{ "en" = "Ruby" }}
[languages]
supported = ["en"]
default = "en"
[protocol]
package = "ruby-protocol"
source = "ruby-protocol/src/lib.rs"
[engine]
package = "ruby-core"
type = "ruby_core::Engine"
[ui]
package = "ruby-ui"
sources = ["ruby-ui/src/lib.rs"]
root = "app"
[targets.{platform}]
platform = "{platform}"
{frontends}
"#
        ),
    )
    .unwrap();
    let config = bern_compiler::config::load(&dir.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: dir.join("generated"),
        target: config.targets[0].clone(),
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    };
    let app = compiled.app.on_platform(platform);
    for file in backend.generate(&app, &context).unwrap() {
        let path = context.output_dir.join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.contents).unwrap();
    }
    context.output_dir
}

fn assert_generated(output: &Path, sources: &str) {
    let generated =
        std::fs::read_to_string(output.join(sources).join("BernGenerated.swift")).unwrap();
    for expected in [
        "BernRubyText(text:",
        "ruby: \"\"",
        "ruby: \"[]\"",
        "size: 30.0",
        "style: \"body\", weight: \"medium\", bold: true",
        "lineHeight: 52.0, lines: 2",
        "lit: { Double(bernEngine.lit) }",
        "path: { BernText.text(bernEngine.path) }",
        "clock: { Double(bernEngine.clock) }",
        "since: { Double(bernEngine.since) }",
        "SwiftUI.Text(verbatim: BernText.text(bernEngine.text))",
        "BernLitText(text: BernText.text(bernEngine.text)",
        "bernLit(BernText.text(bernEngine.text), Double(bernEngine.lit))",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}:\n{generated}"
        );
    }
    let runtime = std::fs::read_to_string(output.join(sources).join("BernStyle.swift")).unwrap();
    for expected in [
        "import CoreText",
        "CTRubyAnnotationCreateWithAttributes",
        "return font as CTFont",
        "CTTypesetterSuggestLineBreak",
        "CTLineGetOffsetForStringIndex",
        ".accessibilityLabel(Text(verbatim: text))",
        "plain()",
    ] {
        assert!(runtime.contains(expected), "missing {expected}");
    }
}

#[test]
fn ruby_is_generated_for_both_ios_front_ends() {
    let output = generate("ios", &SwiftUiIos);
    assert_generated(&output, "Sources");
    assert_generated(&output, "Sources17");
    support::check_ios(&workspace(), &output);
}

#[test]
fn ruby_is_generated_for_tvos() {
    let output = generate("tvos", &SwiftUiTvos);
    assert_generated(&output, "Sources");
    let sources = output.join("Sources");
    let runtime = std::fs::read_to_string(sources.join("BernStyle.swift")).unwrap();
    assert!(runtime.contains(
        "#if os(tvOS)\n        let largeTitleSize: CGFloat = 68\n        #else\n        let largeTitleSize = UIFont.preferredFont(forTextStyle: .largeTitle).pointSize\n        #endif"
    ));
    support::check_front_end(
        &workspace(),
        &sources,
        &sources.join("bern.h"),
        ("appletvsimulator", "tvos", MINIMUM_TVOS),
        &[],
    );
}

#[test]
fn ruby_is_generated_for_macos() {
    let output = generate("macos", &SwiftUiMacos);
    assert_generated(&output, "Sources");
}

#[cfg(target_os = "macos")]
#[test]
fn native_ruby_preserves_indices_wraps_and_reserves_annotation_height() {
    use std::process::Command;
    let root = test_dir("ruby-native");
    std::fs::create_dir_all(&root).unwrap();
    let style = include_str!("../templates/BernStyle.swift");
    let ruby = &style
        [style.find("// MARK: - Ruby text").unwrap()..style.find("// MARK: - Text marks").unwrap()];
    let route = &style[style.find("struct BernLitRoute:").unwrap()
        ..style.find("/// A `Text` lit along").unwrap()];
    let tests = r##"
private extension BernRubyText {
    var testNativeFont: CTFont { nativeFont }
}

@main struct RubyTest {
    @MainActor static func main() {
        let text = "😀東京へ行く。東京へ行く。東京へ行く。";
        let spans = BernRubySpan.parse(#"[[1,3,"とうきょう"],[4,5,"い"]]"#, text: text)
        precondition(spans.count == 2)
        precondition(spans[0].range == NSRange(location: 2, length: 2))
        precondition(BernRubySpan.scalarOffsets("😀e\u{301}") == [0,2,3,4])
        for empty in ["", "[]", "invalid", #"[[true,2,"あ"]]"#, #"[[0.5,2,"あ"]]"#,
                      #"[[0,999,"あ"]]"#, #"[[-1,2,"あ"]]"#, #"[[2,1,"あ"]]"#] {
            precondition(BernRubySpan.parse(empty, text: text).isEmpty, empty)
        }
        let overlap = BernRubySpan.parse(#"[[1,3,"とうきょう"],[2,4,"きょう"]]"#, text: text)
        precondition(overlap.count == 1)
        let font = CTFontCreateWithName("Helvetica" as CFString, 24, nil)
        let document = BernRubyDocument(text: text, spans: spans, font: font, lineHeight: 50, limit: nil)
        let wide = document.layout(width: 1000)
        let narrow = document.layout(width: 90)
        precondition(wide.rows.count == 1)
        let ink = CTLineGetImageBounds(wide.rows[0].line, nil)
        precondition(ink.maxY > CTFontGetAscent(font), "ruby ink must stand above the base glyphs")
        precondition(narrow.rows.count > 1)
        precondition(narrow.size.height >= CGFloat(narrow.rows.count) * 50)
        precondition(narrow.offsets.count == text.unicodeScalars.count + 1)
        precondition(narrow.rows[0].baseline >= CTFontGetAscent(font) + 24 * 0.45)
        let limited = BernRubyDocument(text: text, spans: spans, font: font, lineHeight: nil, limit: 2).layout(width: 90)
        precondition(limited.rows.count == 2)
        let longText = "漢字と漢字"
        let longSpans = BernRubySpan.parse(#"[[0,1,"とてもながいよみかた"]]"#, text: longText)
        let long = BernRubyDocument(text: longText, spans: longSpans, font: font, lineHeight: nil, limit: nil).layout(width: 180)
        precondition(long.size.height > CTFontGetAscent(font) + CTFontGetDescent(font))
        let context = CGContext(data: nil, width: 200, height: 300, bitsPerComponent: 8, bytesPerRow: 0,
                                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        for place in [-1.0, 0, 1.5, 3, 100] {
            narrow.draw(in: context, mark: nil, lit: place)
        }
        // Ruby drawing must not reset the foreground to Core Text's default black.
        // Test saturated and translucent colors, both passes of highlighting,
        // wrapping and truncation rather than only a black-on-clear layout.
        for alpha in [1.0, 0.6] {
            for width in [90.0, 1000.0] {
                for place in [-1.0, 0.0, 1.5, 100.0] {
                    let color = CGColor(red: 1, green: 0, blue: 0, alpha: alpha)
                    // Color emoji deliberately retain their own palette.
                    let colorText = "東京へ行く。東京へ行く。東京へ行く。"
                    let colorSpans = BernRubySpan.parse(#"[[0,2,"とうきょう"],[3,4,"い"]]"#, text: colorText)
                    let drawing = BernRubyDocument(text: colorText, spans: colorSpans, font: font,
                                                  lineHeight: nil, limit: 2).layout(width: width, color: color)
                    let pixels = CGContext(data: nil, width: 1000, height: 300, bitsPerComponent: 8,
                                           bytesPerRow: 4000, space: CGColorSpaceCreateDeviceRGB(),
                                           bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
                    pixels.translateBy(x: 0, y: 300)
                    pixels.scaleBy(x: 1, y: -1)
                    drawing.draw(in: pixels, mark: nil, lit: place)
                    let data = pixels.data!.assumingMemoryBound(to: UInt8.self)
                    var ink = 0
                    for offset in stride(from: 0, to: 4000 * 300, by: 4) where data[offset + 3] > 20 {
                        ink += 1
                        // Core Graphics may color-convert red into the device RGB space.
                        precondition(Double(data[offset + 3]) <= ceil(alpha * 255) + 1,
                                     "highlight must not composite bright text over its dim copy")
                        precondition(abs(Int(data[offset]) - Int(data[offset + 3])) <= 3 &&
                                     Int(data[offset + 1]) * 3 < Int(data[offset]) &&
                                     Int(data[offset + 2]) * 3 < Int(data[offset]),
                                     "base text, ruby and ellipsis must retain the requested red")
                    }
                    precondition(ink > 0)
                }
            }
        }
        // Later wrapped rows have no highlighted characters yet. An empty
        // Core Graphics clipping path must not turn their bright pass on.
        let wrappedText = "漢字漢字漢字漢字"
        let wrapped = BernRubyDocument(text: wrappedText,
            spans: BernRubySpan.parse(#"[[0,2,"かんじ"]]"#, text: wrappedText),
            font: font, lineHeight: nil, limit: nil).layout(width: 70, color: CGColor(gray: 1, alpha: 1))
        precondition(wrapped.rows.count > 1)
        let later = wrapped.rows[1]
        let faded = CGContext(data: nil, width: 100, height: 300, bitsPerComponent: 8, bytesPerRow: 400,
                             space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        faded.translateBy(x: 0, y: 300)
        faded.scaleBy(x: 1, y: -1)
        wrapped.draw(in: faded, mark: nil, lit: 0.5)
        let fadedBytes = faded.data!.assumingMemoryBound(to: UInt8.self)
        var laterInk = 0
        for y in Int(ceil(later.top))..<Int(floor(later.top + later.height)) {
            for x in 0..<100 {
                let alpha = fadedBytes[y * 400 + x * 4 + 3]
                precondition(alpha <= 103, "a not-yet-sung wrapped row must remain dim")
                if alpha > 20 { laterInk += 1 }
            }
        }
        precondition(laterInk > 0)
        var animated = BernRubyText(text: wrappedText, ruby: #"[[0,2,"かんじ"]]"#, size: 32,
            style: "body", weight: "regular", bold: true, color: .white, lineHeight: nil, lines: nil,
            mark: nil, lit: { -1 }, path: { "" }, clock: { 0 }, since: { 0 }) { Text(wrappedText) }
        animated.animatableData = AnimatablePair(28, NSFont.Weight.medium.rawValue)
        precondition(animated.size == 28)
        precondition(animated.animatableData.second == NSFont.Weight.medium.rawValue)
        let systemFont = NSFont.systemFont(ofSize: 28, weight: .medium) as CTFont
        precondition(CFEqual(CTFontCopyFontDescriptor(animated.testNativeFont),
                             CTFontCopyFontDescriptor(systemFont)),
                     "ruby must retain the actual system font descriptor and fallback cascade")
        for scheme in [ColorScheme.light, .dark] {
            let subject = BernRubyText(text: "東京", ruby: #"[[0,2,"とうきょう"]]"#, size: 32,
                style: "body", weight: "regular", bold: false, color: .primary,
                lineHeight: nil, lines: nil, mark: nil,
                lit: { -1 }, path: { "" }, clock: { 0 }, since: { 0 }) { Text("東京") }
                .environment(\.colorScheme, scheme)
                .frame(width: 200, height: 90, alignment: .topLeading)
            let renderer = ImageRenderer(content: subject)
            renderer.scale = 2
            let image = renderer.cgImage!
            let pixels = CGContext(data: nil, width: image.width, height: image.height,
                bitsPerComponent: 8, bytesPerRow: image.width * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            pixels.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            let data = pixels.data!.assumingMemoryBound(to: UInt8.self)
            var ink = 0
            for offset in stride(from: 0, to: image.width * image.height * 4, by: 4) where data[offset + 3] > 200 {
                ink += 1
                for channel in 0..<3 {
                    let expected = scheme == .dark ? Int(data[offset + 3]) : 0
                    precondition(abs(Int(data[offset + channel]) - expected) <= 3,
                                 "macOS primary must resolve in the local SwiftUI environment")
                }
            }
            precondition(ink > 100, "render actual production macOS ruby ink")
        }
        let route = BernLitRoute("0 0 2 4")
        precondition(route.place(at: 1) == 2)
        print("native ruby passed")
    }
}
"##;
    let source = root.join("Ruby.swift");
    let binary = root.join("ruby-test");
    std::fs::write(
        &source,
        format!("import AppKit\nimport SwiftUI\nimport CoreText\n{ruby}\n{route}\n{tests}"),
    )
    .unwrap();
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
    let ran = Command::new(&binary).output().unwrap();
    assert!(
        ran.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
}

/// Build a hosted UI regression from the complete production Ruby renderer.
/// BERN_IOS_TEST_DEVICE opts into execution; launch success alone is not a pass.
#[cfg(target_os = "macos")]
#[test]
fn build_ruby_semantic_color_probe_for_ios_16() {
    use std::process::Command;
    let style = include_str!("../templates/BernStyle.swift");
    let ruby = &style
        [style.find("// MARK: - Ruby text").unwrap()..style.find("// MARK: - Text marks").unwrap()];
    let route = &style[style.find("struct BernLitRoute:").unwrap()
        ..style.find("/// A `Text` lit along").unwrap()];
    let root = test_dir("ruby-semantic-color-ios16");
    let bundle = root.join("ColorProbe.app");
    std::fs::create_dir_all(&bundle).unwrap();
    let source = root.join("ColorProbe.swift");
    let tests = r##"
@MainActor private final class RubyProbeState: ObservableObject {
    @Published var scheme: ColorScheme = .light
    @Published var contrast: ColorSchemeContrast = .standard
    @Published var lit: Double = -1
    @Published var alternate = false
}

private struct RubyProbeRegion: UIViewRepresentable {
    let name: String
    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.isUserInteractionEnabled = false
        view.accessibilityIdentifier = name
        return view
    }
    func updateUIView(_ view: UIView, context: Context) {}
}

private struct RubyProbeContent: View {
    @ObservedObject var state: RubyProbeState
    static let background = Color(.sRGB, white: 0.4, opacity: 1)
    private func row(_ name: String, color: Color, mark: Color? = nil,
                     text: String = "東京", reading: String = "とうきょう",
                     width: CGFloat = 300, lines: Int? = nil, height: CGFloat = 78) -> some View {
        HStack(spacing: 20) {
            BernRubyText(text: text, ruby: "[[0,2,\"\(reading)\"]]", size: 32,
                style: "body", weight: "regular", bold: false, color: color,
                lineHeight: nil, lines: lines, mark: mark,
                lit: { state.lit }, path: { "" }, clock: { 0 }, since: { 0 }) {
                    Text(verbatim: text).font(.system(size: 32)).foregroundColor(color)
                }
                .frame(width: width)
                .background(RubyProbeRegion(name: name))
                .frame(width: 300, height: height, alignment: .topLeading)
            Text(verbatim: text).font(.system(size: 32)).foregroundColor(color)
                .lineLimit(1).frame(width: 180, height: height, alignment: .topLeading)
                .background(RubyProbeRegion(name: name + "-text"))
            color.frame(width: 24, height: 24)
                .background(RubyProbeRegion(name: name + "-color"))
            (mark ?? .clear).frame(width: 24, height: 24)
                .background(RubyProbeRegion(name: name + "-mark"))
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            row("primary", color: .primary)
            row("secondary", color: .secondary)
            row("opacity", color: .primary.opacity(0.5))
            row("explicit", color: (state.alternate ? Color.blue : Color.red).opacity(0.65))
            row("mark", color: .primary, mark: .secondary.opacity(0.7))
            row("emoji", color: .primary, text: "東京😀", reading: "とう😀きょう")
            row("wrapped", color: .primary, text: "東京へ行く。東京へ行く。東京へ行く。",
                width: 110, lines: 2, height: 150)
        }
        .padding(24)
        .background(Self.background)
        .environment(\.colorScheme, state.scheme)
    }
}

private struct RubyProbePixels {
    let width: Int
    let height: Int
    let scale: CGFloat
    let bytes: [UInt8]
    @MainActor init(_ view: UIView) {
        let format = UIGraphicsImageRendererFormat()
        // Inspect native pixels: downsampling text can overshoot its ink opacity.
        scale = view.window!.screen.scale
        format.scale = scale
        format.opaque = true
        let image = UIGraphicsImageRenderer(size: view.bounds.size, format: format).image { _ in
            precondition(view.drawHierarchy(in: view.bounds, afterScreenUpdates: true), "capture the actual hosted UI")
        }.cgImage!
        width = image.width
        height = image.height
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        bytes = Array(UnsafeBufferPointer(start: context.data!.assumingMemoryBound(to: UInt8.self), count: width * height * 4))
    }
    func pixel(_ x: Int, _ y: Int) -> [Double] {
        precondition(x >= 0 && x < width && y >= 0 && y < height)
        let i = (y * width + x) * 4
        return (0..<3).map { Double(bytes[i + $0]) / 255 }
    }
    func sample(_ rect: CGRect) -> [Double] { pixel(Int(rect.midX * scale), Int(rect.midY * scale)) }
    func strongest(_ rect: CGRect, background: [Double]) -> [Double] {
        let rect = rect.applying(CGAffineTransform(scaleX: scale, y: scale))
        var result = background, strength = 0.0
        for y in max(0, Int(ceil(rect.minY)))..<min(height, Int(floor(rect.maxY))) {
            for x in max(0, Int(ceil(rect.minX)))..<min(width, Int(floor(rect.maxX))) {
                let value = pixel(x, y)
                let distance = zip(value, background).reduce(0.0) { $0 + abs($1.0 - $1.1) }
                if distance > strength { strength = distance; result = value }
            }
        }
        precondition(strength > 0.05, "missing ink in \(rect)")
        return result
    }
    func coloredCount(_ rect: CGRect) -> Int {
        let rect = rect.applying(CGAffineTransform(scaleX: scale, y: scale))
        var result = 0
        for y in max(0, Int(ceil(rect.minY)))..<min(height, Int(floor(rect.maxY))) {
            for x in max(0, Int(ceil(rect.minX)))..<min(width, Int(floor(rect.maxX))) {
                let value = pixel(x, y)
                if value.max()! - value.min()! > 0.2 { result += 1 }
            }
        }
        return result
    }
}

@main private final class RubyColorProbe: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    private let state = RubyProbeState()
    private var host: UIHostingController<RubyProbeContent>!
    private var step = 0
    private var primaryIdentity: ObjectIdentifier?
    private var full: RubyProbePixels?
    private var matrix: [(UIUserInterfaceStyle, ColorScheme, ColorSchemeContrast)] = []

    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        setbuf(stdout, nil)
        for native in [UIUserInterfaceStyle.light, .dark] {
            for scheme in [ColorScheme.light, .dark] {
                for contrast in [ColorSchemeContrast.standard, .increased] { matrix.append((native, scheme, contrast)) }
            }
        }
        let window = UIWindow(frame: UIScreen.main.bounds)
        host = UIHostingController(rootView: RubyProbeContent(state: state))
        host.view.backgroundColor = UIColor(white: 0.4, alpha: 1)
        let container = UIViewController()
        container.addChild(host)
        container.view.addSubview(host.view)
        host.view.frame = window.bounds
        host.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        host.didMove(toParent: container)
        window.rootViewController = container
        self.window = window
        window.makeKeyAndVisible()
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) { preconditionFailure("ruby UI probe timed out") }
        advance()
        return true
    }

    private func descendants(_ view: UIView) -> [UIView] {
        [view] + view.subviews.flatMap { descendants($0) }
    }
    private func region(_ name: String) -> CGRect {
        let views = descendants(host.view).filter { $0.accessibilityIdentifier == name }
        precondition(views.count == 1, "missing region \(name)")
        return views[0].convert(views[0].bounds, to: host.view)
    }
    private func ink(_ name: String, marks: Bool = false) -> BernRubyInkView {
        let area = region(name)
        let views = descendants(host.view).compactMap { $0 as? BernRubyInkView }.filter {
            $0.marks == marks && $0.convert($0.bounds, to: host.view).intersects(area.insetBy(dx: 1, dy: 1))
        }
        precondition(views.count == 1, "missing production ink view \(name), marks=\(marks): \(views.count)")
        return views[0]
    }
    private func rgba(_ color: UIColor) -> [CGFloat] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        precondition(color.getRed(&r, green: &g, blue: &b, alpha: &a))
        return [r, g, b, a]
    }
    private func close(_ a: [Double], _ b: [Double], _ message: String, tolerance: Double = 0.02) {
        precondition(zip(a, b).allSatisfy { abs($0 - $1) <= tolerance }, "\(message): \(a), expected \(b)")
    }
    private func advance() {
        if step < matrix.count {
            let entry = matrix[step]
            window!.overrideUserInterfaceStyle = entry.0
            state.scheme = entry.1
            state.contrast = entry.2
        } else {
            state.scheme = .dark
            state.contrast = .standard
            state.lit = [-1.0, 0, 1.5, 100][step - matrix.count]
            state.alternate = true
        }
        window!.rootViewController!.setOverrideTraitCollection(
            UITraitCollection(accessibilityContrast: state.contrast == .increased ? .high : .normal), forChild: host)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { self.check() }
    }
    private func check() {
        host.view.layoutIfNeeded()
        let primary = ink("primary")
        if let identity = primaryIdentity {
            precondition(ObjectIdentifier(primary) == identity, "test runtime changes on the same native view")
        } else { primaryIdentity = ObjectIdentifier(primary) }
        let native = rgba(primary.tintColor.resolvedColor(with: primary.traitCollection))
        let expected: CGFloat = state.scheme == .dark ? 1 : 0
        precondition(native.prefix(3).allSatisfy { abs($0 - expected) < 0.03 }, "primary \(state.scheme): \(native)")
        precondition(native[3] > 0.99)
        let secondary = rgba(ink("secondary").tintColor)
        let brightness = secondary.prefix(3).reduce(0, +) / 3
        precondition(state.scheme == .dark ? brightness > 0.7 : brightness < 0.4, "secondary \(state.scheme): \(secondary)")
        precondition(secondary[3] > 0 && secondary[3] < native[3], "secondary keeps its opacity")
        precondition(abs(rgba(ink("opacity").tintColor)[3] - 0.5) < 0.01)
        precondition(abs(rgba(ink("explicit").tintColor)[3] - 0.65) < 0.01)
        let pixels = RubyProbePixels(host.view)
        let background = pixels.pixel(5, 5)
        if state.lit < 0 {
            for name in ["primary", "secondary", "opacity", "explicit", "mark"] {
                let view = ink(name)
                let drawing = view.document!.layout(width: view.bounds.width)
                let frame = region(name)
                precondition(abs(frame.height - drawing.size.height) < 1, "annotation height participates in layout")
                let row = drawing.rows[0]
                let boundary = row.baseline - CTFontGetAscent(view.document!.font)
                precondition(boundary > 2, "ruby has reserved height")
                let annotation = CGRect(x: frame.minX, y: frame.minY, width: frame.width, height: boundary)
                let base = CGRect(x: frame.minX, y: frame.minY + boundary, width: frame.width,
                                  height: row.baseline - boundary)
                let swatch = pixels.sample(region(name + "-color"))
                let control = pixels.strongest(region(name + "-text"), background: background)
                close(pixels.strongest(base, background: background), swatch, "\(name) base UI pixels \(state.scheme)")
                close(pixels.strongest(annotation, background: background), swatch, "\(name) ruby UI pixels \(state.scheme)")
                close(control, swatch, "SwiftUI Text control \(name)")
            }
            let markView = ink("mark", marks: true)
            let row = markView.document!.layout(width: markView.bounds.width).rows[0]
            let origin = region("mark").origin
            let band = CGRect(x: origin.x + row.width / 2, y: origin.y + row.baseline + 2, width: 1, height: 1)
            close(pixels.sample(band), pixels.sample(region("mark-mark")), "independent mark color and alpha")
            let emojiView = ink("emoji")
            let emojiDrawing = emojiView.document!.layout(width: emojiView.bounds.width)
            let emojiFrame = region("emoji")
            let boundary = emojiDrawing.rows[0].baseline - CTFontGetAscent(emojiView.document!.font)
            precondition(pixels.coloredCount(CGRect(x: emojiFrame.minX, y: emojiFrame.minY, width: emojiFrame.width, height: boundary)) > 8,
                         "ruby color emoji retain their palette")
            precondition(pixels.coloredCount(CGRect(x: emojiFrame.minX, y: emojiFrame.minY + boundary,
                         width: emojiFrame.width, height: emojiFrame.height - boundary)) > 40, "base color emoji retain their palette")
            let wrapped = ink("wrapped")
            let drawing = wrapped.document!.layout(width: wrapped.bounds.width)
            precondition(drawing.rows.count == 2, "wrapping and line limit")
            precondition((CTLineGetGlyphRuns(drawing.rows.last!.line) as! [CTRun]).contains {
                (CTRunGetAttributes($0) as NSDictionary)["BernRubyTruncation"] != nil
            }, "truncation keeps the ellipsis")
            if step == matrix.count { full = pixels }
        } else {
            let full = full!
            for name in ["primary", "opacity", "wrapped"] {
                let view = ink(name), frame = region(name)
                let drawing = view.document!.layout(width: view.bounds.width)
                if state.lit == 0 {
                    let bright = full.strongest(frame, background: background)
                    let expected = zip(bright, background).map { ($0 - $1) * 0.4 + $1 }
                    close(pixels.strongest(frame, background: background), expected, "\(name) unsung alpha", tolerance: 0.025)
                } else if state.lit == 1.5 && name == "wrapped" {
                    let row = drawing.rows[1]
                    let rect = CGRect(x: frame.minX, y: frame.minY + row.top, width: frame.width, height: row.height)
                    let bright = full.strongest(rect, background: background)
                    close(pixels.strongest(rect, background: background), zip(bright, background).map { ($0 - $1) * 0.4 + $1 },
                          "later wrapped rows stay dim", tolerance: 0.025)
                    let first = drawing.rows[0]
                    let firstRect = CGRect(x: frame.minX, y: frame.minY, width: frame.width, height: first.height)
                    close(pixels.strongest(firstRect, background: background), full.strongest(firstRect, background: background), "sung glyphs brighten")
                    let a = CTLineGetOffsetForStringIndex(first.line, drawing.offsets[1], nil)
                    let b = CTLineGetOffsetForStringIndex(first.line, drawing.offsets[2], nil)
                    let middle = (a + b) / 2
                    for (left, right, alpha) in [(a, middle, 1.0), (middle, b, 0.4)] {
                        let part = CGRect(x: frame.minX + left + 2,
                            y: frame.minY + first.baseline - CTFontGetAscent(view.document!.font),
                            width: right - left - 4, height: CTFontGetAscent(view.document!.font))
                        let bright = full.strongest(part, background: background)
                        close(pixels.strongest(part, background: background),
                              zip(bright, background).map { ($0 - $1) * alpha + $1 },
                              "fractional scalar highlight", tolerance: 0.025)
                    }
                } else if state.lit == 100 && name != "wrapped" {
                    close(pixels.strongest(frame, background: background), full.strongest(frame, background: background),
                          "highlight must retain explicit foreground opacity", tolerance: 0.025)
                }
            }
        }
        print("PASS ruby UI", step, "native", window!.overrideUserInterfaceStyle.rawValue,
              "local", state.scheme, "contrast", state.contrast, "lit", state.lit, "primary", native)
        step += 1
        if step == matrix.count + 4 {
            print("PASS all ruby UI cases")
            exit(EXIT_SUCCESS)
        }
        advance()
    }
}
"##;
    std::fs::write(
        &source,
        format!("import UIKit\nimport SwiftUI\nimport CoreText\n{ruby}\n{route}\n{tests}"),
    )
    .unwrap();
    std::fs::write(
        bundle.join("Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>ColorProbe</string>
<key>CFBundleIdentifier</key><string>dev.bern.semantic-color-probe</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>MinimumOSVersion</key><string>16.0</string>
<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
<key>UILaunchScreen</key><dict/>
</dict></plist>"#,
    )
    .unwrap();
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    let result = Command::new("xcrun")
        .args([
            "--sdk",
            "iphonesimulator",
            "swiftc",
            "-swift-version",
            "6",
            "-parse-as-library",
            "-target",
        ])
        .arg(format!("{arch}-apple-ios16.0-simulator"))
        .arg(&source)
        .arg("-o")
        .arg(bundle.join("ColorProbe"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    if let Some(device) = std::env::var_os("BERN_IOS_TEST_DEVICE") {
        let result = Command::new("xcrun")
            .args(["simctl", "install"])
            .arg(&device)
            .arg(&bundle)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = Command::new("xcrun")
            .args([
                "simctl",
                "launch",
                "--console",
                "--terminate-running-process",
            ])
            .arg(&device)
            .arg("dev.bern.semantic-color-probe")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(
            result.status.success() && stdout.contains("PASS all ruby UI cases"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
    } else {
        eprintln!(
            "built UIKit probe at {}; UI assertions NOT RUN (no BERN_IOS_TEST_DEVICE)",
            bundle.display()
        );
    }
}
