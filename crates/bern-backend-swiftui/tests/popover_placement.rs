//! Popover placement follows the platform's available screen space.
#[test]
fn ios_popovers_leave_the_arrow_edge_to_the_system() {
    let source = include_str!("../templates/BernStyle.swift");
    let start = source
        .find("@ViewBuilder private func presentation(_ content: Content)")
        .unwrap();
    let end = source[start..].find("private func release()").unwrap() + start;
    let presentation = &source[start..end];
    assert!(presentation.contains("#if os(tvOS)\n        content.sheet("));
    assert!(presentation.contains(
        "#elseif os(macOS)\n        content.popover(isPresented: $presented, arrowEdge: .bottom)"
    ));
    let ios = presentation.split("#else").last().unwrap();
    assert!(ios.contains("if #available(iOS 18.0, *)"));
    assert!(ios.contains("attachmentAnchor: .rect(.bounds), arrowEdge: nil"));
    assert!(!ios.contains("arrowEdge: .bottom"));
}
