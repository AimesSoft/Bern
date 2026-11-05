//! The iOS 16 front end of the `swiftui + ios` target.
//!
//! As WinUI 3 and the system XAML are two front ends over one source on
//! Windows, the SwiftUI an iOS app is written in serves two front ends: the
//! one every other Apple target uses, from iOS 17, and this one. What SwiftUI
//! gained in iOS 17 is rewritten here, once the Swift is generated, to what
//! iOS 16 has, so nothing changes for the front end of iOS 17 and later.
//! `BernCompat16.swift` holds the stand-ins the rewritten source calls.
//!
//! Observation is the largest part: from iOS 17 SwiftUI notices by itself
//! what a view reads. Here a class observed through a view's environment or
//! state becomes an `ObservableObject`, and the classes any view may read
//! without saying so, such as the language and the theme, change rarely
//! enough that every view follows one signal.

/// Earliest system of this front end.
pub const MINIMUM: &str = "16.0";

/// The file of stand-ins, compiled with the rewritten sources.
pub const COMPAT: &str = include_str!("../templates/BernCompat16.swift");

/// The executable's choice of front end, in an app of both.
pub const LAUNCH: &str = include_str!("../templates/BernLaunch16.swift");

/// The way into the front end of iOS 17 and later as a framework.
pub const ENTRY: &str = include_str!("../templates/BernEntry17.swift");

/// Classes any view may read without declaring it. A change to one draws
/// every view again, so they are classes that seldom change.
const SHARED: &[&str] = &[
    "BernLanguage",
    "BernKeyboard",
    "BernHost",
    "BernHostPlatform",
    "BernWindowPlatform",
    "BernThemeState",
    "BernRemoteShortcuts",
];

/// Classes a view keeps in its state, and follows from there.
const KEPT: &[&str] = &[
    "BernScrollItems",
    "BernCover",
    "BernPointer",
    "BernPaletteState",
];

/// Singletons followed by the views that read what changes in them: the
/// class, the property that follows it, and those reads.
const FOLLOWED: &[(&str, &str, &[&str])] = &[
    (
        "BernMedia",
        "bernMediaFollowed",
        &[
            "BernMedia.shared.players",
            "BernMedia.shared.kernel",
            "= BernMedia.shared\n",
        ],
    ),
    (
        "BernPictures",
        "bernPicturesFollowed",
        &["BernPictures.shared.average("],
    ),
];

/// Calls of iOS 17 and their iOS 16 form, as they stand in the source.
const CALLS: &[(&str, &str)] = &[
    (".onChange(of:", ".bernOnChange(of:"),
    (".onKeyPress(", ".bernOnKeyPress("),
    (".focusable()", ""),
    (".focusable(recording)", ""),
    (".focusEffectDisabled()", ""),
    (".scrollClipDisabled()", ""),
    (".scrollTargetLayout()", ""),
    (".allowedDynamicRange(.high)", ""),
    ("control.updateTraitsIfNeeded()", ""),
    (
        ".buttonStyle(.bordered).buttonBorderShape(.circle)",
        ".buttonStyle(BernCircleButton16())",
    ),
    (".fill.tertiary", "Color(uiColor: .tertiarySystemFill)"),
    (".safeAreaPadding(", ".bernSafeAreaPadding("),
    (
        ".presentationCompactAdaptation(.popover)",
        ".bernCompactPopover()",
    ),
    ("presentationBackground {", "bernPresentationBackground {"),
    (".scrollTargetBehavior(", ".bernScrollTargetBehavior("),
    ("contentMargins(", "bernContentMargins("),
    ("self.listSectionSpacing(", "self.bernListSectionSpacing("),
    (
        "content.toolbar(removing: covered ? .sidebarToggle : nil)",
        "content",
    ),
    (
        ".animation(.easeInOut(duration: 0.18)) { $0.opacity(at == nil ? 0 : 1) }",
        ".opacity(at == nil ? 0 : 1).animation(.easeInOut(duration: 0.18), value: at == nil)",
    ),
    (
        "withAnimation(.easeOut(duration: 0.08)) { scale = 1.06 } completion: {",
        "bernWithAnimation(.easeOut(duration: 0.08)) { scale = 1.06 } completion: {",
    ),
    (
        "content.scrollTransition(.interactive.threshold(.centered))",
        "content.bernScrollTransition",
    ),
    ("content.visualEffect {", "content.bernVisualEffect {"),
    ("MagnifyGesture()", "MagnificationGesture()"),
    ("gesture.magnification", "gesture"),
    (
        ".scrollPosition(id: $target, anchor: anchor)",
        ".bernScrollPosition16(id: $target, anchor: anchor)",
    ),
    (
        "withAnimation(animation) { target = wanted }",
        "bernScrollAnimation16 = animation; withAnimation(animation) { target = wanted }",
    ),
    (
        "@Environment(\\.dismissWindow) private var dismissWindow",
        "private let dismissWindow = DismissWindowAction()",
    ),
    (
        "platform.nativeWindows = multiple && UIDevice.current.userInterfaceIdiom == .pad",
        "_ = multiple; platform.nativeWindows = false",
    ),
    (
        "@State private var query: BernQuery<Data>",
        "@StateObject private var query: BernQuery<Data>",
    ),
    (
        "_query = State(initialValue: query)",
        "_query = StateObject(wrappedValue: query)",
    ),
    (".inspector(isPresented:", ".bernInspector(isPresented:"),
    // A view that glows reads where the pointer is.
    (
        "@Environment(\\.bernPointer) private var pointer\n",
        "@Environment(\\.bernPointer) private var pointer\n    @Environment(\\.bernPointerLocation16) private var bernPointerLocation\n",
    ),
    (
        ".environment(\\.bernPointer, pointer)",
        ".environment(\\.bernPointer, pointer).environment(\\.bernPointerLocation16, pointer.location)",
    ),
];

/// Scene and window settings iOS 16 has no form of, dropped with their line.
const DROPPED: &[&str] = &[
    ".defaultSize(",
    ".windowResizability(",
    ".inspectorColumnWidth(",
];

/// The generated application without its entry point, which an app of both
/// front ends has elsewhere.
pub fn without_entry(source: &str) -> String {
    source.replace(
        "@main\nstruct BernApplication: App",
        "struct BernApplication: App",
    )
}

/// `Info.plist` of a framework of the bundle.
pub fn framework_plist(name: &str, identifier: &str, minimum: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{name}</string>
    <key>CFBundleIdentifier</key>
    <string>{identifier}</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>iPhoneSimulator</string>
    </array>
    <key>MinimumOSVersion</key>
    <string>{minimum}</string>
</dict>
</plist>
"#
    )
}

/// The source of the iOS 16 front end from the source of the other.
pub fn rewrite(source: &str) -> String {
    let mut text = source.to_string();
    for (call, replacement) in CALLS {
        text = text.replace(call, replacement);
    }
    let text = named_spaces(&text);
    let lines: Vec<String> = text
        .lines()
        .filter(|line| {
            !DROPPED
                .iter()
                .any(|drop| line.trim_start().starts_with(drop))
        })
        .map(environment)
        .collect();
    let mut out = views(&classes(lines)).join("\n");
    out.push('\n');
    out
}

/// `coordinateSpace(.named(x))` as `coordinateSpace(name: x)`.
fn named_spaces(text: &str) -> String {
    const OPEN: &str = "coordinateSpace(.named(";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(OPEN) {
        let after = &rest[at + OPEN.len()..];
        let Some(end) = after.find("))") else { break };
        out.push_str(&rest[..at]);
        out.push_str("coordinateSpace(name: ");
        out.push_str(&after[..end]);
        out.push(')');
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Objects of a view's environment and state, as iOS 16 follows them.
fn environment(line: &str) -> String {
    let mut line = line.to_string();
    if let Some(at) = line.find("@Environment(")
        && let Some(end) = line[at..].find(".self)")
    {
        let class = &line[at + "@Environment(".len()..at + end];
        line = if line.trim_end().ends_with('?') {
            // No object of an environment is optional here; only the
            // window is read this way, and only as it stands.
            format!(
                "{}@Environment(\\.bernWindowContext16) private var window",
                &line[..at]
            )
        } else {
            let (head, tail) = (&line[..at], &line[at + end + ".self)".len()..]);
            format!("{head}@BernObject({class}.self){tail}")
        };
    }
    for class in KEPT {
        let kept = format!(" = {class}(");
        if line.trim_start().starts_with("@State ") && line.contains(&kept) {
            line = line.replacen("@State ", "@StateObject ", 1);
        }
    }
    line
}

/// Each `@Observable` class as one iOS 16 can follow.
fn classes(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut pending = false;
    // Inside a class: the attribute for its observed properties and how
    // deep its braces go.
    let mut class: Option<(&str, i32)> = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.contains("@Observable") {
            pending = true;
            let rest = line.replace("@Observable", "");
            let rest = rest.trim_end();
            if rest.trim().is_empty() {
                continue;
            }
            if !rest.contains("final class") {
                out.push(rest.to_string());
                continue;
            }
            let (header, attribute) = observable_header(rest);
            pending = false;
            class = Some((attribute, braces(&header)));
            out.push(header);
            continue;
        }
        if pending && trimmed.contains("final class") {
            pending = false;
            let (header, attribute) = observable_header(&line);
            class = Some((attribute, braces(&header)));
            out.push(header);
            continue;
        }
        if let Some((attribute, depth)) = class {
            let ignored = line.contains("@ObservationIgnored");
            let line = line.replace("@ObservationIgnored ", "");
            let next = depth + braces(&line);
            class = (next > 0).then_some((attribute, next));
            if depth == 1 && !ignored && stored(line.trim()) {
                let indent = &line[..line.len() - line.trim_start().len()];
                out.push(format!("{indent}{attribute} {}", line.trim_start()));
            } else {
                out.push(line);
            }
            continue;
        }
        out.push(line.replace("@ObservationIgnored ", ""));
    }
    out
}

/// A class's header as iOS 16 has it, and the attribute of its properties.
fn observable_header(line: &str) -> (String, &'static str) {
    let at = line.find("final class ").unwrap_or(0) + "final class ".len();
    let name_end = line[at..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .map_or(line.len(), |end| at + end);
    if SHARED.contains(&&line[at..name_end]) {
        return (line.to_string(), "@BernSharedValue");
    }
    // The generics of the name come before its conformances.
    let mut after = name_end;
    if line[after..].starts_with('<') {
        let mut depth = 0;
        for (index, c) in line[after..].char_indices() {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                after += index + 1;
                break;
            }
        }
    }
    let header = if line[after..].starts_with(':') {
        format!(
            "{}: ObservableObject,{}",
            &line[..after],
            &line[after + 1..]
        )
    } else {
        format!("{}: ObservableObject{}", &line[..after], &line[after..])
    };
    (header, "@Published")
}

/// Whether a line declares a stored `var`.
fn stored(line: &str) -> bool {
    let mut line = line;
    for access in [
        "private(set) ",
        "fileprivate(set) ",
        "private ",
        "fileprivate ",
        "public ",
    ] {
        line = line.strip_prefix(access).unwrap_or(line);
    }
    let Some(line) = line.strip_prefix("var ") else {
        return false;
    };
    match (line.find(" = "), line.find('{')) {
        (Some(assign), Some(brace)) => assign < brace,
        (None, Some(_)) => false,
        _ => true,
    }
}

/// The braces a line opens less those it closes, outside strings and
/// comments.
fn braces(line: &str) -> i32 {
    let mut depth = 0;
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if quoted => {
                chars.next();
            }
            '"' => quoted = !quoted,
            '/' if !quoted && chars.peek() == Some(&'/') => break,
            '{' if !quoted => depth += 1,
            '}' if !quoted => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// Whether a line opens a `View` or `ViewModifier`.
fn view_header(line: &str) -> bool {
    let mut line = line.trim();
    for access in ["private ", "fileprivate ", "public "] {
        line = line.strip_prefix(access).unwrap_or(line);
    }
    let Some(line) = line.strip_prefix("struct ") else {
        return false;
    };
    if !line.ends_with('{') {
        return false;
    }
    let mut depth = 0;
    let mut colon = None;
    for (index, c) in line.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ':' if depth == 0 => {
                colon = Some(index);
                break;
            }
            _ => {}
        }
    }
    let Some(colon) = colon else { return false };
    let list = line[colon + 1..].trim_end_matches('{');
    let list = list.split(" where ").next().unwrap_or(list);
    list.split(',').any(|name| {
        matches!(
            name.trim(),
            "View" | "ViewModifier" | "SwiftUI.View" | "SwiftUI.ViewModifier"
        )
    })
}

/// Has every view follow the shared signal, and what it names besides.
fn views(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        out.push(line.clone());
        if !view_header(line) {
            continue;
        }
        let indent = &line[..line.len() - line.trim_start().len()];
        out.push(format!(
            "{indent}    @ObservedObject var bernShared = BernShared.shared"
        ));
        let mut depth = 0;
        let mut body = String::new();
        for line in &lines[index..] {
            body.push_str(line);
            body.push('\n');
            depth += braces(line);
            if depth == 0 {
                break;
            }
        }
        for (class, property, reads) in FOLLOWED {
            if reads.iter().any(|read| body.contains(read)) {
                out.push(format!(
                    "{indent}    @ObservedObject var {property} = {class}.shared"
                ));
            }
        }
    }
    // A view handed its window follows it.
    let mut view_depth = 0;
    let mut result = Vec::with_capacity(out.len());
    for line in out {
        if view_depth == 0 && view_header(&line) {
            view_depth = braces(&line);
            result.push(line);
            continue;
        }
        if view_depth > 0 {
            let before = view_depth;
            view_depth += braces(&line);
            if before == 1 && line.trim() == "let context: BernWindowContext" {
                result.push(line.replace("let context:", "@ObservedObject var context:"));
                continue;
            }
        }
        result.push(line);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_observed_class_is_an_observable_object() {
        let source = "@MainActor\n@Observable\nfinal class Draft: Session {\n    var text = \"\"\n    @ObservationIgnored var hidden = 0\n    var size: Int { 1 }\n    private(set) var count: Int\n}\n";
        let out = rewrite(source);
        assert!(!out.contains("@Observable"), "{out}");
        assert!(
            out.contains("final class Draft: ObservableObject, Session {"),
            "{out}"
        );
        assert!(out.contains("    @Published var text = \"\""), "{out}");
        assert!(
            out.contains("    @Published private(set) var count: Int"),
            "{out}"
        );
        assert!(out.contains("\n    var hidden = 0"), "{out}");
        assert!(out.contains("\n    var size: Int { 1 }"), "{out}");
    }

    #[test]
    fn a_generic_class_keeps_its_parameters() {
        let out =
            rewrite("@Observable\nfinal class BernQuery<Data: Codable> {\n    var phase = 0\n}\n");
        assert!(
            out.contains("final class BernQuery<Data: Codable>: ObservableObject {"),
            "{out}"
        );
    }

    #[test]
    fn a_class_any_view_reads_follows_the_shared_signal() {
        let out = rewrite("@Observable\nfinal class BernThemeState {\n    var accent: Int?\n}\n");
        assert!(out.contains("final class BernThemeState {"), "{out}");
        assert!(
            out.contains("    @BernSharedValue var accent: Int?"),
            "{out}"
        );
    }

    #[test]
    fn views_follow_the_shared_signal_and_what_they_read() {
        let source = "private struct Row<Content: View>: View {\n    let context: BernWindowContext\n    var body: some View { Text(BernPictures.shared.average(url)) }\n}\nstruct Plain<Content: View>: Equatable {\n}\n";
        let out = rewrite(source);
        assert!(
            out.contains("@ObservedObject var bernShared = BernShared.shared"),
            "{out}"
        );
        assert!(
            out.contains("@ObservedObject var bernPicturesFollowed"),
            "{out}"
        );
        assert!(
            out.contains("@ObservedObject var context: BernWindowContext"),
            "{out}"
        );
        assert_eq!(out.matches("bernShared").count(), 1, "{out}");
    }

    #[test]
    fn objects_of_the_environment_and_state_are_followed() {
        let out = rewrite(
            "    @Environment(BernEngine.self) private var engine\n    @Environment(BernWindowContext.self) private var window: BernWindowContext?\n    @State private var cover = BernCover()\n",
        );
        assert!(
            out.contains("@BernObject(BernEngine.self) private var engine"),
            "{out}"
        );
        assert!(
            out.contains("@Environment(\\.bernWindowContext16) private var window"),
            "{out}"
        );
        assert!(
            out.contains("@StateObject private var cover = BernCover()"),
            "{out}"
        );
    }

    #[test]
    fn the_entry_point_moves_out_of_the_application() {
        let out = without_entry("@main\nstruct BernApplication: App {\n}\n");
        assert_eq!(out, "struct BernApplication: App {\n}\n");
    }

    /// A rewrite that no longer finds its call does nothing and says
    /// nothing; the type check only notices what iOS 16 lacks.
    #[test]
    fn every_rewrite_still_finds_its_call() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut known = String::new();
        for dir in ["templates", "src/swift"] {
            for entry in std::fs::read_dir(root.join(dir)).unwrap() {
                let path = entry.unwrap().path();
                if path.is_file()
                    && path
                        .file_name()
                        .is_some_and(|name| name != "BernCompat16.swift")
                {
                    known.push_str(&std::fs::read_to_string(path).unwrap());
                }
            }
        }
        for (call, _) in CALLS {
            // Generator sources escape what they write.
            let escaped = call.replace('{', "{{");
            assert!(
                known.contains(call) || known.contains(&escaped),
                "`{call}` is in neither the templates nor the generator"
            );
        }
        for call in DROPPED {
            assert!(known.contains(call), "`{call}` is gone");
        }
    }
}
