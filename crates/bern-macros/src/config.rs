//! Project language configuration for early localization checks.
//!
//! The build tool performs the authoritative checks; reading `Bern.toml`
//! here lets missing translations fail inside the editor with an exact
//! span. The file is tracked with `include_bytes!` so edits re-run macros.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Languages {
    pub supported: Vec<String>,
    pub strict: bool,
    pub path: PathBuf,
}

/// Find `Bern.toml` in the crate directory or an ancestor.
pub fn languages() -> Result<Option<Languages>, String> {
    let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") else {
        return Ok(None);
    };
    let mut directory = Some(PathBuf::from(manifest));
    while let Some(current) = directory {
        let candidate = current.join("Bern.toml");
        if candidate.is_file() {
            return parse(candidate).map(Some);
        }
        directory = current.parent().map(PathBuf::from);
    }
    Ok(None)
}

fn parse(path: PathBuf) -> Result<Languages, String> {
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let value: toml::Value = text
        .parse()
        .map_err(|error| format!("{} is not valid TOML: {error}", path.display()))?;
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
