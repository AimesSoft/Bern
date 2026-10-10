//! The `web + browser` target.
//!
//! Generates a static page from the shared IR: HTML, the Bern Web
//! stylesheet with theme variables from the Bern theme kernel, the
//! JavaScript runtime and the generated application module. The `app`
//! build compiles the engine, the UI session runtime and the C ABI to one
//! WebAssembly module the page runs; windows are virtual windows inside the
//! page. See `Docs/Adaptation/Web.md` for the visual spec it implements.

mod glue;
mod icons;
mod js;
mod strings;
mod theme;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node};
use bern_protocol::Type;
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport,
    Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common,
};
use std::path::PathBuf;

pub use js::Generator;

const RUNTIME: &str = include_str!("../templates/bern.js");
const STYLES: &str = include_str!("../templates/bern.css");
const WASM: &str = "app.wasm";
const RUST_TARGET: &str = "wasm32-unknown-unknown";

pub struct WebBrowser;

fn component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| COMPONENTS.iter().map(|spec| spec.name).collect())
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn index_html(app: &Application) -> String {
    let title = app
        .app
        .name
        .get(&app.languages.default)
        .unwrap_or(&app.app.id);
    format!(
        r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<link rel="stylesheet" href="theme.css">
<link rel="stylesheet" href="bern.css">
<script type="module" src="app.js"></script>
</head>
<body>
<div id="bern-host" class="bern-host"></div>
</body>
</html>
"#,
        lang = html_escape(&app.languages.default),
        title = html_escape(title),
    )
}

/// Walk every node of the application with its parent component kind.
fn walk(app: &Application, visit: &mut dyn FnMut(&Node, Option<&str>)) {
    fn inner(node: &Node, parent: Option<&str>, visit: &mut dyn FnMut(&Node, Option<&str>)) {
        visit(node, parent);
        let name = match node {
            Node::Element(element) => Some(element.kind.as_str()),
            _ => parent,
        };
        match node {
            Node::Element(element) => {
                for child in &element.children {
                    inner(child, name, visit);
                }
            }
            Node::If(node) => {
                for child in node
                    .branches
                    .iter()
                    .flat_map(|(_, body)| body)
                    .chain(&node.otherwise)
                {
                    inner(child, name, visit);
                }
            }
            Node::IfLet(node) => {
                for child in node.then.iter().chain(&node.otherwise) {
                    inner(child, name, visit);
                }
            }
            Node::Match(node) => {
                for child in node.arms.iter().flat_map(|arm| &arm.body) {
                    inner(child, name, visit);
                }
            }
            Node::Component(_) => {}
        }
    }
    for component in &app.components {
        inner(&component.body, None, visit);
    }
    for window in &app.windows.windows {
        inner(&window.content, None, visit);
    }
}

fn nested_optional(ty: &Type) -> bool {
    match ty {
        Type::Optional { inner } => {
            matches!(**inner, Type::Optional { .. }) || nested_optional(inner)
        }
        Type::List { inner } => nested_optional(inner),
        _ => false,
    }
}

impl Target for WebBrowser {
    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
            id: "web-browser",
            ui: "web",
            platforms: &["browser"],
            version: env!("CARGO_PKG_VERSION"),
            ir_versions: IR_VERSION..=IR_VERSION,
            color_source: ColorSource::BernTheme,
            components: component_names(),
            windows: WindowCapabilities {
                native: false,
                virtual_windows: true,
            },
            channels: &[],
            media: Some(MediaSupport {
                contract_versions: bern_media::MEDIA_CONTRACT_VERSION
                    ..=bern_media::MEDIA_CONTRACT_VERSION,
                backend: "html-media-element",
                backend_version: "1",
                presenter: "html-video",
                presenter_version: "1",
                background_playback: true,
                multi_view: false,
            }),
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let mut diagnostics = check_common(self, app);
        match config.web_mode.as_deref() {
            Some("app") => {}
            Some("ui") => diagnostics.push(Diagnostic::error(format!(
                "target `{}`: the WebUI build (`web_mode = \"ui\"`) is not implemented yet",
                config.name
            ))),
            _ => diagnostics.push(Diagnostic::error(format!(
                "target `{}`: set `web_mode = \"app\"` or `\"ui\"`",
                config.name
            ))),
        }
        for channel in &config.channels {
            diagnostics.push(Diagnostic::error(format!(
                "target `{}`: channel `{channel}` is not available in a browser; the page cannot listen for local connections",
                config.name
            )));
        }
        let framework = strings::framework_languages();
        for language in &app.languages.supported {
            if !framework.contains(&language.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "the Web runtime has no `{language}` translation of its own texts; supported: {}",
                    framework.join(", ")
                )));
            }
        }
        for problem in theme::problems(app) {
            diagnostics.push(Diagnostic::error(problem));
        }
        let mut visit = |node: &Node, _: Option<&str>| {
            if let Node::Element(element) = node {
                for (name, prop) in &element.props {
                    if let bern_ir::Prop::Icon { name: icon } = prop
                        && icons::svg(icon).is_none()
                    {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "the Web icon set has no `{icon}` (property `{name}` of `{}`)",
                                element.kind
                            ))
                            .at(element.source.clone()),
                        );
                    }
                }
                if element.kind == "Picker"
                    && let Some(field) = element.field("value")
                    && let Type::Enum { name } = &field.ty
                    && app
                        .types
                        .enum_def(name)
                        .is_some_and(|def| !def.is_unit_only())
                {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "`Picker` lists the variants of `{name}`, which must not carry data"
                        ))
                        .at(element.source.clone()),
                    );
                }
            }
        };
        walk(app, &mut visit);
        let mut types: Vec<&Type> = app.protocol.state.iter().map(|field| &field.ty).collect();
        types.extend(
            app.sessions
                .iter()
                .flat_map(|session| session.fields.iter().map(|field| &field.ty)),
        );
        if types.iter().any(|ty| nested_optional(ty)) {
            diagnostics.push(Diagnostic::error(
                "the Web target does not support an optional of an optional",
            ));
        }
        diagnostics
    }

    fn generate(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
        let strings = strings::tables(app);
        let mut files = vec![
            GeneratedFile::text("index.html", index_html(app)),
            GeneratedFile::text("bern.js", RUNTIME),
            GeneratedFile::text("bern.css", STYLES),
            GeneratedFile::text("theme.css", theme::stylesheet(app)),
            GeneratedFile::text(
                "app.js",
                Generator::new(app).generate(app.protocol.fingerprint(), &strings, WASM),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app)),
        ];
        let lock = context.workspace_dir.join("Cargo.lock");
        if let Ok(contents) = std::fs::read(&lock)
            && !context.output_dir.join("glue/Cargo.lock").exists()
        {
            files.push(GeneratedFile {
                path: "glue/Cargo.lock".into(),
                contents,
            });
        }
        let manifest = serde_json::json!({
            "target": self.descriptor().id,
            "ui": "web",
            "platform": "browser",
            "web_mode": context.target.web_mode,
            "ir_version": app.ir_version,
            "generator": "bern-backend-web",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "profile": if context.release { "release" } else { "debug" },
            "engine_deployment": "in-page",
            "ui_bridge": "wasm-c-abi",
            "color_source": "bern-theme",
            "theme_algorithm": bern_theme::ALGORITHM_VERSION,
            "theme_mode": app.theme.mode,
            "protocol": app.protocol.namespace,
            "protocol_fingerprint": format!("{:016x}", app.protocol.fingerprint()),
            "languages": app.languages.supported,
            "resources": app.resources.iter().map(|resource| &resource.name).collect::<Vec<_>>(),
        });
        files.push(GeneratedFile::text(
            "build.json",
            serde_json::to_string_pretty(&manifest).expect("manifest"),
        ));
        Ok(files)
    }

    fn plan(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let output = &context.output_dir;
        let profile = if context.release { "release" } else { "debug" };
        let glue_target = context.workspace_dir.join("target/bern-web-glue");
        let module = glue_target
            .join(RUST_TARGET)
            .join(profile)
            .join("bern_web_app.wasm");
        let mut cargo = vec![
            "build".to_string(),
            "--manifest-path".into(),
            output.join("glue/Cargo.toml").display().to_string(),
            "--target".into(),
            RUST_TARGET.into(),
            "--target-dir".into(),
            glue_target.display().to_string(),
        ];
        if context.release {
            cargo.push("--release".into());
        }
        let steps = vec![
            BuildStep::Run {
                description: "Build the engine and runtime for WebAssembly".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("glue"),
                env: vec![],
            },
            BuildStep::Copy {
                from: module,
                to: output.join(WASM),
            },
            // Declared resources are served below `resources/`.
            BuildStep::Mirror {
                dir: output.join("resources"),
                files: app
                    .resources
                    .iter()
                    .map(|resource| {
                        (
                            context.project_dir.join(&resource.name),
                            PathBuf::from(&resource.name),
                        )
                    })
                    .collect(),
                keep: vec![],
            },
        ];
        Ok(BuildPlan {
            steps,
            products: vec![output.join("index.html")],
        })
    }
}
