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
mod installer;
mod strings;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, Target, TargetConfig, TargetDescriptor, WindowCapabilities, archive, check_common,
    dist_dir, files_below, icon,
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
        "bern_ui_widgets.inl",
        include_str!("../templates/cpp/bern_ui_widgets.inl"),
    ),
    (
        "bern_ui_structure.inl",
        include_str!("../templates/cpp/bern_ui_structure.inl"),
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
    fn build_hosts(&self) -> &'static [&'static str] {
        &["windows"]
    }

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
        let target_dir = context.workspace_dir.join("target/bern-glue-windows");
        let engine = target_dir.join(profile).join("bern_app.lib");
        let cache = context.workspace_dir.join("target/bern-winrt");
        let bundle = context.output_dir.join(&product);
        let frontends = glue::Frontends::of(&context.target.frontends);
        let generator = Generator::new(app);
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
                    precompiled: &precompiled_identity(context.release, frontends),
                    frontends,
                }),
            ),
            GeneratedFile::text("app/app.manifest", glue::manifest(app)),
            GeneratedFile::text("app/app.rc", glue::resource_script(app, &executable)),
            GeneratedFile {
                path: "app/app.ico".into(),
                contents: icon::ico(
                    &icon::images(app, &context.project_dir, icon::Shape::AsDrawn)
                        .map_err(|message| vec![Diagnostic::error(message)])?,
                ),
            },
            GeneratedFile::text("app/app_id.h", generator.identity()),
            GeneratedFile::text(
                "app/app_catalog.cpp",
                generator.catalog(app.protocol.fingerprint()),
            ),
            GeneratedFile::text("app/app_ui.inl", generator.interface()),
        ];
        for (name, contents) in RUNTIME {
            files.push(GeneratedFile::text(
                format!("app/runtime/{name}"),
                *contents,
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
        let steps = vec![
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
            notes: Vec::new(),
            run: Some(Run::Program(bundle.join(exe(&executable)))),
            products: vec![
                bundle.join(exe(&executable)),
                output.join("bin").join(exe(&executable)),
                output.join("bin").join(exe(&format!("{executable}-mcp"))),
            ],
        })
    }

    /// An installer (`setup.exe`) that installs for the current user
    /// without administrator rights, and a `.zip` of the same folder for
    /// people who do not install. Both carry the command line and the MCP
    /// server in `bin\`. Signed with the certificate from [`Signing`].
    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
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
}
