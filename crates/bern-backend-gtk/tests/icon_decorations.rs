#[path = "../../bern-target-api/tests/support/icon_decorations.rs"]
mod support;

#[test]
fn icon_progress_and_badge_keep_the_plain_button_path() {
    let source = support::generated(&bern_backend_gtk::GtkLinux, "linux", None);
    assert!(source.contains("icon_progress: Some("), "{source}");
    assert!(
        source.contains(r#"badge_icon: Some("checkmark")"#),
        "{source}"
    );
    assert!(source.contains("badge_visible: Some("), "{source}");
    assert!(source.contains("decorated_icon"));
    assert!(source.contains("plain_icon"));
}
