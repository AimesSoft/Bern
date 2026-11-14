//! Pure-Rust vector animation player and `tiny-skia` SIMD rasterizer.

use crate::codec;
use crate::model::*;
use crate::path::{build_shape, path_bbox};
use std::collections::HashMap;
use std::sync::Arc;
use tiny_skia::{
    Color, FillRule as SkFillRule, GradientStop as SkStop, LineCap as SkCap, LineJoin as SkJoin,
    LinearGradient, Mask, Paint, Path, Pixmap, PixmapPaint, Point, RadialGradient, SpreadMode,
    Stroke, StrokeDash, Transform,
};

/// Aspect ratio fitting mode, matching SVG `preserveAspectRatio`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Fit {
    #[default]
    Contain,
    Cover,
    Fill,
}

/// Output pixel buffer layout requested by a frontend presenter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// Premultiplied `[R, G, B, A]` (`CGImage`, GTK `MemoryTexture`, Android `Bitmap`).
    #[default]
    RgbaPremul,
    /// Premultiplied `[B, G, R, A]` (WinUI 3 `WriteableBitmap`).
    BgraPremul,
    /// Straight-alpha `[R, G, B, A]` (Web `<canvas>` `ImageData`).
    RgbaStraight,
    /// Straight-alpha `[B, G, R, A]` (GPUI `RenderImage`).
    BgraStraight,
}

/// Pointer input forwarded from a native view. Coordinates `x` and `y` are
/// normalized in `[0.0, 1.0]` across the view's logical width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent {
    Click(f32, f32),
    DoubleClick(f32, f32),
    Move(f32, f32),
    Leave,
    Down(f32, f32),
    Up(f32, f32),
}

/// Bound playback and environment parameters of an `Svg` view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub playing: bool,
    pub speed: f64,
    pub restart: f64,
    pub fit: Fit,
    pub interactive: bool,
    pub dark: bool,
    pub reduced_motion: bool,
    pub text_rgba: [u8; 4],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            playing: true,
            speed: 1.0,
            restart: 0.0,
            fit: Fit::Contain,
            interactive: true,
            dark: false,
            reduced_motion: false,
            text_rgba: [0, 0, 0, 255],
        }
    }
}

/// Outcome of configuring, interacting with, or rendering a `Player`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepOutcome {
    /// The rendered pixels (or visual state) changed since the previous frame.
    pub changed: bool,
    /// Animations, transitions, or timed interactions are still running;
    /// when `false`, the frontend should pause its frame clock for 0% idle CPU.
    pub active: bool,
}

impl StepOutcome {
    pub fn to_bits(self) -> u8 {
        u8::from(self.changed) | (u8::from(self.active) << 1)
    }
}

/// Immutable, shareable vector animation scene (`Send + Sync`).
pub struct Scene {
    pub(crate) data: SceneData,
    static_paths: Vec<Option<Path>>,
    clip_paths: Vec<Vec<(Path, [f32; 6], FillRule)>>,
    keyframe_map: HashMap<String, usize>,
}

impl Scene {
    pub fn from_data(data: SceneData) -> Self {
        let static_paths = data
            .nodes
            .iter()
            .map(|n| {
                let geom_animated = n.smil.iter().any(|s| {
                    matches!(
                        s.attr,
                        SmilAttr::R
                            | SmilAttr::Rx
                            | SmilAttr::Ry
                            | SmilAttr::Cx
                            | SmilAttr::Cy
                            | SmilAttr::X
                            | SmilAttr::Y
                            | SmilAttr::Width
                            | SmilAttr::Height
                    )
                });
                if geom_animated {
                    None
                } else {
                    build_shape(&n.shape)
                }
            })
            .collect();

        let clip_paths = data
            .clips
            .iter()
            .map(|clip| {
                clip.items
                    .iter()
                    .filter_map(|item| {
                        Some((build_shape(&item.shape)?, item.transform, item.fill_rule))
                    })
                    .collect()
            })
            .collect();

        let keyframe_map = data
            .keyframes
            .iter()
            .enumerate()
            .map(|(idx, kf)| (kf.name.clone(), idx))
            .collect();

        Self {
            data,
            static_paths,
            clip_paths,
            keyframe_map,
        }
    }

    /// Load a `Scene` from precompiled `BSVG` bytes (or `.svg` UTF-8 text when
    /// the `svg` feature is enabled).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.starts_with(MAGIC) {
            return codec::decode(bytes).map(Self::from_data);
        }
        #[cfg(feature = "svg")]
        {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| "expected BSVG binary or UTF-8 SVG text".to_string())?;
            Self::from_svg(text)
        }
        #[cfg(not(feature = "svg"))]
        {
            Err("expected precompiled BSVG asset".into())
        }
    }

    #[cfg(feature = "svg")]
    pub fn from_svg(source: &str) -> Result<Self, String> {
        crate::compile::compile_scene(source).map(Self::from_data)
    }

    pub fn view_box(&self) -> ViewBox {
        self.data.view_box
    }
}

#[derive(Clone)]
struct CascadedNode {
    fill: ColorSpec,
    fill_opacity: f32,
    fill_rule: FillRule,
    stroke: ColorSpec,
    stroke_width: f32,
    stroke_opacity: f32,
    stroke_linecap: LineCap,
    stroke_linejoin: LineJoin,
    stroke_dasharray: Vec<f32>,
    stroke_dashoffset: f32,
    opacity: f32,
    display_none: bool,
    has_css_transform: bool,
    css_transform: Vec<TransformOp>,
    transform_box: TransformBox,
    transform_origin: OriginSpec,
    anim_name: Option<String>,
    anim_duration: f32,
    anim_delay: f32,
    anim_timing: Easing,
    anim_iterations: Iterations,
    anim_direction: AnimDirection,
    anim_fill: AnimFill,
    transitions: Vec<TransitionSpec>,
}

impl Default for CascadedNode {
    fn default() -> Self {
        Self {
            fill: ColorSpec::Rgba([0, 0, 0, 255]),
            fill_opacity: 1.0,
            fill_rule: FillRule::NonZero,
            stroke: ColorSpec::None,
            stroke_width: 1.0,
            stroke_opacity: 1.0,
            stroke_linecap: LineCap::Butt,
            stroke_linejoin: LineJoin::Miter,
            stroke_dasharray: Vec::new(),
            stroke_dashoffset: 0.0,
            opacity: 1.0,
            display_none: false,
            has_css_transform: false,
            css_transform: Vec::new(),
            transform_box: TransformBox::ViewBox,
            transform_origin: OriginSpec::default(),
            anim_name: None,
            anim_duration: 0.0,
            anim_delay: 0.0,
            anim_timing: Easing::EASE,
            anim_iterations: Iterations::Count(1.0),
            anim_direction: AnimDirection::Normal,
            anim_fill: AnimFill::None,
            transitions: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct RunningAnim {
    name: String,
    start_clock: f64,
}

#[derive(Clone)]
struct RunningTransition {
    prop: TrackProp,
    from: AnimValue,
    to: AnimValue,
    start_clock: f64,
    delay: f32,
    duration: f32,
    easing: Easing,
}

#[derive(Clone)]
struct EvaluatedNode {
    visible: bool,
    opacity: f32,
    world_matrix: [f32; 6],
    fill: ColorSpec,
    fill_opacity: f32,
    fill_rule: FillRule,
    stroke: ColorSpec,
    stroke_width: f32,
    stroke_opacity: f32,
    stroke_linecap: LineCap,
    stroke_linejoin: LineJoin,
    stroke_dasharray: Vec<f32>,
    stroke_dashoffset: f32,
    shape_override: Option<ShapeKind>,
}

/// Stateful per-instance player for a [`Scene`].
pub struct Player {
    scene: Arc<Scene>,
    config: Config,
    restart_initialized: bool,
    clock: f64,
    last_wall: Option<f64>,

    toggled_classes: Vec<String>,
    timed_class: Option<(String, Option<f64>)>,
    cycle_index: Option<usize>,
    last_random: Option<usize>,
    rng_state: u64,

    hovered: Vec<u16>,
    pressed: Vec<u16>,

    cascade_dirty: bool,
    first_cascade: bool,
    cascaded: Vec<CascadedNode>,
    anims: Vec<Option<RunningAnim>>,
    transitions: Vec<Vec<RunningTransition>>,
    evaluated: Vec<EvaluatedNode>,

    pixmap: Option<Pixmap>,
    layer_pixmap: Option<Pixmap>,
    output_buf: Vec<u8>,
    last_frame_hash: Option<u64>,
    last_size: (u32, u32),
}

impl Player {
    pub fn new(scene: Arc<Scene>) -> Self {
        let count = scene.data.nodes.len();
        let seed = (&raw const *scene as usize as u64) ^ 0x9E37_79B9_7F4A_7C15;
        Self {
            scene,
            config: Config::default(),
            restart_initialized: false,
            clock: 0.0,
            last_wall: None,
            toggled_classes: Vec::new(),
            timed_class: None,
            cycle_index: None,
            last_random: None,
            rng_state: seed | 1,
            hovered: Vec::new(),
            pressed: Vec::new(),
            cascade_dirty: true,
            first_cascade: true,
            cascaded: vec![CascadedNode::default(); count],
            anims: vec![None; count],
            transitions: vec![Vec::new(); count],
            evaluated: vec![
                EvaluatedNode {
                    visible: true,
                    opacity: 1.0,
                    world_matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    fill: ColorSpec::None,
                    fill_opacity: 1.0,
                    fill_rule: FillRule::NonZero,
                    stroke: ColorSpec::None,
                    stroke_width: 1.0,
                    stroke_opacity: 1.0,
                    stroke_linecap: LineCap::Butt,
                    stroke_linejoin: LineJoin::Miter,
                    stroke_dasharray: Vec::new(),
                    stroke_dashoffset: 0.0,
                    shape_override: None,
                };
                count
            ],
            pixmap: None,
            layer_pixmap: None,
            output_buf: Vec::new(),
            last_frame_hash: None,
            last_size: (0, 0),
        }
    }

    pub fn scene(&self) -> &Arc<Scene> {
        &self.scene
    }

    pub fn clock(&self) -> f64 {
        self.clock
    }

    /// Update playback and appearance settings.
    pub fn configure(&mut self, mut next: Config) -> StepOutcome {
        if !next.speed.is_finite() || next.speed <= 0.0 {
            next.speed = if self.config.speed > 0.0 {
                self.config.speed
            } else {
                1.0
            };
        }
        if !next.restart.is_finite() {
            next.restart = self.config.restart;
        }

        let mut changed = false;
        if !self.restart_initialized {
            self.restart_initialized = true;
            self.config.restart = next.restart;
        } else if (next.restart - self.config.restart).abs() > f64::EPSILON {
            self.config.restart = next.restart;
            self.reset_timelines();
            changed = true;
        }

        if self.config.dark != next.dark || self.config.reduced_motion != next.reduced_motion {
            self.cascade_dirty = true;
            changed = true;
        }
        if self.config.fit != next.fit || self.config.text_rgba != next.text_rgba {
            changed = true;
        }
        if self.config.playing != next.playing {
            self.last_wall = None;
        }
        if !next.interactive && (!self.hovered.is_empty() || !self.pressed.is_empty()) {
            self.hovered.clear();
            self.pressed.clear();
            self.cascade_dirty = true;
            changed = true;
        }
        self.config = next;
        if self.cascade_dirty {
            self.recompute_cascade(false);
        }
        if changed {
            self.last_frame_hash = None;
        }
        StepOutcome {
            changed: changed || self.last_frame_hash.is_none(),
            active: self.is_active(),
        }
    }

    /// Reset CSS and SMIL timelines to `t = 0`, preserving toggled business classes
    /// while restarting active keyframe animations.
    pub fn reset_timelines(&mut self) {
        self.clock = 0.0;
        self.last_wall = None;
        self.timed_class = None;
        self.anims.fill(None);
        for tr in &mut self.transitions {
            tr.clear();
        }
        self.cascade_dirty = true;
        self.first_cascade = true;
        self.last_frame_hash = None;
        self.recompute_cascade(true);
    }

    /// Dispatch a pointer interaction in normalized `[0, 1]` coordinates.
    pub fn pointer(&mut self, event: PointerEvent) -> StepOutcome {
        if !self.config.interactive {
            return StepOutcome {
                changed: false,
                active: self.is_active(),
            };
        }
        let mut state_mutated = false;
        match event {
            PointerEvent::Move(x, y) => {
                let hit = self.hit_chain(x, y);
                if hit != self.hovered {
                    self.hovered = hit;
                    self.cascade_dirty = true;
                    state_mutated = true;
                }
            }
            PointerEvent::Leave => {
                if !self.hovered.is_empty() || !self.pressed.is_empty() {
                    self.hovered.clear();
                    self.pressed.clear();
                    self.cascade_dirty = true;
                    state_mutated = true;
                }
            }
            PointerEvent::Down(x, y) => {
                let hit = self.hit_chain(x, y);
                if hit != self.pressed {
                    self.pressed = hit;
                    self.cascade_dirty = true;
                    state_mutated = true;
                }
            }
            PointerEvent::Up(_, _) => {
                if !self.pressed.is_empty() {
                    self.pressed.clear();
                    self.cascade_dirty = true;
                    state_mutated = true;
                }
            }
            PointerEvent::Click(_, _) => {
                if let Some(action) = self.scene.data.interaction.on_click.clone() {
                    state_mutated |= self.run_interaction(&action);
                }
            }
            PointerEvent::DoubleClick(_, _) => {
                if let Some(action) = self.scene.data.interaction.on_dblclick.clone() {
                    state_mutated |= self.run_interaction(&action);
                }
            }
        }
        if self.cascade_dirty {
            self.recompute_cascade(false);
            self.last_frame_hash = None;
        }
        StepOutcome {
            changed: state_mutated,
            active: self.is_active(),
        }
    }

    /// Advance the timeline to `wall_secs` (monotonic seconds) and render into an
    /// internal buffer of `width_px × height_px` in `format`.
    pub fn render(
        &mut self,
        wall_secs: f64,
        width_px: u32,
        height_px: u32,
        format: PixelFormat,
    ) -> (StepOutcome, &[u8]) {
        let width_px = width_px.clamp(1, 4096);
        let height_px = height_px.clamp(1, 4096);

        if self.config.playing {
            if let Some(prev) = self.last_wall {
                let dt = (wall_secs - prev).max(0.0);
                self.clock += dt * self.config.speed;
            }
            self.last_wall = Some(wall_secs);
        } else {
            self.last_wall = None;
        }

        // Expire timed interactive class if its duration has elapsed.
        if let Some((_, Some(expire_at))) = self.timed_class
            && self.clock >= expire_at
        {
            self.timed_class = None;
            self.cascade_dirty = true;
        }

        if self.cascade_dirty {
            self.recompute_cascade(false);
        }

        self.evaluate_frame();
        let active = self.is_active();

        let frame_hash = self.compute_frame_hash(width_px, height_px, format);
        if self.last_frame_hash == Some(frame_hash) && self.last_size == (width_px, height_px) {
            return (
                StepOutcome {
                    changed: false,
                    active,
                },
                &self.output_buf,
            );
        }

        self.rasterize(width_px, height_px, format);
        self.last_frame_hash = Some(frame_hash);
        self.last_size = (width_px, height_px);

        (
            StepOutcome {
                changed: true,
                active,
            },
            &self.output_buf,
        )
    }

    /// Advance the internal animation clock directly by `dt_secs` (useful in
    /// deterministic unit tests without wall-clock timestamps).
    pub fn seek(&mut self, clock_secs: f64) {
        self.clock = clock_secs.max(0.0);
        self.last_wall = None;
        if let Some((_, Some(expire_at))) = self.timed_class
            && self.clock >= expire_at
        {
            self.timed_class = None;
            self.cascade_dirty = true;
        }
        if self.cascade_dirty {
            self.recompute_cascade(false);
        }
        self.evaluate_frame();
        self.last_frame_hash = None;
    }

    pub fn root_classes(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .scene
            .data
            .nodes
            .first()
            .map(|n| n.classes.iter().map(String::as_str).collect())
            .unwrap_or_default();
        for c in &self.toggled_classes {
            if !out.contains(&c.as_str()) {
                out.push(c.as_str());
            }
        }
        if let Some((ref tc, _)) = self.timed_class
            && !out.contains(&tc.as_str())
        {
            out.push(tc.as_str());
        }
        out
    }

    fn run_interaction(&mut self, action: &InteractionAction) -> bool {
        let ready = f64::from(self.scene.data.interaction.ready_after);
        if !self.config.reduced_motion && self.clock + 1e-6 < ready {
            return false;
        }
        match action {
            InteractionAction::Toggle(cls) => {
                if let Some(pos) = self.toggled_classes.iter().position(|c| c == cls) {
                    self.toggled_classes.remove(pos);
                } else {
                    self.toggled_classes.push(cls.clone());
                }
                self.cascade_dirty = true;
                true
            }
            InteractionAction::Trigger(tc) => {
                self.apply_timed_class(tc);
                true
            }
            InteractionAction::Cycle(list) => {
                if list.is_empty() {
                    return false;
                }
                let next = match self.cycle_index {
                    Some(i) => (i + 1) % list.len(),
                    None => 0,
                };
                self.cycle_index = Some(next);
                self.apply_timed_class(&list[next]);
                true
            }
            InteractionAction::Random(list) => {
                if list.is_empty() {
                    return false;
                }
                let pick = if list.len() == 1 {
                    0
                } else {
                    let r = self.next_rand() as usize;
                    match self.last_random {
                        Some(last) if last < list.len() => {
                            let idx = r % (list.len() - 1);
                            if idx >= last { idx + 1 } else { idx }
                        }
                        _ => r % list.len(),
                    }
                };
                self.last_random = Some(pick);
                self.apply_timed_class(&list[pick]);
                true
            }
        }
    }

    fn apply_timed_class(&mut self, tc: &TimedClass) {
        // Clear any currently running animations from a previous timed class so
        // re-triggering restarts cleanly from 0%.
        if self.timed_class.is_some() {
            self.timed_class = None;
            self.recompute_cascade(false);
        }
        if let Some(cls) = &tc.class {
            let expire = tc.duration.map(|d| self.clock + f64::from(d));
            self.timed_class = Some((cls.clone(), expire));
        } else {
            self.timed_class = None;
        }
        self.cascade_dirty = true;
    }

    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng_state ^ self.clock.to_bits().wrapping_mul(0x517C_C1B7_2722_0A95);
        if x == 0 {
            x = 0xCAFE_BABE_DEAD_BEEF;
        }
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn hit_chain(&self, norm_x: f32, norm_y: f32) -> Vec<u16> {
        if !(0.0..=1.0).contains(&norm_x) || !(0.0..=1.0).contains(&norm_y) {
            return Vec::new();
        }
        let vb = self.scene.data.view_box;
        let sx = vb.x + norm_x * vb.width;
        let sy = vb.y + norm_y * vb.height;
        let mut best: Option<u16> = Some(0);
        for (idx, node) in self.scene.data.nodes.iter().enumerate().skip(1).rev() {
            if !self
                .evaluated
                .get(idx)
                .is_some_and(|e| e.visible && e.opacity > 0.0)
            {
                continue;
            }
            if matches!(node.shape, ShapeKind::Group) {
                continue;
            }
            if let Some(inv) = invert_affine(self.evaluated[idx].world_matrix) {
                let lx = inv[0] * sx + inv[2] * sy + inv[4];
                let ly = inv[1] * sx + inv[3] * sy + inv[5];
                let b = node.fill_box;
                if b.valid && lx >= b.x && lx <= b.x + b.width && ly >= b.y && ly <= b.y + b.height
                {
                    best = Some(idx as u16);
                    break;
                }
            }
        }
        let mut chain = Vec::new();
        let mut cur = best;
        while let Some(id) = cur {
            chain.push(id);
            cur = self
                .scene
                .data
                .nodes
                .get(id as usize)
                .and_then(|n| n.parent);
        }
        chain
    }

    fn recompute_cascade(&mut self, force_reset: bool) {
        self.cascade_dirty = false;
        let is_initial = self.first_cascade || force_reset;
        self.first_cascade = false;

        // Snapshot previous displayed values before cascade changes for transitions.
        let prev_eval = if !is_initial {
            self.evaluate_frame();
            Some(self.evaluated.clone())
        } else {
            None
        };

        let root_classes: Vec<String> =
            self.root_classes().into_iter().map(str::to_owned).collect();
        let n_nodes = self.scene.data.nodes.len();

        #[derive(Clone)]
        struct RawNodeProps {
            fill: Option<ColorSpec>,
            fill_opacity: Option<f32>,
            fill_rule: Option<FillRule>,
            stroke: Option<ColorSpec>,
            stroke_width: Option<f32>,
            stroke_opacity: Option<f32>,
            stroke_linecap: Option<LineCap>,
            stroke_linejoin: Option<LineJoin>,
            stroke_dasharray: Option<Vec<f32>>,
            stroke_dashoffset: Option<f32>,
            opacity: f32,
            display_none: bool,
            css_transform: Option<Vec<TransformOp>>,
            transform_box: TransformBox,
            transform_origin: OriginSpec,
            anim_name: Option<String>,
            anim_duration: f32,
            anim_delay: f32,
            anim_timing: Easing,
            anim_iterations: Iterations,
            anim_direction: AnimDirection,
            anim_fill: AnimFill,
            transitions: Vec<TransitionSpec>,
        }

        let mut raw = vec![
            RawNodeProps {
                fill: None,
                fill_opacity: None,
                fill_rule: None,
                stroke: None,
                stroke_width: None,
                stroke_opacity: None,
                stroke_linecap: None,
                stroke_linejoin: None,
                stroke_dasharray: None,
                stroke_dashoffset: None,
                opacity: 1.0,
                display_none: false,
                css_transform: None,
                transform_box: TransformBox::ViewBox,
                transform_origin: OriginSpec::default(),
                anim_name: None,
                anim_duration: 0.0,
                anim_delay: 0.0,
                anim_timing: Easing::EASE,
                anim_iterations: Iterations::Count(1.0),
                anim_direction: AnimDirection::Normal,
                anim_fill: AnimFill::None,
                transitions: Vec::new(),
            };
            n_nodes
        ];

        type CascadeKey = (u8, (u16, u16, u16), u32);
        for (idx, dst) in raw.iter_mut().enumerate().take(n_nodes) {
            let node = &self.scene.data.nodes[idx];
            let mut matched: Vec<(CascadeKey, &StyleProp)> = Vec::new();

            for (order, (prop, is_inline, imp)) in node.base_props.iter().enumerate() {
                let tier = match (*is_inline, *imp) {
                    (false, false) => 0,
                    (true, false) => 2,
                    (_, true) => 4,
                };
                matched.push(((tier, (0, 0, 0), order as u32), prop));
            }

            for rule in &self.scene.data.rules {
                let media_ok = match rule.media {
                    MediaCond::Always => true,
                    MediaCond::SchemeDark => self.config.dark,
                    MediaCond::SchemeLight => !self.config.dark,
                    MediaCond::ReducedMotion => self.config.reduced_motion,
                };
                if !media_ok {
                    continue;
                }
                if self.matches_selector(idx, &rule.steps, &root_classes) {
                    for (sub, (prop, imp)) in rule.declarations.iter().enumerate() {
                        let tier = if *imp { 3 } else { 1 };
                        matched
                            .push(((tier, rule.specificity, rule.order * 64 + sub as u32), prop));
                    }
                }
            }

            matched.sort_by_key(|(k, _)| *k);

            for (_, prop) in matched {
                match prop {
                    StyleProp::Fill(c) => dst.fill = Some(c.clone()),
                    StyleProp::FillOpacity(v) => dst.fill_opacity = Some(*v),
                    StyleProp::FillRule(r) => dst.fill_rule = Some(*r),
                    StyleProp::Stroke(c) => dst.stroke = Some(c.clone()),
                    StyleProp::StrokeWidth(v) => dst.stroke_width = Some(*v),
                    StyleProp::StrokeOpacity(v) => dst.stroke_opacity = Some(*v),
                    StyleProp::StrokeLinecap(c) => dst.stroke_linecap = Some(*c),
                    StyleProp::StrokeLinejoin(j) => dst.stroke_linejoin = Some(*j),
                    StyleProp::StrokeDasharray(a) => dst.stroke_dasharray = Some(a.clone()),
                    StyleProp::StrokeDashoffset(v) => dst.stroke_dashoffset = Some(*v),
                    StyleProp::Opacity(v) => dst.opacity = *v,
                    StyleProp::DisplayNone(v) => dst.display_none = *v,
                    StyleProp::Transform(ops) => dst.css_transform = Some(ops.clone()),
                    StyleProp::TransformBox(b) => dst.transform_box = *b,
                    StyleProp::TransformOrigin(o) => dst.transform_origin = *o,
                    StyleProp::AnimName(n) => dst.anim_name = n.clone(),
                    StyleProp::AnimDuration(v) => dst.anim_duration = *v,
                    StyleProp::AnimDelay(v) => dst.anim_delay = *v,
                    StyleProp::AnimTiming(e) => dst.anim_timing = *e,
                    StyleProp::AnimIterations(it) => dst.anim_iterations = *it,
                    StyleProp::AnimDirection(d) => dst.anim_direction = *d,
                    StyleProp::AnimFill(f) => dst.anim_fill = *f,
                    StyleProp::Transitions(ts) => dst.transitions = ts.clone(),
                }
            }
        }

        // Top-down inheritance of SVG paint & stroke properties, plus animation
        // and transition activation.
        for idx in 0..n_nodes {
            let parent = self.scene.data.nodes[idx].parent.map(|p| p as usize);
            let p_cas = parent.map(|p| self.cascaded[p].clone());
            let r = &raw[idx];
            let next = CascadedNode {
                fill: r
                    .fill
                    .clone()
                    .or_else(|| p_cas.as_ref().map(|p| p.fill.clone()))
                    .unwrap_or(ColorSpec::Rgba([0, 0, 0, 255])),
                fill_opacity: r
                    .fill_opacity
                    .or_else(|| p_cas.as_ref().map(|p| p.fill_opacity))
                    .unwrap_or(1.0),
                fill_rule: r
                    .fill_rule
                    .or_else(|| p_cas.as_ref().map(|p| p.fill_rule))
                    .unwrap_or(FillRule::NonZero),
                stroke: r
                    .stroke
                    .clone()
                    .or_else(|| p_cas.as_ref().map(|p| p.stroke.clone()))
                    .unwrap_or(ColorSpec::None),
                stroke_width: r
                    .stroke_width
                    .or_else(|| p_cas.as_ref().map(|p| p.stroke_width))
                    .unwrap_or(1.0),
                stroke_opacity: r
                    .stroke_opacity
                    .or_else(|| p_cas.as_ref().map(|p| p.stroke_opacity))
                    .unwrap_or(1.0),
                stroke_linecap: r
                    .stroke_linecap
                    .or_else(|| p_cas.as_ref().map(|p| p.stroke_linecap))
                    .unwrap_or(LineCap::Butt),
                stroke_linejoin: r
                    .stroke_linejoin
                    .or_else(|| p_cas.as_ref().map(|p| p.stroke_linejoin))
                    .unwrap_or(LineJoin::Miter),
                stroke_dasharray: r
                    .stroke_dasharray
                    .clone()
                    .or_else(|| p_cas.as_ref().map(|p| p.stroke_dasharray.clone()))
                    .unwrap_or_default(),
                stroke_dashoffset: r
                    .stroke_dashoffset
                    .or_else(|| p_cas.as_ref().map(|p| p.stroke_dashoffset))
                    .unwrap_or(0.0),
                opacity: r.opacity,
                display_none: r.display_none || p_cas.as_ref().is_some_and(|p| p.display_none),
                has_css_transform: r.css_transform.is_some(),
                css_transform: r.css_transform.clone().unwrap_or_default(),
                transform_box: r.transform_box,
                transform_origin: r.transform_origin,
                anim_name: r.anim_name.clone(),
                anim_duration: r.anim_duration,
                anim_delay: r.anim_delay,
                anim_timing: r.anim_timing,
                anim_iterations: r.anim_iterations,
                anim_direction: r.anim_direction,
                anim_fill: r.anim_fill,
                transitions: r.transitions.clone(),
            };

            // Update active CSS keyframe animation state.
            match (&self.anims[idx], &next.anim_name) {
                (_, None) => self.anims[idx] = None,
                (Some(cur), Some(name)) if cur.name == *name && !is_initial => {}
                (_, Some(name)) => {
                    self.anims[idx] = Some(RunningAnim {
                        name: name.clone(),
                        start_clock: if is_initial { 0.0 } else { self.clock },
                    });
                }
            }

            // Spawn CSS property transitions when non-initial cascade changes base values.
            if let Some(ref prev_list) = prev_eval
                && !self.config.reduced_motion
                && !next.transitions.is_empty()
            {
                let prev = &prev_list[idx];
                self.maybe_start_transitions(idx, prev, &next);
            }

            self.cascaded[idx] = next;
        }
    }

    fn maybe_start_transitions(&mut self, idx: usize, prev: &EvaluatedNode, next: &CascadedNode) {
        let find_spec = |tp: TrackProp| -> Option<TransitionSpec> {
            next.transitions.iter().copied().rfind(|s| {
                s.prop == TransitionProp::All
                    || matches!(
                        (s.prop, tp),
                        (TransitionProp::Fill, TrackProp::Fill)
                            | (TransitionProp::Stroke, TrackProp::Stroke)
                            | (TransitionProp::Opacity, TrackProp::Opacity)
                            | (TransitionProp::Transform, TrackProp::Transform)
                            | (TransitionProp::StrokeWidth, TrackProp::StrokeWidth)
                            | (
                                TransitionProp::StrokeDashoffset,
                                TrackProp::StrokeDashoffset
                            )
                    )
            })
        };

        let mut push_tr = |tp: TrackProp, from: AnimValue, to: AnimValue| {
            if from == to {
                return;
            }
            if let Some(spec) = find_spec(tp) {
                let list = &mut self.transitions[idx];
                list.retain(|t| t.prop != tp);
                list.push(RunningTransition {
                    prop: tp,
                    from,
                    to,
                    start_clock: self.clock,
                    delay: spec.delay,
                    duration: spec.duration,
                    easing: spec.easing,
                });
            }
        };

        if prev.fill != next.fill {
            push_tr(
                TrackProp::Fill,
                AnimValue::Color(prev.fill.clone()),
                AnimValue::Color(next.fill.clone()),
            );
        }
        if prev.stroke != next.stroke {
            push_tr(
                TrackProp::Stroke,
                AnimValue::Color(prev.stroke.clone()),
                AnimValue::Color(next.stroke.clone()),
            );
        }
        if (prev.opacity - next.opacity).abs() > 1e-4 {
            push_tr(
                TrackProp::Opacity,
                AnimValue::Scalar(prev.opacity),
                AnimValue::Scalar(next.opacity),
            );
        }
        if (prev.stroke_width - next.stroke_width).abs() > 1e-4 {
            push_tr(
                TrackProp::StrokeWidth,
                AnimValue::Scalar(prev.stroke_width),
                AnimValue::Scalar(next.stroke_width),
            );
        }
        if (prev.stroke_dashoffset - next.stroke_dashoffset).abs() > 1e-4 {
            push_tr(
                TrackProp::StrokeDashoffset,
                AnimValue::Scalar(prev.stroke_dashoffset),
                AnimValue::Scalar(next.stroke_dashoffset),
            );
        }
    }

    fn matches_selector(
        &self,
        node_idx: usize,
        steps: &[SelectorStep],
        root_classes: &[String],
    ) -> bool {
        let Some(last) = steps.last() else {
            return false;
        };
        if !self.matches_compound(node_idx, &last.compound, root_classes) {
            return false;
        }
        let mut cur_idx = node_idx;
        for step_i in (0..steps.len() - 1).rev() {
            let need_direct = steps[step_i + 1].direct_parent;
            let target_comp = &steps[step_i].compound;
            let Some(mut parent) = self.scene.data.nodes[cur_idx].parent.map(|p| p as usize) else {
                return false;
            };
            if need_direct {
                if !self.matches_compound(parent, target_comp, root_classes) {
                    return false;
                }
                cur_idx = parent;
            } else {
                loop {
                    if self.matches_compound(parent, target_comp, root_classes) {
                        cur_idx = parent;
                        break;
                    }
                    match self.scene.data.nodes[parent].parent {
                        Some(gp) => parent = gp as usize,
                        None => return false,
                    }
                }
            }
        }
        true
    }

    fn matches_compound(
        &self,
        node_idx: usize,
        comp: &CompoundSel,
        root_classes: &[String],
    ) -> bool {
        let node = &self.scene.data.nodes[node_idx];
        if let Some(ref tag) = comp.tag
            && node.tag != *tag
        {
            return false;
        }
        if let Some(ref id) = comp.id
            && node.id.as_ref() != Some(id)
        {
            return false;
        }
        if comp.hover && !self.hovered.contains(&(node_idx as u16)) {
            return false;
        }
        if comp.active && !self.pressed.contains(&(node_idx as u16)) {
            return false;
        }
        let classes = if node_idx == 0 {
            root_classes
        } else {
            node.classes.as_slice()
        };
        comp.classes
            .iter()
            .all(|req| classes.iter().any(|c| c == req))
    }

    fn evaluate_frame(&mut self) {
        let vb = self.scene.data.view_box;
        let dark = self.config.dark;
        let current_rgba = self.config.text_rgba;

        for idx in 0..self.scene.data.nodes.len() {
            let node = &self.scene.data.nodes[idx];
            let cas = &self.cascaded[idx];

            let mut fill = cas.fill.clone();
            let mut stroke = cas.stroke.clone();
            let mut opacity = cas.opacity;
            let mut stroke_width = cas.stroke_width;
            let mut stroke_dashoffset = cas.stroke_dashoffset;

            let base_tf: &[TransformOp] = if cas.has_css_transform {
                &cas.css_transform
            } else {
                &node.svg_transform
            };
            let mut tf_matrix: Option<[f32; 6]> = None;

            // 1. Active CSS transitions.
            self.transitions[idx].retain(|tr| {
                let elapsed = (self.clock - tr.start_clock) as f32 - tr.delay;
                if elapsed < 0.0 {
                    apply_anim_val(
                        tr.prop,
                        &tr.from,
                        &mut fill,
                        &mut stroke,
                        &mut opacity,
                        &mut stroke_width,
                        &mut stroke_dashoffset,
                        &mut tf_matrix,
                    );
                    true
                } else if elapsed < tr.duration && tr.duration > 0.0 {
                    let t = tr.easing.sample((elapsed / tr.duration).clamp(0.0, 1.0));
                    let val = interpolate_anim_value(&tr.from, &tr.to, t, dark, current_rgba);
                    apply_anim_val(
                        tr.prop,
                        &val,
                        &mut fill,
                        &mut stroke,
                        &mut opacity,
                        &mut stroke_width,
                        &mut stroke_dashoffset,
                        &mut tf_matrix,
                    );
                    true
                } else {
                    false
                }
            });

            // 2. Active CSS `@keyframes` animations.
            if let Some(ref run) = self.anims[idx]
                && let Some(&kf_idx) = self.scene.keyframe_map.get(&run.name)
                && let Some((progress, _in_play)) = sample_css_progress(
                    (self.clock - run.start_clock) as f32,
                    cas.anim_delay,
                    cas.anim_duration,
                    cas.anim_iterations,
                    cas.anim_direction,
                    cas.anim_fill,
                )
            {
                let kf = &self.scene.data.keyframes[kf_idx];
                for track in &kf.tracks {
                    let default_val = match track.prop {
                        TrackProp::Transform => AnimValue::Transform(base_tf.to_vec()),
                        TrackProp::Opacity => AnimValue::Scalar(opacity),
                        TrackProp::Fill => AnimValue::Color(fill.clone()),
                        TrackProp::Stroke => AnimValue::Color(stroke.clone()),
                        TrackProp::StrokeWidth => AnimValue::Scalar(stroke_width),
                        TrackProp::StrokeDashoffset => AnimValue::Scalar(stroke_dashoffset),
                    };
                    let val = sample_track(
                        track,
                        progress,
                        cas.anim_timing,
                        &default_val,
                        dark,
                        current_rgba,
                    );
                    apply_anim_val(
                        track.prop,
                        &val,
                        &mut fill,
                        &mut stroke,
                        &mut opacity,
                        &mut stroke_width,
                        &mut stroke_dashoffset,
                        &mut tf_matrix,
                    );
                }
            }

            // 3. SMIL animations (`<animate>`, `<set>`, `<animateTransform>`).
            let mut shape_override: Option<ShapeKind> = None;
            let mut smil_tf_ops: Vec<TransformOp> = Vec::new();
            if !self.config.reduced_motion {
                for smil in &node.smil {
                    if let Some(val) = sample_smil(smil, self.clock as f32, dark, current_rgba) {
                        match (smil.attr, val) {
                            (SmilAttr::Opacity, SmilValue::Scalar(v)) => opacity = v,
                            (SmilAttr::StrokeWidth, SmilValue::Scalar(v)) => stroke_width = v,
                            (SmilAttr::StrokeDashoffset, SmilValue::Scalar(v)) => {
                                stroke_dashoffset = v
                            }
                            (SmilAttr::Fill, SmilValue::Color(c)) => fill = c,
                            (SmilAttr::Stroke, SmilValue::Color(c)) => stroke = c,
                            (SmilAttr::TransformTranslate, SmilValue::Pair(x, y)) => {
                                smil_tf_ops.push(TransformOp::Translate(x, y));
                            }
                            (SmilAttr::TransformScale, SmilValue::Pair(x, y)) => {
                                smil_tf_ops.push(TransformOp::Scale(x, y));
                            }
                            (SmilAttr::TransformRotate, SmilValue::Rotate(a, pivot)) => {
                                if let Some((cx, cy)) = pivot {
                                    smil_tf_ops.push(TransformOp::Translate(cx, cy));
                                    smil_tf_ops.push(TransformOp::Rotate(a));
                                    smil_tf_ops.push(TransformOp::Translate(-cx, -cy));
                                } else {
                                    smil_tf_ops.push(TransformOp::Rotate(a));
                                }
                            }
                            (attr, SmilValue::Scalar(v)) => {
                                let s = shape_override.get_or_insert_with(|| node.shape.clone());
                                apply_shape_scalar(s, attr, v);
                            }
                            _ => {}
                        }
                    }
                }
            }

            // 4. Local & world affine matrices around `transform-origin`.
            let use_origin = cas.has_css_transform || tf_matrix.is_some();
            let raw_mat = tf_matrix.unwrap_or_else(|| ops_to_matrix(base_tf));
            let mut local_matrix = if use_origin {
                let (ox, oy) = cas
                    .transform_origin
                    .resolve(cas.transform_box, node.fill_box, vb);
                if ox.abs() > 1e-6 || oy.abs() > 1e-6 {
                    mul_affine(
                        [1.0, 0.0, 0.0, 1.0, ox, oy],
                        mul_affine(raw_mat, [1.0, 0.0, 0.0, 1.0, -ox, -oy]),
                    )
                } else {
                    raw_mat
                }
            } else {
                raw_mat
            };
            if !smil_tf_ops.is_empty() {
                local_matrix = mul_affine(local_matrix, ops_to_matrix(&smil_tf_ops));
            }

            let world_matrix = match node.parent {
                Some(p) => mul_affine(self.evaluated[p as usize].world_matrix, local_matrix),
                None => local_matrix,
            };

            let parent_vis = node
                .parent
                .is_none_or(|p| self.evaluated[p as usize].visible);

            self.evaluated[idx] = EvaluatedNode {
                visible: parent_vis && !cas.display_none,
                opacity: opacity.clamp(0.0, 1.0),
                world_matrix,
                fill,
                fill_opacity: cas.fill_opacity,
                fill_rule: cas.fill_rule,
                stroke,
                stroke_width,
                stroke_opacity: cas.stroke_opacity,
                stroke_linecap: cas.stroke_linecap,
                stroke_linejoin: cas.stroke_linejoin,
                stroke_dasharray: cas.stroke_dasharray.clone(),
                stroke_dashoffset,
                shape_override,
            };
        }
    }

    /// Whether any CSS animation, CSS transition, SMIL timeline, or timed
    /// interaction is still active or scheduled in the future.
    pub fn is_active(&self) -> bool {
        if !self.config.playing {
            return false;
        }
        if let Some((_, Some(expire_at))) = self.timed_class
            && self.clock < expire_at
        {
            return true;
        }
        for (idx, cas) in self.cascaded.iter().enumerate() {
            if cas.display_none {
                continue;
            }
            if let Some(ref run) = self.anims[idx]
                && self.scene.keyframe_map.contains_key(&run.name)
                && cas.anim_duration > 0.0
            {
                match cas.anim_iterations {
                    Iterations::Infinite => return true,
                    Iterations::Count(n) if n > 0.0 => {
                        let end = run.start_clock
                            + f64::from(cas.anim_delay)
                            + f64::from(cas.anim_duration * n);
                        if self.clock < end {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            if self.transitions[idx]
                .iter()
                .any(|tr| self.clock < tr.start_clock + f64::from(tr.delay + tr.duration))
            {
                return true;
            }
            if !self.config.reduced_motion {
                for smil in &self.scene.data.nodes[idx].smil {
                    match smil.iterations {
                        Iterations::Infinite => return true,
                        Iterations::Count(n) if n > 0.0 => {
                            let end = f64::from(smil.begin + smil.dur * n);
                            if self.clock < end {
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        false
    }

    fn compute_frame_hash(&self, w: u32, h: u32, format: PixelFormat) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut mix = |bytes: &[u8]| {
            for &b in bytes {
                hash ^= u64::from(b);
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        };
        mix(&w.to_le_bytes());
        mix(&h.to_le_bytes());
        mix(&[
            format as u8,
            self.config.fit as u8,
            u8::from(self.config.dark),
        ]);
        mix(&self.config.text_rgba);
        for ev in &self.evaluated {
            mix(&[u8::from(ev.visible)]);
            if !ev.visible || ev.opacity <= 0.0 {
                continue;
            }
            mix(&ev.opacity.to_bits().to_le_bytes());
            for m in ev.world_matrix {
                mix(&m.to_bits().to_le_bytes());
            }
            hash_color(&ev.fill, &mut mix);
            hash_color(&ev.stroke, &mut mix);
            mix(&ev.stroke_width.to_bits().to_le_bytes());
            mix(&ev.stroke_dashoffset.to_bits().to_le_bytes());
            if let Some(ref s) = ev.shape_override {
                match *s {
                    ShapeKind::Circle { cx, cy, r } => {
                        mix(&cx.to_bits().to_le_bytes());
                        mix(&cy.to_bits().to_le_bytes());
                        mix(&r.to_bits().to_le_bytes());
                    }
                    ShapeKind::Rect {
                        x,
                        y,
                        width,
                        height,
                        rx,
                        ry,
                    } => {
                        for f in [x, y, width, height, rx, ry] {
                            mix(&f.to_bits().to_le_bytes());
                        }
                    }
                    ShapeKind::Ellipse { cx, cy, rx, ry } => {
                        for f in [cx, cy, rx, ry] {
                            mix(&f.to_bits().to_le_bytes());
                        }
                    }
                    _ => {}
                }
            }
        }
        hash
    }

    fn viewport_matrix(&self, w: u32, h: u32) -> [f32; 6] {
        let vb = self.scene.data.view_box;
        let (dw, dh) = (w as f32, h as f32);
        let (sx, sy, ox, oy) = match self.config.fit {
            Fit::Fill => (dw / vb.width, dh / vb.height, 0.0, 0.0),
            Fit::Contain => {
                let s = (dw / vb.width).min(dh / vb.height);
                (s, s, (dw - vb.width * s) * 0.5, (dh - vb.height * s) * 0.5)
            }
            Fit::Cover => {
                let s = (dw / vb.width).max(dh / vb.height);
                (s, s, (dw - vb.width * s) * 0.5, (dh - vb.height * s) * 0.5)
            }
        };
        [sx, 0.0, 0.0, sy, ox - vb.x * sx, oy - vb.y * sy]
    }

    fn rasterize(&mut self, w: u32, h: u32, format: PixelFormat) {
        if self
            .pixmap
            .as_ref()
            .is_none_or(|p| p.width() != w || p.height() != h)
        {
            self.pixmap = Pixmap::new(w, h);
        }
        let Some(mut main_pixmap) = self.pixmap.take() else {
            return;
        };
        main_pixmap.fill(Color::TRANSPARENT);

        let vp = self.viewport_matrix(w, h);
        for &root_idx in &self.scene.data.root_children.clone() {
            self.draw_subtree(root_idx as usize, vp, None, 1.0, &mut main_pixmap);
        }

        let src = main_pixmap.data();
        self.output_buf.resize(src.len(), 0);
        convert_pixels(src, &mut self.output_buf, format);
        self.pixmap = Some(main_pixmap);
    }

    fn draw_subtree(
        &mut self,
        idx: usize,
        vp: [f32; 6],
        inherited_mask: Option<&Mask>,
        inherited_alpha: f32,
        target: &mut Pixmap,
    ) {
        let ev = self.evaluated[idx].clone();
        if !ev.visible || ev.opacity <= 0.0 {
            return;
        }
        let node_clip = self.scene.data.nodes[idx].clip;
        let has_children = !self.scene.data.nodes[idx].children.is_empty();
        let combined_tx = mul_affine(vp, ev.world_matrix);

        // Build local clip mask if this node carries `clip-path="url(#...)"`.
        let owned_mask = node_clip.and_then(|clip_id| {
            self.build_clip_mask(
                clip_id as usize,
                combined_tx,
                target.width(),
                target.height(),
                inherited_mask,
            )
        });
        let active_mask = owned_mask.as_ref().or(inherited_mask);

        // Group opacity isolation when `0 < opacity < 1` on a multi-item group or
        // a shape with both fill and stroke.
        let both_fill_and_stroke = !matches!(ev.fill, ColorSpec::None)
            && !matches!(ev.stroke, ColorSpec::None)
            && ev.stroke_width > 0.0;
        if ev.opacity < 0.999 && (has_children || both_fill_and_stroke) {
            let (w, h) = (target.width(), target.height());
            let mut layer = self
                .layer_pixmap
                .take()
                .filter(|p| p.width() == w && p.height() == h)
                .or_else(|| Pixmap::new(w, h));
            if let Some(ref mut lp) = layer {
                lp.fill(Color::TRANSPARENT);
                self.draw_node_geometry(idx, &ev, combined_tx, None, 1.0, lp);
                let children = self.scene.data.nodes[idx].children.clone();
                for child_idx in children {
                    self.draw_subtree(child_idx as usize, vp, None, 1.0, lp);
                }
                let paint = PixmapPaint {
                    opacity: (ev.opacity * inherited_alpha).clamp(0.0, 1.0),
                    ..PixmapPaint::default()
                };
                target.draw_pixmap(
                    0,
                    0,
                    lp.as_ref(),
                    &paint,
                    Transform::identity(),
                    active_mask,
                );
            }
            self.layer_pixmap = layer;
            return;
        }

        let alpha = (ev.opacity * inherited_alpha).clamp(0.0, 1.0);
        self.draw_node_geometry(idx, &ev, combined_tx, active_mask, alpha, target);

        let children = self.scene.data.nodes[idx].children.clone();
        for child_idx in children {
            self.draw_subtree(child_idx as usize, vp, active_mask, alpha, target);
        }
    }

    fn build_clip_mask(
        &self,
        clip_idx: usize,
        node_tx: [f32; 6],
        w: u32,
        h: u32,
        parent_mask: Option<&Mask>,
    ) -> Option<Mask> {
        let items = self.scene.clip_paths.get(clip_idx)?;
        let mut mask = Mask::new(w, h)?;
        for (path, item_tx, rule) in items {
            let m = mul_affine(node_tx, *item_tx);
            let ts = Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5]);
            let sk_rule = match rule {
                FillRule::NonZero => SkFillRule::Winding,
                FillRule::EvenOdd => SkFillRule::EvenOdd,
            };
            mask.fill_path(path, sk_rule, true, ts);
        }
        if let Some(pm) = parent_mask {
            for (dst, &src) in mask.data_mut().iter_mut().zip(pm.data()) {
                *dst = ((u16::from(*dst) * u16::from(src) + 127) / 255) as u8;
            }
        }
        Some(mask)
    }

    fn draw_node_geometry(
        &self,
        idx: usize,
        ev: &EvaluatedNode,
        tx: [f32; 6],
        mask: Option<&Mask>,
        alpha: f32,
        target: &mut Pixmap,
    ) {
        let dynamic_path;
        let path: &Path = if let Some(ref shape) = ev.shape_override {
            let Some(p) = build_shape(shape) else {
                return;
            };
            dynamic_path = p;
            &dynamic_path
        } else if let Some(ref p) = self.scene.static_paths[idx] {
            p
        } else {
            let Some(p) = build_shape(&self.scene.data.nodes[idx].shape) else {
                return;
            };
            dynamic_path = p;
            &dynamic_path
        };

        let ts = Transform::from_row(tx[0], tx[1], tx[2], tx[3], tx[4], tx[5]);
        let bbox = if ev.shape_override.is_some() {
            path_bbox(path, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
        } else {
            self.scene.data.nodes[idx].fill_box
        };

        // Fill
        let fill_alpha = (alpha * ev.fill_opacity).clamp(0.0, 1.0);
        if fill_alpha > 0.0
            && let Some(paint) = self.make_paint(&ev.fill, fill_alpha, bbox, tx)
        {
            let rule = match ev.fill_rule {
                FillRule::NonZero => SkFillRule::Winding,
                FillRule::EvenOdd => SkFillRule::EvenOdd,
            };
            target.fill_path(path, &paint, rule, ts, mask);
        }

        // Stroke
        let stroke_alpha = (alpha * ev.stroke_opacity).clamp(0.0, 1.0);
        if stroke_alpha > 0.0
            && ev.stroke_width > 0.0
            && let Some(paint) = self.make_paint(&ev.stroke, stroke_alpha, bbox, tx)
        {
            let mut stroke = Stroke {
                width: ev.stroke_width,
                line_cap: match ev.stroke_linecap {
                    LineCap::Butt => SkCap::Butt,
                    LineCap::Round => SkCap::Round,
                    LineCap::Square => SkCap::Square,
                },
                line_join: match ev.stroke_linejoin {
                    LineJoin::Miter => SkJoin::Miter,
                    LineJoin::Round => SkJoin::Round,
                    LineJoin::Bevel => SkJoin::Bevel,
                },
                dash: None,
                ..Stroke::default()
            };
            if !ev.stroke_dasharray.is_empty() {
                let mut intervals = ev.stroke_dasharray.clone();
                if !intervals.len().is_multiple_of(2) {
                    intervals.extend_from_within(..);
                }
                stroke.dash = StrokeDash::new(intervals, ev.stroke_dashoffset);
            }
            target.stroke_path(path, &paint, &stroke, ts, mask);
        }
    }

    fn make_paint(
        &self,
        spec: &ColorSpec,
        alpha_mul: f32,
        bbox: BBox,
        world_tx: [f32; 6],
    ) -> Option<Paint<'static>> {
        match spec {
            ColorSpec::None => None,
            ColorSpec::Gradient(g_idx) => {
                let g = self.scene.data.gradients.get(*g_idx as usize)?;
                let mut sk_stops = Vec::with_capacity(g.stops.len());
                for st in &g.stops {
                    let [r, gg, b, a] = st
                        .color
                        .resolve_rgba(self.config.dark, self.config.text_rgba)
                        .unwrap_or([0, 0, 0, 255]);
                    let final_a = ((f32::from(a) / 255.0) * st.opacity * alpha_mul * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    sk_stops.push(SkStop::new(st.offset, Color::from_rgba8(r, gg, b, final_a)));
                }
                let unit_tx = match g.units {
                    GradientUnits::ObjectBoundingBox if bbox.valid => [
                        bbox.width.max(1e-6),
                        0.0,
                        0.0,
                        bbox.height.max(1e-6),
                        bbox.x,
                        bbox.y,
                    ],
                    _ => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                };
                // Shader coordinates in tiny-skia are evaluated in device space, so
                // compose `world_tx * unit_tx * g.transform`.
                let m = mul_affine(world_tx, mul_affine(unit_tx, g.transform));
                let shader_ts = Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5]);

                let shader = match g.kind {
                    GradientKind::Linear { x1, y1, x2, y2 } => LinearGradient::new(
                        Point::from_xy(x1, y1),
                        Point::from_xy(x2, y2),
                        sk_stops,
                        SpreadMode::Pad,
                        shader_ts,
                    )?,
                    GradientKind::Radial { cx, cy, r, fx, fy } => RadialGradient::new(
                        Point::from_xy(fx, fy),
                        Point::from_xy(cx, cy),
                        r.max(1e-5),
                        sk_stops,
                        SpreadMode::Pad,
                        shader_ts,
                    )?,
                };
                Some(Paint {
                    shader,
                    anti_alias: true,
                    ..Paint::default()
                })
            }
            other => {
                let [r, g, b, a] = other.resolve_rgba(self.config.dark, self.config.text_rgba)?;
                let final_a = ((f32::from(a) * alpha_mul).round().clamp(0.0, 255.0)) as u8;
                if final_a == 0 {
                    return None;
                }
                let mut paint = Paint::default();
                paint.set_color_rgba8(r, g, b, final_a);
                paint.anti_alias = true;
                Some(paint)
            }
        }
    }
}

fn hash_color(c: &ColorSpec, mix: &mut impl FnMut(&[u8])) {
    match c {
        ColorSpec::None => mix(&[0]),
        ColorSpec::Rgba(rgba) => {
            mix(&[1]);
            mix(rgba);
        }
        ColorSpec::CurrentColor => mix(&[2]),
        ColorSpec::LightDark(p) => {
            mix(&[3]);
            hash_color(&p.0, mix);
            hash_color(&p.1, mix);
        }
        ColorSpec::Gradient(i) => {
            mix(&[4]);
            mix(&i.to_le_bytes());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_anim_val(
    prop: TrackProp,
    val: &AnimValue,
    fill: &mut ColorSpec,
    stroke: &mut ColorSpec,
    opacity: &mut f32,
    stroke_width: &mut f32,
    stroke_dashoffset: &mut f32,
    tf_matrix: &mut Option<[f32; 6]>,
) {
    match (prop, val) {
        (TrackProp::Transform, AnimValue::Transform(ops)) => {
            *tf_matrix = Some(ops_to_matrix(ops));
        }
        (TrackProp::Opacity, AnimValue::Scalar(v)) => *opacity = *v,
        (TrackProp::Fill, AnimValue::Color(c)) => *fill = c.clone(),
        (TrackProp::Stroke, AnimValue::Color(c)) => *stroke = c.clone(),
        (TrackProp::StrokeWidth, AnimValue::Scalar(v)) => *stroke_width = *v,
        (TrackProp::StrokeDashoffset, AnimValue::Scalar(v)) => *stroke_dashoffset = *v,
        _ => {}
    }
}

fn apply_shape_scalar(shape: &mut ShapeKind, attr: SmilAttr, val: f32) {
    match (shape, attr) {
        (ShapeKind::Circle { r, .. }, SmilAttr::R) => *r = val.max(0.0),
        (ShapeKind::Circle { cx, .. }, SmilAttr::Cx) => *cx = val,
        (ShapeKind::Circle { cy, .. }, SmilAttr::Cy) => *cy = val,
        (ShapeKind::Ellipse { rx, .. }, SmilAttr::Rx) => *rx = val.max(0.0),
        (ShapeKind::Ellipse { ry, .. }, SmilAttr::Ry) => *ry = val.max(0.0),
        (ShapeKind::Ellipse { cx, .. }, SmilAttr::Cx) => *cx = val,
        (ShapeKind::Ellipse { cy, .. }, SmilAttr::Cy) => *cy = val,
        (ShapeKind::Rect { x, .. }, SmilAttr::X) => *x = val,
        (ShapeKind::Rect { y, .. }, SmilAttr::Y) => *y = val,
        (ShapeKind::Rect { width, .. }, SmilAttr::Width) => *width = val.max(0.0),
        (ShapeKind::Rect { height, .. }, SmilAttr::Height) => *height = val.max(0.0),
        (ShapeKind::Rect { rx, .. }, SmilAttr::Rx) => *rx = val.max(0.0),
        (ShapeKind::Rect { ry, .. }, SmilAttr::Ry) => *ry = val.max(0.0),
        _ => {}
    }
}

fn sample_css_progress(
    local_time: f32,
    delay: f32,
    duration: f32,
    iterations: Iterations,
    direction: AnimDirection,
    fill: AnimFill,
) -> Option<(f32, bool)> {
    let active_time = local_time - delay;
    if active_time < 0.0 {
        return match fill {
            AnimFill::Backwards | AnimFill::Both => {
                let rev = matches!(
                    direction,
                    AnimDirection::Reverse | AnimDirection::AlternateReverse
                );
                Some((if rev { 1.0 } else { 0.0 }, false))
            }
            _ => None,
        };
    }
    if duration <= 0.0 {
        return match fill {
            AnimFill::Forwards | AnimFill::Both => Some((1.0, false)),
            _ => None,
        };
    }

    let raw_iter = active_time / duration;
    let finished = match iterations {
        Iterations::Infinite => false,
        Iterations::Count(total) => raw_iter >= total,
    };

    if finished {
        let Iterations::Count(total) = iterations else {
            unreachable!()
        };
        if !matches!(fill, AnimFill::Forwards | AnimFill::Both) || total <= 0.0 {
            return None;
        }
        let last_cycle = (total - 1e-6).floor().max(0.0) as u32;
        let rem = total - total.floor();
        let end_u = if rem == 0.0 { 1.0 } else { rem };
        let rev = is_cycle_reversed(direction, last_cycle);
        return Some((if rev { 1.0 - end_u } else { end_u }, false));
    }

    let cycle = raw_iter.floor().max(0.0) as u32;
    let u = (raw_iter - raw_iter.floor()).clamp(0.0, 1.0);
    let rev = is_cycle_reversed(direction, cycle);
    Some((if rev { 1.0 - u } else { u }, true))
}

fn is_cycle_reversed(dir: AnimDirection, cycle: u32) -> bool {
    match dir {
        AnimDirection::Normal => false,
        AnimDirection::Reverse => true,
        AnimDirection::Alternate => !cycle.is_multiple_of(2),
        AnimDirection::AlternateReverse => cycle.is_multiple_of(2),
    }
}

fn sample_track(
    track: &KeyframeTrack,
    progress: f32,
    default_easing: Easing,
    base_val: &AnimValue,
    dark: bool,
    current_rgba: [u8; 4],
) -> AnimValue {
    let stops = &track.stops;
    if stops.is_empty() {
        return base_val.clone();
    }
    let first = &stops[0];
    if progress <= first.offset {
        if first.offset > 0.0 {
            let u = default_easing.sample((progress / first.offset).clamp(0.0, 1.0));
            return interpolate_anim_value(base_val, &first.value, u, dark, current_rgba);
        }
        return first.value.clone();
    }
    let last = stops.last().unwrap();
    if progress >= last.offset {
        if last.offset < 1.0 {
            let span = 1.0 - last.offset;
            let easing = last.easing.unwrap_or(default_easing);
            let u = easing.sample(((progress - last.offset) / span).clamp(0.0, 1.0));
            return interpolate_anim_value(&last.value, base_val, u, dark, current_rgba);
        }
        return last.value.clone();
    }
    for win in stops.windows(2) {
        let (a, b) = (&win[0], &win[1]);
        if progress >= a.offset && progress <= b.offset {
            let span = (b.offset - a.offset).max(1e-6);
            let raw_t = ((progress - a.offset) / span).clamp(0.0, 1.0);
            let easing = a.easing.unwrap_or(default_easing);
            let eased_t = easing.sample(raw_t);
            return interpolate_anim_value(&a.value, &b.value, eased_t, dark, current_rgba);
        }
    }
    last.value.clone()
}

fn interpolate_anim_value(
    from: &AnimValue,
    to: &AnimValue,
    t: f32,
    dark: bool,
    current_rgba: [u8; 4],
) -> AnimValue {
    match (from, to) {
        (AnimValue::Scalar(a), AnimValue::Scalar(b)) => AnimValue::Scalar(a + (b - a) * t),
        (AnimValue::Color(a), AnimValue::Color(b)) => {
            if let (Some(ca), Some(cb)) = (
                a.resolve_rgba(dark, current_rgba),
                b.resolve_rgba(dark, current_rgba),
            ) {
                let mix = |x: u8, y: u8| -> u8 {
                    (f32::from(x) + (f32::from(y) - f32::from(x)) * t)
                        .round()
                        .clamp(0.0, 255.0) as u8
                };
                AnimValue::Color(ColorSpec::Rgba([
                    mix(ca[0], cb[0]),
                    mix(ca[1], cb[1]),
                    mix(ca[2], cb[2]),
                    mix(ca[3], cb[3]),
                ]))
            } else if t < 0.5 {
                AnimValue::Color(a.clone())
            } else {
                AnimValue::Color(b.clone())
            }
        }
        (AnimValue::Transform(ops_a), AnimValue::Transform(ops_b)) => {
            AnimValue::Transform(interpolate_transforms(ops_a, ops_b, t))
        }
        _ => {
            if t < 0.5 {
                from.clone()
            } else {
                to.clone()
            }
        }
    }
}

fn interpolate_transforms(a: &[TransformOp], b: &[TransformOp], t: f32) -> Vec<TransformOp> {
    if a.is_empty() && b.is_empty() {
        return Vec::new();
    }
    let prefix_len = a.len().min(b.len());
    let prefix_matches = a[..prefix_len]
        .iter()
        .zip(&b[..prefix_len])
        .all(|(x, y)| x.same_kind(y) && !matches!(x, TransformOp::Matrix(_)));

    if prefix_matches {
        let max_len = a.len().max(b.len());
        let mut out = Vec::with_capacity(max_len);
        for i in 0..max_len {
            let op_a = a.get(i).copied().unwrap_or_else(|| b[i].identity_like());
            let op_b = b.get(i).copied().unwrap_or_else(|| a[i].identity_like());
            if let Some(mixed) = op_a.lerp(op_b, t) {
                out.push(mixed);
            }
        }
        return out;
    }

    // W3C CSS Transforms Level 1 §9.2 2D matrix polar decomposition fallback.
    let ma = ops_to_matrix(a);
    let mb = ops_to_matrix(b);
    vec![TransformOp::Matrix(lerp_matrix_2d(ma, mb, t))]
}

fn lerp_matrix_2d(ma: [f32; 6], mb: [f32; 6], t: f32) -> [f32; 6] {
    #[derive(Clone, Copy)]
    struct Decomp {
        tx: f32,
        ty: f32,
        sx: f32,
        sy: f32,
        angle: f32,
        m11: f32,
        m12: f32,
        m21: f32,
        m22: f32,
    }
    fn decompose(m: [f32; 6]) -> Decomp {
        let (mut a, mut b, mut c, mut d, tx, ty) = (m[0], m[1], m[2], m[3], m[4], m[5]);
        let mut sx = (a * a + b * b).sqrt();
        if sx > 1e-6 {
            a /= sx;
            b /= sx;
        }
        let skew = a * c + b * d;
        c -= a * skew;
        d -= b * skew;
        let mut sy = (c * c + d * d).sqrt();
        if sy > 1e-6 {
            c /= sy;
            d /= sy;
        }
        let det = a * d - b * c;
        if det < 0.0 {
            if sx < sy {
                sx = -sx;
                a = -a;
                b = -b;
            } else {
                sy = -sy;
                c = -c;
                d = -d;
            }
        }
        let angle = b.atan2(a).to_degrees();
        let rad = -angle.to_radians();
        let (rs, rc) = rad.sin_cos();
        Decomp {
            tx,
            ty,
            sx,
            sy,
            angle,
            m11: rc * a - rs * b,
            m12: rs * a + rc * b,
            m21: rc * c - rs * d,
            m22: rs * c + rc * d,
        }
    }

    let mut da = decompose(ma);
    let mut db = decompose(mb);
    if (da.sx < 0.0 && db.sy < 0.0) || (da.sy < 0.0 && db.sx < 0.0) {
        da.sx = -da.sx;
        da.sy = -da.sy;
        da.angle += if da.angle < 0.0 { 180.0 } else { -180.0 };
    }
    while db.angle - da.angle > 180.0 {
        db.angle -= 360.0;
    }
    while db.angle - da.angle < -180.0 {
        db.angle += 360.0;
    }
    let mix = |x: f32, y: f32| x + (y - x) * t;
    let tx = mix(da.tx, db.tx);
    let ty = mix(da.ty, db.ty);
    let sx = mix(da.sx, db.sx);
    let sy = mix(da.sy, db.sy);
    let angle = mix(da.angle, db.angle).to_radians();
    let (s, c) = angle.sin_cos();
    let rem = [
        mix(da.m11, db.m11),
        mix(da.m12, db.m12),
        mix(da.m21, db.m21),
        mix(da.m22, db.m22),
        0.0,
        0.0,
    ];
    mul_affine(
        [1.0, 0.0, 0.0, 1.0, tx, ty],
        mul_affine(
            [c, s, -s, c, 0.0, 0.0],
            mul_affine(rem, [sx, 0.0, 0.0, sy, 0.0, 0.0]),
        ),
    )
}

fn sample_smil(
    smil: &SmilAnim,
    clock: f32,
    dark: bool,
    current_rgba: [u8; 4],
) -> Option<SmilValue> {
    let active = clock - smil.begin;
    if active < 0.0 {
        return None;
    }
    if !smil.dur.is_finite() {
        return smil.values.first().cloned();
    }
    let raw_iter = active / smil.dur;
    let finished = match smil.iterations {
        Iterations::Infinite => false,
        Iterations::Count(total) => raw_iter >= total,
    };
    if finished {
        return if smil.freeze {
            smil.values.last().cloned()
        } else {
            None
        };
    }
    let u = (raw_iter - raw_iter.floor()).clamp(0.0, 1.0);
    if smil.discrete || smil.values.len() == 1 {
        let idx = ((u * smil.values.len() as f32).floor() as usize).min(smil.values.len() - 1);
        return Some(smil.values[idx].clone());
    }
    for i in 0..smil.values.len() - 1 {
        let t0 = smil.key_times[i];
        let t1 = smil.key_times[i + 1];
        if u >= t0 && u <= t1 {
            let local = ((u - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0);
            let eased = smil.splines.get(i).map_or(local, |e| e.sample(local));
            return Some(lerp_smil(
                &smil.values[i],
                &smil.values[i + 1],
                eased,
                dark,
                current_rgba,
            ));
        }
    }
    smil.values.last().cloned()
}

fn lerp_smil(a: &SmilValue, b: &SmilValue, t: f32, dark: bool, current_rgba: [u8; 4]) -> SmilValue {
    let mix = |x: f32, y: f32| x + (y - x) * t;
    match (a, b) {
        (&SmilValue::Scalar(x), &SmilValue::Scalar(y)) => SmilValue::Scalar(mix(x, y)),
        (&SmilValue::Pair(ax, ay), &SmilValue::Pair(bx, by)) => {
            SmilValue::Pair(mix(ax, bx), mix(ay, by))
        }
        (&SmilValue::Rotate(aa, pa), &SmilValue::Rotate(ba, pb)) => {
            let pivot = match (pa, pb) {
                (Some((ax, ay)), Some((bx, by))) => Some((mix(ax, bx), mix(ay, by))),
                (p, None) | (None, p) => p,
            };
            SmilValue::Rotate(mix(aa, ba), pivot)
        }
        (SmilValue::Color(ca), SmilValue::Color(cb)) => {
            match interpolate_anim_value(
                &AnimValue::Color(ca.clone()),
                &AnimValue::Color(cb.clone()),
                t,
                dark,
                current_rgba,
            ) {
                AnimValue::Color(c) => SmilValue::Color(c),
                _ => a.clone(),
            }
        }
        _ => {
            if t < 0.5 {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}

fn invert_affine(m: [f32; 6]) -> Option<[f32; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = 1.0 / det;
    let a = m[3] * inv;
    let b = -m[1] * inv;
    let c = -m[2] * inv;
    let d = m[0] * inv;
    let tx = -(a * m[4] + c * m[5]);
    let ty = -(b * m[4] + d * m[5]);
    Some([a, b, c, d, tx, ty])
}

/// Fast pixel conversion from `tiny-skia`'s native premultiplied RGBA8 buffer.
fn convert_pixels(src: &[u8], dst: &mut [u8], format: PixelFormat) {
    match format {
        PixelFormat::RgbaPremul => dst.copy_from_slice(src),
        PixelFormat::BgraPremul => {
            for (s, d) in src
                .as_chunks::<4>()
                .0
                .iter()
                .zip(dst.as_chunks_mut::<4>().0)
            {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            }
        }
        PixelFormat::RgbaStraight => {
            const RECIP: [u32; 256] = unpremul_table();
            for (s, d) in src
                .as_chunks::<4>()
                .0
                .iter()
                .zip(dst.as_chunks_mut::<4>().0)
            {
                let a = s[3];
                if a == 0 || a == 255 {
                    d.copy_from_slice(s);
                } else {
                    let mul = RECIP[a as usize];
                    d[0] = ((u32::from(s[0]) * mul + 32768) >> 16).min(255) as u8;
                    d[1] = ((u32::from(s[1]) * mul + 32768) >> 16).min(255) as u8;
                    d[2] = ((u32::from(s[2]) * mul + 32768) >> 16).min(255) as u8;
                    d[3] = a;
                }
            }
        }
        PixelFormat::BgraStraight => {
            const RECIP: [u32; 256] = unpremul_table();
            for (s, d) in src
                .as_chunks::<4>()
                .0
                .iter()
                .zip(dst.as_chunks_mut::<4>().0)
            {
                let a = s[3];
                if a == 0 {
                    d.copy_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = 255;
                } else {
                    let mul = RECIP[a as usize];
                    d[0] = ((u32::from(s[2]) * mul + 32768) >> 16).min(255) as u8;
                    d[1] = ((u32::from(s[1]) * mul + 32768) >> 16).min(255) as u8;
                    d[2] = ((u32::from(s[0]) * mul + 32768) >> 16).min(255) as u8;
                    d[3] = a;
                }
            }
        }
    }
}

const fn unpremul_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut a = 1usize;
    while a < 256 {
        table[a] = ((255u32 << 16) + (a as u32 / 2)) / (a as u32);
        a += 1;
    }
    table
}
