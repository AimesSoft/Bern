//! Bern Surface IR (`bern-surface`).
//!
//! A platform-independent intermediate representation for Bern's self-drawn
//! backends — currently Web (`bern-backend-web`) and GPUI (`bern-gpui`), and
//! extensible to future canvas/GPU renderers (Skia, Vello, wgpu, SVG).
//!
//! Native targets (SwiftUI, WinUI 3, GTK 4, Jetpack Compose) stay on their
//! system controls and never use this crate. Self-drawn backends lower every
//! control once through [`controls`] into [`SurfaceNode`] box-model trees and
//! resolve every color through [`SurfacePalette`] (built over
//! [`bern_theme::Snapshot`]), so Web DOM and GPUI render identical geometry,
//! spacing, radii, typography, translucency, shadows, and interaction states.

pub mod controls;
pub mod ir;
pub mod palette;
pub mod web;

#[cfg(test)]
mod tests;

pub use ir::{
    Align, AriaRole, Axis, Border, BorderSides, BoxNode, ColorValue, Corners, Cursor, Display,
    Edges, IconNode, Justify, Layout, Length, Overflow, Position, Rgba8, RingNode, Shadow, SlotId,
    StateOverrides, SurfaceNode, SurfaceTree, TextNode, TrackNode, Typography, VisualStyle,
};
pub use palette::{DEFAULT_GLASS_DARK, DEFAULT_GLASS_LIGHT, GlassTokens, SurfacePalette};

/// Version of the `bern-surface` box-model IR schema.
pub const IR_VERSION: u32 = 1;

/// Adapter trait implemented by self-drawn backends (GPUI, Web DOM/CSS, Skia,
/// Vello, headless snapshot testers) to render a lowered [`SurfaceNode`] tree.
pub trait SurfaceRenderer {
    type Output;

    fn render_node(&mut self, node: &SurfaceNode, palette: &SurfacePalette) -> Self::Output;

    fn render_tree(&mut self, tree: &SurfaceTree, palette: &SurfacePalette) -> Self::Output {
        self.render_node(&tree.root, palette)
    }
}

/// Deterministic structural printer implementing [`SurfaceRenderer`], useful
/// for snapshot tests, diagnostics, and custom backend prototyping.
#[derive(Default)]
pub struct DebugRenderer {
    indent: usize,
}

impl DebugRenderer {
    pub fn render_to_string(tree: &SurfaceTree, palette: &SurfacePalette) -> String {
        let mut renderer = Self::default();
        renderer.render_tree(tree, palette)
    }
}

impl SurfaceRenderer for DebugRenderer {
    type Output = String;

    fn render_node(&mut self, node: &SurfaceNode, palette: &SurfacePalette) -> String {
        let pad = "  ".repeat(self.indent);
        match node {
            SurfaceNode::Box(b) => {
                let mut out = format!(
                    "{pad}Box(part={:?}, role={:?}, dir={:?}",
                    b.part, b.role, b.layout.direction
                );
                if let Some(bg) = b.style.background {
                    let c = palette.resolve(bg);
                    out.push_str(&format!(", bg={}", c.css()));
                }
                if let Some(c) = b.style.corners {
                    out.push_str(&format!(", r={:.1}", c.tl));
                }
                out.push(')');
                if !b.children.is_empty() {
                    out.push_str(" {\n");
                    self.indent += 1;
                    for child in &b.children {
                        out.push_str(&self.render_node(child, palette));
                        out.push('\n');
                    }
                    self.indent -= 1;
                    out.push_str(&format!("{pad}}}"));
                }
                out
            }
            SurfaceNode::Text(t) => format!(
                "{pad}Text({:?}, size={:.1}, weight={}, color={})",
                t.content,
                t.typography.size,
                t.typography.weight,
                palette.resolve(t.color).css()
            ),
            SurfaceNode::Icon(i) => format!(
                "{pad}Icon({:?}, size={:.1}, color={})",
                i.name,
                i.size,
                palette.resolve(i.color).css()
            ),
            SurfaceNode::Track(t) => format!(
                "{pad}Track(axis={:?}, thickness={:.1}, fraction={:.3}, thumb={})",
                t.axis,
                t.thickness,
                t.fraction,
                t.thumb.is_some()
            ),
            SurfaceNode::Ring(r) => format!(
                "{pad}Ring(diameter={:.1}, stroke={:.1}, fraction={:.3})",
                r.diameter, r.stroke_width, r.fraction
            ),
            SurfaceNode::Spacer => format!("{pad}Spacer"),
            SurfaceNode::Slot(slot) => format!("{pad}Slot({})", slot.0),
        }
    }
}
