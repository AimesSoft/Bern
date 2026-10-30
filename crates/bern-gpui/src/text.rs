//! Localized text and the interface language: chosen from the system's
//! preferred languages, and switched while the application runs.

use crate::protocol::format::{self, Arg, Piece};
use crate::protocol::{Type, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Strings of every supported language: `(language, [(key, text)])`.
pub type StringTable = &'static [(&'static str, &'static [(&'static str, &'static str)])];

struct Language {
    current: String,
    strings: HashMap<&'static str, &'static str>,
    supported: Vec<&'static str>,
    table: StringTable,
}

thread_local! {
    static LANGUAGE: RefCell<Option<Language>> = const { RefCell::new(None) };
    static PARSED: RefCell<HashMap<String, Rc<Vec<Piece>>>> = RefCell::new(HashMap::new());
}

/// The supported language that best matches `wanted`, most preferred
/// first: an earlier tag wins over a closer match of a later one.
fn best(supported: &[&str], wanted: &[String], fallback: &str) -> String {
    wanted
        .iter()
        .find_map(|tag| matching(supported, tag))
        .unwrap_or(fallback)
        .to_string()
}

/// The supported language for one wanted tag: the tag itself in any case,
/// else one of its language, Chinese by its script.
fn matching<'a>(supported: &[&'a str], tag: &str) -> Option<&'a str> {
    let tag = tag.replace('_', "-");
    if let Some(exact) = supported
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(&tag))
    {
        return Some(exact);
    }
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
    supported
        .iter()
        .find(|candidate| candidate.to_ascii_lowercase() == script)
        .or_else(|| {
            supported.iter().find(|candidate| {
                candidate.to_ascii_lowercase().split('-').next() == Some(script.as_str())
            })
        })
        .copied()
}

fn strings_of(table: StringTable, language: &str) -> HashMap<&'static str, &'static str> {
    table
        .iter()
        .find(|(tag, _)| *tag == language)
        .map(|(_, strings)| strings.iter().copied().collect())
        .unwrap_or_default()
}

pub(crate) fn install(table: StringTable, supported: &'static [&'static str], fallback: &str) {
    let wanted: Vec<String> = sys_locale::get_locales().collect();
    let current = best(supported, &wanted, fallback);
    LANGUAGE.with(|cell| {
        *cell.borrow_mut() = Some(Language {
            strings: strings_of(table, &current),
            current,
            supported: supported.to_vec(),
            table,
        });
    });
}

fn with<R>(read: impl FnOnce(&Language) -> R) -> R {
    LANGUAGE.with(|cell| read(cell.borrow().as_ref().expect("language installed")))
}

/// The current language tag.
pub fn language() -> String {
    with(|language| language.current.clone())
}

/// The supported languages, in the application's order.
pub fn supported() -> Vec<&'static str> {
    with(|language| language.supported.clone())
}

/// Switch the interface language; every window shows it at once, while
/// drafts, navigation and windows stay. False for unsupported tags.
pub fn set_language(tag: &str) -> bool {
    let switched = LANGUAGE.with(|cell| {
        let mut cell = cell.borrow_mut();
        let Some(language) = cell.as_mut() else {
            return false;
        };
        if !language.supported.contains(&tag) {
            return false;
        }
        language.current = tag.to_string();
        language.strings = strings_of(language.table, tag);
        true
    });
    if switched {
        crate::host::redraw();
    }
    switched
}

/// The translation of `key` in the current language.
pub fn tr(key: &str) -> String {
    with(|language| {
        language
            .strings
            .get(key)
            .map(|text| text.to_string())
            .unwrap_or_else(|| key.to_string())
    })
}

/// A translation with its arguments, in the current language: text and
/// numbers as they are, plural branches by the number's category, dates
/// and durations by their patterns (`bern_protocol::format`).
pub fn fmt(key: &str, args: &[(&str, Arg)]) -> String {
    let pieces = parsed(&tr(key));
    format::format(
        &pieces,
        &language(),
        &|name| {
            args.iter()
                .find(|(arg, _)| *arg == name)
                .map(|(_, value)| value.clone())
        },
        &|key| tr(key),
    )
}

/// A translation's pieces, parsed once. The compiler checked the format,
/// so a text that does not parse is shown as it is.
fn parsed(text: &str) -> Rc<Vec<Piece>> {
    PARSED.with(|cache| {
        Rc::clone(
            cache
                .borrow_mut()
                .entry(text.to_string())
                .or_insert_with(|| {
                    Rc::new(format::parse(text).unwrap_or_else(|_| vec![Piece::Text(text.into())]))
                }),
        )
    })
}

/// A value as a text argument: numbers keep their value for plural
/// branches, enums are their labels.
pub fn arg(value: &Value, enum_name: Option<&str>) -> Arg {
    match value {
        Value::Int(number) => Arg::Number {
            value: *number as f64,
            text: number.to_string(),
        },
        Value::Float(number) => Arg::Number {
            value: *number,
            text: crate::value::number(*number),
        },
        value => Arg::Text(display(value, enum_name)),
    }
}

/// A `CalendarDate` as a text argument.
pub fn date(value: &Value) -> Arg {
    let part = |fields: &[Value], at: usize| match fields.get(at) {
        Some(Value::Int(number)) => *number,
        _ => 0,
    };
    match value {
        Value::Record(fields) => Arg::Date {
            year: part(fields, 0),
            month: part(fields, 1),
            day: part(fields, 2),
        },
        _ => Arg::Text(String::new()),
    }
}

/// The text of a value: an enum's label, anything else as plain text.
pub fn display(value: &Value, enum_name: Option<&str>) -> String {
    match (value, enum_name) {
        (Value::Enum { variant, fields }, Some(name)) => label_of(name, *variant, fields),
        (value, _) => crate::value::plain(value),
    }
}

/// The localized label of a variant without fields.
pub fn label(enum_name: &str, variant: u32) -> String {
    label_of(enum_name, variant, &[])
}

/// The localized label of an enum value; a label that shows the variant's
/// fields writes them as its text says. A variant without a label shows
/// its name.
pub fn label_of(enum_name: &str, variant: u32, fields: &[Value]) -> String {
    let host = crate::host::host();
    let Some(def) = host.def.types.enum_def(enum_name) else {
        return String::new();
    };
    let Some(variant) = def.variants.get(variant as usize) else {
        return String::new();
    };
    let Some(label) = &variant.label else {
        return variant.name.clone();
    };
    let key = crate::protocol::label_key(enum_name, &variant.name);
    if label.params.is_empty() {
        return tr(&key);
    }
    let args: Vec<(&str, Arg)> = label
        .params
        .iter()
        .filter_map(|param| {
            let at = variant
                .fields
                .iter()
                .position(|field| field.name == *param)?;
            let value = fields.get(at)?;
            let arg = match &variant.fields[at].ty {
                Type::Record { name } if name == format::CALENDAR_DATE => date(value),
                Type::Enum { name } => arg(value, Some(name)),
                _ => arg(value, None),
            };
            Some((param.as_str(), arg))
        })
        .collect();
    fmt(&key, &args)
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
    use super::best;

    #[test]
    fn the_first_wanted_language_that_is_supported_wins() {
        let supported = ["zh-Hans", "en"];
        let wanted = |tags: &[&str]| tags.iter().map(|tag| tag.to_string()).collect::<Vec<_>>();
        assert_eq!(
            best(&supported, &wanted(&["zh-Hans-CN", "en-US"]), "en"),
            "zh-Hans"
        );
        assert_eq!(best(&supported, &wanted(&["zh_CN"]), "en"), "zh-Hans");
        assert_eq!(
            best(&supported, &wanted(&["de-DE", "en-GB"]), "zh-Hans"),
            "en"
        );
        assert_eq!(best(&supported, &wanted(&["fr"]), "zh-Hans"), "zh-Hans");
    }
}
