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

/// The class of an application session (a `#[session]` struct) in
/// generated Swift and Kotlin: `URL` → `App_URL`.
///
/// The declared name with `Session` appended was Foundation's
/// `URLSession` for a session named `URL`. Bern type names are unique
/// per application, so sessions share the prefix of value types.
pub fn session_type(name: &str) -> String {
    value_type(name)
}

/// The variable through which a view of generated Swift and Kotlin reads
/// a session: `Window` → `session_Window`.
///
/// `bern` and the declared name gave the views' own `bernWindow` and
/// `bernEngine` for sessions named `Window` and `Engine`. The names
/// generated for parameters and bindings have no underscore.
pub fn session_variable(name: &str) -> String {
    format!("session_{name}")
}

/// The view of a component in generated Swift and Kotlin: `empty` →
/// `AppView_empty`.
///
/// The name in Pascal case with `View` appended was SwiftUI's
/// `EmptyView`, `AnyView` or `ScrollView`, or a runtime's
/// `BernPlayerView`, for components named `empty`, `any`, `scroll` or
/// `bern_player`. No type or function of those frameworks or of the
/// runtimes begins with `AppView_`, none of the types' `App_` names
/// does, and the declared name follows unchanged, so distinct components
/// keep distinct names.
pub fn component(name: &str) -> String {
    format!("AppView_{name}")
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
        assert_eq!(session_type("URL"), "App_URL");
        assert_eq!(session_variable("Window"), "session_Window");
        assert_eq!(component("content_unavailable"), "AppView_content_unavailable");
    }
}
