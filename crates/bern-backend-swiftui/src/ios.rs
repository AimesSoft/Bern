//! The `swiftui + ios` target.
//!
//! The same generated SwiftUI as on macOS, in the one scene an iOS app has:
//! windows are virtual, the primary filling the scene and the others shown
//! as sheets over it. The engine runs in the app process, linked as a
//! static library built for the iOS simulator; the application CLI and MCP
//! server are built for the Mac and reach the app in the simulator through
//! the IPC channel.

use crate::swift::{Generator, Platform};
use crate::{check_swiftui, component_names, glue, product_name, resources};
use bern_ir::IR_VERSION;
use bern_ir::{Application, WindowInitial};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Target, TargetConfig, TargetDescriptor, WindowCapabilities, icon,
};
use std::path::PathBuf;

/// Minimum iOS: the release with Observation and the standard components
/// the runtime uses, as macOS 14 is on the Mac.
pub const MINIMUM_IOS: &str = "17.0";

/// Name of the application icon in the asset catalog.
pub const ICON_NAME: &str = "AppIcon";

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

pub struct SwiftUiIos;

/// The simulator's architecture as Swift and Rust name it, and the Rust
/// target of the Mac, which runs the command line tools.
fn architectures() -> (&'static str, &'static str, &'static str) {
    match std::env::consts::ARCH {
        "x86_64" => ("x86_64", "x86_64-apple-ios", "x86_64-apple-darwin"),
        _ => ("arm64", "aarch64-apple-ios-sim", "aarch64-apple-darwin"),
    }
}

impl Target for SwiftUiIos {
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
            features: crate::FEATURES,
            id: "swiftui-ios",
            ui: "swiftui",
            platforms: &["ios"],
            version: env!("CARGO_PKG_VERSION"),
            ir_versions: IR_VERSION..=IR_VERSION,
            color_source: ColorSource::Native,
            components: component_names(),
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
        let mut diagnostics = check_swiftui(self, app, config, Platform::Ios);
        for window in &app.windows.windows {
            if window.id == app.windows.primary && window.initial != WindowInitial::Open {
                diagnostics.push(Diagnostic::error(format!(
                    "window `{}`: the primary window fills the iOS app's scene from launch; declare `initial: open`",
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
                Generator::new(app, Platform::Ios).generate(app.protocol.fingerprint()),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app, Platform::Ios)),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                bundle.join("Info.plist"),
                resources::info_plist_ios(app, &product, MINIMUM_IOS),
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
        // An iOS bundle keeps its localizations at its top.
        for (path, table) in resources::localization_files(app) {
            files.push(GeneratedFile::text(bundle.join(path), table));
        }
        // The icon: a declared `.appiconset` as it is (with its dark and
        // tinted images), a declared PNG, or the Bern logo on a full square,
        // which iOS cuts to its rounded shape. `actool` compiles it into
        // the bundle's asset catalog.
        let failed = |message: String| vec![Diagnostic::error(message)];
        let set = PathBuf::from(ASSETS).join(format!("{ICON_NAME}.appiconset"));
        files.push(GeneratedFile::text(
            PathBuf::from(ASSETS).join("Contents.json"),
            "{\n  \"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }\n}\n",
        ));
        if let icon::Source::Native(dir) = icon::source(app, "ios", &context.project_dir) {
            for file in icon::app_icon_set(&dir)
                .map_err(|message| failed(format!("the iOS icon {} {message}", dir.display())))?
            {
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
            let images = icon::images(app, "ios", &context.project_dir, icon::Shape::Masked)
                .map_err(failed)?;
            files.push(GeneratedFile::text(
                set.join("Contents.json"),
                format!(
                    "{{\n  \"images\" : [\n    {{\n      \"filename\" : \"{ICON_NAME}.png\",\n      \"idiom\" : \"universal\",\n      \"platform\" : \"ios\",\n      \"size\" : \"1024x1024\"\n    }}\n  ],\n  \"info\" : {{\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }}\n}}\n"
                ),
            ));
            files.push(GeneratedFile {
                path: set.join(format!("{ICON_NAME}.png")),
                contents: icon::largest(&images).png.to_vec(),
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
            "platform": "ios",
            "destination": "simulator",
            "ir_version": app.ir_version,
            "generator": "bern-backend-swiftui",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "minimum_ios": MINIMUM_IOS,
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
        Ok(files)
    }

    fn plan(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
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
            "iphonesimulator".into(),
            "swiftc".into(),
            "-swift-version".into(),
            "6".into(),
            "-parse-as-library".into(),
            "-module-name".into(),
            product.clone(),
            "-target".into(),
            format!("{architecture}-apple-ios{MINIMUM_IOS}-simulator"),
            "-import-objc-header".into(),
            path(&sources.join("bern.h")),
            "-L".into(),
            path(&rust_output),
            "-lbern_app".into(),
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
        let icon_set = output.join(ASSETS).join(format!("{ICON_NAME}.appiconset"));
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
                description: "Build the Rust engine and bridge for the iOS simulator".into(),
                program: "cargo".into(),
                args: cargo("--lib", rust_target),
                cwd: output.join("glue"),
                env: vec![(
                    "IPHONEOS_DEPLOYMENT_TARGET".to_string(),
                    MINIMUM_IOS.to_string(),
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
                            context.project_dir.join(&resource.name),
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
        ];
        // The asset catalog takes a 1024-pixel icon; a declared
        // `.appiconset` has its own sizes.
        let scale = match icon::source(app, "ios", &context.project_dir) {
            icon::Source::Native(_) => false,
            _ => {
                let images = icon::images(app, "ios", &context.project_dir, icon::Shape::Masked)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                icon::largest(&images).size != 1024
            }
        };
        if scale {
            steps.push(run(
                "Scale the icon to 1024 pixels",
                "sips",
                vec![
                    "-z".into(),
                    "1024".into(),
                    "1024".into(),
                    path(&icon_set.join(format!("{ICON_NAME}.png"))),
                ],
                vec![],
            ));
        }
        steps.extend([
            run(
                "Compile the icon",
                "xcrun",
                vec![
                    "actool".into(),
                    path(&output.join(ASSETS)),
                    "--compile".into(),
                    path(&bundle),
                    "--platform".into(),
                    "iphonesimulator".into(),
                    "--minimum-deployment-target".into(),
                    MINIMUM_IOS.into(),
                    "--app-icon".into(),
                    ICON_NAME.into(),
                    "--output-partial-info-plist".into(),
                    path(&output.join("AssetCatalog.plist")),
                    "--target-device".into(),
                    "iphone".into(),
                    "--target-device".into(),
                    "ipad".into(),
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
        ]);
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
        Ok(BuildPlan {
            steps,
            notes: Vec::new(),
            run: Some(Run::Simulator {
                app: bundle.clone(),
                bundle_id: app.app.bundle_id.clone(),
            }),
            products: vec![
                bundle,
                output.join("bin").join(&cli),
                output.join("bin").join(format!("{cli}-mcp")),
                symbols,
            ],
        })
    }
}
