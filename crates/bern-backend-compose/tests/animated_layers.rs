//! Conditional transitions keep the surrounding layout's axis and receivers.
use bern_backend_compose::ComposeAndroid;
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};

#[test]
fn animated_branches_preserve_layers_and_rows() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let dir = target.join(format!("bern-test/animated-layers-{}", std::process::id()));
    for (name, source) in [
        ("probe-protocol", PROTOCOL),
        ("probe-ui", UI),
        ("probe-core", ""),
    ] {
        std::fs::create_dir_all(dir.join(name).join("src")).unwrap();
        std::fs::write(
            dir.join(name).join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join(name).join("src/lib.rs"), source).unwrap();
    }
    std::fs::write(dir.join("Bern.toml"), MANIFEST).unwrap();
    let config = bern_compiler::config::load(&dir.join("Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| panic!("{errors:?}"));
    let target = config
        .targets
        .iter()
        .find(|target| target.platform == "android")
        .unwrap()
        .clone();
    let context = BuildContext {
        project_dir: dir.clone(),
        workspace_dir: workspace.clone(),
        output_dir: dir.join("generated"),
        target,
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: Vec::new(),
        framework: Framework::Path(workspace.join("crates")),
        release: false,
    };
    let files = ComposeAndroid.generate(&compiled.app, &context).unwrap();
    let kotlin = std::str::from_utf8(
        &files
            .iter()
            .find(|f| f.path.ends_with("BernGenerated.kt"))
            .unwrap()
            .contents,
    )
    .unwrap();
    let layers = kotlin
        .split("fun AppView_layers(")
        .nth(1)
        .unwrap()
        .split("/// Generated from")
        .next()
        .unwrap();
    assert!(layers.contains("AnimatedContent("), "{layers}");
    assert!(
        !layers.contains("Column {"),
        "a conditional layer is not a column:\n{layers}"
    );
    assert_eq!(
        layers
            .matches("Box(contentAlignment = Alignment.TopStart)")
            .count(),
        2,
        "both branches need their own layer scope:\n{layers}"
    );
    assert!(
        layers.contains("Canvas(Modifier.matchParentSize()"),
        "an overlay takes its sibling's size:\n{layers}"
    );
    assert!(
        layers.contains("Canvas(Modifier.bernFrame(null, null)"),
        "a lone backdrop still fills available space:\n{layers}"
    );
    let row = kotlin
        .split("fun AppView_row(")
        .nth(1)
        .unwrap()
        .split("/// Generated from")
        .next()
        .unwrap();
    assert!(row.contains("AnimatedContent("), "{row}");
    assert!(
        row.contains("Row {"),
        "a row branch stays horizontal:\n{row}"
    );
    assert!(!row.contains("Column {"), "{row}");
}

const MANIFEST: &str = r#"
[app]
id = "probe"
bundle_id = "dev.bern.probe"
version = "0.1.0"
name = { "en" = "Probe" }
[languages]
supported = ["en"]
default = "en"
[protocol]
package = "probe-protocol"
source = "probe-protocol/src/lib.rs"
[engine]
package = "probe-core"
type = "probe_core::Engine"
[ui]
package = "probe-ui"
sources = ["probe-ui/src/lib.rs"]
root = "app"
[targets.android]
platform = "android"
"#;
const PROTOCOL: &str = r#"
#[bern::protocol(namespace = "probe", version = 1)]
pub mod probe {
    use bern::prelude::*;
    #[state] pub struct State { pub count: i64 }
    #[commands] pub trait Commands { fn add(&mut self) -> Result<(), CommandError>; }
}
"#;
const UI: &str = r#"
use bern::prelude::*;
#[component]
pub fn app(app: probe::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Probe" }, initial: open) {
                Column { layers(app: app) row(app: app) }
            }
        }
    }
}
#[component]
pub fn layers(app: probe::Client) -> impl View {
    ui! {
        Stack(align: top_leading) {
            if app.count > 0 {
                Text(content: tr! { "en" => "Foreground" }, appear: fade)
                Canvas { Shape(kind: rect, fill: accent, opacity: 0.3) }
            } else {
                Canvas { Shape(kind: rect, fill: surface.canvas) }
            }
        }
    }
}
#[component]
pub fn row(app: probe::Client) -> impl View {
    ui! {
        Row {
            if app.count > 0 {
                Text(content: tr! { "en" => "First" }, appear: fade)
                Text(content: tr! { "en" => "Second" })
            }
        }
    }
}
"#;
