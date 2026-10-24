//! The gallery in the browser: a television's layout and margins, the
//! dialog's first focus, and Escape closing the details while they show.

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
fn gallery_generates_television_layouts_shortcuts_and_first_focus() {
    let root = workspace();
    let config = bern_compiler::config::load(&root.join("examples/gallery/Bern.toml")).unwrap();
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
        .find(|target| target.name == "web_app")
        .unwrap()
        .clone();
    let problems = WebBrowser.check(&compiled.app, &target);
    assert!(
        problems
            .iter()
            .all(|problem| problem.severity == bern_target_api::Severity::Warning),
        "{problems:?}"
    );
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: root.join("target/bern-test/web-gallery"),
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
    let script: String = WebBrowser
        .generate(&compiled.app, &context)
        .unwrap()
        .into_iter()
        .filter(|file| {
            file.path
                .extension()
                .is_some_and(|extension| extension == "mjs" || extension == "js")
        })
        .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
        .collect();
    for expected in [
        "tv: (p, ctx) => {",
        "ctx.win.tv.get()",
        "defaultFocus: true",
        "F.KeyShortcut(p, ctx, { key: \"escape\" }",
        "suggest: true",
        ".peek() + \"check\")",
        ".slice(0, -1).join(\"\"))",
        // The about window's first tab shows only its symbol.
        "icon: \"info.circle\", iconOnly: true, build: (p, ctx) => {",
        // The details' keys follow state, and the user chooses them.
        "F.shortcut(() => E.details_key.get(), ",
        "F.ShortcutInput(p, ctx, { id: \"details_key\"",
    ] {
        assert!(
            script.contains(expected),
            "the gallery's script lacks `{expected}`"
        );
    }
}
