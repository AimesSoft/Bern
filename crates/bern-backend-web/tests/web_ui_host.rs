//! The WebUI's engine host starts an engine that keeps its settings and data,
//! as a native host's does.

use bern_backend_web::WebBrowser;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn the_host_installs_the_applications_folders() {
    let root = workspace();
    let config = bern_compiler::config::load(&root.join("examples/planner/Bern.toml")).unwrap();
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
        .find(|target| target.web_mode.as_deref() == Some("ui"))
        .unwrap()
        .clone();
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/web-ui-host"),
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
    let files = WebBrowser.generate(&compiled.app, &context).unwrap();
    let host = files
        .iter()
        .find(|file| file.path == Path::new("glue/host/src/host.rs"))
        .unwrap();
    let host = String::from_utf8_lossy(&host.contents);
    let installed = format!(
        "Store::install_application({:?})",
        compiled.app.app.bundle_id
    );
    assert!(host.contains(&installed), "{host}");
}
