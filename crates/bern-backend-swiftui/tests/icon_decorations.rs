#[path = "../../bern-target-api/tests/support/icon_decorations.rs"]
mod support;

#[test]
fn icon_progress_and_badge_keep_the_plain_button_path() {
    let source = support::generated(&bern_backend_swiftui::SwiftUiMacos, "macos", None);
    assert!(source.contains("BernIconGlyph(name:"), "{source}");
    assert!(source.contains("badgeVisible: false"), "{source}");
    assert!(source.contains("SwiftUI.Label("), "{source}");
    assert!(source.contains("decorated_icon"));
    assert!(source.contains("plain_icon"));
}
