//! Bern icon names (SF Symbols naming) as Material icons: an icon of
//! Compose's `material-icons-core` (`Icons.…`), or one of the Material
//! icons `BernViews.kt` draws from their paths for the names the core set
//! lacks. The table is the one source: it becomes the branches of the
//! runtime's `BernIcons.get`, and the check rejects the names it lacks.

/// `(name, Kotlin expression of its ImageVector)`, sorted by name.
pub const ICONS: &[(&str, &str)] = &[
    ("archivebox", "archive"),
    ("archivebox.fill", "archive"),
    ("arrow.clockwise", "Icons.Filled.Refresh"),
    ("arrow.down", "arrowDownward"),
    ("arrow.down.circle", "download"),
    ("arrow.down.to.line", "download"),
    ("arrow.left", "Icons.AutoMirrored.Filled.ArrowBack"),
    ("arrow.right", "Icons.AutoMirrored.Filled.ArrowForward"),
    ("arrow.triangle.2.circlepath", "sync"),
    ("arrow.up.arrow.down", "swapVert"),
    ("arrow.up.left.and.arrow.down.right", "openInFull"),
    ("arrow.up.right.square", "openInNew"),
    ("backward.end.fill", "skipPrevious"),
    ("bell", "Icons.Filled.Notifications"),
    ("bell.fill", "Icons.Filled.Notifications"),
    ("calendar", "Icons.Filled.DateRange"),
    ("camera", "photoCamera"),
    ("captions.bubble", "subtitles"),
    ("captions.bubble.fill", "subtitles"),
    ("cart", "Icons.Filled.ShoppingCart"),
    ("character.cursor.ibeam", "textFields"),
    ("checkmark", "Icons.Filled.Check"),
    ("checkmark.circle", "Icons.Filled.CheckCircle"),
    ("checkmark.circle.fill", "Icons.Filled.CheckCircle"),
    ("checkmark.seal", "Icons.Filled.Done"),
    ("chevron.down", "Icons.Filled.KeyboardArrowDown"),
    (
        "chevron.left",
        "Icons.AutoMirrored.Filled.KeyboardArrowLeft",
    ),
    (
        "chevron.right",
        "Icons.AutoMirrored.Filled.KeyboardArrowRight",
    ),
    ("chevron.up", "Icons.Filled.KeyboardArrowUp"),
    ("chevron.up.chevron.down", "Icons.Filled.KeyboardArrowDown"),
    ("circle", "radioUnchecked"),
    ("circle.fill", "circle"),
    ("circle.lefthalf.filled", "palette"),
    ("dial.low", "tune"),
    ("doc", "Icons.AutoMirrored.Filled.List"),
    ("doc.text", "Icons.AutoMirrored.Filled.List"),
    ("ellipsis", "Icons.Filled.MoreVert"),
    ("ellipsis.circle", "Icons.Filled.MoreVert"),
    ("envelope", "Icons.Filled.Email"),
    ("exclamationmark.triangle", "Icons.Filled.Warning"),
    ("eye.slash", "visibilityOff"),
    ("face.smiling", "Icons.Filled.Face"),
    ("film", "movie"),
    ("film.fill", "movie"),
    ("film.stack", "videoLibrary"),
    ("folder", "folder"),
    ("folder.fill", "folder"),
    ("forward.end.fill", "skipNext"),
    ("gear", "Icons.Filled.Settings"),
    ("gearshape", "Icons.Filled.Settings"),
    ("gearshape.2", "Icons.Filled.Settings"),
    ("globe", "language"),
    ("gobackward.10", "replay10"),
    ("goforward.10", "forward10"),
    ("hammer", "Icons.Filled.Build"),
    ("hand.tap", "touchApp"),
    ("hand.thumbsup", "Icons.Filled.ThumbUp"),
    ("heart", "Icons.Filled.FavoriteBorder"),
    ("heart.fill", "Icons.Filled.Favorite"),
    ("house", "Icons.Filled.Home"),
    ("house.fill", "Icons.Filled.Home"),
    ("info", "Icons.Filled.Info"),
    ("info.circle", "Icons.Filled.Info"),
    ("keyboard", "keyboard"),
    ("line.3.horizontal", "Icons.Filled.Menu"),
    ("line.3.horizontal.decrease", "filterList"),
    ("link", "link"),
    ("list.bullet", "Icons.AutoMirrored.Filled.List"),
    ("list.bullet.rectangle", "Icons.AutoMirrored.Filled.List"),
    ("location", "Icons.Filled.LocationOn"),
    ("lock", "Icons.Filled.Lock"),
    ("lock.fill", "Icons.Filled.Lock"),
    ("magnifyingglass", "Icons.Filled.Search"),
    ("map", "Icons.Filled.Place"),
    ("mappin", "Icons.Filled.LocationOn"),
    ("minus", "remove"),
    ("moon", "darkMode"),
    ("music.note", "music"),
    ("music.note.list", "music"),
    ("note.text", "Icons.AutoMirrored.Filled.List"),
    ("paintbrush", "palette"),
    ("paintpalette", "palette"),
    ("paperplane", "Icons.AutoMirrored.Filled.Send"),
    ("pause", "pause"),
    ("pause.fill", "pause"),
    ("pencil", "Icons.Filled.Edit"),
    ("pencil.line", "Icons.Filled.Create"),
    ("person", "Icons.Filled.Person"),
    ("person.circle", "Icons.Filled.AccountCircle"),
    ("person.crop.circle", "Icons.Filled.Person"),
    ("person.fill", "Icons.Filled.Person"),
    ("phone", "Icons.Filled.Phone"),
    ("phone.fill", "Icons.Filled.Call"),
    ("photo", "photo"),
    ("photo.fill", "photo"),
    ("photo.on.rectangle", "photo"),
    ("pip.enter", "pictureInPicture"),
    ("play", "Icons.Filled.PlayArrow"),
    ("play.circle", "Icons.Filled.PlayArrow"),
    ("play.fill", "Icons.Filled.PlayArrow"),
    ("play.rectangle", "Icons.Filled.PlayArrow"),
    ("play.rectangle.fill", "movie"),
    ("plus", "Icons.Filled.Add"),
    ("plus.circle", "Icons.Filled.Add"),
    ("puzzlepiece.extension", "extension"),
    ("rectangle.grid.2x2", "grid"),
    ("scissors", "contentCut"),
    ("server.rack", "dns"),
    ("shield", "shield"),
    ("sidebar.left", "Icons.Filled.Menu"),
    ("sidebar.right", "sidebarRight"),
    ("sidebar.trailing", "sidebarRight"),
    ("slider.horizontal.3", "tune"),
    ("sparkles", "autoAwesome"),
    ("speaker", "volumeMute"),
    ("speaker.fill", "volumeMute"),
    ("speaker.slash", "volumeOff"),
    ("speaker.slash.fill", "volumeOff"),
    ("speaker.wave.1", "volumeDown"),
    ("speaker.wave.1.fill", "volumeDown"),
    ("speaker.wave.2", "volumeUp"),
    ("speaker.wave.2.fill", "volumeUp"),
    ("speaker.wave.3", "volumeUp"),
    ("speaker.wave.3.fill", "volumeUp"),
    ("square", "cropSquare"),
    ("square.and.arrow.up", "Icons.Filled.Share"),
    ("square.and.pencil", "Icons.Filled.Edit"),
    ("square.grid.2x2", "grid"),
    ("square.grid.2x2.fill", "grid"),
    ("square.grid.3x3", "gridOn"),
    ("square.grid.3x3.fill", "gridOn"),
    ("square.on.square", "filterNone"),
    ("star", "Icons.Filled.Star"),
    ("star.fill", "Icons.Filled.Star"),
    ("star.slash", "starBorder"),
    ("sun.max", "brightnessHigh"),
    ("sun.max.fill", "brightnessHigh"),
    ("sun.min", "brightnessLow"),
    ("sun.min.fill", "brightnessLow"),
    ("switch.2", "toggleOn"),
    ("tag", "tag"),
    ("tag.fill", "tag"),
    ("text.bubble", "chat"),
    ("text.bubble.fill", "chat"),
    ("textformat", "text"),
    ("textformat.size", "text"),
    ("textformat.size.larger", "text"),
    ("textformat.size.smaller", "text"),
    ("trash", "Icons.Filled.Delete"),
    ("trash.fill", "Icons.Filled.Delete"),
    ("tray", "inbox"),
    ("tray.fill", "inbox"),
    ("tray.full", "inbox"),
    ("tv", "tv"),
    ("video", "movie"),
    ("video.fill", "movie"),
    ("wifi", "wifi"),
    ("wrench", "Icons.Filled.Build"),
    ("xmark", "Icons.Filled.Close"),
    ("xmark.circle", "Icons.Filled.Close"),
    ("xmark.circle.fill", "Icons.Filled.Close"),
];

/// Whether the runtime draws `name`.
pub fn has(name: &str) -> bool {
    ICONS
        .binary_search_by(|(known, _)| known.cmp(&name))
        .is_ok()
}

/// The branches of `BernIcons.get`, one per name.
pub(crate) fn branches() -> String {
    ICONS
        .iter()
        .map(|(name, icon)| format!("\"{name}\" -> {icon}"))
        .collect::<Vec<_>>()
        .join("\n        ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_draws_the_bern_icon_set() {
        assert!(ICONS.windows(2).all(|pair| pair[0].0 < pair[1].0));
        for name in bern_target_api::icon_set::NAMES {
            assert!(has(name), "the Material icon table lacks {name}");
        }
        assert!(!has("no.such.symbol"));
    }

    #[test]
    fn every_icon_is_defined_by_the_runtime() {
        let views = include_str!("../templates/BernViews.kt");
        for (name, icon) in ICONS {
            let defined = match icon.strip_prefix("Icons.") {
                // `Icons.AutoMirrored.Filled.Send` is imported from
                // `….icons.automirrored.filled.Send`.
                Some(core) => {
                    let (style, last) = core.rsplit_once('.').unwrap();
                    let package = style.to_lowercase();
                    views.contains(&format!(
                        "import androidx.compose.material.icons.{package}.{last}\n"
                    ))
                }
                None => views.contains(&format!("private val {icon} by lazy")),
            };
            assert!(defined, "{name}: BernViews.kt does not define {icon}");
        }
    }
}
