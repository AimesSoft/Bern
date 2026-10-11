//! The `gtk + linux` target.
//!
//! Generates a Rust application from the shared IR: GTK 4 with libadwaita,
//! following the GNOME platform's containers, rows and styles, with colors
//! from the system style manager. The engine runs in the application
//! process; the runtime (`bern-gtk`) reaches it through the same
//! in-process interface the other native targets use. GTK is a
//! requirement of this target's build only.

mod glue;
mod rust;
mod strings;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport,
    Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common, icon,
};
use std::path::PathBuf;

pub use rust::Generator;

/// GTK and libadwaita versions the runtime is built against; the
/// application needs at least these on the system.
pub const MINIMUM_GTK: &str = "4.14";
pub const MINIMUM_LIBADWAITA: &str = "1.5";

pub struct GtkLinux;

fn component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| COMPONENTS.iter().map(|spec| spec.name).collect())
}

/// Directory and executable name of the product, e.g. `Showcase` and
/// `showcase` for app id `showcase`.
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

/// The application icon beside the executable, named after the
/// application so the icon theme finds it: `icons/<bundle id>.png`.
pub fn icon_file(app: &Application) -> PathBuf {
    PathBuf::from("icons").join(format!("{}.png", app.app.bundle_id))
}

fn executable(app: &Application) -> String {
    app.app.id.replace('_', "-")
}

impl Target for GtkLinux {
    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
            id: "gtk-linux",
            ui: "gtk",
            platforms: &["linux"],
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
                backend: "gtk-mediafile",
                backend_version: "1",
                presenter: "gtkpicture",
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
                    "the GTK runtime has no `{language}` translation of its own texts; supported: {}",
                    framework.join(", ")
                )));
            }
        }
        for channel in &config.channels {
            if !self.descriptor().channels.contains(&channel.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: channel `{channel}` is not available on Linux",
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
        let mut files = vec![
            GeneratedFile::text("app/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("app/src/main.rs", glue::main_rs(context, app)),
            GeneratedFile::text(
                "app/src/ui.rs",
                Generator::new(app).generate(app.protocol.fingerprint(), &context.protocol_module),
            ),
            GeneratedFile::text("app/src/cli.rs", glue::cli_rs(context, app)),
            GeneratedFile::text("app/src/mcp.rs", glue::mcp_rs(context, app)),
            GeneratedFile::text(
                PathBuf::from(&product).join(format!("{}.desktop", app.app.bundle_id)),
                glue::desktop_entry(
                    app,
                    &context.output_dir.join(&product).join(&executable),
                    &context.output_dir.join(&product).join(icon_file(app)),
                ),
            ),
        ];
        // The application icon next to the executable, which adds the
        // directory to the icon theme and names it after the application.
        let images = icon::images(app, &context.project_dir, icon::Shape::AsDrawn)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        files.push(GeneratedFile {
            path: PathBuf::from(&product).join(icon_file(app)),
            contents: icon::largest(&images).png.to_vec(),
        });
        let lock = context.workspace_dir.join("Cargo.lock");
        if let Ok(contents) = std::fs::read(&lock)
            && !context.output_dir.join("app/Cargo.lock").exists()
        {
            files.push(GeneratedFile {
                path: "app/Cargo.lock".into(),
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
            "ui": "gtk",
            "platform": "linux",
            "ir_version": app.ir_version,
            "generator": "bern-backend-gtk",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "toolkit": { "gtk": MINIMUM_GTK, "libadwaita": MINIMUM_LIBADWAITA },
            "architecture": std::env::consts::ARCH,
            "profile": if context.release { "release" } else { "debug" },
            "engine_deployment": "in-process",
            "ui_bridge": "rust",
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
        let target_dir = context.workspace_dir.join("target/bern-glue-linux");
        let built = target_dir.join(profile);
        let mut cargo = vec![
            "build".to_string(),
            "--manifest-path".into(),
            output.join("app/Cargo.toml").display().to_string(),
            "--target-dir".into(),
            target_dir.display().to_string(),
        ];
        if context.release {
            cargo.push("--release".into());
        }
        let steps = vec![
            BuildStep::Run {
                description: "Build the GTK application, the engine and the entries".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("app"),
                env: vec![],
            },
            BuildStep::CreateDir {
                path: bundle.clone(),
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
            BuildStep::Copy {
                from: built.join(format!("{executable}-ui")),
                to: bundle.join(&executable),
            },
            BuildStep::CreateDir {
                path: output.join("bin"),
            },
            BuildStep::Copy {
                from: built.join(&executable),
                to: output.join("bin").join(&executable),
            },
            BuildStep::Copy {
                from: built.join(format!("{executable}-mcp")),
                to: output.join("bin").join(format!("{executable}-mcp")),
            },
        ];
        Ok(BuildPlan {
            steps,
            products: vec![
                bundle.join(&executable),
                output.join("bin").join(&executable),
                output.join("bin").join(format!("{executable}-mcp")),
            ],
        })
    }
}
