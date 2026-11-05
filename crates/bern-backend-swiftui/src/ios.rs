//! The `swiftui + ios` target.
//!
//! The same generated SwiftUI as on macOS, in the one scene an iOS app has:
//! windows are virtual, the primary filling the scene and the others shown
//! as sheets over it. The engine runs in the app process, linked as a
//! static library built for the iOS simulator; the application CLI and MCP
//! server are built for the Mac and reach the app in the simulator through
//! the IPC channel. The package builds the app again for devices.

use crate::swift::{Generator, Platform};
use crate::{check_swiftui, component_names, glue, ios16, product_name, resources};
use bern_ir::IR_VERSION;
use bern_ir::{Application, WindowInitial};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, SimulatedSystem, Target, TargetConfig, TargetDescriptor, WindowCapabilities, icon,
    skill,
};
use std::path::{Path, PathBuf};

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

/// The framework of the front end of iOS 17 and later, in an app of both.
const UI_FRAMEWORK: &str = "BernUI17";

/// The framework of the engine, which both front ends of an app share.
const ENGINE_FRAMEWORK: &str = "BernEngine";

/// What a target builds, from its `frontends`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FrontEnds {
    /// The front end of iOS 17 and later, the engine linked in.
    Current,
    /// The front end of iOS 16, which also runs on later systems.
    Ios16,
    /// The executable is the front end of iOS 16; the other is a framework
    /// loaded on iOS 17 and later, and the engine a framework of both.
    Both,
    /// Two apps, each the one front end with the engine linked in: the
    /// build makes the app of iOS 17 and later, and the package both.
    Separate,
}

impl FrontEnds {
    fn of(target: &TargetConfig) -> Self {
        let named = |name: &str| target.frontends.iter().any(|frontend| frontend == name);
        match (named("swiftui"), named("swiftui-16")) {
            (_, false) => Self::Current,
            (false, true) => Self::Ios16,
            (true, true) if target.separate_packages => Self::Separate,
            (true, true) => Self::Both,
        }
    }

    /// The earliest system the app runs on.
    fn minimum(self) -> &'static str {
        match self {
            Self::Current | Self::Separate => MINIMUM_IOS,
            Self::Ios16 | Self::Both => ios16::MINIMUM,
        }
    }

    /// The earliest system the engine is built for.
    fn engine_minimum(self) -> &'static str {
        match self {
            Self::Current => MINIMUM_IOS,
            _ => ios16::MINIMUM,
        }
    }
}

/// A framework's executable in `frameworks`.
fn framework(frameworks: &Path, name: &str) -> PathBuf {
    frameworks.join(format!("{name}.framework")).join(name)
}

/// The `xcrun` arguments that link the engine in `rust_output` into its
/// framework, which shows the bridge's functions and nothing else.
fn engine_link(sdk: &str, triple: &str, rust_output: &Path, frameworks: &Path) -> Vec<String> {
    let path = |path: &Path| path.display().to_string();
    vec![
        "--sdk".to_string(),
        sdk.into(),
        "clang".into(),
        "-dynamiclib".into(),
        "-target".into(),
        triple.into(),
        format!(
            "-Wl,-force_load,{}",
            path(&rust_output.join("libbern_app.a"))
        ),
        format!("@{}", path(&crate::native_libs_file(rust_output))),
        "-Wl,-dead_strip".into(),
        "-Wl,-exported_symbol,_bern_*".into(),
        "-Wl,-no_warn_duplicate_libraries".into(),
        "-install_name".into(),
        format!("@rpath/{ENGINE_FRAMEWORK}.framework/{ENGINE_FRAMEWORK}"),
        "-o".into(),
        path(&framework(frameworks, ENGINE_FRAMEWORK)),
    ]
}

/// The asset catalog with the application icon, outside the bundle.
const ASSETS: &str = "Assets.xcassets";

/// The Rust target of iPhones and iPads, which the package is built for.
const DEVICE_TARGET: &str = "aarch64-apple-ios";

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
        let app = &bern_target_api::svg::prepare(app, config);
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
        let app = &bern_target_api::svg::prepare(app, &context.target);
        bern_target_api::svg::validate(app, context)?;
        let product = product_name(app);
        let bundle = PathBuf::from(format!("{product}.app"));
        let frontends = FrontEnds::of(&context.target);
        let mut files = swift_sources(app, frontends);
        files.extend([
            GeneratedFile::text("Sources/bern.h", HEADER),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app, Platform::Ios)),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                bundle.join("Info.plist"),
                resources::info_plist_ios(app, &product, frontends.minimum()),
            ),
        ]);
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
        // The icon: a declared Icon Composer document beside the asset
        // catalog, a declared `.appiconset` as it is (with its dark and
        // tinted images), a declared PNG, or the Bern logo on a full square,
        // which iOS cuts to its rounded shape. `actool` compiles it into
        // the bundle's asset catalog.
        let failed = |message: String| vec![Diagnostic::error(message)];
        let set = PathBuf::from(ASSETS).join(format!("{ICON_NAME}.appiconset"));
        files.push(GeneratedFile::text(
            PathBuf::from(ASSETS).join("Contents.json"),
            "{\n  \"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }\n}\n",
        ));
        if let Some(document) = crate::icon_document(app, "ios", &context.project_dir) {
            files.extend(crate::icon_document_files(
                &document,
                &PathBuf::from(format!("{ICON_NAME}.icon")),
            )?);
        } else if let icon::Source::Native(dir) = icon::source(app, "ios", &context.project_dir) {
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
        // The icons `WindowSet(icon:)` switches to, under their asset names:
        // an Icon Composer document beside the catalog, a declared
        // `.appiconset` as it is, or a 1024-pixel PNG in a set of its own.
        for (name, source) in crate::alternate_icons(app, "ios", &context.project_dir)? {
            let asset = crate::alternate_icon(&name);
            let set = PathBuf::from(ASSETS).join(format!("{asset}.appiconset"));
            match source {
                icon::Source::Native(path) if icon::is_icon_document(&path) => {
                    files.extend(crate::icon_document_files(
                        &path,
                        &PathBuf::from(format!("{asset}.icon")),
                    )?);
                }
                icon::Source::Native(dir) => {
                    for file in icon::app_icon_set(&dir).map_err(|message| {
                        failed(format!("the iOS icon {} {message}", dir.display()))
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
                }
                icon::Source::Png(path) => {
                    let contents = std::fs::read(&path).map_err(|error| {
                        failed(format!("cannot read {}: {error}", path.display()))
                    })?;
                    if icon::png_size(&contents) != Some((1024, 1024)) {
                        return Err(failed(format!(
                            "the iOS icon {} is not 1024 × 1024 pixels; an alternate icon takes that size",
                            path.display()
                        )));
                    }
                    files.push(GeneratedFile::text(
                        set.join("Contents.json"),
                        format!(
                            "{{\n  \"images\" : [\n    {{\n      \"filename\" : \"{asset}.png\",\n      \"idiom\" : \"universal\",\n      \"platform\" : \"ios\",\n      \"size\" : \"1024x1024\"\n    }}\n  ],\n  \"info\" : {{\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }}\n}}\n"
                        ),
                    ));
                    files.push(GeneratedFile {
                        path: set.join(format!("{asset}.png")),
                        contents,
                    });
                }
                icon::Source::Logo => {}
            }
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
            "minimum_ios": FrontEnds::of(&context.target).minimum(),
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
        let executable = bundle.join(&product);
        let frontends = FrontEnds::of(&context.target);
        let minimum = frontends.minimum();
        let frameworks = bundle.join("Frameworks");
        let swiftc = swiftc(
            &product,
            context,
            "iphonesimulator",
            &format!("{architecture}-apple-ios{minimum}-simulator"),
            &rust_output,
            &executable,
            frontends,
            Part::App,
        );
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
                args: crate::listing_native_libs(
                    &cargo("--lib", rust_target),
                    &crate::native_libs_file(&rust_output),
                ),
                cwd: output.join("glue"),
                env: vec![(
                    "IPHONEOS_DEPLOYMENT_TARGET".to_string(),
                    frontends.engine_minimum().to_string(),
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
                actool(app, context, &bundle, "iphonesimulator")?,
                vec![],
            ),
            BuildStep::RemoveIfExists {
                path: symbols.clone(),
            },
        ]);
        if frontends == FrontEnds::Both {
            let ui = framework(&frameworks, UI_FRAMEWORK);
            for name in [ENGINE_FRAMEWORK, UI_FRAMEWORK] {
                let dir = frameworks.join(format!("{name}.framework"));
                steps.extend([
                    BuildStep::CreateDir { path: dir.clone() },
                    BuildStep::Copy {
                        from: output.join(format!("Frameworks/{name}.plist")),
                        to: dir.join("Info.plist"),
                    },
                ]);
            }
            steps.extend([
                run(
                    "Link the engine into its framework",
                    "xcrun",
                    engine_link(
                        "iphonesimulator",
                        &format!("{architecture}-apple-ios{minimum}-simulator"),
                        &rust_output,
                        &frameworks,
                    ),
                    vec![],
                ),
                run(
                    "Compile the front end of iOS 17 and later",
                    "xcrun",
                    self::swiftc(
                        &product,
                        context,
                        "iphonesimulator",
                        &format!("{architecture}-apple-ios{MINIMUM_IOS}-simulator"),
                        &rust_output,
                        &ui,
                        frontends,
                        Part::Ui17,
                    ),
                    vec![],
                ),
                // As the executable's: beside the bundle.
                BuildStep::RemoveIfExists {
                    path: output.join(format!("{UI_FRAMEWORK}.framework.dSYM")),
                },
                run(
                    "Move the framework's debug symbols out of the bundle",
                    "mv",
                    vec![
                        path(&ui.with_extension("dSYM")),
                        path(&output.join(format!("{UI_FRAMEWORK}.framework.dSYM"))),
                    ],
                    vec![],
                ),
            ]);
            if context.release {
                for binary in [ui, framework(&frameworks, ENGINE_FRAMEWORK)] {
                    steps.push(run(
                        "Strip the symbols the dSYM keeps from a framework",
                        "xcrun",
                        vec!["strip".into(), "-S".into(), "-x".into(), path(&binary)],
                        vec![],
                    ));
                }
            }
        }
        steps.extend([
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
        ]);
        if frontends == FrontEnds::Both {
            for name in [ENGINE_FRAMEWORK, UI_FRAMEWORK] {
                let dir = frameworks.join(format!("{name}.framework"));
                steps.push(run(
                    "Sign a framework for the simulator",
                    "codesign",
                    vec!["--force".into(), "--sign".into(), "-".into(), path(&dir)],
                    vec![],
                ));
            }
        }
        steps.push(run(
            "Sign the app for the simulator",
            "codesign",
            vec!["--force".into(), "--sign".into(), "-".into(), path(&bundle)],
            vec![],
        ));
        // The agent skill beside `bin/`, where the app listens.
        let listens = skill::listens(&context.target.channels);
        let place = listens.then(|| {
            skill::Place::beside_bin(app, false).connect(skill::simulator_connect(app, "ios"))
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
                system: SimulatedSystem::Ios,
            }),
            products,
        })
    }

    /// An `.ipa` for iPhones and iPads: the bundle the build made, with the
    /// engine, the frontend and the icons built again for devices, signed
    /// ad hoc. iOS installs it once a tool signs it again, with an Apple ID
    /// (AltStore, Sideloadly) or a developer certificate.
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
        let staged = payload.join(format!("{product}.app"));
        let executable = staged.join(&product);
        let frontends = FrontEnds::of(&context.target);
        let minimum = frontends.minimum();
        let frameworks = staged.join("Frameworks");
        let dist = bern_target_api::dist_dir(context);
        let ipa = dist.join(format!("{product}-{}-ios.ipa", app.app.version));
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
                description: "Build the Rust engine and bridge for iOS devices".into(),
                program: "cargo".into(),
                args: crate::listing_native_libs(&cargo, &crate::native_libs_file(&rust_output)),
                cwd: output.join("glue"),
                env: vec![(
                    "IPHONEOS_DEPLOYMENT_TARGET".to_string(),
                    frontends.engine_minimum().to_string(),
                )],
            },
            BuildStep::RemoveIfExists {
                path: stage.clone(),
            },
            BuildStep::CreateDir {
                path: payload.clone(),
            },
            BuildStep::CreateDir { path: dist.clone() },
            BuildStep::RemoveIfExists { path: ipa.clone() },
            // The simulator's bundle brings the resources, localizations
            // and Info.plist; what was built for the simulator is replaced.
            run(
                "Copy the app",
                "ditto",
                vec![path(&output.join(format!("{product}.app"))), path(&staged)],
            ),
            BuildStep::RemoveIfExists {
                path: staged.join("_CodeSignature"),
            },
            run(
                "Compile the icon for devices",
                "xcrun",
                actool(app, context, &staged, "iphoneos")?,
            ),
        ];
        let ui = framework(&frameworks, UI_FRAMEWORK);
        if frontends == FrontEnds::Both {
            steps.extend([
                run(
                    "Link the engine into its framework for devices",
                    "xcrun",
                    engine_link(
                        "iphoneos",
                        &format!("arm64-apple-ios{minimum}"),
                        &rust_output,
                        &frameworks,
                    ),
                ),
                run(
                    "Compile the front end of iOS 17 and later for devices",
                    "xcrun",
                    swiftc(
                        &product,
                        context,
                        "iphoneos",
                        &format!("arm64-apple-ios{MINIMUM_IOS}"),
                        &rust_output,
                        &ui,
                        frontends,
                        Part::Ui17,
                    ),
                ),
                BuildStep::RemoveIfExists {
                    path: ui.with_extension("dSYM"),
                },
            ]);
        }
        steps.extend([
            run(
                "Compile the SwiftUI frontend for devices",
                "xcrun",
                swiftc(
                    &product,
                    context,
                    "iphoneos",
                    &format!("arm64-apple-ios{minimum}"),
                    &rust_output,
                    &executable,
                    frontends,
                    Part::App,
                ),
            ),
            BuildStep::RemoveIfExists {
                path: staged.join(format!("{product}.dSYM")),
            },
        ]);
        if context.release {
            steps.push(run(
                "Strip the symbols from the executable",
                "xcrun",
                vec!["strip".into(), "-S".into(), "-x".into(), path(&executable)],
            ));
            if frontends == FrontEnds::Both {
                for binary in [ui.clone(), framework(&frameworks, ENGINE_FRAMEWORK)] {
                    steps.push(run(
                        "Strip the symbols from a framework",
                        "xcrun",
                        vec!["strip".into(), "-S".into(), "-x".into(), path(&binary)],
                    ));
                }
            }
        }
        let mark = |plist: PathBuf| {
            run(
                "Mark the app for devices",
                "plutil",
                vec![
                    "-replace".into(),
                    "CFBundleSupportedPlatforms".into(),
                    "-json".into(),
                    r#"["iPhoneOS"]"#.into(),
                    path(&plist),
                ],
            )
        };
        steps.push(mark(staged.join("Info.plist")));
        if frontends == FrontEnds::Both {
            for name in [ENGINE_FRAMEWORK, UI_FRAMEWORK] {
                let dir = frameworks.join(format!("{name}.framework"));
                steps.extend([
                    mark(dir.join("Info.plist")),
                    run(
                        "Sign a framework ad hoc",
                        "codesign",
                        vec!["--force".into(), "--sign".into(), "-".into(), path(&dir)],
                    ),
                ]);
            }
        }
        steps.extend([
            run(
                "Sign the app ad hoc",
                "codesign",
                vec!["--force".into(), "--sign".into(), "-".into(), path(&staged)],
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
        let mut products = vec![ipa];
        if frontends == FrontEnds::Separate {
            // The same app, its executable the front end of iOS 16.
            let ipa = dist.join(format!("{product}-{}-ios16.ipa", app.app.version));
            let stage = output.join("package16");
            let payload = stage.join("Payload");
            let staged16 = payload.join(format!("{product}.app"));
            let executable = staged16.join(&product);
            steps.extend([
                BuildStep::RemoveIfExists {
                    path: stage.clone(),
                },
                BuildStep::CreateDir {
                    path: payload.clone(),
                },
                BuildStep::RemoveIfExists { path: ipa.clone() },
                run(
                    "Copy the app for iOS 16",
                    "ditto",
                    vec![path(&staged), path(&staged16)],
                ),
                BuildStep::RemoveIfExists {
                    path: staged16.join("_CodeSignature"),
                },
                run(
                    "Compile the front end of iOS 16 for devices",
                    "xcrun",
                    swiftc(
                        &product,
                        context,
                        "iphoneos",
                        &format!("arm64-apple-ios{}", ios16::MINIMUM),
                        &rust_output,
                        &executable,
                        frontends,
                        Part::App16,
                    ),
                ),
                BuildStep::RemoveIfExists {
                    path: staged16.join(format!("{product}.dSYM")),
                },
            ]);
            if context.release {
                steps.push(run(
                    "Strip the symbols from the executable",
                    "xcrun",
                    vec!["strip".into(), "-S".into(), "-x".into(), path(&executable)],
                ));
            }
            steps.extend([
                run(
                    "Set the app's earliest system",
                    "plutil",
                    vec![
                        "-replace".into(),
                        "MinimumOSVersion".into(),
                        "-string".into(),
                        ios16::MINIMUM.into(),
                        path(&staged16.join("Info.plist")),
                    ],
                ),
                run(
                    "Sign the app ad hoc",
                    "codesign",
                    vec![
                        "--force".into(),
                        "--sign".into(),
                        "-".into(),
                        path(&staged16),
                    ],
                ),
                run(
                    "Make the .ipa of iOS 16",
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
            products.push(ipa);
        }
        Ok(BuildPlan {
            steps,
            products,
            run: None,
            notes: vec![
                "the .ipa is signed ad hoc; iOS installs it once a tool signs it again with an Apple ID (AltStore, Sideloadly) or a developer certificate"
                    .into(),
            ],
        })
    }
}

/// What one run of `swiftc` builds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    /// The executable.
    App,
    /// The framework of the front end of iOS 17 and later.
    Ui17,
    /// The executable of the separate app of iOS 16.
    App16,
}

/// The Swift of the front ends a target builds: in `Sources` the one the
/// executable is, and in `Sources17` the framework's, in an app of both.
fn swift_sources(app: &Application, frontends: FrontEnds) -> Vec<GeneratedFile> {
    let sources = [
        ("BernRuntime.swift", crate::runtime_with_svg(app, RUNTIME)),
        ("BernMedia.swift", MEDIA.to_string()),
        ("BernStyle.swift", STYLE.to_string()),
        ("BernUIKit.swift", UIKIT.to_string()),
        (
            "BernGenerated.swift",
            Generator::new(app, Platform::Ios).generate(app.protocol.fingerprint()),
        ),
    ];
    let mut files = Vec::new();
    for (name, source) in sources {
        match frontends {
            FrontEnds::Current => {
                files.push(GeneratedFile::text(format!("Sources/{name}"), source));
            }
            FrontEnds::Separate => {
                files.push(GeneratedFile::text(
                    format!("Sources16/{name}"),
                    ios16::rewrite(&source),
                ));
                files.push(GeneratedFile::text(format!("Sources/{name}"), source));
            }
            FrontEnds::Ios16 => files.push(GeneratedFile::text(
                format!("Sources/{name}"),
                ios16::rewrite(&source),
            )),
            FrontEnds::Both => {
                let source = ios16::without_entry(&source);
                files.push(GeneratedFile::text(
                    format!("Sources/{name}"),
                    ios16::rewrite(&source),
                ));
                files.push(GeneratedFile::text(format!("Sources17/{name}"), source));
            }
        }
    }
    match frontends {
        FrontEnds::Current => {}
        FrontEnds::Separate => {
            files.push(GeneratedFile::text(
                "Sources16/BernCompat16.swift",
                ios16::COMPAT,
            ));
        }
        FrontEnds::Ios16 | FrontEnds::Both => {
            files.push(GeneratedFile::text(
                "Sources/BernCompat16.swift",
                ios16::COMPAT,
            ));
        }
    }
    if frontends == FrontEnds::Both {
        let id = &app.app.bundle_id;
        files.extend([
            GeneratedFile::text("Sources/BernLaunch16.swift", ios16::LAUNCH),
            GeneratedFile::text("Sources17/BernEntry17.swift", ios16::ENTRY),
            GeneratedFile::text(
                format!("Frameworks/{ENGINE_FRAMEWORK}.plist"),
                ios16::framework_plist(ENGINE_FRAMEWORK, &format!("{id}.engine"), ios16::MINIMUM),
            ),
            GeneratedFile::text(
                format!("Frameworks/{UI_FRAMEWORK}.plist"),
                ios16::framework_plist(UI_FRAMEWORK, &format!("{id}.ui17"), MINIMUM_IOS),
            ),
        ]);
    }
    files
}

/// The `xcrun` arguments that compile the Swift sources with the `sdk`
/// for `triple`, linking the engine built in `rust_output`, into
/// `executable`.
#[allow(clippy::too_many_arguments)]
fn swiftc(
    product: &str,
    context: &BuildContext,
    sdk: &str,
    triple: &str,
    rust_output: &Path,
    executable: &Path,
    frontends: FrontEnds,
    part: Part,
) -> Vec<String> {
    let path = |path: &Path| path.display().to_string();
    let header = context.output_dir.join("Sources/bern.h");
    let (sources, module) = match part {
        Part::App => (context.output_dir.join("Sources"), product.to_string()),
        Part::App16 => (context.output_dir.join("Sources16"), product.to_string()),
        Part::Ui17 => (
            context.output_dir.join("Sources17"),
            format!("{product}{UI_FRAMEWORK}"),
        ),
    };
    let engine = if frontends == FrontEnds::Both {
        let frameworks = match part {
            Part::App | Part::App16 => executable.parent().map(|bundle| bundle.join("Frameworks")),
            Part::Ui17 => executable
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
        }
        .unwrap_or_default();
        vec![
            "-F".to_string(),
            path(&frameworks),
            "-framework".into(),
            ENGINE_FRAMEWORK.into(),
            "-Xlinker".into(),
            "-rpath".into(),
            "-Xlinker".into(),
            "@executable_path/Frameworks".into(),
        ]
    } else {
        vec![
            "-L".to_string(),
            path(rust_output),
            "-lbern_app".into(),
            format!("@{}", path(&crate::native_libs_file(rust_output))),
            // rustc's list repeats libraries swiftc links anyway.
            "-Xlinker".into(),
            "-no_warn_duplicate_libraries".into(),
        ]
    };
    let mut args = vec![
        "--sdk".to_string(),
        sdk.into(),
        "swiftc".into(),
        "-swift-version".into(),
        "6".into(),
        "-parse-as-library".into(),
        "-module-name".into(),
        module,
        "-target".into(),
        triple.into(),
        "-import-objc-header".into(),
        path(&header),
    ];
    args.extend(engine);
    args.extend([
        "-framework".to_string(),
        "SwiftUI".into(),
        "-framework".into(),
        "UIKit".into(),
        "-framework".into(),
        "AVFoundation".into(),
        "-framework".into(),
        "AVKit".into(),
        "-framework".into(),
        "Photos".into(),
        "-module-cache-path".into(),
        path(&context.workspace_dir.join("target/bern-swift-module-cache")),
    ]);
    if part == Part::Ui17 {
        args.extend([
            "-emit-library".to_string(),
            "-Xlinker".into(),
            "-install_name".into(),
            "-Xlinker".into(),
            format!("@rpath/{UI_FRAMEWORK}.framework/{UI_FRAMEWORK}"),
        ]);
    }
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
    match (part, frontends) {
        (Part::Ui17, _) => args.push(path(&sources.join("BernEntry17.swift"))),
        (Part::App, FrontEnds::Current | FrontEnds::Separate) => {}
        (Part::App16, _) | (Part::App, FrontEnds::Ios16) => {
            args.push(path(&sources.join("BernCompat16.swift")));
        }
        (Part::App, FrontEnds::Both) => {
            args.push(path(&sources.join("BernCompat16.swift")));
            args.push(path(&sources.join("BernLaunch16.swift")));
        }
    }
    args.push("-o".into());
    args.push(path(executable));
    args
}

/// The `xcrun` arguments that compile the asset catalog into `bundle` for
/// `platform` (`iphonesimulator` or `iphoneos`). A declared Icon Composer
/// document is compiled with the catalog: Liquid Glass layers for iOS 26,
/// and flat images drawn from them for earlier systems.
fn actool(
    app: &Application,
    context: &BuildContext,
    bundle: &Path,
    platform: &str,
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let output = &context.output_dir;
    let path = |path: &Path| path.display().to_string();
    let mut catalogs = vec![path(&output.join(ASSETS))];
    if crate::icon_document(app, "ios", &context.project_dir).is_some() {
        catalogs.push(path(&output.join(format!("{ICON_NAME}.icon"))));
    }
    // The alternate icons, from the catalog or their own documents.
    let mut alternates = Vec::new();
    for (name, source) in crate::alternate_icons(app, "ios", &context.project_dir)? {
        let asset = crate::alternate_icon(&name);
        if matches!(&source, icon::Source::Native(path) if icon::is_icon_document(path)) {
            catalogs.push(path(&output.join(format!("{asset}.icon"))));
        }
        alternates.extend(["--alternate-app-icon".to_string(), asset]);
    }
    Ok([
        vec!["actool".into()],
        catalogs,
        vec![
            "--compile".into(),
            path(bundle),
            "--platform".into(),
            platform.into(),
            "--minimum-deployment-target".into(),
            FrontEnds::of(&context.target).minimum().into(),
            "--app-icon".into(),
            ICON_NAME.into(),
        ],
        alternates,
        vec![
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
    ]
    .concat())
}
