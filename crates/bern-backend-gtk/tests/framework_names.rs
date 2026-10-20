//! The `framework-names` example names its value types like types of
//! GTK, the other platforms' frameworks and Rust's (`Window`, `Button`,
//! `Image`, `Orientation`, `Align`, `Widget`, `Text`, `Error`…). The
//! generated GTK application carries values as `bern_gtk` values and
//! names the types and sessions only in strings, and writes components
//! as functions `c_` and the declared name; where GTK 4 and libadwaita
//! are installed, rustc type-checks it.

use bern_backend_gtk::GtkLinux;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn value_types_named_like_framework_types_compile() {
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
        .find(|target| target.platform == "linux")
        .unwrap()
        .clone();
    let problems = GtkLinux.check(&compiled.app, &target);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/gtk-names");
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
    for file in GtkLinux.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let ui = std::fs::read_to_string(output.join("app/src/ui.rs")).unwrap();
    for expected in [
        r#"b::display(&c.engine(1), Some("Color"))"#,
        r#"b::label("Layout", 0)"#,
        "fn c_empty(p: &b::Parent, c: &b::Ctx) {",
        "fn c_bern_player(p: &b::Parent, c: &b::Ctx) {",
        r#"b::SessionDef { name: "URL", fields: vec!["#,
        r#"b::SessionDef { name: "Window", fields: vec!["#,
    ] {
        assert!(ui.contains(expected), "ui.rs lacks `{expected}`");
    }

    let gtk = Command::new("pkg-config")
        .args(["--exists", "gtk4 >= 4.14", "libadwaita-1 >= 1.5"])
        .status()
        .is_ok_and(|status| status.success());
    if !gtk {
        eprintln!("GTK 4.14 and libadwaita 1.5 not installed; skipping the type check");
        return;
    }
    let result = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(output.join("app/Cargo.toml"))
        .arg("--target-dir")
        .arg(root.join("target/bern-glue-linux"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "rustc rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
