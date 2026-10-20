//! Identifiers generators derive from Bern's `snake_case` names. Each
//! target escapes the results that are keywords of its own language.

/// `main_window` → `MainWindow`. A hyphen separates words too.
pub fn pascal(snake: &str) -> String {
    snake
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// The name an application value type (a `#[value]` enum or record)
/// has in generated Swift and Kotlin: `Layout` → `App_Layout`.
///
/// Those types sit at the top of the module the generated code shares
/// with the platform's frameworks and Bern's runtime, so a bare name like
/// SwiftUI's `Layout` or `View`, or Compose's `Modifier`, would hide the
/// framework's type from every file of the module. No type of those
/// frameworks and none of Bern's (`Bern…`) begins with `App_`, and the
/// prefix keeps distinct names distinct.
pub fn value_type(name: &str) -> String {
    format!("App_{name}")
}

/// `last_action` → `lastAction`.
pub fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for (index, c) in snake.chars().enumerate() {
        if c == '_' {
            upper = index > 0;
            continue;
        }
        if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_becomes_pascal_and_camel_case() {
        assert_eq!(pascal("main_window"), "MainWindow");
        assert_eq!(pascal("svg-demo"), "SvgDemo");
        assert_eq!(pascal("a__b_"), "AB");
        assert_eq!(camel("last_action"), "lastAction");
        assert_eq!(camel("_private"), "private");
        assert_eq!(camel("volume_2"), "volume2");
        assert_eq!(value_type("Layout"), "App_Layout");
    }
}
