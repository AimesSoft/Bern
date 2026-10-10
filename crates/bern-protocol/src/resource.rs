//! Names of the resource files an application ships with its build.
//!
//! A resource name is the file's `/`-separated path relative to `Bern.toml`,
//! for example `media/aurora.mp4`. The build tool checks the declared names
//! with this rule and the engine host checks the names an engine resolves,
//! so both sides agree on what a name is.

/// Check a resource name. Every segment uses ASCII letters, digits, `-`,
/// `_` and `.`, and does not start with `.`; absolute paths, empty
/// segments, `..` and backslashes are rejected. The error explains why.
pub fn check_resource_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("a resource name cannot be empty".into());
    }
    if name.starts_with('/') {
        return Err(format!(
            "resource `{name}` is absolute; name it relative to Bern.toml"
        ));
    }
    for segment in name.split('/') {
        if segment.is_empty() {
            return Err(format!("resource `{name}` has an empty path segment"));
        }
        if segment.starts_with('.') {
            return Err(format!(
                "resource `{name}`: `{segment}` starts with `.`; hidden files, `.` and `..` are not resources"
            ));
        }
        if let Some(invalid) = segment
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        {
            return Err(format!(
                "resource `{name}` contains `{invalid}`; names use ASCII letters, digits, `-`, `_` and `.`"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_portable_relative_paths() {
        for name in ["media/aurora.mp4", "seed.json", "a/b-c/d_e.f.txt", "x"] {
            assert_eq!(check_resource_name(name), Ok(()), "{name}");
        }
        for name in [
            "",
            "/media/a.mp4",
            "media//a.mp4",
            "media/",
            "../a.mp4",
            "media/./a.mp4",
            ".hidden",
            "media\\a.mp4",
            "media/a b.mp4",
            "影片.mp4",
        ] {
            assert!(check_resource_name(name).is_err(), "{name}");
        }
    }
}
