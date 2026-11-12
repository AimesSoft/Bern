#[path = "../../bern-target-api/tests/support/icon_decorations.rs"]
mod support;

#[test]
fn icon_progress_and_badge_keep_the_plain_button_path() {
    let source = support::generated(&bern_backend_winui3::WinUi3Windows, "windows", None);
    assert!(source.contains(".iconProgress = "), "{source}");
    assert!(source.contains(r#".badgeIcon = "checkmark""#), "{source}");
    assert!(source.contains(".badgeVisible = "), "{source}");
    assert!(source.contains("decorated_icon"));
    assert!(source.contains("plain_icon"));
}
