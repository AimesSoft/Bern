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
use std::path::{Path, PathBuf};

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
    bern_target_api::feature::TEXT_FORMATS,
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
    bern_target_api::feature::FILL_BUTTONS,
    bern_target_api::feature::TAB_ACCESSORY,
    bern_target_api::feature::GRID_CELLS,
    bern_target_api::feature::PAGE_SEARCH,
    bern_target_api::feature::FORM_LAYOUT,
    bern_target_api::feature::MODAL_COVER,
    bern_target_api::feature::WINDOW_SHORT,
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
    bern_target_api::feature::ICON_TABS,
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

/// The page icons, and their `<link>` elements: the Bern logo as SVG with a
/// PNG beside it and a square one for home screens; or the declared PNG for
/// all three uses; or a declared SVG; or a declared folder of page icons.
fn page_icons(
    app: &Application,
    context: &BuildContext,
    version: &str,
) -> Result<(Vec<GeneratedFile>, String), Vec<Diagnostic>> {
    let failed = |message: String| vec![Diagnostic::error(message)];
    let version = html_escape(version);
    if let icon::Source::Native(path) = icon::source(app, "browser", &context.project_dir) {
        if path.is_dir() {
            return folder_icons(&path, &version).map_err(failed);
        }
        let svg = std::fs::read(&path)
            .map_err(|error| failed(format!("cannot read the icon {}: {error}", path.display())))?;
        return Ok((
            vec![GeneratedFile {
                path: "favicon.svg".into(),
                contents: svg,
            }],
            format!(
                "<link rel=\"icon\" type=\"image/svg+xml\" href=\"favicon.svg?v={version}\">\n"
            ),
        ));
    }
    let round =
        icon::images(app, "browser", &context.project_dir, icon::Shape::AsDrawn).map_err(failed)?;
    let largest = icon::largest(&round);
    let png = format!(
        "<link rel=\"icon\" type=\"image/png\" sizes=\"{size}x{size}\" href=\"icon.png?v={version}\">\n",
        size = largest.size
    );
    let mut files = vec![GeneratedFile {
        path: "icon.png".into(),
        contents: largest.png.to_vec(),
    }];
    if !icon::is_default(app, "browser") {
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

/// A declared folder of page icons, copied to `icons/` and linked by kind:
/// `favicon.ico` for browsers without SVG icons, then SVG icons, PNG icons
/// only when there is neither (the maskable ones belong to the manifest),
/// `apple-touch-icon*.png` for home screens, and the web app manifest.
fn folder_icons(dir: &Path, version: &str) -> Result<(Vec<GeneratedFile>, String), String> {
    let names = icon::page_icons(dir)
        .map_err(|message| format!("the browser icon {} {message}", dir.display()))?;
    let mut files = Vec::new();
    let mut found: Vec<(String, String, Vec<u8>)> = Vec::new();
    for name in names {
        let shown = name.to_string_lossy().into_owned();
        let contents = std::fs::read(dir.join(&name))
            .map_err(|error| format!("cannot read {}: {error}", dir.join(&name).display()))?;
        let extension = name
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        found.push((shown, extension, contents));
    }
    let href = |name: &str| format!("icons/{name}?v={version}");
    let vector = found
        .iter()
        .any(|(_, extension, _)| extension == "svg" || extension == "ico");
    let mut ico = String::new();
    let mut svg = String::new();
    let mut png = String::new();
    let mut touch = String::new();
    let mut manifest = String::new();
    for (name, extension, contents) in &found {
        let lower = name.to_ascii_lowercase();
        match extension.as_str() {
            "ico" => ico.push_str(&format!(
                "<link rel=\"icon\" href=\"{}\" sizes=\"32x32\">\n",
                href(name)
            )),
            "svg" => svg.push_str(&format!(
                "<link rel=\"icon\" type=\"image/svg+xml\" href=\"{}\">\n",
                href(name)
            )),
            "png" if lower.starts_with("apple-touch-icon") => touch.push_str(&format!(
                "<link rel=\"apple-touch-icon\" href=\"{}\">\n",
                href(name)
            )),
            "png" if !vector && !lower.contains("maskable") => {
                let sizes = icon::png_size(contents)
                    .map(|(width, height)| format!(" sizes=\"{width}x{height}\""))
                    .unwrap_or_default();
                png.push_str(&format!(
                    "<link rel=\"icon\" type=\"image/png\"{sizes} href=\"{}\">\n",
                    href(name)
                ));
            }
            "webmanifest" | "json" => manifest.push_str(&format!(
                "<link rel=\"manifest\" href=\"{}\">\n",
                href(name)
            )),
            _ => {}
        }
    }
    for (name, _, contents) in found {
        files.push(GeneratedFile {
            path: PathBuf::from("icons").join(name),
            contents,
        });
    }
    Ok((files, format!("{ico}{svg}{png}{touch}{manifest}")))
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

/// The engine host, the command line and the MCP server of a `ui` build,
/// as their files in `bin/` are named on this machine.
fn programs(app: &Application) -> [String; 3] {
    let program = glue::program(app);
    [
        format!("{program}-host"),
        program.clone(),
        format!("{program}-mcp"),
    ]
    .map(|name| format!("{name}{}", std::env::consts::EXE_SUFFIX))
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
                "scroll_origin_x",
                "scroll_origin_y",
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
        diagnostics.extend(bern_target_api::icon_set::check(
            app,
            "the Web icon set",
            |icon| icons::svg(icon).is_some(),
        ));
        let mut visit = |node: &Node, _: Option<&str>| {
            if let Node::Element(element) = node
                && element.kind == "Picker"
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
            steps.push(BuildStep::Run {
                description: "Build the engine host, CLI and MCP server".into(),
                program: "cargo".into(),
                args: cargo("glue/host/Cargo.toml", &host_target, None),
                cwd: output.join("glue/host"),
                env: vec![],
            });
            // Every project built for this target shares its output
            // directory, and a renamed application leaves its old programs
            // there: `bin/` keeps this build's programs only. They are copied
            // by replacing the files, as rebuilt programs must be, so the
            // mirror only clears the rest.
            steps.push(BuildStep::Mirror {
                dir: output.join("bin"),
                files: vec![],
                keep: programs(app).map(PathBuf::from).to_vec(),
            });
            for name in programs(app) {
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
        // This build's files by name: every project built for this target
        // shares its output directory, which keeps what earlier builds left
        // (other programs, icons, resources). Generated sources and build
        // reports stay behind.
        let mut paths: Vec<PathBuf> = self
            .generate(app, context)?
            .into_iter()
            .map(|file| file.path)
            .filter(|path| !path.starts_with("glue") && path != Path::new("build.json"))
            .collect();
        paths.push(WASM.into());
        paths.extend(
            app.resources
                .iter()
                .map(|resource| Path::new("resources").join(&resource.name)),
        );
        if remote {
            paths.extend(programs(app).map(|name| Path::new("bin").join(name)));
        }
        let mut files: Vec<_> = paths
            .into_iter()
            .map(|path| (output.join(&path), path))
            .collect();
        // The agent skill, which each build writes afresh.
        let skills = output.join("skills");
        if skills.is_dir() {
            let below = files_below(&skills).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot read {}: {error}",
                    skills.display()
                ))]
            })?;
            files.extend(
                below
                    .into_iter()
                    .map(|(source, path)| (source, Path::new("skills").join(path))),
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_of_page_icons_is_linked_by_kind() {
        let dir = std::env::temp_dir().join(format!("bern-page-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&512u32.to_be_bytes());
        png.extend_from_slice(&512u32.to_be_bytes());
        for (name, contents) in [
            ("favicon.ico", vec![0, 0, 1, 0]),
            ("favicon.svg", b"<svg/>".to_vec()),
            ("apple-touch-icon.png", png.clone()),
            ("icon-512.png", png.clone()),
            ("icon-maskable-512.png", png.clone()),
            ("site.webmanifest", b"{}".to_vec()),
        ] {
            std::fs::write(dir.join(name), contents).unwrap();
        }
        let (files, links) = folder_icons(&dir, "7").unwrap();
        assert_eq!(files.len(), 6);
        assert!(files.iter().all(|file| file.path.starts_with("icons")));
        assert_eq!(
            links,
            "<link rel=\"icon\" href=\"icons/favicon.ico?v=7\" sizes=\"32x32\">\n\
             <link rel=\"icon\" type=\"image/svg+xml\" href=\"icons/favicon.svg?v=7\">\n\
             <link rel=\"apple-touch-icon\" href=\"icons/apple-touch-icon.png?v=7\">\n\
             <link rel=\"manifest\" href=\"icons/site.webmanifest?v=7\">\n"
        );
        // Without an SVG or an .ico, the PNG icons are the page icons; the
        // maskable one stays the manifest's.
        std::fs::remove_file(dir.join("favicon.ico")).unwrap();
        std::fs::remove_file(dir.join("favicon.svg")).unwrap();
        let (_, links) = folder_icons(&dir, "7").unwrap();
        assert!(links.contains(
            "<link rel=\"icon\" type=\"image/png\" sizes=\"512x512\" href=\"icons/icon-512.png?v=7\">"
        ));
        assert!(!links.contains("maskable"), "{links}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
