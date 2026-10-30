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
    StyleSupport, Target, TargetConfig, TargetDescriptor, WindowCapabilities, check_common,
    feature, skill,
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
    feature::FILL_BUTTONS,
    feature::MODAL_COVER,
    feature::FORM_LAYOUT,
    feature::POPOVER_BUTTON,
    feature::SIZE_CLASSES,
    feature::SESSION_ACTIONS,
    feature::BOUND_LENGTHS,
    feature::RUNTIME_THEME,
    feature::FILE_PICK,
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
    feature::SCROLL_SNAP,
    feature::PICTURE_FILLS,
    feature::ACCORDION_LAYOUT,
    feature::IDLE_CONTROL,
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
    // Entrances show without motion here, so there is nothing to delay
    // or to grow from an edge.
    feature::DELAYED_APPEAR,
    feature::EDGE_GROW,
];

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
        base: &["background", "foreground", "corner", "shadow"],
        states: &[],
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

/// The application as this target builds it: without a WebView, every
/// `Svg` is a picture drawn when the application is built.
fn prepared(app: &Application, config: &TargetConfig) -> Application {
    let config = TargetConfig {
        webview: Some(false),
        ..config.clone()
    };
    bern_target_api::svg::prepare(app, &config)
}

pub fn product_name(app: &Application) -> String {
    bern_target_api::names::pascal(&app.app.id)
}

/// The macOS product: `<Product>.app` with the program in
/// `Contents/MacOS`, the resources in `Contents/Resources/resources`, the
/// command line and the MCP server in `Contents/Helpers` with the agent
/// skill in `Contents/Resources/skills`, so a copy moved or installed
/// anywhere carries them; `bin/` beside it links to the helpers. The
/// helpers are signed before the app, whose signature seals them.
fn mac_bundle_plan(
    app: &Application,
    context: &BuildContext,
    build: BuildStep,
    shipped: Vec<(PathBuf, PathBuf)>,
    built: &std::path::Path,
    [program, cli, mcp]: [&String; 3],
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
        BuildStep::CreateDir {
            path: helpers.clone(),
        },
    ];
    for helper in [cli, mcp] {
        steps.push(BuildStep::Copy {
            from: built.join(helper),
            to: helpers.join(helper),
        });
        steps.push(sign(
            helpers.join(helper),
            "Sign the command line tools for local use",
        ));
    }
    let listens = skill::listens(&context.target.channels);
    if listens {
        let inside = skill::Place {
            cli: format!("../../../Helpers/{cli}"),
            mcp: format!("../../../Helpers/{mcp}"),
            installed: Some(format!(
                "/Applications/{product}.app/Contents/Helpers/{cli}"
            )),
            connect: None,
        };
        steps.push(BuildStep::Write {
            path: contents.join("Resources").join(skill::path(app)),
            contents: skill::markdown(app, &inside).into_bytes(),
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
        BuildStep::CreateDir {
            path: output.join("bin"),
        },
    ]);
    for helper in [cli, mcp] {
        steps.push(BuildStep::Run {
            description: "Link the command line tools in bin/".into(),
            program: "ln".into(),
            args: vec![
                "-sf".into(),
                format!("../{product}.app/Contents/Helpers/{helper}"),
                output.join("bin").join(helper).display().to_string(),
            ],
            cwd: output.clone(),
            env: vec![],
        });
    }
    // The agent skill beside `bin/` too, where the app listens.
    let place = listens.then(|| skill::Place::beside_bin(app, false));
    steps.extend(skill::steps(app, output, place.as_ref()));
    let mut products = vec![
        bundle.clone(),
        output.join("bin").join(cli),
        output.join("bin").join(mcp),
    ];
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
    <string>AppIcon</string>
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
    let icon = match icon::source(app, "macos", &context.project_dir) {
        icon::Source::Native(path) if icon::is_icon_document(&path) => {
            return Err(failed(format!(
                "the GPUI target takes the macOS icon as a PNG or an `.icns`, not the Icon Composer document {}",
                path.display()
            )));
        }
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
            // The application's media kernel plays; views give it surfaces.
            media: Some(MediaSupport {
                contract_versions: bern_media::MEDIA_CONTRACT_VERSION
                    ..=bern_media::MEDIA_CONTRACT_VERSION,
                backend: "app-kernel",
                backend_version: "1",
                presenter: "gpui-surface",
                presenter_version: "1",
                background_playback: true,
                multi_view: true,
            }),
            styles: STYLES,
            // Pictures turn (the generator warns of other elements); other
            // motion shows as the change at once.
            motion: &["spin", "spin_period"],
            window_chrome: true,
            text_marks: false,
            dynamic_items: true,
            features: FEATURES,
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
        let app = &prepared(app, &context.target);
        bern_target_api::svg::validate(app, context)?;
        let generator = Generator::new(app, self.platform);
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
            GeneratedFile::text("app/src/mcp.rs", glue::mcp_rs(context, app)),
        ];
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
        if self.platform == "macos" {
            files.extend(mac_bundle_files(app, context)?);
        }
        Ok(files)
    }

    /// Build the application crate, then lay the product out as every
    /// target does: the program in `<Product>/` with its resources, the
    /// command line and the MCP server in `bin/`, the agent skill in
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
        let cli = self.program(&executable);
        let mcp = self.program(&format!("{executable}-mcp"));
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
                app,
                context,
                build,
                shipped,
                &built,
                [&program, &cli, &mcp],
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
            BuildStep::CreateDir {
                path: output.join("bin"),
            },
            BuildStep::Copy {
                from: built.join(&cli),
                to: output.join("bin").join(&cli),
            },
            BuildStep::Copy {
                from: built.join(&mcp),
                to: output.join("bin").join(&mcp),
            },
        ];
        // The agent skill beside `bin/`, where the app listens.
        let listens = skill::listens(&context.target.channels);
        let place = listens.then(|| skill::Place::beside_bin(app, self.platform == "windows"));
        steps.extend(skill::steps(app, output, place.as_ref()));
        let mut products = vec![
            bundle.join(&program),
            output.join("bin").join(&cli),
            output.join("bin").join(&mcp),
        ];
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
}
