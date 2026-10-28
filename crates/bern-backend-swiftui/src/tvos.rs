//! The `swiftui + tvos` target.
//!
//! The generated SwiftUI of iOS, on a television: one full-screen scene
//! driven by the focus engine, with the Siri Remote instead of touch or a
//! pointer. Windows are virtual, the primary filling the screen and the
//! others shown as sheets over it. Controls are tvOS's own, with the
//! system's focus effects: buttons lift and turn white under the focus,
//! cards tilt. The engine runs in the app process, linked as a static
//! library built for the Apple TV simulator; the application CLI and MCP
//! server are built for the Mac and reach the app in the simulator through
//! the IPC channel. Only what a television shows is built: an `Adaptive`
//! with a `tv` layout shows that layout, and `window.tv` is known (see
//! `Application::on_television`).

use crate::swift::{Generator, Platform};
use crate::{check_swiftui, component_names, glue, product_name, resources};
use bern_ir::IR_VERSION;
use bern_ir::{Application, WindowInitial};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    SimulatedSystem, Target, TargetConfig, TargetDescriptor, WindowCapabilities, brand_assets,
    feature, icon, skill,
};
use std::path::PathBuf;

/// Minimum tvOS: 18, the release with Observation, switch toggles and the
/// tab views the runtime uses; it runs on every Apple TV that tvOS 17 ran
/// on (Apple TV HD and every Apple TV 4K).
pub const MINIMUM_TVOS: &str = "18.0";

/// Name of the application icon in the asset catalog: the brand assets
/// with the layered icon and the Top Shelf images.
pub const ICON_NAME: &str = brand_assets::NAME;

const RUNTIME: &str = include_str!("../templates/BernRuntime.swift");
const MEDIA: &str = include_str!("../templates/BernMedia.swift");
const STYLE: &str = include_str!("../templates/BernStyle.swift");
const UIKIT: &str = include_str!("../templates/BernUIKit.swift");
const HEADER: &str = include_str!("../templates/bern.h");

/// Swift sources compiled into the app, in order.
const SOURCES: &[&str] = &[
    "BernRuntime.swift",
    "BernMedia.swift",
    "BernStyle.swift",
    "BernUIKit.swift",
    "BernGenerated.swift",
];

/// The asset catalog with the application icon, outside the bundle.
const ASSETS: &str = "Assets.xcassets";

/// IR features SwiftUI builds on tvOS: those of macOS and iOS, without the
/// ones a television has no system part for. tvOS has no open panel or
/// file system to pick from (`file_pick`), no pointer to hover with
/// (`reveal`, whose content then always shows, as the focus engine reaches
/// it like any other), and its sidebar is a floating panel of the system's
/// width (`sidebar_width`). A television is always landscape, so
/// `window_landscape` has nothing to turn and nothing to warn about.
/// A popover's content shows in a sheet, tvOS's own way to show more over a
/// screen; keyboard shortcuts are the Siri Remote's buttons.
pub(crate) const FEATURES: &[&str] = &[
    feature::CONTROLLED_VALUES,
    // A list of row buttons stands for the table tvOS has not; there is
    // no clipboard, so no `paste`.
    feature::TABLE,
    feature::CONTROLLED_TEXT,
    feature::BUTTON_CONTENT,
    feature::TOOLBAR_SEARCH,
    feature::SEARCH_RESULTS,
    feature::SESSION_ACTIONS,
    feature::TOGGLE_BUTTON,
    feature::DIALOG_WINDOWS,
    feature::BOUND_LENGTHS,
    feature::APPEARANCE,
    feature::SHORTCUTS,
    feature::BOUND_SHORTCUTS,
    feature::SHORTCUT_INPUT,
    feature::MENU_BAR,
    feature::STATUS_ITEM,
    feature::RUNTIME_THEME,
    feature::RUNTIME_PALETTE,
    feature::RUNTIME_IMAGES,
    feature::PICTURE_FILLS,
    feature::SCROLL_SNAP,
    feature::RAILS,
    feature::POPOVER_BUTTON,
    feature::SIDEBAR_HEADER,
    feature::TEXT_WEIGHTS,
    feature::TEXT_LINES,
    feature::IMAGE_CORNERS,
    feature::IMAGE_TEMPLATE,
    feature::ADAPTIVE_IMAGES,
    feature::ACCORDION_LAYOUT,
    feature::SCALED_SCENE,
    feature::CAPSULE_CONTROLS,
    feature::MODAL_FLOW_TIMER,
    feature::PAGE_BADGES,
    feature::EDGE_GROW,
    feature::DELAYED_APPEAR,
    feature::ROW_BASELINE,
    feature::ICON_MENUS,
    feature::ICON_LABELS,
    feature::ICON_TABS,
    feature::TOOLBAR_LEADING,
    feature::AXIS_PADDING,
    feature::UNTITLED_DETAIL,
    feature::IDLE_CONTROL,
    feature::FILL_BUTTONS,
    feature::GRID_CELLS,
    feature::TAB_ACCESSORY,
    feature::PAGE_SEARCH,
    feature::FORM_LAYOUT,
    feature::MODAL_COVER,
    feature::WINDOW_SHORT,
    feature::WINDOW_LANDSCAPE,
    feature::SEARCH_PALETTE,
    feature::SIZE_CLASSES,
    feature::TEXT_FORMATS,
];

pub struct SwiftUiTvos;

/// The components SwiftUI builds on tvOS: all but `Svg`, whose animated
/// document needs a web engine, and tvOS has none (no WebKit).
fn tv_component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        component_names()
            .iter()
            .copied()
            .filter(|name| *name != "Svg")
            .collect()
    })
}

/// The simulator's architecture as Swift and Rust name it, and the Rust
/// target of the Mac, which runs the command line tools.
fn architectures() -> (&'static str, &'static str, &'static str) {
    match std::env::consts::ARCH {
        "x86_64" => ("x86_64", "x86_64-apple-tvos", "x86_64-apple-darwin"),
        _ => ("arm64", "aarch64-apple-tvos-sim", "aarch64-apple-darwin"),
    }
}

impl Target for SwiftUiTvos {
    fn build_hosts(&self) -> &'static [&'static str] {
        &["macos"]
    }

    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
            styles: crate::style::SUPPORT,
            motion: crate::MOTION,
            window_chrome: true,
            text_marks: true,
            dynamic_items: true,
            features: FEATURES,
            id: "swiftui-tvos",
            ui: "swiftui",
            platforms: &["tvos"],
            version: env!("CARGO_PKG_VERSION"),
            ir_versions: IR_VERSION..=IR_VERSION,
            color_source: ColorSource::Native,
            components: tv_component_names(),
            windows: WindowCapabilities {
                native: false,
                virtual_windows: true,
                floating: false,
            },
            channels: &["ipc"],
            media: Some(MediaSupport {
                contract_versions: bern_media::MEDIA_CONTRACT_VERSION
                    ..=bern_media::MEDIA_CONTRACT_VERSION,
                backend: "avfoundation",
                backend_version: "1",
                presenter: "avplayerlayer",
                presenter_version: "1",
                background_playback: true,
                multi_view: true,
            }),
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let app = &bern_target_api::svg::prepare(app, config);
        // Only what a television shows is built, and checked.
        let app = &app.on_television();
        let mut diagnostics = check_swiftui(self, app, config, Platform::Tvos);
        for window in &app.windows.windows {
            if window.id == app.windows.primary && window.initial != WindowInitial::Open {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}`: the primary window fills the television from launch; declare `initial: open`",
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
        let app = &app.on_television();
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
            GeneratedFile::text("Sources/BernUIKit.swift", UIKIT),
            GeneratedFile::text("Sources/bern.h", HEADER),
            GeneratedFile::text(
                "Sources/BernGenerated.swift",
                Generator::new(app, Platform::Tvos).generate(app.protocol.fingerprint()),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text(
                "glue/src/lib.rs",
                glue::lib_rs(context, app, Platform::Tvos),
            ),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                bundle.join("Info.plist"),
                resources::info_plist_tvos(app, &product, MINIMUM_TVOS),
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
        // A tvOS bundle keeps its localizations at its top, as iOS's does.
        for (path, table) in resources::localization_files(app) {
            files.push(GeneratedFile::text(bundle.join(path), table));
        }
        // The icon: a declared `.brandassets` folder as it is, or one drawn
        // from the declared square PNG or the Bern logo: the icon floating
        // over a back layer of its own ground, and the Top Shelf images.
        // `actool` compiles it into the bundle's asset catalog.
        let failed = |message: String| vec![Diagnostic::error(message)];
        let set = PathBuf::from(ASSETS).join(format!("{ICON_NAME}.brandassets"));
        files.push(GeneratedFile::text(
            PathBuf::from(ASSETS).join("Contents.json"),
            "{\n  \"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }\n}\n",
        ));
        let drawn = match icon::source(app, "tvos", &context.project_dir) {
            icon::Source::Native(dir) => {
                let listed = brand_assets::check(&dir).map_err(|message| {
                    failed(format!("the tvOS icon {} {message}", dir.display()))
                })?;
                let mut read = Vec::new();
                for file in listed {
                    let contents = std::fs::read(dir.join(&file)).map_err(|error| {
                        failed(format!(
                            "cannot read {}: {error}",
                            dir.join(&file).display()
                        ))
                    })?;
                    read.push((file, contents));
                }
                read
            }
            _ => {
                let images = icon::images(app, "tvos", &context.project_dir, icon::Shape::Masked)
                    .map_err(failed)?;
                brand_assets::generate(&icon::largest(&images).png)
                    .map_err(|message| failed(format!("the tvOS icon {message}")))?
            }
        };
        for (path, contents) in drawn {
            files.push(GeneratedFile {
                path: set.join(path),
                contents,
            });
        }
        let (architecture, _, _) = architectures();
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
            "platform": "tvos",
            "destination": "simulator",
            "ir_version": app.ir_version,
            "generator": "bern-backend-swiftui",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "minimum_tvos": MINIMUM_TVOS,
            "architecture": architecture,
            "profile": if context.release { "release" } else { "debug" },
            "engine_deployment": "in-process",
            "ui_bridge": "c-abi",
            "windows": "virtual",
            "input": "focus",
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
        let (architecture, rust_target, host_target) = architectures();
        let profile = if context.release { "release" } else { "debug" };
        let glue_target = context.workspace_dir.join("target/bern-glue");
        let rust_output = glue_target.join(rust_target).join(profile);
        let host_output = glue_target.join(host_target).join(profile);
        let cli = app.app.id.replace('_', "-");
        let path = |path: &std::path::Path| path.display().to_string();
        let cargo = |what: &str, target: &str| {
            let mut args = vec![
                "build".to_string(),
                what.into(),
                "--manifest-path".into(),
                path(&output.join("glue/Cargo.toml")),
                "--target".into(),
                target.into(),
                "--target-dir".into(),
                path(&glue_target),
            ];
            if context.release {
                args.push("--release".into());
            }
            args
        };
        let sources = output.join("Sources");
        let mut swiftc = vec![
            "--sdk".to_string(),
            "appletvsimulator".into(),
            "swiftc".into(),
            "-swift-version".into(),
            "6".into(),
            "-parse-as-library".into(),
            "-module-name".into(),
            product.clone(),
            "-target".into(),
            format!("{architecture}-apple-tvos{MINIMUM_TVOS}-simulator"),
            "-import-objc-header".into(),
            path(&sources.join("bern.h")),
            "-L".into(),
            path(&rust_output),
            "-lbern_app".into(),
            format!("@{}", path(&crate::native_libs_file(&rust_output))),
            // rustc's list repeats libraries swiftc links anyway.
            "-Xlinker".into(),
            "-no_warn_duplicate_libraries".into(),
            "-framework".into(),
            "SwiftUI".into(),
            "-framework".into(),
            "UIKit".into(),
            "-framework".into(),
            "AVFoundation".into(),
            "-framework".into(),
            "AVKit".into(),
            "-module-cache-path".into(),
            path(&context.workspace_dir.join("target/bern-swift-module-cache")),
        ];
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
            swiftc.push(path(&sources.join(file)));
        }
        let executable = bundle.join(&product);
        swiftc.push("-o".into());
        swiftc.push(path(&executable));
        let linked_symbols = bundle.join(format!("{product}.dSYM"));
        let symbols = output.join(format!("{product}.app.dSYM"));
        let run =
            |description: &str, program: &str, args: Vec<String>, env: Vec<(String, String)>| {
                BuildStep::Run {
                    description: description.into(),
                    program: program.into(),
                    args,
                    cwd: output.clone(),
                    env,
                }
            };

        let mut steps = vec![
            BuildStep::Run {
                description: "Build the Rust engine and bridge for the Apple TV simulator".into(),
                program: "cargo".into(),
                args: crate::listing_native_libs(
                    &cargo("--lib", rust_target),
                    &crate::native_libs_file(&rust_output),
                ),
                cwd: output.join("glue"),
                env: vec![(
                    "TVOS_DEPLOYMENT_TARGET".to_string(),
                    MINIMUM_TVOS.to_string(),
                )],
            },
            BuildStep::Run {
                description: "Build the command line tools for this Mac".into(),
                program: "cargo".into(),
                args: cargo("--bins", host_target),
                cwd: output.join("glue"),
                env: vec![(
                    "MACOSX_DEPLOYMENT_TARGET".to_string(),
                    crate::MINIMUM_MACOS.to_string(),
                )],
            },
            BuildStep::CreateDir {
                path: bundle.clone(),
            },
            // Declared resources ship at the top of the bundle next to the
            // localizations and Info.plist; the steps below add the rest.
            BuildStep::Mirror {
                dir: bundle.clone(),
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
                    .chain([PathBuf::from("Info.plist")])
                    .collect(),
            },
            run(
                "Compile the icon and Top Shelf images",
                "xcrun",
                vec![
                    "actool".into(),
                    path(&output.join(ASSETS)),
                    "--compile".into(),
                    path(&bundle),
                    "--platform".into(),
                    "appletvsimulator".into(),
                    "--minimum-deployment-target".into(),
                    MINIMUM_TVOS.into(),
                    "--app-icon".into(),
                    ICON_NAME.into(),
                    "--output-partial-info-plist".into(),
                    path(&output.join("AssetCatalog.plist")),
                    "--target-device".into(),
                    "tv".into(),
                    "--output-format".into(),
                    "human-readable-text".into(),
                    "--errors".into(),
                    "--warnings".into(),
                ],
                vec![],
            ),
            BuildStep::RemoveIfExists {
                path: symbols.clone(),
            },
            run("Compile the SwiftUI frontend", "xcrun", swiftc, vec![]),
            // swiftc writes the dSYM next to the executable, inside the
            // bundle; it goes beside the bundle instead.
            run(
                "Move the debug symbols out of the bundle",
                "mv",
                vec![path(&linked_symbols), path(&symbols)],
                vec![],
            ),
        ];
        if context.release {
            steps.push(run(
                "Strip the symbols the dSYM keeps from the executable",
                "xcrun",
                vec!["strip".into(), "-S".into(), "-x".into(), path(&executable)],
                vec![],
            ));
        }
        steps.extend([
            BuildStep::CreateDir {
                path: output.join("bin"),
            },
            BuildStep::Copy {
                from: host_output.join(&cli),
                to: output.join("bin").join(&cli),
            },
            BuildStep::Copy {
                from: host_output.join(format!("{cli}-mcp")),
                to: output.join("bin").join(format!("{cli}-mcp")),
            },
            run(
                "Sign the app for the simulator",
                "codesign",
                vec!["--force".into(), "--sign".into(), "-".into(), path(&bundle)],
                vec![],
            ),
        ]);
        // The agent skill beside `bin/`, where the app listens.
        let listens = skill::listens(&context.target.channels);
        let place = listens
            .then(|| skill::Place::beside_bin(app, false).connect(skill::simulator_connect(app)));
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
            run: Some(Run::Simulator {
                app: bundle,
                bundle_id: app.app.bundle_id.clone(),
                system: SimulatedSystem::Tvos,
            }),
            products,
        })
    }
}
