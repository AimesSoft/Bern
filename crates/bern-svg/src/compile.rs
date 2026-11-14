//! Compiles `.svg` XML documents into `SceneData` and `BSVG` bytecode.

use crate::css::{
    parse_angle_deg, parse_declarations, parse_easing, parse_length, parse_paint, parse_property,
    parse_stylesheet, parse_time_seconds, parse_transform_list, parse_unit_alpha, split_top_commas,
};
use crate::model::*;
use crate::path::{build_shape, parse_svg_path, path_bbox};
use roxmltree::{Document, Node};
use std::collections::HashMap;

const SVG_NS: &str = "http://www.w3.org/2000/svg";

/// Validate root `<svg>` element and namespace, rejecting DTDs and external entities.
pub fn root(source: &str) -> Result<&str, String> {
    let doc = Document::parse(source).map_err(|e| format!("invalid SVG: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" || root.tag_name().namespace() != Some(SVG_NS) {
        return Err("expected an SVG root in the SVG namespace".into());
    }
    Ok(&source[root.range()])
}

/// Compile an `.svg` source string into a `SceneData` representation.
pub fn compile_scene(source: &str) -> Result<SceneData, String> {
    let doc = Document::parse(source).map_err(|e| format!("invalid SVG: {e}"))?;
    let svg = doc.root_element();
    if svg.tag_name().name() != "svg" || svg.tag_name().namespace() != Some(SVG_NS) {
        return Err("expected an SVG root in the SVG namespace".into());
    }

    let interaction = parse_interaction(svg)?;
    let has_declarative_interaction = svg.attributes().any(|a| a.name().starts_with("data-bern-"));

    for node in doc.descendants().filter(|n| n.is_element()) {
        let tag = node.tag_name().name();
        if matches!(tag, "foreignObject" | "iframe" | "video" | "audio") {
            return Err(format!(
                "unsupported embedded HTML/media element `<{tag}>` in native Rust SVG player"
            ));
        }
        if tag == "script" && !has_declarative_interaction {
            return Err(
                "`<script>` is not executed by Bern's native Rust SVG player; replace script handlers with declarative `data-bern-click` / `data-bern-dblclick` attributes on `<svg>`"
                    .into(),
            );
        }
        if !has_declarative_interaction {
            for attr in node.attributes() {
                if attr.name().starts_with("on") {
                    return Err(format!(
                        "inline event handler `{}` is not executed by Bern's native Rust SVG player; use `data-bern-click` / `data-bern-dblclick` on `<svg>`",
                        attr.name()
                    ));
                }
            }
        }
    }

    let view_box = parse_view_box(svg);

    // 1. Collect gradients.
    let mut gradients = Vec::new();
    let mut grad_ids = HashMap::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        let tag = node.tag_name().name();
        if matches!(tag, "linearGradient" | "radialGradient")
            && let Some(id) = node.attribute("id")
            && let Some(def) = parse_gradient(node, &doc)
        {
            let idx = gradients.len() as u16;
            gradients.push(def);
            grad_ids.insert(id.to_string(), idx);
        }
    }

    // 2. Collect clipPaths.
    let mut clips = Vec::new();
    let mut clip_ids = HashMap::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.tag_name().name() == "clipPath"
            && let Some(id) = node.attribute("id")
        {
            let mut items = Vec::new();
            collect_clip_items(node, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut items)?;
            let idx = clips.len() as u16;
            clips.push(ClipDef { items });
            clip_ids.insert(id.to_string(), idx);
        }
    }

    // 3. Parse `<style>` blocks.
    let mut rules = Vec::new();
    let mut keyframes = Vec::new();
    let mut order_counter = 0u32;
    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.tag_name().name() == "style" {
            let mut css_text = String::new();
            for child in node.children().filter(|c| c.is_text()) {
                if let Some(t) = child.text() {
                    css_text.push_str(t);
                }
            }
            parse_stylesheet(
                &css_text,
                &grad_ids,
                &mut order_counter,
                &mut rules,
                &mut keyframes,
            );
        }
    }

    // 4. Build scene node graph rooted at `<svg>` (node 0).
    let mut nodes = Vec::new();
    let root_idx = build_node(svg, None, &doc, &grad_ids, &clip_ids, &mut nodes)?
        .ok_or_else(|| "empty SVG root".to_string())?;
    debug_assert_eq!(root_idx, 0);

    // 5. Precompute exact `fill-box` (`getBBox()`) for every node.
    for idx in 0..nodes.len() {
        let bbox = compute_node_bbox(&nodes, idx);
        nodes[idx].fill_box = bbox;
    }

    let root_children = vec![root_idx];
    Ok(SceneData {
        view_box,
        nodes,
        root_children,
        gradients,
        clips,
        rules,
        keyframes,
        interaction,
    })
}

fn parse_view_box(svg: Node<'_, '_>) -> ViewBox {
    if let Some(vb) = svg.attribute("viewBox") {
        let nums: Vec<f32> = vb
            .split(|c: char| c.is_ascii_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        if nums.len() == 4 && nums[2] > 0.0 && nums[3] > 0.0 {
            return ViewBox {
                x: nums[0],
                y: nums[1],
                width: nums[2],
                height: nums[3],
            };
        }
    }
    let w = svg
        .attribute("width")
        .and_then(parse_length)
        .filter(|&v| v > 0.0)
        .unwrap_or(100.0);
    let h = svg
        .attribute("height")
        .and_then(parse_length)
        .filter(|&v| v > 0.0)
        .unwrap_or(100.0);
    ViewBox {
        x: 0.0,
        y: 0.0,
        width: w,
        height: h,
    }
}

fn parse_interaction(svg: Node<'_, '_>) -> Result<Interaction, String> {
    let ready_after = match svg.attribute("data-bern-ready") {
        Some(v) => parse_time_seconds(v)
            .ok_or_else(|| format!("invalid `data-bern-ready` duration `{v}`"))?
            .max(0.0),
        None => 0.0,
    };
    let on_click = match svg.attribute("data-bern-click") {
        Some(v) => Some(parse_action(v)?),
        None => None,
    };
    let on_dblclick = match svg.attribute("data-bern-dblclick") {
        Some(v) => Some(parse_action(v)?),
        None => None,
    };
    Ok(Interaction {
        ready_after,
        on_click,
        on_dblclick,
    })
}

fn parse_action(raw: &str) -> Result<InteractionAction, String> {
    let s = raw.trim();
    let (open, close) = s
        .find('(')
        .zip(s.rfind(')'))
        .filter(|&(o, c)| o < c && c == s.len() - 1)
        .ok_or_else(|| format!("invalid interaction `{raw}`: expected `func(...)`"))?;
    let func = s[..open].trim().to_ascii_lowercase();
    let inner = s[open + 1..close].trim();

    match func.as_str() {
        "toggle" => {
            let cls = inner.trim();
            if cls.is_empty() || cls.chars().any(char::is_whitespace) {
                return Err(format!("invalid `toggle(...)` class `{cls}`"));
            }
            Ok(InteractionAction::Toggle(cls.to_string()))
        }
        "trigger" => {
            let tc = parse_timed_class(inner)?;
            if tc.class.is_none() {
                return Err("`trigger(...)` requires a class name".into());
            }
            Ok(InteractionAction::Trigger(tc))
        }
        "cycle" => {
            let items = parse_timed_class_list(inner)?;
            if items.is_empty() {
                return Err("`cycle(...)` requires at least one state".into());
            }
            Ok(InteractionAction::Cycle(items))
        }
        "random" => {
            let items = parse_timed_class_list(inner)?;
            if items.is_empty() {
                return Err("`random(...)` requires at least one state".into());
            }
            Ok(InteractionAction::Random(items))
        }
        _ => Err(format!(
            "unknown interaction function `{func}`: expected `toggle`, `trigger`, `cycle`, or `random`"
        )),
    }
}

fn parse_timed_class_list(inner: &str) -> Result<Vec<TimedClass>, String> {
    let mut out = Vec::new();
    for part in split_top_commas(inner) {
        out.push(parse_timed_class(part)?);
    }
    Ok(out)
}

fn parse_timed_class(part: &str) -> Result<TimedClass, String> {
    let (name_raw, dur_raw) = match part.split_once(':') {
        Some((a, b)) => (a.trim(), Some(b.trim())),
        None => (part.trim(), None),
    };
    if name_raw.is_empty() {
        return Err("empty class entry in interaction attribute".into());
    }
    let class = if name_raw.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(name_raw.to_string())
    };
    let duration = match dur_raw {
        Some(d) => {
            let secs = parse_time_seconds(d)
                .ok_or_else(|| format!("invalid duration `{d}` in interaction attribute"))?;
            if secs <= 0.0 {
                return Err(format!("interaction duration `{d}` must be positive"));
            }
            Some(secs)
        }
        None => None,
    };
    Ok(TimedClass { class, duration })
}

fn parse_gradient(node: Node<'_, '_>, doc: &Document<'_>) -> Option<GradientDef> {
    let empty_grads = HashMap::new();
    let href_node = node
        .attribute("href")
        .or_else(|| node.attribute(("http://www.w3.org/1999/xlink", "href")))
        .and_then(|h| h.strip_prefix('#'))
        .and_then(|id| {
            doc.descendants()
                .find(|n| n.is_element() && n.attribute("id") == Some(id))
        });

    let attr = |name: &str| {
        node.attribute(name)
            .or_else(|| href_node.and_then(|h| h.attribute(name)))
    };

    let units = match attr("gradientUnits") {
        Some("userSpaceOnUse") => GradientUnits::UserSpaceOnUse,
        _ => GradientUnits::ObjectBoundingBox,
    };

    let coord = |name: &str, default: f32| -> f32 {
        match attr(name) {
            Some(s) => {
                if let Some(pct) = s.trim().strip_suffix('%') {
                    pct.trim()
                        .parse::<f32>()
                        .map(|v| v * 0.01)
                        .unwrap_or(default)
                } else {
                    parse_length(s).unwrap_or(default)
                }
            }
            None => default,
        }
    };

    let kind = if node.tag_name().name() == "linearGradient" {
        GradientKind::Linear {
            x1: coord("x1", 0.0),
            y1: coord("y1", 0.0),
            x2: coord("x2", 1.0),
            y2: coord("y2", 0.0),
        }
    } else {
        let cx = coord("cx", 0.5);
        let cy = coord("cy", 0.5);
        let r = coord("r", 0.5).max(0.0);
        let fx = attr("fx").map(|_| coord("fx", cx)).unwrap_or(cx);
        let fy = attr("fy").map(|_| coord("fy", cy)).unwrap_or(cy);
        GradientKind::Radial { cx, cy, r, fx, fy }
    };

    let transform = attr("gradientTransform")
        .and_then(parse_transform_list)
        .map(|ops| ops_to_matrix(&ops))
        .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    let stop_parent = if node
        .children()
        .any(|c| c.is_element() && c.tag_name().name() == "stop")
    {
        node
    } else {
        href_node.unwrap_or(node)
    };

    let mut stops = Vec::new();
    for child in stop_parent
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "stop")
    {
        let offset = child
            .attribute("offset")
            .and_then(parse_unit_alpha)
            .unwrap_or(0.0);
        let mut color = child
            .attribute("stop-color")
            .and_then(|v| parse_paint(v, &empty_grads))
            .unwrap_or(ColorSpec::Rgba([0, 0, 0, 255]));
        let mut opacity = child
            .attribute("stop-opacity")
            .and_then(parse_unit_alpha)
            .unwrap_or(1.0);
        if let Some(style) = child.attribute("style") {
            for part in style.split(';') {
                if let Some((k, v)) = part.split_once(':') {
                    match k.trim().to_ascii_lowercase().as_str() {
                        "stop-color" => {
                            if let Some(c) = parse_paint(v.trim(), &empty_grads) {
                                color = c;
                            }
                        }
                        "stop-opacity" => {
                            if let Some(o) = parse_unit_alpha(v.trim()) {
                                opacity = o;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        stops.push(GradientStop {
            offset,
            color,
            opacity,
        });
    }
    if stops.is_empty() {
        return None;
    }
    Some(GradientDef {
        kind,
        units,
        transform,
        stops,
    })
}

fn collect_clip_items(
    parent: Node<'_, '_>,
    parent_tx: [f32; 6],
    out: &mut Vec<ClipItem>,
) -> Result<(), String> {
    for child in parent.children().filter(|c| c.is_element()) {
        let local_tx = child
            .attribute("transform")
            .and_then(parse_transform_list)
            .map(|ops| ops_to_matrix(&ops))
            .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let tx = mul_affine(parent_tx, local_tx);
        let fill_rule = match child
            .attribute("clip-rule")
            .or_else(|| child.attribute("fill-rule"))
        {
            Some(v) if v.eq_ignore_ascii_case("evenodd") => FillRule::EvenOdd,
            _ => FillRule::NonZero,
        };
        if child.tag_name().name() == "g" {
            collect_clip_items(child, tx, out)?;
        } else if let Some(shape) = parse_element_shape(child)? {
            out.push(ClipItem {
                shape,
                transform: tx,
                fill_rule,
            });
        }
    }
    Ok(())
}

fn parse_element_shape(node: Node<'_, '_>) -> Result<Option<ShapeKind>, String> {
    let num = |name: &str| node.attribute(name).and_then(parse_length).unwrap_or(0.0);
    match node.tag_name().name() {
        "svg" | "g" | "a" => Ok(Some(ShapeKind::Group)),
        "path" => {
            let d = node.attribute("d").unwrap_or("");
            if d.trim().is_empty() {
                Ok(Some(ShapeKind::Path(Vec::new())))
            } else {
                Ok(Some(ShapeKind::Path(parse_svg_path(d)?)))
            }
        }
        "rect" => Ok(Some(ShapeKind::Rect {
            x: num("x"),
            y: num("y"),
            width: num("width"),
            height: num("height"),
            rx: num("rx"),
            ry: num("ry"),
        })),
        "circle" => Ok(Some(ShapeKind::Circle {
            cx: num("cx"),
            cy: num("cy"),
            r: num("r"),
        })),
        "ellipse" => Ok(Some(ShapeKind::Ellipse {
            cx: num("cx"),
            cy: num("cy"),
            rx: num("rx"),
            ry: num("ry"),
        })),
        "line" => Ok(Some(ShapeKind::Line {
            x1: num("x1"),
            y1: num("y1"),
            x2: num("x2"),
            y2: num("y2"),
        })),
        "polyline" | "polygon" => {
            let closed = node.tag_name().name() == "polygon";
            let pts = parse_points(node.attribute("points").unwrap_or(""));
            let mut segs = Vec::new();
            if let Some(&(x0, y0)) = pts.first() {
                segs.push(PathSeg::Move(x0, y0));
                for &(x, y) in &pts[1..] {
                    segs.push(PathSeg::Line(x, y));
                }
                if closed {
                    segs.push(PathSeg::Close);
                }
            }
            Ok(Some(ShapeKind::Path(segs)))
        }
        _ => Ok(None),
    }
}

fn parse_points(s: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = s
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    nums.as_chunks::<2>()
        .0
        .iter()
        .map(|&[x, y]| (x, y))
        .collect()
}

fn build_node(
    el: Node<'_, '_>,
    parent: Option<u16>,
    doc: &Document<'_>,
    grad_ids: &HashMap<String, u16>,
    clip_ids: &HashMap<String, u16>,
    nodes: &mut Vec<SceneNode>,
) -> Result<Option<u16>, String> {
    let tag = el.tag_name().name();
    if matches!(
        tag,
        "defs"
            | "style"
            | "script"
            | "title"
            | "desc"
            | "metadata"
            | "clipPath"
            | "mask"
            | "filter"
            | "linearGradient"
            | "radialGradient"
            | "animate"
            | "animateTransform"
            | "animateMotion"
            | "animateColor"
            | "set"
    ) {
        return Ok(None);
    }

    if tag == "use" {
        if let Some(target) = el
            .attribute("href")
            .or_else(|| el.attribute(("http://www.w3.org/1999/xlink", "href")))
            .and_then(|h| h.strip_prefix('#'))
            .and_then(|id| {
                doc.descendants()
                    .find(|n| n.is_element() && n.attribute("id") == Some(id))
            })
        {
            let x = el.attribute("x").and_then(parse_length).unwrap_or(0.0);
            let y = el.attribute("y").and_then(parse_length).unwrap_or(0.0);
            let mut svg_transform = el
                .attribute("transform")
                .and_then(parse_transform_list)
                .unwrap_or_default();
            if x != 0.0 || y != 0.0 {
                svg_transform.push(TransformOp::Translate(x, y));
            }
            let base_props = collect_node_props(el, grad_ids);
            let wrapper_idx = nodes.len() as u16;
            nodes.push(SceneNode {
                parent,
                children: Vec::new(),
                tag: "g".into(),
                id: el.attribute("id").map(str::to_owned),
                classes: parse_class_attr(el),
                shape: ShapeKind::Group,
                fill_box: BBox::default(),
                clip: parse_clip_ref(el, clip_ids),
                svg_transform,
                base_props,
                smil: collect_smil(el, &ShapeKind::Group, grad_ids),
            });
            if let Some(child_idx) =
                build_node(target, Some(wrapper_idx), doc, grad_ids, clip_ids, nodes)?
            {
                nodes[wrapper_idx as usize].children.push(child_idx);
            }
            return Ok(Some(wrapper_idx));
        }
        return Ok(None);
    }

    let Some(shape) = parse_element_shape(el)? else {
        return Ok(None);
    };

    let idx = nodes.len() as u16;
    let svg_transform = el
        .attribute("transform")
        .and_then(parse_transform_list)
        .unwrap_or_default();
    let base_props = collect_node_props(el, grad_ids);
    let smil = collect_smil(el, &shape, grad_ids);
    let clip = parse_clip_ref(el, clip_ids);

    nodes.push(SceneNode {
        parent,
        children: Vec::new(),
        tag: tag.to_ascii_lowercase(),
        id: el.attribute("id").map(str::to_owned),
        classes: parse_class_attr(el),
        shape,
        fill_box: BBox::default(),
        clip,
        svg_transform,
        base_props,
        smil,
    });

    if matches!(tag, "svg" | "g" | "a") {
        let mut child_indices = Vec::new();
        for child in el.children().filter(|c| c.is_element()) {
            if let Some(c_idx) = build_node(child, Some(idx), doc, grad_ids, clip_ids, nodes)? {
                child_indices.push(c_idx);
            }
        }
        nodes[idx as usize].children = child_indices;
    }

    Ok(Some(idx))
}

fn parse_class_attr(el: Node<'_, '_>) -> Vec<String> {
    el.attribute("class")
        .map(|c| c.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

fn parse_clip_ref(el: Node<'_, '_>, clip_ids: &HashMap<String, u16>) -> Option<u16> {
    let raw = el.attribute("clip-path").or_else(|| {
        el.attribute("style").and_then(|st| {
            st.split(';').find_map(|p| {
                let (k, v) = p.split_once(':')?;
                k.trim()
                    .eq_ignore_ascii_case("clip-path")
                    .then_some(v.trim())
            })
        })
    })?;
    let inner = raw
        .trim()
        .strip_prefix("url(")?
        .strip_suffix(')')?
        .trim()
        .trim_matches(['"', '\''])
        .strip_prefix('#')?;
    clip_ids.get(inner).copied()
}

fn collect_node_props(
    el: Node<'_, '_>,
    grad_ids: &HashMap<String, u16>,
) -> Vec<(StyleProp, bool, bool)> {
    let mut out = Vec::new();
    for attr in el.attributes() {
        let name = attr.name();
        if matches!(
            name,
            "fill"
                | "fill-opacity"
                | "fill-rule"
                | "stroke"
                | "stroke-width"
                | "stroke-opacity"
                | "stroke-linecap"
                | "stroke-linejoin"
                | "stroke-dasharray"
                | "stroke-dashoffset"
                | "opacity"
                | "display"
        ) {
            let mut tmp = Vec::new();
            parse_property(name, attr.value(), false, grad_ids, &mut tmp);
            for (p, imp) in tmp {
                out.push((p, false, imp));
            }
        }
    }
    if let Some(style) = el.attribute("style") {
        for (p, imp) in parse_declarations(style, grad_ids) {
            out.push((p, true, imp));
        }
    }
    out
}

fn collect_smil(
    el: Node<'_, '_>,
    shape: &ShapeKind,
    grad_ids: &HashMap<String, u16>,
) -> Vec<SmilAnim> {
    let mut out = Vec::new();
    for child in el.children().filter(|c| c.is_element()) {
        match child.tag_name().name() {
            "animate" | "set" => {
                if let Some(anim) = parse_smil_animate(el, child, shape, grad_ids) {
                    out.push(anim);
                }
            }
            "animateTransform" => {
                if let Some(anim) = parse_smil_transform(child) {
                    out.push(anim);
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_smil_timing(child: Node<'_, '_>) -> Option<(f32, f32, Iterations, bool, bool)> {
    let is_set = child.tag_name().name() == "set";
    let dur = match child.attribute("dur") {
        Some("indefinite") => f32::INFINITY,
        Some(d) => parse_time_seconds(d)?,
        None if is_set => f32::INFINITY,
        None => return None,
    };
    if dur <= 0.0 {
        return None;
    }
    let begin = child
        .attribute("begin")
        .and_then(|b| b.split(';').next())
        .and_then(parse_time_seconds)
        .unwrap_or(0.0);
    let iterations = match child.attribute("repeatCount") {
        Some(r) if r.eq_ignore_ascii_case("indefinite") => Iterations::Infinite,
        Some(r) => Iterations::Count(r.trim().parse::<f32>().ok()?.max(0.0)),
        None => Iterations::Count(1.0),
    };
    let freeze = child.attribute("fill") == Some("freeze") || is_set;
    let discrete = child.attribute("calcMode") == Some("discrete") || is_set;
    Some((begin, dur, iterations, freeze, discrete))
}

fn parse_smil_animate(
    parent: Node<'_, '_>,
    child: Node<'_, '_>,
    shape: &ShapeKind,
    grad_ids: &HashMap<String, u16>,
) -> Option<SmilAnim> {
    let (begin, dur, iterations, freeze, discrete) = parse_smil_timing(child)?;
    let attr_name = child.attribute("attributeName")?.trim();
    let attr = match attr_name {
        "r" => SmilAttr::R,
        "rx" => SmilAttr::Rx,
        "ry" => SmilAttr::Ry,
        "cx" => SmilAttr::Cx,
        "cy" => SmilAttr::Cy,
        "x" => SmilAttr::X,
        "y" => SmilAttr::Y,
        "width" => SmilAttr::Width,
        "height" => SmilAttr::Height,
        "opacity" => SmilAttr::Opacity,
        "stroke-width" => SmilAttr::StrokeWidth,
        "stroke-dashoffset" => SmilAttr::StrokeDashoffset,
        "fill" => SmilAttr::Fill,
        "stroke" => SmilAttr::Stroke,
        _ => return None,
    };

    let parse_val = |raw: &str| -> Option<SmilValue> {
        match attr {
            SmilAttr::Fill | SmilAttr::Stroke => parse_paint(raw, grad_ids).map(SmilValue::Color),
            SmilAttr::Opacity => parse_unit_alpha(raw).map(SmilValue::Scalar),
            _ => parse_length(raw).map(SmilValue::Scalar),
        }
    };

    let base_scalar = || -> SmilValue {
        let v = match (attr, shape) {
            (SmilAttr::R, &ShapeKind::Circle { r, .. }) => r,
            (SmilAttr::Cx, &ShapeKind::Circle { cx, .. } | &ShapeKind::Ellipse { cx, .. }) => cx,
            (SmilAttr::Cy, &ShapeKind::Circle { cy, .. } | &ShapeKind::Ellipse { cy, .. }) => cy,
            (SmilAttr::Rx, &ShapeKind::Ellipse { rx, .. } | &ShapeKind::Rect { rx, .. }) => rx,
            (SmilAttr::Ry, &ShapeKind::Ellipse { ry, .. } | &ShapeKind::Rect { ry, .. }) => ry,
            (SmilAttr::X, &ShapeKind::Rect { x, .. }) => x,
            (SmilAttr::Y, &ShapeKind::Rect { y, .. }) => y,
            (SmilAttr::Width, &ShapeKind::Rect { width, .. }) => width,
            (SmilAttr::Height, &ShapeKind::Rect { height, .. }) => height,
            (SmilAttr::Opacity, _) => parent
                .attribute("opacity")
                .and_then(parse_unit_alpha)
                .unwrap_or(1.0),
            (SmilAttr::StrokeWidth, _) => parent
                .attribute("stroke-width")
                .and_then(parse_length)
                .unwrap_or(1.0),
            _ => 0.0,
        };
        SmilValue::Scalar(v)
    };

    let values: Vec<SmilValue> = if let Some(vals) = child.attribute("values") {
        vals.split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(parse_val)
            .collect()
    } else {
        let from = child
            .attribute("from")
            .and_then(parse_val)
            .or_else(|| parent.attribute(attr_name).and_then(parse_val))
            .unwrap_or_else(base_scalar);
        let to = parse_val(child.attribute("to")?)?;
        if discrete && child.attribute("from").is_none() {
            vec![to]
        } else {
            vec![from, to]
        }
    };
    if values.is_empty() {
        return None;
    }

    let (key_times, splines) = parse_smil_keys_and_splines(child, values.len());
    Some(SmilAnim {
        attr,
        values,
        key_times,
        splines,
        discrete,
        begin,
        dur,
        iterations,
        freeze,
    })
}

fn parse_smil_transform(child: Node<'_, '_>) -> Option<SmilAnim> {
    let (begin, dur, iterations, freeze, discrete) = parse_smil_timing(child)?;
    let kind = child.attribute("type").unwrap_or("translate").trim();
    let attr = match kind {
        "translate" => SmilAttr::TransformTranslate,
        "scale" => SmilAttr::TransformScale,
        "rotate" => SmilAttr::TransformRotate,
        _ => return None,
    };

    let parse_tf_val = |raw: &str| -> Option<SmilValue> {
        let nums: Vec<&str> = raw
            .split(|c: char| c.is_ascii_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .collect();
        match attr {
            SmilAttr::TransformTranslate => {
                let x = parse_length(nums.first()?)?;
                let y = nums.get(1).and_then(|s| parse_length(s)).unwrap_or(0.0);
                Some(SmilValue::Pair(x, y))
            }
            SmilAttr::TransformScale => {
                let x = nums.first()?.parse::<f32>().ok()?;
                let y = nums.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(x);
                Some(SmilValue::Pair(x, y))
            }
            SmilAttr::TransformRotate => {
                let a = parse_angle_deg(nums.first()?)?;
                let pivot = if nums.len() >= 3 {
                    Some((parse_length(nums[1])?, parse_length(nums[2])?))
                } else {
                    None
                };
                Some(SmilValue::Rotate(a, pivot))
            }
            _ => None,
        }
    };

    let default_from = match attr {
        SmilAttr::TransformScale => SmilValue::Pair(1.0, 1.0),
        SmilAttr::TransformRotate => SmilValue::Rotate(0.0, None),
        _ => SmilValue::Pair(0.0, 0.0),
    };

    let values: Vec<SmilValue> = if let Some(vals) = child.attribute("values") {
        vals.split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(parse_tf_val)
            .collect()
    } else {
        let from = child
            .attribute("from")
            .and_then(parse_tf_val)
            .unwrap_or(default_from);
        let to = parse_tf_val(child.attribute("to")?)?;
        vec![from, to]
    };
    if values.is_empty() {
        return None;
    }

    let (key_times, splines) = parse_smil_keys_and_splines(child, values.len());
    Some(SmilAnim {
        attr,
        values,
        key_times,
        splines,
        discrete,
        begin,
        dur,
        iterations,
        freeze,
    })
}

fn parse_smil_keys_and_splines(child: Node<'_, '_>, n: usize) -> (Vec<f32>, Vec<Easing>) {
    let key_times = child
        .attribute("keyTimes")
        .map(|kt| {
            kt.split(';')
                .filter_map(|t| t.trim().parse::<f32>().ok())
                .collect::<Vec<_>>()
        })
        .filter(|kt| kt.len() == n)
        .unwrap_or_else(|| {
            if n <= 1 {
                vec![0.0]
            } else {
                (0..n).map(|i| i as f32 / (n - 1) as f32).collect()
            }
        });

    let splines = if child.attribute("calcMode") == Some("spline")
        && let Some(ks) = child.attribute("keySplines")
    {
        ks.split(';')
            .filter_map(|s| parse_easing(&format!("cubic-bezier({s})")))
            .collect()
    } else {
        Vec::new()
    };
    (key_times, splines)
}

/// Exact SVG `getBBox()` of `nodes[target]` in its own local coordinate system
/// (before `nodes[target].svg_transform`).
fn compute_node_bbox(nodes: &[SceneNode], target: usize) -> BBox {
    fn visit(nodes: &[SceneNode], idx: usize, rel_tx: [f32; 6], acc: &mut BBox) {
        let node = &nodes[idx];
        if let Some(path) = build_shape(&node.shape) {
            *acc = acc.union(path_bbox(&path, rel_tx));
        }
        for &child_idx in &node.children {
            let child = &nodes[child_idx as usize];
            let child_tx = mul_affine(rel_tx, ops_to_matrix(&child.svg_transform));
            visit(nodes, child_idx as usize, child_tx, acc);
        }
    }

    let mut acc = BBox::default();
    let root = &nodes[target];
    if let Some(path) = build_shape(&root.shape) {
        acc = acc.union(path_bbox(&path, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    }
    for &child_idx in &root.children {
        let child = &nodes[child_idx as usize];
        let child_tx = ops_to_matrix(&child.svg_transform);
        visit(nodes, child_idx as usize, child_tx, &mut acc);
    }
    acc
}
