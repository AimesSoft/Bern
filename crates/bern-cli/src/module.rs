//! `bern new --module <id>`: a module of an existing application, in
//! `modules/<id>/` next to its `Bern.toml`, with its protocol, engine and
//! interface crates, listed in the application's `Bern.toml` and in the
//! Cargo workspace.

use crate::new::{LANGUAGES, TEXT, rust_string};
use bern_target_api::toml_string;
use std::path::{Path, PathBuf};

const FILES: &[(&str, &str)] = &[
    ("Bern.toml", include_str!("../templates/module/Bern.toml")),
    ("README.md", include_str!("../templates/module/README.md")),
    (
        "protocol/Cargo.toml",
        include_str!("../templates/module/protocol/Cargo.toml.in"),
    ),
    (
        "protocol/src/lib.rs",
        include_str!("../templates/module/protocol/lib.rs"),
    ),
    (
        "core/Cargo.toml",
        include_str!("../templates/module/core/Cargo.toml.in"),
    ),
    (
        "core/src/lib.rs",
        include_str!("../templates/module/core/lib.rs"),
    ),
    (
        "ui/Cargo.toml",
        include_str!("../templates/module/ui/Cargo.toml.in"),
    ),
    (
        "ui/src/lib.rs",
        include_str!("../templates/module/ui/lib.rs"),
    ),
];

/// The crates of a module, relative to the application directory.
const CRATES: [&str; 3] = ["protocol", "core", "ui"];

pub struct NewModule {
    /// Directory holding the application's `Bern.toml`.
    pub app_dir: PathBuf,
    /// Lower snake case: the module id and protocol namespace.
    pub id: String,
    /// The application's languages, its default first.
    pub languages: Vec<String>,
}

impl NewModule {
    /// A module `id` for the application whose manifest is `manifest`.
    pub fn new(manifest: &Path, id: &str) -> Result<NewModule, String> {
        if !bern_protocol::valid_name(id) || bern_protocol::RESERVED_NAMESPACES.contains(&id) {
            return Err(format!(
                "`{id}` cannot name a module: use lower snake case, not one of {}",
                bern_protocol::RESERVED_NAMESPACES.join(", ")
            ));
        }
        let text = std::fs::read_to_string(manifest)
            .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
        let value: toml::Value = text
            .parse()
            .map_err(|error| format!("{}: {error}", manifest.display()))?;
        let languages = value.get("languages");
        let default = languages
            .and_then(|languages| languages.get("default"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{} has no [languages].default", manifest.display()))?
            .to_string();
        let mut supported: Vec<String> = languages
            .and_then(|languages| languages.get("supported"))
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|language| language.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        supported.retain(|language| *language != default);
        supported.insert(0, default);
        Ok(NewModule {
            app_dir: manifest
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            id: id.to_string(),
            languages: supported,
        })
    }

    /// Where the module goes, relative to the application directory.
    pub fn path(&self) -> String {
        format!("modules/{}", self.id)
    }

    /// `downloads` becomes `Downloads`.
    fn title(&self) -> String {
        bern_syntax::pascal_case(&self.id)
    }

    /// `tr!` entries of template text `key` in the application's languages;
    /// languages the template has no text in show the English.
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
                    .unwrap_or(0);
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
        let package = self.id.replace('_', "-");
        let names = self
            .languages
            .iter()
            .map(|language| format!("{} = {}", toml_string(language), toml_string(&self.title())))
            .collect::<Vec<_>>()
            .join(", ");
        let languages = self
            .languages
            .iter()
            .map(|language| toml_string(language))
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = template
            .replace("{{id}}", &self.id)
            .replace("{{crate}}", &self.id)
            .replace("{{package}}", &package)
            .replace("{{engine}}", &format!("{}Engine", self.title()))
            .replace("{{names}}", &names)
            .replace("{{languages}}", &languages)
            .replace("{{default_language}}", &self.languages[0]);
        for (key, _) in TEXT {
            out = out.replace(&format!("{{{{text:{key}}}}}"), &self.text(key));
        }
        out
    }

    /// The module's files, relative to its directory.
    pub fn files(&self) -> Vec<(PathBuf, String)> {
        FILES
            .iter()
            .map(|(path, template)| (PathBuf::from(path), self.render(template)))
            .collect()
    }

    /// Write the module and list it in the application. Returns notes for
    /// what could not be done automatically.
    pub fn write(&self) -> Result<Vec<String>, String> {
        let dir = self.app_dir.join(self.path());
        if dir.exists() {
            return Err(format!("{} already exists", dir.display()));
        }
        for (path, contents) in self.files() {
            let path = dir.join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            }
            std::fs::write(&path, contents)
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        }
        let mut notes = Vec::new();
        let manifest = self.app_dir.join("Bern.toml");
        let text = std::fs::read_to_string(&manifest)
            .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
        let entry = format!("[[modules]]\npath = \"{}\"\n", self.path());
        let mut updated = text.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&entry);
        std::fs::write(&manifest, updated)
            .map_err(|error| format!("cannot write {}: {error}", manifest.display()))?;
        let members: Vec<String> = CRATES
            .iter()
            .map(|part| format!("{}/{part}", self.path()))
            .collect();
        let cargo = self.app_dir.join("Cargo.toml");
        match std::fs::read_to_string(&cargo)
            .ok()
            .and_then(|text| add_members(&text, &members))
        {
            Some(updated) => std::fs::write(&cargo, updated)
                .map_err(|error| format!("cannot write {}: {error}", cargo.display()))?,
            None => notes.push(format!(
                "add {} to the members of your Cargo workspace",
                members
                    .iter()
                    .map(|member| format!("\"{member}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
        Ok(notes)
    }
}

/// `text` (a Cargo manifest) with `members` added to its
/// `[workspace].members` list, or `None` when it has no such list.
fn add_members(text: &str, members: &[String]) -> Option<String> {
    let workspace = text.find("[workspace]")?;
    let start = workspace + text[workspace..].find("members")?;
    let open = start + text[start..].find('[')?;
    let close = open + text[open..].find(']')?;
    let listed = text[open + 1..close].trim().trim_end_matches(',');
    let added: Vec<String> = members
        .iter()
        .map(|member| format!("\"{member}\""))
        .collect();
    let inner = if listed.is_empty() {
        added.join(", ")
    } else if listed.contains('\n') {
        format!("\n    {},\n    {},\n", listed.trim(), added.join(",\n    "))
    } else {
        format!("{listed}, {}", added.join(", "))
    };
    Some(format!("{}[{inner}]{}", &text[..open], &text[close + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_are_added_to_one_line_and_multi_line_lists() {
        let one = "[workspace]\nmembers = [\"a-ui\", \"a-core\"]\n";
        assert_eq!(
            add_members(one, &["modules/x/ui".into()]).unwrap(),
            "[workspace]\nmembers = [\"a-ui\", \"a-core\", \"modules/x/ui\"]\n"
        );
        let many = "[workspace]\nmembers = [\n    \"a-ui\",\n    \"a-core\",\n]\n";
        assert_eq!(
            add_members(many, &["modules/x/ui".into()]).unwrap(),
            "[workspace]\nmembers = [\n    \"a-ui\",\n    \"a-core\",\n    \"modules/x/ui\",\n]\n"
        );
        assert!(add_members("[package]\nname = \"a\"\n", &[]).is_none());
    }

    /// A module added to a new application passes every compiler check.
    #[test]
    fn a_new_module_compiles_with_its_application() {
        let root = std::env::temp_dir().join(format!("bern-module-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = crate::new::NewProject::new(
            root.join("my_app"),
            None,
            Some(vec!["zh-Hans".into(), "en".into()]),
            crate::new::Source::Checkout(PathBuf::from("/src/Bern")),
        )
        .unwrap();
        project.write().unwrap();
        let manifest = project.dir.join("Bern.toml");
        let module = NewModule::new(&manifest, "downloads").unwrap();
        assert_eq!(module.languages, ["zh-Hans", "en"]);
        assert!(module.write().unwrap().is_empty());
        assert!(module.write().unwrap_err().contains("already exists"));
        let cargo = std::fs::read_to_string(project.dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("\"modules/downloads/core\""), "{cargo}");
        let config = bern_compiler::config::load(&manifest).unwrap();
        assert_eq!(config.modules[0].id, "downloads");
        let compiled = bern_compiler::compile(&config).unwrap();
        assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
        assert!(compiled.app.protocol.state_index("downloads").is_some());
        assert!(NewModule::new(&manifest, "modules").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
