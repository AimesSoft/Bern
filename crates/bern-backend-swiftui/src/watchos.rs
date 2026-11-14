//! The `swiftui + watchos` target.
//!
//! Generates idiomatic SwiftUI for Apple Watch from the shared IR: one
//! full-screen scene hosting the primary window, with secondary windows
//! presented as watch sheets over it. Every reachable `Adaptive` must
//! declare an explicit `Layout(size: watch)` (`B401`), and only what a
//! watch shows (`Application::on_watch`) is checked and built. The engine
//! runs in the app process as a static library; the application CLI and MCP
//! server build for the Mac and reach the watchOS simulator through the IPC
//! channel (`--engine watchos`). `bern package` produces a Watch-Only
//! `-watchos.ipa` (`ITSWatchOnlyContainer` stub wrapping `Watch/<App>.app`).

use crate::swift::{Generator, Platform};
use crate::{MINIMUM_IOS, check_swiftui, component_names, glue, product_name, resources};
use bern_ir::{Application, IR_VERSION, Node, WindowInitial};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, SimulatedSystem, Target, TargetConfig, TargetDescriptor, WindowCapabilities, feature,
    icon, skill,
};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Minimum watchOS: 11, the release with scroll phase tracking, text
/// renderers and Observation across every 64-bit `arm64` Apple Watch.
pub const MINIMUM_WATCHOS: &str = "11.0";

/// Name of the application icon in the asset catalog.
pub const ICON_NAME: &str = "AppIcon";

const RUNTIME: &str = include_str!("../templates/BernRuntime.swift");
const MEDIA: &str = include_str!("../templates/BernMedia.swift");
const STYLE: &str = include_str!("../templates/BernStyle.swift");
const WATCHKIT: &str = include_str!("../templates/BernWatchKit.swift");
const HEADER: &str = include_str!("../templates/bern.h");

/// Swift sources compiled into the app, in order.
const SOURCES: &[&str] = &[
    "BernRuntime.swift",
    "BernMedia.swift",
    "BernStyle.swift",
    "BernWatchKit.swift",
    "BernGenerated.swift",
];

/// The asset catalog with the watch application icon, outside the bundle.
const ASSETS: &str = "Assets.xcassets";

/// Separate iOS asset catalog for the outer Watch-Only `.ipa` container.
const CONTAINER_ASSETS: &str = "ContainerAssets.xcassets";

/// The Rust target of 64-bit Apple Watch devices.
const DEVICE_TARGET: &str = "aarch64-apple-watchos";

/// IR features SwiftUI builds on watchOS: those of SwiftUI without a
/// document picker (`file_pick`), clipboard (`paste`), pointer hover
/// (`reveal`) or fixed desktop sidebar widths (`sidebar_width`).
pub(crate) const FEATURES: &[&str] = &[
    feature::SCROLL_ANCHOR,
    feature::SCROLL_BROWSING,
    feature::BOUND_BLUR,
    feature::CONTROLLED_VALUES,
    feature::TABLE,
    feature::CONTROLLED_TEXT,
    feature::WATCH,
    feature::LOGIN_SWITCH,
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
    feature::COVER_IMAGES,
    feature::SCROLL_SNAP,
    feature::RAILS,
    feature::POPOVER_BUTTON,
    feature::SIDEBAR_HEADER,
    feature::TEXT_BOLD,
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

pub struct SwiftUiWatchos;

/// The components SwiftUI builds on watchOS: all but `WebPage`, since
/// watchOS has no WebKit.
fn watch_component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        component_names()
            .iter()
            .copied()
            .filter(|name| *name != "WebPage")
            .collect()
    })
}

/// The simulator's architecture as Swift and Rust name it, and the Rust
/// target of the Mac, which runs the command line tools.
fn architectures() -> (&'static str, &'static str, &'static str) {
    match std::env::consts::ARCH {
        "x86_64" => ("x86_64", "x86_64-apple-watchos-sim", "x86_64-apple-darwin"),
        _ => ("arm64", "aarch64-apple-watchos-sim", "aarch64-apple-darwin"),
    }
}

impl Target for SwiftUiWatchos {
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
            id: "swiftui-watchos",
            ui: "swiftui",
            platforms: &["watchos"],
            version: env!("CARGO_PKG_VERSION"),
            ir_versions: IR_VERSION..=IR_VERSION,
            color_source: ColorSource::Native,
            components: watch_component_names(),
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
                presenter: "videoplayer",
                presenter_version: "1",
                background_playback: true,
                multi_view: true,
            }),
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let app = &bern_target_api::svg::prepare(app, config);
        let mut diagnostics = check_swiftui(self, app, config, Platform::Watchos);
        app.walk(&mut |node, _| {
            if let Node::Element(element) = node
                && element.kind == "Adaptive"
                && !element.children.iter().any(|child| {
                    matches!(child, Node::Element(layout) if layout.keyword("size") == Some("watch"))
                })
            {
                diagnostics.push(
                    Diagnostic::error(
                        "watchOS requires an explicit `Layout(size: watch)` in every reachable `Adaptive`; a phone layout is not a watch layout",
                    )
                    .code("B401")
                    .at(element.source.clone()),
                );
            }
        });
        for window in &app.windows.windows {
            if window.id == app.windows.primary && window.initial != WindowInitial::Open {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}`: the primary window fills the watch screen from launch; declare `initial: open`",
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
            GeneratedFile::text("Sources/BernWatchKit.swift", WATCHKIT),
            GeneratedFile::text("Sources/bern.h", HEADER),
            GeneratedFile::text(
                "Sources/BernGenerated.swift",
                Generator::new(app, Platform::Watchos).generate(app.protocol.fingerprint()),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text(
                "glue/src/lib.rs",
                glue::lib_rs(context, app, Platform::Watchos),
            ),
            GeneratedFile::text("tools/Cargo.toml", glue::tools_cargo_toml(context, app)),
            GeneratedFile::text("tools/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("tools/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                bundle.join("Info.plist"),
                resources::info_plist_watchos(app, &product, MINIMUM_WATCHOS),
            ),
        ];
        let lock = context.workspace_dir.join("Cargo.lock");
        if let Ok(contents) = std::fs::read(&lock) {
            for lock_path in ["glue/Cargo.lock", "tools/Cargo.lock"] {
                if !context.output_dir.join(lock_path).exists() {
                    files.push(GeneratedFile {
                        path: lock_path.into(),
                        contents: contents.clone(),
                    });
                }
            }
        }
        for (path, table) in resources::localization_files(app) {
            files.push(GeneratedFile::text(bundle.join(path), table));
        }
        // The watch icon: an Icon Composer `.icon` document beside the
        // catalog, a declared `.appiconset` folder as it is, a declared
        // square PNG, or the Bern logo on a full square that watchOS masks
        // to a circle.
        let failed = |message: String| vec![Diagnostic::error(message)];
        let set = PathBuf::from(ASSETS).join(format!("{ICON_NAME}.appiconset"));
        files.push(GeneratedFile::text(
            PathBuf::from(ASSETS).join("Contents.json"),
            "{\n  \"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }\n}\n",
        ));
        if let Some(document) = crate::icon_document(app, "watchos", &context.project_dir) {
            files.extend(crate::icon_document_files(
                &document,
                &PathBuf::from(format!("{ICON_NAME}.icon")),
            )?);
        } else if let icon::Source::Native(dir) = icon::source(app, "watchos", &context.project_dir)
        {
            for file in icon::app_icon_set(&dir).map_err(|message| {
                failed(format!("the watchOS icon {} {message}", dir.display()))
            })? {
                let contents = std::fs::read(dir.join(&file)).map_err(|error| {
                    failed(format!(
                        "cannot read {}: {error}",
                        dir.join(&file).display()
                    ))
                })?;
                files.push(GeneratedFile {
                    path: set.join(file),
                    contents,
                });
            }
        } else {
            let images = icon::images(app, "watchos", &context.project_dir, icon::Shape::Masked)
                .map_err(failed)?;
            files.push(GeneratedFile::text(
                set.join("Contents.json"),
                format!(
                    "{{\n  \"images\" : [\n    {{\n      \"filename\" : \"{ICON_NAME}.png\",\n      \"idiom\" : \"universal\",\n      \"platform\" : \"watchos\",\n      \"size\" : \"1024x1024\"\n    }}\n  ],\n  \"info\" : {{\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }}\n}}\n"
                ),
            ));
            files.push(GeneratedFile {
                path: set.join(format!("{ICON_NAME}.png")),
                contents: icon::largest(&images).png.to_vec(),
            });
        }
        // Container icon catalog for the Watch-Only `.ipa` wrapper on iPhoneOS.
        let container_set = PathBuf::from(CONTAINER_ASSETS).join(format!("{ICON_NAME}.appiconset"));
        files.push(GeneratedFile::text(
            PathBuf::from(CONTAINER_ASSETS).join("Contents.json"),
            "{\n  \"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }\n}\n",
        ));
        if crate::icon_document(app, "watchos", &context.project_dir).is_none() {
            let container_images =
                icon::images(app, "watchos", &context.project_dir, icon::Shape::Masked)
                    .or_else(|_| {
                        icon::images(app, "ios", &context.project_dir, icon::Shape::Masked)
                    })
                    .unwrap_or_else(|_| icon::default_images(icon::Shape::Masked));
            files.push(GeneratedFile::text(
                container_set.join("Contents.json"),
                format!(
                    "{{\n  \"images\" : [\n    {{\n      \"filename\" : \"{ICON_NAME}.png\",\n      \"idiom\" : \"universal\",\n      \"platform\" : \"ios\",\n      \"size\" : \"1024x1024\"\n    }}\n  ],\n  \"info\" : {{\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }}\n}}\n"
                ),
            ));
            files.push(GeneratedFile {
                path: container_set.join(format!("{ICON_NAME}.png")),
                contents: icon::largest(&container_images).png.to_vec(),
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
            "platform": "watchos",
            "destination": "simulator",
            "ir_version": app.ir_version,
            "generator": "bern-backend-swiftui",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "minimum_watchos": MINIMUM_WATCHOS,
            "architecture": architecture,
            "profile": if context.release { "release" } else { "debug" },
            "engine_deployment": "in-process",
            "ui_bridge": "c-abi",
            "windows": "virtual",
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
        let tools_target = context.workspace_dir.join("target/bern-tools");
        let host_output = tools_target.join(host_target).join(profile);
        let cli = app.app.id.replace('_', "-");
        let path = |path: &Path| path.display().to_string();
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
        let executable = bundle.join(&product);
        let swiftc = swiftc(
            &product,
            context,
            "watchsimulator",
            &format!("{architecture}-apple-watchos{MINIMUM_WATCHOS}-simulator"),
            &rust_output,
            &executable,
        );
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
                description: "Build the Rust engine and bridge for the Apple Watch simulator"
                    .into(),
                program: "cargo".into(),
                args: crate::listing_native_libs(
                    &cargo("--lib", rust_target),
                    &crate::native_libs_file(&rust_output),
                ),
                cwd: output.join("glue"),
                env: vec![(
                    "WATCHOS_DEPLOYMENT_TARGET".to_string(),
                    MINIMUM_WATCHOS.to_string(),
                )],
            },
            BuildStep::Run {
                description: "Build the command line tools for this Mac".into(),
                program: "cargo".into(),
                args: {
                    let mut args = vec![
                        "build".to_string(),
                        "--manifest-path".into(),
                        output.join("tools/Cargo.toml").display().to_string(),
                        "--target-dir".into(),
                        tools_target.display().to_string(),
                        "--target".into(),
                        host_target.into(),
                    ];
                    if context.release {
                        args.push("--release".into());
                    }
                    args
                },
                cwd: output.join("tools"),
                env: vec![(
                    "MACOSX_DEPLOYMENT_TARGET".to_string(),
                    crate::MINIMUM_MACOS.to_string(),
                )],
            },
            BuildStep::CreateDir {
                path: bundle.clone(),
            },
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
                "Compile the icon",
                "xcrun",
                actool(app, context, &bundle, "watchsimulator"),
                vec![],
            ),
            BuildStep::RemoveIfExists {
                path: symbols.clone(),
            },
            run("Compile the SwiftUI frontend", "xcrun", swiftc, vec![]),
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
        let listens = skill::listens(&context.target.channels);
        let place = listens.then(|| {
            skill::Place::beside_bin(app, false).connect(skill::simulator_connect(app, "watchos"))
        });
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
                system: SimulatedSystem::Watchos,
            }),
            products,
        })
    }

    /// A Watch-Only `.ipa` (`com.apple.product-type.application.watchapp2-container`):
    /// an `ITSWatchOnlyContainer` stub bundle in `Payload/<Product>.app`
    /// carrying `Watch/<Product>.app` built for `arm64-apple-watchos`, both
    /// signed ad hoc.
    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        _signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        let product = product_name(app);
        let output = &context.output_dir;
        let profile = if context.release { "release" } else { "debug" };
        let glue_target = context.workspace_dir.join("target/bern-glue");
        let rust_output = glue_target.join(DEVICE_TARGET).join(profile);
        let stage = output.join("package");
        let payload = stage.join("Payload");
        let container = payload.join(format!("{product}.app"));
        let watch_dir = container.join("Watch");
        let staged_watch = watch_dir.join(format!("{product}.app"));
        let watch_executable = staged_watch.join(&product);
        let dist = bern_target_api::dist_dir(context);
        let ipa = dist.join(format!("{product}-{}-watchos.ipa", app.app.version));
        let path = |path: &Path| path.display().to_string();
        let run = |description: &str, program: &str, args: Vec<String>| BuildStep::Run {
            description: description.into(),
            program: program.into(),
            args,
            cwd: output.clone(),
            env: vec![],
        };
        let mut cargo = vec![
            "build".to_string(),
            "--lib".into(),
            "--manifest-path".into(),
            path(&output.join("glue/Cargo.toml")),
            "--target".into(),
            DEVICE_TARGET.into(),
            "--target-dir".into(),
            path(&glue_target),
        ];
        if context.release {
            cargo.push("--release".into());
        }
        let mut steps = vec![
            BuildStep::Run {
                description: "Build the Rust engine and bridge for Apple Watch".into(),
                program: "cargo".into(),
                args: crate::listing_native_libs(&cargo, &crate::native_libs_file(&rust_output)),
                cwd: output.join("glue"),
                env: vec![(
                    "WATCHOS_DEPLOYMENT_TARGET".to_string(),
                    MINIMUM_WATCHOS.to_string(),
                )],
            },
            BuildStep::RemoveIfExists {
                path: stage.clone(),
            },
            BuildStep::CreateDir {
                path: watch_dir.clone(),
            },
            BuildStep::CreateDir { path: dist.clone() },
            BuildStep::RemoveIfExists { path: ipa.clone() },
            // Populate the outer iPhoneOS Watch-Only stub container.
            BuildStep::Write {
                path: container.join("Info.plist"),
                contents: resources::info_plist_watchos_container(app, &product, MINIMUM_IOS)
                    .into_bytes(),
            },
        ];
        for (loc_path, table) in resources::localization_files(app) {
            steps.push(BuildStep::Write {
                path: container.join(loc_path),
                contents: table.into_bytes(),
            });
        }
        let container_executable = container.join(&product);
        if let Some(stub) = ios_stub_binary() {
            steps.push(run(
                "Thin the Watch-Only container stub for arm64",
                "xcrun",
                vec![
                    "lipo".into(),
                    "-thin".into(),
                    "arm64".into(),
                    path(&stub),
                    "-output".into(),
                    path(&container_executable),
                ],
            ));
        } else {
            let stub_c = stage.join("stub.c");
            steps.push(BuildStep::Write {
                path: stub_c.clone(),
                contents: b"int main(void) { return 0; }\n".to_vec(),
            });
            steps.push(run(
                "Compile the Watch-Only container stub",
                "xcrun",
                vec![
                    "--sdk".into(),
                    "iphoneos".into(),
                    "clang".into(),
                    "-target".into(),
                    format!("arm64-apple-ios{MINIMUM_IOS}"),
                    path(&stub_c),
                    "-o".into(),
                    path(&container_executable),
                ],
            ));
            steps.push(BuildStep::RemoveIfExists { path: stub_c });
        }
        steps.extend([
            run(
                "Compile the Watch-Only container icon",
                "xcrun",
                container_actool(app, context, &container),
            ),
            // Copy and rebuild the embedded watchOS app bundle.
            run(
                "Copy the watchOS app",
                "ditto",
                vec![
                    path(&output.join(format!("{product}.app"))),
                    path(&staged_watch),
                ],
            ),
            BuildStep::RemoveIfExists {
                path: staged_watch.join("_CodeSignature"),
            },
            run(
                "Compile the watch icon for devices",
                "xcrun",
                actool(app, context, &staged_watch, "watchos"),
            ),
            run(
                "Compile the SwiftUI frontend for Apple Watch",
                "xcrun",
                swiftc(
                    &product,
                    context,
                    "watchos",
                    &format!("arm64-apple-watchos{MINIMUM_WATCHOS}"),
                    &rust_output,
                    &watch_executable,
                ),
            ),
            BuildStep::RemoveIfExists {
                path: staged_watch.join(format!("{product}.dSYM")),
            },
        ]);
        if context.release {
            steps.push(run(
                "Strip the symbols from the watch executable",
                "xcrun",
                vec![
                    "strip".into(),
                    "-S".into(),
                    "-x".into(),
                    path(&watch_executable),
                ],
            ));
        }
        let watch_plist = staged_watch.join("Info.plist");
        steps.extend([
            run(
                "Mark the watch app for devices",
                "plutil",
                vec![
                    "-replace".into(),
                    "CFBundleSupportedPlatforms".into(),
                    "-json".into(),
                    r#"["WatchOS"]"#.into(),
                    path(&watch_plist),
                ],
            ),
            run(
                "Set the embedded watch bundle identifier",
                "plutil",
                vec![
                    "-replace".into(),
                    "CFBundleIdentifier".into(),
                    "-string".into(),
                    format!("{}.watchkitapp", app.app.bundle_id),
                    path(&watch_plist),
                ],
            ),
            run(
                "Link the watch app to its container bundle identifier",
                "plutil",
                vec![
                    "-replace".into(),
                    "WKCompanionAppBundleIdentifier".into(),
                    "-string".into(),
                    app.app.bundle_id.clone(),
                    path(&watch_plist),
                ],
            ),
            run(
                "Sign the embedded watch app ad hoc",
                "codesign",
                vec![
                    "--force".into(),
                    "--sign".into(),
                    "-".into(),
                    path(&staged_watch),
                ],
            ),
            run(
                "Sign the Watch-Only container ad hoc",
                "codesign",
                vec![
                    "--force".into(),
                    "--sign".into(),
                    "-".into(),
                    path(&container),
                ],
            ),
            run(
                "Make the .ipa",
                "ditto",
                vec![
                    "-c".into(),
                    "-k".into(),
                    "--norsrc".into(),
                    "--keepParent".into(),
                    path(&payload),
                    path(&ipa),
                ],
            ),
        ]);
        Ok(BuildPlan {
            steps,
            products: vec![ipa],
            run: None,
            notes: vec![
                "the .ipa is signed ad hoc; an Apple Watch installs it once a tool signs it again with an Apple ID or a developer certificate"
                    .into(),
            ],
        })
    }
}

/// Xcode's Watch-Only iOS container stub binary (`MessagesApplicationStub`),
/// if present in the active iPhoneOS platform.
fn ios_stub_binary() -> Option<PathBuf> {
    let platform = Command::new("xcrun")
        .args(["--sdk", "iphoneos", "--show-sdk-platform-path"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())?;
    let stub = Path::new(&platform)
        .join("Library/Application Support/MessagesApplicationStub/MessagesApplicationStub");
    stub.is_file().then_some(stub)
}

/// The `xcrun` arguments that compile the frontend for `sdk`, linked with
/// the engine in `rust_output`.
fn swiftc(
    product: &str,
    context: &BuildContext,
    sdk: &str,
    triple: &str,
    rust_output: &Path,
    executable: &Path,
) -> Vec<String> {
    let path = |path: &Path| path.display().to_string();
    let sources = context.output_dir.join("Sources");
    let mut args = vec![
        "--sdk".to_string(),
        sdk.into(),
        "swiftc".into(),
        "-swift-version".into(),
        "6".into(),
        "-parse-as-library".into(),
        "-module-name".into(),
        product.into(),
        "-target".into(),
        triple.into(),
        "-import-objc-header".into(),
        path(&sources.join("bern.h")),
        "-L".into(),
        path(rust_output),
        "-lbern_app".into(),
        format!("@{}", path(&crate::native_libs_file(rust_output))),
        "-Xlinker".into(),
        "-no_warn_duplicate_libraries".into(),
        "-framework".into(),
        "SwiftUI".into(),
        "-framework".into(),
        "WatchKit".into(),
        "-framework".into(),
        "UIKit".into(),
        "-framework".into(),
        "AVFoundation".into(),
        "-framework".into(),
        "AVKit".into(),
        "-module-cache-path".into(),
        path(&context.workspace_dir.join("target/bern-swift-module-cache")),
    ];
    args.extend(if context.release {
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
        args.push(path(&sources.join(file)));
    }
    args.push("-o".into());
    args.push(path(executable));
    args
}

/// The `xcrun` arguments that compile the watch asset catalog into `bundle`
/// for `platform` (`watchsimulator` or `watchos`).
fn actool(app: &Application, context: &BuildContext, bundle: &Path, platform: &str) -> Vec<String> {
    let output = &context.output_dir;
    let path = |path: &Path| path.display().to_string();
    let mut catalogs = vec![path(&output.join(ASSETS))];
    if crate::icon_document(app, "watchos", &context.project_dir).is_some() {
        catalogs.push(path(&output.join(format!("{ICON_NAME}.icon"))));
    }
    [
        vec!["actool".into()],
        catalogs,
        vec![
            "--compile".into(),
            path(bundle),
            "--platform".into(),
            platform.into(),
            "--minimum-deployment-target".into(),
            MINIMUM_WATCHOS.into(),
            "--app-icon".into(),
            ICON_NAME.into(),
            "--output-partial-info-plist".into(),
            path(&output.join("AssetCatalog.plist")),
            "--target-device".into(),
            "watch".into(),
            "--output-format".into(),
            "human-readable-text".into(),
            "--errors".into(),
            "--warnings".into(),
        ],
    ]
    .concat()
}

/// The `xcrun` arguments that compile the outer Watch-Only container's icon
/// into `container` for `iphoneos`.
fn container_actool(app: &Application, context: &BuildContext, container: &Path) -> Vec<String> {
    let output = &context.output_dir;
    let path = |path: &Path| path.display().to_string();
    let mut catalogs = vec![path(&output.join(CONTAINER_ASSETS))];
    if crate::icon_document(app, "watchos", &context.project_dir).is_some() {
        catalogs.push(path(&output.join(format!("{ICON_NAME}.icon"))));
    }
    [
        vec!["actool".into()],
        catalogs,
        vec![
            "--compile".into(),
            path(container),
            "--platform".into(),
            "iphoneos".into(),
            "--minimum-deployment-target".into(),
            MINIMUM_IOS.into(),
            "--app-icon".into(),
            ICON_NAME.into(),
            "--output-partial-info-plist".into(),
            path(&output.join("ContainerAssetCatalog.plist")),
            "--target-device".into(),
            "iphone".into(),
            "--target-device".into(),
            "ipad".into(),
            "--output-format".into(),
            "human-readable-text".into(),
            "--errors".into(),
            "--warnings".into(),
        ],
    ]
    .concat()
}
