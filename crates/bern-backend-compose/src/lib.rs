//! The `compose + android` target.
//!
//! Generates Jetpack Compose from the shared IR: Material 3 containers and
//! controls in the system's colors, inside a Gradle project. The engine
//! runs in the app process; the build compiles it with the NDK into
//! `libbern_app.so`, which the Kotlin runtime loads through JNI.

mod glue;
mod kotlin;
mod names;
mod project;
mod style;
pub mod toolchain;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common, icon,
};
use std::path::PathBuf;

pub use kotlin::Generator;
pub use project::{MINIMUM_SDK, TARGET_SDK};

const RUNTIME: &str = include_str!("../templates/BernRuntime.kt");
const VIEWS: &str = include_str!("../templates/BernViews.kt");
const STYLE: &str = include_str!("../templates/BernStyle.kt");
const MEDIA: &str = include_str!("../templates/BernMedia.kt");
const FEATURES: &str = include_str!("../templates/BernFeatures.kt");

/// The Rust target of the engine library: 64-bit ARM, which every phone
/// of the supported Android versions runs.
pub const RUST_TARGET: &str = "aarch64-linux-android";

/// Components not realized yet.
const UNSUPPORTED: &[&str] = &[];

/// IR features Compose builds.
const FEATURES_BUILT: &[&str] = &[
    bern_target_api::feature::SESSION_ACTIONS,
    // Without a pointer, revealed content always shows.
    bern_target_api::feature::REVEAL,
    bern_target_api::feature::BOUND_LENGTHS,
    bern_target_api::feature::APPEARANCE,
    bern_target_api::feature::SHORTCUTS,
    bern_target_api::feature::RUNTIME_THEME,
    bern_target_api::feature::BUTTON_CONTENT,
    bern_target_api::feature::TOOLBAR_SEARCH,
    bern_target_api::feature::SEARCH_RESULTS,
    bern_target_api::feature::TOGGLE_BUTTON,
    bern_target_api::feature::DIALOG_WINDOWS,
    bern_target_api::feature::RUNTIME_IMAGES,
    bern_target_api::feature::PICTURE_FILLS,
    bern_target_api::feature::FILE_PICK,
    bern_target_api::feature::SCROLL_SNAP,
    bern_target_api::feature::RAILS,
    bern_target_api::feature::POPOVER_BUTTON,
    bern_target_api::feature::SIZE_CLASSES,
];

/// The features template with the extensions of each file kind from the
/// shared table, so pickers, drops and the manifest agree.
fn features_source() -> String {
    let table: Vec<String> = bern_ir::spec::FILE_KIND_EXTENSIONS
        .iter()
        .map(|(kind, extensions)| {
            let list: Vec<String> = extensions.iter().map(|ext| format!("\"{ext}\"")).collect();
            format!("\"{kind}\" to listOf({})", list.join(", "))
        })
        .collect();
    FEATURES.replace("/*bern:file-extensions*/", &table.join(", "))
}

pub struct ComposeAndroid;

fn component_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        COMPONENTS
            .iter()
            .map(|spec| spec.name)
            .filter(|name| !UNSUPPORTED.contains(name))
            .collect()
    })
}

/// Gradle project and APK name, e.g. `Showcase` for app id `showcase`.
pub fn product_name(app: &Application) -> String {
    names::pascal(&app.app.id)
}

impl Target for ComposeAndroid {
    fn build_hosts(&self) -> &'static [&'static str] {
        &["macos", "linux"]
    }

    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
            styles: style::SUPPORT,
            motion: &[
                "appear",
                "disappear",
                "reflow",
                "hero",
                "animate",
                "scroll_scale",
                "scroll_opacity",
                "nudge",
                "stagger",
            ],
            window_chrome: true,
            text_marks: true,
            dynamic_items: true,
            features: FEATURES_BUILT,
            id: "compose-android",
            ui: "compose",
            platforms: &["android"],
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
                backend: "media3",
                backend_version: "1",
                presenter: "playerview",
                presenter_version: "1",
                // Playing on without a view needs a media service, which
                // the app does not declare.
                background_playback: false,
                multi_view: false,
            }),
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let mut diagnostics = check_common(self, app);
        for channel in &config.channels {
            if !self.descriptor().channels.contains(&channel.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: channel `{channel}` is not available on Android",
                    config.name
                )));
            }
        }
        let framework = bern_target_api::texts::framework_languages();
        for language in &app.languages.supported {
            if !framework.contains(&language.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "the Compose runtime has no `{language}` translation of its own texts; supported: {}",
                    framework.join(", ")
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
        let package = names::package(&app.app.bundle_id);
        let main = PathBuf::from("project/app/src/main");
        let sources = main.join("kotlin");
        let mut files = vec![
            GeneratedFile::text("project/settings.gradle.kts", project::settings(&product)),
            GeneratedFile::text("project/build.gradle.kts", project::root_build()),
            GeneratedFile::text("project/gradle.properties", project::GRADLE_PROPERTIES),
            GeneratedFile::text("project/app/build.gradle.kts", project::app_build(app)),
            GeneratedFile::text(main.join("AndroidManifest.xml"), project::manifest(app)),
            GeneratedFile::text(main.join("res/values/themes.xml"), project::theme(false)),
            GeneratedFile::text(
                main.join("res/values-night/themes.xml"),
                project::theme(true),
            ),
            // A Media3 view that draws into a TextureView, which a rounded
            // frame can clip; the default SurfaceView cannot be clipped.
            GeneratedFile::text(
                main.join("res/layout/bern_player_texture.xml"),
                r#"<?xml version="1.0" encoding="utf-8"?>
<androidx.media3.ui.PlayerView xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:app="http://schemas.android.com/apk/res-auto"
    android:layout_width="match_parent"
    android:layout_height="match_parent"
    app:surface_type="texture_view"
    app:use_controller="false" />
"#,
            ),
            GeneratedFile::text(sources.join("dev/bern/runtime/BernRuntime.kt"), RUNTIME),
            GeneratedFile::text(sources.join("dev/bern/runtime/BernViews.kt"), VIEWS),
            GeneratedFile::text(sources.join("dev/bern/runtime/BernStyle.kt"), STYLE),
            GeneratedFile::text(
                sources.join("dev/bern/runtime/BernFeatures.kt"),
                features_source(),
            ),
            GeneratedFile::text(sources.join("dev/bern/runtime/BernMedia.kt"), MEDIA),
            GeneratedFile::text(
                sources
                    .join(package.replace('.', "/"))
                    .join("BernGenerated.kt"),
                Generator::new(app).generate(&package, app.protocol.fingerprint()),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app)),
        ];
        if glue::ipc(context) {
            files.extend([
                GeneratedFile::text("tools/Cargo.toml", glue::tools_cargo_toml(context, app)),
                GeneratedFile::text("tools/src/cli.rs", glue::cli_rs(context, app)),
                GeneratedFile::text("tools/src/mcp.rs", glue::mcp_rs(context, app)),
            ]);
        }
        if bern_target_api::svg::used(app) {
            files.push(GeneratedFile::text(
                sources.join("dev/bern/runtime/BernSvg.kt"),
                bern_target_api::svg::template(include_str!("../templates/BernSvg.kt"), true),
            ));
        }
        let lock = context.workspace_dir.join("Cargo.lock");
        if let Ok(contents) = std::fs::read(&lock) {
            for crate_dir in ["glue", "tools"] {
                if !context
                    .output_dir
                    .join(crate_dir)
                    .join("Cargo.lock")
                    .exists()
                {
                    files.push(GeneratedFile {
                        path: PathBuf::from(crate_dir).join("Cargo.lock"),
                        contents: contents.clone(),
                    });
                }
            }
        }
        for (dir, contents) in project::app_names(app) {
            files.push(GeneratedFile::text(
                main.join("res").join(dir).join("strings.xml"),
                contents,
            ));
        }
        for (language, table) in project::text_tables(app) {
            files.push(GeneratedFile::text(
                main.join(format!("assets/bern/texts/{language}.json")),
                table,
            ));
        }
        // The launcher icon: the declared one, or the Bern logo, as drawn.
        let images = icon::images(app, &context.project_dir, icon::Shape::AsDrawn)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        files.push(GeneratedFile {
            path: main.join("res/mipmap-xxxhdpi/ic_launcher.png"),
            contents: icon::largest(&images).png.to_vec(),
        });
        let manifest = serde_json::json!({
            "target": self.descriptor().id,
            "ui": "compose",
            "platform": "android",
            "ir_version": app.ir_version,
            "generator": "bern-backend-compose",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "minimum_sdk": MINIMUM_SDK,
            "target_sdk": TARGET_SDK,
            "abi": "arm64-v8a",
            "profile": if context.release { "release" } else { "debug" },
            "engine_deployment": "in-process",
            "ui_bridge": "jni",
            "color_source": "native",
            "protocol": app.protocol.namespace,
            "protocol_fingerprint": format!("{:016x}", app.protocol.fingerprint()),
            "languages": app.languages.supported,
            "resources": app.resources.iter().map(|resource| &resource.name).collect::<Vec<_>>(),
            "package": app.app.bundle_id,
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
        let tools = toolchain::find().map_err(|message| vec![Diagnostic::error(message)])?;
        let product = product_name(app);
        let output = &context.output_dir;
        let gradle_project = output.join("project");
        let profile = if context.release { "release" } else { "debug" };
        let glue_target = context.workspace_dir.join("target/bern-glue");
        let library = glue_target
            .join(RUST_TARGET)
            .join(profile)
            .join("libbern_app.so");
        let bin = toolchain::ndk_bin(&tools.ndk);
        let linker = bin.join(format!("{RUST_TARGET}{MINIMUM_SDK}-clang"));
        let path = |path: &PathBuf| path.display().to_string();
        let env = vec![
            (
                "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER".to_string(),
                path(&linker),
            ),
            ("CC_aarch64_linux_android".to_string(), path(&linker)),
            (
                "AR_aarch64_linux_android".to_string(),
                path(&bin.join("llvm-ar")),
            ),
        ];
        let mut cargo = vec![
            "build".to_string(),
            "--manifest-path".into(),
            path(&output.join("glue/Cargo.toml")),
            "--target".into(),
            RUST_TARGET.into(),
            "--target-dir".into(),
            path(&glue_target),
        ];
        if context.release {
            cargo.push("--release".into());
        }
        let mut gradle_env = vec![("ANDROID_HOME".to_string(), path(&tools.sdk))];
        if let Some(java_home) = &tools.java_home {
            gradle_env.push(("JAVA_HOME".to_string(), path(java_home)));
        }
        let task = if context.release {
            "assembleRelease"
        } else {
            "assembleDebug"
        };
        let apk = gradle_project
            .join("app/build/outputs/apk")
            .join(profile)
            .join(format!("app-{profile}.apk"));
        let product_apk = output.join(format!("{product}.apk"));
        let jni = gradle_project.join("app/src/main/jniLibs/arm64-v8a");
        // Only this application's Kotlin: every application built here
        // shares the output directory, and sources another one left behind
        // would be compiled into this one.
        let package = names::package(&app.app.bundle_id);
        let kotlin = BuildStep::Mirror {
            dir: gradle_project.join("app/src/main/kotlin"),
            files: Vec::new(),
            keep: [
                "BernRuntime",
                "BernViews",
                "BernStyle",
                "BernFeatures",
                "BernMedia",
            ]
            .iter()
            .map(|name| PathBuf::from(format!("dev/bern/runtime/{name}.kt")))
            .chain(
                bern_target_api::svg::used(app)
                    .then(|| PathBuf::from("dev/bern/runtime/BernSvg.kt")),
            )
            .chain([PathBuf::from(package.replace('.', "/")).join("BernGenerated.kt")])
            .collect(),
        };
        let assets = gradle_project.join("app/src/main/assets/bern/resources");
        let mut steps = vec![
            BuildStep::Run {
                description: "Build the Rust engine and bridge".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("glue"),
                env,
            },
            BuildStep::CreateDir { path: jni.clone() },
            BuildStep::Copy {
                from: library,
                to: jni.join("libbern_app.so"),
            },
            // Declared resources ship as assets, which the runtime unpacks.
            BuildStep::Mirror {
                dir: assets,
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
                keep: Vec::new(),
            },
            kotlin,
            BuildStep::Run {
                description: "Build the Compose frontend".into(),
                program: path(&tools.gradle),
                args: vec![
                    "--project-dir".into(),
                    path(&gradle_project),
                    "--console=plain".into(),
                    "--quiet".into(),
                    task.into(),
                ],
                cwd: gradle_project.clone(),
                env: gradle_env,
            },
            BuildStep::Copy {
                from: apk,
                to: product_apk.clone(),
            },
        ];
        let mut products = vec![product_apk.clone()];
        let mut forward = None;
        if glue::ipc(context) {
            // The command line and MCP server run on this computer.
            let tools_target = context.workspace_dir.join("target/bern-tools");
            let mut cargo = vec![
                "build".to_string(),
                "--manifest-path".into(),
                path(&output.join("tools/Cargo.toml")),
                "--target-dir".into(),
                path(&tools_target),
            ];
            if context.release {
                cargo.push("--release".into());
            }
            steps.push(BuildStep::Run {
                description: "Build the command line and MCP server".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("tools"),
                env: Vec::new(),
            });
            steps.push(BuildStep::CreateDir {
                path: output.join("bin"),
            });
            let cli = app.app.id.replace('_', "-");
            for program in [cli.clone(), format!("{cli}-mcp")] {
                let program = format!("{program}{}", std::env::consts::EXE_SUFFIX);
                steps.push(BuildStep::Copy {
                    from: tools_target.join(profile).join(&program),
                    to: output.join("bin").join(&program),
                });
                products.push(output.join("bin").join(&program));
            }
            forward = Some((
                bern_transport_ipc::socket_dir(&app.app.bundle_id)
                    .join(bern_transport_ipc::ANDROID_FORWARD),
                bern_transport_ipc::android_socket(&app.app.bundle_id),
            ));
        }
        Ok(BuildPlan {
            steps,
            notes: Vec::new(),
            run: Some(Run::Android {
                apk: product_apk,
                package: app.app.bundle_id.clone(),
                adb: toolchain::adb(),
                forward,
            }),
            products,
        })
    }

    /// The release APK, for installing directly, and the Android App
    /// Bundle, for stores, in the distribution directory. Gradle signs both
    /// with the key the `BERN_ANDROID_*` variables name, or with the debug
    /// key.
    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let tools = toolchain::find().map_err(|message| vec![Diagnostic::error(message)])?;
        if signing.android_keystore.is_some()
            && (signing.android_keystore_password.is_none() || signing.android_key_alias.is_none())
        {
            return Err(vec![Diagnostic::error(
                "BERN_ANDROID_KEYSTORE needs BERN_ANDROID_KEYSTORE_PASSWORD and BERN_ANDROID_KEY_ALIAS",
            )]);
        }
        let product = product_name(app);
        let gradle_project = context.output_dir.join("project");
        let dist = bern_target_api::dist_dir(context);
        let name = |extension: &str| {
            dist.join(format!("{product}-{}-android.{extension}", app.app.version))
        };
        let path = |path: &PathBuf| path.display().to_string();
        let mut env = vec![("ANDROID_HOME".to_string(), path(&tools.sdk))];
        if let Some(java_home) = &tools.java_home {
            env.push(("JAVA_HOME".to_string(), path(java_home)));
        }
        let steps = vec![
            BuildStep::Run {
                description: "Bundle the app for stores".into(),
                program: path(&tools.gradle),
                args: vec![
                    "--project-dir".into(),
                    path(&gradle_project),
                    "--console=plain".into(),
                    "--quiet".into(),
                    "bundleRelease".into(),
                ],
                cwd: gradle_project.clone(),
                env,
            },
            BuildStep::CreateDir { path: dist.clone() },
            BuildStep::Copy {
                from: gradle_project.join("app/build/outputs/apk/release/app-release.apk"),
                to: name("apk"),
            },
            BuildStep::Copy {
                from: gradle_project.join("app/build/outputs/bundle/release/app-release.aab"),
                to: name("aab"),
            },
        ];
        let mut notes = Vec::new();
        if signing.android_keystore.is_none() {
            notes.push(
                "signed with the debug key: installable for testing, not accepted by stores (set BERN_ANDROID_KEYSTORE, BERN_ANDROID_KEYSTORE_PASSWORD and BERN_ANDROID_KEY_ALIAS)"
                    .into(),
            );
        }
        Ok(BuildPlan {
            steps,
            products: vec![name("apk"), name("aab")],
            run: None,
            notes,
        })
    }
}
