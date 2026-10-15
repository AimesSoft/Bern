//! `Bern.toml`: project, language, theme, source, resource and target
//! configuration.

use bern_ir::{LayoutConfig, ResourceDef, SourceLocation, Srgb, ThemeMode};
use bern_target_api::{Diagnostic, TargetConfig, WINDOWS_FRONTENDS};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    app: RawApp,
    languages: RawLanguages,
    #[serde(default)]
    theme: RawTheme,
    protocol: RawProtocol,
    engine: RawEngine,
    ui: RawUi,
    #[serde(default)]
    targets: BTreeMap<String, RawTarget>,
    #[serde(default)]
    libraries: Vec<RawLibraryRef>,
    #[serde(default)]
    modules: Vec<RawModuleRef>,
    resources: Option<RawResources>,
    layouts: Option<RawLayouts>,
}

/// `[[modules]]`: a module directory holding its own `Bern.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModuleRef {
    path: String,
}

/// A module manifest: `[module]` with its protocol, engine and UI, and the
/// languages its translations cover.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModuleConfig {
    module: RawModule,
    languages: RawLanguages,
    #[serde(default)]
    pages: Vec<RawPage>,
    /// Slot id to the module's components placed there, in order.
    #[serde(default)]
    contributes: BTreeMap<String, Vec<String>>,
}

/// `[[pages]]`: a page the module adds to one of the shell's page slots.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    slot: String,
    id: String,
    label: BTreeMap<String, String>,
    icon: Option<String>,
    badge: Option<String>,
    component: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModule {
    id: String,
    name: BTreeMap<String, String>,
    #[serde(default)]
    depends: Vec<String>,
    #[serde(default)]
    optional: bool,
    /// Whether an optional module runs before the application saved a
    /// choice; by default it does.
    enabled: Option<bool>,
    protocol: RawProtocol,
    engine: RawEngine,
    ui: Option<RawModuleUi>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModuleUi {
    package: String,
    sources: RawSources,
}

/// Source files: a list of `.rs` files, or one directory whose `.rs` files
/// are all collected.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawSources {
    Directory(String),
    Files(Vec<String>),
}

/// One module of the application.
#[derive(Clone, Debug)]
pub struct ModuleConfig {
    pub id: String,
    pub dir: PathBuf,
    pub manifest: PathBuf,
    pub name: BTreeMap<String, String>,
    pub depends: Vec<String>,
    pub optional: bool,
    /// Whether an optional module runs before the application saved a choice.
    pub enabled: bool,
    pub protocol_package: Package,
    pub protocol_source: SourceFile,
    pub engine_package: Package,
    /// Rust path of the module engine type, which implements `Default`.
    pub engine_type: String,
    pub ui_package: Option<Package>,
    pub ui_sources: Vec<SourceFile>,
    pub languages: Vec<String>,
    /// Pages the module adds to the shell's page slots.
    pub pages: Vec<PageConfig>,
    /// Slot id to the module's components placed there, in order.
    pub contributes: BTreeMap<String, Vec<String>>,
}

/// A page a module contributes to a page slot.
#[derive(Clone, Debug)]
pub struct PageConfig {
    pub slot: String,
    pub id: String,
    pub label: BTreeMap<String, String>,
    pub icon: Option<String>,
    /// Numeric state field of this module shown as the page's live badge.
    pub badge: Option<String>,
    /// The module component showing the page.
    pub component: String,
}

/// `[layouts]`: breakpoints between layout sizes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayouts {
    /// Available width, in logical units, below which `compact` applies.
    compact_below: f64,
    /// Below which `medium` applies; without it there is no `medium`.
    medium_below: Option<f64>,
    /// Below which `expanded` applies, `large` from it; without it there
    /// is no `large`.
    expanded_below: Option<f64>,
}

/// `[resources]`: files and directories, relative to `Bern.toml`, that ship
/// with the built application.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResources {
    paths: Vec<toml::Spanned<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLibraryRef {
    manifest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLibraryConfig {
    library: RawLibrary,
    languages: RawLanguages,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLibrary {
    package: String,
    sources: Vec<String>,
}

/// A component library crate, described by its own `Bern.toml`.
#[derive(Clone, Debug)]
pub struct LibraryConfig {
    pub dir: PathBuf,
    pub manifest: PathBuf,
    pub package: Package,
    pub version: String,
    pub sources: Vec<SourceFile>,
    pub languages: Vec<String>,
    pub default_language: String,
    pub strict: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApp {
    id: String,
    bundle_id: String,
    version: String,
    name: BTreeMap<String, String>,
    /// A square PNG, relative to `Bern.toml`; the Bern logo without it.
    icon: Option<toml::Spanned<String>>,
    /// Who publishes the application, for installers and packages.
    publisher: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLanguages {
    supported: Vec<String>,
    default: String,
    #[serde(default = "yes")]
    strict: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    mode: Option<String>,
    brand_seed: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProtocol {
    package: String,
    source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEngine {
    package: String,
    #[serde(rename = "type")]
    type_path: String,
    /// The package directory, relative to the manifest; by default the
    /// directory named like the package.
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUi {
    package: String,
    sources: RawSources,
    root: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTarget {
    platform: String,
    ui: Option<String>,
    web_mode: Option<String>,
    application_transport: Option<String>,
    application_codec: Option<String>,
    #[serde(default)]
    channels: Vec<String>,
    frontends: Option<Vec<String>>,
}

/// A Rust source file listed in the configuration.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// Path as written in `Bern.toml`, used in diagnostics.
    pub display: String,
    pub path: PathBuf,
    /// Module path inside its crate (`[]` for `lib.rs`).
    pub module: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub dir: PathBuf,
}

impl Package {
    /// Rust crate name of the package.
    pub fn crate_name(&self) -> String {
        self.name.replace('-', "_")
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub dir: PathBuf,
    pub app_id: String,
    pub bundle_id: String,
    pub version: String,
    pub name: BTreeMap<String, String>,
    /// `[app].icon`: a checked square PNG relative to `dir`.
    pub icon: Option<String>,
    /// `[app].publisher`, trimmed; `None` when absent or blank.
    pub publisher: Option<String>,
    pub languages: Vec<String>,
    pub default_language: String,
    pub strict: bool,
    pub theme_mode: ThemeMode,
    pub brand_seed: Option<Srgb>,
    pub protocol_package: Package,
    pub protocol_source: SourceFile,
    pub engine_package: Package,
    pub engine_type: String,
    pub ui_package: Package,
    pub ui_sources: Vec<SourceFile>,
    pub root: String,
    pub targets: Vec<TargetConfig>,
    /// Manifests of component libraries the UI uses.
    pub libraries: Vec<PathBuf>,
    /// The application's modules, in the order `Bern.toml` lists them.
    pub modules: Vec<ModuleConfig>,
    /// Files shipped with the application, directories expanded, sorted by
    /// name.
    pub resources: Vec<ResourceDef>,
    /// Breakpoints from `[layouts]`, if declared.
    pub layouts: Option<LayoutConfig>,
}

fn valid_language(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    (2..=3).contains(&language.len())
        && language.chars().all(|c| c.is_ascii_lowercase())
        && parts.all(|part| {
            (2..=8).contains(&part.len()) && part.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

/// The source files `sources` names, relative to `base` inside the project
/// `dir`: listed files as written, or every `.rs` file below a directory,
/// sorted by path.
fn expand_sources(dir: &Path, base: &str, sources: &RawSources) -> Result<Vec<String>, Diagnostic> {
    let join = |written: &str| {
        if base.is_empty() {
            written.to_string()
        } else {
            format!("{base}/{written}")
        }
    };
    match sources {
        RawSources::Files(files) => Ok(files.iter().map(|file| join(file)).collect()),
        RawSources::Directory(directory) if directory.ends_with(".rs") => Ok(vec![join(directory)]),
        RawSources::Directory(directory) => {
            let relative = join(directory.trim_end_matches('/'));
            let mut found = Vec::new();
            rust_files(&dir.join(&relative), &relative, &mut found)?;
            found.sort();
            if found.is_empty() {
                return Err(Diagnostic::error(format!(
                    "source directory `{relative}` holds no .rs files"
                )));
            }
            Ok(found)
        }
    }
}

fn rust_files(path: &Path, written: &str, found: &mut Vec<String>) -> Result<(), Diagnostic> {
    let entries = std::fs::read_dir(path).map_err(|error| {
        Diagnostic::error(format!(
            "cannot read source directory {}: {error}",
            path.display()
        ))
    })?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let child = format!("{written}/{name}");
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => rust_files(&entry.path(), &child, found)?,
            Ok(kind) if kind.is_file() && name.ends_with(".rs") => found.push(child),
            _ => {}
        }
    }
    Ok(())
}

/// Resolve `pkg/src/a/b.rs` into the package directory and module path.
fn source_file(dir: &Path, written: &str) -> Result<(PathBuf, Vec<String>), Diagnostic> {
    let relative = Path::new(written);
    let components: Vec<_> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();
    let src = components
        .iter()
        .position(|part| part == "src")
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "source `{written}` must be inside a package's `src` directory"
            ))
        })?;
    let package_dir = dir.join(components[..src].iter().collect::<PathBuf>());
    let mut module: Vec<String> = components[src + 1..].to_vec();
    let file = module
        .pop()
        .ok_or_else(|| Diagnostic::error(format!("source `{written}` does not name a file")))?;
    let stem = file
        .strip_suffix(".rs")
        .ok_or_else(|| Diagnostic::error(format!("source `{written}` must be a `.rs` file")))?;
    if !matches!(stem, "lib" | "main" | "mod") {
        module.push(stem.to_string());
    }
    if matches!(stem, "lib" | "main") && !module.is_empty() {
        return Err(Diagnostic::error(format!(
            "`{written}`: only the crate root may be named lib.rs or main.rs"
        )));
    }
    Ok((package_dir, module))
}

fn read_package_field(dir: &Path, field: &str) -> Result<String, Diagnostic> {
    let manifest = dir.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).map_err(|error| {
        Diagnostic::error(format!("cannot read {}: {error}", manifest.display()))
    })?;
    let value: toml::Value = text
        .parse()
        .map_err(|error| Diagnostic::error(format!("{}: {error}", manifest.display())))?;
    value
        .get("package")
        .and_then(|package| package.get(field))
        .and_then(|value| match value {
            toml::Value::String(text) => Some(text.clone()),
            // `field.workspace = true` inherits from the workspace manifest.
            toml::Value::Table(table) if table.get("workspace").is_some() => {
                workspace_package_field(dir, field)
            }
            _ => None,
        })
        .ok_or_else(|| Diagnostic::error(format!("{} has no package {field}", manifest.display())))
}

fn workspace_package_field(dir: &Path, field: &str) -> Option<String> {
    let mut directory = dir.parent();
    while let Some(current) = directory {
        if let Ok(text) = std::fs::read_to_string(current.join("Cargo.toml"))
            && let Ok(value) = text.parse::<toml::Value>()
            && let Some(found) = value
                .get("workspace")
                .and_then(|workspace| workspace.get("package"))
                .and_then(|package| package.get(field))
                .and_then(toml::Value::as_str)
        {
            return Some(found.to_string());
        }
        directory = current.parent();
    }
    None
}

fn read_package_name(dir: &Path) -> Result<String, Diagnostic> {
    let manifest = dir.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).map_err(|error| {
        Diagnostic::error(format!("cannot read {}: {error}", manifest.display()))
    })?;
    let value: toml::Value = text
        .parse()
        .map_err(|error| Diagnostic::error(format!("{}: {error}", manifest.display())))?;
    value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Diagnostic::error(format!("{} has no package name", manifest.display())))
}

fn package(
    dir: &Path,
    expected: &str,
    sources: &[(String, PathBuf)],
    what: &str,
) -> Result<Package, Diagnostic> {
    let package_dir = sources
        .first()
        .map(|(_, path)| path.clone())
        .unwrap_or_else(|| dir.join(expected));
    if sources.iter().any(|(_, path)| *path != package_dir) {
        return Err(Diagnostic::error(format!(
            "all {what} sources must belong to the package `{expected}`"
        )));
    }
    let name = read_package_name(&package_dir)?;
    if name != expected {
        return Err(Diagnostic::error(format!(
            "{what} sources are in package `{name}`, but Bern.toml names `{expected}`"
        )));
    }
    Ok(Package {
        name,
        dir: package_dir,
    })
}

/// Line and column (both from 1) of a byte offset in the manifest.
fn manifest_location(file: &str, text: &str, offset: usize) -> SourceLocation {
    let before = &text[..offset.min(text.len())];
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    SourceLocation {
        file: file.to_string(),
        line: before.matches('\n').count() as u32 + 1,
        column: before[line_start..].chars().count() as u32 + 1,
    }
}

/// Check `[app].icon`: a file inside the project, reached without symbolic
/// links, holding a square PNG of a size every platform scales down from.
fn app_icon(
    dir: &Path,
    manifest: &str,
    text: &str,
    entry: &toml::Spanned<String>,
    errors: &mut Vec<Diagnostic>,
) -> Option<String> {
    let written = entry.get_ref().as_str();
    let declared = manifest_location(manifest, text, entry.span().start);
    let error = |message: String| Diagnostic::error(message).at(declared.clone());
    if let Err(message) = bern_protocol::check_resource_name(written) {
        errors.push(error(format!("[app].icon: {message}")));
        return None;
    }
    let mut path = dir.to_path_buf();
    for segment in written.split('/') {
        path.push(segment);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                errors.push(error(format!(
                    "[app].icon `{written}`: {} is a symbolic link; name the file it points to",
                    path.display()
                )));
                return None;
            }
            Ok(_) => {}
            Err(_) => {
                errors.push(error(format!(
                    "[app].icon `{written}` does not exist: {}",
                    dir.join(written).display()
                )));
                return None;
            }
        }
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(failure) => {
            errors.push(error(format!(
                "[app].icon `{written}` cannot be read: {failure}"
            )));
            return None;
        }
    };
    if let Err(message) = bern_target_api::icon::check(&bytes) {
        errors.push(error(format!("[app].icon `{written}` {message}")));
        return None;
    }
    Some(written.to_string())
}

/// Check the `[resources]` entries and expand directories into the files
/// they ship. Every problem is an error at the entry that declared it.
fn resources(
    dir: &Path,
    manifest: &str,
    text: &str,
    raw: &RawResources,
    errors: &mut Vec<Diagnostic>,
) -> Vec<ResourceDef> {
    let mut found: Vec<ResourceDef> = Vec::new();
    for entry in &raw.paths {
        let written = entry.get_ref().as_str();
        let declared = manifest_location(manifest, text, entry.span().start);
        let error = |message: String| Diagnostic::error(message).at(declared.clone());
        if let Err(message) = bern_protocol::check_resource_name(written) {
            errors.push(error(message));
            continue;
        }
        // No part of the path may be a symbolic link: what ships is what
        // the entry names, inside the project directory.
        let mut path = dir.to_path_buf();
        let mut kind = None;
        for segment in written.split('/') {
            path.push(segment);
            match std::fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    kind = Some(Err(format!(
                        "resource `{written}`: {} is a symbolic link; list the file it points to",
                        path.display()
                    )));
                    break;
                }
                Ok(metadata) => kind = Some(Ok(metadata.file_type())),
                Err(_) => {
                    kind = Some(Err(format!(
                        "resource `{written}` does not exist: {}",
                        dir.join(written).display()
                    )));
                    break;
                }
            }
        }
        match kind {
            Some(Ok(file_type)) if file_type.is_file() => found.push(ResourceDef {
                name: written.to_string(),
                declared,
            }),
            Some(Ok(file_type)) if file_type.is_dir() => {
                let before = found.len();
                if let Err(message) = directory_files(&path, written, &declared, &mut found) {
                    errors.push(error(message));
                } else if found.len() == before {
                    errors.push(error(format!(
                        "resource directory `{written}` has no files to ship"
                    )));
                }
            }
            Some(Ok(_)) => errors.push(error(format!(
                "resource `{written}` is neither a file nor a directory"
            ))),
            Some(Err(message)) => errors.push(error(message)),
            None => {}
        }
    }
    // One name, one file: no duplicates, and no names a case-insensitive
    // file system would merge.
    let mut by_name: std::collections::BTreeMap<String, &ResourceDef> = Default::default();
    for resource in &found {
        match by_name.get(&resource.name.to_lowercase()) {
            Some(first) if first.name == resource.name => errors.push(
                Diagnostic::error(format!(
                    "resource `{}` is listed twice; it is already declared at line {}",
                    resource.name, first.declared.line
                ))
                .at(resource.declared.clone()),
            ),
            Some(first) => errors.push(
                Diagnostic::error(format!(
                    "resources `{}` and `{}` differ only in case",
                    first.name, resource.name
                ))
                .at(resource.declared.clone()),
            ),
            None => {
                by_name.insert(resource.name.to_lowercase(), resource);
            }
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found.dedup_by(|a, b| a.name == b.name);
    found
}

/// Every non-hidden file below `path`, named `prefix/relative/path`.
fn directory_files(
    path: &Path,
    prefix: &str,
    declared: &SourceLocation,
    found: &mut Vec<ResourceDef>,
) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
        .map_err(|error| format!("cannot read resource directory {}: {error}", path.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') {
            continue;
        }
        let name = format!("{prefix}/{file_name}");
        bern_protocol::check_resource_name(&name)?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot read {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "resource `{name}`: {} is a symbolic link; list the file it points to",
                entry.path().display()
            ));
        } else if file_type.is_dir() {
            directory_files(&entry.path(), &name, declared, found)?;
        } else if file_type.is_file() {
            found.push(ResourceDef {
                name,
                declared: declared.clone(),
            });
        } else {
            return Err(format!(
                "resource `{name}` is neither a file nor a directory"
            ));
        }
    }
    Ok(())
}

/// Load and validate a project configuration.
pub fn load(path: &Path) -> Result<Config, Vec<Diagnostic>> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read {}: {error}",
            path.display()
        ))]
    })?;
    let raw: RawConfig = toml::from_str(&text)
        .map_err(|error| vec![Diagnostic::error(format!("{}: {error}", path.display()))])?;
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = canonical(dir);
    let mut errors = Vec::new();

    if !bern_protocol::valid_name(&raw.app.id) {
        errors.push(Diagnostic::error(format!(
            "[app].id `{}` must be lower snake case",
            raw.app.id
        )));
    }
    let bundle_ok = raw.app.bundle_id.split('.').count() >= 2
        && raw.app.bundle_id.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if !bundle_ok {
        errors.push(Diagnostic::error(format!(
            "[app].bundle_id `{}` must be reverse-DNS, e.g. `com.example.app`",
            raw.app.bundle_id
        )));
    }
    let mut seen = std::collections::BTreeSet::new();
    for tag in &raw.languages.supported {
        if !valid_language(tag) {
            errors.push(Diagnostic::error(format!(
                "language tag `{tag}` is not a standard tag such as `zh-Hans` or `en`"
            )));
        }
        if !seen.insert(tag) {
            errors.push(Diagnostic::error(format!(
                "language `{tag}` is listed twice"
            )));
        }
    }
    if raw.languages.supported.is_empty() {
        errors.push(Diagnostic::error("[languages].supported is empty"));
    }
    if !raw.languages.supported.contains(&raw.languages.default) {
        errors.push(Diagnostic::error(format!(
            "default language `{}` is not in [languages].supported",
            raw.languages.default
        )));
    }
    for language in raw.app.name.keys() {
        if !raw.languages.supported.contains(language) {
            errors.push(Diagnostic::error(format!(
                "[app].name has language `{language}`, which is not supported"
            )));
        }
    }
    if raw.languages.strict {
        for language in &raw.languages.supported {
            if !raw.app.name.contains_key(language) {
                errors.push(
                    Diagnostic::error(format!(
                        "Missing translation\nProperty: [app].name\nMissing language: {language}"
                    ))
                    .code("B102"),
                );
            }
        }
    }
    let theme_mode = match raw.theme.mode.as_deref().unwrap_or("system") {
        "system" => ThemeMode::System,
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        other => {
            errors.push(Diagnostic::error(format!(
                "[theme].mode `{other}` must be system, light or dark"
            )));
            ThemeMode::System
        }
    };
    let layouts = match &raw.layouts {
        None => None,
        Some(layouts) if layouts.compact_below.is_finite() && layouts.compact_below > 0.0 => {
            // Each breakpoint lies beyond the ones before it.
            let mut previous = ("compact_below", layouts.compact_below);
            let mut ordered = true;
            for (name, value) in [
                ("medium_below", layouts.medium_below),
                ("expanded_below", layouts.expanded_below),
            ] {
                let Some(value) = value else { continue };
                if !value.is_finite() || value <= previous.1 {
                    errors.push(Diagnostic::error(format!(
                        "[layouts].{name} must be wider than {} ({}), not {value}",
                        previous.0, previous.1
                    )));
                    ordered = false;
                }
                previous = (name, value);
            }
            ordered.then_some(LayoutConfig {
                compact_below: layouts.compact_below,
                medium_below: layouts.medium_below,
                expanded_below: layouts.expanded_below,
            })
        }
        Some(layouts) => {
            errors.push(Diagnostic::error(format!(
                "[layouts].compact_below must be a positive width, not {}",
                layouts.compact_below
            )));
            None
        }
    };
    let brand_seed = match &raw.theme.brand_seed {
        None => None,
        Some(seed) => {
            let parsed = Srgb::parse(seed);
            if parsed.is_none() {
                errors.push(Diagnostic::error(format!(
                    "[theme].brand_seed `{seed}` must be a `#RRGGBB` color"
                )));
            }
            parsed
        }
    };

    let ui_written = expand_sources(&dir, "", &raw.ui.sources).unwrap_or_else(|error| {
        errors.push(error);
        Vec::new()
    });
    let mut resolve = |written: &str| -> Option<SourceFile> {
        match source_file(&dir, written) {
            Ok((_, module)) => Some(SourceFile {
                display: written.to_string(),
                path: dir.join(written),
                module,
            }),
            Err(error) => {
                errors.push(error);
                None
            }
        }
    };
    let protocol_source = resolve(&raw.protocol.source);
    let ui_sources: Vec<SourceFile> = ui_written.iter().filter_map(|s| resolve(s)).collect();

    let protocol_package = protocol_source.as_ref().and_then(|source| {
        let package_dir = source_file(&dir, &source.display).ok()?.0;
        package(
            &dir,
            &raw.protocol.package,
            &[(source.display.clone(), package_dir)],
            "protocol",
        )
        .map_err(|error| errors.push(error))
        .ok()
    });
    let ui_dirs: Vec<(String, PathBuf)> = ui_sources
        .iter()
        .filter_map(|source| {
            source_file(&dir, &source.display)
                .ok()
                .map(|(package_dir, _)| (source.display.clone(), package_dir))
        })
        .collect();
    let ui_package = package(&dir, &raw.ui.package, &ui_dirs, "UI")
        .map_err(|error| errors.push(error))
        .ok();
    let engine_dir = dir.join(raw.engine.path.as_deref().unwrap_or(&raw.engine.package));
    let engine_package = package(
        &dir,
        &raw.engine.package,
        &[(raw.engine.package.clone(), engine_dir)],
        "engine",
    )
    .map_err(|error| errors.push(error))
    .ok();
    if ui_written.is_empty() {
        errors.push(Diagnostic::error("[ui].sources lists no files"));
    }

    let mut targets = Vec::new();
    for (name, target) in raw.targets {
        if target.platform == "browser" {
            match target.web_mode.as_deref() {
                None => errors.push(Diagnostic::error(format!(
                    "target `{name}`: browser targets must set `web_mode = \"app\"` or `\"ui\"`"
                ))),
                Some("app") | Some("ui") => {}
                Some(other) => errors.push(Diagnostic::error(format!(
                    "target `{name}`: `web_mode = \"{other}\"` is unknown; use \"app\" or \"ui\""
                ))),
            }
            // `app` runs the engine in the page; `ui` reaches a separate
            // engine over a browser transport. Mismatches fail here
            // instead of building a page that cannot reach its engine.
            let (transport, codec) = match target.web_mode.as_deref() {
                Some("ui") => ("websocket", Some("json")),
                _ => ("in-process", None),
            };
            if let Some(declared) = &target.application_transport
                && declared != transport
            {
                errors.push(Diagnostic::error(format!(
                    "target `{name}`: `web_mode = \"{}\"` uses `application_transport = \"{transport}\"`, not `\"{declared}\"`",
                    target.web_mode.as_deref().unwrap_or("app")
                )));
            }
            if let Some(declared) = &target.application_codec
                && Some(declared.as_str()) != codec
            {
                errors.push(Diagnostic::error(format!(
                    "target `{name}`: `application_codec = \"{declared}\"` does not apply; {}",
                    match codec {
                        Some(codec) => format!("the WebUI channel uses `{codec}`"),
                        None => "the in-page engine is called directly".into(),
                    }
                )));
            }
        } else if target.web_mode.is_some() {
            errors.push(Diagnostic::error(format!(
                "target `{name}`: `web_mode` applies to browser targets only"
            )));
        }
        // Windows builds both front ends unless the target names fewer.
        let frontends = match (target.platform.as_str(), target.frontends) {
            ("windows", None) => WINDOWS_FRONTENDS.map(str::to_string).to_vec(),
            ("windows", Some(frontends)) => {
                if frontends.is_empty() {
                    errors.push(Diagnostic::error(format!(
                        "target `{name}`: `frontends` lists none; name \"winui3\", \"xaml-islands\" or both"
                    )));
                }
                for (index, frontend) in frontends.iter().enumerate() {
                    if !WINDOWS_FRONTENDS.contains(&frontend.as_str()) {
                        errors.push(Diagnostic::error(format!(
                            "target `{name}`: unknown front end `{frontend}`; use \"winui3\" or \"xaml-islands\""
                        )));
                    } else if frontends[..index].contains(frontend) {
                        errors.push(Diagnostic::error(format!(
                            "target `{name}`: `frontends` lists `{frontend}` twice"
                        )));
                    }
                }
                frontends
            }
            (_, Some(_)) => {
                errors.push(Diagnostic::error(format!(
                    "target `{name}`: `frontends` applies to Windows targets only"
                )));
                Vec::new()
            }
            (_, None) => Vec::new(),
        };
        for channel in &target.channels {
            if channel != "ipc" {
                errors.push(Diagnostic::error(format!(
                    "target `{name}`: unknown channel `{channel}`; supported: ipc"
                )));
            }
        }
        targets.push(TargetConfig {
            name,
            platform: target.platform,
            ui: target.ui,
            web_mode: target.web_mode,
            channels: target.channels,
            frontends,
        });
    }

    let libraries: Vec<PathBuf> = raw
        .libraries
        .iter()
        .map(|library| dir.join(&library.manifest))
        .collect();
    for library in &libraries {
        if !library.is_file() {
            errors.push(Diagnostic::error(format!(
                "library manifest {} does not exist",
                library.display()
            )));
        }
    }
    let mut modules: Vec<ModuleConfig> = Vec::new();
    for entry in &raw.modules {
        match load_module(&dir, &entry.path) {
            Ok(module) => {
                if modules.iter().any(|other| other.id == module.id) {
                    errors.push(Diagnostic::error(format!(
                        "module `{}` is listed twice",
                        module.id
                    )));
                }
                for language in &raw.languages.supported {
                    if !module.languages.contains(language) {
                        errors.push(Diagnostic::error(format!(
                            "module `{}` does not translate `{language}`, which the application supports; add it to {}",
                            module.id,
                            module.manifest.display()
                        )));
                    }
                }
                modules.push(module);
            }
            Err(module_errors) => errors.extend(module_errors),
        }
    }
    for (index, module) in modules.iter().enumerate() {
        for dependency in &module.depends {
            match modules.iter().position(|other| &other.id == dependency) {
                Some(position) if position < index && modules[position].optional && !module.optional => {
                    errors.push(Diagnostic::error(format!(
                        "module `{}` always runs but depends on the optional module `{dependency}`; make it optional too",
                        module.id
                    )))
                }
                Some(position) if position < index => {}
                Some(_) => errors.push(Diagnostic::error(format!(
                    "module `{}` depends on `{dependency}`, which Bern.toml lists after it; list modules after the ones they depend on",
                    module.id
                ))),
                None => errors.push(Diagnostic::error(format!(
                    "module `{}` depends on `{dependency}`, which is not a module of this application",
                    module.id
                ))),
            }
        }
    }
    let resources = raw
        .resources
        .as_ref()
        .map(|raw| resources(&dir, &path.display().to_string(), &text, raw, &mut errors))
        .unwrap_or_default();
    let icon =
        raw.app.icon.as_ref().and_then(|entry| {
            app_icon(&dir, &path.display().to_string(), &text, entry, &mut errors)
        });

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Config {
        dir,
        app_id: raw.app.id,
        bundle_id: raw.app.bundle_id,
        version: raw.app.version,
        name: raw.app.name,
        icon,
        publisher: raw
            .app
            .publisher
            .map(|publisher| publisher.trim().to_string())
            .filter(|publisher| !publisher.is_empty()),
        languages: raw.languages.supported,
        default_language: raw.languages.default,
        strict: raw.languages.strict,
        theme_mode,
        brand_seed,
        protocol_package: protocol_package.unwrap(),
        protocol_source: protocol_source.unwrap(),
        engine_package: engine_package.unwrap(),
        engine_type: raw.engine.type_path,
        ui_package: ui_package.unwrap(),
        ui_sources,
        root: raw.ui.root,
        targets,
        libraries,
        modules,
        resources,
        layouts,
    })
}

/// Load the module at `written` (relative to the project directory `dir`).
/// Its source paths are kept relative to `dir`, like the application's.
fn load_module(dir: &Path, written: &str) -> Result<ModuleConfig, Vec<Diagnostic>> {
    let base = written.trim_end_matches('/');
    let manifest = dir.join(base).join("Bern.toml");
    let text = std::fs::read_to_string(&manifest).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read module manifest {}: {error}",
            manifest.display()
        ))]
    })?;
    let raw: RawModuleConfig = toml::from_str(&text).map_err(|error| {
        vec![Diagnostic::error(format!(
            "{}: {error}",
            manifest.display()
        ))]
    })?;
    let mut errors = Vec::new();
    let id = raw.module.id;
    if !bern_protocol::valid_name(&id) {
        errors.push(Diagnostic::error(format!(
            "{}: module id `{id}` must be lower snake case",
            manifest.display()
        )));
    }
    let mut resolve = |written: &str| -> Option<SourceFile> {
        match source_file(dir, written) {
            Ok((_, module)) => Some(SourceFile {
                display: written.to_string(),
                path: dir.join(written),
                module,
            }),
            Err(error) => {
                errors.push(error);
                None
            }
        }
    };
    let protocol_source = resolve(&format!("{base}/{}", raw.module.protocol.source));
    let mut ui_sources = Vec::new();
    let mut ui_dirs = Vec::new();
    if let Some(ui) = &raw.module.ui {
        match expand_sources(dir, base, &ui.sources) {
            Ok(files) => {
                for file in files {
                    if let Ok((package_dir, _)) = source_file(dir, &file) {
                        ui_dirs.push((file.clone(), package_dir));
                    }
                    ui_sources.extend(resolve(&file));
                }
            }
            Err(error) => errors.push(error),
        }
    }
    let protocol_package = protocol_source.as_ref().and_then(|source| {
        let package_dir = source_file(dir, &source.display).ok()?.0;
        package(
            dir,
            &raw.module.protocol.package,
            &[(source.display.clone(), package_dir)],
            "protocol",
        )
        .map_err(|error| errors.push(error))
        .ok()
    });
    let ui_package = raw.module.ui.as_ref().and_then(|ui| {
        package(dir, &ui.package, &ui_dirs, "UI")
            .map_err(|error| errors.push(error))
            .ok()
    });
    let engine_dir = dir.join(base).join(
        raw.module
            .engine
            .path
            .as_deref()
            .unwrap_or(&raw.module.engine.package),
    );
    let engine_package = package(
        dir,
        &raw.module.engine.package,
        &[(raw.module.engine.package.clone(), engine_dir)],
        "engine",
    )
    .map_err(|error| errors.push(error))
    .ok();
    for page in &raw.pages {
        if let Some(badge) = &page.badge {
            if !bern_protocol::valid_name(badge) {
                errors.push(Diagnostic::error(format!(
                    "{}: page badge `{badge}` must name a lower snake case state field",
                    manifest.display()
                )));
            }
        }
        for (what, name) in [("slot", &page.slot), ("page id", &page.id)] {
            if !bern_protocol::valid_name(name) {
                errors.push(Diagnostic::error(format!(
                    "{}: {what} `{name}` must be lower snake case",
                    manifest.display()
                )));
            }
        }
        if raw.module.ui.is_none() {
            errors.push(Diagnostic::error(format!(
                "{}: page `{}` needs the module's [module.ui] to show it",
                manifest.display(),
                page.id
            )));
        }
    }
    for slot in raw.contributes.keys() {
        if !bern_protocol::valid_name(slot) {
            errors.push(Diagnostic::error(format!(
                "{}: slot `{slot}` must be lower snake case",
                manifest.display()
            )));
        }
    }
    if raw.module.enabled.is_some() && !raw.module.optional {
        errors.push(Diagnostic::error(format!(
            "{}: `enabled` applies to optional modules; add `optional = true` or remove it",
            manifest.display()
        )));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ModuleConfig {
        id,
        dir: canonical(dir.join(base)),
        manifest,
        name: raw.module.name,
        depends: raw.module.depends,
        optional: raw.module.optional,
        enabled: raw.module.enabled.unwrap_or(true),
        protocol_package: protocol_package.unwrap(),
        protocol_source: protocol_source.unwrap(),
        engine_package: engine_package.unwrap(),
        engine_type: raw.module.engine.type_path,
        ui_package,
        ui_sources,
        languages: raw.languages.supported,
        pages: raw
            .pages
            .into_iter()
            .map(|page| PageConfig {
                slot: page.slot,
                id: page.id,
                label: page.label,
                icon: page.icon,
                badge: page.badge,
                component: page.component,
            })
            .collect(),
        contributes: raw.contributes,
    })
}

/// Load a component library manifest (`[library]` + `[languages]`).
pub fn load_library(path: &Path) -> Result<LibraryConfig, Vec<Diagnostic>> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read {}: {error}",
            path.display()
        ))]
    })?;
    let raw: RawLibraryConfig = toml::from_str(&text)
        .map_err(|error| vec![Diagnostic::error(format!("{}: {error}", path.display()))])?;
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = canonical(dir);
    let mut errors = Vec::new();
    let mut sources = Vec::new();
    let mut dirs = Vec::new();
    for written in &raw.library.sources {
        match source_file(&dir, written) {
            Ok((package_dir, module)) => {
                dirs.push((written.clone(), package_dir));
                sources.push(SourceFile {
                    display: format!("{}/{written}", raw.library.package),
                    path: dir.join(written),
                    module,
                });
            }
            Err(error) => errors.push(error),
        }
    }
    for tag in &raw.languages.supported {
        if !valid_language(tag) {
            errors.push(Diagnostic::error(format!(
                "language tag `{tag}` is not a standard tag"
            )));
        }
    }
    let package = package(&dir, &raw.library.package, &dirs, "library")
        .map_err(|error| errors.push(error))
        .ok();
    let version = package.as_ref().and_then(|package| {
        read_package_field(&package.dir, "version")
            .map_err(|error| errors.push(error))
            .ok()
    });
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(LibraryConfig {
        dir,
        manifest: path.to_path_buf(),
        package: package.unwrap(),
        version: version.unwrap(),
        sources,
        languages: raw.languages.supported,
        default_language: raw.languages.default,
        strict: raw.languages.strict,
    })
}

/// The absolute form of `dir`, without the verbatim prefix Windows gives
/// canonical paths (`\\?\C:\…`), which Cargo manifests and tools reject.
fn canonical(dir: PathBuf) -> PathBuf {
    let Ok(path) = dir.canonicalize() else {
        return dir;
    };
    if cfg!(windows)
        && let Some(rest) = path.to_string_lossy().strip_prefix(r"\\?\")
        && !rest.starts_with(r"UNC\")
    {
        return PathBuf::from(rest);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags_are_validated() {
        for tag in ["zh-Hans", "en", "ja", "pt-BR", "zh-Hant-TW"] {
            assert!(valid_language(tag), "{tag}");
        }
        for tag in ["", "EN", "english", "zh_Hans", "zh-"] {
            assert!(!valid_language(tag), "{tag}");
        }
    }

    /// Check `[resources]` written as `entries` in a project holding
    /// `files` (paths relative to the project, created empty unless they
    /// end in `/`).
    fn check_resources(files: &[&str], entries: &str) -> (Vec<ResourceDef>, Vec<Diagnostic>) {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "bern-resources-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for file in files {
            let path = dir.join(file);
            if file.ends_with('/') {
                std::fs::create_dir_all(&path).unwrap();
            } else {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, b"x").unwrap();
            }
        }
        #[derive(Deserialize)]
        struct Manifest {
            resources: RawResources,
        }
        let text = format!("# Bern.toml\n[resources]\npaths = [\n{entries}\n]\n");
        let raw: Manifest = toml::from_str(&text).unwrap();
        let mut errors = Vec::new();
        let found = resources(&dir, "Bern.toml", &text, &raw.resources, &mut errors);
        let _ = std::fs::remove_dir_all(&dir);
        (found, errors)
    }

    fn names(found: &[ResourceDef]) -> Vec<&str> {
        found
            .iter()
            .map(|resource| resource.name.as_str())
            .collect()
    }

    #[test]
    fn resources_expand_directories_and_point_at_their_entries() {
        let (found, errors) = check_resources(
            &[
                "media/b.mp4",
                "data/seed.json",
                "data/nested/more.json",
                "data/.DS_Store",
                "data/.git/config",
            ],
            "  \"media/b.mp4\",\n  \"data\",",
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            names(&found),
            ["data/nested/more.json", "data/seed.json", "media/b.mp4"]
        );
        // Each file points at the entry that declared it: line 4 is the
        // first entry, line 5 the directory.
        let media = &found[2].declared;
        assert_eq!(
            (media.file.as_str(), media.line, media.column),
            ("Bern.toml", 4, 3)
        );
        assert_eq!(found[0].declared.line, 5);
    }

    #[test]
    fn resource_errors_are_strict_and_located() {
        let cases: &[(&[&str], &str, &str)] = &[
            (&[], "\"media/missing.mp4\"", "does not exist"),
            (&["a.mp4"], "\"/a.mp4\"", "is absolute"),
            (&["a.mp4"], "\"../a.mp4\"", "starts with `.`"),
            (&["a b.mp4"], "\"a b.mp4\"", "contains ` `"),
            (&["empty/"], "\"empty\"", "has no files to ship"),
            (&["only/.hidden"], "\"only\"", "has no files to ship"),
            (&["d/a b.txt"], "\"d\"", "contains ` `"),
            (&["a.mp4"], "\"a.mp4\", \"a.mp4\"", "listed twice"),
            (&["d/a.mp4"], "\"d\", \"d/a.mp4\"", "listed twice"),
            (&["d/A.mp4", "e/a.mp4"], "\"d/A.mp4\", \"e/a.mp4\"", ""),
        ];
        for (files, entries, expected) in cases {
            let (_, errors) = check_resources(files, entries);
            if expected.is_empty() {
                assert!(errors.is_empty(), "{entries}: {errors:?}");
                continue;
            }
            assert_eq!(errors.len(), 1, "{entries}: {errors:?}");
            assert!(
                errors[0].message.contains(expected),
                "{entries}: {}",
                errors[0].message
            );
            let location = errors[0].location.as_ref().expect("located");
            assert_eq!((location.file.as_str(), location.line), ("Bern.toml", 4));
        }
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_and_case_collisions_are_rejected() {
        let (_, errors) = check_resources(&["a.mp4", "A.MP4"], "\"a.mp4\", \"A.MP4\"");
        // A case-insensitive file system has one file; a case-sensitive one
        // has two that would merge on another machine. Both are errors.
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].message.contains("differ only in case")
                || errors[0].message.contains("listed twice"),
            "{}",
            errors[0].message
        );

        let dir = std::env::temp_dir().join(format!("bern-resources-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::fs::write(dir.join("real/a.mp4"), b"x").unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("linked")).unwrap();
        std::os::unix::fs::symlink(dir.join("real/a.mp4"), dir.join("real/b.mp4")).unwrap();
        #[derive(Deserialize)]
        struct Manifest {
            resources: RawResources,
        }
        for entry in ["linked/a.mp4", "real"] {
            let text = format!("[resources]\npaths = [\"{entry}\"]\n");
            let raw: Manifest = toml::from_str(&text).unwrap();
            let mut errors = Vec::new();
            resources(&dir, "Bern.toml", &text, &raw.resources, &mut errors);
            assert_eq!(errors.len(), 1, "{entry}: {errors:?}");
            assert!(
                errors[0].message.contains("symbolic link"),
                "{}",
                errors[0].message
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_paths_follow_cargo_layout() {
        let dir = Path::new("/p");
        assert_eq!(
            source_file(dir, "ui/src/lib.rs").unwrap().1,
            Vec::<String>::new()
        );
        assert_eq!(
            source_file(dir, "ui/src/pages.rs").unwrap().1,
            vec!["pages"]
        );
        assert_eq!(
            source_file(dir, "ui/src/a/b/mod.rs").unwrap().1,
            vec!["a", "b"]
        );
        assert_eq!(
            source_file(dir, "ui/src/pages.rs").unwrap().0,
            PathBuf::from("/p/ui")
        );
        assert!(source_file(dir, "ui/pages.rs").is_err());
    }

    /// Check `[app].icon = entry` in a project holding `files`.
    fn check_icon(files: &[(&str, Vec<u8>)], entry: &str) -> (Option<String>, Vec<Diagnostic>) {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "bern-icon-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, bytes) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
        }
        #[derive(Deserialize)]
        struct Manifest {
            icon: toml::Spanned<String>,
        }
        let text = format!("# Bern.toml\nicon = \"{entry}\"\n");
        let raw: Manifest = toml::from_str(&text).unwrap();
        let mut errors = Vec::new();
        let found = app_icon(&dir, "Bern.toml", &text, &raw.icon, &mut errors);
        let _ = std::fs::remove_dir_all(&dir);
        (found, errors)
    }

    /// The first bytes of a PNG of `width` × `height` pixels.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn app_icons_are_square_pngs_in_the_project() {
        let (found, errors) = check_icon(&[("art/icon.png", png(1024, 1024))], "art/icon.png");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(found.as_deref(), Some("art/icon.png"));
        for (files, entry, expected) in [
            (vec![], "icon.png", "does not exist"),
            (vec![("icon.png", png(300, 300))], "icon.png", "1024"),
            (vec![("icon.png", png(512, 256))], "icon.png", "square"),
            (
                vec![("icon.png", b"GIF89a".to_vec())],
                "icon.png",
                "not a PNG",
            ),
            (vec![], "../icon.png", "[app].icon"),
        ] {
            let (found, errors) = check_icon(&files, entry);
            assert!(found.is_none(), "{entry}");
            assert_eq!(errors.len(), 1, "{entry}: {errors:?}");
            assert!(
                errors[0].message.contains(expected),
                "{entry}: {}",
                errors[0].message
            );
            assert_eq!(errors[0].location.as_ref().map(|at| at.line), Some(2));
        }
    }
}
