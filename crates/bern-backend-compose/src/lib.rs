//! The `compose + android` target.
//!
//! Generates Jetpack Compose from the shared IR: Material 3 containers and
//! controls in the system's colors, inside a Gradle project. The engine
//! runs in the app process; the build compiles it with the NDK into
//! `libbern_app.so`, which the Kotlin runtime loads through JNI.

mod glue;
pub mod icons;
mod kotlin;
mod names;
mod project;
mod style;
pub mod toolchain;
mod wear_runtime;
mod wearos;
pub use wearos::ComposeWearos;

pub use project::KOTLIN;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common, icon, skill,
};
use std::path::PathBuf;

pub use kotlin::Generator;
pub use project::{MINIMUM_SDK, TARGET_SDK, WEAR_MINIMUM_SDK};

const RUNTIME: &str = include_str!("../templates/BernRuntime.kt");
const VIEWS: &str = include_str!("../templates/BernViews.kt");
const STYLE: &str = include_str!("../templates/BernStyle.kt");
const MEDIA: &str = include_str!("../templates/BernMedia.kt");
const FEATURES: &str = include_str!("../templates/BernFeatures.kt");
const PICTURE_TRIES: &str = include_str!("../templates/BernPictureTries.kt");
const SCENE: &str = include_str!("../templates/BernScene.kt");
const APP_ICON: &str = include_str!("../templates/BernAppIcon.kt");

/// The Rust target of the engine library: 64-bit ARM, which every phone
/// of the supported Android versions runs.
pub const RUST_TARGET: &str = "aarch64-linux-android";
/// The 32-bit ARM Rust target included by default in Wear OS releases so older
/// Android 8+ watches are covered alongside 64-bit watches.
pub const ARMV7_RUST_TARGET: &str = "armv7-linux-androideabi";

/// An Android ABI: its name, its Rust target, and what the NDK's compilers
/// start with, which for 32-bit ARM is not the Rust target.
struct Abi {
    name: &'static str,
    rust: &'static str,
    clang: &'static str,
}

const ABIS: &[Abi] = &[
    Abi {
        name: "arm64-v8a",
        rust: RUST_TARGET,
        clang: RUST_TARGET,
    },
    Abi {
        name: "armeabi-v7a",
        rust: ARMV7_RUST_TARGET,
        clang: "armv7a-linux-androideabi",
    },
    Abi {
        name: "x86_64",
        rust: "x86_64-linux-android",
        clang: "x86_64-linux-android",
    },
];

/// A release for more than one ABI, which makes an APK for each.
fn split_abis(context: &BuildContext) -> bool {
    build_abis(context).len() > 1
}

/// The ABIs of this build: those of `abis` in a release, the first alone
/// otherwise, for a build to try on one device. When `abis` is omitted,
/// Android defaults to `arm64-v8a`, while Wear OS releases default to both
/// `arm64-v8a` and `armeabi-v7a` so 32-bit watches are supported out of the
/// box.
fn build_abis(context: &BuildContext) -> Vec<&'static Abi> {
    let named = |name: &str| ABIS.iter().find(|abi| abi.name == name);
    let mut abis: Vec<&Abi> = context
        .target
        .abis
        .iter()
        .filter_map(|name| named(name))
        .collect();
    if abis.is_empty() {
        abis.push(&ABIS[0]);
        if context.target.platform == "wearos" {
            abis.push(&ABIS[1]);
        }
    }
    if !context.release {
        abis.truncate(1);
    }
    abis
}

/// Effective `minSdk` for this Android or Wear OS build.
fn minimum_sdk(context: &BuildContext) -> u32 {
    let default = if context.target.platform == "wearos" {
        WEAR_MINIMUM_SDK
    } else {
        MINIMUM_SDK
    };
    context
        .target
        .android
        .as_ref()
        .and_then(|options| options.min_sdk)
        .unwrap_or(default)
}

/// Components not realized yet.
const UNSUPPORTED: &[&str] = &[];

/// IR features Compose builds.
const FEATURES_BUILT: &[&str] = &[
    bern_target_api::feature::SESSION_ACTIONS,
    bern_target_api::feature::TAB_ACCESSORY,
    // Without a pointer, revealed content always shows.
    bern_target_api::feature::REVEAL,
    bern_target_api::feature::POINTER_EVENTS,
    bern_target_api::feature::WHEEL,
    bern_target_api::feature::HOVER,
    bern_target_api::feature::SLIDER_HOVER,
    bern_target_api::feature::PLAYER_STATE,
    bern_target_api::feature::BOUND_LENGTHS,
    bern_target_api::feature::APPEARANCE,
    bern_target_api::feature::SHORTCUTS,
    bern_target_api::feature::BOUND_SHORTCUTS,
    bern_target_api::feature::SHORTCUT_INPUT,
    bern_target_api::feature::MENU_BAR,
    bern_target_api::feature::STATUS_ITEM,
    bern_target_api::feature::TABLE,
    bern_target_api::feature::CONTROLLED_TEXT,
    bern_target_api::feature::WATCH,
    bern_target_api::feature::LOGIN_SWITCH,
    bern_target_api::feature::PASTE,
    bern_target_api::feature::RUNTIME_THEME,
    bern_target_api::feature::BUTTON_CONTENT,
    bern_target_api::feature::TOOLBAR_SEARCH,
    bern_target_api::feature::SEARCH_RESULTS,
    bern_target_api::feature::TOGGLE_BUTTON,
    bern_target_api::feature::DIALOG_WINDOWS,
    bern_target_api::feature::RUNTIME_IMAGES,
    bern_target_api::feature::PICTURE_FILLS,
    bern_target_api::feature::COVER_IMAGES,
    bern_target_api::feature::FILE_PICK,
    bern_target_api::feature::SCROLL_SNAP,
    bern_target_api::feature::SCROLL_ANCHOR,
    bern_target_api::feature::SCROLL_BROWSING,
    bern_target_api::feature::BOUND_BLUR,
    bern_target_api::feature::TEXT_BOLD,
    bern_target_api::feature::RAILS,
    bern_target_api::feature::POPOVER_BUTTON,
    bern_target_api::feature::SIZE_CLASSES,
    bern_target_api::feature::TEXT_WEIGHTS,
    bern_target_api::feature::TEXT_LINES,
    bern_target_api::feature::IMAGE_CORNERS,
    bern_target_api::feature::SIDEBAR_WIDTH,
    bern_target_api::feature::CONTROLLED_VALUES,
    bern_target_api::feature::SEARCH_PALETTE,
    bern_target_api::feature::IMAGE_TEMPLATE,
    bern_target_api::feature::ADAPTIVE_IMAGES,
    bern_target_api::feature::SCALED_SCENE,
    bern_target_api::feature::ACCORDION_LAYOUT,
    bern_target_api::feature::CAPSULE_CONTROLS,
    bern_target_api::feature::MODAL_FLOW_TIMER,
    bern_target_api::feature::PAGE_BADGES,
    bern_target_api::feature::IDLE_CONTROL,
    bern_target_api::feature::QUIET_SHORTCUTS,
    bern_target_api::feature::FILL_BUTTONS,
    bern_target_api::feature::GRID_CELLS,
    bern_target_api::feature::PAGE_SEARCH,
    bern_target_api::feature::FORM_LAYOUT,
    bern_target_api::feature::MODAL_COVER,
    // A phone turns to landscape while `landscape` holds.
    bern_target_api::feature::WINDOW_LANDSCAPE,
    bern_target_api::feature::TEXT_FORMATS,
    bern_target_api::feature::WINDOW_SHORT,
    bern_target_api::feature::EDGE_GROW,
    bern_target_api::feature::DELAYED_APPEAR,
    bern_target_api::feature::TOOLBAR_LEADING,
    bern_target_api::feature::AXIS_PADDING,
    bern_target_api::feature::ICON_MENUS,
    bern_target_api::feature::ICON_LABELS,
    bern_target_api::feature::ICON_TABS,
    bern_target_api::feature::ROW_BASELINE,
    bern_target_api::feature::SIDEBAR_HEADER,
    bern_target_api::feature::UNTITLED_DETAIL,
    bern_target_api::feature::NATIVE_VIEW,
    bern_target_api::feature::APP_ICONS,
    bern_target_api::feature::POPUP_PLACEMENT,
    bern_target_api::feature::SWITCH_KNOB,
    bern_target_api::feature::BOLD_MENUS,
    bern_target_api::feature::TOOLTIP_KEYS,
    bern_target_api::feature::VERTICAL_SLIDERS,
    bern_target_api::feature::HOLD_SHORTCUTS,
    bern_target_api::feature::NATIVE_LISTS,
];

/// The features template with the extensions of each file kind from the
/// shared table, so pickers, drops and the manifest agree.
fn features_source(engine_kinds: &str) -> String {
    let table: Vec<String> = bern_ir::spec::FILE_KIND_EXTENSIONS
        .iter()
        .map(|(kind, extensions)| {
            let list: Vec<String> = extensions.iter().map(|ext| format!("\"{ext}\"")).collect();
            format!("\"{kind}\" to listOf({})", list.join(", "))
        })
        .collect();
    FEATURES
        .replace("/*bern:file-extensions*/", &table.join(", "))
        .replace("/*bern:engine-opens*/", engine_kinds)
}

/// Outline every declared application `.svg` icon referenced by the UI.
fn app_icons(
    app: &Application,
    project_dir: &std::path::Path,
) -> Result<Vec<(String, String)>, Vec<Diagnostic>> {
    let mut out = Vec::new();
    for name in bern_target_api::app_icon::used(app) {
        let path = project_dir.join(&name);
        let data = bern_target_api::app_icon::outline(&path)
            .map_err(|message| vec![Diagnostic::error(format!("app icon `{name}`: {message}"))])?;
        out.push((name, data));
    }
    Ok(out)
}

/// The views template with the icon branches from the icon table and outlined app icons.
fn views_source(app_icons: &[(String, String)]) -> String {
    let custom: Vec<String> = app_icons
        .iter()
        .map(|(name, data)| {
            format!(
                "{} -> appIcon({}, {})",
                names::string(name),
                names::string(name),
                names::string(data)
            )
        })
        .collect();
    VIEWS
        .replace("/*bern:icon-branches*/", &icons::branches())
        .replace("/*bern:app-icon-branches*/", &custom.join("\n        "))
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
                "scroll_origin_x",
                "scroll_origin_y",
                "nudge",
                "nudge_key",
                "nudge_scale",
                "nudge_motion",
                "reduce_motion",
                "spin",
                "spin_period",
                "glow",
                "glow_opacity",
                "blur",
                "appear_delay",
                "stagger",
                "idle_area",
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
        let app = &bern_target_api::svg::prepare(app, config);
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
        diagnostics.extend(bern_target_api::icon_set::check(
            app,
            "the Material icon set",
            icons::has,
        ));
        diagnostics
    }

    fn generate(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        bern_target_api::svg::validate(app, context)?;
        let watch = context.target.platform == "wearos";
        let platform = context.target.platform.as_str();
        let product = product_name(app);
        let package = names::package(&app.app.bundle_id);
        let main = PathBuf::from("project/app/src/main");
        let sources = main.join("kotlin");
        // The launcher icon: a declared `res` folder as it is (with its
        // adaptive, round and themed icons), or the declared PNG or the
        // Bern logo as drawn.
        let failed = |message: String| vec![Diagnostic::error(message)];
        let (mut launcher, round) = match icon::source(app, platform, &context.project_dir) {
            icon::Source::Native(dir) => {
                let res = icon::android_res(&dir).map_err(|message| {
                    failed(format!("the Android icon {} {message}", dir.display()))
                })?;
                let mut launcher = Vec::new();
                for file in &res.files {
                    let contents = std::fs::read(dir.join(file)).map_err(|error| {
                        failed(format!("cannot read {}: {error}", dir.join(file).display()))
                    })?;
                    launcher.push(GeneratedFile {
                        path: main.join("res").join(file),
                        contents,
                    });
                }
                (launcher, res.round)
            }
            _ => {
                let images =
                    icon::images(app, platform, &context.project_dir, icon::Shape::AsDrawn)
                        .map_err(failed)?;
                let launcher = adaptive_icon(&main, "ic_launcher", &icon::largest(&images).png)
                    .map_err(failed)?;
                (launcher, false)
            }
        };
        // The icons `WindowSet(icon:)` switches to: each its own launcher
        // alias, its resources renamed from `ic_launcher` to
        // `ic_launcher_<name>` so they sit beside the primary icon's.
        let mut icons = Vec::new();
        let alternates = icon::alternates(app, platform, &context.project_dir).map_err(failed)?;
        if !alternates.is_empty() {
            icons.push(project::LauncherIcon {
                alias: ".BernIconDefault".into(),
                icon: "ic_launcher".into(),
                round,
            });
        }
        for (variant, alternate) in icon::choices(app) {
            let Some(name) = alternate else { continue };
            let Some((_, source)) = alternates.iter().find(|(other, _)| *other == name) else {
                continue;
            };
            let renamed = format!("ic_launcher_{name}");
            let alternate_round = match source {
                icon::Source::Native(dir) => {
                    let res = icon::android_res(dir).map_err(|message| {
                        failed(format!("the Android icon {} {message}", dir.display()))
                    })?;
                    for file in &res.files {
                        let mut contents = std::fs::read(dir.join(file)).map_err(|error| {
                            failed(format!("cannot read {}: {error}", dir.join(file).display()))
                        })?;
                        let shown = file.to_string_lossy().replace('\\', "/");
                        if shown.ends_with(".xml") {
                            contents = String::from_utf8_lossy(&contents)
                                .replace("ic_launcher", &renamed)
                                .into_bytes();
                        }
                        launcher.push(GeneratedFile {
                            path: main
                                .join("res")
                                .join(shown.replace("ic_launcher", &renamed)),
                            contents,
                        });
                    }
                    res.round
                }
                icon::Source::Png(path) => {
                    let contents = std::fs::read(path).map_err(|error| {
                        failed(format!("cannot read {}: {error}", path.display()))
                    })?;
                    launcher.extend(adaptive_icon(&main, &renamed, &contents).map_err(failed)?);
                    false
                }
                icon::Source::Logo => false,
            };
            icons.push(project::LauncherIcon {
                alias: format!(".BernIcon{variant}"),
                icon: renamed,
                round: alternate_round,
            });
        }
        let min_sdk = minimum_sdk(context);
        let custom_icons = app_icons(app, &context.project_dir)?;
        let mut files = vec![
            GeneratedFile::text("project/settings.gradle.kts", project::settings(&product)),
            GeneratedFile::text("project/build.gradle.kts", project::root_build()),
            GeneratedFile::text("project/gradle.properties", project::GRADLE_PROPERTIES),
            GeneratedFile::text(
                "project/app/build.gradle.kts",
                project::app_build_for(
                    app,
                    &build_abis(context)
                        .iter()
                        .map(|abi| abi.name)
                        .collect::<Vec<_>>(),
                    split_abis(context),
                    watch,
                    min_sdk,
                ),
            ),
            GeneratedFile::text(
                main.join("AndroidManifest.xml"),
                project::manifest_for(app, round, context.target.android.as_ref(), &icons, watch),
            ),
            GeneratedFile::text(
                main.join("res/xml/bern_file_paths.xml"),
                project::FILE_PROVIDER_PATHS,
            ),
            GeneratedFile::text(
                main.join("baseline-prof.txt"),
                project::baseline_profile(app),
            ),
            GeneratedFile::text(
                main.join("res/values/themes.xml"),
                project::theme_for(false, watch),
            ),
            GeneratedFile::text(
                main.join("res/values-night/themes.xml"),
                project::theme_for(true, watch),
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
            GeneratedFile::text(
                main.join("res/drawable/bern_now_playing_like.xml"),
                project::NOW_PLAYING_LIKE,
            ),
            GeneratedFile::text(
                main.join("res/drawable/bern_now_playing_liked.xml"),
                project::NOW_PLAYING_LIKED,
            ),
            GeneratedFile::text(
                sources.join("dev/bern/runtime/BernRuntime.kt"),
                RUNTIME.replace("__APP_PACKAGE__", &package),
            ),
            GeneratedFile::text(
                sources.join("dev/bern/runtime/BernViews.kt"),
                views_source(&custom_icons),
            ),
            GeneratedFile::text(sources.join("dev/bern/runtime/BernStyle.kt"), STYLE),
            GeneratedFile::text(
                sources.join("dev/bern/runtime/BernFeatures.kt"),
                features_source(&app.engine_kinds()),
            ),
            GeneratedFile::text(
                sources.join("dev/bern/runtime/BernPictureTries.kt"),
                PICTURE_TRIES,
            ),
            GeneratedFile::text(
                sources.join("dev/bern/runtime/BernMedia.kt"),
                MEDIA.replace("__APP_PACKAGE__", &package),
            ),
            GeneratedFile::text(sources.join("dev/bern/runtime/BernScene.kt"), SCENE),
            GeneratedFile::text(sources.join("dev/bern/runtime/BernAppIcon.kt"), APP_ICON),
            GeneratedFile::text(
                sources
                    .join(package.replace('.', "/"))
                    .join("BernGenerated.kt"),
                Generator::new(app)
                    .with_watch(watch)
                    .with_multicast(
                        context
                            .target
                            .android
                            .as_ref()
                            .is_some_and(|options| options.multicast),
                    )
                    .generate(&package, app.protocol.fingerprint()),
            ),
            GeneratedFile::text("glue/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("glue/src/lib.rs", glue::lib_rs(context, app)),
        ];
        for native_file in kotlin::native::compose_native_files(app, &package, &context.project_dir)
        {
            files.push(GeneratedFile::text(
                sources.join(&native_file.rel_kotlin_path),
                native_file.contents,
            ));
        }
        if watch {
            files.retain(|file| !file.path.starts_with(sources.join("dev/bern/runtime")));
            files.extend(wear_runtime::files(
                &app.engine_kinds(),
                &package,
                &custom_icons,
            ));
        }
        if glue::ipc(context) {
            files.extend([
                GeneratedFile::text("tools/Cargo.toml", glue::tools_cargo_toml(context, app)),
                GeneratedFile::text("tools/src/cli.rs", glue::cli_rs(context, app)),
            ]);
        }
        if !watch && project::for_television(app) {
            files.push(GeneratedFile::text(
                main.join("res/drawable/bern_tv_banner.xml"),
                project::TV_BANNER,
            ));
        }
        if bern_target_api::svg::svg_used(app) {
            files.push(GeneratedFile::text(
                sources.join("dev/bern/runtime/BernSvg.kt"),
                include_str!("../templates/BernSvg.kt"),
            ));
        }
        if !watch && bern_target_api::svg::webpage_used(app) {
            files.push(GeneratedFile::text(
                sources.join("dev/bern/runtime/BernWebPage.kt"),
                include_str!("../templates/BernWebPage.kt"),
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
        files.extend(launcher);
        let manifest = serde_json::json!({
            "target": if watch { "compose-wearos" } else { self.descriptor().id },
            "ui": "compose",
            "platform": platform,
            "ir_version": app.ir_version,
            "generator": "bern-backend-compose",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "minimum_sdk": min_sdk,
            "target_sdk": TARGET_SDK,
            "abis": build_abis(context).iter().map(|abi| abi.name).collect::<Vec<_>>(),
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
        let tools = toolchain::find().map_err(|message| vec![Diagnostic::error(message)])?;
        let product = product_name(app);
        let output = &context.output_dir;
        let gradle_project = output.join("project");
        let profile = if context.release { "release" } else { "debug" };
        let glue_target = context.workspace_dir.join("target/bern-glue");
        let bin = toolchain::ndk_bin(&tools.ndk);
        let path = |path: &PathBuf| path.display().to_string();
        let abis = build_abis(context);
        let watch = context.target.platform == "wearos";
        let min_sdk = minimum_sdk(context);
        let engine = |abi: &Abi| {
            let compiler =
                |suffix: &str| path(&toolchain::ndk_compiler(&bin, abi.clang, min_sdk, suffix));
            let target = abi.rust.replace('-', "_");
            let env = vec![
                (
                    format!("CARGO_TARGET_{}_LINKER", target.to_uppercase()),
                    compiler("clang"),
                ),
                (format!("CC_{target}"), compiler("clang")),
                // C++ in the engine's crates, such as Oboe's audio.
                (format!("CXX_{target}"), compiler("clang++")),
                (format!("AR_{target}"), path(&bin.join("llvm-ar"))),
            ];
            let mut cargo = vec![
                "build".to_string(),
                "--manifest-path".into(),
                path(&output.join("glue/Cargo.toml")),
                "--target".into(),
                abi.rust.into(),
                "--target-dir".into(),
                path(&glue_target),
            ];
            if context.release {
                cargo.push("--release".into());
            }
            BuildStep::Run {
                description: format!("Build the Rust engine and bridge for {}", abi.name),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("glue"),
                env,
            }
        };
        let mut gradle_env = vec![("ANDROID_HOME".to_string(), path(&tools.sdk))];
        if let Some(java_home) = &tools.java_home {
            gradle_env.push(("JAVA_HOME".to_string(), path(java_home)));
        }
        let task = if context.release {
            "assembleRelease"
        } else {
            "assembleDebug"
        };
        // Split by ABI, each APK is named after its ABI, and the first
        // stands for the build.
        let built = if split_abis(context) {
            format!("app-{}-{profile}", abis[0].name)
        } else {
            format!("app-{profile}")
        };
        let apk = gradle_project
            .join("app/build/outputs/apk")
            .join(profile)
            .join(format!("{built}.apk"));
        let product_apk = output.join(format!("{product}.apk"));
        // Only this application's Kotlin: every application built here
        // shares the output directory, and sources another one left behind
        // would be compiled into this one.
        let package = names::package(&app.app.bundle_id);
        let native_keep: Vec<PathBuf> =
            kotlin::native::compose_native_files(app, &package, &context.project_dir)
                .into_iter()
                .map(|file| file.rel_kotlin_path)
                .collect();
        let kotlin = BuildStep::Mirror {
            dir: gradle_project.join("app/src/main/kotlin"),
            files: Vec::new(),
            keep: if watch {
                wear_runtime::files(&app.engine_kinds(), &package, &[])
                    .into_iter()
                    .map(|file| {
                        file.path
                            .strip_prefix("project/app/src/main/kotlin")
                            .expect("runtime source")
                            .to_path_buf()
                    })
                    .chain([PathBuf::from(package.replace('.', "/")).join("BernGenerated.kt")])
                    .chain(native_keep)
                    .collect()
            } else {
                [
                    "BernRuntime",
                    "BernViews",
                    "BernStyle",
                    "BernFeatures",
                    "BernPictureTries",
                    "BernMedia",
                    "BernScene",
                    "BernAppIcon",
                ]
                .iter()
                .map(|name| PathBuf::from(format!("dev/bern/runtime/{name}.kt")))
                .chain(
                    bern_target_api::svg::svg_used(app)
                        .then(|| PathBuf::from("dev/bern/runtime/BernSvg.kt")),
                )
                .chain(
                    (!watch && bern_target_api::svg::webpage_used(app))
                        .then(|| PathBuf::from("dev/bern/runtime/BernWebPage.kt")),
                )
                .chain([PathBuf::from(package.replace('.', "/")).join("BernGenerated.kt")])
                .chain(native_keep)
                .collect()
            },
        };
        let assets = gradle_project.join("app/src/main/assets/bern/resources");
        let mut steps = Vec::new();
        // Drop native libraries left over from ABIs no longer in this build so
        // `bundleRelease` never packs stale architectures.
        for abi in ABIS {
            if !abis.iter().any(|active| active.name == abi.name) {
                steps.push(BuildStep::RemoveIfExists {
                    path: gradle_project.join("app/src/main/jniLibs").join(abi.name),
                });
            }
        }
        for abi in &abis {
            let jni = gradle_project.join("app/src/main/jniLibs").join(abi.name);
            steps.extend([
                engine(abi),
                BuildStep::CreateDir { path: jni.clone() },
                BuildStep::Copy {
                    from: glue_target
                        .join(abi.rust)
                        .join(profile)
                        .join("libbern_app.so"),
                    to: jni.join("libbern_app.so"),
                },
            ]);
            if context
                .target
                .android
                .as_ref()
                .is_some_and(|options| options.cxx_runtime)
            {
                // The sysroot names 32-bit ARM by its older triple.
                let folder = abi.rust.replace("armv7", "arm");
                steps.push(BuildStep::Copy {
                    from: bin
                        .parent()
                        .expect("the NDK's bin folder has a parent")
                        .join("sysroot/usr/lib")
                        .join(folder)
                        .join("libc++_shared.so"),
                    to: jni.join("libc++_shared.so"),
                });
            }
        }
        steps.extend([
            // Declared resources ship as assets, which the runtime unpacks.
            BuildStep::Mirror {
                dir: assets,
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
        ]);
        // A release build's baseline profile beside the APK, in the form
        // Android 12 and later take at install (Gradle's metadata lists it
        // for API 31 and up): `bern run` installs the two together, so the
        // runtime and the interface are compiled ahead of time from the
        // first start. A debug build is never compiled; an earlier
        // release's profile must not go with it.
        let product_profile = product_apk.with_extension("dm");
        steps.push(if context.release {
            BuildStep::Copy {
                from: gradle_project
                    .join("app/build/outputs/apk/release/baselineProfiles/0")
                    .join(format!("{built}.dm")),
                to: product_profile,
            }
        } else {
            BuildStep::RemoveIfExists {
                path: product_profile,
            }
        });
        let mut products = vec![product_apk.clone()];
        let mut forward = None;
        if glue::ipc(context) {
            // The command line runs on this computer.
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
                description: "Build the command line".into(),
                program: "cargo".into(),
                args: cargo,
                cwd: output.join("tools"),
                env: Vec::new(),
            });
            let cli = app.app.id.replace('_', "-");
            let program = format!("{cli}{}", std::env::consts::EXE_SUFFIX);
            steps.push(BuildStep::program_dir(output.join("bin"), [&program]));
            steps.push(BuildStep::Copy {
                from: tools_target.join(profile).join(&program),
                to: output.join("bin").join(&program),
            });
            products.push(output.join("bin").join(&program));
            forward = Some((
                bern_transport_ipc::socket_dir(&app.app.bundle_id).join(if watch {
                    bern_transport_ipc::WEAROS_FORWARD
                } else {
                    bern_transport_ipc::ANDROID_FORWARD
                }),
                bern_transport_ipc::android_socket(&app.app.bundle_id),
            ));
            products.push(context.output_dir.join(skill::path(app)));
        } else {
            steps.push(BuildStep::RemoveIfExists {
                path: output.join("bin"),
            });
        }
        // The agent skill beside `bin/`, where the app listens.
        let place = glue::ipc(context).then(|| {
            skill::Place::beside_bin(app, cfg!(windows))
                .connect(skill::android_connect_for(app, watch))
        });
        steps.extend(skill::steps(app, &context.output_dir, place.as_ref()));
        Ok(BuildPlan {
            steps,
            notes: Vec::new(),
            run: Some(Run::Android {
                apk: product_apk,
                package: app.app.bundle_id.clone(),
                adb: toolchain::adb(),
                forward,
                watch,
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
        let app = &bern_target_api::svg::prepare(app, &context.target);
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
        let platform = context.target.platform.as_str();
        let name = |extension: &str| {
            dist.join(format!(
                "{product}-{}-{platform}.{extension}",
                app.app.version
            ))
        };
        let path = |path: &PathBuf| path.display().to_string();
        let mut env = vec![("ANDROID_HOME".to_string(), path(&tools.sdk))];
        if let Some(java_home) = &tools.java_home {
            env.push(("JAVA_HOME".to_string(), path(java_home)));
        }
        let mut steps = vec![
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
                from: gradle_project.join("app/build/outputs/bundle/release/app-release.aab"),
                to: name("aab"),
            },
        ];
        let apks = gradle_project.join("app/build/outputs/apk/release");
        let mut products = vec![name("aab")];
        if split_abis(context) {
            for abi in build_abis(context) {
                let apk = dist.join(format!(
                    "{product}-{}-{platform}-{}.apk",
                    app.app.version, abi.name
                ));
                steps.push(BuildStep::Copy {
                    from: apks.join(format!("app-{}-release.apk", abi.name)),
                    to: apk.clone(),
                });
                products.push(apk);
            }
        } else {
            steps.push(BuildStep::Copy {
                from: apks.join("app-release.apk"),
                to: name("apk"),
            });
            products.push(name("apk"));
        }
        let mut notes = Vec::new();
        if signing.android_keystore.is_none() {
            notes.push(
                "signed with the debug key: installable for testing, not accepted by stores (set BERN_ANDROID_KEYSTORE, BERN_ANDROID_KEYSTORE_PASSWORD and BERN_ANDROID_KEY_ALIAS)"
                    .into(),
            );
        }
        Ok(BuildPlan {
            steps,
            products,
            run: None,
            notes,
        })
    }

    fn eject(
        &self,
        app: &Application,
        context: &BuildContext,
        options: &bern_target_api::EjectOptions,
    ) -> Result<bern_target_api::EjectedProject, Vec<Diagnostic>> {
        use bern_target_api::eject::{
            EjectEngine, EjectedProject, clean_generated_file, copy_declared_resources, readme,
            sanitized_context,
        };
        let context = &sanitized_context(context, options);
        let generated = self.generate(app, context)?;
        let app = &bern_target_api::svg::prepare(app, &context.target);
        let watch = context.target.platform == "wearos";
        let product = product_name(app);
        let package = names::package(&app.app.bundle_id);
        let pkg_dir = PathBuf::from("app/src/main/kotlin").join(package.replace('.', "/"));
        let generated_kt_rel = PathBuf::from("project")
            .join(&pkg_dir)
            .join("BernGenerated.kt");

        let mut files: Vec<GeneratedFile> = Vec::new();
        let mut rust_manifests = Vec::new();
        let mut prebuilt_staging = Vec::new();
        let mut prebuilt_steps = Vec::new();
        let abis = build_abis(context);
        let min_sdk = minimum_sdk(context);

        for file in generated {
            if file.path == std::path::Path::new("build.json") || file.path == generated_kt_rel {
                continue;
            }
            if let Ok(rel) = file.path.strip_prefix("project") {
                if rel == std::path::Path::new("app/build.gradle.kts")
                    && options.engine == EjectEngine::Source
                {
                    let mut text = String::from_utf8_lossy(&file.contents).into_owned();
                    let abi_triples: Vec<(&str, &str, &str)> = abis
                        .iter()
                        .map(|abi| (abi.name, abi.rust, abi.clang))
                        .collect();
                    text.push_str(&project::rust_gradle_tasks(&abi_triples, min_sdk));
                    files.push(GeneratedFile::text(rel, text));
                } else {
                    files.push(GeneratedFile {
                        path: rel.to_path_buf(),
                        contents: file.contents,
                    });
                }
            } else if file.path.starts_with("glue") || file.path.starts_with("tools") {
                match options.engine {
                    EjectEngine::Source => {
                        if file.path.starts_with("tools") && !options.with_cli {
                            continue;
                        }
                        if file.path.file_name().is_some_and(|n| n == "Cargo.lock") {
                            continue;
                        }
                        let dest = PathBuf::from("rust").join(&file.path);
                        if dest.file_name().is_some_and(|n| n == "Cargo.toml") {
                            rust_manifests.push(dest.clone());
                        }
                        files.push(GeneratedFile {
                            path: dest,
                            contents: file.contents,
                        });
                    }
                    EjectEngine::Prebuilt => {
                        if file.path.starts_with("glue") {
                            prebuilt_staging.push(file);
                        }
                    }
                }
            } else {
                // Static SVG assets sit at `app/src/main/assets/bern/resources/...`.
                files.push(GeneratedFile {
                    path: PathBuf::from("app/src/main/assets/bern/resources").join(&file.path),
                    contents: file.contents,
                });
            }
        }

        // Split generated Kotlin sources (`fingerprint = 0` relaxes catalog check).
        let split = Generator::new(app)
            .with_watch(watch)
            .with_multicast(
                context
                    .target
                    .android
                    .as_ref()
                    .is_some_and(|android| android.multicast),
            )
            .generate_split(&package, 0);
        for (rel, content) in split {
            files.push(GeneratedFile::text(pkg_dir.join(rel), content));
        }

        // Bundle declared application resources into assets.
        files.extend(copy_declared_resources(
            app,
            context,
            std::path::Path::new("app/src/main/assets/bern/resources"),
        )?);

        // Gradle wrapper configuration for Android Studio.
        files.push(GeneratedFile::text(
            "gradle/wrapper/gradle-wrapper.properties",
            project::GRADLE_WRAPPER_PROPERTIES,
        ));

        if options.engine == EjectEngine::Prebuilt {
            let tools = toolchain::find().map_err(|message| vec![Diagnostic::error(message)])?;
            let bin = toolchain::ndk_bin(&tools.ndk);
            let glue_target = context.workspace_dir.join("target/bern-glue");
            let profile = if options.release { "release" } else { "debug" };
            let staging = &options.staging_dir;
            for abi in &abis {
                let compiler = |suffix: &str| {
                    toolchain::ndk_compiler(&bin, abi.clang, min_sdk, suffix)
                        .display()
                        .to_string()
                };
                let env_target = abi.rust.replace('-', "_");
                let env = vec![
                    (
                        format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
                        compiler("clang"),
                    ),
                    (format!("CC_{env_target}"), compiler("clang")),
                    (format!("CXX_{env_target}"), compiler("clang++")),
                    (
                        format!("AR_{env_target}"),
                        bin.join("llvm-ar").display().to_string(),
                    ),
                ];
                let mut cargo = vec![
                    "build".to_string(),
                    "--manifest-path".into(),
                    staging.join("glue/Cargo.toml").display().to_string(),
                    "--target".into(),
                    abi.rust.into(),
                    "--target-dir".into(),
                    glue_target.display().to_string(),
                ];
                if options.release {
                    cargo.push("--release".into());
                }
                let jni_dir = context
                    .output_dir
                    .join("app/src/main/jniLibs")
                    .join(abi.name);
                prebuilt_steps.extend([
                    BuildStep::Run {
                        description: format!("Build prebuilt Rust engine for {}", abi.name),
                        program: "cargo".into(),
                        args: cargo,
                        cwd: staging.join("glue"),
                        env,
                    },
                    BuildStep::CreateDir {
                        path: jni_dir.clone(),
                    },
                    BuildStep::Copy {
                        from: glue_target
                            .join(abi.rust)
                            .join(profile)
                            .join("libbern_app.so"),
                        to: jni_dir.join("libbern_app.so"),
                    },
                ]);
            }
        }

        if options.with_cli && skill::listens(&context.target.channels) {
            let cli_name = app.app.id.replace('_', "-");
            files.push(GeneratedFile::text(
                skill::path(app),
                skill::markdown(
                    app,
                    &skill::Place {
                        cli: cli_name,
                        installed: None,
                        connect: None,
                    },
                ),
            ));
        }

        let app_title = app.app.name.get(&app.languages.default).unwrap_or(&product);
        let platform_label = if watch {
            "Wear OS · Jetpack Compose"
        } else {
            "Android · Jetpack Compose"
        };
        let build_help = match options.engine {
            EjectEngine::Source => {
                "Open this folder in **Android Studio** and click **Run**, or run `gradle assembleDebug`.\n\nGradle automatically runs `buildRustEngineDebug` / `buildRustEngineRelease` against the self-contained Rust workspace in `rust/` using Android Studio's NDK."
            }
            EjectEngine::Prebuilt => {
                "Open this folder in **Android Studio** and click **Run**, or run `gradle assembleDebug`.\n\nNo Rust toolchain is required: pre-compiled `libbern_app.so` binaries are bundled under `app/src/main/jniLibs/`."
            }
        };
        let layout_help = format!(
            "- `app/src/main/kotlin/{}/MainActivity.kt` — Application entry point\n- `app/src/main/kotlin/{}/views/` — One Compose file per UI component\n- `app/src/main/kotlin/{}/windows/` — One Compose file per window\n- `app/src/main/kotlin/dev/bern/runtime/` — Self-contained Bern Compose runtime\n- `app/src/main/assets/bern/resources/` — Bundled application resources",
            package.replace('.', "/"),
            package.replace('.', "/"),
            package.replace('.', "/"),
        );
        files.push(GeneratedFile::text(
            "README.md",
            readme(
                app_title,
                platform_label,
                options.engine,
                &[
                    ("Opening & Building", build_help),
                    ("Project Layout", &layout_help),
                ],
            ),
        ));

        let files = files.into_iter().map(clean_generated_file).collect();
        Ok(EjectedProject {
            files,
            executables: Vec::new(),
            rust_manifests,
            rust_root: PathBuf::from("rust"),
            prebuilt_staging,
            prebuilt_steps,
            notes: vec!["Open the ejected folder in Android Studio to build and edit.".into()],
        })
    }
}

/// The launcher icon `name` from a square PNG: the image itself for
/// launchers before Android 8, and an adaptive icon whose layers fill the
/// launcher's shape ([`icon::adaptive_layers`]); a picture that cannot be
/// read stays the image alone.
fn adaptive_icon(
    main: &std::path::Path,
    name: &str,
    png: &[u8],
) -> Result<Vec<GeneratedFile>, String> {
    let legacy = GeneratedFile {
        path: main.join(format!("res/mipmap-xxxhdpi/{name}.png")),
        contents: png.to_vec(),
    };
    let Ok((foreground, [r, g, b])) = icon::adaptive_layers(png) else {
        return Ok(vec![legacy]);
    };
    Ok(vec![
        legacy,
        GeneratedFile {
            path: main.join(format!("res/mipmap-xxxhdpi/{name}_foreground.png")),
            contents: foreground,
        },
        GeneratedFile::text(
            main.join(format!("res/drawable/{name}_background.xml")),
            format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<shape xmlns:android=\"http://schemas.android.com/apk/res/android\">\n    <solid android:color=\"#{r:02X}{g:02X}{b:02X}\" />\n</shape>\n"
            ),
        ),
        GeneratedFile::text(
            main.join(format!("res/mipmap-anydpi-v26/{name}.xml")),
            format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n    <background android:drawable=\"@drawable/{name}_background\" />\n    <foreground android:drawable=\"@mipmap/{name}_foreground\" />\n</adaptive-icon>\n"
            ),
        ),
    ])
}
