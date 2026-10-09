//! Static WebUI export. This crate does not link or execute an application engine.

use bern_core::App;
use std::{
    error::Error,
    fs, io,
    net::Ipv6Addr,
    path::{Component, Path, PathBuf},
};

pub const BUILD_MODE: &str = "ui";

/// Export a validated UI and an already built frontend WASM package.
/// The caller owns the generated output directory; handwritten files do not belong there.
pub fn export(
    app: &App,
    output: &Path,
    wasm_package: &Path,
    endpoint: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    app.validate()?;
    validate_endpoint(endpoint)?;
    let wasm_package = wasm_package.canonicalize()?;
    let output = resolve_output_path(output)?;
    reject_overlap(&wasm_package, &output)?;
    let package_output = resolve_output_path(&output.join("pkg"))?;
    reject_overlap(&wasm_package, &package_output)?;
    for required in ["bern_web_runtime.js", "bern_web_runtime_bg.wasm"] {
        if !wasm_package.join(required).is_file() {
            return Err(format!("missing frontend WASM package file: {required}").into());
        }
    }
    // An app build must never be silently reused as a UI-only distribution.
    let manifest_path = output.join("build.json");
    if manifest_path.exists() {
        let previous: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        if previous["mode"] != BUILD_MODE {
            return Err(
                "output belongs to a different build mode; choose a fresh output directory".into(),
            );
        }
    }
    fs::create_dir_all(&output)?;
    fs::write(output.join("ui.json"), serde_json::to_vec_pretty(app)?)?;
    fs::write(
        output.join("connection.json"),
        serde_json::to_vec_pretty(&serde_json::json!({ "endpoint": endpoint }))?,
    )?;
    fs::write(
        output.join("index.html"),
        include_str!("../../bern-web-runtime/assets/index.html"),
    )?;
    fs::write(
        output.join("runtime.js"),
        include_str!("../../bern-web-runtime/assets/runtime.js"),
    )?;
    fs::write(
        output.join("style.css"),
        include_str!("../../bern-web-runtime/assets/style.css"),
    )?;
    copy_directory(&wasm_package, &package_output)?;
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "mode": BUILD_MODE,
            "schema_version": app.schema_version,
            "generator": "bern-web",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "engine_included": false,
            "entrypoint": "index.html",
            "frontend_runtime": "pkg/bern_web_runtime_bg.wasm",
            "wasm_bindgen_version": "0.2.126"
        }))?,
    )?;
    Ok(())
}

fn validate_endpoint(endpoint: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    const INVALID: &str = "WebUI endpoint must be an absolute ws(s) URL or an origin-relative path without credentials, fragments, backslashes or whitespace";
    if endpoint.chars().any(|character| {
        character.is_whitespace() || character.is_control() || matches!(character, '\\' | '#')
    }) {
        return Err(INVALID.into());
    }
    let uri: http::Uri = endpoint.parse().map_err(|_| INVALID)?;
    if endpoint.starts_with('/') {
        return if !endpoint.starts_with("//") && uri.scheme().is_none() && uri.authority().is_none()
        {
            Ok(())
        } else {
            Err(INVALID.into())
        };
    }
    if !uri.scheme_str().is_some_and(|scheme| {
        scheme.eq_ignore_ascii_case("ws") || scheme.eq_ignore_ascii_case("wss")
    }) {
        return Err(INVALID.into());
    }
    let authority = uri.authority().ok_or(INVALID)?;
    if authority.as_str().contains('@') || authority.host().is_empty() {
        return Err(INVALID.into());
    }
    let host = authority.host();
    if host.starts_with('[') {
        host.strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .ok_or(INVALID)?
            .parse::<Ipv6Addr>()
            .map_err(|_| INVALID)?;
    }
    let port_suffix = authority.as_str().strip_prefix(host).ok_or(INVALID)?;
    if !port_suffix.is_empty() {
        let port = port_suffix.strip_prefix(':').ok_or(INVALID)?;
        if port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port.parse::<u16>().is_err()
        {
            return Err(INVALID.into());
        }
    }
    Ok(())
}

/// Resolve existing path components through the filesystem before appending
/// missing ones. Resolving symlinks before `..` preserves filesystem semantics.
/// Unlike create-then-canonicalize, this does not write to an invalid destination.
fn resolve_output_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Prefix(_) | Component::RootDir => resolved.push(component.as_os_str()),
            Component::Normal(_) => {
                resolved.push(component.as_os_str());
                match fs::symlink_metadata(&resolved) {
                    Ok(_) => resolved = resolved.canonicalize()?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    Ok(resolved)
}

fn reject_overlap(source: &Path, destination: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
    if source.starts_with(destination) || destination.starts_with(source) {
        return Err("WASM package and output directories must not overlap".into());
    }
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err("WASM package must not contain symlinks".into());
        }
        let output = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_directory(&entry.path(), &output)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), output)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_core::Node;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        package: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "bern-web-{}-{nonce}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let package = root.join("package");
            fs::create_dir_all(package.join("snippets")).unwrap();
            fs::write(
                package.join("bern_web_runtime.js"),
                "export default function init() {}",
            )
            .unwrap();
            fs::write(package.join("bern_web_runtime_bg.wasm"), b"\0asm").unwrap();
            fs::write(
                package.join("snippets/helper.js"),
                "export const helper = true;",
            )
            .unwrap();
            Self { root, package }
        }

        fn app(&self) -> App {
            App::new("Test", Node::text("title", "Text"), vec![], vec![])
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn invalid_endpoints_fail_before_writing_output() {
        let fixture = Fixture::new();
        let output = fixture.root.join("output");
        for endpoint in [
            "",
            "ws://",
            "wss://",
            "ws:///api/ws",
            "ws://:3000/api/ws",
            "ws://localhost:3000/api/ws#fragment",
            "/api/ws#fragment",
            "ws://user:secret@localhost/api/ws",
            "ws://user@localhost/api/ws",
            "/\\other-host/api/ws",
            "ws://localhost\\api/ws",
            "//other-host/api/ws",
            "ws://local host/api/ws",
            " /api/ws",
            "/api/ws\n",
            "/api/ws\u{00a0}",
            "http://localhost/api/ws",
            "https://localhost/api/ws",
            "file:///api/ws",
            "api/ws",
            "localhost:3000",
            "ws://localhost:99999/api/ws",
            "ws://localhost:/api/ws",
            "ws://localhost:abc/api/ws",
            "ws://[not-ipv6]/api/ws",
        ] {
            let error = export(&fixture.app(), &output, &fixture.package, endpoint).unwrap_err();
            assert!(
                error.to_string().contains("WebUI endpoint"),
                "{endpoint}: {error}"
            );
            assert!(
                !output.exists(),
                "invalid endpoint wrote output: {endpoint}"
            );
        }
    }

    #[test]
    fn valid_endpoints_export_the_complete_package_without_rewriting_the_address() {
        let fixture = Fixture::new();
        for (index, endpoint) in [
            "/",
            "/api/ws",
            "/api/ws?token=encoded%20value",
            "ws://127.0.0.1:3000/api/ws",
            "wss://example.com/api/ws?version=1",
            "ws://[::1]:3000/api/ws",
            "wss://example.com",
        ]
        .into_iter()
        .enumerate()
        {
            let output = fixture.root.join(format!("output-{index}"));
            export(&fixture.app(), &output, &fixture.package, endpoint).unwrap();
            let connection: serde_json::Value =
                serde_json::from_slice(&fs::read(output.join("connection.json")).unwrap()).unwrap();
            assert_eq!(connection["endpoint"], endpoint);
            assert_eq!(
                fs::read(output.join("pkg/bern_web_runtime_bg.wasm")).unwrap(),
                b"\0asm"
            );
            assert!(output.join("pkg/snippets/helper.js").is_file());
            assert!(output.join("build.json").is_file());
        }
    }

    #[test]
    fn overlapping_directories_are_rejected_in_both_directions_before_writing() {
        let fixture = Fixture::new();
        for output in [
            fixture.package.clone(),
            fixture.package.join("dist"),
            fixture.root.clone(),
            fixture.package.join("missing/../dist"),
        ] {
            let error = export(&fixture.app(), &output, &fixture.package, "/api/ws").unwrap_err();
            assert!(error.to_string().contains("must not overlap"), "{error}");
            assert!(!fixture.package.join("dist").exists());
            assert!(!fixture.package.join("missing").exists());
            assert!(!output.join("ui.json").exists());
        }
        assert_eq!(
            fs::read(fixture.package.join("bern_web_runtime_bg.wasm")).unwrap(),
            b"\0asm"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_aliases_cannot_hide_overlapping_directories() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let alias = fixture.root.join("alias");
        symlink(&fixture.package, &alias).unwrap();
        for output in [alias.clone(), alias.join("dist")] {
            let error = export(&fixture.app(), &output, &fixture.package, "/api/ws").unwrap_err();
            assert!(error.to_string().contains("must not overlap"));
        }
        let output = fixture.root.join("output");
        fs::create_dir(&output).unwrap();
        symlink(&fixture.package, output.join("pkg")).unwrap();
        let error = export(&fixture.app(), &output, &alias, "/api/ws").unwrap_err();
        assert!(error.to_string().contains("must not overlap"));
        assert!(!output.join("ui.json").exists());
        assert!(!fixture.package.join("dist").exists());
    }
}
