//! Keep the C ABI for plain modules, and prepare wasm-bindgen dependencies
//! before serving a module that imports JavaScript. No tool is downloaded and
//! no global installation is changed by a build.
use std::ffi::OsStr;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::process::Command;

const HEADER: &[u8] = b"\0asm\x01\0\0\0";

fn u32_leb(bytes: &mut &[u8]) -> Result<usize, String> {
    let mut value = 0u32;
    for shift in (0..35).step_by(7) {
        let (&byte, rest) = bytes.split_first().ok_or("truncated WebAssembly length")?;
        *bytes = rest;
        if shift == 28 && byte > 15 {
            return Err("invalid WebAssembly length".into());
        }
        value |= u32::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Ok(value as usize);
        }
    }
    Err("invalid WebAssembly length".into())
}

fn needs_bindgen(bytes: &[u8]) -> Result<bool, String> {
    let mut bytes = bytes
        .strip_prefix(HEADER)
        .ok_or("not a WebAssembly module")?;
    let mut found = false;
    while let Some((&kind, rest)) = bytes.split_first() {
        bytes = rest;
        let size = u32_leb(&mut bytes)?;
        let (mut section, rest) = bytes
            .split_at_checked(size)
            .ok_or("truncated WebAssembly section")?;
        bytes = rest;
        if kind == 0 {
            let size = u32_leb(&mut section)?;
            let name = section
                .get(..size)
                .ok_or("truncated WebAssembly section name")?;
            found |= name == b"__wasm_bindgen_unstable";
        } else if kind == 2 {
            // Also report the tool's useful diagnostic if a build stripped the
            // metadata but left its unprocessed JavaScript imports behind.
            found |= section
                .windows(b"__wbindgen_placeholder__".len())
                .any(|part| part == b"__wbindgen_placeholder__");
        }
    }
    Ok(found)
}

/// Prepare app.wasm and, when needed, bindgen/ in the page's output directory.
/// The caller has already built the source module. A failed tool does not
/// replace the previously usable artifact with an unprocessed module.
pub fn prepare(from: &Path, dir: &Path) -> Result<(), String> {
    let tool = std::env::var_os("BERN_WASM_BINDGEN").unwrap_or_else(|| "wasm-bindgen".into());
    prepare_with(from, dir, &tool)
}

fn prepare_with(from: &Path, dir: &Path, tool: &OsStr) -> Result<(), String> {
    let bytes = std::fs::read(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?;
    let bindgen = needs_bindgen(&bytes)?;
    let bindings = dir.join("bindgen");
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if !bindgen {
        std::fs::write(dir.join("app.wasm"), bytes).map_err(|e| e.to_string())?;
        if bindings.exists() {
            std::fs::remove_dir_all(bindings).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let staged = dir.join(".bindgen-new");
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(|e| e.to_string())?;
    }
    let payload = staged.join("payload");
    std::fs::create_dir_all(&payload).map_err(|e| e.to_string())?;
    let result = (|| {
        let output = Command::new(tool)
            .args(["--target", "bundler", "--no-typescript", "--out-name", "bern_bindgen", "--out-dir"])
            .arg(&payload)
            .arg(from)
            .output()
            .map_err(|e| format!("this module uses wasm-bindgen; install its matching wasm-bindgen-cli or set BERN_WASM_BINDGEN to that executable: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "wasm-bindgen failed ({}). Use the CLI version matching the application's wasm-bindgen dependency; BERN_WASM_BINDGEN selects an isolated executable.\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let module = payload.join("bern_bindgen_bg.wasm");
        let mut bytes = std::fs::read(&module).map_err(|e| e.to_string())?;
        if needs_bindgen(&bytes)? || !payload.join("bern_bindgen_bg.js").is_file() {
            return Err(
                "wasm-bindgen did not produce a processed module and its JavaScript bindings"
                    .into(),
            );
        }
        // The bundler entry imports WASM directly; browsers instead use the
        // runtime's loader, combining these bindings with the Bern C ABI.
        std::fs::remove_file(payload.join("bern_bindgen.js")).map_err(|e| e.to_string())?;
        std::fs::remove_file(module).map_err(|e| e.to_string())?;
        // Inline snippet names encode the crate version, not their contents.
        // Version the whole tree so their relative imports cannot use stale JS.
        let mut hash = DefaultHasher::new();
        bytes.hash(&mut hash);
        for (source, relative) in
            bern_target_api::files_below(&payload).map_err(|e| e.to_string())?
        {
            relative.hash(&mut hash);
            std::fs::read(source)
                .map_err(|e| e.to_string())?
                .hash(&mut hash);
        }
        let version = format!("{:016x}", hash.finish());
        std::fs::rename(&payload, staged.join(&version)).map_err(|e| e.to_string())?;
        let path = format!("bindgen/{version}/bern_bindgen_bg.js");
        let name = b"bern.bindgen";
        // Both fixed-format lengths fit in one unsigned LEB128 byte.
        bytes.extend([0, (1 + name.len() + path.len()) as u8, name.len() as u8]);
        bytes.extend(name);
        bytes.extend(path.bytes());
        if bindings.exists() {
            std::fs::remove_dir_all(&bindings).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&staged, &bindings).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("app.wasm"), bytes).map_err(|e| e.to_string())
    })();
    if staged.exists() {
        let _ = std::fs::remove_dir_all(staged);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_section(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = HEADER.to_vec();
        assert!(payload.len() < 128);
        bytes.extend([kind, payload.len() as u8]);
        bytes.extend(payload);
        bytes
    }

    #[test]
    fn identifies_metadata_not_strings_in_data_sections() {
        let name = b"__wasm_bindgen_unstable";
        let mut payload = vec![name.len() as u8];
        payload.extend(name);
        assert!(needs_bindgen(&with_section(0, &payload)).unwrap());
        assert!(!needs_bindgen(&with_section(11, &payload)).unwrap());
        assert!(needs_bindgen(&with_section(2, b"__wbindgen_placeholder__")).unwrap());
        assert!(!needs_bindgen(HEADER).unwrap());
        assert!(needs_bindgen(b"built").is_err());
        assert!(needs_bindgen(&[HEADER, &[0, 8, 0]].concat()).is_err());
        assert!(needs_bindgen(&[HEADER, &[0, 1, 4]].concat()).is_err());
        assert!(u32_leb(&mut &[255, 255, 255, 255, 16][..]).is_err());
    }

    #[test]
    fn plain_module_needs_no_tool_and_removes_previous_bindings() {
        let root = std::env::temp_dir().join(format!("bern-plain-wasm-{}", std::process::id()));
        std::fs::create_dir_all(root.join("page/bindgen")).unwrap();
        std::fs::write(root.join("source.wasm"), HEADER).unwrap();
        std::fs::write(root.join("page/bindgen/stale.js"), b"stale").unwrap();
        prepare_with(
            &root.join("source.wasm"),
            &root.join("page"),
            OsStr::new("not-a-tool"),
        )
        .unwrap();
        assert_eq!(std::fs::read(root.join("page/app.wasm")).unwrap(), HEADER);
        assert!(!root.join("page/bindgen").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn tool_output_ships_snippets_and_replaces_all_previous_bindings() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("bern-bindgen-output-{}", std::process::id()));
        std::fs::create_dir_all(root.join("page/bindgen")).unwrap();
        std::fs::write(root.join("page/bindgen/stale.js"), b"stale").unwrap();
        std::fs::write(root.join("processed.wasm"), HEADER).unwrap();
        std::fs::write(
            root.join("source.wasm"),
            with_section(2, b"__wbindgen_placeholder__"),
        )
        .unwrap();
        let tool = root.join("tool");
        std::fs::write(
            &tool,
            r##"#!/bin/sh
set -eu
[ "$1" = --target ] && [ "$2" = bundler ] && [ "$3" = --no-typescript ]
[ "$4" = --out-name ] && [ "$5" = bern_bindgen ] && [ "$6" = --out-dir ]
cp "$(dirname "$0")/processed.wasm" "$7/bern_bindgen_bg.wasm"
printf 'export {};\n' > "$7/bern_bindgen.js"
printf 'export {};\n' > "$7/bern_bindgen_bg.js"
mkdir -p "$7/snippets/local"
printf 'export {};\n' > "$7/snippets/local/inline0.js"
"##,
        )
        .unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o700)).unwrap();
        prepare_with(
            &root.join("source.wasm"),
            &root.join("page"),
            tool.as_os_str(),
        )
        .unwrap();
        let artifact = std::fs::read(root.join("page/app.wasm")).unwrap();
        assert!(!needs_bindgen(&artifact).unwrap());
        let path = std::fs::read_dir(root.join("page/bindgen"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let version = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(version.len(), 16);
        assert!(artifact.ends_with(format!("bindgen/{version}/bern_bindgen_bg.js").as_bytes()));
        assert!(path.join("bern_bindgen_bg.js").is_file());
        assert!(path.join("snippets/local/inline0.js").is_file());
        assert!(!root.join("page/bindgen/stale.js").exists());
        assert!(!path.join("bern_bindgen.js").exists());
        assert!(!path.join("bern_bindgen_bg.wasm").exists());
        assert!(!root.join("page/.bindgen-new").exists());
        // A local snippet can change without any change to its crate version.
        let script = std::fs::read_to_string(&tool).unwrap();
        std::fs::write(
            &tool,
            script.replace("export {};", "export const changed = true;"),
        )
        .unwrap();
        prepare_with(
            &root.join("source.wasm"),
            &root.join("page"),
            tool.as_os_str(),
        )
        .unwrap();
        let replacement = std::fs::read(root.join("page/app.wasm")).unwrap();
        assert_ne!(artifact, replacement);
        assert!(!path.exists(), "previous binding tree is removed");
        std::fs::write(
            &tool,
            "#!/bin/sh\nprintf 'schema version mismatch' >&2\nexit 13\n",
        )
        .unwrap();
        let error = prepare_with(
            &root.join("source.wasm"),
            &root.join("page"),
            tool.as_os_str(),
        )
        .unwrap_err();
        assert!(error.contains("schema version mismatch"));
        assert!(error.contains("BERN_WASM_BINDGEN"));
        assert_eq!(
            std::fs::read(root.join("page/app.wasm")).unwrap(),
            replacement
        );
        assert_eq!(
            std::fs::read_dir(root.join("page/bindgen"))
                .unwrap()
                .count(),
            1
        );
        assert!(!root.join("page/.bindgen-new").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_tool_fails_before_replacing_previous_artifacts() {
        let root = std::env::temp_dir().join(format!("bern-bindgen-failed-{}", std::process::id()));
        std::fs::create_dir_all(root.join("page")).unwrap();
        std::fs::write(root.join("page/app.wasm"), HEADER).unwrap();
        std::fs::write(
            root.join("source.wasm"),
            with_section(2, b"__wbindgen_placeholder__"),
        )
        .unwrap();
        let error = prepare_with(
            &root.join("source.wasm"),
            &root.join("page"),
            root.join("missing-tool").as_os_str(),
        )
        .unwrap_err();
        assert!(error.contains("BERN_WASM_BINDGEN"));
        assert_eq!(std::fs::read(root.join("page/app.wasm")).unwrap(), HEADER);
        assert!(!root.join("page/.bindgen-new").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
