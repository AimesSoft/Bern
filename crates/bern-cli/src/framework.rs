//! Where a project takes the Bern crates from: the source of the `bern`
//! dependency of its engine, as Cargo reads the workspace. Generated crates
//! take every Bern crate from the same source; when the project's Cargo
//! configuration patches `bern` to a checkout, from that checkout.

use bern_target_api::Framework;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The source of the `bern` dependency of `package` in the workspace at
/// `workspace`.
pub fn detect(workspace: &Path, package: &str) -> Result<Framework, String> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .current_dir(workspace)
        .output()
        .map_err(|error| format!("cannot run `cargo metadata`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata` failed:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cannot read `cargo metadata`: {error}"))?;
    check_script_host(&metadata)?;
    let dependency = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|candidate| candidate["name"] == package)
        .ok_or_else(|| format!("the workspace has no package `{package}`"))?["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|dependency| dependency["name"] == "bern" && dependency["kind"].is_null())
        .ok_or_else(|| format!("the package `{package}` does not depend on `bern`"))?;
    let declared = from_dependency(dependency)?;
    if matches!(declared, Framework::Path(_)) {
        return Ok(declared);
    }
    Ok(patched_checkout(workspace)
        .map(Framework::Path)
        .unwrap_or(declared))
}

/// Which JavaScript host a target runs is Bern's choice: a package that
/// picks one itself would bring it to every target, whatever the system
/// has. Tests may, having no target to run in.
fn check_script_host(metadata: &Value) -> Result<(), String> {
    for package in metadata["packages"].as_array().into_iter().flatten() {
        for dependency in package["dependencies"].as_array().into_iter().flatten() {
            let features = dependency["features"].as_array().map_or(0, Vec::len);
            if dependency["name"] == "bern-script" && dependency["kind"] != "dev" && features > 0 {
                return Err(format!(
                    "the package `{}` sets features on `bern-script`: Bern picks the JavaScript host of each target. Remove them, and set `javascript = true` on the targets that run scripts",
                    package["name"].as_str().unwrap_or_default()
                ));
            }
        }
    }
    Ok(())
}

/// The checkout a `[patch]` puts in place of the declared source of `bern`,
/// as Cargo resolves the workspace: offline first, then fetching what a
/// computer that never built the workspace lacks. Nothing when the workspace
/// cannot be resolved: the declared source stands.
fn patched_checkout(workspace: &Path) -> Option<PathBuf> {
    let resolve = |offline: bool| {
        let mut command = Command::new("cargo");
        command.args(["metadata", "--format-version", "1"]);
        if offline {
            command.arg("--offline");
        }
        let output = command
            .arg("--manifest-path")
            .arg(workspace.join("Cargo.toml"))
            .current_dir(workspace)
            .output()
            .ok()?;
        output.status.success().then_some(output.stdout)
    };
    let metadata = resolve(true).or_else(|| resolve(false))?;
    checkout_of(&serde_json::from_slice(&metadata).ok()?)
}

/// The `crates` directory of the resolved `bern` package in `metadata`
/// when it is a local Bern checkout.
fn checkout_of(metadata: &Value) -> Option<PathBuf> {
    let bern = metadata["packages"]
        .as_array()?
        .iter()
        .find(|package| package["name"] == "bern" && package["source"].is_null())?;
    let crates = Path::new(bern["manifest_path"].as_str()?)
        .parent()?
        .parent()?;
    crates
        .join("bern-ffi/Cargo.toml")
        .is_file()
        .then(|| crates.to_path_buf())
}

/// Read one dependency entry of `cargo metadata`.
fn from_dependency(dependency: &Value) -> Result<Framework, String> {
    if let Some(path) = dependency["path"].as_str() {
        let crates = Path::new(path)
            .parent()
            .ok_or_else(|| format!("`bern` at {path} is not in a directory"))?;
        if !crates.join("bern-ffi/Cargo.toml").is_file() {
            return Err(format!(
                "`bern` at {path} is not in a Bern checkout: {} has no bern-ffi",
                crates.display()
            ));
        }
        return Ok(Framework::Path(crates.to_path_buf()));
    }
    let source = dependency["source"].as_str().unwrap_or_default();
    if let Some(git) = source.strip_prefix("git+") {
        let (url, query) = git.split_once('?').unwrap_or((git, ""));
        let reference = query.split('&').find_map(|pair| {
            let (kind, value) = pair.split_once('=')?;
            matches!(kind, "branch" | "tag" | "rev").then(|| (kind.to_string(), decode(value)))
        });
        return Ok(Framework::Git {
            url: url.to_string(),
            reference,
        });
    }
    let crates_io = source == "registry+https://github.com/rust-lang/crates.io-index"
        || source == "sparse+https://index.crates.io/";
    if crates_io {
        let version = dependency["req"].as_str().unwrap_or("*");
        return Ok(Framework::Registry(version.to_string()));
    }
    Err(format!(
        "`bern` comes from {source}; Bern builds with a checkout, a Git repository or crates.io"
    ))
}

/// Undo the percent-encoding of a query value.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                index += 3;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_kind_of_source_is_read() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let path = json!({ "name": "bern", "path": crates.join("bern").display().to_string() });
        assert_eq!(
            from_dependency(&path).unwrap(),
            Framework::Path(crates.to_path_buf())
        );
        let git = json!({
            "name": "bern",
            "source": "git+https://github.com/AimesSoft/Bern?branch=feature%2Fnew",
        });
        assert_eq!(
            from_dependency(&git).unwrap(),
            Framework::Git {
                url: "https://github.com/AimesSoft/Bern".into(),
                reference: Some(("branch".into(), "feature/new".into())),
            }
        );
        let plain = json!({ "name": "bern", "source": "git+https://github.com/AimesSoft/Bern" });
        assert_eq!(
            from_dependency(&plain).unwrap(),
            Framework::Git {
                url: "https://github.com/AimesSoft/Bern".into(),
                reference: None,
            }
        );
        let release = json!({ "name": "bern", "source": "registry+https://github.com/rust-lang/crates.io-index", "req": "^0.2" });
        assert_eq!(
            from_dependency(&release).unwrap(),
            Framework::Registry("^0.2".into())
        );
        let elsewhere = json!({ "name": "bern", "path": "/nowhere/bern" });
        assert!(from_dependency(&elsewhere).is_err());
    }

    #[test]
    fn the_javascript_host_is_not_the_applications_to_pick() {
        let metadata = |features: Value, kind: Value| {
            json!({ "packages": [{
                "name": "my-core",
                "dependencies": [{ "name": "bern-script", "features": features, "kind": kind }],
            }] })
        };
        assert!(check_script_host(&metadata(json!([]), Value::Null)).is_ok());
        let error = check_script_host(&metadata(json!(["boa"]), Value::Null)).unwrap_err();
        assert!(
            error.contains("`my-core`") && error.contains("javascript = true"),
            "{error}"
        );
        assert!(check_script_host(&metadata(json!(["__boa"]), Value::Null)).is_err());
        // A test has no target to run in.
        assert!(check_script_host(&metadata(json!(["__boa"]), json!("dev"))).is_ok());
    }

    #[test]
    fn a_patched_bern_is_taken_from_its_checkout() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let manifest = crates.join("bern/Cargo.toml").display().to_string();
        let patched = json!({ "packages": [
            { "name": "serde", "source": "registry+https://github.com/rust-lang/crates.io-index", "manifest_path": "/registry/serde/Cargo.toml" },
            { "name": "bern", "source": null, "manifest_path": manifest },
        ] });
        assert_eq!(checkout_of(&patched), Some(crates.to_path_buf()));
        let declared = json!({ "packages": [
            { "name": "bern", "source": "git+https://github.com/AimesSoft/Bern?rev=abc#abc", "manifest_path": "/git/bern/crates/bern/Cargo.toml" },
        ] });
        assert_eq!(checkout_of(&declared), None);
        let elsewhere = json!({ "packages": [
            { "name": "bern", "source": null, "manifest_path": "/nowhere/crates/bern/Cargo.toml" },
        ] });
        assert_eq!(checkout_of(&elsewhere), None);
    }

    #[test]
    fn this_workspace_uses_its_own_checkout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Framework::Path(crates) = detect(&root, "showcase-core").unwrap() else {
            panic!("a path dependency");
        };
        // Compared canonically: Windows spells the same directory with and
        // without the verbatim prefix.
        assert_eq!(
            crates.canonicalize().unwrap(),
            root.join("crates").canonicalize().unwrap()
        );
    }
}
