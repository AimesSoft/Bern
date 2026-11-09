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
