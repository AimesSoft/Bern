//! The tables of `style!`: motion tokens, transitions, patterns and
//! style overrides, and the theme the `WindowSet` chooses at run time.

use super::{Generator, Out, PLATFORM, float, ident, motion_ref, string};
use bern_ir::style::{ColorRef, ControlSize, StyleEffect, TransitionKind};
use bern_protocol::Type;
use bern_target_api::style::Resolved;

/// A color of `style!` or a role keyword as a `B::Paint`.
fn paint(color: &ColorRef) -> String {
    match color {
        ColorRef::Role { role } => format!("B::Paint{{{}}}", string(role)),
        ColorRef::Fixed { light, dark } => format!(
            "B::Paint{{nullptr, 0x{:02X}{:02X}{:02X}u, 0x{:02X}{:02X}{:02X}u}}",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

impl Generator<'_> {
    /// One look of a style: a state's colors and transforms, assigned
    /// through `target` (`s.base.` or `s.hover->`).
    fn look(out: &mut Out, target: &str, effects: &[StyleEffect]) {
        for effect in effects {
            match effect {
                StyleEffect::Background { color } => {
                    out.line(format!("{target}background = {};", paint(color)))
                }
                StyleEffect::Foreground { color } => {
                    out.line(format!("{target}foreground = {};", paint(color)))
                }
                StyleEffect::Border { color, width } => {
                    out.line(format!("{target}border = {};", paint(color)));
                    out.line(format!("{target}borderWidth = {};", float(*width)));
                }
                StyleEffect::Fill { color } => {
                    out.line(format!("{target}fill = {};", paint(color)))
                }
                StyleEffect::Indicator { color, edge, width } => {
                    out.line(format!("{target}indicator = {};", paint(color)));
                    out.line(format!(
                        "{target}indicatorEdge = {};",
                        string(&format!("{edge:?}").to_lowercase())
                    ));
                    out.line(format!("{target}indicatorWidth = {};", float(*width)));
                }
                StyleEffect::Scale { x, y, motion } => {
                    out.line(format!("{target}scaleX = {};", float(*x)));
                    out.line(format!("{target}scaleY = {};", float(*y)));
                    out.line(format!("{target}motion = {};", motion_ref(motion)));
                }
                StyleEffect::Offset { x, y, motion } => {
                    out.line(format!("{target}offsetX = {};", float(*x)));
                    out.line(format!("{target}offsetY = {};", float(*y)));
                    out.line(format!("{target}motion = {};", motion_ref(motion)));
                }
                StyleEffect::Opacity { value, motion } => {
                    out.line(format!("{target}opacity = {};", float(*value)));
                    out.line(format!("{target}motion = {};", motion_ref(motion)));
                }
                StyleEffect::Shadow { radius, y, opacity } => out.line(format!(
                    "{target}shadow = std::array<double, 3>{{{}, {}, {}}};",
                    float(*radius),
                    float(*y),
                    float(*opacity)
                )),
                StyleEffect::System => out.line(format!("{target}system = true;")),
                StyleEffect::Nothing => out.line(format!("{target}quiet = true;")),
                StyleEffect::Corner { .. }
                | StyleEffect::Padding { .. }
                | StyleEffect::Size { .. } => {}
            }
        }
    }

    /// The motion tokens (`M_`), transitions (`X_`), patterns (`P_`) and
    /// style overrides (`S_`) of `style!`.
    pub(super) fn style_tables(&self, out: &mut Out) {
        let style = &self.app.style;
        out.line("// Motion tokens: period and damping ratio of each spring.");
        for token in &style.motion {
            out.line(format!(
                "[[maybe_unused]] static B::Motion const M_{}{{{}, {}}};",
                ident(&token.name),
                float(token.period),
                float(token.damping)
            ));
        }
        out.line("// Transitions of `appear` and `disappear`.");
        for transition in &style.transitions {
            let motion = motion_ref(&transition.motion);
            let body = match &transition.kind {
                TransitionKind::Fade => format!(".kind = B::Transition::Fade, .motion = {motion}"),
                // Rejected by the edge_grow capability check before generation.
                TransitionKind::Grow { .. } => continue,
                TransitionKind::Pop { from } => format!(
                    ".kind = B::Transition::Pop, .from = {}, .motion = {motion}",
                    float(*from)
                ),
                TransitionKind::Slide { edge, distance } => format!(
                    ".kind = B::Transition::Slide, .edge = {}, .distance = {}, .motion = {motion}",
                    string(&format!("{edge:?}").to_lowercase()),
                    float(*distance)
                ),
            };
            out.line(format!(
                "[[maybe_unused]] static B::Transition const X_{}{{{body}}};",
                ident(&transition.name)
            ));
        }
        out.line("// Patterns of `style!`.");
        for pattern in &style.patterns {
            let bands: Vec<String> = pattern
                .bands
                .iter()
                .map(|band| {
                    format!(
                        "B::Band{{{}, {}, {}}}",
                        band.color
                            .as_ref()
                            .map(paint)
                            .unwrap_or_else(|| "B::Paint{}".into()),
                        float(band.width),
                        float(band.opacity)
                    )
                })
                .collect();
            out.line(format!(
                "[[maybe_unused]] static B::PatternDef const P_{}{{{{{}}}, {}}};",
                ident(&pattern.name),
                bands.join(", "),
                pattern.twill
            ));
        }
        out.line("// Style overrides of `style!`, as this target shows them.");
        for style in bern_target_api::style::used(self.app, PLATFORM) {
            Self::style_def(out, &style);
        }
        out.line("");
    }

    /// One style override, `S_name`: its look outside any state and in
    /// each state, and its corner, padding, size and settling motion.
    fn style_def(out: &mut Out, style: &Resolved) {
        out.open(format!(
            "[[maybe_unused]] static B::StyleDef const S_{} = [] {{",
            style.name
        ));
        out.line("B::StyleDef s;");
        Self::look(out, "s.base.", &style.base);
        for effect in &style.base {
            match effect {
                StyleEffect::Corner { radius } => {
                    out.line(format!("s.corner = {};", float(*radius)))
                }
                StyleEffect::Padding { length } => {
                    out.line(format!("s.padding = {};", float(*length)))
                }
                _ => {}
            }
        }
        if let Some(size) = style.size() {
            let size = match size {
                ControlSize::Small => "small",
                ControlSize::Regular => "regular",
                ControlSize::Large => "large",
                ControlSize::ExtraLarge => "extra_large",
            };
            out.line(format!("s.size = {};", string(size)));
        }
        if style.draws_look() {
            out.line("s.draws = true;");
        }
        for (state, effects) in &style.states {
            let name = state.name();
            out.line(format!("s.{name} = B::Look{{}};"));
            Self::look(out, &format!("s.{name}->"), effects);
        }
        if let Some(settle) = &style.settle {
            out.line(format!("s.settle = {};", motion_ref(settle)));
        }
        out.line("return s;");
        out.close("}();");
    }

    /// The accent and appearance the `WindowSet` chooses at run time.
    pub(super) fn theme_functions(&self, out: &mut Out) {
        let windows = &self.app.windows;
        if let Some(accent) = &windows.accent {
            out.line(
                "/// The accent of `WindowSet(accent:)`: its index in the accents of `style!`.",
            );
            out.open("static int ThemeAccent(Ctx const& c) {");
            out.line(format!("auto tag = R::Tag({});", self.expr(accent)));
            if let Type::Enum { name } = &accent.ty
                && let Some(def) = self.types().enum_def(name)
            {
                for (index, variant) in def.variants.iter().enumerate() {
                    if let Some(position) = self
                        .app
                        .style
                        .accents
                        .iter()
                        .position(|accent| Some(accent) == self.app.style.accent(&variant.name))
                    {
                        out.line(format!("if (tag == {index}u) return {position};"));
                    }
                }
            }
            out.line("return -1;");
            out.close("}");
        }
        if let Some(appearance) = &windows.appearance {
            out.line(
                "/// The appearance of `WindowSet(appearance:)`: light, dark or the system's.",
            );
            out.open("static char const* ThemeAppearance(Ctx const& c) {");
            out.line(format!("auto tag = R::Tag({});", self.expr(appearance)));
            if let Type::Enum { name } = &appearance.ty
                && let Some(def) = self.types().enum_def(name)
            {
                for (index, variant) in def.variants.iter().enumerate() {
                    let mode = variant.name.to_lowercase();
                    if mode == "light" || mode == "dark" {
                        out.line(format!("if (tag == {index}u) return {};", string(&mode)));
                    }
                }
            }
            out.line("return \"\";");
            out.close("}");
        }
        out.line("");
    }
}
