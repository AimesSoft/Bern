//! The `gtk + linux` target.
//!
//! Generates a Rust application from the shared IR: GTK 4 with libadwaita,
//! following the GNOME platform's containers, rows and styles, with colors
//! from the system style manager. The engine runs in the application
//! process; the runtime (`bern-gtk`) reaches it through the same
//! in-process interface the other native targets use. GTK is a
//! requirement of this target's build only.

mod glue;
pub mod icons;
mod rust;
mod style;

use bern_ir::spec::COMPONENTS;
use bern_ir::{Application, IR_VERSION, Node};
use bern_target_api::{
    BuildContext, BuildPlan, BuildStep, ColorSource, Diagnostic, GeneratedFile, MediaSupport, Run,
    Signing, Target, TargetConfig, TargetDescriptor, WindowCapabilities, archive, check_common,
    dist_dir, feature, files_below, icon, package_architecture, skill,
};
use std::path::{Path, PathBuf};

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

/// The further parts of the IR this target builds.
const FEATURES: &[&str] = &[
    feature::BUTTON_CONTENT,
    feature::TAB_ACCESSORY,
    feature::REVEAL,
    feature::TOOLBAR_SEARCH,
    feature::SEARCH_RESULTS,
    feature::SESSION_ACTIONS,
    feature::TOGGLE_BUTTON,
    feature::DIALOG_WINDOWS,
    feature::BOUND_LENGTHS,
    feature::APPEARANCE,
    feature::SHORTCUTS,
    feature::RUNTIME_THEME,
    feature::RUNTIME_PALETTE,
    feature::RUNTIME_IMAGES,
    feature::PICTURE_FILLS,
    feature::FILE_PICK,
    feature::SCROLL_SNAP,
    feature::RAILS,
    feature::POPOVER_BUTTON,
    feature::SIZE_CLASSES,
    feature::TEXT_FORMATS,
    feature::TEXT_WEIGHTS,
    feature::TEXT_LINES,
    feature::IMAGE_CORNERS,
    feature::SIDEBAR_WIDTH,
    feature::CONTROLLED_VALUES,
    feature::SEARCH_PALETTE,
    feature::IMAGE_TEMPLATE,
    feature::ADAPTIVE_IMAGES,
    feature::SCALED_SCENE,
    feature::ACCORDION_LAYOUT,
    feature::CAPSULE_CONTROLS,
    feature::MODAL_FLOW_TIMER,
    feature::PAGE_BADGES,
    feature::IDLE_CONTROL,
    feature::FILL_BUTTONS,
    feature::GRID_CELLS,
    feature::PAGE_SEARCH,
    feature::FORM_LAYOUT,
    feature::MODAL_COVER,
    feature::WINDOW_SHORT,
    // Not applicable, so nothing to warn about: a desktop window has no
    // orientation, and where a Linux phone turns its screen, Wayland lets
    // an application ask for no orientation.
    feature::WINDOW_LANDSCAPE,
    feature::EDGE_GROW,
    feature::DELAYED_APPEAR,
    feature::TOOLBAR_LEADING,
    feature::AXIS_PADDING,
    feature::ICON_MENUS,
    feature::ICON_LABELS,
    feature::ICON_TABS,
    feature::ROW_BASELINE,
    feature::SIDEBAR_HEADER,
    feature::UNTITLED_DETAIL,
];

/// `reflow` GTK cannot show, in components and in the content written
/// directly in a window: the children of a column, row or stack and the
/// cells of a grid with `cell_ratio` slide; a flow box grid lays out its
/// own cells, so it warns there and the cells move at once.
fn reflow_warnings(app: &Application) -> Vec<Diagnostic> {
    fn slides(kind: Option<&str>) -> bool {
        matches!(kind, Some("Column" | "Row" | "Stack" | "CellGrid"))
    }
    fn visit(nodes: &[Node], container: Option<&str>, out: &mut Vec<Diagnostic>) {
        for node in nodes {
            match node {
                Node::Element(element) => {
                    let kind = match element.kind.as_str() {
                        "Grid" if element.number("cell_ratio").is_some() => "CellGrid",
                        other => other,
                    };
                    let moves = if kind == "ForEach" {
                        slides(container)
                    } else {
                        slides(Some(kind))
                    };
                    if element.keyword("reflow").is_some() && !moves {
                        out.push(
                            Diagnostic::warning(format!(
                                "target `gtk-linux` slides only the children of a `Column`, `Row` or `Stack` and the cells of a `Grid` with `cell_ratio` with `reflow`; the items of this `{}` move at once",
                                element.kind
                            ))
                            .at(element.source.clone()),
                        );
                    }
                    // Conditions and repetition keep the container.
                    let inner = if element.kind == "ForEach" {
                        container
                    } else {
                        Some(kind)
                    };
                    visit(&element.children, inner, out);
                }
                Node::If(node) => {
                    for (_, body) in &node.branches {
                        visit(body, container, out);
                    }
                    visit(&node.otherwise, container, out);
                }
                Node::IfLet(node) => {
                    visit(&node.then, container, out);
                    visit(&node.otherwise, container, out);
                }
                Node::Match(node) => {
                    for arm in &node.arms {
                        visit(&arm.body, container, out);
                    }
                }
                Node::Component(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    for component in &app.components {
        visit(std::slice::from_ref(&component.body), None, &mut out);
    }
    for window in &app.windows.windows {
        visit(std::slice::from_ref(&window.content), None, &mut out);
    }
    out
}

/// Directory and executable name of the product, e.g. `Showcase` and
/// `showcase` for app id `showcase`.
pub fn product_name(app: &Application) -> String {
    bern_target_api::names::pascal(&app.app.id)
}

/// Where the agent skill finds the programs: in `bin/` beside it, in the
/// build, the `.tar.gz` and the folder the `.deb` installs.
fn skill_place(app: &Application) -> skill::Place {
    skill::Place::beside_bin(app, false).installed(format!(
        "/opt/{}/bin/{}",
        app.app.bundle_id,
        executable(app)
    ))
}

/// The application icon beside the executable, named after the
/// application so the icon theme finds it: `icons/<bundle id>.png`.
pub fn icon_file(app: &Application) -> PathBuf {
    PathBuf::from("icons").join(format!("{}.png", app.app.bundle_id))
}

/// Where a declared icon theme folder goes beside the executable, for the
/// package to install: `icons/hicolor`.
const THEME_DIR: &str = "icons/hicolor";

/// The icons of a declared icon theme folder, each with its place in
/// `hicolor`, named after the application (`48x48/apps/<bundle id>.png`).
fn theme_files(app: &Application, dir: &Path) -> Result<Vec<(icon::ThemeIcon, PathBuf)>, String> {
    let id = &app.app.bundle_id;
    Ok(icon::theme_icons(dir)?
        .into_iter()
        .map(|one| {
            let name = if one.symbolic {
                format!("{id}-symbolic.{}", one.extension)
            } else {
                format!("{id}.{}", one.extension)
            };
            let place = one.folder.join(name);
            (one, place)
        })
        .collect())
}

fn executable(app: &Application) -> String {
    app.app.id.replace('_', "-")
}

/// Files writable only by their owner, as installed files are: 644, and
/// 755 for programs and directories.
fn normalize_permissions(dir: &std::path::Path) -> BuildStep {
    BuildStep::Run {
        description: "Set the file permissions".into(),
        program: "chmod".into(),
        args: vec![
            "-R".into(),
            "u+rwX,go+rX,go-w".into(),
            dir.display().to_string(),
        ],
        cwd: dir.parent().unwrap_or(dir).to_path_buf(),
        env: vec![],
    }
}

impl Target for GtkLinux {
    fn build_hosts(&self) -> &'static [&'static str] {
        &["linux"]
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
                "reduce_motion",
                "appear_delay",
                "stagger",
                "idle_area",
            ],
            window_chrome: true,
            text_marks: true,
            dynamic_items: true,
            features: FEATURES,
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
        let app = &bern_target_api::svg::prepare(app, config);
        let mut diagnostics = check_common(self, app);
        let framework = bern_target_api::texts::framework_languages();
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
        diagnostics.extend(bern_target_api::icon_set::check(
            app,
            "the GTK icon set",
            icons::has,
        ));
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
        diagnostics.extend(reflow_warnings(app));
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
        let executable = executable(app);
        let mut files = vec![
            GeneratedFile::text("app/Cargo.toml", glue::cargo_toml(context, app)),
            GeneratedFile::text("app/src/main.rs", glue::main_rs(context, app)),
            GeneratedFile::text(
                "app/src/ui.rs",
                Generator::new(app).generate(
                    app.protocol.fingerprint(),
                    &bern_target_api::glue::descriptor_expr(context),
                ),
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
        // directory to the icon theme and names it after the application:
        // the declared PNG or the Bern logo, or the largest PNG of a
        // declared icon theme, whose icons all go below it for the package.
        let failed = |message: String| vec![Diagnostic::error(message)];
        let read = |path: &Path| {
            std::fs::read(path)
                .map_err(|error| failed(format!("cannot read {}: {error}", path.display())))
        };
        match icon::source(app, "linux", &context.project_dir) {
            icon::Source::Native(dir) => {
                let icons = theme_files(app, &dir).map_err(|message| {
                    failed(format!("the Linux icon {} {message}", dir.display()))
                })?;
                let largest = icons
                    .iter()
                    .filter(|(one, _)| !one.symbolic)
                    .filter_map(|(one, _)| one.size.map(|size| (size, &one.path)))
                    .max_by_key(|(size, _)| *size)
                    .map(|(_, path)| path)
                    .expect("an icon theme has a PNG");
                files.push(GeneratedFile {
                    path: PathBuf::from(&product).join(icon_file(app)),
                    contents: read(&dir.join(largest))?,
                });
                for (one, place) in &icons {
                    files.push(GeneratedFile {
                        path: PathBuf::from(&product).join(THEME_DIR).join(place),
                        contents: read(&dir.join(&one.path))?,
                    });
                }
            }
            _ => {
                let images = icon::images(app, "linux", &context.project_dir, icon::Shape::AsDrawn)
                    .map_err(failed)?;
                files.push(GeneratedFile {
                    path: PathBuf::from(&product).join(icon_file(app)),
                    contents: icon::largest(&images).png.to_vec(),
                });
            }
        }
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
        let mut steps = vec![
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
                            bern_target_api::svg::resource_path(context, &resource.name),
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
        // The agent skill beside `bin/`, where the app listens.
        let listens = skill::listens(&context.target.channels);
        let place = listens.then(|| skill_place(app));
        steps.extend(skill::steps(app, output, place.as_ref()));
        let mut products = vec![
            bundle.join(&executable),
            output.join("bin").join(&executable),
            output.join("bin").join(format!("{executable}-mcp")),
        ];
        if listens {
            products.push(output.join(skill::path(app)));
        }
        Ok(BuildPlan {
            steps,
            notes: Vec::new(),
            run: Some(Run::Program(bundle.join(&executable))),
            products,
        })
    }

    /// A Debian package (with `dpkg-deb`, where the system has it) that
    /// installs into `/opt/<bundle id>` with a menu entry, an icon in the
    /// icon theme and a launcher in `/usr/bin`, and a `.tar.gz` of the same
    /// folder to run without installing. Both carry the command line and
    /// the MCP server in `bin/`, and the agent skill listing their commands
    /// in `skills/`. With `Svg`, the `.deb` also installs the
    /// AppArmor profile WebKitGTK's sandbox needs (`glue::apparmor_files`).
    fn package(
        &self,
        app: &Application,
        context: &BuildContext,
        _signing: &Signing,
    ) -> Result<BuildPlan, Vec<Diagnostic>> {
        let app = &bern_target_api::svg::prepare(app, &context.target);
        let product = product_name(app);
        let executable = executable(app);
        let output = &context.output_dir;
        let bundle = output.join(&product);
        let stage = output.join("package");
        let dist = dist_dir(context);
        let id = &app.app.bundle_id;
        let desktop = format!("{id}.desktop");
        let read = |error: std::io::Error| {
            vec![Diagnostic::error(format!(
                "cannot read {}: {error}",
                bundle.display()
            ))]
        };
        // The desktop entry beside the build points into the build; the
        // package writes its own.
        let mut files: Vec<_> = files_below(&bundle)
            .map_err(read)?
            .into_iter()
            .filter(|(_, path)| path != std::path::Path::new(&desktop))
            .collect();
        for program in [executable.clone(), format!("{executable}-mcp")] {
            files.push((
                output.join("bin").join(&program),
                PathBuf::from("bin").join(&program),
            ));
        }
        if skill::listens(&context.target.channels) {
            files.push((output.join(skill::path(app)), skill::path(app)));
        }
        let portable = stage.join(&product);
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
            normalize_permissions(&stage),
            archive_step,
        ];
        let mut products = vec![archive_file];
        let mut notes = Vec::new();
        let dpkg = std::process::Command::new("dpkg-deb")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success());
        if dpkg {
            let root = stage.join("deb");
            let installed = PathBuf::from("/opt").join(id);
            // Every icon of a declared icon theme, or the one PNG at its size.
            let icons: Vec<(PathBuf, Vec<u8>)> =
                match icon::source(app, "linux", &context.project_dir) {
                    icon::Source::Native(dir) => {
                        let mut icons = Vec::new();
                        for (_, place) in theme_files(app, &dir)
                            .map_err(|message| vec![Diagnostic::error(message)])?
                        {
                            let contents =
                                std::fs::read(bundle.join(THEME_DIR).join(&place)).map_err(read)?;
                            icons.push((place, contents));
                        }
                        icons
                    }
                    _ => {
                        let icon_bytes =
                            std::fs::read(bundle.join(icon_file(app))).map_err(read)?;
                        let size = icon::png_size(&icon_bytes).map_or(256, |(width, _)| width);
                        vec![(
                            PathBuf::from(format!("{size}x{size}/apps/{id}.png")),
                            icon_bytes,
                        )]
                    }
                };
            let architecture = match std::env::consts::ARCH {
                "aarch64" => "arm64",
                "x86_64" => "amd64",
                other => other,
            };
            let deb = dist.join(format!(
                "{executable}_{}_{architecture}.deb",
                app.app.version
            ));
            let installed_size: u64 = files
                .iter()
                .filter_map(|(source, _)| std::fs::metadata(source).ok())
                .map(|metadata| metadata.len())
                .sum();
            steps.extend([
                BuildStep::Mirror {
                    dir: root.join(installed.strip_prefix("/").expect("absolute")),
                    files,
                    keep: vec![],
                },
                BuildStep::Write {
                    path: root.join("usr/share/applications").join(&desktop),
                    contents: glue::desktop_entry(
                        app,
                        &installed.join(&executable),
                        std::path::Path::new(id),
                    )
                    .into_bytes(),
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
                    contents: glue::debian_control(
                        app,
                        &executable,
                        architecture,
                        installed_size.div_ceil(1024),
                        bern_target_api::script::webview(context),
                    )
                    .into_bytes(),
                },
            ]);
            for (place, contents) in icons {
                steps.push(BuildStep::Write {
                    path: root.join("usr/share/icons/hicolor").join(place),
                    contents,
                });
            }
            let mut programs = Vec::new();
            for file in glue::apparmor_files(
                app,
                &installed.join(&executable),
                bern_target_api::script::webview(context),
            ) {
                let path = root.join(&file.path);
                if file.program {
                    programs.push(path.display().to_string());
                }
                steps.push(BuildStep::Write {
                    path,
                    contents: file.contents.into_bytes(),
                });
            }
            steps.extend([
                BuildStep::RemoveIfExists { path: deb.clone() },
                normalize_permissions(&root),
            ]);
            if !programs.is_empty() {
                steps.push(BuildStep::Run {
                    description: "Make the maintainer scripts executable".into(),
                    program: "chmod".into(),
                    args: [vec!["755".to_string()], programs].concat(),
                    cwd: stage.clone(),
                    env: vec![],
                });
            }
            steps.push(BuildStep::Run {
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
            });
            products.insert(0, deb);
        } else {
            notes.push("dpkg-deb is not installed here, so there is no .deb; the .tar.gz runs on any Linux with GTK 4.14 and libadwaita 1.5".into());
        }
        if bern_target_api::svg::used(app) || bern_target_api::script::webview(context) {
            notes.push(format!(
                "`Svg` uses WebKitGTK, whose sandbox stops the app on Ubuntu 24.04 and later unless an AppArmor profile grants it `userns`: the .deb installs one for /opt/{id}/{executable}; the .tar.gz does not, so give the unpacked {executable} one there"
            ));
        }
        Ok(BuildPlan {
            steps,
            products,
            run: None,
            notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflow_placed_directly_in_a_window_warns() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello/Bern.toml");
        let config = bern_compiler::config::load(&manifest).unwrap();
        let mut hello = bern_compiler::compile(&config).unwrap().app;
        assert!(reflow_warnings(&hello).is_empty());
        // A flow box grid around the window's text, sliding its cells.
        let content = &mut hello.windows.windows[0].content;
        let Node::Element(text) = content.clone() else {
            panic!("{content:?}");
        };
        let mut grid = text.clone();
        grid.kind = "Grid".into();
        grid.props = [(
            "reflow".to_string(),
            bern_ir::Prop::Keyword {
                value: "snappy".into(),
            },
        )]
        .into();
        grid.children = vec![Node::Element(text)];
        *content = Node::Element(grid);
        let warnings = reflow_warnings(&hello);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0]
                .message
                .contains("the items of this `Grid` move at once"),
            "{warnings:?}"
        );
    }
}
