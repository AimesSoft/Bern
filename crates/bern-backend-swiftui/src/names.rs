//! Swift spellings of Bern names and literals.

const KEYWORDS: &[&str] = &[
    "associatedtype",
    "class",
    "deinit",
    "enum",
    "extension",
    "fileprivate",
    "func",
    "import",
    "init",
    "inout",
    "internal",
    "let",
    "open",
    "operator",
    "private",
    "precedencegroup",
    "protocol",
    "public",
    "rethrows",
    "static",
    "struct",
    "subscript",
    "typealias",
    "var",
    "break",
    "case",
    "catch",
    "continue",
    "default",
    "defer",
    "do",
    "else",
    "fallthrough",
    "for",
    "guard",
    "if",
    "in",
    "repeat",
    "return",
    "throw",
    "switch",
    "where",
    "while",
    "Any",
    "as",
    "await",
    "false",
    "is",
    "nil",
    "self",
    "Self",
    "super",
    "throws",
    "true",
    "try",
    "some",
    "any",
    "async",
    "actor",
    "consume",
    "borrowing",
    "consuming",
    "package",
    "Type",
];

fn escape(name: String) -> String {
    if KEYWORDS.contains(&name.as_str()) {
        format!("`{name}`")
    } else {
        name
    }
}

/// `last_action` → `lastAction`, escaped when it is a keyword.
pub fn member(snake: &str) -> String {
    escape(bern_target_api::names::camel(snake))
}

/// `Primary` → `primary`, `URLKind` → `urlKind`.
pub fn case(pascal: &str) -> String {
    let chars: Vec<char> = pascal.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() && chars[index].is_uppercase() {
        let next_is_lower = chars.get(index + 1).is_some_and(|c| c.is_lowercase());
        if index > 0 && next_is_lower {
            break;
        }
        out.extend(chars[index].to_lowercase());
        index += 1;
    }
    out.extend(&chars[index..]);
    escape(out)
}

pub use bern_target_api::names::pascal;

/// A Swift string literal.
pub fn string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A Swift `Double` literal that keeps its fraction.
pub fn double(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.1}")
    } else {
        format!("{value:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_swift_conventions() {
        assert_eq!(member("last_action"), "lastAction");
        assert_eq!(member("default"), "`default`");
        assert_eq!(case("Primary"), "primary");
        assert_eq!(case("URLKind"), "urlKind");
        assert_eq!(case("Default"), "`default`");
        assert_eq!(pascal("main_window"), "MainWindow");
        assert_eq!(string("a\"b\\(c)\n"), "\"a\\\"b\\\\(c)\\n\"");
        assert_eq!(double(50.0), "50.0");
        assert_eq!(double(0.6), "0.6");
    }
}
