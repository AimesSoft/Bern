//! `bern new`: a new application from the starter template, a window with
//! a counter. The project is a Cargo workspace of its own, with the
//! protocol, engine and interface crates and a `Bern.toml` for every target.

use bern_target_api::toml_string;
use std::path::{Path, PathBuf};

/// The public repository, for projects whose `bern` is not built from a
/// checkout on this computer.
pub const REPOSITORY: &str = "https://github.com/AimesSoft/Bern";

/// Languages the template has text in.
pub const LANGUAGES: [&str; 4] = ["en", "zh-Hans", "zh-Hant", "ja"];

/// The template's text in each of [`LANGUAGES`].
const TEXT: &[(&str, [&str; 4])] = &[
    (
        "greeting",
        [
            "Hello, World!",
            "你好，世界！",
            "你好，世界！",
            "こんにちは、世界！",
        ],
    ),
    (
        "count",
        [
            "Button presses: {count}",
            "按钮按下次数：{count}",
            "按鈕按下次數：{count}",
            "ボタンを押した回数：{count}",
        ],
    ),
    ("add_one", ["Add One", "加一", "加一", "1 増やす"]),
    ("reset", ["Reset", "重置", "重設", "リセット"]),
];

/// The template: path in the project (with `{{package}}` standing for the
/// package prefix) and contents.
const FILES: &[(&str, &str)] = &[
    ("Bern.toml", include_str!("../templates/new/Bern.toml")),
    ("Cargo.toml", include_str!("../templates/new/Cargo.toml.in")),
    (".gitignore", include_str!("../templates/new/gitignore")),
    (
        "rust-toolchain.toml",
        include_str!("../templates/new/rust-toolchain.toml"),
    ),
    (
        "{{package}}-protocol/Cargo.toml",
        include_str!("../templates/new/protocol/Cargo.toml.in"),
    ),
    (
        "{{package}}-protocol/src/lib.rs",
        include_str!("../templates/new/protocol/lib.rs"),
    ),
    (
        "{{package}}-core/Cargo.toml",
        include_str!("../templates/new/core/Cargo.toml.in"),
    ),
    (
        "{{package}}-core/src/lib.rs",
        include_str!("../templates/new/core/lib.rs"),
    ),
    (
        "{{package}}-ui/Cargo.toml",
        include_str!("../templates/new/ui/Cargo.toml.in"),
    ),
    (
        "{{package}}-ui/src/lib.rs",
        include_str!("../templates/new/ui/lib.rs"),
    ),
];

/// Where the new project takes Bern from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A checkout on this computer.
    Checkout(PathBuf),
    /// A Git repository.
    Git(String),
}

impl Source {
    /// The checkout this `bern` was built from, if it is still there and
    /// is not Cargo's copy of a Git repository; else the public repository.
    pub fn default_for_this_build() -> Source {
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        match checkout {
            Some(checkout) if is_checkout(&checkout) && !in_cargo_git_cache(&checkout) => {
                Source::Checkout(checkout)
            }
            _ => Source::Git(REPOSITORY.into()),
        }
    }

    fn dependency(&self) -> String {
        match self {
            Source::Checkout(checkout) => {
                let path = checkout.join("crates/bern").display().to_string();
                format!("path = {}", toml_string(&path.replace('\\', "/")))
            }
            Source::Git(url) => format!("git = {}", toml_string(url)),
        }
    }
}

/// Whether `dir` is a Bern checkout.
pub fn is_checkout(dir: &Path) -> bool {
    dir.join("crates/bern/Cargo.toml").is_file() && dir.join("crates/bern-ffi/Cargo.toml").is_file()
}

fn in_cargo_git_cache(dir: &Path) -> bool {
    let parts: Vec<_> = dir.components().map(|part| part.as_os_str()).collect();
    parts
        .windows(2)
        .any(|pair| pair[0] == "git" && pair[1] == "checkouts")
}

pub struct NewProject {
    pub dir: PathBuf,
    /// Lower snake case: the app id, the protocol namespace and the crate
    /// name prefix.
    pub id: String,
    pub bundle_id: String,
    /// The first is the default language.
    pub languages: Vec<String>,
    pub source: Source,
}

impl NewProject {
    /// A project in `dir`, named after its last component.
    pub fn new(
        dir: PathBuf,
        bundle_id: Option<String>,
        languages: Option<Vec<String>>,
        source: Source,
    ) -> Result<NewProject, String> {
        let written = dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("`{}` does not name a directory", dir.display()))?;
        let id = written.replace('-', "_");
        if !bern_protocol::valid_name(&id) {
            return Err(format!(
                "`{written}` cannot name an application: use lowercase letters, digits and `_` or `-`, starting with a letter"
            ));
        }
        let bundle_id =
            bundle_id.unwrap_or_else(|| format!("com.example.{}", id.replace('_', "-")));
        let languages = languages.unwrap_or_else(|| vec!["en".into(), "zh-Hans".into()]);
        if languages.is_empty() {
            return Err("--languages needs at least one language".into());
        }
        for language in &languages {
            if !LANGUAGES.contains(&language.as_str()) {
                return Err(format!(
                    "the template has no text in `{language}`; choose from {} and add other languages after creating the project",
                    LANGUAGES.join(", ")
                ));
            }
        }
        if languages
            .iter()
            .enumerate()
            .any(|(index, language)| languages[..index].contains(language))
        {
            return Err("--languages lists a language twice".into());
        }
        Ok(NewProject {
            dir,
            id,
            bundle_id,
            languages,
            source,
        })
    }

    /// Display name: `my_app` becomes `My App`.
    pub fn name(&self) -> String {
        self.id
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn text(&self, key: &str) -> String {
        let translations = TEXT
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, text)| text)
            .expect("template text");
        self.languages
            .iter()
            .map(|language| {
                let index = LANGUAGES
                    .iter()
                    .position(|known| known == language)
                    .expect("known language");
                format!(
                    "{} => {}",
                    rust_string(language),
                    rust_string(translations[index])
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn render(&self, template: &str) -> String {
        let name = self.name();
        let package = self.id.replace('_', "-");
        let engine = format!("{}Engine", name.replace(' ', ""));
        let app_names = self
            .languages
            .iter()
            .map(|language| format!("{} = {}", toml_string(language), toml_string(&name)))
            .collect::<Vec<_>>()
            .join(", ");
        let languages = self
            .languages
            .iter()
            .map(|language| toml_string(language))
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = template
            .replace("{{name}}", &name)
            .replace("{{id}}", &self.id)
            .replace("{{package}}", &package)
            .replace("{{engine}}", &engine)
            .replace("{{bundle_id}}", &self.bundle_id)
            .replace("{{app_names}}", &app_names)
            .replace("{{languages}}", &languages)
            .replace("{{default_language}}", &self.languages[0])
            .replace("{{bern}}", &self.source.dependency())
            .replace("{{text:title}}", &self.title());
        for (key, _) in TEXT {
            out = out.replace(&format!("{{{{text:{key}}}}}"), &self.text(key));
        }
        out
    }

    /// The window title: the app name, the same in every language.
    fn title(&self) -> String {
        self.languages
            .iter()
            .map(|language| format!("{} => {}", rust_string(language), rust_string(&self.name())))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The project's files: path relative to the project directory and
    /// contents.
    pub fn files(&self) -> Vec<(PathBuf, String)> {
        FILES
            .iter()
            .map(|(path, template)| (PathBuf::from(self.render(path)), self.render(template)))
            .collect()
    }

    /// Write the project. The directory must not exist or be empty.
    pub fn write(&self) -> Result<(), String> {
        if self.dir.exists() {
            let empty = std::fs::read_dir(&self.dir)
                .map_err(|error| format!("cannot read {}: {error}", self.dir.display()))?
                .next()
                .is_none();
            if !empty {
                return Err(format!(
                    "{} already exists and is not empty",
                    self.dir.display()
                ));
            }
        }
        for (path, contents) in self.files() {
            let path = self.dir.join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            }
            std::fs::write(&path, contents)
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        }
        Ok(())
    }
}

/// A Rust string literal.
fn rust_string(text: &str) -> String {
    format!("{text:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(dir: &Path, languages: &[&str]) -> NewProject {
        NewProject::new(
            dir.join("my_app"),
            None,
            Some(
                languages
                    .iter()
                    .map(|language| language.to_string())
                    .collect(),
            ),
            Source::Checkout(PathBuf::from("/src/Bern")),
        )
        .unwrap()
    }

    #[test]
    fn names_come_from_the_directory() {
        let new = |name: &str| {
            NewProject::new(
                PathBuf::from(name),
                None,
                None,
                Source::Git(REPOSITORY.into()),
            )
        };
        let project = new("work/my-app").unwrap();
        assert_eq!(project.id, "my_app");
        assert_eq!(project.name(), "My App");
        assert_eq!(project.bundle_id, "com.example.my-app");
        assert_eq!(project.languages, ["en", "zh-Hans"]);
        assert!(new("My App").is_err());
        assert!(new("1st").is_err());
        let error = NewProject::new(
            PathBuf::from("app"),
            None,
            Some(vec!["fr".into()]),
            Source::Git(REPOSITORY.into()),
        )
        .err()
        .unwrap();
        assert!(error.contains("en, zh-Hans, zh-Hant, ja"), "{error}");
    }

    #[test]
    fn every_placeholder_is_filled() {
        let project = project(Path::new("/tmp"), &["ja", "en"]);
        for (path, contents) in project.files() {
            let path = path.display().to_string();
            assert!(!path.contains("{{"), "{path}");
            assert!(!contents.contains("{{"), "{path}:\n{contents}");
        }
        let files = project.files();
        let read = |name: &str| {
            files
                .iter()
                .find(|(path, _)| path == Path::new(name))
                .map(|(_, contents)| contents.clone())
                .unwrap()
        };
        let manifest = read("Bern.toml");
        assert!(
            manifest.contains(r#"supported = ["ja", "en"]"#),
            "{manifest}"
        );
        assert!(manifest.contains(r#"default = "ja""#), "{manifest}");
        assert!(
            manifest.contains(r#"type = "my_app_core::MyAppEngine""#),
            "{manifest}"
        );
        assert!(read("Cargo.toml").contains(r#"bern = { path = "/src/Bern/crates/bern" }"#));
        let ui = read("my-app-ui/src/lib.rs");
        assert!(ui.contains(r#"tr! { "ja" => "ボタンを押した回数：{count}", "en" => "Button presses: {count}"; count: app.count }"#), "{ui}");
    }

    #[test]
    fn the_toolchain_is_the_one_bern_builds_with() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rust-toolchain.toml");
        assert_eq!(
            std::fs::read_to_string(workspace).unwrap(),
            include_str!("../templates/new/rust-toolchain.toml")
        );
    }

    /// The new project passes every compiler check, for every target and
    /// in every language the template knows.
    #[test]
    fn a_new_project_compiles() {
        let root = std::env::temp_dir().join(format!("bern-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = project(&root, &LANGUAGES);
        project.write().unwrap();
        assert!(project.write().unwrap_err().contains("not empty"));
        let config = bern_compiler::config::load(&project.dir.join("Bern.toml")).unwrap();
        assert_eq!(config.targets.len(), 4);
        let compiled = bern_compiler::compile(&config).unwrap();
        assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
        let _ = std::fs::remove_dir_all(&root);
    }
}
