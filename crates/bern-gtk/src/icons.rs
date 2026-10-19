//! Bern icon names (SF Symbols naming) as freedesktop symbolic icons from
//! the system icon theme. GNOME draws them from Adwaita; other themes
//! provide the same names, so icons follow the user's theme.

const ICONS: &[(&str, &str)] = &[
    ("plus", "list-add-symbolic"),
    ("minus", "list-remove-symbolic"),
    ("xmark", "window-close-symbolic"),
    ("checkmark", "object-select-symbolic"),
    ("chevron.left", "go-previous-symbolic"),
    ("chevron.right", "go-next-symbolic"),
    ("chevron.down", "pan-down-symbolic"),
    ("chevron.up.chevron.down", "pan-down-symbolic"),
    ("ellipsis", "view-more-symbolic"),
    ("ellipsis.circle", "view-more-symbolic"),
    ("square.on.square", "edit-copy-symbolic"),
    ("magnifyingglass", "system-search-symbolic"),
    ("xmark.circle.fill", "edit-clear-symbolic"),
    ("doc", "text-x-generic-symbolic"),
    ("photo", "image-x-generic-symbolic"),
    ("film", "video-x-generic-symbolic"),
    ("music.note", "audio-x-generic-symbolic"),
    ("arrow.down.circle", "folder-download-symbolic"),
    ("gearshape", "emblem-system-symbolic"),
    ("gearshape.2", "preferences-other-symbolic"),
    ("paintbrush", "preferences-desktop-appearance-symbolic"),
    ("hand.tap", "input-touchpad-symbolic"),
    ("list.bullet.rectangle", "view-list-symbolic"),
    ("play.rectangle", "camera-video-symbolic"),
    ("play.fill", "media-playback-start-symbolic"),
    ("pause.fill", "media-playback-pause-symbolic"),
    ("speaker.fill", "audio-volume-low-symbolic"),
    ("speaker.wave.3.fill", "audio-volume-high-symbolic"),
    ("speaker.slash.fill", "audio-volume-muted-symbolic"),
    ("sun.min", "display-brightness-symbolic"),
    ("sun.max.fill", "display-brightness-symbolic"),
    ("textformat", "font-x-generic-symbolic"),
    ("textformat.size.smaller", "zoom-out-symbolic"),
    ("textformat.size.larger", "zoom-in-symbolic"),
    ("character.cursor.ibeam", "input-keyboard-symbolic"),
    ("switch.2", "checkbox-checked-symbolic"),
    ("star", "non-starred-symbolic"),
    ("star.fill", "starred-symbolic"),
    ("square.and.arrow.up", "send-to-symbolic"),
    ("trash", "user-trash-symbolic"),
    ("exclamationmark.triangle", "dialog-warning-symbolic"),
    ("tray", "folder-symbolic"),
    ("heart", "emblem-favorite-symbolic"),
    ("square.grid.2x2", "view-grid-symbolic"),
    ("square.grid.3x3", "view-app-grid-symbolic"),
    ("archivebox", "user-bookmarks-symbolic"),
    ("note.text", "accessories-text-editor-symbolic"),
    ("person.crop.circle", "avatar-default-symbolic"),
    ("info.circle", "dialog-information-symbolic"),
    ("star.slash", "semi-starred-symbolic"),
    ("folder", "folder-open-symbolic"),
    ("paintpalette", "preferences-color-symbolic"),
    ("sidebar.right", "sidebar-show-right-symbolic"),
    ("captions.bubble", "media-view-subtitles-symbolic"),
    ("textformat.size", "format-text-plaintext-symbolic"),
];

/// The theme icon of a Bern icon name.
pub(crate) fn name(icon: &str) -> &'static str {
    ICONS
        .iter()
        .find(|(bern, _)| *bern == icon)
        .map(|(_, theme)| *theme)
        .unwrap_or("image-missing-symbolic")
}

pub(crate) fn image(icon: &str) -> gtk::Image {
    gtk::Image::from_icon_name(name(icon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bern_names_map_to_symbolic_theme_icons() {
        assert_eq!(name("plus"), "list-add-symbolic");
        assert_eq!(name("star.fill"), "starred-symbolic");
        assert_eq!(name("no.such.icon"), "image-missing-symbolic");
    }

    #[test]
    fn every_bern_name_is_listed_once() {
        for (index, (bern, theme)) in ICONS.iter().enumerate() {
            assert!(theme.ends_with("-symbolic"), "{bern}");
            assert!(
                !ICONS[..index].iter().any(|(earlier, _)| earlier == bern),
                "{bern} is listed twice"
            );
        }
    }
}
