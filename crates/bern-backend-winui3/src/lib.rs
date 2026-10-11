//! The `winui3 + windows` target.
//!
//! Generates a C# application from the shared IR: WinUI 3 on the Windows
//! App SDK, following the Fluent controls, cards and navigation of Windows
//! 11, with colors from the system theme. The engine runs in the
//! application process: the runtime (C# templates in `templates/`) reaches
//! it through the same in-process C ABI the SwiftUI target uses, built here
//! as `bern_app.dll`. .NET and the Windows App SDK are requirements of this
//! target's build only.

mod cs;
mod glue;
mod strings;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport,
    Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common,
};
use std::path::PathBuf;

pub use cs::Generator;
pub use glue::{MINIMUM_WINDOWS, TARGET_FRAMEWORK, WINDOWS_APP_SDK};

/// The C# runtime, copied into every generated application.
const RUNTIME: &[(&str, &str)] = &[
    ("BernValue.cs", include_str!("../templates/BernValue.cs")),
    (
        "BernReactive.cs",
        include_str!("../templates/BernReactive.cs"),
    ),
    ("BernCore.cs", include_str!("../templates/BernCore.cs")),
    ("BernHost.cs", include_str!("../templates/BernHost.cs")),
    ("BernLayout.cs", include_str!("../templates/BernLayout.cs")),
    (
        "BernWindows.cs",
        include_str!("../templates/BernWindows.cs"),
    ),
    (
        "BernSession.cs",
        include_str!("../templates/BernSession.cs"),
    ),
    (
        "BernWidgets.cs",
        include_str!("../templates/BernWidgets.cs"),
    ),
    (
        "BernStructure.cs",
        include_str!("../templates/BernStructure.cs"),
    ),
    ("BernQuery.cs", include_str!("../templates/BernQuery.cs")),
    ("BernMedia.cs", include_str!("../templates/BernMedia.cs")),
];

pub struct WinUi3Windows;

fn component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| COMPONENTS.iter().map(|spec| spec.name).collect())
}

/// Directory name of the product, e.g. `Showcase` for app id `showcase`.
pub fn product_name(app: &Application) -> String {
    app.app
        .id
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

fn executable(app: &Application) -> String {
    app.app.id.replace('_', "-")
}

fn exe(name: &str) -> String {
    format!("{name}.exe")
}

impl Target for WinUi3Windows {
    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
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
                backend: "windows-mediaplayer",
                backend_version: "1",
                presenter: "mediaplayerelement",
                presenter_version: "1",
                background_playback: true,
                multi_view: true,
            }),
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let mut diagnostics = check_common(self, app);
        let framework = strings::framework_languages();
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
        let product = product_name(app);
        let executable = executable(app);
        let profile = if context.release { "release" } else { "debug" };
        let library = context
            .workspace_dir
            .join("target/bern-glue-windows")
            .join(profile)
            .join("bern_app.dll");
        let mut files = vec![
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app)),
            GeneratedFile::text("glue/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("glue/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                format!("app/{product}.csproj"),
                glue::csproj(app, &executable, &library),
            ),
            GeneratedFile::text("app/app.manifest", glue::manifest(app)),
            GeneratedFile::text(
                "app/App.cs",
                Generator::new(app).generate(app.protocol.fingerprint()),
            ),
        ];
        for (name, contents) in RUNTIME {
            files.push(GeneratedFile::text(format!("app/Bern/{name}"), *contents));
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
                "dotnet": TARGET_FRAMEWORK,
                "minimum_windows": MINIMUM_WINDOWS,
            },
            "architecture": std::env::consts::ARCH,
            "runtime_identifier": glue::runtime_identifier(),
            "profile": profile,
            "engine_deployment": "in-process",
            "ui_bridge": "csharp",
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
        Ok(files)
    }

    fn plan(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
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
        let dotnet = vec![
            "build".to_string(),
            output
                .join("app")
                .join(format!("{product}.csproj"))
                .display()
                .to_string(),
            "--configuration".into(),
            if context.release { "Release" } else { "Debug" }.into(),
            "--output".into(),
            bundle.display().to_string(),
            "--nologo".into(),
        ];
        let steps = vec![
            BuildStep::Run {
                description: "Build the engine library and the entries".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("glue"),
                env: vec![],
            },
            BuildStep::Run {
                description: "Build the WinUI 3 application".into(),
                program: "dotnet".into(),
                args: dotnet,
                cwd: output.join("app"),
                env: vec![
                    ("DOTNET_CLI_TELEMETRY_OPTOUT".into(), "1".into()),
                    ("DOTNET_NOLOGO".into(), "1".into()),
                ],
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
                            context.project_dir.join(&resource.name),
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
        Ok(BuildPlan {
            steps,
            products: vec![
                bundle.join(exe(&executable)),
                output.join("bin").join(exe(&executable)),
                output.join("bin").join(exe(&format!("{executable}-mcp"))),
            ],
        })
    }
}
