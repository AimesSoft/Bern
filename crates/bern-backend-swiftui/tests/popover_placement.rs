//! Popover placement follows the platform's available screen space.
#[test]
fn ios_popovers_leave_the_arrow_edge_to_the_system() {
    let source = include_str!("../templates/BernStyle.swift");
    let start = source
        .find("@ViewBuilder private func presentation(_ content: Content)")
        .unwrap();
    let end = source[start..].find("private func release()").unwrap() + start;
    let presentation = &source[start..end];
    assert!(presentation.contains("#if os(tvOS) || os(watchOS)\n        content.sheet("));
    assert!(presentation.contains(
        "#elseif os(macOS)\n        content.popover(isPresented: $presented, arrowEdge: .bottom)"
    ));
    let ios = presentation.split("#else").last().unwrap();
    assert!(ios.contains("if #available(iOS 18.0, *)"));
    assert!(ios.contains("attachmentAnchor: .rect(.bounds), arrowEdge: nil"));
    assert!(!ios.contains("arrowEdge: .bottom"));
}

#[test]
fn unstyled_popovers_preserve_native_material_on_every_apple_platform() {
    let source = include_str!("../templates/BernStyle.swift");
    let start = source
        .find("@ViewBuilder func bernPopoverBackground(")
        .unwrap();
    let end = start
        + source[start..]
            .find("/// A button that shows its content")
            .unwrap();
    let background = &source[start..end];
    assert!(background.contains("if let background {\n            presentationBackground {"));
    assert!(background.contains("Rectangle().fill(background.color)"));
    let native = background.split("} else {").nth(1).unwrap();
    assert!(native.contains("preferredColorScheme(scheme)"));
    assert!(!native.contains("presentationBackground"));
    assert!(!native.contains("regularMaterial"));
    assert!(
        !native.contains("#if"),
        "the system owns the surface on iOS as well as macOS"
    );
}
