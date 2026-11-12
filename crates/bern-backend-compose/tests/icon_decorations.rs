#[path = "../../bern-target-api/tests/support/icon_decorations.rs"]
mod support;

#[test]
fn icon_progress_and_badge_keep_the_plain_button_path() {
    let source = support::generated(&bern_backend_compose::ComposeAndroid, "android", None);
    assert!(
        source.contains("iconProgress = (0.5).toDouble()"),
        "{source}"
    );
    assert!(
        source.contains(r#"badgeIcon = BernIcons.get("checkmark")"#),
        "{source}"
    );
    assert!(source.contains("badgeVisible = false"), "{source}");
    assert!(
        source.contains("iconProgress = (1L).toDouble()"),
        "{source}"
    );
    assert!(source.contains("decorated_icon"));
    assert!(source.contains("plain_icon"));
}
