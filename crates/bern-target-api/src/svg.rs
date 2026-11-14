//! Shared SVG lowering, `BSVG` compilation, and static rasterization across targets.
use crate::{BuildContext, Diagnostic};
use bern_ir::{Application, Node};

/// Whether the application renders an animated or interactive `Svg`.
pub fn svg_used(app: &Application) -> bool {
    has_kind(app, |kind| kind == "Svg")
}

/// Whether the application embeds a `WebPage` (which requires a WebView).
pub fn webpage_used(app: &Application) -> bool {
    has_kind(app, |kind| kind == "WebPage")
}

/// Whether the application shows an `Svg` or a `WebPage`.
pub fn used(app: &Application) -> bool {
    has_kind(app, |kind| kind == "Svg" || kind == "WebPage")
}

fn has_kind(app: &Application, pred: impl Fn(&str) -> bool) -> bool {
    let mut found = false;
    for node in app
        .components
        .iter()
        .map(|c| &c.body)
        .chain(app.windows.windows.iter().map(|w| &w.content))
    {
        node.walk(&mut |node| {
            if let Node::Element(e) = node
                && pred(&e.kind)
            {
                found = true;
            }
        });
    }
    found
}

const STATIC_PREFIX: &str = "__bern_static_svg/";
const DARK_SUFFIX: &str = ".dark.png";
const ANIM_PREFIX: &str = "__bern_anim_svg/";
const ANIM_SUFFIX: &str = ".bsvg";

fn original_anim_source(name: &str) -> &str {
    name.strip_prefix(ANIM_PREFIX)
        .and_then(|rest| rest.strip_suffix(ANIM_SUFFIX))
        .unwrap_or(name)
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
                && let Some(raw_name) = e.resource("source")
            {
                let name = original_anim_source(raw_name);
                if seen.insert(name.to_owned()) {
                    let result = std::fs::read_to_string(context.project_dir.join(name))
                        .map_err(|e| e.to_string())
                        .and_then(|source| bern_svg::compile(&source).map(|_| ()));
                    if let Err(error) = result {
                        errors.push(
                            Diagnostic::error(format!("SVG `{name}`: {error}"))
                                .at(e.source.clone()),
                        );
                    }
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

/// Target lowering shared by checking, generation, and packaging:
/// - Static SVGs (`resource.picture.is_some()`) lower to build-time PNG `Image`s
///   (template for `currentColor`, light + dark pair for appearance-themed SVGs).
/// - Animated and interactive SVGs (`resource.picture.is_none()`) compile to
///   compact binary `BSVG` assets played natively by `bern-svg` on every target,
///   with zero WebView dependency.
pub fn prepare(app: &Application, config: &crate::TargetConfig) -> Application {
    let app = if matches!(config.platform.as_str(), "wearos" | "watchos") {
        app.on_watch()
    } else {
        app.without_watch()
    };
    let mut app = app.with_webview(config.has_webview());
    if config.platform == "tvos" {
        app = app.on_television();
    }
    let pictures: std::collections::BTreeMap<String, bern_ir::SvgPicture> = app
        .resources
        .iter()
        .filter_map(|resource| Some((resource.name.clone(), resource.picture?)))
        .collect();
    let mut generated_resources = std::collections::BTreeMap::new();
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
            let Some(source) = e.resource("source").map(str::to_owned) else {
                return;
            };
            if source.starts_with(ANIM_PREFIX) {
                return;
            }
            let picture = pictures.get(&source).copied();
            let Some(picture) = picture else {
                let anim_name = format!("{ANIM_PREFIX}{source}{ANIM_SUFFIX}");
                generated_resources.insert(anim_name.clone(), e.source.clone());
                e.props.remove("static_source");
                e.props
                    .insert("source".into(), bern_ir::Prop::Resource { name: anim_name });
                return;
            };
            let name = format!("{STATIC_PREFIX}{source}.png");
            generated_resources.insert(name.clone(), e.source.clone());
            let dark = (picture == bern_ir::SvgPicture::Themed)
                .then(|| format!("{STATIC_PREFIX}{source}{DARK_SUFFIX}"));
            if let Some(dark) = &dark {
                generated_resources.insert(dark.clone(), e.source.clone());
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
            if picture == bern_ir::SvgPicture::Tinted {
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
    for (name, declared) in generated_resources {
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

/// Whether an `Image` shows its picture at one size: it gives `width` or
/// `height` without animating them and does not fly to another (`hero`).
/// Such a picture — whether a declared resource or a `MediaSource` URL, and
/// including conditional sizes like `if window.watch { 34 } else { 44 }` —
/// is double-sampled where it shows: decoded at twice the physical pixels it
/// covers, which the compositor halves.
pub fn one_size(element: &bern_ir::Element) -> bool {
    element.keyword("hero").is_none()
        && element.prop("animate").is_none()
        && (element.prop("width").is_some() || element.prop("height").is_some())
}

/// An upper bound in logical points on the longer side an `Image` needs when
/// decoded, when `width` and `height` alone do not already fix it:
/// - a static `blur` of `radius >= 4` destroys detail finer than `1600 / radius`
///   points (clamped to `32..=256`, i.e. `128px` at `2x` for a `blur: 90`
///   window backdrop);
/// - an `aspect`-sized card inside a `Grid(min_width: M, gap: G)` is never
///   wider than `2 * M + G` points at any window width.
pub fn decode_max_side(app: &Application, target: &bern_ir::Element) -> Option<f64> {
    if target.keyword("hero").is_some() {
        return None;
    }
    let blur_cap = prop_min_number(target, "blur")
        .filter(|radius| *radius >= 4.0)
        .map(|radius| (1600.0 / radius).clamp(32.0, 256.0).ceil());
    let grid_cap = if target.prop("width").is_none() && target.prop("height").is_none() {
        prop_min_number(target, "aspect")
            .filter(|aspect| *aspect > 0.0)
            .and_then(|aspect| {
                let cell_width = enclosing_grid_max_width(app, target)?;
                Some((cell_width * (1.0_f64).max(1.0 / aspect)).ceil())
            })
    } else {
        None
    };
    match (blur_cap, grid_cap) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn expr_bounds(expr: &bern_ir::Expr) -> Option<(f64, f64)> {
    match &expr.kind {
        bern_ir::ExprKind::Literal {
            value: bern_protocol::Value::Int(n),
        } => Some((*n as f64, *n as f64)),
        bern_ir::ExprKind::Literal {
            value: bern_protocol::Value::Float(n),
        } => Some((*n, *n)),
        bern_ir::ExprKind::If {
            then, otherwise, ..
        } => {
            let (lo1, hi1) = expr_bounds(then)?;
            let (lo2, hi2) = expr_bounds(otherwise)?;
            Some((lo1.min(lo2), hi1.max(hi2)))
        }
        bern_ir::ExprKind::Corner { radius, .. } => Some((*radius, *radius)),
        _ => None,
    }
}

fn prop_min_number(element: &bern_ir::Element, name: &str) -> Option<f64> {
    match element.prop(name)? {
        bern_ir::Prop::Number { value } => Some(*value),
        bern_ir::Prop::Expr { expr } => expr_bounds(expr).map(|(lo, _)| lo),
        _ => None,
    }
}

fn prop_max_number(element: &bern_ir::Element, name: &str) -> Option<f64> {
    match element.prop(name)? {
        bern_ir::Prop::Number { value } => Some(*value),
        bern_ir::Prop::Expr { expr } => expr_bounds(expr).map(|(_, hi)| hi),
        _ => None,
    }
}

/// The maximum cell width (`2 * min_width + gap`) of the `Grid` enclosing
/// `target`, either directly in the same tree or across every call site of
/// the component containing `target`.
fn enclosing_grid_max_width(app: &Application, target: &bern_ir::Element) -> Option<f64> {
    fn grid_bound(element: &bern_ir::Element) -> Option<f64> {
        if element.kind != "Grid" {
            return None;
        }
        let min_width = prop_max_number(element, "min_width")?;
        let gap = prop_max_number(element, "gap").unwrap_or(8.0).max(0.0);
        (min_width > 0.0).then_some(2.0 * min_width + gap)
    }

    fn find_target(
        node: &Node,
        target: &bern_ir::Element,
        active_grid: Option<f64>,
    ) -> Option<Option<f64>> {
        match node {
            Node::Element(el) => {
                if std::ptr::eq(el, target) {
                    return Some(active_grid);
                }
                let next_grid = grid_bound(el).or(active_grid);
                for child in &el.children {
                    if let Some(found) = find_target(child, target, next_grid) {
                        return Some(found);
                    }
                }
                None
            }
            Node::If(node) => node
                .branches
                .iter()
                .flat_map(|(_, body)| body)
                .chain(&node.otherwise)
                .find_map(|child| find_target(child, target, active_grid)),
            Node::IfLet(node) => node
                .then
                .iter()
                .chain(&node.otherwise)
                .find_map(|child| find_target(child, target, active_grid)),
            Node::Match(node) => node
                .arms
                .iter()
                .flat_map(|arm| &arm.body)
                .find_map(|child| find_target(child, target, active_grid)),
            Node::Component(_) => None,
        }
    }

    for window in &app.windows.windows {
        if let Some(bound) = find_target(&window.content, target, None) {
            return bound;
        }
    }
    let (owner, direct_bound) = app.components.iter().find_map(|comp| {
        find_target(&comp.body, target, None).map(|bound| (comp.name.as_str(), bound))
    })?;
    if direct_bound.is_some() {
        return direct_bound;
    }

    // `target` stands in `owner` outside a local Grid: check if every root
    // path reaching `owner` across windows and components passes through a Grid.
    fn collect_calls<'a>(
        app: &'a Application,
        node: &'a Node,
        callee: &str,
        active_grid: Option<f64>,
        visited: &mut std::collections::BTreeSet<&'a str>,
        saw_any: &mut bool,
        max_bound: &mut f64,
    ) -> bool {
        match node {
            Node::Component(call) if call.component == callee => {
                *saw_any = true;
                let Some(bound) = active_grid else {
                    return false;
                };
                *max_bound = max_bound.max(bound);
                true
            }
            Node::Component(call) => {
                let Some(comp) = app.components.iter().find(|c| c.name == call.component) else {
                    return true;
                };
                if !visited.insert(comp.name.as_str()) {
                    return true;
                }
                let ok = collect_calls(
                    app,
                    &comp.body,
                    callee,
                    active_grid,
                    visited,
                    saw_any,
                    max_bound,
                );
                visited.remove(comp.name.as_str());
                ok
            }
            Node::Element(el) => {
                let next_grid = grid_bound(el).or(active_grid);
                el.children
                    .iter()
                    .all(|c| collect_calls(app, c, callee, next_grid, visited, saw_any, max_bound))
            }
            Node::If(node) => node
                .branches
                .iter()
                .flat_map(|(_, body)| body)
                .chain(&node.otherwise)
                .all(|c| collect_calls(app, c, callee, active_grid, visited, saw_any, max_bound)),
            Node::IfLet(node) => {
                node.then.iter().chain(&node.otherwise).all(|c| {
                    collect_calls(app, c, callee, active_grid, visited, saw_any, max_bound)
                })
            }
            Node::Match(node) => {
                node.arms.iter().flat_map(|arm| &arm.body).all(|c| {
                    collect_calls(app, c, callee, active_grid, visited, saw_any, max_bound)
                })
            }
        }
    }

    let mut saw_any = false;
    let mut max_bound = 0.0_f64;
    let mut visited = std::collections::BTreeSet::new();
    for window in &app.windows.windows {
        if !collect_calls(
            app,
            &window.content,
            owner,
            None,
            &mut visited,
            &mut saw_any,
            &mut max_bound,
        ) {
            return None;
        }
    }
    (saw_any && max_bound > 0.0).then_some(max_bound)
}

pub fn resource_path(context: &BuildContext, name: &str) -> std::path::PathBuf {
    if let Some(file) = name
        .strip_prefix(crate::WEB_UI_FOLDER)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        crate::web_ui_output(context).join(file)
    } else if name.starts_with(STATIC_PREFIX) || name.starts_with(ANIM_PREFIX) {
        context.output_dir.join(name)
    } else {
        context.project_dir.join(name)
    }
}

/// Build-time compilation of static SVG pictures (`.png`) and animated vector
/// scenes (`.bsvg`).
pub fn static_files(
    app: &Application,
    context: &BuildContext,
) -> Result<Vec<crate::GeneratedFile>, Vec<Diagnostic>> {
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
                    let side = prop_max_number(e, "width")
                        .unwrap_or(0.0)
                        .max(prop_max_number(e, "height").unwrap_or(0.0));
                    let largest = declared.entry(name.to_owned()).or_insert(0.0);
                    *largest = largest.max(side);
                }
            }
        });
    }
    let mut files = Vec::new();
    for resource in &app.resources {
        if let Some(rest) = resource.name.strip_prefix(ANIM_PREFIX)
            && let Some(source) = rest.strip_suffix(ANIM_SUFFIX)
        {
            let path = context.project_dir.join(source);
            let compiled = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| bern_svg::compile(&text));
            files.push(crate::GeneratedFile {
                path: resource.name.clone().into(),
                contents: compiled.map_err(|e| {
                    vec![
                        Diagnostic::error(format!("animated SVG `{source}`: {e}"))
                            .at(resource.declared.clone()),
                    ]
                })?,
            });
            continue;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use bern_ir::{Element, Expr, ExprKind, Prop, SourceLocation};
    use bern_protocol::{Type, Value};

    fn cond_int(lo: i64, hi: i64) -> Prop {
        Prop::Expr {
            expr: Expr {
                kind: ExprKind::If {
                    cond: Box::new(Expr {
                        kind: ExprKind::Literal {
                            value: Value::Bool(true),
                        },
                        ty: Type::Bool,
                    }),
                    then: Box::new(Expr {
                        kind: ExprKind::Literal {
                            value: Value::Int(lo),
                        },
                        ty: Type::Int,
                    }),
                    otherwise: Box::new(Expr {
                        kind: ExprKind::Literal {
                            value: Value::Int(hi),
                        },
                        ty: Type::Int,
                    }),
                },
                ty: Type::Int,
            },
        }
    }

    fn element(kind: &str, props: impl IntoIterator<Item = (&'static str, Prop)>) -> Element {
        Element {
            kind: kind.into(),
            id: None,
            path: kind.into(),
            props: props.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            template: None,
            children: Vec::new(),
            action: None,
            source: SourceLocation::default(),
        }
    }

    #[test]
    fn conditional_numbers_bound_sizes_and_radii() {
        let conditional_sized = element(
            "Image",
            [("width", cond_int(34, 44)), ("height", cond_int(34, 44))],
        );
        let animated = element(
            "Image",
            [
                ("width", cond_int(34, 44)),
                (
                    "animate",
                    Prop::Keyword {
                        value: "smooth".into(),
                    },
                ),
            ],
        );
        let card = element("Image", [("aspect", Prop::Number { value: 1.0 })]);
        assert!(one_size(&conditional_sized));
        assert!(!one_size(&animated));
        assert!(!one_size(&card));
        assert_eq!(prop_min_number(&conditional_sized, "width"), Some(34.0));
        assert_eq!(prop_max_number(&conditional_sized, "width"), Some(44.0));
    }
}
