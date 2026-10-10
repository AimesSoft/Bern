//! The `web + browser` target.
//!
//! Generates a static page from the shared IR: HTML, the Bern Web
//! stylesheet with theme variables from the Bern theme kernel, the
//! JavaScript runtime and the generated application module. Windows are
//! virtual windows inside the page. See `Docs/Adaptation/Web.md` for the
//! visual spec it implements.
//!
//! * `web_mode = "app"`: the engine, the UI session runtime and the C ABI
//!   run in the page as one WebAssembly module.
//! * `web_mode = "ui"`: the page runs the UI session runtime only and
//!   reaches the engine of a separate engine host over WebSocket; the build
//!   also produces that host, which serves the page, and the CLI and MCP
//!   programs that reach the same engine through its IPC channel.

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
            channels: &["ipc"],
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
        let ui = match config.web_mode.as_deref() {
            Some("app") => false,
            Some("ui") => true,
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: set `web_mode = \"app\"` or `\"ui\"`",
                    config.name
                )));
                false
            }
        };
        for channel in &config.channels {
            if !ui {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: channel `{channel}` needs a process that can listen; the `app` build runs only in the page (use `web_mode = \"ui\"`, whose engine host assembles it)",
                    config.name
                )));
            } else if channel != "ipc" {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: unknown channel `{channel}`",
                    config.name
                )));
            }
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
        let remote = context.target.web_mode.as_deref() == Some("ui");
        let mut files = vec![
            GeneratedFile::text("index.html", index_html(app)),
            GeneratedFile::text("bern.js", RUNTIME),
            GeneratedFile::text("bern.css", STYLES),
            GeneratedFile::text("theme.css", theme::stylesheet(app)),
            GeneratedFile::text(
                "app.js",
                Generator::new(app).generate(
                    app.protocol.fingerprint(),
                    &strings,
                    WASM,
                    if remote { "ui" } else { "app" },
                ),
            ),
            GeneratedFile::text(
                "glue/page/Cargo.toml",
                glue::page_cargo_toml(context, app, remote),
            ),
            GeneratedFile::text(
                "glue/page/src/lib.rs",
                glue::page_lib_rs(context, app, remote),
            ),
        ];
        let mut crates = vec!["glue/page"];
        if remote {
            files.extend([
                GeneratedFile::text("glue/host/Cargo.toml", glue::host_cargo_toml(context, app)),
                GeneratedFile::text("glue/host/src/host.rs", glue::host_rs(context, app)),
                GeneratedFile::text("glue/host/src/cli.rs", glue::cli_rs(context, app)),
                GeneratedFile::text("glue/host/src/mcp.rs", glue::mcp_rs(context, app)),
            ]);
            crates.push("glue/host");
        }
        // Pin the generated crates to the workspace's dependency versions.
        let lock = context.workspace_dir.join("Cargo.lock");
        if let Ok(contents) = std::fs::read(&lock) {
            for dir in crates {
                let path = PathBuf::from(dir).join("Cargo.lock");
                if !context.output_dir.join(&path).exists() {
                    files.push(GeneratedFile {
                        path,
                        contents: contents.clone(),
                    });
                }
            }
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
            "engine_deployment": if remote { "engine-host" } else { "in-page" },
            "application_transport": if remote { "websocket" } else { "in-process" },
            "application_codec": if remote { Some("json") } else { None },
            "channels": context.target.channels,
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
        let remote = context.target.web_mode.as_deref() == Some("ui");
        let profile = if context.release { "release" } else { "debug" };
        // Separate target directories, so switching modes never mixes an
        // engine-carrying module with a WebUI one.
        let mode = if remote { "ui" } else { "app" };
        let page_target = context
            .workspace_dir
            .join(format!("target/bern-web-glue/{mode}"));
        let module = page_target
            .join(RUST_TARGET)
            .join(profile)
            .join("bern_web_app.wasm");
        let cargo = |manifest: &str, target_dir: &PathBuf, rust_target: Option<&str>| {
            let mut args = vec![
                "build".to_string(),
                "--manifest-path".into(),
                output.join(manifest).display().to_string(),
                "--target-dir".into(),
                target_dir.display().to_string(),
            ];
            if let Some(rust_target) = rust_target {
                args.push("--target".into());
                args.push(rust_target.into());
            }
            if context.release {
                args.push("--release".into());
            }
            args
        };
        let mut steps = vec![
            BuildStep::Run {
                description: if remote {
                    "Build the page runtime for WebAssembly".into()
                } else {
                    "Build the engine and runtime for WebAssembly".into()
                },
                program: "cargo".into(),
                args: cargo("glue/page/Cargo.toml", &page_target, Some(RUST_TARGET)),
                cwd: output.join("glue/page"),
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
        let mut products = vec![output.join("index.html")];
        if remote {
            let host_target = context.workspace_dir.join("target/bern-web-glue/host");
            let built = host_target.join(profile);
            let program = glue::program(app);
            steps.push(BuildStep::Run {
                description: "Build the engine host, CLI and MCP server".into(),
                program: "cargo".into(),
                args: cargo("glue/host/Cargo.toml", &host_target, None),
                cwd: output.join("glue/host"),
                env: vec![],
            });
            steps.push(BuildStep::CreateDir {
                path: output.join("bin"),
            });
            for name in [
                format!("{program}-host"),
                program.clone(),
                format!("{program}-mcp"),
            ] {
                steps.push(BuildStep::Copy {
                    from: built.join(&name),
                    to: output.join("bin").join(&name),
                });
                products.push(output.join("bin").join(&name));
            }
        }
        Ok(BuildPlan { steps, products })
    }
}
