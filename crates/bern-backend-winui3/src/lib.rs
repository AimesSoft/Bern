//! The `winui3 + windows` target.
//!
//! Generates a C++/WinRT application from the shared IR, following the
//! Fluent controls, cards and navigation of Windows. One executable carries
//! two front ends over the same runtime (C++ templates in `templates/cpp`):
//! WinUI 3, through the Windows App Runtime the system has, and the system
//! XAML of Windows 10 and 11 in XAML Islands when there is none. Nothing is
//! installed or carried along: no .NET, no Windows App SDK copy, and the C
//! runtime is linked statically. The engine runs in the application
//! process behind the same in-process C ABI the SwiftUI target uses, linked
//! in as a static library. The Visual Studio C++ tools, a Windows SDK and
//! the Windows App SDK headers are requirements of this target's build
//! only.

mod cpp;
mod glue;
mod icons;
mod installer;
mod style;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, Target, TargetConfig, TargetDescriptor, WindowCapabilities, archive, check_common,
    dist_dir, files_below, icon, skill,
};
use std::path::PathBuf;

pub use cpp::Generator;
pub use glue::{MINIMUM_WINDOWS, PACKAGES, WINDOWS_APP_SDK};

/// The C++ runtime, copied into every generated application.
const RUNTIME: &[(&str, &str)] = &[
    ("bern_base.h", include_str!("../templates/cpp/bern_base.h")),
    (
        "bern_base.cpp",
        include_str!("../templates/cpp/bern_base.cpp"),
    ),
    (
        "bern_main.cpp",
        include_str!("../templates/cpp/bern_main.cpp"),
    ),
    ("bern_pch.h", include_str!("../templates/cpp/bern_pch.h")),
    (
        "bern_pch.cpp",
        include_str!("../templates/cpp/bern_pch.cpp"),
    ),
    (
        "bern_winui.cpp",
        include_str!("../templates/cpp/bern_winui.cpp"),
    ),
    (
        "bern_islands.cpp",
        include_str!("../templates/cpp/bern_islands.cpp"),
    ),
    (
        "bern_ui_core.inl",
        include_str!("../templates/cpp/bern_ui_core.inl"),
    ),
    (
        "bern_ui_motion.inl",
        include_str!("../templates/cpp/bern_ui_motion.inl"),
    ),
    (
        "bern_ui_drawing.inl",
        include_str!("../templates/cpp/bern_ui_drawing.inl"),
    ),
    (
        "bern_ui_widgets.inl",
        include_str!("../templates/cpp/bern_ui_widgets.inl"),
    ),
    (
        "bern_ui_features.inl",
        include_str!("../templates/cpp/bern_ui_features.inl"),
    ),
    (
        "bern_ui_structure.inl",
        include_str!("../templates/cpp/bern_ui_structure.inl"),
    ),
    (
        "bern_ui_scene.inl",
        include_str!("../templates/cpp/bern_ui_scene.inl"),
    ),
    (
        "bern_ui_windows.inl",
        include_str!("../templates/cpp/bern_ui_windows.inl"),
    ),
];

/// What the precompiled header is made of that Bern knows, as a
/// fingerprint: every runtime header it includes and the compiler options.
/// The build script adds the compiler, the SDK and the paths.
fn precompiled_identity(release: bool, frontends: glue::Frontends) -> String {
    let mut bytes = Vec::new();
    for (name, contents) in RUNTIME
        .iter()
        .filter(|(name, _)| name.ends_with(".h") || name.ends_with(".inl"))
    {
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(contents.as_bytes());
    }
    bytes.extend_from_slice(glue::compile_options(release, frontends).as_bytes());
    format!("{:016x}", bern_protocol::fingerprint_bytes(&bytes))
}

pub struct WinUi3Windows;

/// IR features the target builds, in both front ends.
const FEATURES: &[&str] = &[
    bern_target_api::feature::BUTTON_CONTENT,
    bern_target_api::feature::TAB_ACCESSORY,
    bern_target_api::feature::REVEAL,
    bern_target_api::feature::TOOLBAR_SEARCH,
    bern_target_api::feature::SEARCH_RESULTS,
    bern_target_api::feature::SESSION_ACTIONS,
    bern_target_api::feature::TOGGLE_BUTTON,
    bern_target_api::feature::DIALOG_WINDOWS,
    bern_target_api::feature::BOUND_LENGTHS,
    bern_target_api::feature::APPEARANCE,
    bern_target_api::feature::SHORTCUTS,
    bern_target_api::feature::BOUND_SHORTCUTS,
    bern_target_api::feature::SHORTCUT_INPUT,
    bern_target_api::feature::MENU_BAR,
    bern_target_api::feature::STATUS_ITEM,
    bern_target_api::feature::TABLE,
    bern_target_api::feature::CONTROLLED_TEXT,
    bern_target_api::feature::WATCH,
    bern_target_api::feature::LOGIN_SWITCH,
    bern_target_api::feature::PASTE,
    bern_target_api::feature::RUNTIME_THEME,
    bern_target_api::feature::RUNTIME_PALETTE,
    bern_target_api::feature::RUNTIME_IMAGES,
    bern_target_api::feature::PICTURE_FILLS,
    bern_target_api::feature::FILE_PICK,
    bern_target_api::feature::SCROLL_SNAP,
    bern_target_api::feature::RAILS,
    bern_target_api::feature::POPOVER_BUTTON,
    bern_target_api::feature::SIZE_CLASSES,
    bern_target_api::feature::TEXT_WEIGHTS,
    bern_target_api::feature::TEXT_LINES,
    bern_target_api::feature::IMAGE_CORNERS,
    bern_target_api::feature::SIDEBAR_WIDTH,
    bern_target_api::feature::CONTROLLED_VALUES,
    bern_target_api::feature::SEARCH_PALETTE,
    bern_target_api::feature::IMAGE_TEMPLATE,
    bern_target_api::feature::ADAPTIVE_IMAGES,
    bern_target_api::feature::SCALED_SCENE,
    bern_target_api::feature::ACCORDION_LAYOUT,
    bern_target_api::feature::CAPSULE_CONTROLS,
    bern_target_api::feature::MODAL_FLOW_TIMER,
    bern_target_api::feature::PAGE_BADGES,
    bern_target_api::feature::IDLE_CONTROL,
    bern_target_api::feature::FILL_BUTTONS,
    bern_target_api::feature::GRID_CELLS,
    bern_target_api::feature::PAGE_SEARCH,
    bern_target_api::feature::FORM_LAYOUT,
    bern_target_api::feature::MODAL_COVER,
    bern_target_api::feature::WINDOW_SHORT,
    // Not applicable, so nothing to warn about: a desktop window has no
    // orientation; a tablet turns its whole screen, as an iPad keeps its
    // own with `landscape` too.
    bern_target_api::feature::WINDOW_LANDSCAPE,
    bern_target_api::feature::TEXT_FORMATS,
    bern_target_api::feature::EDGE_GROW,
    bern_target_api::feature::DELAYED_APPEAR,
    bern_target_api::feature::TOOLBAR_LEADING,
    bern_target_api::feature::AXIS_PADDING,
    bern_target_api::feature::ICON_MENUS,
    bern_target_api::feature::ICON_LABELS,
    bern_target_api::feature::ICON_TABS,
    bern_target_api::feature::ROW_BASELINE,
    bern_target_api::feature::SIDEBAR_HEADER,
    bern_target_api::feature::UNTITLED_DETAIL,
];

/// Motion properties the target shows.
const MOTION: &[&str] = &[
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
    "nudge_key",
    "nudge_scale",
    "reduce_motion",
    "appear_delay",
    "stagger",
    "idle_area",
];

fn component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| COMPONENTS.iter().map(|spec| spec.name).collect())
}

/// Directory name of the product, e.g. `Showcase` for app id `showcase`.
pub fn product_name(app: &Application) -> String {
    bern_target_api::names::pascal(&app.app.id)
}

fn executable(app: &Application) -> String {
    app.app.id.replace('_', "-")
}

/// The application icon: a declared `.ico` as it is, or the declared PNG
/// or the Bern logo packed into one.
fn app_ico(app: &Application, context: &BuildContext) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let failed = |message: String| vec![Diagnostic::error(message)];
    match icon::source(app, "windows", &context.project_dir) {
        icon::Source::Native(path) => std::fs::read(&path)
            .map_err(|error| failed(format!("cannot read the icon {}: {error}", path.display()))),
        _ => Ok(icon::ico(
            &icon::images(app, "windows", &context.project_dir, icon::Shape::AsDrawn)
                .map_err(failed)?,
        )),
    }
}

/// `StatusItem(image:)`'s icons for a light and a dark taskbar: the SVG's
/// shape in black and in white at the sizes of the notification area.
fn status_icos(
    app: &Application,
    context: &BuildContext,
) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
    let Some(name) = icon::status_image(app) else {
        return Ok(Vec::new());
    };
    let path = bern_target_api::svg::resource_path(context, name);
    [
        ("app/status-light.ico", icon::ON_LIGHT),
        ("app/status-dark.ico", icon::ON_DARK),
    ]
    .into_iter()
    .map(|(file, color)| {
        let images = icon::status_pictures(&path, &[16, 20, 24, 32, 40, 48, 64], color)
            .map_err(|message| vec![Diagnostic::error(format!("StatusItem image: {message}"))])?;
        Ok(GeneratedFile {
            path: file.into(),
            contents: icon::ico(&images),
        })
    })
    .collect()
}

/// The file of the alternate icon `name` beside the executable.
pub(crate) fn alternate_ico(name: &str) -> String {
    format!("app-{name}.ico")
}

fn exe(name: &str) -> String {
    format!("{name}.exe")
}

impl Target for WinUi3Windows {
    fn build_hosts(&self) -> &'static [&'static str] {
        &["windows"]
    }

    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
            styles: style::SUPPORT,
            motion: MOTION,
            window_chrome: true,
            text_marks: true,
            dynamic_items: true,
            features: FEATURES,
            id: "winui3-windows",
            ui: "winui3",
            platforms: &["windows"],
            version: env!("CARGO_PKG_VERSION"),
            ir_versions: IR_VERSION..=IR_VERSION,
            color_source: ColorSource::Native,
            components: component_names(),
            windows: WindowCapabilities {
                native: true,
                virtual_windows: false,
                floating: true,
            },
            channels: &["ipc"],
            media: Some(MediaSupport {
                contract_versions: bern_media::MEDIA_CONTRACT_VERSION
                    ..=bern_media::MEDIA_CONTRACT_VERSION,
                backend: "windows-mediaengine",
                backend_version: "1",
                presenter: "swapchainpanel",
                presenter_version: "1",
                background_playback: true,
                // A media engine draws into one swap chain, which the
                // newest view shows.
                multi_view: false,
            }),
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let app = &bern_target_api::svg::prepare(app, config);
        let mut diagnostics = check_common(self, app);
        let framework = bern_target_api::texts::framework_languages();
        for language in &app.languages.supported {
            if !framework.contains(&language.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "the WinUI 3 runtime has no `{language}` translation of its own texts; supported: {}",
                    framework.join(", ")
                )));
            }
        }
        for channel in &config.channels {
            if !self.descriptor().channels.contains(&channel.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: channel `{channel}` is not available on Windows",
                    config.name
                )));
            }
        }
        diagnostics.extend(bern_target_api::icon_set::check(
            app,
            "the Windows icon set",
            |icon| icons::glyph(icon).is_some(),
        ));
        let mut visit = |node: &Node| {
            if let Node::Element(element) = node
                && element.kind == "Picker"
                && let Some(field) = element.field("value")
                && let bern_protocol::Type::Enum { name } = &field.ty
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
        for component in &app.components {
            component.body.walk(&mut visit);
        }
        for window in &app.windows.windows {
            window.content.walk(&mut visit);
        }
        diagnostics
    }

    fn generate(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        bern_target_api::svg::validate(app, context)?;
        let product = product_name(app);
        let executable = executable(app);
        let profile = if context.release { "release" } else { "debug" };
        let target_dir = context.workspace_dir.join("target/bern-glue-windows");
        let engine = target_dir.join(profile).join("bern_app.lib");
        let cache = context.workspace_dir.join("target/bern-winrt");
        let bundle = context.output_dir.join(&product);
        let frontends = glue::Frontends::of(&context.target.frontends);
        // The icons `WindowSet(icon:)` switches to, beside the executable: a
        // declared `.ico` as it is, or the declared PNG packed into one.
        let mut alternate_icos = Vec::new();
        for (name, source) in icon::alternates(app, "windows", &context.project_dir)
            .map_err(|message| vec![Diagnostic::error(message)])?
        {
            let failed = |message: String| vec![Diagnostic::error(message)];
            let contents = match source {
                icon::Source::Native(path) => std::fs::read(&path).map_err(|error| {
                    failed(format!("cannot read the icon {}: {error}", path.display()))
                })?,
                icon::Source::Png(path) => {
                    let png = std::fs::read(&path).map_err(|error| {
                        failed(format!("cannot read the icon {}: {error}", path.display()))
                    })?;
                    let size = icon::png_size(&png).map_or(256, |(width, _)| width);
                    icon::ico(&[icon::Image {
                        size,
                        png: png.into(),
                    }])
                }
                icon::Source::Logo => continue,
            };
            alternate_icos.push(GeneratedFile {
                path: PathBuf::from(&product).join(alternate_ico(&name)),
                contents,
            });
        }
        let generator = Generator::new(app);
        let svg_identity = format!(
            "{}-{}-{}-{:016x}-{:016x}",
            precompiled_identity(context.release, frontends),
            bern_target_api::svg::used(app),
            bern_target_api::script::webview(context),
            bern_protocol::fingerprint_bytes(
                include_str!("../../bern-target-api/src/script.rs").as_bytes()
            ) ^ bern_protocol::fingerprint_bytes(
                include_str!("../../bern-target-api/templates/script/host.cpp").as_bytes()
            ) ^ bern_protocol::fingerprint_bytes(bern_target_api::script::RUNTIME.as_bytes()),
            bern_protocol::fingerprint_bytes(
                bern_target_api::svg::cpp_template(include_str!(
                    "../templates/cpp/bern_ui_svg.inl"
                ))
                .as_bytes()
            )
        );
        let mut files = vec![
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/.cargo/config.toml", glue::cargo_config()),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app)),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                "app/build.bat",
                glue::build_script(&glue::Script {
                    bundle: &bundle,
                    executable: &executable,
                    engine: &engine,
                    cache: &cache,
                    release: context.release,
                    precompiled: &svg_identity,
                    frontends,
                }),
            ),
            GeneratedFile::text("app/app.manifest", glue::manifest(app)),
            GeneratedFile::text("app/app.rc", glue::resource_script(app, &executable)),
            GeneratedFile {
                path: "app/app.ico".into(),
                contents: app_ico(app, context)?,
            },
            GeneratedFile::text("app/app_id.h", generator.identity()),
            GeneratedFile::text(
                "app/app_catalog.cpp",
                generator.catalog(app.protocol.fingerprint()),
            ),
            GeneratedFile::text("app/app_ui.inl", generator.interface()),
        ];
        files.extend(status_icos(app, context)?);
        let svg = bern_target_api::svg::used(app);
        files.push(GeneratedFile::text(
            "app/runtime/bern_ui_svg.inl",
            if svg {
                bern_target_api::svg::cpp_template(include_str!("../templates/cpp/bern_ui_svg.inl"))
            } else {
                String::new()
            },
        ));
        for (name, contents) in RUNTIME {
            files.push(GeneratedFile::text(
                format!("app/runtime/{name}"),
                if *name == "bern_pch.h" {
                    contents.replace("/*bern:svg-enabled*/", if svg { "1" } else { "0" })
                } else {
                    contents.to_string()
                },
            ));
        }
        let lock = context.workspace_dir.join("Cargo.lock");
        if let Ok(contents) = std::fs::read(&lock)
            && !context.output_dir.join("glue/Cargo.lock").exists()
        {
            files.push(GeneratedFile {
                path: "glue/Cargo.lock".into(),
                contents,
            });
        }
        let media = self
            .descriptor()
            .media
            .filter(|_| app.uses_media())
            .map(|media| {
                serde_json::json!({
                    "contract": bern_media::MEDIA_CONTRACT_VERSION,
                    "backend": media.backend,
                    "backend_version": media.backend_version,
                    "presenter": media.presenter,
                    "presenter_version": media.presenter_version,
                    "players": app.players.iter().map(|player| &player.id).collect::<Vec<_>>(),
                })
            });
        let manifest = serde_json::json!({
            "target": self.descriptor().id,
            "ui": "winui3",
            "platform": "windows",
            "ir_version": app.ir_version,
            "generator": "bern-backend-winui3",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "toolkit": {
                "windows_app_sdk": WINDOWS_APP_SDK,
                "packages": PACKAGES.iter().map(|(id, version)| format!("{id} {version}")).collect::<Vec<_>>(),
                "minimum_windows": MINIMUM_WINDOWS,
            },
            "architecture": std::env::consts::ARCH,
            "runtime_identifier": glue::runtime_identifier(),
            "profile": profile,
            "engine_deployment": "in-process",
            "ui_bridge": "cpp",
            "frontends": frontends.names(),
            "channels": context.target.channels,
            "color_source": "native",
            "protocol": app.protocol.namespace,
            "protocol_fingerprint": format!("{:016x}", app.protocol.fingerprint()),
            "languages": app.languages.supported,
            "media": media,
            "resources": app.resources.iter().map(|resource| &resource.name).collect::<Vec<_>>(),
            "product": product,
        });
        files.push(GeneratedFile::text(
            "build.json",
            serde_json::to_string_pretty(&manifest).expect("manifest"),
        ));
        files.extend(bern_target_api::svg::static_files(app, context)?);
        bern_target_api::script::configure(&mut files, context);
        files.extend(alternate_icos);
        Ok(files)
    }

    fn plan(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        let product = product_name(app);
        let executable = executable(app);
        let output = &context.output_dir;
        let bundle = output.join(&product);
        let profile = if context.release { "release" } else { "debug" };
        let target_dir = context.workspace_dir.join("target/bern-glue-windows");
        let built = target_dir.join(profile);
        let mut cargo = vec![
            "build".to_string(),
            "--manifest-path".into(),
            output.join("glue/Cargo.toml").display().to_string(),
            "--target-dir".into(),
            target_dir.display().to_string(),
        ];
        if context.release {
            cargo.push("--release".into());
        }
        let mut steps = vec![
            BuildStep::Run {
                description: "Build the engine library and the entries".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("glue"),
                env: vec![],
            },
            BuildStep::Run {
                description: "Build the Windows application".into(),
                program: "cmd".into(),
                args: vec![
                    "/d".into(),
                    "/c".into(),
                    output.join("app").join("build.bat").display().to_string(),
                ],
                cwd: output.join("app"),
                env: vec![],
            },
            // Declared resources ship next to the executable, which finds
            // them relative to itself.
            BuildStep::Mirror {
                dir: bundle.join("resources"),
                files: app
                    .resources
                    .iter()
                    .map(|resource| {
                        (
                            bern_target_api::svg::resource_path(context, &resource.name),
                            PathBuf::from(&resource.name),
                        )
                    })
                    .collect(),
                keep: vec![],
            },
            BuildStep::CreateDir {
                path: output.join("bin"),
            },
            BuildStep::Copy {
                from: built.join(exe(&executable)),
                to: output.join("bin").join(exe(&executable)),
            },
            BuildStep::Copy {
                from: built.join(exe(&format!("{executable}-mcp"))),
                to: output.join("bin").join(exe(&format!("{executable}-mcp"))),
            },
        ];
        // The agent skill beside `bin\`, where the app listens: in the
        // build, the `.zip` and the folder the installer fills.
        let listens = skill::listens(&context.target.channels);
        let place = listens.then(|| {
            skill::Place::beside_bin(app, true).installed(format!(
                "%LOCALAPPDATA%\\Programs\\{product}\\bin\\{}",
                exe(&executable)
            ))
        });
        steps.extend(skill::steps(app, output, place.as_ref()));
        let mut products = vec![
            bundle.join(exe(&executable)),
            output.join("bin").join(exe(&executable)),
            output.join("bin").join(exe(&format!("{executable}-mcp"))),
        ];
        if listens {
            products.push(output.join(skill::path(app)));
        }
        Ok(BuildPlan {
            steps,
            notes: Vec::new(),
            run: Some(Run::Program(bundle.join(exe(&executable)))),
            products,
        })
    }

    /// An installer (`setup.exe`) that installs for the current user
    /// without administrator rights, and a `.zip` of the same folder for
    /// people who do not install. Both carry the command line and the MCP
    /// server in `bin\`, and the agent skill listing their commands in
    /// `skills\`. Signed with the certificate from [`Signing`].
    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        let product = product_name(app);
        let executable = executable(app);
        let output = &context.output_dir;
        let bundle = output.join(&product);
        let staged = output.join("package").join(&product);
        let installer_dir = output.join("installer");
        let dist = dist_dir(context);
        let setup = dist.join(installer::setup_name(app));
        let mut files = files_below(&bundle).map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot read {}: {error}",
                bundle.display()
            ))]
        })?;
        for program in [executable.clone(), format!("{executable}-mcp")] {
            let name = exe(&program);
            files.push((
                output.join("bin").join(&name),
                PathBuf::from("bin").join(&name),
            ));
        }
        if skill::listens(&context.target.channels) {
            files.push((output.join(skill::path(app)), skill::path(app)));
        }
        // The installer carries the staged copies, which are the signed ones.
        let payload: Vec<installer::Payload> = installer::payload(&files)
            .into_iter()
            .map(|file| installer::Payload {
                source: staged.join(&file.path),
                ..file
            })
            .collect();
        let programs: Vec<PathBuf> = payload
            .iter()
            .filter(|file| {
                file.path
                    .extension()
                    .is_some_and(|extension| extension == "exe")
            })
            .map(|file| file.source.clone())
            .collect();
        let text = |name: &str, contents: String| BuildStep::Write {
            path: installer_dir.join(name),
            contents: contents.into_bytes(),
        };
        let (zip_step, zip) = archive(
            &staged,
            &dist.join(format!(
                "{product}-{}-windows-{}",
                app.app.version,
                glue::architecture()
            )),
        );
        let steps = vec![
            BuildStep::RemoveIfExists {
                path: output.join("package"),
            },
            BuildStep::Mirror {
                dir: staged.clone(),
                files,
                keep: vec![],
            },
            text("setup.cpp", installer::SETUP_CPP.to_string()),
            text(
                "setup_info.h",
                installer::setup_info(app, &executable, &payload),
            ),
            text(
                "setup.rc",
                installer::setup_rc(app, &output.join("app/app.ico"), &payload),
            ),
            text("setup.manifest", installer::SETUP_MANIFEST.to_string()),
            text("package.bat", installer::package_script(&programs, &setup)),
            BuildStep::CreateDir { path: dist.clone() },
            BuildStep::RemoveIfExists {
                path: setup.clone(),
            },
            BuildStep::RemoveIfExists { path: zip.clone() },
            BuildStep::Run {
                description: "Make the installer".into(),
                program: "cmd".into(),
                args: vec![
                    "/d".into(),
                    "/c".into(),
                    installer_dir.join("package.bat").display().to_string(),
                ],
                cwd: installer_dir.clone(),
                env: vec![],
            },
            zip_step,
        ];
        let mut notes = Vec::new();
        if signing.windows_certificate.is_none() {
            notes.push(
                "the installer and the app are not signed: SmartScreen warns about them and security software may flag a new program (set BERN_WINDOWS_CERTIFICATE)"
                    .into(),
            );
        }
        Ok(BuildPlan {
            steps,
            products: vec![setup, zip],
            run: None,
            notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_profile_has_its_own_precompiled_header() {
        let both = glue::Frontends::of(&[]);
        let debug = precompiled_identity(false, both);
        assert_eq!(debug, precompiled_identity(false, both), "stable");
        assert_ne!(debug, precompiled_identity(true, both));
        // Each choice of front ends has its own.
        let winui = glue::Frontends::of(&["winui3".to_string()]);
        assert_eq!(
            winui,
            glue::Frontends {
                winui: true,
                islands: false
            }
        );
        assert_ne!(debug, precompiled_identity(false, winui));
        // Nothing generated per application goes into it, so it survives
        // every change to the application.
        let header = RUNTIME
            .iter()
            .find(|(name, _)| *name == "bern_pch.h")
            .map(|(_, contents)| *contents)
            .unwrap();
        assert!(!header.contains("app_ui.inl") && !header.contains("app_id.h"));
    }

    fn runtime(file: &str) -> &'static str {
        RUNTIME
            .iter()
            .find(|(name, _)| *name == file)
            .map(|(_, contents)| *contents)
            .unwrap()
    }

    /// The navigation view slides its selection pill from the old item to
    /// the new only where the two pills line up, measured with the items'
    /// moves and scales; otherwise both pills shrink and grow at once. So
    /// the items stand at rest whenever the selection changes, and scale
    /// about the line of the pill.
    #[test]
    fn sidebar_items_rest_while_the_selection_changes() {
        let structure = runtime("bern_ui_structure.inl");
        // The runtime's own choice, from the selection field.
        let lines: Vec<&str> = structure.lines().map(str::trim).collect();
        let write = lines
            .iter()
            .position(|line| *line == "view.SelectedItem(item);")
            .unwrap();
        assert_eq!(lines[write - 1], "registry->Rest(true);");
        assert_eq!(lines[write + 1], "registry->Rest(false);");
        // A click or a key: ItemInvoked comes before the view measures,
        // SelectionChanged after.
        let between = |from: &str, to: &str| {
            let start = structure.find(from).unwrap();
            &structure[start..start + structure[start..].find(to).unwrap()]
        };
        let invoked = between("view.ItemInvoked(", "view.SelectionChanged(");
        assert!(invoked.contains("registry->Rest(true);"), "{invoked}");
        let changed = between("view.SelectionChanged(", "TextFn selectedLabel");
        assert!(changed.contains("registry->Rest(false);"), "{changed}");

        let motion = runtime("bern_ui_motion.inl");
        let layers = motion
            .split_once("inline winrt::com_ptr<LayerStack> LayersOf(")
            .unwrap()
            .1;
        let item = layers.split_once("NavigationViewItem").unwrap().1;
        let item = item.split_once('}').unwrap().0;
        assert!(item.contains("made->alongPill = true;"), "{item}");
        assert!(motion.contains("if (resting) x = y = 0, sx = sy = 1;"));
    }

    /// The pane folds by the title bar's button in WinUI 3 and by its own in
    /// the system XAML; a pane of no declared width takes the width its
    /// edge is dragged to, and a declared width stays.
    #[test]
    fn sidebar_folds_and_takes_the_dragged_width() {
        let structure = runtime("bern_ui_structure.inl");
        let split = structure
            .split_once("inline void SplitView(")
            .unwrap()
            .1
            .split_once("\n}\n")
            .unwrap()
            .0;
        let (winui, islands) = split
            .split_once("#if BERN_WINUI")
            .unwrap()
            .1
            .split_once("#else")
            .unwrap();
        assert!(winui.contains("view.IsPaneToggleButtonVisible(false);"));
        assert!(winui.contains("PaneToggle(ctx.win->titleBar, view);"));
        let islands = islands.split_once("#endif").unwrap().0;
        assert!(islands.contains("view.IsPaneToggleButtonVisible(true);"));
        let declared = split.split_once("if (props.width) {").unwrap().1;
        let (fixed, sized) = declared.split_once("return;").unwrap();
        assert!(fixed.contains("p.Add(view);") && !fixed.contains("PaneSizer"));
        assert!(sized.contains("frame.Children().Append(PaneSizer(view));"));
        assert!(structure.contains("kPaneNarrowest = 180, kPaneWidest = 480;"));
    }
}
