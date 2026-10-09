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

    /// Interpolation names referenced by one translation, in order of first use.
    pub fn placeholders(text: &str) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        let mut chars = text.char_indices().peekable();
        while let Some((index, c)) = chars.next() {
            match c {
                '{' if matches!(chars.peek(), Some((_, '{'))) => {
                    chars.next();
                }
                '}' if matches!(chars.peek(), Some((_, '}'))) => {
                    chars.next();
                }
                '{' => {
                    let start = index + 1;
                    let mut end = None;
                    for (position, next) in chars.by_ref() {
                        if next == '}' {
                            end = Some(position);
                            break;
                        }
                    }
                    let end = end.ok_or_else(|| format!("unclosed `{{` in \"{text}\""))?;
                    let name = &text[start..end];
                    if name.is_empty()
                        || !name
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                        || name.starts_with(|c: char| c.is_ascii_digit())
                    {
                        return Err(format!("invalid placeholder `{{{name}}}` in \"{text}\""));
                    }
                    if !names.iter().any(|existing| existing == name) {
                        names.push(name.to_string());
                    }
                }
                '}' => return Err(format!("unmatched `}}` in \"{text}\"")),
                _ => {}
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
