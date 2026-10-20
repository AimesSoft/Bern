//! Lay out `BernRow` in a hosting view, without an application window.
#[test]
#[cfg(target_os = "macos")]
fn a_spacer_takes_no_room_from_the_texts_of_a_narrow_row() {
    use std::{path::Path, process::Command};
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("struct BernRowSpacer:").unwrap();
    let end = style.find("struct BernFlow:").unwrap();
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/row-layout");
    std::fs::create_dir_all(&output).unwrap();
    let source = format!(
        "import AppKit\nimport SwiftUI\n{}\n{}",
        &style[start..end],
        r#"
nonisolated(unsafe) var ideal: [String: CGFloat] = [:]
nonisolated(unsafe) var placed: [String: CGFloat] = [:]

/// Notes the width a text wants and the width it is given.
struct Measured: Layout {
    let name: String
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        ideal[name] = subviews[0].sizeThatFits(.unspecified).width
        return subviews[0].sizeThatFits(proposal)
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        placed[name] = bounds.width
        subviews[0].place(at: bounds.origin, proposal: ProposedViewSize(bounds.size))
    }
}

func text(_ name: String, size: CGFloat, weight: Font.Weight) -> some View {
    Measured(name: name) { Text(verbatim: name).font(.system(size: size, weight: weight)).lineLimit(1) }
}

/// A spacer as tvOS measures `Spacer()`: at least 60 points.
func spacer() -> some View {
    Spacer(minLength: 60).layoutValue(key: BernRowSpacer.self, value: true)
}

/// A day's heading in its 86-point column: a 16-point inset, then the day
/// and its count, each a row that ends in a spacer.
struct Heading: View {
    var body: some View {
        BernRow(alignment: .center, spacing: 0) {
            Color.clear.frame(width: 16, height: 39)
            VStack(alignment: .leading, spacing: 0) {
                BernRow(alignment: .center, spacing: 0) {
                    text("Mon", size: 22, weight: .bold)
                    spacer()
                }
                .frame(width: nil, height: 28, alignment: .leading)
                BernRow(alignment: .center, spacing: 3) {
                    text("Today", size: 13.5, weight: .medium)
                    text("· 3", size: 13.5, weight: .medium)
                    spacer()
                }
                .frame(width: nil, height: 18, alignment: .leading)
            }
        }
        .frame(width: 86, alignment: .topLeading)
    }
}

@main struct RowLayoutTest {
    @MainActor static func main() {
        let view = NSHostingView(rootView: Heading())
        view.frame = CGRect(x: 0, y: 0, width: 86, height: 80)
        view.layoutSubtreeIfNeeded()
        for name in ["Mon", "Today", "· 3"] {
            guard let wanted = ideal[name], let given = placed[name] else {
                fatalError("\(name) was not laid out")
            }
            precondition(wanted > 0 && wanted < 50, "\(name) wants \(wanted)")
            precondition(given >= wanted - 0.5, "\(name) wants \(wanted) but is given \(given)")
        }
    }
}
"#
    );
    let file = output.join("main.swift");
    let executable = output.join("row-layout");
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
