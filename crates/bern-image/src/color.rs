//! Transfer functions, primaries and ICC profiles.

/// RGB to XYZ, or any 3×3 color matrix.
pub(crate) type Mat3 = [[f32; 3]; 3];

/// The RGB primaries of decoded pixels. White is D65 for all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primaries {
    /// BT.709 and sRGB; scRGB on Windows.
    Bt709,
    /// Display P3.
    DisplayP3,
    /// BT.2020 and BT.2100.
    Bt2020,
}

impl Primaries {
    /// The cICP / H.273 `ColourPrimaries` code, if Bern knows it.
    pub(crate) fn from_cicp(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Bt709),
            9 => Some(Self::Bt2020),
            12 => Some(Self::DisplayP3),
            _ => None,
        }
    }

    /// RGB (linear) to XYZ with a D65 white.
    pub(crate) fn to_xyz(self) -> Mat3 {
        let (r, g, b) = match self {
            Self::Bt709 => ((0.640, 0.330), (0.300, 0.600), (0.150, 0.060)),
            Self::DisplayP3 => ((0.680, 0.320), (0.265, 0.690), (0.150, 0.060)),
            Self::Bt2020 => ((0.708, 0.292), (0.170, 0.797), (0.131, 0.046)),
        };
        from_chromaticities(r, g, b, (0.3127, 0.3290))
    }
}

/// RGB to XYZ from the chromaticities of the primaries and the white.
fn from_chromaticities(r: (f32, f32), g: (f32, f32), b: (f32, f32), w: (f32, f32)) -> Mat3 {
    let column = |(x, y): (f32, f32)| [x / y, 1.0, (1.0 - x - y) / y];
    let (cr, cg, cb) = (column(r), column(g), column(b));
    let primaries = [
        [cr[0], cg[0], cb[0]],
        [cr[1], cg[1], cb[1]],
        [cr[2], cg[2], cb[2]],
    ];
    let white = column(w);
    let s = apply(&invert(&primaries), white);
    let mut m = primaries;
    for row in &mut m {
        for (value, scale) in row.iter_mut().zip(s) {
            *value *= scale;
        }
    }
    m
}

pub(crate) fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

pub(crate) fn invert(m: &Mat3) -> Mat3 {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    let s = 1.0 / det;
    [
        [
            (e * i - f * h) * s,
            (c * h - b * i) * s,
            (b * f - c * e) * s,
        ],
        [
            (f * g - d * i) * s,
            (a * i - c * g) * s,
            (c * d - a * f) * s,
        ],
        [
            (d * h - e * g) * s,
            (b * g - a * h) * s,
            (a * e - b * d) * s,
        ],
    ]
}

pub(crate) fn apply(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// RGB with the source's RGB-to-XYZ matrix to RGB in `to`.
pub(crate) fn conversion(from_xyz: &Mat3, to: Primaries) -> Mat3 {
    mul(&invert(&to.to_xyz()), from_xyz)
}

/// sRGB code value to linear light.
#[cfg(test)]
pub(crate) fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to an sRGB code value.
pub(crate) fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Nits of SDR reference white in PQ and HLG content (BT.2408).
pub(crate) const REFERENCE_WHITE: f32 = 203.0;

/// PQ (SMPTE ST 2084) code value to linear light, 1.0 at reference white.
pub(crate) fn pq_to_linear(v: f32) -> f32 {
    const M1: f32 = 2610.0 / 16384.0;
    const M2: f32 = 2523.0 / 4096.0 * 128.0;
    const C1: f32 = 3424.0 / 4096.0;
    const C2: f32 = 2413.0 / 4096.0 * 32.0;
    const C3: f32 = 2392.0 / 4096.0 * 32.0;
    let p = v.max(0.0).powf(1.0 / M2);
    let nits = ((p - C1).max(0.0) / (C2 - C3 * p)).powf(1.0 / M1) * 10000.0;
    nits / REFERENCE_WHITE
}

/// HLG code values to linear display light, 1.0 at reference white, for a
/// 1000-nit display (BT.2100 with system gamma 1.2). `luma` gives the
/// luminance weights of the signal's primaries.
pub(crate) fn hlg_to_linear(rgb: [f32; 3], luma: [f32; 3]) -> [f32; 3] {
    const A: f32 = 0.178_832_77;
    const B: f32 = 0.284_668_92;
    const C: f32 = 0.559_910_7;
    let scene = rgb.map(|v| {
        let v = v.max(0.0);
        if v <= 0.5 {
            v * v / 3.0
        } else {
            (((v - C) / A).exp() + B) / 12.0
        }
    });
    let ys = luma[0] * scene[0] + luma[1] * scene[1] + luma[2] * scene[2];
    let scale = 1000.0 / REFERENCE_WHITE * ys.max(0.0).powf(0.2);
    scene.map(|v| v * scale)
}

/// The tone curve of one channel of an ICC profile.
#[derive(Clone, Debug)]
pub(crate) enum Curve {
    Gamma(f32),
    /// ICC parametric curve: g, a, b, c, d, e, f (function type 4 form).
    Parametric([f32; 7]),
    Table(Vec<f32>),
}

impl Curve {
    pub(crate) fn linear(&self, v: f32) -> f32 {
        match self {
            Self::Gamma(g) => v.max(0.0).powf(*g),
            Self::Parametric([g, a, b, c, d, e, f]) => {
                if v >= *d {
                    (a * v + b).max(0.0).powf(*g) + e
                } else {
                    c * v + f
                }
            }
            Self::Table(table) => {
                let position = v.clamp(0.0, 1.0) * (table.len() - 1) as f32;
                let i = (position as usize).min(table.len() - 2);
                let t = position - i as f32;
                table[i] * (1.0 - t) + table[i + 1] * t
            }
        }
    }
}

/// What Bern uses of an RGB ICC profile: its matrix and its curves.
#[derive(Clone, Debug)]
pub(crate) struct Icc {
    /// Linear RGB to XYZ with a D65 white.
    pub to_xyz: Mat3,
    pub curves: [Curve; 3],
}

impl Icc {
    /// sRGB, for pictures without a profile.
    pub(crate) fn srgb() -> Self {
        let curve = Curve::Parametric([
            2.4,
            1.0 / 1.055,
            0.055 / 1.055,
            1.0 / 12.92,
            0.04045,
            0.0,
            0.0,
        ]);
        Self {
            to_xyz: Primaries::Bt709.to_xyz(),
            curves: [curve.clone(), curve.clone(), curve],
        }
    }

    /// A matrix/TRC RGB profile; `None` for anything else, such as LUT
    /// profiles, which the caller then treats as sRGB.
    pub(crate) fn parse(data: &[u8]) -> Option<Self> {
        let u32_at = |at: usize| {
            data.get(at..at + 4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        };
        if data.get(16..20)? != b"RGB " {
            return None;
        }
        let count = u32_at(128)? as usize;
        let tag = |signature: &[u8; 4]| -> Option<&[u8]> {
            (0..count.min(256)).find_map(|i| {
                let entry = 132 + i * 12;
                if data.get(entry..entry + 4)? != signature {
                    return None;
                }
                let offset = u32_at(entry + 4)? as usize;
                let size = u32_at(entry + 8)? as usize;
                data.get(offset..offset.checked_add(size)?)
            })
        };
        let s15 = |bytes: &[u8], at: usize| -> Option<f32> {
            let b = bytes.get(at..at + 4)?;
            Some(i32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f32 / 65536.0)
        };
        let xyz = |signature: &[u8; 4]| -> Option<[f32; 3]> {
            let t = tag(signature)?;
            (t.get(0..4)? == b"XYZ ").then_some(())?;
            Some([s15(t, 8)?, s15(t, 12)?, s15(t, 16)?])
        };
        let curve = |signature: &[u8; 4]| -> Option<Curve> {
            let t = tag(signature)?;
            match t.get(0..4)? {
                b"curv" => {
                    let n = u32::from_be_bytes(t.get(8..12)?.try_into().ok()?) as usize;
                    let u16_at = |i: usize| -> Option<u16> {
                        let b = t.get(12 + i * 2..14 + i * 2)?;
                        Some(u16::from_be_bytes([b[0], b[1]]))
                    };
                    match n {
                        0 => Some(Curve::Gamma(1.0)),
                        1 => Some(Curve::Gamma(u16_at(0)? as f32 / 256.0)),
                        _ => (0..n)
                            .map(|i| u16_at(i).map(|v| v as f32 / 65535.0))
                            .collect::<Option<Vec<_>>>()
                            .map(Curve::Table),
                    }
                }
                b"para" => {
                    let kind = u16::from_be_bytes(t.get(8..10)?.try_into().ok()?);
                    let p = |i: usize| s15(t, 12 + i * 4);
                    // Every function type in the type 4 form, `c * v + f`
                    // below `d`, `(a * v + b) ^ g + e` above it.
                    Some(Curve::Parametric(match kind {
                        0 => [p(0)?, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        1 => {
                            let (g, a, b) = (p(0)?, p(1)?, p(2)?);
                            [g, a, b, 0.0, -b / a, 0.0, 0.0]
                        }
                        2 => {
                            let (g, a, b, c) = (p(0)?, p(1)?, p(2)?, p(3)?);
                            [g, a, b, 0.0, -b / a, c, c]
                        }
                        3 => [p(0)?, p(1)?, p(2)?, p(3)?, p(4)?, 0.0, 0.0],
                        4 => [p(0)?, p(1)?, p(2)?, p(3)?, p(4)?, p(5)?, p(6)?],
                        _ => return None,
                    }))
                }
                _ => None,
            }
        };
        let (r, g, b) = (xyz(b"rXYZ")?, xyz(b"gXYZ")?, xyz(b"bXYZ")?);
        let d50 = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
        // ICC colorants are adapted to D50; Bradford back to D65.
        const D50_TO_D65: Mat3 = [
            [0.955_576_6, -0.023_039_3, 0.063_163_6],
            [-0.028_289_5, 1.009_941_6, 0.021_007_7],
            [0.012_298_2, -0.020_483, 1.329_909_8],
        ];
        Some(Self {
            to_xyz: mul(&D50_TO_D65, &d50),
            curves: [curve(b"rTRC")?, curve(b"gTRC")?, curve(b"bTRC")?],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_functions_meet_reference_points() {
        assert!((pq_to_linear(0.0)).abs() < 1e-6);
        // PQ code 0.58 is about 203 nits.
        assert!(
            (pq_to_linear(0.580_689) - 1.0).abs() < 0.01,
            "{}",
            pq_to_linear(0.580_689)
        );
        assert!((pq_to_linear(1.0) - 10000.0 / 203.0).abs() < 0.5);
        // HLG 75% grey is reference white on a 1000-nit display.
        let luma = [0.2627, 0.6780, 0.0593];
        let white = hlg_to_linear([0.75; 3], luma);
        assert!((white[0] - 1.0).abs() < 0.03, "{white:?}");
        assert!((srgb_to_linear(linear_to_srgb(0.25)) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn primaries_convert_between_each_other() {
        let to_709 = conversion(&Primaries::Bt2020.to_xyz(), Primaries::Bt709);
        // BT.2020 white stays white; its red is outside BT.709.
        let white = apply(&to_709, [1.0; 3]);
        assert!(white.iter().all(|v| (v - 1.0).abs() < 1e-3), "{white:?}");
        let red = apply(&to_709, [1.0, 0.0, 0.0]);
        assert!(red[0] > 1.0 && red[1] < 0.0, "{red:?}");
        let identity = conversion(&Icc::srgb().to_xyz, Primaries::Bt709);
        for (i, row) in identity.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                assert!((v - if i == j { 1.0 } else { 0.0 }).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn curves_follow_srgb() {
        let srgb = Icc::srgb();
        for v in [0.0, 0.02, 0.5, 1.0] {
            assert!((srgb.curves[0].linear(v) - srgb_to_linear(v)).abs() < 1e-5);
        }
        let table = Curve::Table(vec![0.0, 0.25, 1.0]);
        assert!((table.linear(0.25) - 0.125).abs() < 1e-6);
    }
}
