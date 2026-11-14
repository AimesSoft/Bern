//! CSS subset parser for animated SVG: selectors, `@media`, `@keyframes`,
//! `animation`, `transition`, transforms, origins, and colors.

use crate::model::*;
use std::collections::HashMap;

pub fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(idx) = rest.find("/*") {
        out.push_str(&rest[..idx]);
        rest = match rest[idx + 2..].find("*/") {
            Some(end) => &rest[idx + 2 + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

pub fn parse_stylesheet(
    raw: &str,
    grad_ids: &HashMap<String, u16>,
    order_counter: &mut u32,
    rules: &mut Vec<StyleRule>,
    keyframes: &mut Vec<Keyframes>,
) {
    let clean = strip_comments(raw);
    parse_blocks(
        &clean,
        MediaCond::Always,
        grad_ids,
        order_counter,
        rules,
        keyframes,
    );
}

fn parse_blocks(
    css: &str,
    media: MediaCond,
    grad_ids: &HashMap<String, u16>,
    order_counter: &mut u32,
    rules: &mut Vec<StyleRule>,
    keyframes: &mut Vec<Keyframes>,
) {
    let bytes = css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let Some(open) = find_top_brace(&css[i..]).map(|o| i + o) else {
            // Skip stray `@import` / `@charset` ending in `;`.
            if let Some(semi) = css[i..].find(';') {
                i += semi + 1;
                continue;
            }
            break;
        };
        let prelude = css[i..open].trim();
        let Some(close) = find_matching_brace(&css[open..]).map(|c| open + c) else {
            break;
        };
        let body = &css[open + 1..close];
        i = close + 1;

        if prelude.starts_with('@') {
            let lower = prelude.to_ascii_lowercase();
            if let Some(name) = lower
                .strip_prefix("@keyframes")
                .or_else(|| lower.strip_prefix("@-webkit-keyframes"))
            {
                let orig_len = prelude.len() - name.len();
                let ident = prelude[orig_len..]
                    .trim()
                    .trim_matches(['"', '\''])
                    .to_string();
                if !ident.is_empty()
                    && let Some(kf) = parse_keyframes(&ident, body, grad_ids)
                {
                    if let Some(existing) = keyframes.iter_mut().find(|k| k.name == ident) {
                        *existing = kf;
                    } else {
                        keyframes.push(kf);
                    }
                }
            } else if let Some(cond) = lower.strip_prefix("@media")
                && let Some(sub_media) = parse_media_cond(cond)
            {
                parse_blocks(body, sub_media, grad_ids, order_counter, rules, keyframes);
            }
            continue;
        }

        let decls = parse_declarations(body, grad_ids);
        if decls.is_empty() {
            continue;
        }
        for sel_text in split_top_commas(prelude) {
            let Some((steps, specificity)) = parse_selector(sel_text) else {
                continue;
            };
            let order = *order_counter;
            *order_counter += 1;
            rules.push(StyleRule {
                media,
                steps,
                specificity,
                order,
                declarations: decls.clone(),
            });
        }
    }
}

fn find_top_brace(s: &str) -> Option<usize> {
    let mut paren = 0usize;
    for (idx, b) in s.bytes().enumerate() {
        match b {
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'{' if paren == 0 => return Some(idx),
            _ => {}
        }
    }
    None
}

fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, b) in s.bytes().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_media_cond(cond: &str) -> Option<MediaCond> {
    let compact: String = cond
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if compact.contains("not(") {
        return None;
    }
    if compact.contains("(prefers-color-scheme:dark)") {
        Some(MediaCond::SchemeDark)
    } else if compact.contains("(prefers-color-scheme:light)") {
        Some(MediaCond::SchemeLight)
    } else if compact.contains("(prefers-reduced-motion:reduce)") {
        Some(MediaCond::ReducedMotion)
    } else {
        None
    }
}

pub fn split_top_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, b) in s.bytes().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                let piece = s[start..idx].trim();
                if !piece.is_empty() {
                    out.push(piece);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

fn parse_selector(raw: &str) -> Option<(Vec<SelectorStep>, (u16, u16, u16))> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let mut steps = Vec::new();
    let mut spec = (0u16, 0u16, 0u16);
    let mut next_direct = false;
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b'>' {
            if steps.is_empty() || next_direct {
                return None;
            }
            next_direct = true;
            i += 1;
            continue;
        }
        if matches!(bytes[i], b'+' | b'~') {
            return None;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
            if bytes[i] == b'(' {
                while i < bytes.len() && bytes[i] != b')' {
                    i += 1;
                }
            }
            if i < bytes.len() {
                i += 1;
            }
        }
        let token = &s[start..i];
        let compound = parse_compound(token, &mut spec)?;
        steps.push(SelectorStep {
            compound,
            direct_parent: next_direct,
        });
        next_direct = false;
    }

    if steps.is_empty() || next_direct {
        None
    } else {
        Some((steps, spec))
    }
}

fn parse_compound(token: &str, spec: &mut (u16, u16, u16)) -> Option<CompoundSel> {
    let mut sel = CompoundSel {
        tag: None,
        id: None,
        classes: Vec::new(),
        hover: false,
        active: false,
    };
    let bytes = token.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'*' if i == 0 => {
                i += 1;
            }
            b'.' => {
                i += 1;
                let start = i;
                while i < bytes.len() && is_ident_byte(bytes[i]) {
                    i += 1;
                }
                if i == start {
                    return None;
                }
                sel.classes.push(token[start..i].to_string());
                spec.1 = spec.1.saturating_add(1);
            }
            b'#' => {
                i += 1;
                let start = i;
                while i < bytes.len() && is_ident_byte(bytes[i]) {
                    i += 1;
                }
                if i == start {
                    return None;
                }
                sel.id = Some(token[start..i].to_string());
                spec.0 = spec.0.saturating_add(1);
            }
            b':' => {
                i += 1;
                if i < bytes.len() && bytes[i] == b':' {
                    return None;
                }
                let start = i;
                while i < bytes.len() && is_ident_byte(bytes[i]) {
                    i += 1;
                }
                match token[start..i].to_ascii_lowercase().as_str() {
                    "hover" => {
                        sel.hover = true;
                        spec.1 = spec.1.saturating_add(1);
                    }
                    "active" => {
                        sel.active = true;
                        spec.1 = spec.1.saturating_add(1);
                    }
                    "root" => {
                        sel.tag = Some("svg".into());
                        spec.1 = spec.1.saturating_add(1);
                    }
                    _ => return None,
                }
            }
            b if i == 0 && is_ident_byte(b) => {
                let start = i;
                while i < bytes.len() && is_ident_byte(bytes[i]) {
                    i += 1;
                }
                sel.tag = Some(token[start..i].to_ascii_lowercase());
                spec.2 = spec.2.saturating_add(1);
            }
            _ => return None,
        }
    }
    Some(sel)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')
}

fn parse_keyframes(name: &str, body: &str, grad_ids: &HashMap<String, u16>) -> Option<Keyframes> {
    let mut track_map: Vec<(TrackProp, Vec<KeyframeStop>)> = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let Some(open) = body[i..].find('{').map(|o| i + o) else {
            break;
        };
        let sel_part = body[i..open].trim();
        let Some(close) = find_matching_brace(&body[open..]).map(|c| open + c) else {
            break;
        };
        let decl_body = &body[open + 1..close];
        i = close + 1;

        let mut offsets = Vec::new();
        for piece in split_top_commas(sel_part) {
            let p = piece.trim().to_ascii_lowercase();
            let off = if p == "from" {
                Some(0.0)
            } else if p == "to" {
                Some(1.0)
            } else {
                p.strip_suffix('%')
                    .and_then(|n| n.trim().parse::<f32>().ok())
                    .map(|n| (n * 0.01).clamp(0.0, 1.0))
            };
            if let Some(o) = off {
                offsets.push(o);
            }
        }
        if offsets.is_empty() {
            continue;
        }

        let decls = parse_declarations(decl_body, grad_ids);
        let mut step_easing = None;
        let mut values = Vec::new();
        for (prop, _) in decls {
            match prop {
                StyleProp::AnimTiming(e) => step_easing = Some(e),
                StyleProp::Transform(ops) => {
                    values.push((TrackProp::Transform, AnimValue::Transform(ops)))
                }
                StyleProp::Opacity(v) => values.push((TrackProp::Opacity, AnimValue::Scalar(v))),
                StyleProp::Fill(c) => values.push((TrackProp::Fill, AnimValue::Color(c))),
                StyleProp::Stroke(c) => values.push((TrackProp::Stroke, AnimValue::Color(c))),
                StyleProp::StrokeWidth(v) => {
                    values.push((TrackProp::StrokeWidth, AnimValue::Scalar(v)))
                }
                StyleProp::StrokeDashoffset(v) => {
                    values.push((TrackProp::StrokeDashoffset, AnimValue::Scalar(v)))
                }
                _ => {}
            }
        }
        for offset in offsets {
            for (tp, val) in &values {
                let stops = match track_map.iter_mut().find(|(p, _)| *p == *tp) {
                    Some((_, stops)) => stops,
                    None => {
                        track_map.push((*tp, Vec::new()));
                        &mut track_map.last_mut().unwrap().1
                    }
                };
                if let Some(existing) = stops.iter_mut().find(|s| (s.offset - offset).abs() < 1e-5)
                {
                    existing.value = val.clone();
                    existing.easing = step_easing;
                } else {
                    stops.push(KeyframeStop {
                        offset,
                        value: val.clone(),
                        easing: step_easing,
                    });
                }
            }
        }
    }

    let mut tracks = Vec::with_capacity(track_map.len());
    for (prop, mut stops) in track_map {
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        if !stops.is_empty() {
            tracks.push(KeyframeTrack { prop, stops });
        }
    }
    Some(Keyframes {
        name: name.to_string(),
        tracks,
    })
}

pub fn parse_declarations(css: &str, grad_ids: &HashMap<String, u16>) -> Vec<(StyleProp, bool)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, b) in css.bytes().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => {
                parse_one_decl(&css[start..idx], grad_ids, &mut out);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parse_one_decl(&css[start..], grad_ids, &mut out);
    out
}

fn parse_one_decl(raw: &str, grad_ids: &HashMap<String, u16>, out: &mut Vec<(StyleProp, bool)>) {
    let Some((prop_raw, val_raw)) = raw.split_once(':') else {
        return;
    };
    let prop = prop_raw.trim().to_ascii_lowercase();
    let val_trim = val_raw.trim();
    let (val, important) = match strip_important(val_trim) {
        Some(rest) => (rest.trim(), true),
        None => (val_trim, false),
    };
    if val.is_empty() {
        return;
    }
    parse_property(&prop, val, important, grad_ids, out);
}

fn strip_important(s: &str) -> Option<&str> {
    let lower = s.to_ascii_lowercase();
    let rest = lower.strip_suffix("important")?.trim_end();
    let rest_len = rest.strip_suffix('!')?.len();
    Some(&s[..rest_len])
}

pub fn parse_property(
    prop: &str,
    val: &str,
    important: bool,
    grad_ids: &HashMap<String, u16>,
    out: &mut Vec<(StyleProp, bool)>,
) {
    let mut push = |p: StyleProp| out.push((p, important));
    match prop {
        "fill" => {
            if let Some(c) = parse_paint(val, grad_ids) {
                push(StyleProp::Fill(c));
            }
        }
        "fill-opacity" => {
            if let Some(n) = parse_unit_alpha(val) {
                push(StyleProp::FillOpacity(n));
            }
        }
        "fill-rule" => match val.to_ascii_lowercase().as_str() {
            "evenodd" => push(StyleProp::FillRule(FillRule::EvenOdd)),
            "nonzero" => push(StyleProp::FillRule(FillRule::NonZero)),
            _ => {}
        },
        "stroke" => {
            if let Some(c) = parse_paint(val, grad_ids) {
                push(StyleProp::Stroke(c));
            }
        }
        "stroke-width" => {
            if let Some(n) = parse_length(val) {
                push(StyleProp::StrokeWidth(n.max(0.0)));
            }
        }
        "stroke-opacity" => {
            if let Some(n) = parse_unit_alpha(val) {
                push(StyleProp::StrokeOpacity(n));
            }
        }
        "stroke-linecap" => match val.to_ascii_lowercase().as_str() {
            "butt" => push(StyleProp::StrokeLinecap(LineCap::Butt)),
            "round" => push(StyleProp::StrokeLinecap(LineCap::Round)),
            "square" => push(StyleProp::StrokeLinecap(LineCap::Square)),
            _ => {}
        },
        "stroke-linejoin" => match val.to_ascii_lowercase().as_str() {
            "miter" => push(StyleProp::StrokeLinejoin(LineJoin::Miter)),
            "round" => push(StyleProp::StrokeLinejoin(LineJoin::Round)),
            "bevel" => push(StyleProp::StrokeLinejoin(LineJoin::Bevel)),
            _ => {}
        },
        "stroke-dasharray" => {
            if val.eq_ignore_ascii_case("none") {
                push(StyleProp::StrokeDasharray(Vec::new()));
            } else {
                let nums: Vec<f32> = val
                    .split(|c: char| c.is_ascii_whitespace() || c == ',')
                    .filter(|t| !t.is_empty())
                    .filter_map(parse_length)
                    .collect();
                if !nums.is_empty() {
                    push(StyleProp::StrokeDasharray(nums));
                }
            }
        }
        "stroke-dashoffset" => {
            if let Some(n) = parse_length(val) {
                push(StyleProp::StrokeDashoffset(n));
            }
        }
        "opacity" => {
            if let Some(n) = parse_unit_alpha(val) {
                push(StyleProp::Opacity(n));
            }
        }
        "display" => {
            push(StyleProp::DisplayNone(val.eq_ignore_ascii_case("none")));
        }
        "transform" => {
            if let Some(ops) = parse_transform_list(val) {
                push(StyleProp::Transform(ops));
            }
        }
        "transform-box" => match val.to_ascii_lowercase().as_str() {
            "fill-box" | "content-box" | "border-box" => {
                push(StyleProp::TransformBox(TransformBox::FillBox))
            }
            "view-box" | "stroke-box" => push(StyleProp::TransformBox(TransformBox::ViewBox)),
            _ => {}
        },
        "transform-origin" => {
            if let Some(o) = parse_origin(val) {
                push(StyleProp::TransformOrigin(o));
            }
        }
        "animation-name" => {
            if val.eq_ignore_ascii_case("none") {
                push(StyleProp::AnimName(None));
            } else {
                push(StyleProp::AnimName(Some(
                    val.trim_matches(['"', '\'']).to_string(),
                )));
            }
        }
        "animation-duration" => {
            if let Some(t) = parse_time_seconds(val) {
                push(StyleProp::AnimDuration(t.max(0.0)));
            }
        }
        "animation-delay" => {
            if let Some(t) = parse_time_seconds(val) {
                push(StyleProp::AnimDelay(t));
            }
        }
        "animation-timing-function" => {
            if let Some(e) = parse_easing(val) {
                push(StyleProp::AnimTiming(e));
            }
        }
        "animation-iteration-count" => {
            if val.eq_ignore_ascii_case("infinite") {
                push(StyleProp::AnimIterations(Iterations::Infinite));
            } else if let Ok(n) = val.parse::<f32>() {
                push(StyleProp::AnimIterations(Iterations::Count(n.max(0.0))));
            }
        }
        "animation-direction" => match val.to_ascii_lowercase().as_str() {
            "normal" => push(StyleProp::AnimDirection(AnimDirection::Normal)),
            "reverse" => push(StyleProp::AnimDirection(AnimDirection::Reverse)),
            "alternate" => push(StyleProp::AnimDirection(AnimDirection::Alternate)),
            "alternate-reverse" => push(StyleProp::AnimDirection(AnimDirection::AlternateReverse)),
            _ => {}
        },
        "animation-fill-mode" => match val.to_ascii_lowercase().as_str() {
            "none" => push(StyleProp::AnimFill(AnimFill::None)),
            "forwards" => push(StyleProp::AnimFill(AnimFill::Forwards)),
            "backwards" => push(StyleProp::AnimFill(AnimFill::Backwards)),
            "both" => push(StyleProp::AnimFill(AnimFill::Both)),
            _ => {}
        },
        "animation" => {
            for p in parse_animation_shorthand(val) {
                push(p);
            }
        }
        "transition" => {
            let specs = parse_transition_shorthand(val);
            push(StyleProp::Transitions(specs));
        }
        _ => {}
    }
}

fn tokenize_ws_respecting_parens(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        let mut depth = 0usize;
        while i < bytes.len() && (depth > 0 || !bytes[i].is_ascii_whitespace()) {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            i += 1;
        }
        out.push(&s[start..i]);
    }
    out
}

fn parse_animation_shorthand(val: &str) -> Vec<StyleProp> {
    // Support comma-separated list by taking the last/primary or first animation;
    // CSS single-element keyframes use one animation rule per element.
    let first = split_top_commas(val).into_iter().next().unwrap_or(val);
    let mut name: Option<Option<String>> = None;
    let mut duration: Option<f32> = None;
    let mut delay: Option<f32> = None;
    let mut timing = Easing::EASE;
    let mut iterations = Iterations::Count(1.0);
    let mut direction = AnimDirection::Normal;
    let mut fill = AnimFill::None;

    for tok in tokenize_ws_respecting_parens(first) {
        let low = tok.to_ascii_lowercase();
        if let Some(t) = parse_time_seconds(&low) {
            if duration.is_none() {
                duration = Some(t.max(0.0));
            } else if delay.is_none() {
                delay = Some(t);
            }
            continue;
        }
        if let Some(e) = parse_easing(tok) {
            timing = e;
            continue;
        }
        match low.as_str() {
            "infinite" => iterations = Iterations::Infinite,
            "normal" => direction = AnimDirection::Normal,
            "reverse" => direction = AnimDirection::Reverse,
            "alternate" => direction = AnimDirection::Alternate,
            "alternate-reverse" => direction = AnimDirection::AlternateReverse,
            "forwards" => fill = AnimFill::Forwards,
            "backwards" => fill = AnimFill::Backwards,
            "both" => fill = AnimFill::Both,
            "running" | "paused" => {}
            "none" => {
                if name.is_none() {
                    name = Some(None);
                } else {
                    fill = AnimFill::None;
                }
            }
            _ => {
                if let Ok(n) = low.parse::<f32>() {
                    iterations = Iterations::Count(n.max(0.0));
                } else if name.is_none() {
                    name = Some(Some(tok.trim_matches(['"', '\'']).to_string()));
                }
            }
        }
    }

    vec![
        StyleProp::AnimName(name.flatten()),
        StyleProp::AnimDuration(duration.unwrap_or(0.0)),
        StyleProp::AnimDelay(delay.unwrap_or(0.0)),
        StyleProp::AnimTiming(timing),
        StyleProp::AnimIterations(iterations),
        StyleProp::AnimDirection(direction),
        StyleProp::AnimFill(fill),
    ]
}

fn parse_transition_shorthand(val: &str) -> Vec<TransitionSpec> {
    if val.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    let mut specs = Vec::new();
    for item in split_top_commas(val) {
        let mut prop = TransitionProp::All;
        let mut duration: Option<f32> = None;
        let mut delay: Option<f32> = None;
        let mut easing = Easing::EASE;
        for tok in tokenize_ws_respecting_parens(item) {
            let low = tok.to_ascii_lowercase();
            if let Some(t) = parse_time_seconds(&low) {
                if duration.is_none() {
                    duration = Some(t.max(0.0));
                } else if delay.is_none() {
                    delay = Some(t);
                }
                continue;
            }
            if let Some(e) = parse_easing(tok) {
                easing = e;
                continue;
            }
            prop = match low.as_str() {
                "all" => TransitionProp::All,
                "fill" => TransitionProp::Fill,
                "stroke" => TransitionProp::Stroke,
                "opacity" => TransitionProp::Opacity,
                "transform" => TransitionProp::Transform,
                "stroke-width" => TransitionProp::StrokeWidth,
                "stroke-dashoffset" => TransitionProp::StrokeDashoffset,
                _ => continue,
            };
        }
        if let Some(d) = duration.filter(|&d| d > 0.0) {
            specs.push(TransitionSpec {
                prop,
                duration: d,
                delay: delay.unwrap_or(0.0),
                easing,
            });
        }
    }
    specs
}

pub fn parse_easing(s: &str) -> Option<Easing> {
    let low = s.trim().to_ascii_lowercase();
    match low.as_str() {
        "linear" => return Some(Easing::Linear),
        "ease" => return Some(Easing::EASE),
        "ease-in" => return Some(Easing::EASE_IN),
        "ease-out" => return Some(Easing::EASE_OUT),
        "ease-in-out" => return Some(Easing::EASE_IN_OUT),
        "step-start" => return Some(Easing::Steps(1, true)),
        "step-end" => return Some(Easing::Steps(1, false)),
        _ => {}
    }
    if let Some(inner) = low
        .strip_prefix("cubic-bezier(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let nums: Vec<f32> = inner
            .split(|c: char| c.is_ascii_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            return Some(Easing::CubicBezier(
                nums[0].clamp(0.0, 1.0),
                nums[1],
                nums[2].clamp(0.0, 1.0),
                nums[3],
            ));
        }
    }
    if let Some(inner) = low.strip_prefix("steps(").and_then(|r| r.strip_suffix(')')) {
        let mut parts = inner.split(',').map(str::trim);
        let n = parts.next()?.parse::<u16>().ok()?.max(1);
        let jump_start = matches!(parts.next(), Some("start" | "jump-start"));
        return Some(Easing::Steps(n, jump_start));
    }
    None
}

pub fn parse_time_seconds(s: &str) -> Option<f32> {
    let low = s.trim().to_ascii_lowercase();
    if let Some(ms) = low.strip_suffix("ms") {
        ms.trim().parse::<f32>().ok().map(|v| v * 0.001)
    } else if let Some(sec) = low.strip_suffix('s') {
        sec.trim().parse::<f32>().ok()
    } else {
        None
    }
}

pub fn parse_length(s: &str) -> Option<f32> {
    let t = s.trim();
    let stripped = t
        .strip_suffix("px")
        .or_else(|| t.strip_suffix("pt"))
        .unwrap_or(t);
    stripped.trim().parse::<f32>().ok()
}

pub fn parse_unit_alpha(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(pct) = t.strip_suffix('%') {
        pct.trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v * 0.01).clamp(0.0, 1.0))
    } else {
        t.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
    }
}

pub fn parse_coord_val(s: &str) -> Option<CoordVal> {
    let low = s.trim().to_ascii_lowercase();
    match low.as_str() {
        "left" | "top" => return Some(CoordVal::Percent(0.0)),
        "center" => return Some(CoordVal::Percent(50.0)),
        "right" | "bottom" => return Some(CoordVal::Percent(100.0)),
        _ => {}
    }
    if let Some(pct) = low.strip_suffix('%') {
        pct.trim().parse::<f32>().ok().map(CoordVal::Percent)
    } else {
        parse_length(&low).map(CoordVal::Px)
    }
}

pub fn parse_origin(s: &str) -> Option<OriginSpec> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.as_slice() {
        [single] => {
            let low = single.to_ascii_lowercase();
            match low.as_str() {
                "top" => Some(OriginSpec {
                    x: CoordVal::Percent(50.0),
                    y: CoordVal::Percent(0.0),
                }),
                "bottom" => Some(OriginSpec {
                    x: CoordVal::Percent(50.0),
                    y: CoordVal::Percent(100.0),
                }),
                "left" => Some(OriginSpec {
                    x: CoordVal::Percent(0.0),
                    y: CoordVal::Percent(50.0),
                }),
                "right" => Some(OriginSpec {
                    x: CoordVal::Percent(100.0),
                    y: CoordVal::Percent(50.0),
                }),
                "center" => Some(OriginSpec {
                    x: CoordVal::Percent(50.0),
                    y: CoordVal::Percent(50.0),
                }),
                _ => {
                    let x = parse_coord_val(single)?;
                    Some(OriginSpec {
                        x,
                        y: CoordVal::Percent(50.0),
                    })
                }
            }
        }
        [a, b, ..] => {
            let (mut a, mut b) = (*a, *b);
            if matches!(a.to_ascii_lowercase().as_str(), "top" | "bottom")
                || matches!(b.to_ascii_lowercase().as_str(), "left" | "right")
            {
                std::mem::swap(&mut a, &mut b);
            }
            Some(OriginSpec {
                x: parse_coord_val(a)?,
                y: parse_coord_val(b)?,
            })
        }
        [] => None,
    }
}

pub fn parse_angle_deg(s: &str) -> Option<f32> {
    let low = s.trim().to_ascii_lowercase();
    if let Some(v) = low.strip_suffix("deg") {
        v.trim().parse::<f32>().ok()
    } else if let Some(v) = low.strip_suffix("rad") {
        v.trim().parse::<f32>().ok().map(f32::to_degrees)
    } else if let Some(v) = low.strip_suffix("turn") {
        v.trim().parse::<f32>().ok().map(|t| t * 360.0)
    } else if let Some(v) = low.strip_suffix("grad") {
        v.trim().parse::<f32>().ok().map(|g| g * 0.9)
    } else {
        low.parse::<f32>().ok()
    }
}

/// Parse both CSS `transform` lists (`translateY(-470px) rotate(-38deg)`) and
/// SVG `transform` attributes (`translate(512 512) rotate(30 140 140)`).
pub fn parse_transform_list(raw: &str) -> Option<Vec<TransformOp>> {
    let s = raw.trim();
    if s.eq_ignore_ascii_case("none") || s.is_empty() {
        return Some(Vec::new());
    }
    let mut ops = Vec::new();
    let mut rest = s;
    while !rest.trim().is_empty() {
        let trimmed = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        let open = trimmed.find('(')?;
        let close = trimmed[open..].find(')')? + open;
        let func = trimmed[..open].trim().to_ascii_lowercase();
        let args_str = &trimmed[open + 1..close];
        rest = &trimmed[close + 1..];

        let args: Vec<&str> = args_str
            .split(|c: char| c.is_ascii_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .collect();

        match func.as_str() {
            "translate" => {
                let tx = parse_length(args.first().copied()?)?;
                let ty = args.get(1).and_then(|v| parse_length(v)).unwrap_or(0.0);
                ops.push(TransformOp::Translate(tx, ty));
            }
            "translatex" => {
                ops.push(TransformOp::Translate(
                    parse_length(args.first().copied()?)?,
                    0.0,
                ));
            }
            "translatey" => {
                ops.push(TransformOp::Translate(
                    0.0,
                    parse_length(args.first().copied()?)?,
                ));
            }
            "scale" => {
                let sx = parse_scale_arg(args.first().copied()?)?;
                let sy = args.get(1).and_then(|v| parse_scale_arg(v)).unwrap_or(sx);
                ops.push(TransformOp::Scale(sx, sy));
            }
            "scalex" => {
                ops.push(TransformOp::Scale(
                    parse_scale_arg(args.first().copied()?)?,
                    1.0,
                ));
            }
            "scaley" => {
                ops.push(TransformOp::Scale(
                    1.0,
                    parse_scale_arg(args.first().copied()?)?,
                ));
            }
            "rotate" => {
                let deg = parse_angle_deg(args.first().copied()?)?;
                if args.len() >= 3 {
                    let cx = parse_length(args[1])?;
                    let cy = parse_length(args[2])?;
                    ops.push(TransformOp::Translate(cx, cy));
                    ops.push(TransformOp::Rotate(deg));
                    ops.push(TransformOp::Translate(-cx, -cy));
                } else {
                    ops.push(TransformOp::Rotate(deg));
                }
            }
            "skewx" => ops.push(TransformOp::SkewX(parse_angle_deg(args.first().copied()?)?)),
            "skewy" => ops.push(TransformOp::SkewY(parse_angle_deg(args.first().copied()?)?)),
            "matrix" if args.len() == 6 => {
                let mut m = [0.0f32; 6];
                for (dst, src) in m.iter_mut().zip(&args) {
                    *dst = src.parse::<f32>().ok()?;
                }
                ops.push(TransformOp::Matrix(m));
            }
            _ => return None,
        }
    }
    Some(ops)
}

fn parse_scale_arg(s: &str) -> Option<f32> {
    if let Some(pct) = s.trim().strip_suffix('%') {
        pct.trim().parse::<f32>().ok().map(|v| v * 0.01)
    } else {
        s.trim().parse::<f32>().ok()
    }
}

pub fn parse_paint(val: &str, grad_ids: &HashMap<String, u16>) -> Option<ColorSpec> {
    let t = val.trim();
    let low = t.to_ascii_lowercase();
    if low == "none" {
        return Some(ColorSpec::None);
    }
    if low == "transparent" {
        return Some(ColorSpec::Rgba([0, 0, 0, 0]));
    }
    if low == "currentcolor" {
        return Some(ColorSpec::CurrentColor);
    }
    if low.starts_with("url(") && t.ends_with(')') {
        let orig_inner = &t[4..t.len() - 1];
        let id = orig_inner
            .trim()
            .trim_matches(['"', '\''])
            .strip_prefix('#')?;
        return grad_ids.get(id).copied().map(ColorSpec::Gradient);
    }
    if low.starts_with("light-dark(") && t.ends_with(')') {
        let inner = &t["light-dark(".len()..t.len() - 1];
        let parts = split_top_commas(inner);
        if parts.len() == 2 {
            let a = parse_paint(parts[0], grad_ids)?;
            let b = parse_paint(parts[1], grad_ids)?;
            return Some(ColorSpec::LightDark(Box::new((a, b))));
        }
    }
    parse_rgba_color(t).map(ColorSpec::Rgba)
}

pub fn parse_rgba_color(s: &str) -> Option<[u8; 4]> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix('#') {
        let bytes = hex.as_bytes();
        let h = |b: u8| -> Option<u8> {
            match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            }
        };
        return match bytes.len() {
            3 => Some([h(bytes[0])? * 17, h(bytes[1])? * 17, h(bytes[2])? * 17, 255]),
            4 => Some([
                h(bytes[0])? * 17,
                h(bytes[1])? * 17,
                h(bytes[2])? * 17,
                h(bytes[3])? * 17,
            ]),
            6 => Some([
                (h(bytes[0])? << 4) | h(bytes[1])?,
                (h(bytes[2])? << 4) | h(bytes[3])?,
                (h(bytes[4])? << 4) | h(bytes[5])?,
                255,
            ]),
            8 => Some([
                (h(bytes[0])? << 4) | h(bytes[1])?,
                (h(bytes[2])? << 4) | h(bytes[3])?,
                (h(bytes[4])? << 4) | h(bytes[5])?,
                (h(bytes[6])? << 4) | h(bytes[7])?,
            ]),
            _ => None,
        };
    }

    let low = t.to_ascii_lowercase();
    if let Some(inner) = low
        .strip_prefix("rgba(")
        .or_else(|| low.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'))
    {
        let (rgb_part, alpha_slash) = match inner.split_once('/') {
            Some((a, b)) => (a, Some(b)),
            None => (inner, None),
        };
        let parts: Vec<&str> = rgb_part
            .split(|c: char| c.is_ascii_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let chan = |p: &str| -> Option<u8> {
            if let Some(pct) = p.strip_suffix('%') {
                Some(
                    (pct.trim().parse::<f32>().ok()? * 2.55)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                )
            } else {
                Some(p.trim().parse::<f32>().ok()?.round().clamp(0.0, 255.0) as u8)
            }
        };
        let r = chan(parts[0])?;
        let g = chan(parts[1])?;
        let b = chan(parts[2])?;
        let a = match alpha_slash.or_else(|| parts.get(3).copied()) {
            Some(a_str) => (parse_unit_alpha(a_str)? * 255.0).round() as u8,
            None => 255,
        };
        return Some([r, g, b, a]);
    }

    match low.as_str() {
        "black" => Some([0, 0, 0, 255]),
        "white" => Some([255, 255, 255, 255]),
        "red" => Some([255, 0, 0, 255]),
        "green" => Some([0, 128, 0, 255]),
        "lime" => Some([0, 255, 0, 255]),
        "blue" => Some([0, 0, 255, 255]),
        "yellow" => Some([255, 255, 0, 255]),
        "cyan" | "aqua" => Some([0, 255, 255, 255]),
        "magenta" | "fuchsia" => Some([255, 0, 255, 255]),
        "gray" | "grey" => Some([128, 128, 128, 255]),
        "silver" => Some([192, 192, 192, 255]),
        "orange" => Some([255, 165, 0, 255]),
        "purple" => Some([128, 0, 128, 255]),
        "pink" => Some([255, 192, 203, 255]),
        "navy" => Some([0, 0, 128, 255]),
        "teal" => Some([0, 128, 128, 255]),
        _ => None,
    }
}
