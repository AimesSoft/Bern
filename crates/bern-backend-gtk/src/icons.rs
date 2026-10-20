//! The icons the GTK runtime draws: its table of Bern names and theme
//! icons, read from `bern-gtk` so the check and the runtime agree.

/// `(Bern name, freedesktop symbolic icon)`, sorted by Bern name.
pub const ICONS: &[(&str, &str)] = include!("../../bern-gtk/src/icon_table.rs");

/// Whether the runtime draws `name`.
pub fn has(name: &str) -> bool {
    ICONS
        .binary_search_by(|(known, _)| known.cmp(&name))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_draws_the_bern_icon_set() {
        assert!(ICONS.windows(2).all(|pair| pair[0].0 < pair[1].0));
        for name in bern_target_api::icon_set::NAMES {
            assert!(has(name), "the GTK icon table lacks {name}");
        }
        assert!(!has("no.such.symbol"));
    }
}
