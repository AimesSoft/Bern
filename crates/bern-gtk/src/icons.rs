//! Bern icon names (SF Symbols naming) as freedesktop symbolic icons of
//! Adwaita, which the application keeps as its icon theme where it is
//! installed (other themes, Breeze on KDE among them, lack many of these
//! names or color them). Every name is in Adwaita 46 and in the releases
//! after it, which dropped some emblems. `bern-backend-gtk` keeps the same
//! table, and its check rejects the names it lacks;
//! `image-missing-symbolic` is only a last resort.

use std::cell::RefCell;

const ICONS: &[(&str, &str)] = include!("icon_table.rs");

thread_local! {
    /// Registered application `.svg` icons: `(bern_name, theme_symbolic_name)`.
    static APP_ICONS: RefCell<Vec<(&'static str, &'static str)>> = const { RefCell::new(Vec::new()) };
}

/// Register application `.svg` icons into a per-build symbolic icon directory
/// added to GTK's icon theme search path so every widget tints them natively.
pub(crate) fn install(app_id: &str, fingerprint: u64, icons: &'static [(&'static str, &'static str)]) {
    if icons.is_empty() {
        return;
    }
    let dir = gtk::glib::user_cache_dir()
        .join("bern-icons")
        .join(format!("{app_id}-{fingerprint:016x}"));
    let _ = std::fs::create_dir_all(&dir);
    let mut mapped = Vec::with_capacity(icons.len());
    for (index, (bern_name, svg)) in icons.iter().enumerate() {
        let theme_name: &'static str = Box::leak(
            format!("bern-app-icon-{index}-symbolic").into_boxed_str(),
        );
        let file = dir.join(format!("{theme_name}.svg"));
        let _ = std::fs::write(&file, svg.as_bytes());
        mapped.push((*bern_name, theme_name));
    }
    APP_ICONS.with(|cell| *cell.borrow_mut() = mapped);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_search_path(&dir);
    }
}

/// The theme icon of a Bern icon name or registered application `.svg` icon.
pub(crate) fn name(icon: &str) -> &'static str {
    if let Some(theme) = APP_ICONS.with(|cell| {
        cell.borrow()
            .iter()
            .find(|(bern, _)| *bern == icon)
            .map(|(_, theme)| *theme)
    }) {
        return theme;
    }
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
        assert_eq!(name("checkmark.circle.fill"), "object-select-symbolic");
        assert_eq!(name("gearshape"), "preferences-system-symbolic");
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
