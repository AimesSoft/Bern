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
mod style;
mod theme;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node};
use bern_protocol::Type;
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, Target, TargetConfig, TargetDescriptor, WindowCapabilities, archive, check_common,
    dist_dir, files_below, icon, package_architecture,
};
use std::path::PathBuf;

pub use js::Generator;

const RUNTIME: &str = include_str!("../templates/bern.js");
const FEATURE_RUNTIME: &str = include_str!("../templates/features.js");
const STYLES: &str = include_str!("../templates/bern.css");
const FEATURE_STYLES: &str = include_str!("../templates/features.css");
const WASM: &str = "app.wasm";
const RUST_TARGET: &str = "wasm32-unknown-unknown";

pub struct WebBrowser;

/// Components this target does not build yet.
/// IR features the Web builds.
const FEATURES: &[&str] = &[
    bern_target_api::feature::BUTTON_CONTENT,
    bern_target_api::feature::REVEAL,
    bern_target_api::feature::TOOLBAR_SEARCH,
    bern_target_api::feature::SEARCH_RESULTS,
    bern_target_api::feature::SESSION_ACTIONS,
    bern_target_api::feature::TOGGLE_BUTTON,
    bern_target_api::feature::DIALOG_WINDOWS,
    bern_target_api::feature::BOUND_LENGTHS,
    bern_target_api::feature::APPEARANCE,
    bern_target_api::feature::SHORTCUTS,
    bern_target_api::feature::RUNTIME_THEME,
    bern_target_api::feature::RUNTIME_IMAGES,
    bern_target_api::feature::PICTURE_FILLS,
    bern_target_api::feature::FILE_PICK,
    bern_target_api::feature::SCROLL_SNAP,
    bern_target_api::feature::RAILS,
    bern_target_api::feature::POPOVER_BUTTON,
    bern_target_api::feature::SIZE_CLASSES,
    bern_target_api::feature::TEXT_WEIGHTS,
    bern_target_api::feature::IMAGE_CORNERS,
    bern_target_api::feature::SIDEBAR_WIDTH,
    bern_target_api::feature::IMAGE_TEMPLATE,
    bern_target_api::feature::PAGE_BADGES,
    bern_target_api::feature::EDGE_GROW,
    bern_target_api::feature::DELAYED_APPEAR,
    bern_target_api::feature::AXIS_PADDING,
    bern_target_api::feature::SIDEBAR_HEADER,
    bern_target_api::feature::UNTITLED_DETAIL,
    bern_target_api::feature::IDLE_CONTROL,
    bern_target_api::feature::TEXT_LINES,
    bern_target_api::feature::CONTROLLED_VALUES,
    bern_target_api::feature::SEARCH_PALETTE,
    bern_target_api::feature::ADAPTIVE_IMAGES,
    bern_target_api::feature::ACCORDION_LAYOUT,
    bern_target_api::feature::SCALED_SCENE,
    bern_target_api::feature::CAPSULE_CONTROLS,
    bern_target_api::feature::MODAL_FLOW_TIMER,
    bern_target_api::feature::TOOLBAR_LEADING,
    bern_target_api::feature::ICON_MENUS,
    bern_target_api::feature::ICON_LABELS,
    bern_target_api::feature::ROW_BASELINE,
];

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

/// A version for this build's page files. Browsers and static hosts cache
/// them; URLs carrying the version make every rebuild load fresh files.
fn build_version() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{:x}", nanos as u64)
}

/// The page icons: the Bern logo as SVG with a PNG beside it and a square
/// one for home screens, or the declared PNG for all three uses. Returns
/// the files and their `<link>` elements.
fn page_icons(
    app: &Application,
    context: &BuildContext,
    version: &str,
) -> Result<(Vec<GeneratedFile>, String), Vec<Diagnostic>> {
    let failed = |message: String| vec![Diagnostic::error(message)];
    let round = icon::images(app, &context.project_dir, icon::Shape::AsDrawn).map_err(failed)?;
    let largest = icon::largest(&round);
    let version = html_escape(version);
    let png = format!(
        "<link rel=\"icon\" type=\"image/png\" sizes=\"{size}x{size}\" href=\"icon.png?v={version}\">\n",
        size = largest.size
    );
    let mut files = vec![GeneratedFile {
        path: "icon.png".into(),
        contents: largest.png.to_vec(),
    }];
    if !icon::is_default(app) {
        return Ok((
            files,
            format!("{png}<link rel=\"apple-touch-icon\" href=\"icon.png?v={version}\">\n"),
        ));
    }
    let square = icon::default_images(icon::Shape::Masked);
    let touch = square
        .iter()
        .find(|image| image.size == 256)
        .expect("the logo has a 256 pixel square");
    files.push(GeneratedFile::text("favicon.svg", icon::DEFAULT_SVG));
    files.push(GeneratedFile {
        path: "apple-touch-icon.png".into(),
        contents: touch.png.to_vec(),
    });
    Ok((
        files,
        format!(
            "<link rel=\"icon\" type=\"image/svg+xml\" href=\"favicon.svg?v={version}\">\n{png}<link rel=\"apple-touch-icon\" href=\"apple-touch-icon.png?v={version}\">\n"
        ),
    ))
}

fn index_html(app: &Application, version: &str, icons: &str) -> String {
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
{icons}<link rel="stylesheet" href="theme.css?v={version}">
<link rel="stylesheet" href="bern.css?v={version}">
<script type="module" src="app.js?v={version}"></script>
</head>
<body>
<div id="bern-host" class="bern-host"></div>
</body>
</html>
"#,
        lang = html_escape(&app.languages.default),
        title = html_escape(title),
        version = html_escape(version),
    )
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
            styles: style::SUPPORT,
            motion: &[
                "appear",
                "disappear",
                "reflow",
                "hero",
                "animate",
                "scroll_scale",
                "scroll_opacity",
                "nudge",
                "stagger",
                "appear_delay",
                "nudge_key",
                "nudge_scale",
                "reduce_motion",
            ],
            window_chrome: true,
            text_marks: true,
            dynamic_items: true,
            features: FEATURES,
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
                floating: true,
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
        let framework = bern_target_api::texts::framework_languages();
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
        app.walk(&mut visit);
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
        bern_target_api::svg::validate(app, context)?;
        let strings = strings::tables(app);
        let remote = context.target.web_mode.as_deref() == Some("ui");
        let version = build_version();
        let (icon_files, icon_links) = page_icons(app, context, &version)?;
        let mut files = vec![
            GeneratedFile::text("index.html", index_html(app, &version, &icon_links)),
            GeneratedFile::text("bern.js", RUNTIME),
            // It imports `bern.js` by the same versioned URL as the page, so
            // both share one runtime.
            GeneratedFile::text(
                "features.js",
                format!(
                    "{}{}",
                    FEATURE_RUNTIME.replace("__BERN_VERSION__", &version),
                    if bern_target_api::svg::used(app) {
                        bern_target_api::svg::template(include_str!("../templates/svg.js"), false)
                    } else {
                        String::new()
                    }
                ),
            ),
            GeneratedFile::text("bern.css", format!("{STYLES}\n{FEATURE_STYLES}")),
            GeneratedFile::text("theme.css", theme::stylesheet(app)),
            GeneratedFile::text("THIRD_PARTY_NOTICES.txt", icons::NOTICES),
            GeneratedFile::text(
                "app.js",
                Generator::new(app).generate(
                    app.protocol.fingerprint(),
                    &strings,
                    &format!("{WASM}?v={version}"),
                    if remote { "ui" } else { "app" },
                    &version,
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
        files.extend(icon_files);
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
        if !remote {
            // A target switched from `ui` to `app` must not keep the engine
            // host and its programs next to a page that no longer uses them.
            steps.push(BuildStep::RemoveIfExists {
                path: output.join("bin"),
            });
            steps.push(BuildStep::RemoveIfExists {
                path: output.join("glue/host"),
            });
        }
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
                let name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
                steps.push(BuildStep::Copy {
                    from: built.join(&name),
                    to: output.join("bin").join(&name),
                });
                products.push(output.join("bin").join(&name));
            }
        }
        let run = if remote {
            let host = format!(
                "{}-host{}",
                glue::program(app),
                std::env::consts::EXE_SUFFIX
            );
            Run::PageHost(output.join("bin").join(host))
        } else {
            Run::Page(output.clone())
        };
        Ok(BuildPlan {
            steps,
            products,
            notes: Vec::new(),
            run: Some(run),
        })
    }

    /// The files a server needs, in one archive: the page and its assets
    /// for an `app` build; with a `ui` build also the engine host and its
    /// command line and MCP server, for this machine.
    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        _signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let output = &context.output_dir;
        let remote = context.target.web_mode.as_deref() == Some("ui");
        let program = glue::program(app);
        let dist = dist_dir(context);
        let staged = output.join("package").join(&program);
        // Generated sources and build reports stay behind.
        let files: Vec<_> = files_below(output)
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot read {}: {error}",
                    output.display()
                ))]
            })?
            .into_iter()
            .filter(|(_, path)| {
                let top = path.components().next().map(|part| part.as_os_str());
                !matches!(
                    top.and_then(|top| top.to_str()),
                    Some("glue" | "package" | "build.json" | "features.json")
                )
            })
            .collect();
        let stem = if remote {
            format!(
                "{program}-{}-webui-{}-{}",
                app.app.version,
                std::env::consts::OS,
                package_architecture()
            )
        } else {
            format!("{program}-{}-web", app.app.version)
        };
        let (archive_step, archive_file) = archive(&staged, &dist.join(stem));
        Ok(BuildPlan {
            steps: vec![
                BuildStep::RemoveIfExists {
                    path: output.join("package"),
                },
                BuildStep::CreateDir { path: dist.clone() },
                BuildStep::Mirror {
                    dir: staged,
                    files,
                    keep: vec![],
                },
                BuildStep::RemoveIfExists {
                    path: archive_file.clone(),
                },
                archive_step,
            ],
            products: vec![archive_file],
            run: None,
            notes: Vec::new(),
        })
    }
}
