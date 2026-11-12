#[path = "../../bern-target-api/tests/support/icon_decorations.rs"]
mod support;

#[test]
fn icon_progress_and_badge_keep_the_plain_button_path() {
    let source = support::generated(&bern_backend_gpui::Gpui::MACOS, "macos", Some("gpui"));
    assert!(source.contains("icon_progress: Some("), "{source}");
    assert!(
        source.contains(r#"badge_icon: Some("checkmark")"#),
        "{source}"
    );
    assert!(
        source.contains("badge_visible: rt::truthy(&bern_ffi::protocol::Value::Bool(false))"),
        "{source}"
    );
    assert!(source.contains("decorated_icon"));
    assert!(source.contains("plain_icon"));
}
