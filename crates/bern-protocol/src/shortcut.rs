//! Keyboard shortcuts as Bern writes them: the modifiers `cmd` (the
//! primary modifier: Command on Apple systems, Control elsewhere), `ctrl`,
//! `alt` and `shift`, then one key, joined by `+`, such as `"cmd+k"` or
//! `"space"`. Declarations write them as literals, which the compiler
//! normalizes; a shortcut that follows state is a text in the same form,
//! which engines normalize with [`parse_shortcut`] before they keep it, and
//! every runtime reads by the same rules.

/// Keys a shortcut may name besides a single printable character.
pub const SHORTCUT_KEYS: &[&str] = &[
    "space", "escape", "return", "tab", "delete", "up", "down", "left", "right", "home", "end",
    "pageup", "pagedown",
];

/// Normalize a keyboard shortcut: modifiers `cmd`, `ctrl`, `alt` and
/// `shift` in that order, then one key: a printable character in lower
/// case or one of [`SHORTCUT_KEYS`]. Case and spaces around the parts do
/// not matter, so `"Shift + Cmd + K"` is `"cmd+shift+k"`.
pub fn parse_shortcut(text: &str) -> Result<String, String> {
    let parts: Vec<String> = text
        .split('+')
        .map(|part| part.trim().to_lowercase())
        .collect();
    let (key, modifiers) = parts.split_last().ok_or("an empty shortcut")?;
    let mut ordered = Vec::new();
    for wanted in ["cmd", "ctrl", "alt", "shift"] {
        match modifiers.iter().filter(|m| *m == wanted).count() {
            0 => {}
            1 => ordered.push(wanted.to_string()),
            _ => return Err(format!("`{wanted}` is written twice")),
        }
    }
    if let Some(other) = modifiers
        .iter()
        .find(|m| !["cmd", "ctrl", "alt", "shift"].contains(&m.as_str()))
    {
        return Err(format!(
            "`{other}` is not a modifier; use cmd, ctrl, alt or shift"
        ));
    }
    let printable = key.chars().count() == 1
        && key
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_ascii_uppercase());
    if !printable && !SHORTCUT_KEYS.contains(&key.as_str()) {
        return Err(format!(
            "`{key}` is not a key; use one character or one of {}",
            SHORTCUT_KEYS.join(", ")
        ));
    }
    ordered.push(key.clone());
    Ok(ordered.join("+"))
}

/// Whether two shortcuts are pressed with the same keys on a system that is
/// (`apple`) or is not an Apple one: elsewhere `cmd` is Control, so
/// `cmd+k` and `ctrl+k` are one shortcut there. Texts that are no shortcut
/// are the same as nothing else.
pub fn same_keys(a: &str, b: &str, apple: bool) -> bool {
    let pressed = |text: &str| -> Option<String> {
        let normal = parse_shortcut(text).ok()?;
        if apple {
            return Some(normal);
        }
        let mut parts: Vec<&str> = normal.split('+').collect();
        let key = parts.pop()?;
        let control = parts.iter().any(|part| *part == "cmd" || *part == "ctrl");
        let mut keys: Vec<&str> = Vec::new();
        if control {
            keys.push("cmd");
        }
        keys.extend(
            parts
                .iter()
                .filter(|part| **part == "alt" || **part == "shift"),
        );
        keys.push(key);
        Some(keys.join("+"))
    };
    matches!((pressed(a), pressed(b)), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_normalize_their_modifiers_and_key() {
        assert_eq!(parse_shortcut("Shift + Cmd + K").unwrap(), "cmd+shift+k");
        assert_eq!(parse_shortcut("space").unwrap(), "space");
        assert_eq!(parse_shortcut("alt+ctrl+left").unwrap(), "ctrl+alt+left");
        assert_eq!(parse_shortcut("cmd+,").unwrap(), "cmd+,");
        // `+` joins the parts: it is no key of its own.
        assert!(parse_shortcut("cmd++").is_err());
        assert!(parse_shortcut("").is_err());
        assert!(parse_shortcut("cmd+cmd+k").is_err());
        assert!(parse_shortcut("hyper+k").is_err());
        assert!(parse_shortcut("cmd+f13").is_err());
    }

    #[test]
    fn control_and_command_are_one_key_away_from_apple() {
        assert!(same_keys("cmd+k", "ctrl+k", false));
        assert!(!same_keys("cmd+k", "ctrl+k", true));
        assert!(same_keys("shift+cmd+k", "cmd+shift+k", true));
        assert!(!same_keys("", "", true));
        assert!(!same_keys("cmd+k", "cmd+j", false));
    }
}
