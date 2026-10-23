//! Shared animated SVG assets and runtime templates for all presenters.
use crate::{BuildContext, Diagnostic};
use bern_ir::{Application, Node};

pub fn used(app: &Application) -> bool {
    let mut used = false;
    for node in app
        .components
        .iter()
        .map(|c| &c.body)
        .chain(app.windows.windows.iter().map(|w| &w.content))
    {
        node.walk(&mut |node| {
            if matches!(node, Node::Element(e) if e.kind == "Svg") {
                used = true;
            }
        });
    }
    used
}

pub fn validate(app: &Application, context: &BuildContext) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for node in app
        .components
        .iter()
        .map(|c| &c.body)
        .chain(app.windows.windows.iter().map(|w| &w.content))
    {
        node.walk(&mut |node| {
            if let Node::Element(e) = node
                && e.kind == "Svg"
                && let Some(name) = e.resource("source")
                && seen.insert(name.to_owned())
            {
                let result = std::fs::read_to_string(context.project_dir.join(name))
                    .map_err(|e| e.to_string())
                    .and_then(|source| bern_svg::root(&source).map(|_| ()));
                if let Err(error) = result {
                    errors.push(
                        Diagnostic::error(format!("SVG `{name}`: {error}")).at(e.source.clone()),
                    );
                }
            }
        });
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Templates use JSON-compatible quoted strings; Kotlin additionally
/// needs dollar signs escaped to preserve JavaScript template literals.
pub fn template(source: &str, kotlin: bool) -> String {
    let quote = |s: &str| {
        let quoted = serde_json::to_string(s).unwrap();
        if kotlin {
            quoted.replace('$', "\\$")
        } else {
            quoted
        }
    };
    source
        .replace("/*bern:svg-prefix*/", &quote(bern_svg::PREFIX))
        .replace("/*bern:svg-suffix*/", &quote(&bern_svg::suffix()))
}

pub fn cpp_template(source: &str) -> String {
    source
        .replace(
            "/*bern:svg-prefix*/",
            &format!("R\"BERNSVG({})BERNSVG\"", bern_svg::PREFIX),
        )
        .replace(
            "/*bern:svg-suffix*/",
            &format!("R\"BERNSVG({})BERNSVG\"", bern_svg::suffix()),
        )
}

const STATIC_PREFIX: &str = "__bern_static_svg/";

/// Target lowering is shared by checking, generation, and packaging.
pub fn prepare(app: &Application, config: &crate::TargetConfig) -> Application {
    let mut app = app.with_webview(config.has_webview());
    if config.platform == "tvos" {
        app = app.on_television();
    }
    if config.has_webview() {
        return app;
    }
    let mut resources = std::collections::BTreeMap::new();
    for node in app
        .components
        .iter_mut()
        .map(|c| &mut c.body)
        .chain(app.windows.windows.iter_mut().map(|w| &mut w.content))
    {
        node.elements_mut(&mut |e| {
            if e.kind != "Svg" {
                return;
            }
            let Some(source) = e.resource("static_source").or_else(|| e.resource("source")) else {
                return;
            };
            let name = format!("{STATIC_PREFIX}{source}.png");
            resources.insert(name.clone(), e.source.clone());
            e.kind = "Image".into();
            for prop in [
                "static_source",
                "playing",
                "speed",
                "restart",
                "interactive",
            ] {
                e.props.remove(prop);
            }
            e.props
                .insert("source".into(), bern_ir::Prop::Resource { name });
        });
    }
    for (name, declared) in resources {
        if !app.resources.iter().any(|r| r.name == name) {
            app.resources.push(bern_ir::ResourceDef { name, declared });
        }
    }
    app
}

pub fn resource_path(context: &BuildContext, name: &str) -> std::path::PathBuf {
    if name.starts_with(STATIC_PREFIX) {
        context.output_dir.join(name)
    } else {
        context.project_dir.join(name)
    }
}

/// Rasterization runs in the build tool, never in the shipped app.
/// `static_source` supplies a deliberate final pose for animated artwork.
pub fn static_files(
    app: &Application,
    context: &BuildContext,
) -> Result<Vec<crate::GeneratedFile>, Vec<Diagnostic>> {
    let mut files = Vec::new();
    for resource in &app.resources {
        let Some(source) = resource
            .name
            .strip_prefix(STATIC_PREFIX)
            .and_then(|s| s.strip_suffix(".png"))
        else {
            continue;
        };
        let render = || -> Result<Vec<u8>, String> {
            let path = context.project_dir.join(source);
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            let mut options = resvg::usvg::Options {
                resources_dir: path.parent().map(|p| p.to_owned()),
                ..Default::default()
            };
            options.fontdb_mut().load_system_fonts();
            let tree = resvg::usvg::Tree::from_data(&data, &options).map_err(|e| e.to_string())?;
            // Two pixels per intrinsic unit; cap each edge at 4096.
            let size = tree.size();
            let scale = 2.0_f32.min(4096.0 / size.width().max(size.height()));
            let mut pixmap = resvg::tiny_skia::Pixmap::new(
                (size.width() * scale).ceil().max(1.0) as u32,
                (size.height() * scale).ceil().max(1.0) as u32,
            )
            .ok_or("cannot allocate SVG raster")?;
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            pixmap.encode_png().map_err(|e| e.to_string())
        };
        files.push(crate::GeneratedFile {
            path: resource.name.clone().into(),
            contents: render().map_err(|e| {
                vec![
                    Diagnostic::error(format!("static SVG `{source}`: {e}"))
                        .at(resource.declared.clone()),
                ]
            })?,
        });
    }
    Ok(files)
}
