//! Resource files an application declares in `Bern.toml` and ships with
//! its build.
//!
//! An engine refers to a resource by name — the file's path relative to
//! `Bern.toml`, such as `media/aurora.mp4` — and resolves it through
//! [`Resources`]; it never knows where a target puts the files. The host
//! that runs the engine installs the resources of its product before it
//! constructs the engine, for example the declared files inside a macOS app
//! bundle, or the URL a page serves them from. Tests and headless tools pass
//! resources explicitly, as they pass a clock. A lookup never falls back to
//! another location.

use bern_protocol::{CommandError, check_resource_name};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// Where an engine finds its resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resources {
    located: Option<Arc<Located>>,
}

#[derive(Debug, PartialEq, Eq)]
struct Located {
    /// Directory holding the files, when the engine can read them.
    root: Option<PathBuf>,
    /// URL prefix the files are served from, ending in `/`, when whoever
    /// plays or shows them is a web page.
    url_base: Option<String>,
    /// Declared names; `None` serves every file below `root`.
    catalog: Option<BTreeSet<String>>,
}

static HOST: OnceLock<Resources> = OnceLock::new();

impl Resources {
    /// The resources the host installed for this process. An engine that
    /// runs without a host, for example in a unit test, gets resources
    /// whose every lookup fails with `resources_unavailable`.
    pub fn host() -> Self {
        HOST.get().cloned().unwrap_or(Self { located: None })
    }

    /// The declared `names` below `root`. Hosts install this, with the
    /// names the build declared and the directory the product keeps them in.
    pub fn catalog(
        root: impl Into<PathBuf>,
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            located: Some(Arc::new(Located {
                root: Some(root.into()),
                url_base: None,
                catalog: Some(names.into_iter().map(Into::into).collect()),
            })),
        }
    }

    /// The declared `names` as served below `url_base`, a URL prefix such as
    /// `resources/` relative to the page. The build copies every declared
    /// file there, so lookups check the catalog but not the server. With
    /// `root`, the engine can also read the files from that directory.
    pub fn served(
        url_base: impl Into<String>,
        root: Option<PathBuf>,
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut url_base = url_base.into();
        if !url_base.is_empty() && !url_base.ends_with('/') {
            url_base.push('/');
        }
        Self {
            located: Some(Arc::new(Located {
                root,
                url_base: Some(url_base),
                catalog: Some(names.into_iter().map(Into::into).collect()),
            })),
        }
    }

    /// Every file below `root`, without a catalog: for tests and tools that
    /// run an engine against the project's sources. Products never use it.
    pub fn directory(root: impl Into<PathBuf>) -> Self {
        Self {
            located: Some(Arc::new(Located {
                root: Some(root.into()),
                url_base: None,
                catalog: None,
            })),
        }
    }

    /// Install these resources as the host's for this process. A host does
    /// this once, before it constructs the engine; installing different
    /// resources afterwards fails.
    pub fn install(self) -> Result<(), String> {
        let installed = HOST.get_or_init(|| self.clone());
        if *installed == self {
            Ok(())
        } else {
            Err(format!(
                "this process already uses the resources in {}",
                installed
                    .root()
                    .map_or_else(|| "no directory".into(), |root| root.display().to_string())
            ))
        }
    }

    /// Whether the resources are served to a page under a URL prefix.
    pub fn is_served(&self) -> bool {
        self.located
            .as_deref()
            .is_some_and(|located| located.url_base.is_some())
    }

    /// Directory that holds the resources, if any.
    pub fn root(&self) -> Option<&Path> {
        self.located
            .as_deref()
            .and_then(|located| located.root.as_deref())
    }

    /// The file of a declared resource.
    ///
    /// Errors: `invalid_resource_name`, `resources_unavailable` (no
    /// resources were installed), `undeclared_resource` (not in the
    /// catalog, even if a file of that name exists) and `resource_missing`
    /// (declared, but the product lacks the file).
    pub fn path(&self, name: &str) -> Result<PathBuf, CommandError> {
        let located = self.declared(name)?;
        let Some(root) = &located.root else {
            return Err(CommandError::new(
                "resources_unavailable",
                format!("`{name}` is served to a page; this engine cannot read its file"),
            ));
        };
        let path = root.join(name);
        if path.is_file() {
            Ok(path)
        } else {
            Err(CommandError::new(
                "resource_missing",
                format!("resource `{name}` is missing: {}", path.display()),
            ))
        }
    }

    /// The URL a player or view loads a declared resource from: below the
    /// served URL prefix, or the `file:` URL of its file.
    pub fn url(&self, name: &str) -> Result<String, CommandError> {
        let located = self.declared(name)?;
        match &located.url_base {
            Some(base) => Ok(format!("{base}{name}")),
            None => self.path(name).map(|path| file_url(&path)),
        }
    }

    fn declared(&self, name: &str) -> Result<&Located, CommandError> {
        check_resource_name(name)
            .map_err(|message| CommandError::new("invalid_resource_name", message))?;
        let Some(located) = &self.located else {
            return Err(CommandError::new(
                "resources_unavailable",
                format!(
                    "cannot resolve `{name}`: no host installed resources; pass `Resources` explicitly when the engine runs without an app"
                ),
            ));
        };
        if let Some(catalog) = &located.catalog
            && !catalog.contains(name)
        {
            return Err(CommandError::new(
                "undeclared_resource",
                format!("`{name}` is not declared in the [resources] of Bern.toml"),
            ));
        }
        Ok(located)
    }
}

/// `file://` URL of an absolute path, percent-encoding what URLs reserve.
pub fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                url.push(byte as char)
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(test: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bern-resources-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("media")).unwrap();
        std::fs::write(dir.join("media/clip.mp4"), b"clip").unwrap();
        std::fs::write(dir.join("media/extra.mp4"), b"extra").unwrap();
        dir
    }

    #[test]
    fn lookups_are_strict() {
        let dir = project("strict");
        let resources = Resources::catalog(&dir, ["media/clip.mp4", "media/gone.mp4"]);
        assert_eq!(
            resources.path("media/clip.mp4").unwrap(),
            dir.join("media/clip.mp4")
        );
        let code = |name: &str| resources.path(name).unwrap_err().code;
        // A file outside the catalog is not a resource, even if it exists.
        assert_eq!(code("media/extra.mp4"), "undeclared_resource");
        assert_eq!(code("media/gone.mp4"), "resource_missing");
        assert_eq!(code("../media/clip.mp4"), "invalid_resource_name");
        assert_eq!(code("/etc/hosts"), "invalid_resource_name");

        // Without a catalog every file below the directory resolves.
        let sources = Resources::directory(&dir);
        assert!(sources.path("media/extra.mp4").is_ok());
        assert_eq!(
            sources.path("media/none.mp4").unwrap_err().code,
            "resource_missing"
        );
        assert_eq!(
            Resources { located: None }
                .path("media/clip.mp4")
                .unwrap_err()
                .code,
            "resources_unavailable"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn served_resources_resolve_to_urls_below_the_prefix() {
        let resources = Resources::served("resources", None, ["media/clip.mp4"]);
        assert_eq!(
            resources.url("media/clip.mp4").unwrap(),
            "resources/media/clip.mp4"
        );
        assert_eq!(
            resources.url("media/other.mp4").unwrap_err().code,
            "undeclared_resource"
        );
        // A page's resources have no file for the engine to read.
        assert_eq!(
            resources.path("media/clip.mp4").unwrap_err().code,
            "resources_unavailable"
        );
        let dir = project("served");
        let files = Resources::catalog(&dir, ["media/clip.mp4"]);
        assert_eq!(
            files.url("media/clip.mp4").unwrap(),
            file_url(&dir.join("media/clip.mp4"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
