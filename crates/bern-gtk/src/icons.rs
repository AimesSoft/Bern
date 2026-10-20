//! Bern icon names (SF Symbols naming) as freedesktop symbolic icons from
//! the system icon theme. GNOME draws them from Adwaita; other themes
//! provide the same names or inherit Adwaita, so icons follow the user's
//! theme. The table is shared with `bern-backend-gtk`, whose check rejects
//! the names it lacks; `image-missing-symbolic` is only a last resort.

const ICONS: &[(&str, &str)] = include!("icon_table.rs");

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
        assert!(ICONS.windows(2).all(|pair| pair[0].0 < pair[1].0));
        for (bern, theme) in ICONS {
            assert!(theme.ends_with("-symbolic"), "{bern}");
        }
    }

    /// Where Adwaita is installed, every icon of the table is in it, or in
    /// libadwaita's own resources (`adw-`).
    #[test]
    fn every_theme_icon_is_in_adwaita() {
        let root = std::path::Path::new("/usr/share/icons/Adwaita");
        if !root.is_dir() {
            return;
        }
        fn files(dir: &std::path::Path, found: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files(&path, found);
                } else if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    found.push(stem.to_owned());
                }
            }
        }
        let mut found = Vec::new();
        files(root, &mut found);
        for (bern, theme) in ICONS {
            assert!(
                theme.starts_with("adw-") || found.iter().any(|stem| stem == theme),
                "{bern}: Adwaita has no {theme}"
            );
        }
    }
}
