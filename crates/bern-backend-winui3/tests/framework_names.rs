//! The `framework-names` example names its value types like types of
//! WinUI, WinRT and the other platforms' frameworks (`Window`, `Button`,
//! `Image`, `Color`, `Orientation`, `Text`, `Error`…). The generated C++
//! describes the types to the runtime by name, as strings, and declares
//! no C++ type for them; on Windows with the Visual Studio C++ tools, the
//! compiler builds both front ends.

use bern_backend_winui3::WinUi3Windows;
use bern_target_api::{BuildContext, Framework, Severity, Target};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    // Without the verbatim prefix Windows gives canonical paths.
    match root.to_string_lossy().strip_prefix(r"\\?\") {
        Some(plain) => PathBuf::from(plain),
        None => root,
    }
}

/// Compile the generated sources on Windows where the Visual Studio C++
/// tools are installed. Linking needs the engine library, which `bern
/// build` makes; compiling both front ends checks the generator.
#[cfg(windows)]
fn compile(output: &Path) {
    use std::process::Command;
    let vswhere = Path::new(&std::env::var("ProgramFiles(x86)").unwrap_or_default())
        .join("Microsoft Visual Studio/Installer/vswhere.exe");
    if !vswhere.is_file() {
        eprintln!("the Visual Studio C++ tools are not installed; skipping the compile check");
        return;
    }
    let result = Command::new("cmd")
        .args(["/d", "/c"])
        .arg(output.join("app/build.bat"))
        .arg("compile")
        .current_dir(output.join("app"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "the C++ compiler rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[cfg(not(windows))]
fn compile(_: &Path) {
    eprintln!("not on Windows; skipping the compile check");
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
        .find(|target| target.platform == "windows")
        .unwrap()
        .clone();
    let problems: Vec<_> = WinUi3Windows
        .check(&compiled.app, &target)
        .into_iter()
        .filter(|problem| problem.severity != Severity::Note)
        .collect();
    assert!(problems.is_empty(), "{problems:#?}");
    let output = root.join("target/bern-test/winui3-framework-names");
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
    for file in WinUi3Windows.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    let catalog = std::fs::read_to_string(output.join("app/app_catalog.cpp")).unwrap();
    for expected in [
        r#"BType::Enum("Layout")"#,
        r#"BType::Enum("Window")"#,
        r#"BType::Record("Text")"#,
        r#"BType::Record("Image")"#,
    ] {
        assert!(catalog.contains(expected), "app_catalog.cpp lacks `{expected}`");
    }
    compile(&output);
}
