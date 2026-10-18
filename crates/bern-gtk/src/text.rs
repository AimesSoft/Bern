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
    tags.iter().filter_map(|tag| locale_tag(tag)).collect()
}

/// The language tag of a locale variable's value; none for the C and
/// POSIX locales.
fn locale_tag(value: &str) -> Option<String> {
    // zh_CN.UTF-8@variant → zh-CN
    let tag = value.split(['.', '@']).next().unwrap_or_default();
    let tag = tag.replace('_', "-");
    (!tag.is_empty() && tag != "C" && tag != "POSIX").then_some(tag)
}

/// The supported language that best matches what the user asks for.
pub(crate) fn choose(supported: &[String], fallback: &str) -> String {
    best(supported, &wanted(), fallback)
}

/// The supported language that best matches `wanted`, most preferred
/// first.
fn best(supported: &[String], wanted: &[String], fallback: &str) -> String {
    for tag in wanted {
        if let Some(exact) = supported
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(tag))
        {
            return exact.clone();
        }
    }
    for tag in wanted {
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
    use crate::reactive::{Scope, effect};
    use std::cell::RefCell;

    const STRINGS: StringTable = &[
        ("en", &[("greeting", "Hello, {name}"), ("title", "Inbox")]),
        (
            "fr",
            &[
                ("greeting", "Bonjour, {name}"),
                ("title", "Boîte de réception"),
            ],
        ),
    ];

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|tag| tag.to_string()).collect()
    }

    #[test]
    fn placeholders_fill_and_escape() {
        assert_eq!(
            fill("按钮共按下 {count} 次", &[("count", "3".into())]),
            "按钮共按下 3 次"
        );
        assert_eq!(fill("{{x}} {y}", &[("y", "1".into())]), "{x} 1");
        assert_eq!(fill("{missing}!", &[]), "!");
    }

    #[test]
    fn placeholders_repeat_and_values_stay_as_given() {
        let args = [("name", "Ada".to_string()), ("other", "{name}".to_string())];
        assert_eq!(fill("{name}, {name}!", &args), "Ada, Ada!");
        // A value is not read as a template again.
        assert_eq!(fill("<{other}>", &args), "<{name}>");
        // A lone closing brace is text.
        assert_eq!(fill("a } b", &args), "a } b");
        assert_eq!(fill("", &args), "");
    }

    #[test]
    fn locale_variables_become_language_tags() {
        assert_eq!(locale_tag("zh_CN.UTF-8@pinyin").as_deref(), Some("zh-CN"));
        assert_eq!(locale_tag("en_US.UTF-8").as_deref(), Some("en-US"));
        assert_eq!(locale_tag("fr").as_deref(), Some("fr"));
        assert_eq!(locale_tag("C.UTF-8"), None);
        assert_eq!(locale_tag("POSIX"), None);
        assert_eq!(locale_tag(".UTF-8"), None);
    }

    #[test]
    fn exact_languages_match_in_any_case() {
        let supported = tags(&["en", "zh-Hans", "pt-BR"]);
        assert_eq!(best(&supported, &tags(&["pt-br"]), "en"), "pt-BR");
        assert_eq!(best(&supported, &tags(&["ZH-HANS"]), "en"), "zh-Hans");
    }

    #[test]
    fn regions_fall_back_to_their_language() {
        let supported = tags(&["en", "fr", "pt-BR"]);
        assert_eq!(best(&supported, &tags(&["fr-CA"]), "en"), "fr");
        assert_eq!(best(&supported, &tags(&["pt-PT"]), "en"), "pt-BR");
        assert_eq!(best(&supported, &tags(&["de-DE", "fr-FR"]), "en"), "fr");
        assert_eq!(best(&supported, &tags(&["ja-JP"]), "en"), "en");
        assert_eq!(best(&supported, &[], "en"), "en");
    }

    #[test]
    fn chinese_regions_choose_their_script() {
        let supported = tags(&["en", "zh-Hans", "zh-Hant"]);
        for (tag, script) in [
            ("zh", "zh-Hans"),
            ("zh-CN", "zh-Hans"),
            ("zh-SG", "zh-Hans"),
            ("zh-TW", "zh-Hant"),
            ("zh-HK", "zh-Hant"),
            ("zh-MO", "zh-Hant"),
            ("zh-Hant-TW", "zh-Hant"),
        ] {
            assert_eq!(best(&supported, &tags(&[tag]), "en"), script, "{tag}");
        }
        // Readers of Traditional Chinese do not get Simplified text.
        assert_eq!(
            best(&tags(&["en", "zh-Hans"]), &tags(&["zh-TW"]), "en"),
            "en"
        );
    }

    #[test]
    fn switching_language_shows_its_text_in_one_run() {
        install(STRINGS, &["en", "fr"], "en");
        assert!(set_language("en"));
        let shown = Rc::new(RefCell::new(Vec::new()));
        let scope = Scope::detached();
        let seen = Rc::clone(&shown);
        scope.run(|| {
            effect(move || {
                seen.borrow_mut()
                    .push(format!("{}: {}", language(), tr("title")))
            })
        });
        assert!(set_language("fr"));
        assert!(!set_language("de"));
        assert_eq!(*shown.borrow(), ["en: Inbox", "fr: Boîte de réception"]);
        scope.dispose();
    }

    #[test]
    fn translations_fill_placeholders_and_fall_back_to_the_key() {
        install(STRINGS, &["en", "fr", "de"], "en");
        assert!(set_language("fr"));
        assert_eq!(fmt("greeting", &[("name", "Ada".into())]), "Bonjour, Ada");
        assert_eq!(tr("missing.key"), "missing.key");
        // A supported language without strings shows the keys.
        assert!(set_language("de"));
        assert_eq!(tr("title"), "title");
    }

    #[test]
    fn languages_name_themselves() {
        assert_eq!(autonym("zh-Hant"), "繁體中文");
        assert_eq!(autonym("de"), "Deutsch");
        assert_eq!(autonym("x-klingon"), "x-klingon");
    }
}
