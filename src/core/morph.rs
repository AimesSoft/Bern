//! 矢量形变底层 (vector morph engine).
//!
//! 这个模块是所有控件共用的「图标形变」基础：
//!
//! 1. 用 `ttf-parser` 从内嵌的 Material Icons 字体里提取字形轮廓
//!    （CFF/二次/三次贝塞尔全部采样成折线）；
//! 2. 按弧长均匀重采样每个轮廓，归一化到 24×24 图标网格；
//! 3. 两个字形之间做逐点插值，配合带过冲的果冻缓动，
//!    就得到「图标 1 果冻扭曲成图标 2」的矢量形变动画。
//!
//! 轮廓数量不一致时（例如月牙 1 个轮廓 → 太阳 9 个轮廓），缺失的
//! 轮廓会从对方字形的质心“生长”/“收缩”，所以任意两个图标都能形变。

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use ttf_parser::Face;

/// 图标设计网格边长（与 Material Icons 的 24 网格一致）。
pub const GLYPH_SIZE: f32 = 24.0;

/// 每个轮廓重采样后的点数（弧长均匀采样）。
pub const POINTS_PER_CONTOUR: usize = 64;

/// 一个闭合折线轮廓（归一化坐标：0..24，Y 轴向下，居中）。
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    /// 轮廓顶点，首尾隐含相连。
    pub points: Vec<[f32; 2]>,
}

/// 一个完整的字形：由若干按面积降序排列的轮廓组成。
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphShape {
    /// 轮廓，按面积从大到小排序（保证形变时大轮廓优先配对）。
    pub contours: Vec<Contour>,
}

impl GlyphShape {
    /// 所有轮廓的质心（用于缺失轮廓的生长/收缩锚点）。
    fn centroid(&self) -> [f32; 2] {
        let mut sum = [0.0_f64; 2];
        let mut count = 0.0_f64;
        for contour in &self.contours {
            for point in &contour.points {
                sum[0] += f64::from(point[0]);
                sum[1] += f64::from(point[1]);
                count += 1.0;
            }
        }
        if count == 0.0 {
            [GLYPH_SIZE / 2.0, GLYPH_SIZE / 2.0]
        } else {
            [(sum[0] / count) as f32, (sum[1] / count) as f32]
        }
    }

    /// 返回一个“退化轮廓”（所有点都叠在质心上），用来配对缺失轮廓。
    fn degenerate_contour(&self) -> Contour {
        let center = self.centroid();
        Contour {
            points: vec![center; POINTS_PER_CONTOUR],
        }
    }
}

/// 字形提取缓存：同一字符只解析一次。
static SHAPE_CACHE: LazyLock<Mutex<HashMap<char, Option<Arc<GlyphShape>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 内嵌的 Material Icons 字体字节。
static FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/MaterialIcons-Regular.otf");

/// 提取并缓存字符 `ch` 的字形轮廓。
///
/// 解析失败（字体里没有该字形）返回 `None`。
pub fn glyph_shape(ch: char) -> Option<Arc<GlyphShape>> {
    if let Some(cached) = SHAPE_CACHE.lock().unwrap().get(&ch) {
        return cached.clone();
    }

    let shape = extract_shape(ch);
    let mut cache = SHAPE_CACHE.lock().unwrap();
    if let Some(cached) = cache.get(&ch) {
        return cached.clone();
    }
    cache.insert(ch, shape.clone());
    shape
}

/// 从字体提取 + 归一化 + 重采样的完整流程。
fn extract_shape(ch: char) -> Option<Arc<GlyphShape>> {
    let face = Face::parse(FONT_BYTES, 0).ok()?;
    let glyph_id = face.glyph_index(ch)?;
    let mut builder = RawOutline::default();
    face.outline_glyph(glyph_id, &mut builder)?;
    if builder.contours.is_empty() {
        return None;
    }

    let normalized = normalize(builder.contours);
    let mut contours: Vec<Contour> = normalized
        .into_iter()
        .map(|points| Contour {
            points: resample_closed(&points, POINTS_PER_CONTOUR),
        })
        .collect();

    // 丢掉面积过小的噪声轮廓，并按面积降序排序。
    contours.retain(|contour| signed_area(&contour.points).abs() > 0.4);
    contours.sort_by(|a, b| {
        signed_area(&b.points)
            .abs()
            .total_cmp(&signed_area(&a.points).abs())
    });
    if contours.is_empty() {
        return None;
    }
    Some(Arc::new(GlyphShape { contours }))
}

/// 从字体原始坐标（Y 向上）归一化到 0..24（Y 向下、居中、保持比例）。
fn normalize(raw: Vec<Vec<[f32; 2]>>) -> Vec<Vec<[f32; 2]>> {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for contour in &raw {
        for point in contour {
            min[0] = min[0].min(point[0]);
            min[1] = min[1].min(point[1]);
            max[0] = max[0].max(point[0]);
            max[1] = max[1].max(point[1]);
        }
    }
    let width = (max[0] - min[0]).max(1.0);
    let height = (max[1] - min[1]).max(1.0);
    let scale = (GLYPH_SIZE / width).min(GLYPH_SIZE / height);
    let cx = (min[0] + max[0]) / 2.0;
    let cy = (min[1] + max[1]) / 2.0;

    raw.into_iter()
        .map(|contour| {
            contour
                .into_iter()
                .map(|[x, y]| {
                    [
                        GLYPH_SIZE / 2.0 + (x - cx) * scale,
                        GLYPH_SIZE / 2.0 - (y - cy) * scale,
                    ]
                })
                .collect()
        })
        .collect()
}

/// 按弧长把闭合折线均匀重采样成 `n` 个点。
fn resample_closed(points: &[[f32; 2]], n: usize) -> Vec<[f32; 2]> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut lengths = Vec::with_capacity(points.len());
    let mut total = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        lengths.push(d);
        total += d;
    }
    if total <= 1e-6 {
        return vec![points[0]; n];
    }

    let step = total / n as f32;
    let mut out = Vec::with_capacity(n);
    let mut seg = 0usize;
    let mut acc = 0.0;
    for k in 0..n {
        let want = step * k as f32;
        while acc + lengths[seg] < want {
            acc += lengths[seg];
            seg = (seg + 1) % lengths.len();
        }
        let a = points[seg];
        let b = points[(seg + 1) % points.len()];
        let seg_len = lengths[seg];
        let t = if seg_len <= 1e-9 {
            0.0
        } else {
            ((want - acc) / seg_len).clamp(0.0, 1.0)
        };
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
    out
}

/// 鞋带公式：有符号面积（Y 向下时顺逆时针会反号，用绝对值）。
fn signed_area(points: &[[f32; 2]]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a[0] * b[1] - b[0] * a[1];
    }
    area / 2.0
}

/// 果冻缓动：easeOutBack 的过冲 + 衰减的正弦抖动，t=0 返回 0，t=1 返回 1。
///
/// 中间过程会超过 1.0（过冲）并小幅回弹，正是「果冻扭曲」的节奏感。
pub fn jelly(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let u = t - 1.0;
    // easeOutBack: 过冲到 ~1.1 再回落。
    let back = 1.0 + 2.70158 * u * u * u + 1.70158 * u * u;
    // 衰减正弦抖动：两圈半摆动，幅度随时间衰减到 0。
    let wobble = 0.055 * (1.0 - t) * (t * std::f32::consts::TAU * 2.5).sin();
    back + wobble
}

/// 一次从 `from` 字形到 `to` 字形的形变。
#[derive(Clone, Debug)]
pub struct Morph {
    from: Arc<GlyphShape>,
    to: Arc<GlyphShape>,
}

impl Morph {
    /// 创建形变。任一字形解析失败则返回 `None`。
    pub fn new(from: char, to: char) -> Option<Self> {
        Some(Self {
            from: glyph_shape(from)?,
            to: glyph_shape(to)?,
        })
    }

    /// 按插值进度 `t`（0..1，通常传入 [`jelly`] 缓动后的值）取中间轮廓。
    ///
    /// 轮廓按索引配对；不足的一方用对方质心的退化轮廓补齐，
    /// 因此任意两个图标（哪怕轮廓数量不同）都能形变。
    pub fn interpolate(&self, t: f32) -> Vec<Contour> {
        let count = self.from.contours.len().max(self.to.contours.len());
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let a = self
                .from
                .contours
                .get(i)
                .cloned()
                .unwrap_or_else(|| self.to.degenerate_contour());
            let b = self
                .to
                .contours
                .get(i)
                .cloned()
                .unwrap_or_else(|| self.from.degenerate_contour());
            let points = a
                .points
                .iter()
                .zip(&b.points)
                .map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t])
                .collect();
            out.push(Contour { points });
        }
        out
    }
}

/// 从 ttf-parser 回调里累积的原始轮廓（字体坐标，Y 向上）。
#[derive(Default)]
struct RawOutline {
    contours: Vec<Vec<[f32; 2]>>,
    current: Vec<[f32; 2]>,
    cursor: [f32; 2],
    started: bool,
}

impl RawOutline {
    /// 曲线采样步数（二次/三次贝塞尔各按固定步数细分）。
    const QUAD_STEPS: usize = 10;
    const CUBIC_STEPS: usize = 14;

    fn push_contour(&mut self) {
        if self.started && !self.current.is_empty() {
            self.contours.push(std::mem::take(&mut self.current));
            self.started = false;
        }
    }
}

impl ttf_parser::OutlineBuilder for RawOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.push_contour();
        self.current.push([x, y]);
        self.cursor = [x, y];
        self.started = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if self.started {
            self.current.push([x, y]);
            self.cursor = [x, y];
        }
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        if !self.started {
            return;
        }
        let (x0, y0) = (self.cursor[0], self.cursor[1]);
        for i in 1..=Self::QUAD_STEPS {
            let t = i as f32 / Self::QUAD_STEPS as f32;
            let a = (1.0 - t) * (1.0 - t);
            let b = 2.0 * (1.0 - t) * t;
            let c = t * t;
            self.current
                .push([a * x0 + b * x1 + c * x, a * y0 + b * y1 + c * y]);
        }
        self.cursor = [x, y];
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        if !self.started {
            return;
        }
        let (x0, y0) = (self.cursor[0], self.cursor[1]);
        for i in 1..=Self::CUBIC_STEPS {
            let t = i as f32 / Self::CUBIC_STEPS as f32;
            let a = (1.0 - t) * (1.0 - t) * (1.0 - t);
            let b = 3.0 * (1.0 - t) * (1.0 - t) * t;
            let c = 3.0 * (1.0 - t) * t * t;
            let d = t * t * t;
            self.current.push([
                a * x0 + b * x1 + c * x2 + d * x,
                a * y0 + b * y1 + c * y2 + d * y,
            ]);
        }
        self.cursor = [x, y];
    }

    fn close(&mut self) {
        self.push_contour();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_known_glyph_into_normalized_box() {
        let shape = glyph_shape('\u{f852}').expect("light_mode_rounded");
        // 太阳 = 外圆 + 8 条光线。
        assert_eq!(shape.contours.len(), 9);
        for contour in &shape.contours {
            assert_eq!(contour.points.len(), POINTS_PER_CONTOUR);
            for point in &contour.points {
                assert!(point[0] >= 0.0 && point[0] <= GLYPH_SIZE);
                assert!(point[1] >= 0.0 && point[1] <= GLYPH_SIZE);
            }
        }
        // 面积降序：最大的轮廓（外圆）在前。
        let areas: Vec<f32> = shape
            .contours
            .iter()
            .map(|c| signed_area(&c.points).abs())
            .collect();
        assert!(areas.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn crescent_has_single_contour() {
        let shape = glyph_shape('\u{f68c}').expect("dark_mode_rounded");
        assert_eq!(shape.contours.len(), 1);
    }

    #[test]
    fn morph_ends_match_source_and_target() {
        let morph = Morph::new('\u{f852}', '\u{f68c}').expect("morph");
        let from = glyph_shape('\u{f852}').unwrap();
        let to = glyph_shape('\u{f68c}').unwrap();

        // 太阳 9 轮廓 → 月牙 1 轮廓，按最大轮廓数生成中间帧。
        let at_start = morph.interpolate(0.0);
        let at_end = morph.interpolate(1.0);
        assert_eq!(at_start.len(), from.contours.len());
        assert_eq!(at_end.len(), from.contours.len());

        // 逐点插值（t=0 取 p + (q-p)*0，t=1 取 p + (q-p)*1，浮点有少量噪声）。
        let close =
            |a: &[f32; 2], b: &[f32; 2]| (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4;
        assert!(
            at_start[0]
                .points
                .iter()
                .zip(&from.contours[0].points)
                .all(|(a, b)| close(a, b))
        );
        assert!(
            at_end[0]
                .points
                .iter()
                .zip(&to.contours[0].points)
                .all(|(a, b)| close(a, b))
        );

        // 月牙独有的轮廓不存在，太阳独有的 8 条光线在 t=1 收拢到太阳
        // 质心，形成零面积退化轮廓（绘制时不可见）。
        let is_degenerate = |contour: &Contour| {
            contour
                .points
                .iter()
                .all(|point| *point == contour.points[0])
        };
        assert!(at_end[to.contours.len()..].iter().all(is_degenerate));
    }

    #[test]
    fn jelly_easing_overshoots_and_settles() {
        assert_eq!(jelly(0.0), 0.0);
        assert_eq!(jelly(1.0), 1.0);
        let mid = jelly(0.5);
        assert!(mid > 1.0, "jelly must overshoot past 1.0, got {mid}");
    }

    #[test]
    fn resample_keeps_arc_length() {
        let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let sampled = resample_closed(&square, 32);
        assert_eq!(sampled.len(), 32);
        // 周长 40，重采样后相邻点距应为 40/32。
        let mut perimeter = 0.0;
        for i in 0..sampled.len() {
            let a = sampled[i];
            let b = sampled[(i + 1) % sampled.len()];
            perimeter += ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        }
        assert!((perimeter - 40.0).abs() < 1e-3);
    }
}
