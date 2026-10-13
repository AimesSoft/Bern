//! Engine of the gallery.

use bern::CommandError;
use bern::media::MediaSource;
use bern::query::Answer;
use bern::resources::Resources;
use gallery_protocol::gallery::{self, Accent, Appearance, State, Swatch};

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
                notice: String::new(),
                notice_id: 0,
                clip: MediaSource::resource(&Resources::host(), "media/tides.mp4")
                    .expect("the gallery bundles media/tides.mp4"),
                poster: MediaSource::resource(&Resources::host(), "media/poster.jpg")
                    .expect("the gallery bundles media/poster.jpg"),
                accent: Accent::Crimson,
                appearance: Appearance::System,
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

    fn notify(&mut self, notice: String) {
        self.state.notice = notice;
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
        let notice = if swatch.favorite {
            format!("★ {}", swatch.name)
        } else {
            format!("☆ {}", swatch.name)
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

    fn play(&mut self, source: MediaSource) -> Result<(), CommandError> {
        let name = file_name(&source.url);
        self.state.clip = source;
        self.notify(format!("▶ {name}"));
        Ok(())
    }

    fn set_poster(&mut self, source: MediaSource) -> Result<(), CommandError> {
        self.state.poster = source;
        Ok(())
    }

    fn clear_favorites(&mut self) -> Result<(), CommandError> {
        for swatch in &mut self.state.swatches {
            swatch.favorite = false;
        }
        self.collect_favorites();
        Ok(())
    }
}

/// The last part of a URL's path, percent-decoded.
fn file_name(url: &str) -> String {
    let last = url.rsplit('/').next().unwrap_or(url).as_bytes();
    let mut bytes = Vec::with_capacity(last.len());
    let mut i = 0;
    while i < last.len() {
        let hex = last
            .get(i + 1..i + 3)
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match (last[i], hex) {
            (b'%', Some(byte)) => {
                bytes.push(byte);
                i += 3;
            }
            (byte, _) => {
                bytes.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
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
