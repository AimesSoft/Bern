//! Project configuration for early checks.
//!
//! The build tool performs the authoritative checks; reading `Bern.toml`
//! here lets missing translations fail inside the editor with an exact
//! span, and gives the shell protocol's `Client` the module switches the
//! application composes. Files read are tracked with `include_bytes!` so
//! edits re-run macros.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Languages {
    pub supported: Vec<String>,
    pub strict: bool,
    pub path: PathBuf,
}

/// The optional modules of an application, in the order of its
/// `[[modules]]`: what `app.modules.<id>` reads.
#[derive(Clone, Debug, Default)]
pub struct OptionalModules {
    pub ids: Vec<String>,
    /// Every module manifest read, to re-run the macro when one changes.
    pub manifests: Vec<PathBuf>,
}

/// Find `Bern.toml` in the crate directory or an ancestor.
fn nearest_manifest() -> Option<PathBuf> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let mut directory = Some(PathBuf::from(manifest));
    while let Some(current) = directory {
        let candidate = current.join("Bern.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        directory = current.parent().map(PathBuf::from);
    }
    None
}

fn read(path: &Path) -> Result<toml::Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    text.parse()
        .map_err(|error| format!("{} is not valid TOML: {error}", path.display()))
}

pub fn languages() -> Result<Option<Languages>, String> {
    match nearest_manifest() {
        Some(path) => parse(path).map(Some),
        None => Ok(None),
    }
}

fn parse(path: PathBuf) -> Result<Languages, String> {
    let value = read(&path)?;
    let languages = value
        .get("languages")
        .ok_or_else(|| format!("{} needs a [languages] table", path.display()))?;
    let supported: Vec<String> = languages
        .get("supported")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "[languages] needs `supported = [\"…\"]`".to_string())?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect();
    let strict = languages
        .get("strict")
        .and_then(toml::Value::as_bool)
        .unwrap_or(true);
    Ok(Languages {
        supported,
        strict,
        path,
    })
}

/// The manifests of the modules of the application in `app`, read from
/// `path`, in the order of its `[[modules]]`.
fn module_manifests(path: &Path, app: &toml::Value) -> Result<Vec<(PathBuf, toml::Value)>, String> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let entries = app.get("modules").and_then(toml::Value::as_array);
    entries
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("path").and_then(toml::Value::as_str))
        .map(|module_dir| {
            let manifest = dir.join(module_dir).join("Bern.toml");
            read(&manifest).map(|module| (manifest, module))
        })
        .collect()
}

/// The optional modules when this crate is the shell protocol of the
/// application in the nearest `Bern.toml` (its `[protocol] package`);
/// none for any other crate.
pub fn optional_modules() -> Result<OptionalModules, String> {
    match (nearest_manifest(), std::env::var("CARGO_PKG_NAME")) {
        (Some(path), Ok(package)) => optional_modules_of(&path, &package),
        _ => Ok(OptionalModules::default()),
    }
}

/// The optional modules of the application in `path` when `package` is
/// its shell protocol.
fn optional_modules_of(path: &Path, package: &str) -> Result<OptionalModules, String> {
    let app = read(path)?;
    let shell = app
        .get("protocol")
        .and_then(|protocol| protocol.get("package"))
        .and_then(toml::Value::as_str);
    if shell != Some(package) {
        return Ok(OptionalModules::default());
    }
    let mut modules = OptionalModules::default();
    for (manifest, module) in module_manifests(path, &app)? {
        modules.manifests.push(manifest);
        let module = module.get("module");
        let optional = module
            .and_then(|module| module.get("optional"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        let id = module
            .and_then(|module| module.get("id"))
            .and_then(toml::Value::as_str);
        if let (true, Some(id)) = (optional, id) {
            modules.ids.push(id.to_string());
        }
    }
    Ok(modules)
}

/// A page a module adds to a page slot: its id and its label by language.
#[derive(Clone, Debug)]
pub struct ModulePage {
    pub id: String,
    pub label: Vec<(String, String)>,
}

/// The pages the modules of the application in the nearest `Bern.toml`
/// add to `slot`, in order, and every module manifest read.
pub fn module_pages(slot: &str) -> Result<(Vec<ModulePage>, Vec<PathBuf>), String> {
    match nearest_manifest() {
        Some(path) => module_pages_of(&path, slot),
        None => Ok(Default::default()),
    }
}

/// The pages the modules of the application in `path` add to `slot`.
fn module_pages_of(path: &Path, slot: &str) -> Result<(Vec<ModulePage>, Vec<PathBuf>), String> {
    let app = read(path)?;
    let mut pages = Vec::new();
    let mut manifests = Vec::new();
    for (manifest, module) in module_manifests(path, &app)? {
        manifests.push(manifest);
        let entries = module.get("pages").and_then(toml::Value::as_array);
        for page in entries.into_iter().flatten() {
            if page.get("slot").and_then(toml::Value::as_str) != Some(slot) {
                continue;
            }
            let Some(id) = page.get("id").and_then(toml::Value::as_str) else {
                continue;
            };
            let label = page
                .get("label")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flatten()
                .filter_map(|(language, text)| Some((language.clone(), text.as_str()?.to_string())))
                .collect();
            pages.push(ModulePage {
                id: id.to_string(),
                label,
            });
        }
    }
    Ok((pages, manifests))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An application with a shell protocol `app-protocol`, an optional
    /// `notes` module adding a page to `main`, and a required `sync` module.
    fn application(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("bern-macros-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for module in ["notes", "sync"] {
            std::fs::create_dir_all(root.join("modules").join(module)).unwrap();
        }
        std::fs::write(
            root.join("Bern.toml"),
            "[protocol]\npackage = \"app-protocol\"\n\n\
             [[modules]]\npath = \"modules/notes\"\n\n\
             [[modules]]\npath = \"modules/sync\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("modules/notes/Bern.toml"),
            "[module]\nid = \"notes\"\noptional = true\n\n\
             [[pages]]\nslot = \"main\"\nid = \"recent_notes\"\n\
             label = { \"en\" = \"Recent notes\", \"ja\" = \"最近のメモ\" }\n\n\
             [[pages]]\nslot = \"settings\"\nid = \"notes\"\nlabel = { \"en\" = \"Notes\" }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("modules/sync/Bern.toml"),
            "[module]\nid = \"sync\"\n",
        )
        .unwrap();
        root.join("Bern.toml")
    }

    #[test]
    fn the_shell_protocol_gets_the_optional_modules() {
        let manifest = application("optional");
        let modules = optional_modules_of(&manifest, "app-protocol").unwrap();
        assert_eq!(modules.ids, ["notes"]);
        assert_eq!(modules.manifests.len(), 2);
        // Any other crate of the application gets none.
        let other = optional_modules_of(&manifest, "app-ui").unwrap();
        assert!(other.ids.is_empty());
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    #[test]
    fn a_page_enum_gets_the_pages_of_its_slot() {
        let manifest = application("pages");
        let (pages, manifests) = module_pages_of(&manifest, "main").unwrap();
        assert_eq!(manifests.len(), 2);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "recent_notes");
        let mut label = pages[0].label.clone();
        label.sort();
        assert_eq!(
            label,
            [
                ("en".to_string(), "Recent notes".to_string()),
                ("ja".to_string(), "最近のメモ".to_string()),
            ]
        );
        let (none, _) = module_pages_of(&manifest, "sidebar").unwrap();
        assert!(none.is_empty());
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }

    #[test]
    fn a_missing_module_manifest_is_named() {
        let manifest = application("missing");
        std::fs::remove_file(manifest.parent().unwrap().join("modules/sync/Bern.toml")).unwrap();
        let error = optional_modules_of(&manifest, "app-protocol").unwrap_err();
        assert!(error.contains("modules/sync/Bern.toml"), "{error}");
        let _ = std::fs::remove_dir_all(manifest.parent().unwrap());
    }
}
