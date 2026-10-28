//! Engine of the gallery.

use bern::CommandError;
use bern::media::MediaSource;
use bern::query::Answer;
use bern::resources::Resources;
use gallery_protocol::gallery::{
    self, Accent, Appearance, Container, Notice, NoticeKind, State, Swatch,
};
use std::io::Read;

pub struct GalleryEngine {
    state: State,
}

impl Default for GalleryEngine {
    fn default() -> Self {
        let names = [
            "Gingham",
            "Tartan",
            "Madras",
            "Glen",
            "Houndstooth",
            "Windowpane",
        ];
        Self {
            state: State {
                swatches: names
                    .iter()
                    .zip(1..)
                    .map(|(name, id)| Swatch {
                        id,
                        name: (*name).into(),
                        favorite: false,
                        at: (id - 1) as f64 / (names.len() - 1).max(1) as f64,
                    })
                    .collect(),
                favorites: Vec::new(),
                opened: 0,
                opened_name: String::new(),
                notice: None,
                notice_id: 0,
                clip: MediaSource::resource(&Resources::host(), "media/tides.mp4")
                    .expect("the gallery bundles media/tides.mp4"),
                poster: MediaSource::resource(&Resources::host(), "media/poster.jpg")
                    .expect("the gallery bundles media/poster.jpg"),
                accent: Accent::Crimson,
                appearance: Appearance::System,
                caption: true,
                details_key: "cmd+alt+i".into(),
                by_name: false,
                descending: false,
            },
        }
    }
}

impl GalleryEngine {
    fn swatch(&mut self, id: i64) -> Result<&mut Swatch, CommandError> {
        self.state
            .swatches
            .iter_mut()
            .find(|swatch| swatch.id == id)
            .ok_or_else(|| CommandError::new("unknown_swatch", format!("no pattern {id}")))
    }

    fn notify(&mut self, notice: Notice) {
        self.state.notice = Some(notice);
        self.state.notice_id += 1;
    }

    fn collect_favorites(&mut self) {
        self.state.favorites = self
            .state
            .swatches
            .iter()
            .filter(|swatch| swatch.favorite)
            .cloned()
            .collect();
    }
}

impl gallery::Commands for GalleryEngine {
    fn open(&mut self, id: i64) -> Result<(), CommandError> {
        let name = self.swatch(id)?.name.clone();
        self.state.opened = id;
        self.state.opened_name = name;
        Ok(())
    }

    fn toggle_favorite(&mut self, id: i64) -> Result<(), CommandError> {
        let swatch = self.swatch(id)?;
        swatch.favorite = !swatch.favorite;
        let notice = Notice {
            kind: if swatch.favorite {
                NoticeKind::Favorite
            } else {
                NoticeKind::NoLongerFavorite
            },
            name: swatch.name.clone(),
            container: Container::Other,
            kilobytes: 0,
        };
        self.collect_favorites();
        self.notify(notice);
        Ok(())
    }

    fn set_accent(&mut self, accent: Accent) -> Result<(), CommandError> {
        self.state.accent = accent;
        Ok(())
    }

    fn set_appearance(&mut self, appearance: Appearance) -> Result<(), CommandError> {
        self.state.appearance = appearance;
        Ok(())
    }

    fn set_caption(&mut self, shown: bool) -> Result<(), CommandError> {
        self.state.caption = shown;
        Ok(())
    }

    fn set_details_key(&mut self, keys: String) -> Result<(), CommandError> {
        self.state.details_key = if keys.trim().is_empty() {
            String::new()
        } else {
            bern::shortcut::parse_shortcut(&keys)
                .map_err(|message| CommandError::new("invalid_shortcut", message))?
        };
        Ok(())
    }

    fn play(&mut self, source: MediaSource) -> Result<(), CommandError> {
        let notice = describe(&source);
        self.state.clip = source;
        self.notify(notice);
        Ok(())
    }

    fn set_poster(&mut self, source: MediaSource) -> Result<(), CommandError> {
        self.state.poster = source;
        Ok(())
    }

    fn toggle_favorites(&mut self, ids: Vec<i64>) -> Result<(), CommandError> {
        for swatch in &mut self.state.swatches {
            if ids.contains(&swatch.id) {
                swatch.favorite = !swatch.favorite;
            }
        }
        self.collect_favorites();
        Ok(())
    }

    fn open_chosen(&mut self, ids: Vec<i64>) -> Result<(), CommandError> {
        match ids.first() {
            Some(&id) => self.open(id),
            None => Ok(()),
        }
    }

    fn open_named(&mut self, name: String) -> Result<(), CommandError> {
        let wanted = name.trim().to_lowercase();
        let id = self
            .state
            .swatches
            .iter()
            .find(|swatch| swatch.name.to_lowercase() == wanted)
            .map(|swatch| swatch.id)
            .ok_or_else(|| CommandError::new("unknown_swatch", format!("no pattern {name}")))?;
        self.open(id)
    }

    fn rename(&mut self, id: i64, name: String) -> Result<(), CommandError> {
        if id == 0 {
            return Ok(());
        }
        self.swatch(id)?.name = name.clone();
        if self.state.opened == id {
            self.state.opened_name = name;
        }
        self.collect_favorites();
        Ok(())
    }

    fn sort_by_name(&mut self) -> Result<(), CommandError> {
        match (self.state.by_name, self.state.descending) {
            (false, _) => (self.state.by_name, self.state.descending) = (true, false),
            (true, false) => self.state.descending = true,
            (true, true) => (self.state.by_name, self.state.descending) = (false, false),
        }
        if self.state.by_name {
            let descending = self.state.descending;
            self.state.swatches.sort_by(|a, b| {
                let order = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                if descending { order.reverse() } else { order }
            });
        } else {
            self.state.swatches.sort_by_key(|swatch| swatch.id);
        }
        Ok(())
    }

    fn copy_names(&mut self, ids: Vec<i64>) -> Result<(), CommandError> {
        let names: Vec<&str> = self
            .state
            .swatches
            .iter()
            .filter(|swatch| ids.contains(&swatch.id))
            .map(|swatch| swatch.name.as_str())
            .collect();
        bern::platform::copy_text(&names.join("\n"))
    }

    fn clear_favorites(&mut self) -> Result<(), CommandError> {
        for swatch in &mut self.state.swatches {
            swatch.favorite = false;
        }
        self.collect_favorites();
        Ok(())
    }
}

impl gallery::Queries for GalleryEngine {
    fn find(&self, text: String, answer: Answer<Vec<Swatch>>) {
        let needle = text.trim().to_lowercase();
        answer.ok(if needle.is_empty() {
            Vec::new()
        } else {
            self.state
                .swatches
                .iter()
                .filter(|swatch| swatch.name.to_lowercase().contains(&needle))
                .cloned()
                .collect()
        });
    }
}

impl gallery::Engine for GalleryEngine {
    fn state(&self) -> &State {
        &self.state
    }
}

/// What the notice says about a clip: the name the user knows it by, and,
/// when the engine can read the file, its container and size, taken from
/// the file itself as an engine that recognizes a video by its first bytes
/// would.
fn describe(source: &MediaSource) -> Notice {
    let name = source.file_name().unwrap_or_else(|| source.url.clone());
    let Ok(mut file) = source.open() else {
        return Notice {
            kind: NoticeKind::Playing,
            name,
            container: Container::Other,
            kilobytes: 0,
        };
    };
    let mut head = [0u8; 12];
    let container = match file.read_exact(&mut head) {
        Ok(()) if &head[4..8] == b"ftyp" => Container::Mp4,
        Ok(()) if head[..4] == [0x1a, 0x45, 0xdf, 0xa3] => Container::Matroska,
        _ => Container::Other,
    };
    Notice {
        kind: NoticeKind::PlayingFile,
        name,
        container,
        kilobytes: (file.len() as i64 + 999) / 1000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_are_described_from_their_files() {
        let dir = std::env::temp_dir().join(format!("gallery-describe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Tides #1.mp4");
        let mut bytes = b"\0\0\0\x18ftypmp42".to_vec();
        bytes.resize(2_500_000, 0);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            describe(&MediaSource::file(&path)),
            Notice {
                kind: NoticeKind::PlayingFile,
                name: "Tides #1.mp4".into(),
                container: Container::Mp4,
                kilobytes: 2_500,
            }
        );
        assert_eq!(
            describe(&MediaSource::new("https://example.com/a%20b.mkv")),
            Notice {
                kind: NoticeKind::Playing,
                name: "a b.mkv".into(),
                container: Container::Other,
                kilobytes: 0,
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
