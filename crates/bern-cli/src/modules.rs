//! Carry the compiler's module selection into generated Cargo workspaces.
use bern_compiler::config::Config;
use bern_target_api::{BuildStep, GeneratedFile};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub fn excluded(config: &Config) -> String {
    config
        .build_modules
        .0
        .iter()
        .filter(|(_, enabled)| !**enabled)
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn selected_features(path: &Path, config: &Config) -> Result<Option<Vec<String>>, String> {
    let manifest = path.join("Cargo.toml");
    let text =
        std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let value: toml::Value = text
        .parse()
        .map_err(|e| format!("{}: {e}", manifest.display()))?;
    let Some(features) = value.get("features").and_then(toml::Value::as_table) else {
        return Ok(None);
    };
    if !features.keys().any(|key| key.starts_with("bern-module-")) {
        return Ok(None);
    }
    let forbidden: BTreeSet<_> = config
        .build_modules
        .0
        .iter()
        .filter(|(_, enabled)| !**enabled)
        .map(|(id, _)| format!("bern-module-{id}"))
        .collect();
    let mut selected = BTreeSet::new();
    if let Some(defaults) = features.get("default").and_then(toml::Value::as_array) {
        selected.extend(
            defaults
                .iter()
                .filter_map(toml::Value::as_str)
                .filter(|name| !forbidden.contains(*name))
                .map(str::to_string),
        );
    }
    for (id, enabled) in &config.build_modules.0 {
        let name = format!("bern-module-{id}");
        if *enabled && features.contains_key(&name) {
            selected.insert(name);
        }
    }
    fn check(
        name: &str,
        features: &toml::map::Map<String, toml::Value>,
        forbidden: &BTreeSet<String>,
        seen: &mut BTreeSet<String>,
        chain: &mut Vec<String>,
    ) -> Result<(), String> {
        chain.push(name.to_string());
        if forbidden.contains(name)
            || name
                .rsplit_once('/')
                .is_some_and(|(_, feature)| forbidden.contains(feature))
        {
            return Err(format!(
                "Cargo feature chain {} enables an excluded module; put module features directly in default",
                chain.join(" -> ")
            ));
        }
        if seen.insert(name.to_string()) {
            for next in features
                .get(name)
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(toml::Value::as_str)
            {
                check(next, features, forbidden, seen, chain)?;
            }
        }
        chain.pop();
        Ok(())
    }
    for name in &selected {
        if name.contains(':') || name.contains('/') {
            return Err(format!(
                "{}: default feature entry `{name}` must be wrapped in a named Cargo feature (for example `base = [{name:?}]`) so module selection can preserve it",
                manifest.display()
            ));
        }
        check(
            name,
            features,
            &forbidden,
            &mut BTreeSet::new(),
            &mut Vec::new(),
        )
        .map_err(|e| format!("{}: {e}", manifest.display()))?;
    }
    Ok(Some(selected.into_iter().collect()))
}

/// Every generated Cargo manifest, including CLI/MCP, Web page and host,
/// gets the same selected features and a reproducible local environment.
pub fn configure(files: &mut Vec<GeneratedFile>, config: &Config) -> Result<(), String> {
    let mut cache = BTreeMap::new();
    let mut extra = Vec::new();
    for file in files
        .iter_mut()
        .filter(|f| f.path.file_name().is_some_and(|n| n == "Cargo.toml"))
    {
        let text = std::str::from_utf8(&file.contents).map_err(|e| e.to_string())?;
        let mut manifest: toml::Value = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        if let Some(deps) = manifest
            .get_mut("dependencies")
            .and_then(toml::Value::as_table_mut)
        {
            for dep in deps
                .iter_mut()
                .map(|(_, value)| value)
                .filter_map(toml::Value::as_table_mut)
            {
                let Some(path) = dep
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .map(str::to_string)
                else {
                    continue;
                };
                if !cache.contains_key(&path) {
                    cache.insert(path.clone(), selected_features(Path::new(&path), config)?);
                }
                if let Some(features) = &cache[&path] {
                    dep.insert("default-features".into(), toml::Value::Boolean(false));
                    let list = dep
                        .entry("features")
                        .or_insert_with(|| toml::Value::Array(Vec::new()))
                        .as_array_mut()
                        .ok_or("Cargo features must be an array")?;
                    for name in features {
                        let name = toml::Value::String(name.clone());
                        if !list.contains(&name) {
                            list.push(name);
                        }
                    }
                }
            }
        }
        file.contents = toml::to_string_pretty(&manifest)
            .map_err(|e| e.to_string())?
            .into_bytes();
        let env = BTreeMap::from([
            (
                "BERN_BUILD_ROOT",
                BTreeMap::from([
                    (
                        "value",
                        toml::Value::String(config.dir.display().to_string()),
                    ),
                    ("force", toml::Value::Boolean(true)),
                ]),
            ),
            (
                "BERN_EXCLUDE_MODULES",
                BTreeMap::from([
                    ("value", toml::Value::String(excluded(config))),
                    ("force", toml::Value::Boolean(true)),
                ]),
            ),
        ]);
        extra.push(GeneratedFile::text(
            file.path.parent().unwrap().join(".cargo/config.toml"),
            toml::to_string(&BTreeMap::from([("env", env)])).map_err(|e| e.to_string())?,
        ));
    }
    extra.push(GeneratedFile::text(
        "modules.json",
        serde_json::to_string_pretty(&config.build_modules.0).unwrap(),
    ));
    for addition in extra {
        if let Some(existing) = files.iter_mut().find(|f| f.path == addition.path) {
            // WinUI provides linker flags here; keep target/backend configuration.
            let mut config: toml::Value = std::str::from_utf8(&existing.contents)
                .map_err(|e| e.to_string())?
                .parse()
                .map_err(|e: toml::de::Error| e.to_string())?;
            let addition: toml::Value = std::str::from_utf8(&addition.contents)
                .unwrap()
                .parse()
                .unwrap();
            let table = config
                .as_table_mut()
                .ok_or("Cargo config must be a table")?;
            let env = table
                .entry("env")
                .or_insert_with(|| toml::Value::Table(Default::default()))
                .as_table_mut()
                .ok_or("Cargo env must be a table")?;
            for (key, value) in addition["env"].as_table().unwrap() {
                env.insert(key.clone(), value.clone());
            }
            existing.contents = toml::to_string_pretty(&config)
                .map_err(|e| e.to_string())?
                .into_bytes();
        } else {
            files.push(addition);
        }
    }
    Ok(())
}

pub fn environment(steps: &mut [BuildStep], config: &Config) {
    for step in steps {
        if let BuildStep::Run { env, .. } = step {
            env.push(("BERN_BUILD_ROOT".into(), config.dir.display().to_string()));
            env.push(("BERN_EXCLUDE_MODULES".into(), excluded(config)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_manifests_keep_ordinary_features_and_platform_configuration() {
        let mut config = bern_compiler::config::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/planner/Bern.toml"),
        )
        .unwrap();
        config.build_modules.0.insert("notes".into(), false);
        let dir = std::env::temp_dir().join(format!("bern-module-cargo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            r#"
            [package]
            name = "shell"
            version = "0.1.0"
            [features]
            default = ["storage", "bern-module-notes", "bern-module-tags"]
            storage = []
            bern-module-notes = ["dep:notes"]
            bern-module-tags = []
        "#,
        )
        .unwrap();
        let manifest = format!(
            "[package]\nname = \"glue\"\nversion = \"0.1.0\"\n[dependencies]\nshell = {{ path = {:?} }}\n",
            dir.display().to_string()
        );
        let mut files = vec![
            GeneratedFile::text("glue/Cargo.toml", manifest),
            GeneratedFile::text(
                "glue/.cargo/config.toml",
                "[target.x86_64-pc-windows-msvc]\nrustflags = [\"-Ctarget-feature=+crt-static\"]\n[env]\nEXISTING = \"preserved\"\n",
            ),
        ];
        configure(&mut files, &config).unwrap();
        let text = |name: &str| {
            std::str::from_utf8(
                &files
                    .iter()
                    .find(|f| f.path == Path::new(name))
                    .unwrap()
                    .contents,
            )
            .unwrap()
        };
        let manifest: toml::Value = text("glue/Cargo.toml").parse().unwrap();
        assert_eq!(
            manifest["dependencies"]["shell"]["default-features"].as_bool(),
            Some(false)
        );
        assert_eq!(
            manifest["dependencies"]["shell"]["features"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<_>>(),
            ["bern-module-tags", "storage"]
        );
        let cargo_config: toml::Value = text("glue/.cargo/config.toml").parse().unwrap();
        assert_eq!(cargo_config["env"]["EXISTING"].as_str(), Some("preserved"));
        assert!(
            cargo_config["target"]["x86_64-pc-windows-msvc"]
                .get("rustflags")
                .is_some()
        );
        assert_eq!(
            cargo_config["env"]["BERN_EXCLUDE_MODULES"]["value"].as_str(),
            Some("notes")
        );
        assert_eq!(
            files
                .iter()
                .filter(|f| f.path == Path::new("glue/.cargo/config.toml"))
                .count(),
            1
        );
        std::fs::write(
            dir.join("Cargo.toml"),
            r#"
            [features]
            default = ["all"]
            all = ["bern-module-notes"]
            bern-module-notes = []
        "#,
        )
        .unwrap();
        let error = selected_features(&dir, &config).unwrap_err();
        assert!(error.contains("all -> bern-module-notes"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
