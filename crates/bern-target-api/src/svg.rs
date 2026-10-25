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
/// The picture of a document in the dark appearance ends so, beside the
/// one in the light appearance that ends in `.png`.
const DARK_SUFFIX: &str = ".dark.png";

/// Target lowering is shared by checking, generation, and packaging. A
/// document engine is heavy: every `Svg` whose document neither moves nor
/// runs becomes a picture of it, drawn when the app is built (one in the
/// color of the text around it as a template; one with colors per
/// appearance as two, the dark one in the `Image`'s own `dark_source`,
/// which each frontend shows where the image is dark), and the others do
/// too where the target has no WebView.
pub fn prepare(app: &Application, config: &crate::TargetConfig) -> Application {
    let mut app = app.with_webview(config.has_webview());
    if config.platform == "tvos" {
        app = app.on_television();
    }
    let pictures: std::collections::BTreeMap<String, bern_ir::SvgPicture> = app
        .resources
        .iter()
        .filter_map(|resource| Some((resource.name.clone(), resource.picture?)))
        .collect();
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
            let picture = e
                .resource("source")
                .and_then(|source| pictures.get(source))
                .copied();
            if picture.is_none() && config.has_webview() {
                return;
            }
            let Some(source) = (match picture {
                Some(_) => e.resource("source"),
                None => e.resource("static_source").or_else(|| e.resource("source")),
            }) else {
                return;
            };
            let name = format!("{STATIC_PREFIX}{source}.png");
            resources.insert(name.clone(), e.source.clone());
            let dark = (picture == Some(bern_ir::SvgPicture::Themed))
                .then(|| format!("{STATIC_PREFIX}{source}{DARK_SUFFIX}"));
            if let Some(dark) = &dark {
                resources.insert(dark.clone(), e.source.clone());
            }
            e.kind = "Image".into();
            for prop in [
                "static_source",
                "playing",
                "speed",
                "restart",
                "interactive",
                "data",
                "external_links",
            ] {
                e.props.remove(prop);
            }
            e.props
                .insert("source".into(), bern_ir::Prop::Resource { name });
            if picture == Some(bern_ir::SvgPicture::Tinted) {
                e.props.insert(
                    "rendering".into(),
                    bern_ir::Prop::Keyword {
                        value: "template".into(),
                    },
                );
            }
            if let Some(name) = dark {
                e.props
                    .insert("dark_source".into(), bern_ir::Prop::Resource { name });
            }
        });
    }
    for (name, declared) in resources {
        if !app.resources.iter().any(|r| r.name == name) {
            app.resources.push(bern_ir::ResourceDef {
                name,
                declared,
                picture: None,
            });
        }
    }
    app
}

/// Whether an `Image` shows a declared picture at one size: every side it
/// gives is a number, and it does not fly to another (`hero`). Such a
/// picture is double-sampled where it shows: decoded at twice the physical
/// pixels it covers, which the compositor halves. Squeezed down from a much
/// larger picture by the compositor alone, its edges would break up.
pub fn one_size(element: &bern_ir::Element) -> bool {
    element.resource("source").is_some()
        && element.keyword("hero").is_none()
        && (element.prop("width").is_some() || element.prop("height").is_some())
        && ["width", "height"]
            .iter()
            .all(|side| element.prop(side).is_none() || element.number(side).is_some())
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
    // The largest side each picture is declared at, in points.
    let mut declared = std::collections::BTreeMap::<String, f64>::new();
    for node in app
        .components
        .iter()
        .map(|c| &c.body)
        .chain(app.windows.windows.iter().map(|w| &w.content))
    {
        node.walk(&mut |node| {
            let Node::Element(e) = node else { return };
            if e.kind != "Image" {
                return;
            }
            for prop in ["source", "dark_source"] {
                if let Some(name) = e.resource(prop)
                    && name.starts_with(STATIC_PREFIX)
                {
                    let side = e
                        .number("width")
                        .unwrap_or(0.0)
                        .max(e.number("height").unwrap_or(0.0));
                    let largest = declared.entry(name.to_owned()).or_insert(0.0);
                    *largest = largest.max(side);
                }
            }
        });
    }
    let mut files = Vec::new();
    for resource in &app.resources {
        let Some(picture) = resource.name.strip_prefix(STATIC_PREFIX) else {
            continue;
        };
        let (source, dark) = match picture.strip_suffix(DARK_SUFFIX) {
            Some(source) => (source, true),
            None => match picture.strip_suffix(".png") {
                Some(source) => (source, false),
                None => continue,
            },
        };
        let render = || -> Result<Vec<u8>, String> {
            let path = context.project_dir.join(source);
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            // resvg follows neither media queries nor `light-dark()`: the
            // document as it shows in this appearance.
            let data = match std::str::from_utf8(&data) {
                Ok(text) => bern_svg::in_appearance(text, dark).into_bytes(),
                Err(_) => data,
            };
            let mut options = resvg::usvg::Options {
                resources_dir: path.parent().map(|p| p.to_owned()),
                ..Default::default()
            };
            options.fontdb_mut().load_system_fonts();
            let tree = resvg::usvg::Tree::from_data(&data, &options).map_err(|e| e.to_string())?;
            // As large as it shows, double-sampled on a display of three
            // pixels per point: six pixels per point of its largest
            // declared side. Without a size of its own, two pixels per
            // intrinsic unit. Each edge at most 4096.
            let size = tree.size();
            let longer = size.width().max(size.height());
            let shown = declared.get(&resource.name).copied().unwrap_or(0.0) as f32;
            let pixels = if shown > 0.0 {
                shown * 6.0
            } else {
                longer * 2.0
            };
            let scale = (pixels / longer).min(4096.0 / longer);
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
