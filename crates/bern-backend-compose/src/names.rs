//! Kotlin names and literals for schema names and values.

/// Kotlin's hard keywords, which names escape with backticks.
const KEYWORDS: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
];

fn escape(name: String) -> String {
    if KEYWORDS.contains(&name.as_str()) {
        format!("`{name}`")
    } else {
        name
    }
}

/// `snake_case` as a Kotlin member, `lowerCamelCase`.
pub fn member(snake: &str) -> String {
    escape(bern_target_api::names::camel(snake))
}

/// `snake_case` as a Kotlin type, `UpperCamelCase`.
pub use bern_target_api::names::pascal;

/// A Kotlin string literal; `$` is escaped so nothing interpolates.
pub fn string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A Kotlin `Double` literal.
pub fn double(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.1}")
    } else if value.is_finite() {
        format!("{value:?}")
    } else if value.is_nan() {
        "Double.NaN".into()
    } else if value > 0.0 {
        "Double.POSITIVE_INFINITY".into()
    } else {
        "Double.NEGATIVE_INFINITY".into()
    }
}

/// A Kotlin `Float` literal.
pub fn float(value: f64) -> String {
    format!("{}f", double(value).trim_end_matches(".0"))
}

/// The Kotlin package of an application: its bundle id, with characters
/// Java packages cannot hold replaced.
pub fn package(bundle_id: &str) -> String {
    bundle_id
        .split('.')
        .map(|part| {
            let mut part: String = part
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            if part.is_empty() || part.starts_with(|c: char| c.is_ascii_digit()) {
                part.insert(0, '_');
            }
            if KEYWORDS.contains(&part.as_str()) {
                part.push('_');
            }
            part
        })
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_literals() {
        assert_eq!(member("large_title"), "largeTitle");
        assert_eq!(member("in"), "`in`");
        assert_eq!(pascal("main_window"), "MainWindow");
        assert_eq!(string("a\"$b"), "\"a\\\"\\$b\"");
        assert_eq!(double(2.0), "2.0");
        assert_eq!(float(2.0), "2f");
        assert_eq!(float(0.5), "0.5f");
        assert_eq!(package("dev.bern.my-app"), "dev.bern.my_app");
    }
}
