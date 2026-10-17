//! Width conservation is what keeps neighboring columns from crossing.
#[test]
#[cfg(target_os = "macos")]
fn accordion_preserves_bounds_and_gaps_during_springs_and_interruptions() {
    use std::{path::Path, process::Command};
    let style = include_str!("../templates/BernStyle.swift");
    let start = style.find("struct BernAccordionWeights:").unwrap();
    let end = style.find("private struct BernAccordionLayout:").unwrap();
    let output =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bern-test/accordion-layout");
    std::fs::create_dir_all(&output).unwrap();
    let source = format!(
        "import SwiftUI\n{}\n{}",
        &style[start..end],
        r#"
func selected(_ day: Int) -> BernAccordionWeights {
    BernAccordionWeights(values: (0..<7).map { $0 == day ? 1 : 0 })
}
func blend(_ from: BernAccordionWeights, _ to: BernAccordionWeights, _ progress: Double) -> BernAccordionWeights {
    var delta = to - from
    delta.scale(by: progress)
    return from + delta
}
func check(_ weights: BernAccordionWeights, total: Double) {
    let gap = 8.0
    let widths = BernAccordionGeometry.widths(total: total, collapsedWidth: 86, gap: gap, weights: weights.values)
    precondition(widths.count == 7 && widths.allSatisfy { $0.isFinite && $0 >= 0 })
    precondition(abs(widths.reduce(0, +) - max(0, total - 6 * gap)) < 0.00001)
    var right = 0.0
    for width in widths {
        let left = right + gap
        precondition(left >= right, "columns must never overlap")
        right = left + width
    }
}
@main struct AccordionTest {
    static func main() {
        precondition(selected(2) + .zero == selected(2))
        for from in 0..<7 {
            for to in 0..<7 {
                // Include spring overshoot and interrupt with a third target.
                for step in -10...110 {
                    let frame = blend(selected(from), selected(to), Double(step) / 100)
                    for total in [200.0, 640, 1088, 5000] { check(frame, total: total) }
                    check(blend(frame, selected((to + 3) % 7), 0.35), total: 1088)
                }
            }
        }
        let resting = BernAccordionGeometry.widths(total: 1088, collapsedWidth: 86, gap: 8, weights: selected(3).values)
        precondition(resting[3] == 524)
        precondition(resting.enumerated().allSatisfy { $0.offset == 3 || $0.element == 86 })
        precondition(BernAccordionGeometry.widths(total: 100, collapsedWidth: 86, gap: 8, weights: []).isEmpty)
    }
}
"#
    );
    let file = output.join("main.swift");
    let executable = output.join("accordion-layout");
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
