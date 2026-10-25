//! Icons `WindowSet(icon:)` switches to, as this target ships them.

use bern_backend_web::WebBrowser;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The gallery choosing its icon by its accent enum: every variant names
/// an alternate icon, a 1024-pixel PNG for every platform.
fn gallery_with_alternates(art: &Path) -> (bern_compiler::Config, bern_compiler::Compiled) {
    let root = workspace();
    let config = bern_compiler::config::load(&root.join("examples/gallery/Bern.toml")).unwrap();
    let mut compiled =
        bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    let _ = std::fs::remove_dir_all(art);
    std::fs::create_dir_all(art).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend_from_slice(&1024u32.to_be_bytes());
    png.extend_from_slice(&1024u32.to_be_bytes());
    std::fs::write(art.join("alt.png"), png).unwrap();
    compiled.app.windows.icon = compiled.app.windows.accent.clone();
    let picture = art.join("alt.png").display().to_string();
    for (variant, _) in bern_target_api::icon::choices(&compiled.app) {
        compiled.app.app.alternate_icons.insert(
            bern_target_api::icon::alternate_name(&variant),
            [("default".to_string(), picture.clone())].into(),
        );
    }
    (config, compiled)
}

fn context(
    config: &bern_compiler::Config,
    compiled: &bern_compiler::Compiled,
    target: &str,
    output: &Path,
) -> BuildContext {
    let root = workspace();
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.to_path_buf(),
        target: config
            .targets
            .iter()
            .find(|candidate| candidate.name == target)
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
    }
}

fn text<'a>(files: &'a [bern_target_api::GeneratedFile], suffix: &str) -> &'a str {
    let file = files
        .iter()
        .find(|file| {
            file.path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with(suffix)
        })
        .unwrap_or_else(|| panic!("no generated `{suffix}`"));
    std::str::from_utf8(&file.contents).unwrap()
}

fn has(files: &[bern_target_api::GeneratedFile], suffix: &str) -> bool {
    files.iter().any(|file| {
        file.path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with(suffix)
    })
}

/// Each icon's links wait in a template the runtime swaps in.
#[test]
fn alternate_icons_wait_in_templates() {
    let root = workspace();
    let art = root.join("target/bern-test/web-alternate-icons");
    let (config, compiled) = gallery_with_alternates(&art);
    let output = root.join("target/bern-test/web-alternate-icons-out");
    let files = WebBrowser
        .generate(
            &compiled.app,
            &context(&config, &compiled, "web_app", &output),
        )
        .unwrap();
    assert!(has(&files, "icons-crimson/icon.png"));
    let index = text(&files, "index.html");
    assert!(
        index.contains("<template id=\"bern-icon-neon\">"),
        "{index}"
    );
    assert!(index.contains("href=\"icons-neon/icon.png?v="), "{index}");
    let app = text(&files, "app.js");
    assert!(app.contains("F.appIcon(() => "), "{app}");
    assert!(
        app.contains("Crimson: \"crimson\"") || app.contains("\"Crimson\": \"crimson\""),
        "{app}"
    );
    let _ = std::fs::remove_dir_all(&art);
}
