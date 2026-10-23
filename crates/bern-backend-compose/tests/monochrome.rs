//! Execute the actual Compose theme helper on the JVM, so a new Material
//! role cannot quietly reintroduce the default purple into a gray scheme.
//! Opt in with BERN_TEST_KOTLIN=1; this needs Gradle and the Android SDK.

use bern_backend_compose::{ComposeAndroid, toolchain};
use bern_target_api::{BuildContext, Framework, Target};
use std::path::Path;
use std::process::Command;

#[test]
fn gray_accents_have_achromatic_material_roles_and_readable_pairs() {
    if std::env::var_os("BERN_TEST_KOTLIN").is_none() {
        eprintln!("BERN_TEST_KOTLIN is not set; skipping the JVM color assertions");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config = bern_compiler::config::load(&root.join("examples/hello/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap();
    let output = root.join("target/bern-test/compose-monochrome");
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: config
            .targets
            .iter()
            .find(|t| t.platform == "android")
            .unwrap()
            .clone(),
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
    for file in ComposeAndroid.generate(&compiled.app, &context).unwrap() {
        let path = output.join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.contents).unwrap();
    }
    let project = output.join("project");
    let build = project.join("app/build.gradle.kts");
    let mut text = std::fs::read_to_string(&build).unwrap();
    text.push_str("\ndependencies { testImplementation(\"junit:junit:4.13.2\") }\n");
    std::fs::write(build, text).unwrap();
    let test = project.join("app/src/test/kotlin/dev/bern/runtime/BernMonochromeTest.kt");
    std::fs::create_dir_all(test.parent().unwrap()).unwrap();
    std::fs::write(test, include_str!("fixtures/BernMonochromeTest.kt")).unwrap();
    let (Some(gradle), Some(sdk)) = (toolchain::gradle(), toolchain::sdk()) else {
        panic!("BERN_TEST_KOTLIN needs Gradle and the Android SDK");
    };
    let mut command = Command::new(gradle);
    command
        .arg("--project-dir")
        .arg(project)
        .args(["--console=plain", "--quiet", ":app:testDebugUnitTest"])
        .env("ANDROID_HOME", sdk);
    if let Some(java) = toolchain::java_home() {
        command.env("JAVA_HOME", java);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "Compose color tests failed:\n{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
