//! Media values for protocols and declarations.
//!
//! `MediaSource` is the framework type for playable media; applications put
//! it in their state, command parameters and UI values. Its schema comes
//! from the shared media contract, so every entry and target agrees on it.

use crate::bind::{BernValue, Binding, Readable, ToRef};
use crate::resources::Resources;
use bern_protocol::{CommandError, SchemaError, Value};

/// Playable media: an http(s) URL or a file URL. A resource locator is not
/// user-visible text and needs no translation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MediaSource {
    pub url: String,
}

impl MediaSource {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// A `file://` source for a local path. Relative paths and `..` are
    /// resolved when the file exists; other characters are percent-encoded.
    pub fn file(path: impl AsRef<std::path::Path>) -> Self {
        let path = path.as_ref();
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut url = String::from("file://");
        for byte in path.to_string_lossy().bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                    url.push(byte as char)
                }
                other => url.push_str(&format!("%{other:02X}")),
            }
        }
        Self { url }
    }

    /// A `file://` source for a resource declared in `Bern.toml`, located
    /// by `resources`. The engine names the resource; the host decides
    /// where the file is, for example inside the app bundle.
    pub fn resource(resources: &Resources, name: &str) -> Result<Self, CommandError> {
        resources.path(name).map(Self::file)
    }
}

/// Typed reference to a `MediaSource` inside declarations.
#[derive(Clone)]
pub struct MediaSourceRef {
    pub url: Binding<String>,
}

impl BernValue for MediaSource {
    type Ref = MediaSourceRef;
    fn reference() -> Self::Ref {
        MediaSourceRef {
            url: Binding::declared(),
        }
    }
    fn to_value(&self) -> Value {
        Value::Record(vec![Value::Text(self.url.clone())])
    }
    fn from_value(value: &Value) -> Result<Self, SchemaError> {
        match value {
            Value::Record(fields) if fields.len() == 1 => Ok(MediaSource {
                url: String::from_value(&fields[0])?,
            }),
            _ => Err(SchemaError::new("expected a `MediaSource` record")),
        }
    }
}

impl Readable<MediaSource> for MediaSourceRef {}

impl ToRef for MediaSourceRef {
    type Ref = MediaSourceRef;
    fn to_ref(&self) -> MediaSourceRef {
        self.clone()
    }
}

/// The type of fields in a `#[bern::players]` struct.
pub type Player = crate::bind::PlayerRef;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_sources_are_encoded_urls() {
        assert_eq!(
            MediaSource::file("/no such dir/clip #1.mp4").url,
            "file:///no%20such%20dir/clip%20%231.mp4"
        );
    }

    #[test]
    fn resource_sources_are_file_urls_of_declared_files() {
        let dir = std::env::temp_dir().join(format!("bern-media-resource-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("media")).unwrap();
        std::fs::write(dir.join("media/clip.mp4"), b"clip").unwrap();
        let resources = Resources::catalog(&dir, ["media/clip.mp4"]);
        assert_eq!(
            MediaSource::resource(&resources, "media/clip.mp4").unwrap(),
            MediaSource::file(dir.join("media/clip.mp4"))
        );
        let error = MediaSource::resource(&resources, "media/other.mp4").unwrap_err();
        assert_eq!(error.code, "undeclared_resource");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn media_source_matches_the_contract_schema() {
        let value = MediaSource::new("file:///clip.mp4").to_value();
        let mut types = bern_protocol::TypeTable::default();
        types.records.push(bern_media::media_source_def());
        value
            .check(
                &bern_protocol::Type::Record {
                    name: bern_media::MEDIA_SOURCE.into(),
                },
                &types,
            )
            .unwrap();
        assert_eq!(
            MediaSource::from_value(&value).unwrap().url,
            "file:///clip.mp4"
        );
    }
}
