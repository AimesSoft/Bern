//! Build-time selection and shared sources of the JavaScript host.
use crate::BuildContext;

pub fn dependency(context: &BuildContext) -> String {
    let Some(backend) = context.target.javascript_backend() else {
        return String::new();
    };
    format!(
        "bern-script = {{ {}, features = [{}] }}\n",
        context.framework.dependency("bern-script"),
        if backend == "boa" { "\"boa\"" } else { "" }
    )
}

pub fn webview(_context: &BuildContext) -> bool {
    false
}

pub const RUNTIME: &str = bern_script::HOST_RUNTIME;

/// Identity inputs of the Windows script host (`boa` runs in Rust, so no C++
/// WebView2 bridge is injected).
pub const WINDOWS_SOURCES: [&str; 1] = [include_str!("script.rs")];

/// Add only the selected host to generated sources and dependencies.
pub fn configure(files: &mut [crate::GeneratedFile], context: &BuildContext) {
    let Some(backend) = context.target.javascript_backend() else {
        return;
    };
    for file in files {
        let path = file.path.to_string_lossy();
        if path.ends_with("Cargo.toml") {
            let source = String::from_utf8_lossy(&file.contents);
            file.contents = source
                .replacen(
                    "[dependencies]\n",
                    &format!("[dependencies]\n{}", dependency(context)),
                    1,
                )
                .into_bytes();
        } else if backend == "browser" && path.ends_with("bern.js") {
            let mut source = String::from_utf8_lossy(&file.contents).into_owned();
            source = source.replace(
                "  start(fingerprint, resources) {",
                "  start(fingerprint, resources) {\n    this.stopScripts = startScriptHost(this);",
            );
            source = source.replace(
                "      throw new Error(message);",
                "      this.stopScripts?.();\n      throw new Error(message);",
            );
            source.push_str(include_str!("../templates/script/host.js"));
            file.contents = source.into_bytes();
        }
    }
}
