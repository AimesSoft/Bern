//! The `gpui` target on macOS, Windows and Linux.
//!
//! Generates a Rust application from the shared IR that draws its
//! interface with GPUI after Bern's own visual specification, in colors
//! from the Bern theme algorithm, the same on every desktop. The engine
//! runs in the application process; the runtime (`bern-gpui`) reaches it
//! through the in-process interface the other native targets use. GPUI is
//! a requirement of this target's build only.
//!
//! The target is chosen explicitly (`ui = "gpui"`) and is no platform's
//! default. Its capabilities grow component by component: what it does
//! not list fails the build where it is declared.

mod glue;
mod rust;

use bern_ir::style::StyleState;
use bern_ir::{Application, IR_VERSION};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, StyleSupport, Target, TargetConfig, TargetDescriptor, WindowCapabilities, archive,
    check_common, dist_dir, feature, files_below, package_architecture, skill,
};
use std::path::PathBuf;

pub use rust::Generator;

/// The `gpui` target on one desktop platform; it builds on that platform.
pub struct Gpui {
    platform: &'static str,
}

impl Gpui {
    pub const MACOS: Gpui = Gpui { platform: "macos" };
    pub const WINDOWS: Gpui = Gpui {
        platform: "windows",
    };
    pub const LINUX: Gpui = Gpui { platform: "linux" };
}

/// The further parts of the IR this target builds.
const FEATURES: &[&str] = &[
    feature::TEXT_FORMATS,
    feature::TEXT_LINES,
    feature::AXIS_PADDING,
    feature::ADAPTIVE_IMAGES,
    feature::SCALED_SCENE,
    feature::MODAL_FLOW_TIMER,
    feature::CONTROLLED_VALUES,
    feature::CONTROLLED_TEXT,
    feature::FILL_BUTTONS,
    feature::MODAL_COVER,
    feature::FORM_LAYOUT,
    feature::POPOVER_BUTTON,
    feature::SIZE_CLASSES,
    feature::SESSION_ACTIONS,
    feature::BOUND_LENGTHS,
    feature::RUNTIME_THEME,
    feature::RUNTIME_PALETTE,
    feature::FILE_PICK,
    feature::PASTE,
    feature::TABLE,
    feature::MENU_BAR,
    feature::SHORTCUTS,
    feature::IMAGE_CORNERS,
    feature::IMAGE_TEMPLATE,
    feature::RUNTIME_IMAGES,
    feature::TOOLBAR_SEARCH,
    feature::TOOLBAR_LEADING,
    feature::SEARCH_RESULTS,
    feature::SEARCH_PALETTE,
    feature::PAGE_SEARCH,
    feature::ROW_BASELINE,
    feature::BUTTON_CONTENT,
    feature::REVEAL,
    feature::POINTER_EVENTS,
    feature::WHEEL,
    feature::SLIDER_HOVER,
    feature::PLAYER_STATE,
    feature::SCROLL_SNAP,
    feature::PICTURE_FILLS,
    feature::COVER_IMAGES,
    feature::ACCORDION_LAYOUT,
    feature::IDLE_CONTROL,
    feature::QUIET_SHORTCUTS,
    feature::DIALOG_WINDOWS,
    feature::WINDOW_LANDSCAPE,
    feature::WINDOW_SHORT,
    feature::CAPSULE_CONTROLS,
    feature::ICON_LABELS,
    feature::APPEARANCE,
    feature::BOUND_SHORTCUTS,
    feature::TOGGLE_BUTTON,
    feature::TEXT_WEIGHTS,
    feature::SHORTCUT_INPUT,
    feature::WATCH,
    feature::ICON_MENUS,
    feature::RAILS,
    feature::SIDEBAR_WIDTH,
    feature::SIDEBAR_HEADER,
    feature::UNTITLED_DETAIL,
    feature::PAGE_BADGES,
    feature::GRID_CELLS,
    feature::ICON_TABS,
    feature::TAB_ACCESSORY,
    feature::DELAYED_APPEAR,
    feature::EDGE_GROW,
    feature::TEXT_BOLD,
    feature::SCROLL_ANCHOR,
    feature::SCROLL_BROWSING,
    feature::BOUND_BLUR,
    feature::NATIVE_VIEW,
    feature::STATUS_ITEM,
    feature::LOGIN_SWITCH,
    feature::WEB_CONTENT_MODE,
];

/// PNG pixel sizes generated for `StatusItem(image:)`.
pub(crate) const STATUS_SIDES: [u32; 6] = [16, 22, 24, 32, 48, 64];

const CONTROL_STATES: &[&str] = &[
    "background",
    "foreground",
    "border",
    "shadow",
    "opacity",
    "scale",
    "system",
    "none",
];

const ROW_STATES: &[&str] = &[
    "background",
    "foreground",
    "scale",
    "opacity",
    "system",
    "none",
];

/// What `style!` can change: the surface of a `Stack`, a `Form`, the
/// sidebar, the detail and a popover's panel, and the look of buttons and
/// sidebar items at rest, under the pointer, pressed, disabled and chosen.
const STYLES: &[StyleSupport] = &[
    StyleSupport {
        component: "Stack",
        base: &["background", "foreground", "border", "corner", "shadow"],
        states: &[(StyleState::Hover, ROW_STATES)],
    },
    StyleSupport {
        component: "Form",
        base: &["background"],
        states: &[],
    },
    StyleSupport {
        component: "Sidebar",
        base: &["background"],
        states: &[],
    },
    StyleSupport {
        component: "Detail",
        base: &["background"],
        states: &[],
    },
    // The panel it opens.
    StyleSupport {
        component: "PopoverButton",
        base: &["background", "foreground"],
        states: &[],
    },
    StyleSupport {
        component: "SidebarItem",
        base: &["background", "foreground", "corner", "padding"],
        states: &[
            (StyleState::Hover, ROW_STATES),
            (StyleState::Pressed, ROW_STATES),
            (
                StyleState::Selected,
                &[
                    "fill",
                    "background",
                    "foreground",
                    "scale",
                    "opacity",
                    "system",
                    "none",
                ],
            ),
        ],
    },
    StyleSupport {
        component: "Button",
        base: &[
            "background",
            "foreground",
            "border",
            "shadow",
            "corner",
            "padding",
            "size",
        ],
        states: &[
            (StyleState::Hover, CONTROL_STATES),
            (StyleState::Pressed, CONTROL_STATES),
            (StyleState::Disabled, CONTROL_STATES),
            (StyleState::Focus, &["system", "none"]),
        ],
    },
    StyleSupport {
        component: "IconButton",
        base: &[
            "background",
            "foreground",
            "border",
            "shadow",
            "corner",
            "padding",
            "size",
        ],
        states: &[
            (StyleState::Hover, CONTROL_STATES),
            (StyleState::Pressed, CONTROL_STATES),
            (StyleState::Disabled, CONTROL_STATES),
            (StyleState::Focus, &["system", "none"]),
        ],
    },
];

/// The pictures this target shows as the application's icon: `[app.icon]`
/// at `icons/app.<ext>`, and each alternate `WindowSet(icon:)` switches to
/// at `icons/<name>.<ext>`, with the declared files they come from. Only a
/// PNG or an `.icns` file can be shown; for another form of the platform's
/// own, the `default` entry is taken.
pub(crate) fn icon_pictures(app: &Application, platform: &str) -> Vec<(String, String)> {
    let shown = |entries: &[Option<&String>]| {
        entries
            .iter()
            .flatten()
            .find(|name| {
                let name = name.to_ascii_lowercase();
                name.ends_with(".png") || name.ends_with(".icns")
            })
            .map(|name| name.to_string())
    };
    let extension = |name: &str| {
        std::path::Path::new(name)
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    };
    let mut pictures = Vec::new();
    if let Some(name) = shown(&[app.app.platform_icons.get(platform), app.app.icon.as_ref()]) {
        pictures.push((format!("icons/app.{}", extension(&name)), name));
    }
    for (_, alternate) in bern_target_api::icon::choices(app) {
        let Some(alternate) = alternate else { continue };
        let Some(entries) = app.app.alternate_icons.get(&alternate) else {
            continue;
        };
        if let Some(name) = shown(&[entries.get(platform), entries.get("default")]) {
            pictures.push((format!("icons/{alternate}.{}", extension(&name)), name));
        }
    }
    pictures
}

/// The application as this target builds it: native WebViews (`WKWebView` /
/// `WebView2` / `WebKitGTK`) host `WebPage`, static SVGs lower to `Image`, and
/// animated SVGs compile to `BSVG` scenes played natively in Rust.
fn prepared(app: &Application, config: &TargetConfig) -> Application {
    bern_target_api::svg::prepare(app, config)
}

pub fn product_name(app: &Application) -> String {
    bern_target_api::names::pascal(&app.app.id)
}

/// The macOS product: `<Product>.app` with the program in
/// `Contents/MacOS`, the resources in `Contents/Resources/resources`, the
/// command line in `Contents/Helpers` with the agent
/// skill in `Contents/Resources/skills`, so a copy moved or installed
/// anywhere carries them; `bin/` beside it links to the helpers. The
/// helpers are signed before the app, whose signature seals them.
fn mac_bundle_plan(
    app: &Application,
    context: &BuildContext,
    build: BuildStep,
    shipped: Vec<(PathBuf, PathBuf)>,
    built: &std::path::Path,
    program: &str,
) -> BuildPlan {
    let output = &context.output_dir;
    let product = product_name(app);
    let bundle = output.join(format!("{product}.app"));
    let contents = bundle.join("Contents");
    let helpers = contents.join("Helpers");
    let sign = |path: PathBuf, description: &str| BuildStep::Run {
        description: description.into(),
        program: "codesign".into(),
        args: vec![
            "--force".into(),
            "--sign".into(),
            "-".into(),
            path.display().to_string(),
        ],
        cwd: output.clone(),
        env: vec![],
    };
    let mut steps = vec![
        build,
        BuildStep::CreateDir {
            path: contents.join("MacOS"),
        },
        BuildStep::Mirror {
            dir: contents.join("Resources/resources"),
            files: shipped,
            keep: vec![],
        },
        BuildStep::Copy {
            from: built.join(format!("{program}-ui")),
            to: contents.join("MacOS").join(program),
        },
        BuildStep::program_dir(helpers.clone(), [program]),
    ];
    steps.push(BuildStep::Copy {
        from: built.join(program),
        to: helpers.join(program),
    });
    steps.push(sign(
        helpers.join(program),
        "Sign the command line for local use",
    ));
    let listens = skill::listens(&context.target.channels);
    if listens {
        let inside = skill::Place {
            cli: format!("../../../Helpers/{program}"),
            installed: Some(format!(
                "/Applications/{product}.app/Contents/Helpers/{program}"
            )),
            connect: None,
        };
        steps.push(BuildStep::Write {
            path: contents.join("Resources").join(skill::path(app)),
            contents: skill::markdown(app, &inside).into_bytes(),
        });
    }
    // The primary icon and the alternates as Icon Composer documents, into
    // the bundle's `Assets.car`.
    use bern_target_api::icon;
    let documents: Vec<String> = icon::alternates(app, "macos", &context.project_dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, source)| {
            matches!(source, icon::Source::Native(path) if icon::is_icon_document(path))
        })
        .map(|(name, _)| alternate_icon(&name))
        .collect();
    let primary = icon_document(app, context).is_some();
    if primary || !documents.is_empty() {
        let mut args = vec!["actool".to_string()];
        let mut names = Vec::new();
        if primary {
            args.push(output.join(ICON_DOCUMENT).display().to_string());
            names.extend(["--app-icon".to_string(), "AppIcon".into()]);
        }
        for document in &documents {
            args.push(
                output
                    .join(format!("{document}.icon"))
                    .display()
                    .to_string(),
            );
            names.extend(["--alternate-app-icon".to_string(), document.clone()]);
        }
        args.extend([
            "--compile".to_string(),
            contents.join("Resources").display().to_string(),
            "--platform".into(),
            "macosx".into(),
            "--minimum-deployment-target".into(),
            MINIMUM_MACOS.into(),
        ]);
        args.extend(names);
        args.extend([
            "--output-partial-info-plist".to_string(),
            output.join("AssetCatalog.plist").display().to_string(),
            "--output-format".into(),
            "human-readable-text".into(),
            "--errors".into(),
            "--warnings".into(),
        ]);
        steps.push(BuildStep::Run {
            description: "Compile the icons".into(),
            program: "xcrun".into(),
            args,
            cwd: output.clone(),
            env: vec![],
        });
    }
    steps.extend([
        sign(bundle.clone(), "Sign the app for local use"),
        // Each build replaces the bundle at the same place, where Finder
        // and the Dock keep showing the icon they first saw; registering it
        // again makes them read the new one.
        BuildStep::Run {
            description: "Refresh the app's icon in Finder and the Dock".into(),
            program: "touch".into(),
            args: vec![bundle.display().to_string()],
            cwd: output.clone(),
            env: vec![],
        },
        BuildStep::Run {
            description: "Register the app again with Launch Services".into(),
            program: LSREGISTER.into(),
            args: vec!["-f".into(), bundle.display().to_string()],
            cwd: output.clone(),
            env: vec![],
        },
        BuildStep::program_dir(output.join("bin"), [program]),
    ]);
    steps.push(BuildStep::Run {
        description: "Link the command line in bin/".into(),
        program: "ln".into(),
        args: vec![
            "-sf".into(),
            format!("../{product}.app/Contents/Helpers/{program}"),
            output.join("bin").join(program).display().to_string(),
        ],
        cwd: output.clone(),
        env: vec![],
    });
    // The agent skill beside `bin/` too, where the app listens.
    let place = listens.then(|| skill::Place::beside_bin(app, false));
    steps.extend(skill::steps(app, output, place.as_ref()));
    let mut products = vec![bundle.clone(), output.join("bin").join(program)];
    if listens {
        products.push(output.join(skill::path(app)));
    }
    BuildPlan {
        steps,
        notes: Vec::new(),
        run: Some(Run::Program(contents.join("MacOS").join(program))),
        products,
    }
}

/// A declared Icon Composer document for macOS, copied beside the bundle
/// under this name for `actool`, which compiles it into the bundle's
/// `Assets.car` and writes `AppIcon.icns` beside it for earlier systems.
const ICON_DOCUMENT: &str = "AppIcon.icon";

/// The asset or resource name of the alternate icon `name`, beside
/// `AppIcon`: what `WindowSet(icon:)` switches to.
fn alternate_icon(name: &str) -> String {
    format!("AppIcon-{name}")
}

/// The Icon Composer document declared as the macOS icon, if it is one.
fn icon_document(app: &Application, context: &BuildContext) -> Option<PathBuf> {
    use bern_target_api::icon;
    match icon::source(app, "macos", &context.project_dir) {
        icon::Source::Native(path) if icon::is_icon_document(&path) => Some(path),
        _ => None,
    }
}

/// Where macOS keeps an icon's cache fresh.
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

/// The oldest macOS GPUI draws on.
const MINIMUM_MACOS: &str = "11.0";

/// The macOS app bundle's own files: its Info.plist, the application's
/// name in each language, and its icon as an `.icns` of the declared PNG
/// (or the Bern logo) on a full square that macOS cuts to its shape, as
/// the SwiftUI target ships it.
fn mac_bundle_files(
    app: &Application,
    context: &BuildContext,
) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
    use bern_target_api::icon;
    let contents = PathBuf::from(format!("{}.app/Contents", product_name(app)));
    let name = |language: &str| {
        let names = &app.app.name.translations;
        names
            .get(language)
            .or_else(|| names.get(&app.languages.default))
            .or_else(|| names.values().next())
            .cloned()
            .unwrap_or_else(|| product_name(app))
    };
    let xml = |value: &str| {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let localizations: String = app
        .languages
        .supported
        .iter()
        .map(|language| format!("\n        <string>{}</string>", xml(language)))
        .collect();
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>{region}</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundleExecutable</key>
    <string>{executable}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>{icon_name}
    <key>CFBundleIdentifier</key>
    <string>{bundle}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleLocalizations</key>
    <array>{localizations}
    </array>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>LSMinimumSystemVersion</key>
    <string>{MINIMUM_MACOS}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#,
        region = xml(&app.languages.default),
        name = xml(&name(&app.languages.default)),
        executable = xml(&bern_target_api::glue::program(app)),
        bundle = xml(&app.app.bundle_id),
        version = xml(&app.app.version),
        icon_name = if icon_document(app, context).is_some() {
            "\n    <key>CFBundleIconName</key>\n    <string>AppIcon</string>"
        } else {
            ""
        },
    );
    let mut files = vec![GeneratedFile::text(contents.join("Info.plist"), plist)];
    let strings = |value: &str| value.replace('\\', "\\\\").replace('"', "\\\"");
    for language in &app.languages.supported {
        let name = strings(&name(language));
        files.push(GeneratedFile::text(
            contents.join(format!("Resources/{language}.lproj/InfoPlist.strings")),
            format!("\"CFBundleDisplayName\" = \"{name}\";\n\"CFBundleName\" = \"{name}\";\n"),
        ));
    }
    let failed = |message: String| vec![Diagnostic::error(message)];
    // An Icon Composer document goes beside the bundle under `into`, for
    // `actool` to compile into it (`mac_bundle_plan`).
    let document_files = |document: &std::path::Path, into: &str| {
        icon::icon_document(document)
            .map_err(|message| failed(format!("the icon {} {message}", document.display())))?
            .into_iter()
            .map(|file| {
                let contents = std::fs::read(document.join(&file)).map_err(|error| {
                    failed(format!(
                        "cannot read {}: {error}",
                        document.join(&file).display()
                    ))
                })?;
                Ok(GeneratedFile {
                    path: PathBuf::from(into).join(file),
                    contents,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()
    };
    // The icons `WindowSet(icon:)` switches to, under their asset names, as
    // the SwiftUI target ships them: an Icon Composer document for
    // `actool`, an `.icns` or a PNG in `Contents/Resources`. The runtime
    // shows them by name, which the system draws in its shape.
    for (name, source) in icon::alternates(app, "macos", &context.project_dir).map_err(failed)? {
        match source {
            icon::Source::Native(path) if icon::is_icon_document(&path) => {
                files.extend(document_files(
                    &path,
                    &format!("{}.icon", alternate_icon(&name)),
                )?);
            }
            icon::Source::Native(path) | icon::Source::Png(path) => {
                let extension = if icon::is_png_name(&path.display().to_string()) {
                    "png"
                } else {
                    "icns"
                };
                files.push(GeneratedFile {
                    path: contents.join(format!("Resources/{}.{extension}", alternate_icon(&name))),
                    contents: std::fs::read(&path).map_err(|error| {
                        failed(format!("cannot read the icon {}: {error}", path.display()))
                    })?,
                });
            }
            icon::Source::Logo => {}
        }
    }
    if let Some(document) = icon_document(app, context) {
        files.extend(document_files(&document, ICON_DOCUMENT)?);
        return Ok(files);
    }
    let icon = match icon::source(app, "macos", &context.project_dir) {
        icon::Source::Native(path) => std::fs::read(&path)
            .map_err(|error| failed(format!("cannot read the icon {}: {error}", path.display())))?,
        _ => icon::icns(
            &icon::images(app, "macos", &context.project_dir, icon::Shape::Masked)
                .map_err(failed)?,
        ),
    };
    files.push(GeneratedFile {
        path: contents.join("Resources/AppIcon.icns"),
        contents: icon,
    });
    Ok(files)
}

/// The Windows programs' resources, linked into each by the application
/// crate's build script: the application icon, which GPUI gives every
/// window as resource 1 and Windows shows for the executable, and the
/// version, whose description Windows shows as the process's name.
fn windows_resource_files(
    app: &Application,
    context: &BuildContext,
) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
    use bern_target_api::icon;
    let failed = |message: String| vec![Diagnostic::error(message)];
    let ico = match icon::source(app, "windows", &context.project_dir) {
        icon::Source::Native(path) => std::fs::read(&path)
            .map_err(|error| failed(format!("cannot read the icon {}: {error}", path.display())))?,
        _ => icon::ico(
            &icon::images(app, "windows", &context.project_dir, icon::Shape::AsDrawn)
                .map_err(failed)?,
        ),
    };
    Ok(vec![
        GeneratedFile {
            path: "app/app.ico".into(),
            contents: ico,
        },
        GeneratedFile::text("app/app.rc", glue::resource_script(app)),
        GeneratedFile::text("app/build.rs", glue::BUILD_RS.to_string()),
    ])
}

impl Gpui {
    fn id(&self) -> &'static str {
        match self.platform {
            "macos" => "gpui-macos",
            "windows" => "gpui-windows",
            _ => "gpui-linux",
        }
    }

    fn platforms(&self) -> &'static [&'static str] {
        match self.platform {
            "macos" => &["macos"],
            "windows" => &["windows"],
            _ => &["linux"],
        }
    }

    /// A program's file name on this platform.
    fn program(&self, name: &str) -> String {
        if self.platform == "windows" {
            format!("{name}.exe")
        } else {
            name.to_string()
        }
    }
}

impl Target for Gpui {
    fn build_hosts(&self) -> &'static [&'static str] {
        self.platforms()
    }

    fn descriptor(&self) -> TargetDescriptor {
        TargetDescriptor {
            id: self.id(),
            ui: "gpui",
            platforms: self.platforms(),
            version: env!("CARGO_PKG_VERSION"),
            ir_versions: IR_VERSION..=IR_VERSION,
            color_source: ColorSource::BernTheme,
            components: rust::COMPONENT_NAMES,
            windows: WindowCapabilities {
                native: true,
                virtual_windows: false,
                floating: true,
            },
            channels: &["ipc"],
            // Built-in native media executor per platform; an application-supplied
            // media kernel still claims priority at startup when registered.
            media: Some(MediaSupport {
                contract_versions: bern_media::MEDIA_CONTRACT_VERSION
                    ..=bern_media::MEDIA_CONTRACT_VERSION,
                backend: match self.platform {
                    "macos" => "avfoundation",
                    "windows" => "media-foundation",
                    _ => "gstreamer",
                },
                backend_version: "1",
                presenter: "gpui-surface",
                presenter_version: "1",
                background_playback: true,
                multi_view: true,
            }),
            styles: STYLES,
            // Pictures turn (the generator warns of other elements);
            // scroll effects show as the change at once.
            motion: &[
                "appear",
                "appear_delay",
                "disappear",
                "reflow",
                "hero",
                "scroll_scale",
                "scroll_opacity",
                "scroll_origin_x",
                "scroll_origin_y",
                "animate",
                "nudge",
                "nudge_key",
                "nudge_scale",
                "nudge_motion",
                "reduce_motion",
                "stagger",
                "spin",
                "spin_period",
                "blur",
                "glow",
                "glow_opacity",
                "idle_area",
            ],
            window_chrome: true,
            text_marks: true,
            dynamic_items: true,
            features: if self.platform == "macos" {
                FEATURES
            } else {
                // `WEB_CONTENT_MODE` is the final entry in `FEATURES` and is
                // supported natively via WebKit's `preferredContentMode` on macOS.
                &FEATURES[..FEATURES.len() - 1]
            },
        }
    }

    fn check(&self, app: &Application, config: &TargetConfig) -> Vec<Diagnostic> {
        let app = &prepared(app, config);
        let mut diagnostics = check_common(self, app);
        for channel in &config.channels {
            if !self.descriptor().channels.contains(&channel.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}`: channel `{channel}` is not available with GPUI",
                    config.name
                )));
            }
        }
        diagnostics.extend(Generator::new(app, self.platform).check());
        diagnostics
    }

    fn generate(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<Vec<GeneratedFile>, Vec<Diagnostic>> {
        if matches!(context.framework, bern_target_api::Framework::Registry(_)) {
            return Err(vec![Diagnostic::error(
                "the GPUI runtime requires a Bern path or Git checkout with its patched desktop GPUI dependencies",
            )]);
        }
        let app = &prepared(app, &context.target);
        bern_target_api::svg::validate(app, context)?;
        let generator = Generator::with_project_dir(app, self.platform, &context.project_dir);
        let ui = generator.generate(
            app.protocol.fingerprint(),
            &bern_target_api::glue::descriptor_expr(context),
        );
        let mut files = vec![
            GeneratedFile::text(
                "app/Cargo.toml",
                glue::cargo_toml(context, app, self.platform),
            ),
            GeneratedFile::text("app/src/main.rs", glue::main_rs(context, app)),
            GeneratedFile::text("app/src/ui.rs", ui),
            GeneratedFile::text("app/src/cli.rs", glue::cli_rs(context, app)),
        ];
        bern_target_api::script::configure(&mut files, context);
        // The workspace's versions of the shared crates, so the application
        // builds against what Bern was tested with.
        let lock = context.workspace_dir.join("Cargo.lock");
        if let Ok(contents) = std::fs::read(&lock)
            && !context.output_dir.join("app/Cargo.lock").exists()
        {
            files.push(GeneratedFile {
                path: "app/Cargo.lock".into(),
                contents,
            });
        }
        let manifest = serde_json::json!({
            "target": self.descriptor().id,
            "ui": "gpui",
            "platform": self.platform,
            "ir_version": app.ir_version,
            "generator": "bern-backend-gpui",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "architecture": std::env::consts::ARCH,
            "profile": if context.release { "release" } else { "debug" },
            "engine_deployment": "in-process",
            "ui_bridge": "rust",
            "channels": context.target.channels,
            "color_source": "bern_theme",
            "protocol": app.protocol.namespace,
            "protocol_fingerprint": format!("{:016x}", app.protocol.fingerprint()),
            "languages": app.languages.supported,
            "resources": app.resources.iter().map(|resource| &resource.name).collect::<Vec<_>>(),
            "product": product_name(app),
        });
        files.push(GeneratedFile::text(
            "build.json",
            serde_json::to_string_pretty(&manifest).expect("manifest"),
        ));
        files.extend(bern_target_api::svg::static_files(app, context)?);
        if let Some(name) = bern_target_api::icon::status_image(app) {
            let path = bern_target_api::svg::resource_path(context, name);
            for side in STATUS_SIDES {
                files.push(GeneratedFile {
                    path: format!("app/src/status/{side}.png").into(),
                    contents: bern_target_api::icon::status_picture(
                        &path,
                        side,
                        bern_target_api::icon::ON_LIGHT,
                    )
                    .map_err(|message| {
                        vec![Diagnostic::error(format!("StatusItem image: {message}"))]
                    })?,
                });
            }
        }
        if self.platform == "macos" {
            files.extend(mac_bundle_files(app, context)?);
        }
        if self.platform == "windows" {
            files.extend(windows_resource_files(app, context)?);
        }
        Ok(files)
    }

    /// Build the application crate, then lay the product out as every
    /// target does: the program in `<Product>/` with its resources, the
    /// command line in `bin/`, the agent skill in
    /// `skills/`.
    fn plan(
        &self,
        app: &Application,
        context: &BuildContext,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &prepared(app, &context.target);
        let executable = bern_target_api::glue::program(app);
        let output = &context.output_dir;
        let bundle = output.join(product_name(app));
        let profile = if context.release { "release" } else { "debug" };
        let target_dir = context.workspace_dir.join("target/bern-glue-gpui");
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
        let program = self.program(&executable);
        let build = BuildStep::Run {
            description: "Build the GPUI application, the engine and the entries".into(),
            program: "cargo".into(),
            args: cargo,
            cwd: output.join("app"),
            env: vec![],
        };
        let shipped: Vec<(PathBuf, PathBuf)> = app
            .resources
            .iter()
            .map(|resource| {
                (
                    bern_target_api::svg::resource_path(context, &resource.name),
                    PathBuf::from(&resource.name),
                )
            })
            .chain(
                icon_pictures(app, self.platform)
                    .into_iter()
                    .map(|(shipped, declared)| {
                        (context.project_dir.join(declared), PathBuf::from(shipped))
                    }),
            )
            .collect();
        if self.platform == "macos" {
            return Ok(mac_bundle_plan(
                app, context, build, shipped, &built, &program,
            ));
        }
        let mut steps = vec![
            build,
            BuildStep::CreateDir {
                path: bundle.clone(),
            },
            // Declared resources ship next to the program, which finds them
            // relative to itself.
            BuildStep::Mirror {
                dir: bundle.join("resources"),
                files: shipped,
                keep: vec![],
            },
            BuildStep::Copy {
                from: built.join(self.program(&format!("{executable}-ui"))),
                to: bundle.join(&program),
            },
            BuildStep::program_dir(output.join("bin"), [&program]),
            BuildStep::Copy {
                from: built.join(&program),
                to: output.join("bin").join(&program),
            },
        ];
        // The agent skill beside `bin/`, where the app listens.
        let listens = skill::listens(&context.target.channels);
        let place = listens.then(|| skill::Place::beside_bin(app, self.platform == "windows"));
        steps.extend(skill::steps(app, output, place.as_ref()));
        let mut products = vec![bundle.join(&program), output.join("bin").join(&program)];
        if listens {
            products.push(output.join(skill::path(app)));
        }
        Ok(BuildPlan {
            steps,
            notes: Vec::new(),
            run: Some(Run::Program(bundle.join(&program))),
            products,
        })
    }

    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &prepared(app, &context.target);
        match self.platform {
            "macos" => package_macos(app, context, signing),
            "windows" => package_windows(app, context, signing),
            _ => package_linux(app, context),
        }
    }

    fn supports_prebuilt_eject(&self) -> bool {
        false
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
        if options.engine == EjectEngine::Prebuilt {
            return Err(vec![Diagnostic::error(
                "the GPUI frontend is native Rust source code and always requires Cargo; eject with `--engine source`",
            )]);
        }
        if matches!(context.framework, bern_target_api::Framework::Registry(_)) {
            return Err(vec![Diagnostic::error(
                "the GPUI runtime requires a Bern path or Git checkout with its patched desktop GPUI dependencies",
            )]);
        }
        let context = &sanitized_context(context, options);
        let app = &prepared(app, &context.target);
        bern_target_api::svg::validate(app, context)?;
        let product = product_name(app);
        let exe = bern_target_api::glue::program(app);

        let mut files: Vec<GeneratedFile> = vec![
            GeneratedFile::text(
                "Cargo.toml",
                glue::cargo_toml_for(context, app, self.platform, options.with_cli, true),
            ),
            GeneratedFile::text("src/main.rs", glue::main_rs_for(context, app, 0, true)),
        ];
        if options.with_cli {
            files.push(GeneratedFile::text(
                "src/cli.rs",
                glue::cli_rs(context, app),
            ));
        }

        let generator = Generator::with_project_dir(app, self.platform, &context.project_dir);
        for (rel, content) in
            generator.generate_split(0, &bern_target_api::glue::descriptor_expr(context))
        {
            files.push(GeneratedFile::text(
                PathBuf::from("src/ui").join(rel),
                content,
            ));
        }

        // Declared resources, SVG static files, and GPUI runtime icon pictures.
        files.extend(copy_declared_resources(
            app,
            context,
            std::path::Path::new("resources"),
        )?);
        for svg_file in bern_target_api::svg::static_files(app, context)? {
            files.push(GeneratedFile {
                path: PathBuf::from("resources").join(svg_file.path),
                contents: svg_file.contents,
            });
        }
        if let Some(name) = bern_target_api::icon::status_image(app) {
            let path = bern_target_api::svg::resource_path(context, name);
            for side in STATUS_SIDES {
                files.push(GeneratedFile {
                    path: format!("src/ui/status/{side}.png").into(),
                    contents: bern_target_api::icon::status_picture(
                        &path,
                        side,
                        bern_target_api::icon::ON_LIGHT,
                    )
                    .map_err(|message| {
                        vec![Diagnostic::error(format!("StatusItem image: {message}"))]
                    })?,
                });
            }
        }
        for (shipped, declared) in icon_pictures(app, self.platform) {
            let src = context.project_dir.join(&declared);
            let contents = std::fs::read(&src).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot read icon `{}` ({}): {error}",
                    declared,
                    src.display()
                ))]
            })?;
            files.push(GeneratedFile {
                path: PathBuf::from("resources").join(shipped),
                contents,
            });
        }

        // Platform packaging metadata.
        if self.platform == "macos" {
            for file in mac_bundle_files(app, context)? {
                files.push(GeneratedFile {
                    path: PathBuf::from("assets/macos").join(file.path),
                    contents: file.contents,
                });
            }
        }
        if self.platform == "windows" {
            for file in windows_resource_files(app, context)? {
                let rel = file
                    .path
                    .strip_prefix("app")
                    .unwrap_or(&file.path)
                    .to_path_buf();
                files.push(GeneratedFile {
                    path: rel,
                    contents: file.contents,
                });
            }
        }

        if options.with_cli && skill::listens(&context.target.channels) {
            let cli_name = format!("{exe}-cli");
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

        bern_target_api::script::configure(&mut files, context);

        let app_title = app.app.name.get(&app.languages.default).unwrap_or(&product);
        let platform_label = format!("Desktop · GPUI ({})", self.platform);
        let build_help = format!(
            "Build and run directly with Cargo:\n\n```sh\ncargo run\n```\n\nFor an optimized binary, run `cargo build --release` (executable: `target/release/{exe}`)."
        );
        let layout_help = "- `src/main.rs` — Application entry point\n- `src/ui/mod.rs` — App definition, menus, and window registration\n- `src/ui/views/` — One Rust module per UI component\n- `src/ui/windows/` — One Rust module per window\n- `src/ui/styles.rs` & `src/ui/strings.rs` — Styles, motion tokens, icons, and strings\n- `resources/` — Bundled application resources\n- `crates/` & `vendor/bern/` — Self-contained application engine and patched GPUI runtime";
        files.push(GeneratedFile::text(
            "README.md",
            readme(
                app_title,
                &platform_label,
                EjectEngine::Source,
                &[
                    ("Building & Running", &build_help),
                    ("Project Layout", layout_help),
                ],
            ),
        ));

        let files = files.into_iter().map(clean_generated_file).collect();
        Ok(EjectedProject {
            files,
            executables: Vec::new(),
            rust_manifests: vec![PathBuf::from("Cargo.toml")],
            rust_root: PathBuf::from("."),
            prebuilt_staging: Vec::new(),
            prebuilt_steps: Vec::new(),
            notes: vec!["Run `cargo run` inside the ejected folder to build and run.".into()],
        })
    }
}

fn package_macos(
    app: &Application,
    context: &BuildContext,
    signing: &Signing,
) -> Result<BuildPlan, Vec<Diagnostic>> {
    if signing.macos_notary_profile.is_some() && signing.macos_identity.is_none() {
        return Err(vec![Diagnostic::error(
            "BERN_MACOS_NOTARY_PROFILE needs BERN_MACOS_SIGNING_IDENTITY: notarization takes a Developer ID signature",
        )]);
    }
    let product = product_name(app);
    let executable = bern_target_api::glue::program(app);
    let output = &context.output_dir;
    let stage = output.join("package");
    let volume = stage.join("volume");
    let staged = volume.join(format!("{product}.app"));
    let helpers = staged.join("Contents/Helpers");
    let dist = dist_dir(context);
    let image = dist.join(format!(
        "{product}-{}-macos-{}.dmg",
        app.app.version,
        package_architecture()
    ));
    let name = app
        .app
        .name
        .get(&app.languages.default)
        .map(str::to_string)
        .unwrap_or_else(|| product.clone());
    let run = |description: &str, program: &str, args: Vec<String>| BuildStep::Run {
        description: description.into(),
        program: program.into(),
        args,
        cwd: output.clone(),
        env: vec![],
    };
    let path = |p: &std::path::Path| p.display().to_string();
    let sign = |target: &std::path::Path| -> Vec<String> {
        match &signing.macos_identity {
            Some(identity) => vec![
                "--force".into(),
                "--options".into(),
                "runtime".into(),
                "--timestamp".into(),
                "--sign".into(),
                identity.clone(),
                path(target),
            ],
            None => vec!["--force".into(), "--sign".into(), "-".into(), path(target)],
        }
    };
    let mut steps = vec![
        BuildStep::RemoveIfExists {
            path: stage.clone(),
        },
        BuildStep::CreateDir {
            path: volume.clone(),
        },
        BuildStep::CreateDir { path: dist.clone() },
        BuildStep::RemoveIfExists {
            path: image.clone(),
        },
        run(
            "Copy the app",
            "ditto",
            vec![path(&output.join(format!("{product}.app"))), path(&staged)],
        ),
        run(
            "Sign the command line",
            "codesign",
            sign(&helpers.join(&executable)),
        ),
        run("Sign the app", "codesign", sign(&staged)),
        run(
            "Link Applications",
            "ln",
            vec![
                "-s".into(),
                "/Applications".into(),
                path(&volume.join("Applications")),
            ],
        ),
    ];
    let diskutil = std::process::Command::new("diskutil")
        .args(["image", "create", "from", "--help"])
        .output()
        .is_ok_and(|o| o.status.success());
    if diskutil {
        steps.push(run(
            "Make the disk image",
            "diskutil",
            vec![
                "image".into(),
                "create".into(),
                "from".into(),
                "--format".into(),
                "ULMO".into(),
                "--volumeName".into(),
                name,
                path(&volume),
                path(&image),
            ],
        ));
    } else {
        steps.push(run(
            "Make the disk image",
            "hdiutil",
            vec![
                "create".into(),
                "-volname".into(),
                name,
                "-srcfolder".into(),
                path(&volume),
                "-ov".into(),
                "-format".into(),
                "ULMO".into(),
                path(&image),
            ],
        ));
    }
    let mut notes = Vec::new();
    match (&signing.macos_identity, &signing.macos_notary_profile) {
        (Some(identity), profile) => {
            steps.push(run(
                "Sign the disk image",
                "codesign",
                vec![
                    "--force".into(),
                    "--timestamp".into(),
                    "--sign".into(),
                    identity.clone(),
                    path(&image),
                ],
            ));
            if let Some(profile) = profile {
                steps.push(run(
                    "Notarize the disk image",
                    "xcrun",
                    vec![
                        "notarytool".into(),
                        "submit".into(),
                        path(&image),
                        "--keychain-profile".into(),
                        profile.clone(),
                        "--wait".into(),
                    ],
                ));
                steps.push(run(
                    "Staple the notarization",
                    "xcrun",
                    vec!["stapler".into(), "staple".into(), path(&image)],
                ));
            } else {
                notes.push(
                    "the disk image is signed but not notarized; Gatekeeper on other Macs warns about it (set BERN_MACOS_NOTARY_PROFILE)"
                        .into(),
                );
            }
        }
        (None, _) => notes.push(
            "the app is signed for this Mac only; Gatekeeper on other Macs refuses it (set BERN_MACOS_SIGNING_IDENTITY to a Developer ID Application identity)"
                .into(),
        ),
    }
    Ok(BuildPlan {
        steps,
        products: vec![image],
        run: None,
        notes,
    })
}

fn package_windows(
    app: &Application,
    context: &BuildContext,
    signing: &Signing,
) -> Result<BuildPlan, Vec<Diagnostic>> {
    let product = product_name(app);
    let executable = format!("{}.exe", bern_target_api::glue::program(app));
    let output = &context.output_dir;
    let bundle = output.join(&product);
    let stage = output.join("package");
    let staged = stage.join(&product);
    let dist = dist_dir(context);
    let mut files = files_below(&bundle).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read {}: {error}",
            bundle.display()
        ))]
    })?;
    files.push((
        output.join("bin").join(&executable),
        PathBuf::from("bin").join(&executable),
    ));
    if skill::listens(&context.target.channels) {
        files.push((output.join(skill::path(app)), skill::path(app)));
    }
    let (zip_step, zip) = archive(
        &staged,
        &dist.join(format!(
            "{product}-{}-windows-{}",
            app.app.version,
            package_architecture()
        )),
    );
    let mut steps = vec![
        BuildStep::RemoveIfExists {
            path: stage.clone(),
        },
        BuildStep::CreateDir { path: dist.clone() },
        BuildStep::Mirror {
            dir: staged.clone(),
            files: files.clone(),
            keep: vec![],
        },
        BuildStep::RemoveIfExists { path: zip.clone() },
    ];
    let mut notes = Vec::new();
    if let Some(cert) = &signing.windows_certificate {
        for (_, rel) in &files {
            if rel.extension().is_some_and(|ext| ext == "exe") {
                let mut args = vec![
                    "sign".into(),
                    "/fd".into(),
                    "SHA256".into(),
                    "/tr".into(),
                    "http://timestamp.digicert.com".into(),
                    "/td".into(),
                    "SHA256".into(),
                    "/f".into(),
                    cert.clone(),
                ];
                if let Some(password) = &signing.windows_certificate_password {
                    args.push("/p".into());
                    args.push(password.clone());
                }
                args.push(staged.join(rel).display().to_string());
                steps.push(BuildStep::Run {
                    description: format!("Sign {}", rel.display()),
                    program: "signtool".into(),
                    args,
                    cwd: stage.clone(),
                    env: vec![],
                });
            }
        }
    } else {
        notes.push(
            "the packaged executables are not signed: SmartScreen warns about them (set BERN_WINDOWS_CERTIFICATE)"
                .into(),
        );
    }
    steps.push(zip_step);
    Ok(BuildPlan {
        steps,
        products: vec![zip],
        run: None,
        notes,
    })
}

fn package_linux(app: &Application, context: &BuildContext) -> Result<BuildPlan, Vec<Diagnostic>> {
    let product = product_name(app);
    let executable = bern_target_api::glue::program(app);
    let output = &context.output_dir;
    let bundle = output.join(&product);
    let stage = output.join("package");
    let portable = stage.join(&product);
    let dist = dist_dir(context);
    let id = &app.app.bundle_id;
    let mut files = files_below(&bundle).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read {}: {error}",
            bundle.display()
        ))]
    })?;
    files.push((
        output.join("bin").join(&executable),
        PathBuf::from("bin").join(&executable),
    ));
    if skill::listens(&context.target.channels) {
        files.push((output.join(skill::path(app)), skill::path(app)));
    }
    let (archive_step, archive_file) = archive(
        &portable,
        &dist.join(format!(
            "{product}-{}-linux-{}",
            app.app.version,
            package_architecture()
        )),
    );
    let mut steps = vec![
        BuildStep::RemoveIfExists {
            path: stage.clone(),
        },
        BuildStep::CreateDir { path: dist.clone() },
        BuildStep::Mirror {
            dir: portable,
            files: files.clone(),
            keep: vec![],
        },
        BuildStep::RemoveIfExists {
            path: archive_file.clone(),
        },
        archive_step,
    ];
    let mut products = vec![archive_file];
    let mut notes = Vec::new();
    let dpkg = std::process::Command::new("dpkg-deb")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if dpkg {
        let root = stage.join("deb");
        let installed = PathBuf::from("/opt").join(id);
        let deb_arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "amd64",
            other => other,
        };
        let deb = dist.join(format!(
            "{product}-{}-linux-{}.deb",
            app.app.version,
            package_architecture()
        ));
        let installed_size: u64 = files
            .iter()
            .filter_map(|(source, _)| std::fs::metadata(source).ok())
            .map(|m| m.len())
            .sum();
        let app_name = app.app.name.get(&app.languages.default).unwrap_or(&product);
        let desktop_content = format!(
            "[Desktop Entry]\nType=Application\nName={app_name}\nExec={} %U\nIcon={id}\nTerminal=false\nCategories=Utility;\nStartupWMClass={id}\n",
            installed.join(&executable).display()
        );
        let pkg_name: String = id
            .to_ascii_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let control = format!(
            "Package: {pkg_name}\nVersion: {}\nSection: utils\nPriority: optional\nArchitecture: {deb_arch}\nInstalled-Size: {}\nMaintainer: Bern\nDescription: {app_name}\n",
            app.app.version,
            installed_size.div_ceil(1024).max(1),
        );
        steps.extend([
            BuildStep::Mirror {
                dir: root.join(installed.strip_prefix("/").expect("absolute")),
                files,
                keep: vec![],
            },
            BuildStep::Write {
                path: root
                    .join("usr/share/applications")
                    .join(format!("{id}.desktop")),
                contents: desktop_content.into_bytes(),
            },
            BuildStep::CreateDir {
                path: root.join("usr/bin"),
            },
            BuildStep::Run {
                description: "Link the launcher".into(),
                program: "ln".into(),
                args: vec![
                    "-s".into(),
                    installed.join(&executable).display().to_string(),
                    root.join("usr/bin").join(&executable).display().to_string(),
                ],
                cwd: stage.clone(),
                env: vec![],
            },
            BuildStep::Write {
                path: root.join("DEBIAN/control"),
                contents: control.into_bytes(),
            },
            BuildStep::RemoveIfExists { path: deb.clone() },
            BuildStep::Run {
                description: "Make the Debian package".into(),
                program: "dpkg-deb".into(),
                args: vec![
                    "--root-owner-group".into(),
                    "--build".into(),
                    root.display().to_string(),
                    deb.display().to_string(),
                ],
                cwd: stage.clone(),
                env: vec![],
            },
        ]);
        products.insert(0, deb);
    } else {
        notes.push(
            "dpkg-deb is not installed here, so only the .tar.gz archive was generated".into(),
        );
    }
    Ok(BuildPlan {
        steps,
        products,
        run: None,
        notes,
    })
}
