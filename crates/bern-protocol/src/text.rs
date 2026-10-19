use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A user-visible text declared together with all of its translations.
///
/// `key` is assigned by the compiler and traces back to the declaration.
/// `params` lists interpolation names such as `count` in `{count}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalizedText {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    pub translations: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<String>,
}

/// Resource key of an enum variant label.
pub fn label_key(enum_name: &str, variant: &str) -> String {
    format!("{enum_name}.{variant}")
}

impl LocalizedText {
    pub fn new<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self {
            key: String::new(),
            translations: pairs
                .into_iter()
                .map(|(language, text)| (language.to_string(), text.to_string()))
                .collect(),
            params: Vec::new(),
        }
    }

    pub fn get(&self, language: &str) -> Option<&str> {
        self.translations.get(language).map(String::as_str)
    }

    /// Interpolation names referenced by one translation, in order of first
    /// use. The text is checked as a whole: see [`crate::format`].
    pub fn placeholders(text: &str) -> Result<Vec<String>, String> {
        let pieces = crate::format::parse(text)?;
        let mut names: Vec<String> = Vec::new();
        for (name, _) in crate::format::uses(&pieces) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_parsed_and_braces_escape() {
        assert_eq!(
            LocalizedText::placeholders("已播放 {count} 集 {{x}}").unwrap(),
            vec!["count"]
        );
        assert!(LocalizedText::placeholders("{Count}").is_err());
        assert!(LocalizedText::placeholders("{count").is_err());
        assert!(LocalizedText::placeholders("a } b").is_err());
    }
}
