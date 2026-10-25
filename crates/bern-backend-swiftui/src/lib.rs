//! The `swiftui + macos` target, and with it the `swiftui + ios` and
//! `swiftui + tvos` targets.
//!
//! Generates idiomatic SwiftUI from the shared IR: standard scenes,
//! containers and controls, which keep the system styles and colors
//! unless the app declares style overrides. The engine runs in the app
//! process; the build links it as a static library.

mod glue;
pub mod ios;
mod names;
mod resources;
mod style;
mod swift;
pub mod tvos;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node, WindowInitial, WindowRole};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common, dist_dir,
    icon, package_architecture, skill,
};
use std::path::{Path, PathBuf};

pub use ios::{MINIMUM_IOS, SwiftUiIos};
pub use swift::{Generator, Platform};
pub use tvos::{MINIMUM_TVOS, SwiftUiTvos};

/// Minimum deployment target, chosen in the design document.
pub const MINIMUM_MACOS: &str = "14.0";

/// Launch Services' tool, which registers an app bundle and its icon.
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

/// The application icon in `Contents/Resources` (`CFBundleIconFile`).
pub const ICON_FILE: &str = "AppIcon.icns";

/// A declared Icon Composer document, copied outside the bundle under the
/// icon's name for `actool`, which compiles it into the bundle's
/// `Assets.car` and writes [`ICON_FILE`] beside it for earlier systems.
pub const ICON_DOCUMENT: &str = "AppIcon.icon";

/// The asset or resource name of the alternate icon `name`, beside
/// `AppIcon`: what `WindowSet(icon:)` switches to.
pub(crate) fn alternate_icon(name: &str) -> String {
    format!("AppIcon-{name}")
}

/// The alternate icons `WindowSet(icon:)` can switch to on `platform`, by
/// name, and where each comes from; an error names one with nothing for
/// the platform.
pub(crate) fn alternate_icons(
    app: &Application,
    platform: &str,
    project_dir: &Path,
) -> Result<Vec<(String, icon::Source)>, Vec<Diagnostic>> {
    icon::alternates(app, platform, project_dir).map_err(|message| vec![Diagnostic::error(message)])
}

/// The Icon Composer document `app` declares for `platform`, if any.
pub(crate) fn icon_document(
    app: &Application,
    platform: &str,
    project_dir: &Path,
) -> Option<PathBuf> {
    match icon::source(app, platform, project_dir) {
        icon::Source::Native(path) if icon::is_icon_document(&path) => Some(path),
        _ => None,
    }
}

/// The files of the Icon Composer document at `dir`, generated under
/// `into`.
pub(crate) fn icon_document_files(
    dir: &Path,
    into: &Path,
) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
    let failed = |message: String| vec![Diagnostic::error(message)];
    icon::icon_document(dir)
        .map_err(|message| failed(format!("the icon {} {message}", dir.display())))?
        .into_iter()
        .map(|file| {
            let contents = std::fs::read(dir.join(&file)).map_err(|error| {
                failed(format!(
                    "cannot read {}: {error}",
                    dir.join(&file).display()
                ))
            })?;
            Ok(GeneratedFile {
                path: into.join(file),
                contents,
            })
        })
        .collect()
}

/// Motion SwiftUI shows, on macOS and iOS.
pub(crate) const MOTION: &[&str] = &[
    "appear",
    "appear_delay",
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
    "stagger",
    "idle_area",
];

/// IR features SwiftUI builds, on macOS and iOS.
pub(crate) const FEATURES: &[&str] = &[
    bern_target_api::feature::CONTROLLED_VALUES,
    bern_target_api::feature::SIDEBAR_WIDTH,
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
    bern_target_api::feature::BOUND_SHORTCUTS,
    bern_target_api::feature::SHORTCUT_INPUT,
    bern_target_api::feature::MENU_BAR,
    bern_target_api::feature::RUNTIME_THEME,
    bern_target_api::feature::RUNTIME_PALETTE,
    bern_target_api::feature::RUNTIME_IMAGES,
    bern_target_api::feature::PICTURE_FILLS,
    bern_target_api::feature::FILE_PICK,
    bern_target_api::feature::SCROLL_SNAP,
    bern_target_api::feature::RAILS,
    bern_target_api::feature::POPOVER_BUTTON,
    bern_target_api::feature::SIDEBAR_HEADER,
    bern_target_api::feature::TEXT_WEIGHTS,
    bern_target_api::feature::TEXT_LINES,
    bern_target_api::feature::IMAGE_CORNERS,
    bern_target_api::feature::IMAGE_TEMPLATE,
    bern_target_api::feature::ADAPTIVE_IMAGES,
    bern_target_api::feature::ACCORDION_LAYOUT,
    bern_target_api::feature::SCALED_SCENE,
    bern_target_api::feature::CAPSULE_CONTROLS,
    bern_target_api::feature::MODAL_FLOW_TIMER,
    bern_target_api::feature::PAGE_BADGES,
    bern_target_api::feature::EDGE_GROW,
    bern_target_api::feature::DELAYED_APPEAR,
    bern_target_api::feature::ROW_BASELINE,
    bern_target_api::feature::ICON_MENUS,
    bern_target_api::feature::ICON_LABELS,
    bern_target_api::feature::ICON_TABS,
    bern_target_api::feature::TOOLBAR_LEADING,
    bern_target_api::feature::AXIS_PADDING,
    bern_target_api::feature::UNTITLED_DETAIL,
    bern_target_api::feature::IDLE_CONTROL,
    bern_target_api::feature::FILL_BUTTONS,
    bern_target_api::feature::GRID_CELLS,
    bern_target_api::feature::TAB_ACCESSORY,
    bern_target_api::feature::PAGE_SEARCH,
    bern_target_api::feature::FORM_LAYOUT,
    bern_target_api::feature::MODAL_COVER,
    bern_target_api::feature::WINDOW_SHORT,
    bern_target_api::feature::WINDOW_LANDSCAPE,
    bern_target_api::feature::SEARCH_PALETTE,
    bern_target_api::feature::SIZE_CLASSES,
    bern_target_api::feature::TEXT_FORMATS,
];

const RUNTIME: &str = include_str!("../templates/BernRuntime.swift");

/// The runtime template with the extensions of each file kind from the
/// shared table, so pickers, drops and the Info.plist agree.
pub(crate) fn runtime_source(template: &str) -> String {
    let table: Vec<String> = bern_ir::spec::FILE_KIND_EXTENSIONS
        .iter()
        .map(|(kind, extensions)| {
            let list: Vec<String> = extensions.iter().map(|ext| format!("\"{ext}\"")).collect();
            format!("\"{kind}\": [{}]", list.join(", "))
        })
        .collect();
    template.replace("/*bern:file-extensions*/", &table.join(", "))
}
pub(crate) fn runtime_with_svg(app: &Application, source: &str) -> String {
    let mut source = runtime_source(source);
    if bern_target_api::svg::used(app) {
        source.push_str(&bern_target_api::svg::template(
            include_str!("../templates/BernSvg.swift"),
            false,
        ));
    }
    source
}
const MEDIA: &str = include_str!("../templates/BernMedia.swift");
const STYLE: &str = include_str!("../templates/BernStyle.swift");
const APPKIT: &str = include_str!("../templates/BernAppKit.swift");

/// Swift sources compiled into the app, in order.
const SOURCES: &[&str] = &[
    "BernRuntime.swift",
    "BernMedia.swift",
    "BernStyle.swift",
    "BernAppKit.swift",
    "BernGenerated.swift",
];
const HEADER: &str = include_str!("../templates/bern.h");

pub struct SwiftUiMacos;

pub(crate) fn component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| COMPONENTS.iter().map(|spec| spec.name).collect())
}

/// Bundle and executable name, e.g. `Showcase` for app id `showcase`.
pub fn product_name(app: &Application) -> String {
    names::pascal(&app.app.id)
}

/// Cargo arguments that build the engine's static library like `build`
/// (`cargo build --lib …`) and have rustc write the native libraries and
/// frameworks it needs to `file` (its `native-static-libs`). swiftc reads
/// the file as a response file: a dependency of the engine, such as a
/// media kernel, may link system frameworks the frontend does not name.
pub(crate) fn listing_native_libs(build: &[String], file: &std::path::Path) -> Vec<String> {
    let mut args: Vec<String> = build
        .iter()
        .filter(|arg| arg.as_str() != "--lib" && arg.as_str() != "--bins")
        .cloned()
        .collect();
    if let Some(first) = args.first_mut() {
        *first = "rustc".into();
    }
    args.insert(1, "--lib".into());
    args.extend([
        "--".into(),
        "--print".into(),
        format!("native-static-libs={}", file.display()),
    ]);
    args
}

/// The response file of [`listing_native_libs`] next to the library.
pub(crate) fn native_libs_file(rust_output: &std::path::Path) -> PathBuf {
    rust_output.join("libbern_app.native-libs")
}

fn architecture() -> (&'static str, &'static str) {
    match std::env::consts::ARCH {
        "x86_64" => ("x86_64", "x86_64-apple-darwin"),
        _ => ("arm64", "aarch64-apple-darwin"),
    }
}

impl Target for SwiftUiMacos {
    fn build_hosts(&self) -> &'static [&'static str] {
        &["macos"]
    }

    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
            styles: style::SUPPORT,
            motion: MOTION,
            window_chrome: true,
            text_marks: true,
            dynamic_items: true,
            features: FEATURES,
            id: "swiftui-macos",
            ui: "swiftui",
            platforms: &["macos"],
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
                backend: "avfoundation",
                backend_version: "1",
                presenter: "avplayerview",
                presenter_version: "1",
                background_playback: true,
                multi_view: true,
            }),
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let app = &bern_target_api::svg::prepare(app, config);
        let mut diagnostics = check_swiftui(self, app, config, Platform::Macos);
        for window in &app.windows.windows {
            if window.id == app.windows.primary && window.initial != WindowInitial::Open {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}`: macOS 14 always shows the primary window at launch; declare `initial: open`",
                    window.id
                )));
            }
            if window.role == WindowRole::Settings && window.initial == WindowInitial::Open {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}`: a settings window cannot open at launch on macOS",
                    window.id
                )));
            }
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
        let bundle = PathBuf::from(format!("{product}.app"));
        let mut files = vec![
            GeneratedFile::text(
                "Sources/BernRuntime.swift",
                crate::runtime_with_svg(app, RUNTIME),
            ),
            GeneratedFile::text("Sources/BernMedia.swift", MEDIA),
            GeneratedFile::text("Sources/BernStyle.swift", STYLE),
            GeneratedFile::text("Sources/BernAppKit.swift", APPKIT),
            GeneratedFile::text("Sources/bern.h", HEADER),
            GeneratedFile::text(
                "Sources/BernGenerated.swift",
                Generator::new(app, Platform::Macos).generate(app.protocol.fingerprint()),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text(
                "glue/src/lib.rs",
                glue::lib_rs(context, app, Platform::Macos),
            ),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                bundle.join("Contents/Info.plist"),
                resources::info_plist(
                    app,
                    &product,
                    MINIMUM_MACOS,
                    icon_document(app, "macos", &context.project_dir).is_some(),
                ),
            ),
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
        for (path, table) in resources::localization_files(app) {
            files.push(GeneratedFile::text(
                bundle.join("Contents/Resources").join(path),
                table,
            ));
        }
        // The application icon: a declared Icon Composer document for
        // `actool` to compile, a declared `.icns` as it is, a declared PNG,
        // or the Bern logo on a full square that macOS cuts to its rounded
        // shape.
        let failed = |message: String| vec![Diagnostic::error(message)];
        if let Some(document) = icon_document(app, "macos", &context.project_dir) {
            files.extend(icon_document_files(&document, Path::new(ICON_DOCUMENT))?);
        } else {
            let contents = match icon::source(app, "macos", &context.project_dir) {
                icon::Source::Native(path) => std::fs::read(&path).map_err(|error| {
                    failed(format!("cannot read the icon {}: {error}", path.display()))
                })?,
                _ => icon::icns(
                    &icon::images(app, "macos", &context.project_dir, icon::Shape::Masked)
                        .map_err(failed)?,
                ),
            };
            files.push(GeneratedFile {
                path: bundle.join("Contents/Resources").join(ICON_FILE),
                contents,
            });
        }
        // The icons `WindowSet(icon:)` switches to, under their asset names:
        // an Icon Composer document beside the primary one for `actool`, an
        // `.icns` or a PNG in `Contents/Resources`.
        for (name, source) in alternate_icons(app, "macos", &context.project_dir)? {
            match source {
                icon::Source::Native(path) if icon::is_icon_document(&path) => {
                    files.extend(icon_document_files(
                        &path,
                        Path::new(&format!("{}.icon", alternate_icon(&name))),
                    )?);
                }
                icon::Source::Native(path) | icon::Source::Png(path) => {
                    let contents = std::fs::read(&path).map_err(|error| {
                        failed(format!("cannot read the icon {}: {error}", path.display()))
                    })?;
                    let extension = if icon::is_png_name(&path.display().to_string()) {
                        "png"
                    } else {
                        "icns"
                    };
                    files.push(GeneratedFile {
                        path: bundle
                            .join("Contents/Resources")
                            .join(format!("{}.{extension}", alternate_icon(&name))),
                        contents,
                    });
                }
                icon::Source::Logo => {}
            }
        }
        let (architecture, _) = architecture();
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
            "ui": "swiftui",
            "platform": "macos",
            "ir_version": app.ir_version,
            "generator": "bern-backend-swiftui",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "minimum_macos": MINIMUM_MACOS,
            "architecture": architecture,
            "profile": if context.release { "release" } else { "debug" },
            "engine_deployment": "in-process",
            "ui_bridge": "c-abi",
            "channels": context.target.channels,
            "color_source": "native",
            "protocol": app.protocol.namespace,
            "protocol_fingerprint": format!("{:016x}", app.protocol.fingerprint()),
            "languages": app.languages.supported,
            "media": media,
            "resources": app.resources.iter().map(|resource| &resource.name).collect::<Vec<_>>(),
            "app_bundle": bundle,
        });
        files.push(GeneratedFile::text(
            "build.json",
            serde_json::to_string_pretty(&manifest).expect("manifest"),
        ));
        files.extend(bern_target_api::svg::static_files(app, context)?);
        bern_target_api::script::configure(&mut files, context);
        Ok(files)
    }

    fn plan(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        let product = product_name(app);
        let output = &context.output_dir;
        let bundle = output.join(format!("{product}.app"));
        let (architecture, rust_target) = architecture();
        let profile = if context.release { "release" } else { "debug" };
        let glue_target = context.workspace_dir.join("target/bern-glue");
        let rust_output = glue_target.join(rust_target).join(profile);
        let cli = app.app.id.replace('_', "-");
        let env = vec![(
            "MACOSX_DEPLOYMENT_TARGET".to_string(),
            MINIMUM_MACOS.to_string(),
        )];

        let mut cargo = vec![
            "build".to_string(),
            "--manifest-path".into(),
            output.join("glue/Cargo.toml").display().to_string(),
            "--target".into(),
            rust_target.into(),
            "--target-dir".into(),
            glue_target.display().to_string(),
        ];
        if context.release {
            cargo.push("--release".into());
        }
        let sources = output.join("Sources");
        let mut swiftc = vec![
            "swiftc".to_string(),
            "-swift-version".into(),
            "6".into(),
            "-parse-as-library".into(),
            "-module-name".into(),
            product.clone(),
            "-target".into(),
            format!("{architecture}-apple-macos{MINIMUM_MACOS}"),
            "-import-objc-header".into(),
            sources.join("bern.h").display().to_string(),
            "-L".into(),
            rust_output.display().to_string(),
            "-lbern_app".into(),
            format!("@{}", native_libs_file(&rust_output).display()),
            // rustc's list repeats libraries swiftc links anyway.
            "-Xlinker".into(),
            "-no_warn_duplicate_libraries".into(),
            "-framework".into(),
            "SwiftUI".into(),
            "-framework".into(),
            "AppKit".into(),
            "-framework".into(),
            "AVFoundation".into(),
            "-framework".into(),
            "AVKit".into(),
            "-module-cache-path".into(),
            context
                .workspace_dir
                .join("target/bern-swift-module-cache")
                .display()
                .to_string(),
        ];
        // Both profiles keep debug symbols, in a dSYM outside the bundle;
        // release builds also drop unreferenced code.
        swiftc.extend(if context.release {
            vec![
                "-O".to_string(),
                "-g".into(),
                "-Xlinker".into(),
                "-dead_strip".into(),
            ]
        } else {
            vec!["-Onone".to_string(), "-g".into()]
        });
        for file in SOURCES {
            swiftc.push(sources.join(file).display().to_string());
        }
        let executable = bundle.join("Contents/MacOS").join(&product);
        swiftc.push("-o".into());
        swiftc.push(executable.display().to_string());
        // swiftc writes the dSYM next to the executable, inside the bundle;
        // it ships beside the bundle instead, where lldb and crash report
        // symbolication find it.
        let linked_symbols = bundle
            .join("Contents/MacOS")
            .join(format!("{product}.dSYM"));
        let symbols = output.join(format!("{product}.app.dSYM"));

        let alternates = alternate_icons(app, "macos", &context.project_dir)?;
        let native_libs = listing_native_libs(&cargo, &native_libs_file(&rust_output));
        let mut steps = vec![
            BuildStep::Run {
                description: "Build the Rust engine and bridge".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("glue"),
                env: env.clone(),
            },
            BuildStep::Run {
                description: "List the native libraries the engine links".into(),
                program: "cargo".into(),
                args: native_libs,
                cwd: output.join("glue"),
                env: env.clone(),
            },
            BuildStep::CreateDir {
                path: bundle.join("Contents/MacOS"),
            },
            // Declared resources ship in Contents/Resources next to the
            // generated localizations, before the bundle is signed.
            BuildStep::Mirror {
                dir: bundle.join("Contents/Resources"),
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
                keep: resources::localization_files(app)
                    .into_iter()
                    .map(|(path, _)| path)
                    .chain([PathBuf::from(ICON_FILE)])
                    .chain(alternates.iter().flat_map(|(name, _)| {
                        ["png", "icns"].map(|extension| {
                            PathBuf::from(format!("{}.{extension}", alternate_icon(name)))
                        })
                    }))
                    .collect(),
            },
            // Symbols of an earlier build must not stay in or beside the
            // bundle.
            BuildStep::RemoveIfExists {
                path: linked_symbols.clone(),
            },
            BuildStep::RemoveIfExists {
                path: symbols.clone(),
            },
            BuildStep::Run {
                description: "Compile the SwiftUI frontend".into(),
                program: "xcrun".into(),
                args: swiftc,
                cwd: output.clone(),
                env,
            },
            BuildStep::Run {
                description: "Move the debug symbols out of the bundle".into(),
                program: "mv".into(),
                args: vec![
                    linked_symbols.display().to_string(),
                    symbols.display().to_string(),
                ],
                cwd: output.clone(),
                env: vec![],
            },
        ];
        // Icon Composer documents, the primary icon's and the alternates':
        // Liquid Glass layers in Assets.car for macOS 26, and an .icns drawn
        // from the primary for earlier systems.
        let primary_document = icon_document(app, "macos", &context.project_dir).is_some();
        let documents: Vec<String> = alternates
            .iter()
            .filter(|(_, source)| matches!(source, icon::Source::Native(path) if icon::is_icon_document(path)))
            .map(|(name, _)| alternate_icon(name))
            .collect();
        if primary_document || !documents.is_empty() {
            let mut inputs = Vec::new();
            let mut names = Vec::new();
            if primary_document {
                inputs.push(output.join(ICON_DOCUMENT).display().to_string());
                names.extend(["--app-icon".to_string(), "AppIcon".into()]);
            }
            for document in &documents {
                inputs.push(
                    output
                        .join(format!("{document}.icon"))
                        .display()
                        .to_string(),
                );
                names.extend(["--alternate-app-icon".to_string(), document.clone()]);
            }
            steps.push(BuildStep::Run {
                description: "Compile the icon".into(),
                program: "xcrun".into(),
                args: [
                    vec!["actool".into()],
                    inputs,
                    vec![
                        "--compile".into(),
                        bundle.join("Contents/Resources").display().to_string(),
                        "--platform".into(),
                        "macosx".into(),
                        "--minimum-deployment-target".into(),
                        MINIMUM_MACOS.into(),
                    ],
                    names,
                    vec![
                        "--output-partial-info-plist".into(),
                        output.join("AssetCatalog.plist").display().to_string(),
                        "--output-format".into(),
                        "human-readable-text".into(),
                        "--errors".into(),
                        "--warnings".into(),
                    ],
                ]
                .concat(),
                cwd: output.clone(),
                env: vec![],
            });
        }
        if context.release {
            steps.push(BuildStep::Run {
                description: "Strip the symbols the dSYM keeps from the executable".into(),
                program: "xcrun".into(),
                args: vec![
                    "strip".into(),
                    "-S".into(),
                    "-x".into(),
                    executable.display().to_string(),
                ],
                cwd: output.clone(),
                env: vec![],
            });
        }
        steps.extend([
            BuildStep::CreateDir {
                path: output.join("bin"),
            },
            BuildStep::Copy {
                from: rust_output.join(&cli),
                to: output.join("bin").join(&cli),
            },
            BuildStep::Copy {
                from: rust_output.join(format!("{cli}-mcp")),
                to: output.join("bin").join(format!("{cli}-mcp")),
            },
            BuildStep::Run {
                description: "Sign the app for local use".into(),
                program: "codesign".into(),
                args: vec![
                    "--force".into(),
                    "--sign".into(),
                    "-".into(),
                    bundle.display().to_string(),
                ],
                cwd: output.clone(),
                env: vec![],
            },
            // Each build replaces the bundle at the same place, where Finder
            // and the Dock keep showing the icon they first saw; registering
            // it again makes them read the new one.
            BuildStep::Run {
                description: "Refresh the app's icon in Finder and the Dock".into(),
                program: "touch".into(),
                args: vec![bundle.display().to_string()],
                cwd: output.clone(),
                env: vec![],
            },
            BuildStep::Run {
                description: "Register the app again with Launch Services".into(),
                program: LSREGISTER.into(),
                args: vec!["-f".into(), bundle.display().to_string()],
                cwd: output.clone(),
                env: vec![],
            },
        ]);
        // The agent skill beside `bin/`, where the app listens.
        let listens = skill::listens(&context.target.channels);
        let place = listens.then(|| skill::Place::beside_bin(app, false));
        steps.extend(skill::steps(app, output, place.as_ref()));
        let mut products = vec![
            bundle.clone(),
            output.join("bin").join(&cli),
            output.join("bin").join(format!("{cli}-mcp")),
            symbols,
        ];
        if listens {
            products.push(output.join(skill::path(app)));
        }
        Ok(BuildPlan {
            steps,
            notes: Vec::new(),
            run: Some(Run::Program(bundle.join("Contents/MacOS").join(&product))),
            products,
        })
    }

    /// A disk image with the app and a link to Applications. The command
    /// line and MCP server go into `Contents/Helpers`, and the agent skill
    /// listing their commands into `Contents/Resources/skills`, so the
    /// installed app carries its entries for agents. Signed with the
    /// identity from [`Signing`] (hardened runtime and a secure timestamp,
    /// as notarization requires) and notarized when a notary profile is
    /// given; otherwise signed for this Mac only.
    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        let product = product_name(app);
        let output = &context.output_dir;
        let cli = app.app.id.replace('_', "-");
        let stage = output.join("package");
        let volume = stage.join("volume");
        let staged = volume.join(format!("{product}.app"));
        let helpers = staged.join("Contents/Helpers");
        let dist = dist_dir(context);
        let image = dist.join(format!(
            "{product}-{}-macos-{}.dmg",
            app.app.version,
            package_architecture()
        ));
        let name = app
            .app
            .name
            .get(&app.languages.default)
            .map(str::to_string)
            .unwrap_or_else(|| product.clone());
        if signing.macos_notary_profile.is_some() && signing.macos_identity.is_none() {
            return Err(vec![Diagnostic::error(
                "BERN_MACOS_NOTARY_PROFILE needs BERN_MACOS_SIGNING_IDENTITY: notarization takes a Developer ID signature",
            )]);
        }
        let run = |description: &str, program: &str, args: Vec<String>| BuildStep::Run {
            description: description.into(),
            program: program.into(),
            args,
            cwd: output.clone(),
            env: vec![],
        };
        let path = |path: &std::path::Path| path.display().to_string();
        // Hardened runtime and a timestamp with a real identity; an ad hoc
        // signature otherwise.
        let sign = |target: &std::path::Path| -> Vec<String> {
            match &signing.macos_identity {
                Some(identity) => vec![
                    "--force".into(),
                    "--options".into(),
                    "runtime".into(),
                    "--timestamp".into(),
                    "--sign".into(),
                    identity.clone(),
                    path(target),
                ],
                None => vec!["--force".into(), "--sign".into(), "-".into(), path(target)],
            }
        };
        let mut steps = vec![
            BuildStep::RemoveIfExists {
                path: stage.clone(),
            },
            BuildStep::CreateDir {
                path: volume.clone(),
            },
            BuildStep::CreateDir { path: dist.clone() },
            BuildStep::RemoveIfExists {
                path: image.clone(),
            },
            run(
                "Copy the app",
                "ditto",
                vec![path(&output.join(format!("{product}.app"))), path(&staged)],
            ),
            BuildStep::CreateDir {
                path: helpers.clone(),
            },
        ];
        for program in [cli.clone(), format!("{cli}-mcp")] {
            steps.push(BuildStep::Copy {
                from: output.join("bin").join(&program),
                to: helpers.join(&program),
            });
            steps.push(run(
                "Sign the command line tools",
                "codesign",
                sign(&helpers.join(&program)),
            ));
        }
        // The agent skill goes with the resources, which the signature
        // seals; it finds the programs in `Helpers`.
        if skill::listens(&context.target.channels) {
            let place = skill::Place {
                cli: format!("../../../Helpers/{cli}"),
                mcp: format!("../../../Helpers/{cli}-mcp"),
                installed: Some(format!(
                    "/Applications/{product}.app/Contents/Helpers/{cli}"
                )),
                connect: None,
            };
            steps.push(BuildStep::Write {
                path: staged.join("Contents/Resources").join(skill::path(app)),
                contents: skill::markdown(app, &place).into_bytes(),
            });
        }
        steps.push(run("Sign the app", "codesign", sign(&staged)));
        steps.push(run(
            "Link Applications",
            "ln",
            vec![
                "-s".into(),
                "/Applications".into(),
                path(&volume.join("Applications")),
            ],
        ));
        // `diskutil image` where macOS has it; `hdiutil create`, which newer
        // systems call deprecated, before that.
        let diskutil = std::process::Command::new("diskutil")
            .args(["image", "create", "from", "--help"])
            .output()
            .is_ok_and(|output| output.status.success());
        steps.push(if diskutil {
            run(
                "Make the disk image",
                "diskutil",
                vec![
                    "image".into(),
                    "create".into(),
                    "from".into(),
                    "--format".into(),
                    "UDZO".into(),
                    "--volumeName".into(),
                    name,
                    path(&volume),
                    path(&image),
                ],
            )
        } else {
            run(
                "Make the disk image",
                "hdiutil",
                vec![
                    "create".into(),
                    "-volname".into(),
                    name,
                    "-srcfolder".into(),
                    path(&volume),
                    "-ov".into(),
                    "-format".into(),
                    "UDZO".into(),
                    path(&image),
                ],
            )
        });
        let mut notes = Vec::new();
        match (&signing.macos_identity, &signing.macos_notary_profile) {
            (Some(identity), profile) => {
                steps.push(run(
                    "Sign the disk image",
                    "codesign",
                    vec![
                        "--force".into(),
                        "--timestamp".into(),
                        "--sign".into(),
                        identity.clone(),
                        path(&image),
                    ],
                ));
                if let Some(profile) = profile {
                    steps.push(run(
                        "Notarize the disk image",
                        "xcrun",
                        vec![
                            "notarytool".into(),
                            "submit".into(),
                            path(&image),
                            "--keychain-profile".into(),
                            profile.clone(),
                            "--wait".into(),
                        ],
                    ));
                    steps.push(run(
                        "Staple the notarization",
                        "xcrun",
                        vec!["stapler".into(), "staple".into(), path(&image)],
                    ));
                } else {
                    notes.push(
                        "the disk image is signed but not notarized; Gatekeeper on other Macs warns about it (set BERN_MACOS_NOTARY_PROFILE)"
                            .into(),
                    );
                }
            }
            (None, _) => notes.push(
                "the app is signed for this Mac only; Gatekeeper on other Macs refuses it (set BERN_MACOS_SIGNING_IDENTITY to a Developer ID Application identity)"
                    .into(),
            ),
        }
        Ok(BuildPlan {
            steps,
            products: vec![image],
            run: None,
            notes,
        })
    }
}

/// What both SwiftUI targets check: the common rules, the bundle layout of
/// the resources, the runtime's own translations, channels, and where
/// `Search` and `Picker` can go.
pub(crate) fn check_swiftui(
    target: &dyn Target,
    app: &Application,
    config: &TargetConfig,
    platform: Platform,
) -> Vec<Diagnostic> {
    let system = platform.system();
    let mut diagnostics = check_common(target, app);
    diagnostics.extend(resources::check_declared(app, platform));
    let framework = resources::framework_languages();
    for language in &app.languages.supported {
        if !framework.contains(&language.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "the SwiftUI runtime has no `{language}` translation of its own texts; supported: {}",
                framework.join(", ")
            )));
        }
    }
    for channel in &config.channels {
        if !target.descriptor().channels.contains(&channel.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "target `{}`: channel `{channel}` is not available on {system}",
                config.name
            )));
        }
    }
    let mut visit = |node: &Node, parent: Option<&str>| {
        if let Node::Element(element) = node {
            if element.kind == "Search"
                && element.keyword("style") == Some("palette")
                && parent != Some("Toolbar")
            {
                diagnostics.push(
                    Diagnostic::error("a search palette goes in a Toolbar")
                        .at(element.source.clone()),
                );
            }
            if element.kind == "Picker"
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
        }
    };
    app.walk(&mut visit);
    diagnostics
}
