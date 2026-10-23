//! The `framework-names` example names its value types like types of
//! the platforms' frameworks and of the browser (`Text`, `Image`, `Event`,
//! `Map`, `Error`…). Generated JavaScript keeps the codecs, variant names
//! and labels of the types as properties of `T`, `N` and `L`, describes
//! sessions by name in strings and writes components as functions `c_`
//! and the declared name, so the names meet no global; Node.js parses the
//! module where it is installed.

use bern_backend_web::WebBrowser;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Generate the example into `target/bern-test/web-names`.
fn generate() -> PathBuf {
    let root = workspace();
    let config =
        bern_compiler::config::load(&root.join("examples/framework-names/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let target = config
        .targets
        .iter()
        .find(|target| target.platform == "browser")
        .unwrap()
        .clone();
    let problems = WebBrowser.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/web-names");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target,
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    };
    for file in WebBrowser.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    output
}

#[test]
fn value_types_named_like_framework_types_stay_properties() {
    let output = generate();
    let js = std::fs::read_to_string(output.join("app.js")).unwrap();
    for expected in [
        "T.Layout = {",
        "N.Layout = [",
        "L.Layout = ",
        "T.Text = {",
        "T.Image = {",
        "T.Event = {",
        "T.Map = {",
        "T.Error = {",
        "T.Content = {",
        "T.Shape = {",
        "function c_empty(p, ctx, a) {",
        "function c_bern_player(p, ctx, a) {",
        r#"{ name: "URL", fields: ["#,
        r#"{ name: "Window", fields: ["#,
    ] {
        assert!(js.contains(expected), "app.js lacks `{expected}`");
    }
    // No type becomes a binding of the module.
    for name in [
        "Layout", "Text", "Image", "Event", "Map", "Error", "Content",
    ] {
        for binding in ["const", "let", "function", "class"] {
            assert!(
                !js.contains(&format!("{binding} {name} ")),
                "app.js declares `{binding} {name}`"
            );
        }
    }

    // Where Node.js is installed, it parses the module.
    let module = output.join("app.mjs");
    std::fs::copy(output.join("app.js"), &module).unwrap();
    match Command::new("node").arg("--check").arg(&module).output() {
        Ok(result) => assert!(
            result.status.success(),
            "node rejected app.js:\n{}",
            String::from_utf8_lossy(&result.stderr)
        ),
        Err(_) => eprintln!("node not available; skipping the syntax check"),
    }
}
