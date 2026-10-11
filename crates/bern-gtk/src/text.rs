//! Localized text and the session's interface language.
//!
//! The language and its strings are cells: switching language re-runs
//! every binding that shows text, in one update, while drafts, navigation
//! and windows stay.

use crate::reactive::{Cell, batch};
use std::collections::HashMap;
use std::rc::Rc;

/// Strings of every supported language: `(language, [(key, text)])`.
pub type StringTable = &'static [(&'static str, &'static [(&'static str, &'static str)])];

pub(crate) struct Language {
    pub(crate) current: Cell<String>,
    strings: Cell<Rc<HashMap<&'static str, &'static str>>>,
    pub(crate) supported: Vec<String>,
    table: StringTable,
}

thread_local! {
    static LANGUAGE: std::cell::OnceCell<Language> = const { std::cell::OnceCell::new() };
}

fn strings_of(table: StringTable, language: &str) -> Rc<HashMap<&'static str, &'static str>> {
    Rc::new(
        table
            .iter()
            .find(|(tag, _)| *tag == language)
            .map(|(_, strings)| strings.iter().copied().collect())
            .unwrap_or_default(),
    )
}

/// The language tags the process asks for, most preferred first, from the
/// standard locale variables.
fn wanted() -> Vec<String> {
    let mut tags = Vec::new();
    if let Ok(list) = std::env::var("LANGUAGE") {
        tags.extend(
            list.split(':')
                .filter(|tag| !tag.is_empty())
                .map(String::from),
        );
    }
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name)
            && !value.is_empty()
        {
            tags.push(value);
            break;
        }
    }
    tags.into_iter()
        .map(|tag| {
            // zh_CN.UTF-8@variant → zh-CN
            let tag = tag.split(['.', '@']).next().unwrap_or_default();
            tag.replace('_', "-")
        })
        .filter(|tag| !tag.is_empty() && tag != "C" && tag != "POSIX")
        .collect()
}

/// The supported language that best matches what the user asks for.
pub(crate) fn choose(supported: &[String], fallback: &str) -> String {
    let wanted = wanted();
    for tag in &wanted {
        if let Some(exact) = supported
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(tag))
        {
            return exact.clone();
        }
    }
    for tag in &wanted {
        let lower = tag.to_ascii_lowercase();
        // zh-CN, zh-SG → zh-Hans; zh-TW, zh-HK → zh-Hant.
        let script = if ["zh-tw", "zh-hk", "zh-mo", "zh-hant"]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            "zh-hant".to_string()
        } else if lower.starts_with("zh") {
            "zh-hans".to_string()
        } else {
            lower.split('-').next().unwrap_or_default().to_string()
        };
        let found = supported
            .iter()
            .find(|candidate| candidate.to_ascii_lowercase() == script)
            .or_else(|| {
                supported.iter().find(|candidate| {
                    candidate.to_ascii_lowercase().split('-').next() == Some(script.as_str())
                })
            });
        if let Some(found) = found {
            return found.clone();
        }
    }
    fallback.to_string()
}

pub(crate) fn install(table: StringTable, supported: &[&str], fallback: &str) {
    let supported: Vec<String> = supported.iter().map(|tag| tag.to_string()).collect();
    let language = choose(&supported, fallback);
    LANGUAGE.with(|cell| {
        let _ = cell.set(Language {
            strings: Cell::new(strings_of(table, &language)),
            current: Cell::new(language),
            supported,
            table,
        });
    });
}

pub(crate) fn with<R>(read: impl FnOnce(&Language) -> R) -> R {
    LANGUAGE.with(|cell| read(cell.get().expect("language installed")))
}

/// The current language tag, recording a dependency.
pub fn language() -> String {
    with(|language| language.current.get())
}

/// Switch the interface language. Returns false for unsupported tags.
pub(crate) fn set_language(tag: &str) -> bool {
    with(|language| {
        if !language.supported.iter().any(|supported| supported == tag) {
            return false;
        }
        batch(|| {
            language.current.set(tag.to_string());
            language.strings.set(strings_of(language.table, tag));
        });
        true
    })
}

/// The translation of `key` in the current language.
pub fn tr(key: &str) -> String {
    with(|language| {
        language
            .strings
            .with(|strings| strings.get(key).map(|text| text.to_string()))
            .unwrap_or_else(|| key.to_string())
    })
}

/// A translation with `{name}` placeholders filled from `args`.
pub fn fmt(key: &str, args: &[(&str, String)]) -> String {
    fill(&tr(key), args)
}

pub(crate) fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut name = String::new();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    name.push(next);
                }
                if let Some((_, value)) = args.iter().find(|(arg, _)| *arg == name) {
                    out.push_str(value);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// A language's own name for itself, such as “简体中文” or “English”.
pub fn autonym(tag: &str) -> String {
    match tag {
        "zh-Hans" => "简体中文",
        "zh-Hant" => "繁體中文",
        "en" => "English",
        "ja" => "日本語",
        "ko" => "한국어",
        "fr" => "Français",
        "de" => "Deutsch",
        "es" => "Español",
        "it" => "Italiano",
        "pt" => "Português",
        "ru" => "Русский",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_fill_and_escape() {
        assert_eq!(
            fill("按钮共按下 {count} 次", &[("count", "3".into())]),
            "按钮共按下 3 次"
        );
        assert_eq!(fill("{{x}} {y}", &[("y", "1".into())]), "{x} 1");
        assert_eq!(fill("{missing}!", &[]), "!");
    }
}
