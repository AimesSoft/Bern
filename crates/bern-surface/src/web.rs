//! Web DOM/CSS compiler over the `bern-surface` IR.
//!
//! Translates [`SurfacePalette`] into CSS `--bern-*` custom properties and
//! lowers canonical [`crate::controls`] [`SurfaceNode`] trees into the shared
//! self-drawn CSS rules that keep Web DOM visually locked to GPUI while
//! preserving native HTML semantics (`<input>`, `<button>`, `<dialog>`,
//! Chinese IME, browser scroll physics, and WAI-ARIA).

use crate::controls::{
    self, BadgeSpec, BadgeTone, BannerSeverity, BannerSpec, BottomSheetSpec, ButtonSpec,
    CheckboxSpec, DisclosureSpec, EmptyStateSpec, NavLinkSpec, PickerChipsSpec,
    PickerSegmentedSpec, ProgressSpec, ReorderRowSpec, SectionSpec, SidebarItemSpec, SidebarSpec,
    SliderSpec, SwitchSpec, TableSpec, ToastSpec, TooltipSpec, TreeRowSpec,
};
use crate::ir::{
    Axis, Border, ColorValue, Corners, Edges, Length, Shadow, SurfaceNode, VisualStyle,
};
use crate::palette::SurfacePalette;
use bern_theme::Snapshot;
use std::fmt::Write as _;

/// Convert a semantic token name into its CSS custom property name.
/// Example: `"slider.thumb.border"` -> `"--bern-slider-thumb-border"`.
pub fn css_var(token: &str) -> String {
    format!("--bern-{}", token.replace(['.', '_'], "-"))
}

/// Serialize a [`ColorValue`] as a CSS expression (`var(--bern-...)`,
/// `color-mix(...)`, `transparent`, or literal RGBA).
pub fn css_color(color: ColorValue) -> String {
    match color {
        ColorValue::Transparent => "transparent".into(),
        ColorValue::Role(role) => format!("var({})", css_var(role)),
        ColorValue::RoleAlpha(role, alpha) => {
            let pct = (alpha.clamp(0.0, 1.0) * 100.0).round() as u32;
            format!(
                "color-mix(in srgb, var({}) {pct}%, transparent)",
                css_var(role)
            )
        }
        ColorValue::Recipe {
            recipe,
            state,
            slot,
        } => {
            let role = SurfacePalette::slot_role(recipe, state, slot);
            if role == "transparent" {
                "transparent".into()
            } else {
                format!("var({})", css_var(role))
            }
        }
        ColorValue::Fixed { light, .. } => light.css(),
        ColorValue::Rgba(rgba) => rgba.css(),
    }
}

/// Emit CSS custom-property declarations for one resolved [`Snapshot`] using
/// [`SurfacePalette`] (including translucent overlays, separators, sidebar tint,
/// white vertical pill slider tokens, and glass tokens).
pub fn theme_declarations(snapshot: &Snapshot) -> String {
    let palette = SurfacePalette::from_snapshot(snapshot.clone());
    let mut css = format!("  color-scheme: {};\n", snapshot.mode.name());
    for (name, rgba) in palette.resolved_tokens() {
        let _ = writeln!(css, "  {}: {};", css_var(name), rgba.css());
    }
    css
}

fn px(v: f32) -> String {
    if (v - v.round()).abs() < 1e-4 {
        format!("{}px", v.round() as i32)
    } else {
        let s = format!("{v:.2}");
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        format!("{trimmed}px")
    }
}

fn format_edges(e: Edges) -> String {
    if (e.top - e.bottom).abs() < 1e-4 && (e.left - e.right).abs() < 1e-4 {
        if (e.top - e.left).abs() < 1e-4 {
            px(e.top)
        } else {
            format!("{} {}", px(e.top), px(e.left))
        }
    } else {
        format!(
            "{} {} {} {}",
            px(e.top),
            px(e.right),
            px(e.bottom),
            px(e.left)
        )
    }
}

fn format_corners(c: Corners) -> String {
    if c.is_pill() {
        "999px".into()
    } else if c.is_uniform() {
        px(c.tl)
    } else {
        format!("{} {} {} {}", px(c.tl), px(c.tr), px(c.br), px(c.bl))
    }
}

fn format_shadow(s: &Shadow) -> String {
    let prefix = if s.inset { "inset " } else { "" };
    format!(
        "{prefix}{} {} {} {} {}",
        px(s.offset_x),
        px(s.offset_y),
        px(s.blur),
        px(s.spread),
        css_color(s.color)
    )
}

fn emit_border(out: &mut String, border: Border) {
    let stroke = format!("{} solid {}", px(border.width), css_color(border.color));
    let s = border.sides;
    if s.top && s.right && s.bottom && s.left {
        let _ = writeln!(out, "  border: {stroke};");
    } else {
        if s.top {
            let _ = writeln!(out, "  border-top: {stroke};");
        }
        if s.right {
            let _ = writeln!(out, "  border-right: {stroke};");
        }
        if s.bottom {
            let _ = writeln!(out, "  border-bottom: {stroke};");
        }
        if s.left {
            let _ = writeln!(out, "  border-left: {stroke};");
        }
    }
}

fn emit_visual_style(out: &mut String, style: &VisualStyle) {
    if let Some(bg) = style.background {
        let _ = writeln!(out, "  background: {};", css_color(bg));
    }
    if let Some(fg) = style.foreground {
        let _ = writeln!(out, "  color: {};", css_color(fg));
    }
    if let Some(border) = style.border {
        emit_border(out, border);
    }
    if let Some(corners) = style.corners {
        let _ = writeln!(out, "  border-radius: {};", format_corners(corners));
    }
    if !style.shadows.is_empty() {
        let list: Vec<String> = style.shadows.iter().map(format_shadow).collect();
        let _ = writeln!(out, "  box-shadow: {};", list.join(", "));
    }
    if let Some(opacity) = style.opacity {
        let _ = writeln!(out, "  opacity: {opacity};");
    }
    if let Some(typo) = style.typography {
        let _ = writeln!(
            out,
            "  font-size: calc({} + var(--bern-text-up));",
            px(typo.size)
        );
        if let Some(lh) = typo.line_height {
            let _ = writeln!(out, "  line-height: {};", px(lh));
        }
        let _ = writeln!(out, "  font-weight: {};", typo.weight);
        if typo.tabular_nums {
            let _ = writeln!(out, "  font-variant-numeric: tabular-nums;");
        }
    }
}

fn emit_box_visual_rule(
    out: &mut String,
    selector: &str,
    node: &SurfaceNode,
    include_padding: bool,
) {
    let SurfaceNode::Box(b) = node else {
        return;
    };
    let _ = writeln!(out, "{selector} {{");
    if include_padding && !b.layout.padding.is_zero() {
        let _ = writeln!(out, "  padding: {};", format_edges(b.layout.padding));
    }
    if b.layout.gap > 0.0 {
        let _ = writeln!(out, "  gap: {};", px(b.layout.gap));
    }
    if let Length::Px(h) = b.layout.height {
        let _ = writeln!(out, "  height: {};", px(h));
    }
    emit_visual_style(out, &b.style);
    let _ = writeln!(out, "}}\n");
}

/// Emit the canonical self-drawn CSS stylesheet generated directly from
/// `bern-surface` IR trees. Appended after Web's structural/responsive CSS
/// so every self-drawn control matches GPUI in geometry, colors, shadows,
/// translucency, and the white vertical pill slider thumb.
pub fn stylesheet() -> String {
    let mut css = format!(
        "/* Generated by bern-surface IR v{}. Single source of truth shared with GPUI. */\n\n",
        crate::IR_VERSION
    );

    // 1. Canonical geometry custom properties derived directly from IR constants.
    let h_thumb = controls::slider_thumb(Axis::Horizontal, true);
    let thumb_w = match h_thumb.layout.width {
        Length::Px(v) => v,
        _ => controls::SLIDER_THUMB_THICKNESS,
    };
    let thumb_h = match h_thumb.layout.height {
        Length::Px(v) => v,
        _ => controls::SLIDER_THUMB_LENGTH,
    };
    let thumb_shadow = h_thumb
        .style
        .shadows
        .first()
        .map(format_shadow)
        .unwrap_or_else(|| "0 1px 4px var(--bern-slider-thumb-shadow)".into());
    let _ = writeln!(
        css,
        ":root {{\n  --bern-track: {};\n  --bern-slider-thumb: {};\n  --bern-slider-thumb-w: {};\n  --bern-slider-thumb-h: {};\n  --bern-slider-thumb-radius: {};\n  --bern-slider-thumb-scale: {};\n  --bern-slider-thumb-shadow-box: {};\n  --bern-switch-width: {};\n  --bern-switch-height: {};\n  --bern-switch-thumb: {};\n  --bern-nav-row: {};\n  --bern-radius-card: {};\n  --bern-radius-dialog: {};\n}}\n",
        px(controls::SLIDER_TRACK_THICKNESS),
        px(thumb_w),
        px(thumb_w),
        px(thumb_h),
        px(controls::SLIDER_THUMB_RADIUS),
        controls::SLIDER_THUMB_SCALE_ACTIVE,
        thumb_shadow,
        px(controls::SWITCH_WIDTH),
        px(controls::SWITCH_HEIGHT),
        px(controls::SWITCH_KNOB),
        px(controls::navigation::SIDEBAR_ITEM_HEIGHT),
        px(controls::form::SECTION_CARD_CORNER),
        px(controls::overlay::MODAL_CORNER),
    );
    let _ = writeln!(
        css,
        "@media (pointer: coarse) {{\n  :root {{\n    --bern-slider-thumb: 10px;\n    --bern-slider-thumb-w: 10px;\n    --bern-slider-thumb-h: 24px;\n  }}\n}}\n"
    );

    // 2. Slider (horizontal & vertical white vertical pill thumb from controls::slider IR).
    let slider_node = controls::slider(SliderSpec {
        label_hidden: true,
        enabled: true,
        ..SliderSpec::default()
    });
    if let SurfaceNode::Box(track_row) = &slider_node {
        let _ = writeln!(
            css,
            ".bern-slider-track {{\n  gap: {};\n}}\n",
            px(track_row.layout.gap)
        );
    }
    let _ = writeln!(
        css,
        ".bern-slider::-webkit-slider-runnable-track {{\n  height: var(--bern-track);\n  border-radius: calc(var(--bern-track) / 2);\n  background: linear-gradient(to right, var(--bern-accent) var(--bern-fill), var(--bern-separator) var(--bern-fill));\n}}\n\
.bern-slider::-moz-range-track {{\n  height: var(--bern-track);\n  border-radius: calc(var(--bern-track) / 2);\n  background: var(--bern-separator);\n}}\n\
.bern-slider::-moz-range-progress {{\n  height: var(--bern-track);\n  border-radius: calc(var(--bern-track) / 2);\n  background: var(--bern-accent);\n}}\n\
.bern-slider::-webkit-slider-thumb {{\n  box-sizing: border-box;\n  width: var(--bern-slider-thumb-w);\n  height: var(--bern-slider-thumb-h);\n  margin-top: calc((var(--bern-track) - var(--bern-slider-thumb-h)) / 2);\n  border: 1px solid var(--bern-slider-thumb-border);\n  border-radius: var(--bern-slider-thumb-radius);\n  background: var(--bern-slider-thumb);\n  box-shadow: var(--bern-slider-thumb-shadow-box);\n  appearance: none;\n  -webkit-appearance: none;\n  transition: transform var(--bern-motion) ease-out, box-shadow var(--bern-motion) ease-out;\n}}\n\
.bern-slider::-moz-range-thumb {{\n  box-sizing: border-box;\n  width: var(--bern-slider-thumb-w);\n  height: var(--bern-slider-thumb-h);\n  border: 1px solid var(--bern-slider-thumb-border);\n  border-radius: var(--bern-slider-thumb-radius);\n  background: var(--bern-slider-thumb);\n  box-shadow: var(--bern-slider-thumb-shadow-box);\n  transition: transform var(--bern-motion) ease-out, box-shadow var(--bern-motion) ease-out;\n}}\n\
.bern-slider-vertical::-webkit-slider-thumb {{\n  width: var(--bern-slider-thumb-h);\n  height: var(--bern-slider-thumb-w);\n  margin-top: 0;\n  margin-left: calc((var(--bern-track) - var(--bern-slider-thumb-h)) / 2);\n}}\n\
.bern-slider-vertical::-moz-range-thumb {{\n  width: var(--bern-slider-thumb-h);\n  height: var(--bern-slider-thumb-w);\n}}\n\
@media (hover: hover) {{\n  .bern-slider:hover:not(:disabled)::-webkit-slider-thumb {{\n    transform: scale(var(--bern-slider-thumb-scale));\n  }}\n  .bern-slider:hover:not(:disabled)::-moz-range-thumb {{\n    transform: scale(var(--bern-slider-thumb-scale));\n  }}\n}}\n\
.bern-slider:active:not(:disabled)::-webkit-slider-thumb {{\n  transform: scale(var(--bern-slider-thumb-scale));\n}}\n\
.bern-slider:active:not(:disabled)::-moz-range-thumb {{\n  transform: scale(var(--bern-slider-thumb-scale));\n}}\n\
.bern-slider:disabled::-webkit-slider-thumb {{\n  border-color: var(--bern-disabled-foreground);\n  background: var(--bern-surface-raised);\n  box-shadow: none;\n  opacity: 0.85;\n}}\n\
.bern-slider:disabled::-moz-range-thumb {{\n  border-color: var(--bern-disabled-foreground);\n  background: var(--bern-surface-raised);\n  box-shadow: none;\n  opacity: 0.85;\n}}\n"
    );

    // 3. Translucent Sidebar & Navigation surfaces from controls::navigation IR.
    let sb = controls::sidebar(SidebarSpec::default());
    if let SurfaceNode::Box(b) = &sb
        && let Some(bg) = b.style.background
    {
        let _ = writeln!(
            css,
            ".bern-sidebar,\n.bern-tabs-vertical .bern-tabbar {{\n  background: {};\n}}\n",
            css_color(bg)
        );
    }
    let nav = controls::nav_link(NavLinkSpec {
        in_section: false,
        enabled: true,
    });
    if let SurfaceNode::Box(b) = &nav {
        let _ = writeln!(
            css,
            ".bern-navlink {{\n  gap: {};\n  padding: {};\n  border-radius: {};\n}}\n",
            px(b.layout.gap),
            format_edges(b.layout.padding),
            format_corners(b.style.corners.unwrap_or(Corners::all(8.0)))
        );
    }
    let sb_item = controls::sidebar_item(SidebarItemSpec {
        enabled: true,
        ..SidebarItemSpec::default()
    });
    if let SurfaceNode::Box(b) = &sb_item {
        let _ = writeln!(
            css,
            ".bern-sidebar-item {{\n  gap: {};\n  padding: {};\n  border-radius: {};\n}}\n",
            px(b.layout.gap),
            format_edges(b.layout.padding),
            format_corners(b.style.corners.unwrap_or(Corners::all(8.0)))
        );
    }

    // 4. Lowered control surface rules emitted directly from SurfaceNode specs.
    let sw = controls::switch(SwitchSpec {
        label_hidden: true,
        enabled: true,
        ..SwitchSpec::default()
    });
    let _ = &sw;
    let cb = controls::checkbox(CheckboxSpec {
        label_hidden: true,
        enabled: true,
        ..CheckboxSpec::default()
    });
    emit_box_visual_rule(&mut css, ".bern-checkbox", &cb, false);

    let btn = controls::button(ButtonSpec {
        enabled: true,
        ..ButtonSpec::default()
    });
    if let SurfaceNode::Box(b) = &btn {
        let _ = writeln!(
            css,
            ".bern-button {{\n  gap: {};\n  padding: {};\n  border-radius: {};\n}}\n",
            px(b.layout.gap),
            format_edges(b.layout.padding),
            format_corners(b.style.corners.unwrap_or(Corners::all(8.0)))
        );
    }

    let sec = controls::section(SectionSpec::default());
    if let SurfaceNode::Box(outer) = &sec
        && let Some(group) = outer.children.first()
    {
        emit_box_visual_rule(&mut css, ".bern-group", group, false);
    }

    let badge_neutral = controls::badge(BadgeSpec {
        tone: BadgeTone::Neutral,
        ..BadgeSpec::default()
    });
    emit_box_visual_rule(&mut css, ".bern-badge", &badge_neutral, true);
    for (suffix, tone) in [
        ("accent", BadgeTone::Accent),
        ("success", BadgeTone::Success),
        ("warning", BadgeTone::Warning),
        ("error", BadgeTone::Error),
    ] {
        let node = controls::badge(BadgeSpec {
            tone,
            ..BadgeSpec::default()
        });
        emit_box_visual_rule(&mut css, &format!(".bern-badge-{suffix}"), &node, false);
    }

    let banner_info = controls::banner(BannerSpec {
        severity: BannerSeverity::Info,
        ..BannerSpec::default()
    });
    emit_box_visual_rule(&mut css, ".bern-banner", &banner_info, true);
    for (suffix, sev) in [
        ("success", BannerSeverity::Success),
        ("warning", BannerSeverity::Warning),
        ("error", BannerSeverity::Error),
    ] {
        let node = controls::banner(BannerSpec {
            severity: sev,
            ..BannerSpec::default()
        });
        emit_box_visual_rule(&mut css, &format!(".bern-banner-{suffix}"), &node, false);
    }

    let tip = controls::tooltip(TooltipSpec::default());
    emit_box_visual_rule(&mut css, ".bern-tooltip", &tip, true);

    let toast_node = controls::toast(ToastSpec::default());
    emit_box_visual_rule(&mut css, ".bern-toast", &toast_node, true);

    let empty = controls::empty_state(EmptyStateSpec::default());
    emit_box_visual_rule(&mut css, ".bern-empty-state", &empty, true);

    let sheet = controls::bottom_sheet(BottomSheetSpec {
        width: 640.0,
        max_height: 0.0,
        drag_indicator: true,
    });
    if let SurfaceNode::Box(b) = &sheet {
        let _ = writeln!(
            css,
            "dialog.bern-bottom-sheet {{\n  padding: {};\n  border-radius: {};\n}}\n",
            format_edges(b.layout.padding),
            format_corners(b.style.corners.unwrap_or(Corners::top(18.0)))
        );
    }

    let _ = (
        controls::disclosure(DisclosureSpec::default()),
        controls::picker_chips(PickerChipsSpec::default()),
        controls::picker_segmented(PickerSegmentedSpec::default()),
        controls::progress(ProgressSpec::default()),
        controls::table(TableSpec),
        controls::tree_row(TreeRowSpec::default()),
        controls::reorder_row(ReorderRowSpec::default()),
    );

    css
}
