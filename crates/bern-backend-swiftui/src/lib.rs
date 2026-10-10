//! The `swiftui + macos` target.
//!
//! Generates idiomatic SwiftUI from the shared IR: standard scenes,
//! containers and controls with system styles, no custom colors and no
//! version-gated calls to newer visual APIs. The engine runs in the app
//! process; the build links it as a static library.

mod glue;
mod names;
mod resources;
mod swift;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node, WindowInitial, WindowRole};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport,
    Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common,
};
use std::path::PathBuf;

pub use swift::Generator;

/// Minimum deployment target, chosen in the design document.
pub const MINIMUM_MACOS: &str = "14.0";

const RUNTIME: &str = include_str!("../templates/BernRuntime.swift");
const MEDIA: &str = include_str!("../templates/BernMedia.swift");

/// Swift sources compiled into the app, in order.
const SOURCES: &[&str] = &[
    "BernRuntime.swift",
    "BernMedia.swift",
    "BernGenerated.swift",
];
const HEADER: &str = include_str!("../templates/bern.h");

pub struct SwiftUiMacos;

fn component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| COMPONENTS.iter().map(|spec| spec.name).collect())
}

/// Bundle and executable name, e.g. `Showcase` for app id `showcase`.
pub fn product_name(app: &Application) -> String {
    names::pascal(&app.app.id)
}

fn architecture() -> (&'static str, &'static str) {
    match std::env::consts::ARCH {
        "x86_64" => ("x86_64", "x86_64-apple-darwin"),
        _ => ("arm64", "aarch64-apple-darwin"),
    }
}

impl Target for SwiftUiMacos {
    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
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
        let mut diagnostics = check_common(self, app);
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
            if !self.descriptor().channels.contains(&channel.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: channel `{channel}` is not available on macOS",
                    config.name
                )));
            }
        }
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
        let mut visit = |node: &Node, parent: Option<&str>| {
            if let Node::Element(element) = node {
                if element.kind == "Search" && parent != Some("Sidebar") {
                    diagnostics.push(
                        Diagnostic::error("the SwiftUI target places `Search` in a `Sidebar` only")
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
        fn walk(node: &Node, parent: Option<&str>, visit: &mut dyn FnMut(&Node, Option<&str>)) {
            visit(node, parent);
            let name = match node {
                Node::Element(element) => Some(element.kind.as_str()),
                _ => parent,
            };
            match node {
                Node::Element(element) => {
                    for child in &element.children {
                        walk(child, name, visit);
                    }
                }
                Node::If(node) => {
                    for child in node
                        .branches
                        .iter()
                        .flat_map(|(_, body)| body)
                        .chain(&node.otherwise)
                    {
                        walk(child, name, visit);
                    }
                }
                Node::IfLet(node) => {
                    for child in node.then.iter().chain(&node.otherwise) {
                        walk(child, name, visit);
                    }
                }
                Node::Match(node) => {
                    for child in node.arms.iter().flat_map(|arm| &arm.body) {
                        walk(child, name, visit);
                    }
                }
                Node::Component(_) => {}
            }
        }
        for component in &app.components {
            walk(&component.body, None, &mut visit);
        }
        for window in &app.windows.windows {
            walk(&window.content, None, &mut visit);
        }
        diagnostics
    }

    fn generate(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
        let product = product_name(app);
        let bundle = PathBuf::from(format!("{product}.app"));
        let mut files = vec![
            GeneratedFile::text("Sources/BernRuntime.swift", RUNTIME),
            GeneratedFile::text("Sources/BernMedia.swift", MEDIA),
            GeneratedFile::text("Sources/bern.h", HEADER),
            GeneratedFile::text(
                "Sources/BernGenerated.swift",
                Generator::new(app).generate(app.protocol.fingerprint()),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app)),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                bundle.join("Contents/Info.plist"),
                resources::info_plist(app, &product, MINIMUM_MACOS),
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
        for (language, table) in resources::string_tables(app) {
            files.push(GeneratedFile::text(
                bundle.join(format!(
                    "Contents/Resources/{language}.lproj/Localizable.strings"
                )),
                table,
            ));
        }
        for (language, table) in resources::info_plist_tables(app) {
            files.push(GeneratedFile::text(
                bundle.join(format!(
                    "Contents/Resources/{language}.lproj/InfoPlist.strings"
                )),
                table,
            ));
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
        swiftc.extend(if context.release {
            vec!["-O".to_string()]
        } else {
            vec!["-Onone".to_string(), "-g".into()]
        });
        for file in SOURCES {
            swiftc.push(sources.join(file).display().to_string());
        }
        swiftc.push("-o".into());
        swiftc.push(
            bundle
                .join("Contents/MacOS")
                .join(&product)
                .display()
                .to_string(),
        );

        let steps = vec![
            BuildStep::Run {
                description: "Build the Rust engine and bridge".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("glue"),
                env: env.clone(),
            },
            BuildStep::CreateDir {
                path: bundle.join("Contents/MacOS"),
            },
            BuildStep::Run {
                description: "Compile the SwiftUI frontend".into(),
                program: "xcrun".into(),
                args: swiftc,
                cwd: output.clone(),
                env,
            },
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
        ];
        Ok(BuildPlan {
            steps,
            products: vec![
                bundle,
                output.join("bin").join(&cli),
                output.join("bin").join(format!("{cli}-mcp")),
            ],
        })
    }
}
