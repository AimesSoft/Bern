//! `Bern.toml`: project, language, theme, source and target configuration.

use bern_ir::{Srgb, ThemeMode};
use bern_target_api::{Diagnostic, TargetConfig};
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApp {
    id: String,
    bundle_id: String,
    version: String,
    name: BTreeMap<String, String>,
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUi {
    package: String,
    sources: Vec<String>,
    root: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTarget {
    platform: String,
    ui: Option<String>,
    web_mode: Option<String>,
    #[serde(default)]
    channels: Vec<String>,
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
    let dir = dir.canonicalize().unwrap_or(dir);
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
    let ui_sources: Vec<SourceFile> = raw.ui.sources.iter().filter_map(|s| resolve(s)).collect();

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
    let engine_package = package(&dir, &raw.engine.package, &[], "engine")
        .map_err(|error| errors.push(error))
        .ok();
    if raw.ui.sources.is_empty() {
        errors.push(Diagnostic::error("[ui].sources lists no files"));
    }

    let mut targets = Vec::new();
    for (name, target) in raw.targets {
        if target.platform == "browser" && target.web_mode.is_none() {
            errors.push(Diagnostic::error(format!(
                "target `{name}`: browser targets must set `web_mode = \"app\"` or `\"ui\"`"
            )));
        }
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
            channels: target.channels,
        });
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Config {
        dir,
        app_id: raw.app.id,
        bundle_id: raw.app.bundle_id,
        version: raw.app.version,
        name: raw.app.name,
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
    })
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
}
